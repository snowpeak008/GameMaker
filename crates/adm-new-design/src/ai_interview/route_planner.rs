use std::collections::BTreeSet;

use adm_new_contracts::ai::{AiInterviewState, AiRouteOverview, MDA_STAGES};
use adm_new_contracts::project::{DecisionState, NodeState, ProjectState};
use serde_json::{Value, json};

use crate::{DesignEngineService, DesignNodeSpec};

use super::{
    CANDIDATE_NODE_MIN_LIMIT, CLARIFICATION_CONFIDENCE_THRESHOLD, QUESTION_GROUP_CHECK_INTERVAL,
    state::now_iso,
};

pub fn text_tokens(value: &str) -> BTreeSet<String> {
    let text = value.to_lowercase();
    let mut tokens = BTreeSet::new();
    let mut ascii = String::new();
    for ch in text.chars() {
        if ch.is_ascii_alphanumeric() || ch == '_' {
            ascii.push(ch);
        } else {
            if ascii.chars().count() >= 2 {
                tokens.insert(ascii.clone());
            }
            ascii.clear();
        }
    }
    if ascii.chars().count() >= 2 {
        tokens.insert(ascii);
    }
    let cjk = text
        .chars()
        .filter(|ch| ('\u{4e00}'..='\u{9fff}').contains(ch))
        .collect::<Vec<_>>();
    if cjk.len() >= 2 {
        for window in cjk.windows(2) {
            tokens.insert(window.iter().collect());
        }
    }
    tokens
}

pub fn recent_question_target_ids(ai_state: &AiInterviewState, limit: usize) -> BTreeSet<String> {
    let mut target_ids = BTreeSet::new();
    for entry in ai_state.recent_question_targets.iter().rev().take(limit) {
        if let Some(text) = entry.as_str() {
            target_ids.insert(text.to_string());
        }
        if let Some(ids) = entry.get("nodeIds").and_then(Value::as_array) {
            for id in ids {
                if let Some(id) = id.as_str() {
                    target_ids.insert(id.to_string());
                }
            }
        }
    }
    target_ids
}

pub fn applicability_entry(ai_state: &AiInterviewState, node_id: &str) -> (f64, u64) {
    let entry = ai_state
        .applicability_scores
        .get(node_id)
        .cloned()
        .unwrap_or_else(|| json!({}));
    let score = entry.get("score").and_then(Value::as_f64).unwrap_or(0.5);
    let evidence_count = entry
        .get("evidenceCount")
        .and_then(Value::as_u64)
        .unwrap_or_default();
    (score, evidence_count)
}

pub fn candidate_node_ids(
    engine: &DesignEngineService,
    project_state: &ProjectState,
    user_text: &str,
    limit: usize,
) -> Vec<String> {
    let mut focus_domains = focus_domain_ids(engine, project_state);
    if focus_domains.is_empty() {
        focus_domains = first_domain_ids(engine, 4);
    }
    let user_tokens = text_tokens(user_text);
    let recent_targets = recent_question_target_ids(&project_state.ai_interview, 3);
    let mut scored = Vec::new();
    for spec in &engine.specs {
        let node_state = project_state
            .nodes
            .get(&spec.node_id)
            .cloned()
            .unwrap_or_default();
        let effective = engine.effective_node_state(&node_state);
        let mut score = 0.0;
        if focus_domains.iter().any(|domain| domain == &spec.domain_id) {
            score += 3.0;
        }
        let (applicability, evidence_count) =
            applicability_entry(&project_state.ai_interview, &spec.node_id);
        score += applicability * 2.0;
        if evidence_count > 0 && (0.35..0.75).contains(&applicability) {
            score += 1.5;
        }
        if !node_state.risk_note.trim().is_empty() {
            score += 2.0;
        }
        if node_has_l4_gap(spec, &node_state) {
            score += 1.25;
        }
        if !user_tokens.is_empty() {
            let node_tokens = text_tokens(&node_search_index(spec));
            score += ((user_tokens.intersection(&node_tokens).count() as f64) * 0.35).min(3.0);
        }
        match effective {
            DecisionState::NotStarted => score += 0.5,
            DecisionState::Completed => score -= 1.0,
            DecisionState::NotApplicable => score -= 3.0,
            DecisionState::Selected | DecisionState::Risk => {}
        }
        if recent_targets.contains(&spec.node_id) {
            score -= 2.5;
        }
        scored.push((score, spec.node_id.clone()));
    }
    scored.sort_by(|left, right| {
        right
            .0
            .total_cmp(&left.0)
            .then_with(|| right.1.cmp(&left.1))
    });
    let mut chosen = scored
        .into_iter()
        .filter(|(score, _)| *score > -2.0)
        .take(limit)
        .map(|(_, node_id)| node_id)
        .collect::<Vec<_>>();
    if chosen.len() < limit {
        for spec in &engine.specs {
            if focus_domains.iter().any(|domain| domain == &spec.domain_id)
                && !chosen.iter().any(|node_id| node_id == &spec.node_id)
            {
                chosen.push(spec.node_id.clone());
            }
            if chosen.len() >= limit {
                break;
            }
        }
    }
    chosen.truncate(limit.max(CANDIDATE_NODE_MIN_LIMIT).min(limit));
    chosen
}

