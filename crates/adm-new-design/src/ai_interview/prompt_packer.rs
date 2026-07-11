use std::collections::BTreeMap;

use adm_new_contracts::ai::{AiInterviewState, MDA_STAGES};
use adm_new_contracts::project::{DecisionState, NodeState, ProjectState};
use adm_new_foundation::sha256_hex;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value, json};

use crate::{DesignEngineService, DesignNodeSpec};

use super::{
    CANDIDATE_NODE_LIMIT, CANDIDATE_NODE_MIN_LIMIT, InterviewOutputMode, InterviewSchemaMode,
    OUTPUT_PARTITION_CANDIDATE_COUNTS, OUTPUT_PARTITION_PROMPT_BUDGET, PROMPT_CHAR_BUDGET_TURN,
    RECENT_MESSAGE_LIMIT_FULL, RECENT_MESSAGE_LIMIT_TURN,
    route_planner::{
        candidate_node_ids, detect_force_output, first_domain_ids, focus_domain_ids, humanize_id,
        router_decision_payload, update_route_overview,
    },
    state::{mda_progress_for_count, normalize_interview_state, recent_messages, short_text},
};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PromptBuildOptions {
    pub turn_id: String,
    pub framework_version: String,
    pub manifest_hash: String,
    pub force_output: bool,
    pub force_readiness_check: bool,
    pub store_full_prompt: bool,
    pub memory_signals: Vec<Value>,
}