pub fn update_route_overview(
    engine: &DesignEngineService,
    project_state: &mut ProjectState,
) -> AiRouteOverview {
    let stage_index = project_state.ai_interview.question_group_count as usize % MDA_STAGES.len();
    let current_stage = MDA_STAGES[stage_index].1.to_string();
    let mut focus_domains = focus_domain_ids(engine, project_state);
    if focus_domains.is_empty() {
        focus_domains = first_domain_ids(engine, 4);
    }
    let expected_domains = focus_domains
        .iter()
        .map(|domain| humanize_id(domain))
        .collect::<Vec<_>>();
    let mut completed = Vec::new();
    let mut clarification = Vec::new();
    let mut low_applicability = Vec::new();
    for spec in &engine.specs {
        let node_state = project_state
            .nodes
            .get(&spec.node_id)
            .cloned()
            .unwrap_or_default();
        let effective = engine.effective_node_state(&node_state);
        let (score, evidence_count) =
            applicability_entry(&project_state.ai_interview, &spec.node_id);
        if matches!(
            effective,
            DecisionState::Completed | DecisionState::NotApplicable
        ) {
            completed.push(spec.name.clone());
        } else if !node_state.risk_note.trim().is_empty()
            || (evidence_count > 0 && score >= 0.35 && score < super::HIGH_CONFIDENCE_THRESHOLD)
        {
            clarification.push(spec.name.clone());
        }
        if node_state.decision_state == DecisionState::NotApplicable
            || (evidence_count > 0 && score < CLARIFICATION_CONFIDENCE_THRESHOLD.min(0.35))
        {
            low_applicability.push(format!("{}（{score:.2}）", spec.name));
        }
    }
    completed.truncate(12);
    clarification.truncate(12);
    low_applicability.truncate(12);
    let overview = AiRouteOverview {
        current_mda_stage: current_stage,
        expected_domains,
        completed_nodes: completed,
        clarification_targets: clarification.into_iter().map(Value::String).collect(),
        low_applicability_candidates: low_applicability.into_iter().map(Value::String).collect(),
        ..AiRouteOverview::default()
    };
    project_state.ai_interview.route_overview = overview.clone();
    project_state.ai_interview.updated_at = now_iso();
    overview
}

pub fn router_decision_payload(
    engine: &DesignEngineService,
    candidate_ids: &[String],
    degradations: &[String],
) -> Value {
    json!({
        "candidateNodeIds": candidate_ids,
        "candidateNodes": candidate_ids.iter().map(|node_id| {
            let spec = engine.specs.iter().find(|spec| &spec.node_id == node_id);
            json!({
                "id": node_id,
                "name": spec.map(|spec| spec.name.clone()).unwrap_or_else(|| node_id.clone()),
                "domain": spec.map(|spec| spec.domain_id.clone()).unwrap_or_default(),
                "roleClass": spec.map(|spec| spec.role_class.clone()).unwrap_or_default(),
                "designEntityTarget": spec.map(|spec| super::CONCRETE_ROLE_CLASSES.contains(&spec.role_class.as_str())).unwrap_or(false),
            })
        }).collect::<Vec<_>>(),
        "degradations": degradations,
    })
}

pub fn should_force_readiness_check(ai_state: &AiInterviewState) -> bool {
    let count = ai_state.question_group_count;
    count > 0
        && count % QUESTION_GROUP_CHECK_INTERVAL == 0
        && ai_state.last_readiness_check_group < count
}

pub fn detect_force_output(user_text: &str) -> bool {
    ["输出", "生成完整", "生成方案", "完整方案", "全项目输出"]
        .iter()
        .any(|keyword| user_text.contains(keyword))
}

pub(crate) fn focus_domain_ids(
    engine: &DesignEngineService,
    project_state: &ProjectState,
) -> Vec<String> {
    let mut domains = Vec::new();
    for key in [
        "focusDomainIds",
        "focus_domain_ids",
        "domainIds",
        "domain_ids",
        "domains",
    ] {
        if let Some(value) = project_state.profile.get(key) {
            collect_domains(value, &mut domains);
        }
    }
    domains.retain(|domain| engine.specs.iter().any(|spec| spec.domain_id == *domain));
    domains.sort();
    domains.dedup();
    domains
}

pub(crate) fn first_domain_ids(engine: &DesignEngineService, limit: usize) -> Vec<String> {
    let mut domains = Vec::new();
    for spec in &engine.specs {
        if !domains.iter().any(|domain| domain == &spec.domain_id) {
            domains.push(spec.domain_id.clone());
        }
        if domains.len() >= limit {
            break;
        }
    }
    domains
}

fn collect_domains(value: &Value, output: &mut Vec<String>) {
    match value {
        Value::Array(items) => {
            for item in items {
                collect_domains(item, output);
            }
        }
        Value::String(text) => {
            for part in text.split([',', ';', '，', '；']) {
                let part = part.trim();
                if !part.is_empty() {
                    output.push(part.to_string());
                }
            }
        }
        Value::Object(object) => {
            for key in ["id", "domainId", "domain_id"] {
                if let Some(text) = object.get(key).and_then(Value::as_str) {
                    output.push(text.to_string());
                }
            }
        }
        _ => {}
    }
}

pub(crate) fn node_has_l4_gap(spec: &DesignNodeSpec, node_state: &NodeState) -> bool {
    spec.checklist.iter().any(|item| {
        item.option_groups.iter().any(|group| {
            node_state
                .checklist_options
                .get(&item.item_id)
                .and_then(|groups| groups.get(&group.group_id))
                .map(|state| state.selected.is_empty())
                .unwrap_or(true)
        })
    })
}

pub(crate) fn node_search_index(spec: &DesignNodeSpec) -> String {
    let mut parts = vec![
        spec.node_id.clone(),
        spec.domain_id.clone(),
        spec.name.clone(),
        spec.description.clone(),
        spec.role_class.clone(),
    ];
    for item in &spec.checklist {
        parts.push(item.item_id.clone());
        parts.push(item.label.clone());
        for group in &item.option_groups {
            parts.push(group.group_id.clone());
            parts.extend(group.options.clone());
        }
    }
    parts.join(" ")
}

pub(crate) fn humanize_id(value: &str) -> String {
    value
        .split(['_', '-'])
        .filter(|part| !part.is_empty())
        .map(|part| {
            let mut chars = part.chars();
            match chars.next() {
                Some(first) => format!("{}{}", first.to_uppercase(), chars.as_str()),
                None => String::new(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}