impl Default for PromptBuildOptions {
    fn default() -> Self {
        Self {
            turn_id: String::new(),
            framework_version: String::new(),
            manifest_hash: String::new(),
            force_output: false,
            force_readiness_check: false,
            store_full_prompt: false,
            memory_signals: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PromptBuildResult {
    pub turn_id: String,
    pub schema_mode: InterviewSchemaMode,
    pub output_mode: InterviewOutputMode,
    pub prompt_text: String,
    pub prompt_payload: Value,
    pub meter: PromptMeterEntry,
    pub replay: Value,
    pub degradations: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PromptMeterEntry {
    pub turn_id: String,
    pub schema_mode: String,
    pub output_mode: String,
    pub prompt_chars: usize,
    pub prompt_estimated_tokens: usize,
    pub section_chars: BTreeMap<String, usize>,
    pub degradations: Vec<String>,
    pub response_schema_empty_fields_removed: Vec<String>,
    pub codex_session_id: String,
    pub session_accumulated_turns: u32,
    pub question_group_count: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OutputPartitionPlan {
    pub partitions: Vec<Vec<String>>,
    pub prompt_budget: usize,
    pub prompt_sizes: Vec<usize>,
    pub max_prompt_chars: usize,
    pub avg_prompt_chars: usize,
}

pub fn compact_json(value: &Value) -> String {
    match value {
        Value::Null => "null".to_string(),
        Value::Bool(value) => value.to_string(),
        Value::Number(value) => value.to_string(),
        Value::String(value) => serde_json::to_string(value).unwrap_or_else(|_| "\"\"".to_string()),
        Value::Array(items) => {
            let body = items.iter().map(compact_json).collect::<Vec<_>>().join(",");
            format!("[{body}]")
        }
        Value::Object(object) => {
            let mut keys = object.keys().collect::<Vec<_>>();
            keys.sort();
            let body = keys
                .into_iter()
                .map(|key| {
                    format!(
                        "{}:{}",
                        serde_json::to_string(key).unwrap_or_else(|_| "\"\"".to_string()),
                        compact_json(object.get(key).unwrap_or(&Value::Null))
                    )
                })
                .collect::<Vec<_>>()
                .join(",");
            format!("{{{body}}}")
        }
    }
}

pub fn stable_hash(value: &Value) -> String {
    sha256_hex(compact_json(value).as_bytes())
}

pub fn prompt_section_sizes(prompt_payload: &Value) -> BTreeMap<String, usize> {
    prompt_payload
        .as_object()
        .map(|object| {
            object
                .iter()
                .map(|(key, value)| (key.clone(), compact_json(value).chars().count()))
                .collect()
        })
        .unwrap_or_default()
}

pub fn prompt_replay_fields(prompt_text: &str, preview_limit: usize, store_full: bool) -> Value {
    let mut fields = Map::new();
    fields.insert(
        "packedPromptSha256".to_string(),
        Value::String(sha256_hex(prompt_text.as_bytes())),
    );
    fields.insert(
        "packedPromptChars".to_string(),
        json!(prompt_text.chars().count()),
    );
    fields.insert(
        "packedPromptPreview".to_string(),
        Value::String(prompt_text.chars().take(preview_limit).collect()),
    );
    if store_full {
        fields.insert(
            "packedPrompt".to_string(),
            Value::String(prompt_text.to_string()),
        );
    }
    Value::Object(fields)
}

pub fn build_prompt_text(prompt_snapshot: &Value, prompt_payload: &Value) -> String {
    format!(
        "{}\n\n请基于下面 JSON 上下文继续同一个 AI 访谈线程，并只返回符合 output schema 的 JSON。\n\n{}",
        compact_prompt_prefix(prompt_snapshot),
        compact_json(prompt_payload)
    )
}

pub fn compact_prompt_prefix(prompt_snapshot: &Value) -> String {
    [
        "以下为当前锁定的 AI 提问提示词框架。它只约束 AI 如何提问、解释、映射和判断置信度；不得修改设计选项框架。".to_string(),
        format!(
            "frameworkVersion: {}",
            prompt_snapshot
                .get("frameworkVersion")
                .and_then(Value::as_str)
                .unwrap_or_default()
        ),
        format!(
            "manifestHash: {}",
            prompt_snapshot
                .get("manifestHash")
                .and_then(Value::as_str)
                .unwrap_or_default()
        ),
        "规则、项目摘要、候选节点和本轮输出 schema 见下方 JSON；只返回符合 schema 的 JSON。".to_string(),
    ]
    .join("\n")
}

pub fn build_interview_prompt(
    engine: &DesignEngineService,
    project_state: &mut ProjectState,
    user_text: &str,
    options: PromptBuildOptions,
) -> PromptBuildResult {
    normalize_interview_state(&mut project_state.ai_interview);
    update_route_overview(engine, project_state);
    let include_full = options.force_output || detect_force_output(user_text);
    let schema_mode = if include_full {
        InterviewSchemaMode::FullOutput
    } else if options.force_readiness_check {
        InterviewSchemaMode::Readiness
    } else {
        InterviewSchemaMode::Turn
    };
    let output_mode = if include_full {
        InterviewOutputMode::FullProjectOutput
    } else {
        InterviewOutputMode::InterviewTurn
    };
    let turn_id = if options.turn_id.trim().is_empty() {
        "turn_prompt".to_string()
    } else {
        options.turn_id.clone()
    };
    project_state.ai_interview.summary.v1.mda_progress =
        mda_progress_for_count(project_state.ai_interview.question_group_count);
    let mut candidate_ids = if include_full {
        Vec::new()
    } else {
        candidate_node_ids(engine, project_state, user_text, CANDIDATE_NODE_LIMIT)
    };
    let mut degradations = Vec::<String>::new();
    let prompt_snapshot = prompt_snapshot(&options);
    let mut prompt_payload = json!({
        "turnId": turn_id,
        "task": "commercial_game_design_ai_interview",
        "promptFramework": {
            "snapshot": prompt_snapshot,
            "rules": prompt_rules(),
            "visibility": "hidden_to_user",
            "designOptionFrameworkMutation": "forbidden"
        },
        "schemaMode": schema_mode.as_str(),
        "outputMode": output_mode.as_str(),
        "sessionPolicy": {
            "mode": if include_full { "full_output_turn" } else { "stateless_fast_turn" },
            "sourceOfTruth": "aiInterview.summary.v1 + recentMessages + projectDigest",
            "doNotRelyOnHiddenSessionMemory": true
        },
        "projectSummary": compact_project_summary(engine, project_state),
        "projectDigest": project_digest(engine, project_state, false),
        "conversationSummary": serde_json::to_value(&project_state.ai_interview.summary.v1).unwrap_or_else(|_| json!({})),
        "routeOverview": serde_json::to_value(&project_state.ai_interview.route_overview).unwrap_or_else(|_| json!({})),
        "routerDecision": router_decision_payload(engine, &candidate_ids, &degradations),
        "questionGroupCount": project_state.ai_interview.question_group_count,
        "evaluationBatchId": project_state.ai_interview.framework_memory.evaluation_batch_id,
        "projectMemoryId": project_state.ai_interview.framework_memory.project_memory_id,
        "forceReadinessCheck": options.force_readiness_check,
        "memoryInfluence": memory_influence(&options),
        "recentMessages": recent_messages(&project_state.ai_interview, if include_full { RECENT_MESSAGE_LIMIT_FULL } else { RECENT_MESSAGE_LIMIT_TURN }),
        "frameworkContext": framework_context(engine, project_state, include_full, &candidate_ids, CANDIDATE_NODE_LIMIT),
        "userMessage": user_text,
    });
    if include_full {
        prompt_payload["currentProjectState"] =
            serde_json::to_value(project_state.clone()).unwrap_or_else(|_| json!({}));
        prompt_payload["fullOutputRequirements"] = json!([
            "必须返回 fullProjectOutput.projectStateJson，值是 JSON 字符串，解析后结构与 currentProjectState 相同。",
            "必须返回 fullProjectOutput.confidenceMapJson，值是 JSON 字符串，解析后至少包含 groups 或 nodes 置信度。",
            "只把高置信设计作为候选写入；低置信内容留在 inferences 里继续澄清。",
            "optionDifferences 说明当前项目与 AI 全项目输出的选项差异。",
            "For nodes with roleClass system_concrete/content_concrete, write L5 cards to node.designEntities only when required fields in designEntityPrompt.allowedSchemas are present and confidenceMap.nodes[nodeId] >= 0.75."
        ]);
    } else {
        let mut requirements = vec![
            "如果需要追问，返回 mode=question_group 和 questionGroup。",
            "如果已经接近可生成，返回 mode=readiness_check 并询问是否输出。",
            "普通追问轮次不需要强行完成完整选项映射；证据不足时 inferences 可以为空数组。",
            "如果用户自然语言纠偏，先确认重排路线，不要让用户手动选节点。",
        ];
        if options.force_readiness_check {
            requirements.insert(
                0,
                "本轮是工具侧生成就绪检查点，必须返回 mode=readiness_check。",
            );
        }
        prompt_payload["turnRequirements"] = json!(requirements);
    }

    let mut prompt_text = build_prompt_text(&prompt_snapshot, &prompt_payload);
    if !include_full && prompt_text.chars().count() > PROMPT_CHAR_BUDGET_TURN {
        if let Some(signals) = prompt_payload
            .get_mut("memoryInfluence")
            .and_then(|value| value.get_mut("signals"))
            .and_then(Value::as_array_mut)
        {
            if signals.len() > 1 {
                signals.truncate(1);
                prompt_payload["memoryInfluence"]["policy"] = json!("budget_top_1");
                degradations.push("memoryInfluence:top3_to_top1".to_string());
            }
        }
    }
    prompt_text = build_prompt_text(&prompt_snapshot, &prompt_payload);
    if !include_full && prompt_text.chars().count() > PROMPT_CHAR_BUDGET_TURN {
        prompt_payload["recentMessages"] = json!(recent_messages(&project_state.ai_interview, 4));
        degradations.push("recentMessages:limit_to_4".to_string());
    }
    prompt_text = build_prompt_text(&prompt_snapshot, &prompt_payload);
    if !include_full
        && prompt_text.chars().count() > PROMPT_CHAR_BUDGET_TURN
        && candidate_ids.len() > CANDIDATE_NODE_MIN_LIMIT
    {
        candidate_ids.truncate(CANDIDATE_NODE_MIN_LIMIT);
        prompt_payload["frameworkContext"] = framework_context(
            engine,
            project_state,
            false,
            &candidate_ids,
            CANDIDATE_NODE_MIN_LIMIT,
        );
        prompt_payload["routerDecision"] =
            router_decision_payload(engine, &candidate_ids, &degradations);
        degradations.push("candidateNodes:top5_to_top3".to_string());
    }
    prompt_text = build_prompt_text(&prompt_snapshot, &prompt_payload);
    if !include_full && prompt_text.chars().count() > PROMPT_CHAR_BUDGET_TURN {
        prompt_payload["projectDigest"] = project_digest(engine, project_state, true);
        degradations.push("projectDigest:minimal".to_string());
    }
    prompt_text = build_prompt_text(&prompt_snapshot, &prompt_payload);
    if !include_full && prompt_text.chars().count() > PROMPT_CHAR_BUDGET_TURN {
        degradations.push("budget_warning:over_limit".to_string());
    }
    prompt_payload["routerDecision"] =
        router_decision_payload(engine, &candidate_ids, &degradations);
    prompt_text = build_prompt_text(&prompt_snapshot, &prompt_payload);
    let meter = prompt_meter_entry(
        &turn_id,
        &schema_mode,
        &output_mode,
        &prompt_payload,
        &prompt_text,
        &degradations,
        &project_state.ai_interview,
    );
    let mut replay = prompt_replay_fields(&prompt_text, 2000, options.store_full_prompt);
    merge_replay(
        &mut replay,
        json!({
            "turnId": turn_id,
            "schemaMode": schema_mode.as_str(),
            "outputMode": output_mode.as_str(),
            "forceOutput": include_full,
            "forceReadinessCheck": options.force_readiness_check,
            "routerDecision": prompt_payload.get("routerDecision").cloned().unwrap_or_else(|| json!({})),
            "projectStateHash": stable_hash(&project_digest(engine, project_state, true)),
            "promptMeter": serde_json::to_value(&meter).unwrap_or_else(|_| json!({})),
        }),
    );
    PromptBuildResult {
        turn_id,
        schema_mode,
        output_mode,
        prompt_text,
        prompt_payload,
        meter,
        replay,
        degradations,
    }
}

pub fn build_output_partition_prompt(
    engine: &DesignEngineService,
    project_state: &mut ProjectState,
    user_text: &str,
    domain_ids: &[String],
    partition_index: usize,
    partition_count: usize,
    options: PromptBuildOptions,
) -> PromptBuildResult {
    normalize_interview_state(&mut project_state.ai_interview);
    update_route_overview(engine, project_state);
    let domain_ids = domain_ids
        .iter()
        .filter(|domain_id| {
            engine
                .specs
                .iter()
                .any(|spec| &spec.domain_id == *domain_id)
        })
        .cloned()
        .collect::<Vec<_>>();
    let turn_id = if options.turn_id.trim().is_empty() {
        "turn_part".to_string()
    } else {
        options.turn_id.clone()
    };
    let prompt_snapshot = prompt_snapshot(&options);
    let prompt_payload = json!({
        "turnId": turn_id,
        "task": "commercial_game_design_ai_interview_output_partition",
        "promptFramework": {
            "snapshot": prompt_snapshot,
            "rules": prompt_rules(),
            "visibility": "hidden_to_user",
            "designOptionFrameworkMutation": "forbidden"
        },
        "schemaMode": "partial_output",
        "outputMode": "partial_project_output",
        "partition": {
            "index": partition_index,
            "count": partition_count,
            "domainIds": domain_ids,
            "policy": "只输出这些 domainIds 下节点的 projectState patch，不要补全其他领域。"
        },
        "projectSummary": compact_project_summary(engine, project_state),
        "projectDigest": project_digest(engine, project_state, false),
        "conversationSummary": serde_json::to_value(&project_state.ai_interview.summary.v1).unwrap_or_else(|_| json!({})),
        "routeOverview": serde_json::to_value(&project_state.ai_interview.route_overview).unwrap_or_else(|_| json!({})),
        "questionGroupCount": project_state.ai_interview.question_group_count,
        "evaluationBatchId": project_state.ai_interview.framework_memory.evaluation_batch_id,
        "projectMemoryId": project_state.ai_interview.framework_memory.project_memory_id,
        "recentMessages": recent_messages(&project_state.ai_interview, RECENT_MESSAGE_LIMIT_FULL),
        "domainProjectState": project_state_for_domains(project_state, engine, &domain_ids),
        "frameworkContext": packed_framework_context_for_domains(engine, &domain_ids),
        "userMessage": user_text,
        "partialOutputRequirements": [
            "必须返回 mode=partial_project_output 和 partialProjectOutput。",
            "partialProjectOutput.domainIds 必须等于本分片 domainIds。",
            "partialProjectOutput.projectStatePatchJson 必须是 JSON 字符串，解析后结构至少包含 nodes 对象。",
            "nodes 只能包含本分片 domainIds 下的节点；不要包含其他领域节点。",
            "confidenceMapJson 必须是 JSON 字符串，解析后至少包含 groups 或 nodes 置信度。",
            "低置信内容留在 inferences 中，不要提高置信度。",
            "For concrete nodes in this partition, include node.designEntities only when the entity matches one of the packed entitySchemas and confidenceMap.nodes[nodeId] >= 0.75."
        ]
    });
    let prompt_text = build_prompt_text(&prompt_snapshot, &prompt_payload);
    let schema_mode = InterviewSchemaMode::PartialOutput;
    let output_mode = InterviewOutputMode::PartialProjectOutput;
    let meter = prompt_meter_entry(
        &turn_id,
        &schema_mode,
        &output_mode,
        &prompt_payload,
        &prompt_text,
        &[],
        &project_state.ai_interview,
    );
    let mut replay = prompt_replay_fields(&prompt_text, 2000, options.store_full_prompt);
    merge_replay(
        &mut replay,
        json!({
            "turnId": turn_id,
            "schemaMode": "partial_output",
            "outputMode": "partial_project_output",
            "partition": prompt_payload.get("partition").cloned().unwrap_or_else(|| json!({})),
            "projectStateHash": stable_hash(&project_state_for_domains(project_state, engine, &domain_ids)),
            "promptMeter": serde_json::to_value(&meter).unwrap_or_else(|_| json!({})),
        }),
    );
    PromptBuildResult {
        turn_id,
        schema_mode,
        output_mode,
        prompt_text,
        prompt_payload,
        meter,
        replay,
        degradations: Vec::new(),
    }
}

pub fn choose_output_domain_partitions(
    engine: &DesignEngineService,
    project_state: &mut ProjectState,
    user_text: &str,
    options: PromptBuildOptions,
    budget: Option<usize>,
    candidate_counts: Option<&[usize]>,
) -> OutputPartitionPlan {
    let budget = budget.unwrap_or(OUTPUT_PARTITION_PROMPT_BUDGET);
    let counts = candidate_counts.unwrap_or(OUTPUT_PARTITION_CANDIDATE_COUNTS);
    let mut best = None;
    for count in counts {
        let partitions = output_domain_partitions(engine, *count);
        if partitions.is_empty() {
            continue;
        }
        let mut sizes = Vec::new();
        for (index, domain_ids) in partitions.iter().enumerate() {
            let prompt = build_output_partition_prompt(
                engine,
                project_state,
                user_text,
                domain_ids,
                index + 1,
                partitions.len(),
                options.clone(),
            );
            sizes.push(prompt.prompt_text.chars().count());
        }
        let max_prompt_chars = sizes.iter().copied().max().unwrap_or_default();
        let avg_prompt_chars = if sizes.is_empty() {
            0
        } else {
            sizes.iter().sum::<usize>() / sizes.len()
        };
        let plan = OutputPartitionPlan {
            partitions,
            prompt_budget: budget,
            prompt_sizes: sizes,
            max_prompt_chars,
            avg_prompt_chars,
        };
        if plan.max_prompt_chars <= budget {
            return plan;
        }
        best = Some(plan);
    }
    best.unwrap_or(OutputPartitionPlan {
        partitions: Vec::new(),
        prompt_budget: budget,
        prompt_sizes: Vec::new(),
        max_prompt_chars: 0,
        avg_prompt_chars: 0,
    })
}

pub fn output_domain_partitions(
    engine: &DesignEngineService,
    part_count: usize,
) -> Vec<Vec<String>> {
    let domain_ids = first_domain_ids(engine, usize::MAX);
    if domain_ids.is_empty() {
        return Vec::new();
    }
    let part_count = part_count.max(1).min(domain_ids.len());
    let mut partitions = vec![Vec::new(); part_count];
    for (index, domain_id) in domain_ids.into_iter().enumerate() {
        partitions[index % part_count].push(domain_id);
    }
    partitions
        .into_iter()
        .filter(|partition| !partition.is_empty())
        .collect()
}

pub fn prompt_meter_entry(
    turn_id: &str,
    schema_mode: &InterviewSchemaMode,
    output_mode: &InterviewOutputMode,
    prompt_payload: &Value,
    prompt_text: &str,
    degradations: &[String],
    ai_state: &AiInterviewState,
) -> PromptMeterEntry {
    let response_schema_empty_fields_removed = match schema_mode {
        InterviewSchemaMode::Turn => vec!["fullProjectOutput", "optionDifferences"],
        InterviewSchemaMode::Readiness => {
            vec!["questionGroup", "fullProjectOutput", "optionDifferences"]
        }
        _ => Vec::new(),
    }
    .into_iter()
    .map(str::to_string)
    .collect();
    PromptMeterEntry {
        turn_id: turn_id.to_string(),
        schema_mode: schema_mode.as_str().to_string(),
        output_mode: output_mode.as_str().to_string(),
        prompt_chars: prompt_text.chars().count(),
        prompt_estimated_tokens: prompt_text.chars().count() / 4,
        section_chars: prompt_section_sizes(prompt_payload),
        degradations: degradations.to_vec(),
        response_schema_empty_fields_removed,
        codex_session_id: ai_state.codex_session_id.clone(),
        session_accumulated_turns: ai_state.session_turn_count,
        question_group_count: ai_state.question_group_count,
    }
}

pub fn compact_project_summary(engine: &DesignEngineService, state: &ProjectState) -> Value {
    let view = engine.view_model(state);
    json!({
        "projectName": state.project_name,
        "profile": state.profile,
        "coverage": view.project_coverage,
        "l4Progress": view.project_l4_progress,
        "focusDomainIds": focus_domain_ids(engine, state),
    })
}

pub fn project_digest(engine: &DesignEngineService, state: &ProjectState, minimal: bool) -> Value {
    let view = engine.view_model(state);
    let mut non_default_nodes = Vec::new();
    for spec in &engine.specs {
        if let Some(node_state) = state.nodes.get(&spec.node_id) {
            if let Some(compact) = compact_node_state(engine, spec, node_state, minimal) {
                non_default_nodes.push(compact);
            }
        }
    }
    let mut applicability = Vec::new();
    for (node_id, entry) in &state.ai_interview.applicability_scores {
        let evidence_count = entry
            .get("evidenceCount")
            .and_then(Value::as_u64)
            .unwrap_or_default();
        if evidence_count == 0 {
            continue;
        }
        applicability.push(json!({
            "nodeId": node_id,
            "score": entry.get("score").cloned().unwrap_or_else(|| json!(0.5)),
            "evidenceCount": evidence_count,
            "reason": short_text(entry.get("reason").and_then(Value::as_str).unwrap_or_default(), 120),
        }));
    }
    json!({
        "projectName": state.project_name,
        "profile": state.profile,
        "coverage": view.project_coverage,
        "l4Progress": view.project_l4_progress,
        "focusDomainIds": focus_domain_ids(engine, state),
        "nonDefaultNodes": non_default_nodes.into_iter().take(if minimal { 8 } else { 16 }).collect::<Vec<_>>(),
        "clarificationTargets": state.ai_interview.route_overview.clarification_targets.iter().take(if minimal { 6 } else { 12 }).cloned().collect::<Vec<_>>(),
        "recentInferences": state.ai_interview.inferences.iter().rev().take(if minimal { 4 } else { 8 }).cloned().collect::<Vec<_>>().into_iter().rev().collect::<Vec<_>>(),
        "applicabilityHighlights": applicability.into_iter().take(if minimal { 8 } else { 16 }).collect::<Vec<_>>(),
    })
}

pub fn framework_context(
    engine: &DesignEngineService,
    state: &ProjectState,
    include_full: bool,
    candidate_ids: &[String],
    node_limit: usize,
) -> Value {
    let domain_ids = if include_full {
        first_domain_ids(engine, usize::MAX)
    } else if !candidate_ids.is_empty() {
        candidate_ids
            .iter()
            .filter_map(|node_id| {
                engine
                    .specs
                    .iter()
                    .find(|spec| &spec.node_id == node_id)
                    .map(|spec| spec.domain_id.clone())
            })
            .fold(Vec::new(), |mut domains, domain| {
                if !domains.iter().any(|item| item == &domain) {
                    domains.push(domain);
                }
                domains
            })
    } else {
        let focus = focus_domain_ids(engine, state);
        if focus.is_empty() {
            first_domain_ids(engine, 4)
        } else {
            focus
        }
    };
    let candidate_set = candidate_ids
        .iter()
        .cloned()
        .collect::<std::collections::BTreeSet<_>>();
    let stage_index = state.ai_interview.question_group_count as usize % MDA_STAGES.len();
    let mda_stage_id = MDA_STAGES[stage_index].0;
    let domains = domain_ids
        .into_iter()
        .map(|domain_id| {
            let mut nodes = engine
                .specs
                .iter()
                .filter(|spec| spec.domain_id == domain_id)
                .filter(|spec| candidate_set.is_empty() || candidate_set.contains(&spec.node_id))
                .collect::<Vec<_>>();
            if !include_full {
                nodes.truncate(node_limit);
            }
            json!({
                "id": domain_id,
                "name": humanize_id(&domain_id),
                "description": if include_full { format!("{} domain", domain_id) } else { short_text(&format!("{} domain", domain_id), 180) },
                "nodes": nodes.into_iter().map(|spec| node_context(spec, include_full, !include_full, mda_stage_id)).collect::<Vec<_>>(),
            })
        })
        .collect::<Vec<_>>();
    Value::Array(domains)
}

pub fn packed_framework_context_for_domains(
    engine: &DesignEngineService,
    domain_ids: &[String],
) -> Value {
    let domains = domain_ids
        .iter()
        .filter_map(|domain_id| {
            let nodes = engine
                .specs
                .iter()
                .filter(|spec| &spec.domain_id == domain_id)
                .map(|spec| {
                    let items = spec
                        .checklist
                        .iter()
                        .map(|item| {
                            let groups = item
                                .option_groups
                                .iter()
                                .map(|group| {
                                    json!([
                                        group.group_id,
                                        humanize_id(&group.group_id),
                                        group.selection_mode,
                                        "",
                                        "",
                                        group
                                            .options
                                            .iter()
                                            .map(|option| json!([option, humanize_id(option)]))
                                            .collect::<Vec<_>>()
                                    ])
                                })
                                .collect::<Vec<_>>();
                            json!([item.item_id, item.label, groups])
                        })
                        .collect::<Vec<_>>();
                    json!([
                        spec.node_id,
                        spec.name,
                        spec.domain_id,
                        spec.role_class,
                        short_text(&spec.description, 140),
                        if super::CONCRETE_ROLE_CLASSES.contains(&spec.role_class.as_str()) {
                            concrete_entity_summaries()
                        } else {
                            json!([])
                        },
                        items
                    ])
                })
                .collect::<Vec<_>>();
            Some(json!([domain_id, humanize_id(domain_id), nodes]))
        })
        .collect::<Vec<_>>();
    json!({
        "format": {
            "domain": ["id", "name", "nodes"],
            "node": ["id", "name", "domainId", "roleClass", "description", "entitySchemas", "items"],
            "item": ["id", "label", "groups"],
            "group": ["id", "label", "selectionMode", "mdaLayer", "designQuestion", "options"],
            "option": ["id", "label"]
        },
        "domains": domains
    })
}

pub fn project_state_for_domains(
    state: &ProjectState,
    engine: &DesignEngineService,
    domain_ids: &[String],
) -> Value {
    let domain_set = domain_ids.iter().collect::<std::collections::BTreeSet<_>>();
    let nodes = engine
        .specs
        .iter()
        .filter(|spec| domain_set.contains(&spec.domain_id))
        .filter_map(|spec| {
            state.nodes.get(&spec.node_id).map(|node| {
                (
                    spec.node_id.clone(),
                    serde_json::to_value(node).unwrap_or_else(|_| json!({})),
                )
            })
        })
        .collect::<Map<String, Value>>();
    json!({
        "projectName": state.project_name,
        "profile": state.profile,
        "nodes": nodes,
    })
}

fn compact_node_state(
    engine: &DesignEngineService,
    spec: &DesignNodeSpec,
    node_state: &NodeState,
    minimal: bool,
) -> Option<Value> {
    let effective = engine.effective_node_state(node_state);
    let selected_items = selected_groups_for_node(spec, node_state);
    let has_note = !node_state.design_note.trim().is_empty();
    let has_risk = !node_state.risk_note.trim().is_empty();
    let has_not_applicable = node_state.decision_state == DecisionState::NotApplicable;
    let has_entities = !node_state.design_entities.is_empty();
    if effective == DecisionState::NotStarted
        && selected_items.is_empty()
        && !has_note
        && !has_risk
        && !has_not_applicable
        && !has_entities
    {
        return None;
    }
    let mut payload = json!({
        "nodeId": spec.node_id,
        "name": spec.name,
        "domain": spec.domain_id,
        "roleClass": spec.role_class,
        "decisionState": node_state.decision_state.as_str(),
        "effectiveState": effective.as_str(),
    });
    if !selected_items.is_empty() {
        payload["selectedItems"] = json!(
            selected_items
                .into_iter()
                .take(if minimal { 4 } else { 8 })
                .collect::<Vec<_>>()
        );
    }
    if has_risk {
        payload["riskNote"] = json!(short_text(
            &node_state.risk_note,
            if minimal { 120 } else { 240 }
        ));
    }
    if has_not_applicable {
        payload["notApplicableReason"] = json!(short_text(
            &node_state.not_applicable_reason,
            if minimal { 120 } else { 240 }
        ));
    }
    if has_note && !minimal {
        payload["designNote"] = json!(short_text(&node_state.design_note, 240));
    }
    if has_entities {
        payload["designEntities"] = json!(
            node_state
                .design_entities
                .iter()
                .take(if minimal { 2 } else { 4 })
                .map(|entity| {
                    json!({
                        "kind": entity.get("kind").and_then(Value::as_str).unwrap_or_default(),
                        "schema": entity.get("schema").and_then(Value::as_str).unwrap_or_default(),
                        "id": entity.get("id").and_then(Value::as_str).unwrap_or_default(),
                        "label": entity.get("label").and_then(Value::as_str).unwrap_or_default(),
                    })
                })
                .collect::<Vec<_>>()
        );
    }
    Some(payload)
}

fn selected_groups_for_node(spec: &DesignNodeSpec, node_state: &NodeState) -> Vec<Value> {
    let mut selected_items = Vec::new();
    for item in &spec.checklist {
        let mut groups = Vec::new();
        if let Some(item_options) = node_state.checklist_options.get(&item.item_id) {
            for group in &item.option_groups {
                if let Some(group_state) = item_options.get(&group.group_id) {
                    if !group_state.selected.is_empty() {
                        groups.push(json!({
                            "groupId": group.group_id,
                            "selected": group_state.selected,
                            "primary": group_state.primary,
                        }));
                    }
                }
            }
        }
        if *node_state.checklist.get(&item.item_id).unwrap_or(&false) || !groups.is_empty() {
            selected_items.push(json!({
                "itemId": item.item_id,
                "label": item.label,
                "groups": groups,
            }));
        }
    }
    selected_items
}

fn node_context(
    spec: &DesignNodeSpec,
    include_options: bool,
    compact: bool,
    _mda_stage_id: &str,
) -> Value {
    let items = spec
        .checklist
        .iter()
        .take(if compact { 4 } else { usize::MAX })
        .map(|item| {
            let groups = item
                .option_groups
                .iter()
                .take(if compact { 1 } else { usize::MAX })
                .map(|group| {
                    let mut payload = json!({
                        "id": group.group_id,
                        "label": humanize_id(&group.group_id),
                        "required": false,
                        "selectionMode": group.selection_mode,
                        "mdaLayer": "",
                        "mdaLayerLabel": "",
                        "designQuestion": "",
                    });
                    if include_options {
                        payload["options"] = json!(
                            group
                                .options
                                .iter()
                                .map(|option| {
                                    json!({
                                        "id": option,
                                        "label": humanize_id(option),
                                        "description": "",
                                    })
                                })
                                .collect::<Vec<_>>()
                        );
                    }
                    payload
                })
                .collect::<Vec<_>>();
            json!({
                "id": item.item_id,
                "label": item.label,
                "description": if compact { Value::Null } else { Value::String(String::new()) },
                "optionGroups": groups,
            })
        })
        .collect::<Vec<_>>();
    let mut payload = json!({
        "id": spec.node_id,
        "name": spec.name,
        "domain": spec.domain_id,
        "roleClass": spec.role_class,
        "description": if compact { short_text(&spec.description, 180) } else { spec.description.clone() },
        "checklist": items,
    });
    if super::CONCRETE_ROLE_CLASSES.contains(&spec.role_class.as_str()) {
        payload["designEntityPrompt"] = json!({
            "writeField": "designEntities",
            "validation": "schema_required_fields_must_pass_before_write",
            "allowedSchemas": concrete_entity_summaries(),
        });
    }
    payload
}

fn concrete_entity_summaries() -> Value {
    json!([{
        "id": "generic_design_entity",
        "kind": "design_entity",
        "schemaVersion": "1.0",
        "required": ["id", "label", "kind"]
    }])
}

fn prompt_snapshot(options: &PromptBuildOptions) -> Value {
    json!({
        "frameworkVersion": options.framework_version,
        "manifestHash": options.manifest_hash,
    })
}

fn prompt_rules() -> Value {
    json!([
        "保持商业游戏设计访谈口吻。",
        "不得修改设计选项框架，只能解释、提问、映射和判断置信度。",
        "证据不足时降低置信度并追问。",
    ])
}

fn memory_influence(options: &PromptBuildOptions) -> Value {
    json!({
        "policy": if options.memory_signals.is_empty() { "none" } else { "top_3" },
        "signals": options.memory_signals.iter().take(3).cloned().collect::<Vec<_>>(),
    })
}

fn merge_replay(target: &mut Value, update: Value) {
    if let (Some(target), Some(update)) = (target.as_object_mut(), update.as_object()) {
        for (key, value) in update {
            target.insert(key.clone(), value.clone());
        }
    }
}
