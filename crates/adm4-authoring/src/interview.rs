use crate::custom::CustomMechanicDraft;
use crate::engine::AuthoringEngine;
use crate::state::InterviewEntry;
use adm4_ai::{AiProvider, AiRequest};
use adm4_contracts::{MatrixCell, TypedValue};
use adm4_decision::system_module::PromptEntry;
use adm4_decision::{
    DecisionId, DecisionOption, DecisionPoint, DesignLevel, ParameterSchema, ParameterValues,
    Provenance, counts_toward_completeness,
};
use adm4_foundation::{Adm4Error, Adm4Result, UtcTimestamp};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

/// 机制访谈（实例内逐点提案）AI 调用的 purpose 键（T-W7-3d ③）。
pub const PURPOSE_MECHANISM: &str = "interview_mechanism";
/// 机制访谈 custom 草案起草 AI 调用的 purpose 键。
pub const PURPOSE_MECHANISM_CUSTOM: &str = "interview_mechanism_custom";

/// AI 访谈提案（结构层逐条 / L5-L6 整表）。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct InterviewProposal {
    pub decision_id: DecisionId,
    pub option_id: String,
    pub rationale: String,
    #[serde(default)]
    pub parameters: ParameterValues,
}

#[derive(Debug, Clone, PartialEq)]
pub enum InterviewTurn {
    /// 结构层（L0-L4）：单点提案。
    StructuralPoint(InterviewProposal),
    /// 参数表层（L5-L6）：整表提案（确认整表为一个决策，可例外下钻）。
    TableProposal(InterviewProposal),
    /// 全部激活点已确认。
    Complete,
}

/// 某一 L 层的访谈进度：已确认 / 适用（激活）计数。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct LevelProgress {
    pub level: DesignLevel,
    pub confirmed: usize,
    pub applicable: usize,
}

/// 分层访谈进度（D9 配套查询）：当前层与各层计数，供 UI/CLI 展示推进位置。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InterviewProgress {
    /// 访谈当前所在层：最低的仍有未确认适用点的层；None = 全部适用点已确认。
    pub current_level: Option<DesignLevel>,
    /// 各层计数（只列出存在适用点的层，按 L 层升序）。
    pub levels: Vec<LevelProgress>,
}

impl InterviewProgress {
    pub fn is_complete(&self) -> bool {
        self.current_level.is_none()
    }
}

/// AI 访谈服务：分层逐条确认（D9-D12）。
/// 按 L 层升序推进（低层未全确认不进高层）、同层按 DAG 拓扑序；
/// L5/L6 整表为一个确认单元，支持例外下钻；AI 永不代提交；非法输出即停（R7）。
pub struct InterviewService;

impl InterviewService {
    /// 生成下一个提案（D9 分层推进）：
    /// - 只在「最低的仍有待确认适用点的 L 层」内出提案，低层未全确认不得对高层出提案；
    /// - 同层内按决策图拓扑序；
    /// - 本轮被拒绝的点排到同层最后，同层只剩被拒点时才允许重提；
    /// - AI 不可用或输出非法 → Err（模式 B 停止，不锁死手动模式）。
    pub fn propose_next(
        engine: &mut AuthoringEngine,
        provider: &dyn AiProvider,
    ) -> Adm4Result<InterviewTurn> {
        let pending = engine.pending_decisions()?;
        if pending.is_empty() {
            let interview = engine_interview_mut(engine);
            interview.cursor = None;
            interview.skipped_this_round.clear();
            return Ok(InterviewTurn::Complete);
        }
        let next_id = select_layered_next(engine, &pending)?;
        let point = engine.space().graph.require_point(&next_id)?.clone();
        let request = build_request(engine, &point)?;
        let response = provider.invoke(&request)?;
        let proposal = parse_proposal(&point, &response.text)?;

        let interview = engine_interview_mut(engine);
        interview.cursor = Some(next_id.clone());
        // 重提即清除本轮跳过标记；再次拒绝会重新登记。
        interview.skipped_this_round.remove(&next_id);
        interview.transcript.push(InterviewEntry {
            decision_id: next_id,
            role: "ai_proposal".into(),
            content: response.text,
            at: UtcTimestamp::now().to_iso8601(),
        });

        let is_table_layer = matches!(
            point
                .option(&proposal.option_id)
                .map(|option| &option.parameter_schema),
            Some(ParameterSchema::Table(_)) | Some(ParameterSchema::Matrix(_))
        );
        Ok(if is_table_layer {
            InterviewTurn::TableProposal(proposal)
        } else {
            InterviewTurn::StructuralPoint(proposal)
        })
    }

    /// 机制访谈（T-W7-3d ③）：范围限定到某系统实例命名空间下的激活点，逐点提案。
    ///
    /// 与 `propose_next` 同一选点纪律（层升序 + 拓扑序 + 被拒点排后），只是把
    /// 待办过滤到 `<instance_id>.` 前缀（tier 合成点与模块决策点都在该命名空间下）。
    /// `ammo` 为 PromptLibrary 按当前系统 domain 取出的追问弹药——注入 AI 上下文
    /// 作为**追问素材**（帮 AI 提出更尖锐的取舍建议），不是决策点、不进决策图。
    /// 实例下无待办 → Complete；AI 非法输出 → Err 即停（与主路径同防线）。
    pub fn propose_next_scoped(
        engine: &mut AuthoringEngine,
        provider: &dyn AiProvider,
        instance_id: &str,
        ammo: &[PromptEntry],
    ) -> Adm4Result<InterviewTurn> {
        if instance_id.trim().is_empty() {
            return Err(Adm4Error::invalid_input(
                "机制访谈需要系统实例 id（范围限定到该实例命名空间下的激活点）",
            ));
        }
        let prefix = format!("{instance_id}.");
        let pending: Vec<DecisionId> = engine
            .pending_decisions()?
            .into_iter()
            .filter(|id| id.starts_with(&prefix))
            .collect();
        if pending.is_empty() {
            return Ok(InterviewTurn::Complete);
        }
        let next_id = select_layered_next(engine, &pending)?;
        let point = engine.space().graph.require_point(&next_id)?.clone();
        let mut request = build_request(engine, &point)?;
        request.purpose = PURPOSE_MECHANISM.into();
        if !ammo.is_empty() {
            let ammo_text = ammo
                .iter()
                .map(|entry| {
                    let follow_ups = if entry.follow_ups.is_empty() {
                        String::new()
                    } else {
                        format!("（追问：{}）", entry.follow_ups.join("；"))
                    };
                    format!("- {}{follow_ups}", entry.question_zh)
                })
                .collect::<Vec<_>>()
                .join("\n");
            request.user_prompt.push_str(&format!(
                "\n\n追问弹药（该系统领域的真实取舍问句，供你在 rationale 里回应取舍、\
                 不是决策点）：\n{ammo_text}"
            ));
        }
        let response = provider.invoke(&request)?;
        let proposal = parse_proposal(&point, &response.text)?;

        let interview = engine_interview_mut(engine);
        interview.cursor = Some(next_id.clone());
        interview.skipped_this_round.remove(&next_id);
        interview.transcript.push(InterviewEntry {
            decision_id: next_id,
            role: "ai_proposal".into(),
            content: response.text,
            at: UtcTimestamp::now().to_iso8601(),
        });

        let is_table_layer = matches!(
            point
                .option(&proposal.option_id)
                .map(|option| &option.parameter_schema),
            Some(ParameterSchema::Table(_)) | Some(ParameterSchema::Matrix(_))
        );
        Ok(if is_table_layer {
            InterviewTurn::TableProposal(proposal)
        } else {
            InterviewTurn::StructuralPoint(proposal)
        })
    }

    /// custom 机制草案 AI 起草（T-W7-3d ③）：rule_text + effects + GWT 三段。
    ///
    /// 产出 `CustomMechanicDraft`——**不登记**：登记仍走 `custom_add`（用户确认手势），
    /// 本方法只是把「想要一个库里没有的机制」翻译成结构化草案。effects 全集校验
    /// 在登记时由 `EffectTemplateValidator` 做（不在此处复制第二份规则）；
    /// 这里只做形状与非空校验（slug/label/rule_text/rationale/effects 至少一条，
    /// custom 效果三段非空——起草空白等于没起草）。
    pub fn draft_custom_mechanic(
        engine: &AuthoringEngine,
        provider: &dyn AiProvider,
        host_system_id: &str,
        idea: &str,
        ammo: &[PromptEntry],
    ) -> Adm4Result<CustomMechanicDraft> {
        if idea.trim().is_empty() {
            return Err(Adm4Error::invalid_input(
                "custom 机制起草需要用户的机制想法（idea 不可为空）",
            ));
        }
        let host = engine.space().graph.require_point(host_system_id)?;
        let ammo_text = if ammo.is_empty() {
            String::new()
        } else {
            format!(
                "\n\n追问弹药（该系统领域的取舍问句，rationale 应回应其中的取舍）：\n{}",
                ammo.iter()
                    .map(|entry| format!("- {}", entry.question_zh))
                    .collect::<Vec<_>>()
                    .join("\n")
            )
        };
        let request = AiRequest {
            purpose: PURPOSE_MECHANISM_CUSTOM.into(),
            system_prompt: "你是游戏设计访谈助手，为用户起草一个自定义（custom）机制草案。\
                 输出 JSON（CustomMechanicDraft 形态）：{\"host_system_id\":给定值,\
                 \"slug\":小写下划线短名,\"label_zh\":中文机制名,\"rule_text\":规则文本,\
                 \"effects\":[{\"effect\":\"custom\",\"verb\":动词,\"given\":前置条件,\
                 \"when\":触发时机,\"then\":发生什么}],\"new_nouns\":[新名词，没有则空],\
                 \"rationale\":设计理由}。\
                 rule_text/GWT 三段（given/when/then）都必须非空——这是机制的验收语义。\
                 你起草的是草案，登记与确认由用户决定。"
                .into(),
            user_prompt: format!(
                "归属系统决策点：{}（{}）\n用户的机制想法：\n{}{ammo_text}",
                host.id,
                host.question,
                idea.trim()
            ),
            expect_json: true,
        };
        let response = provider.invoke(&request)?;
        let mut draft: CustomMechanicDraft =
            serde_json::from_str(response.text.trim()).map_err(|error| {
                Adm4Error::validation(format!(
                    "custom 草案不是合法 CustomMechanicDraft JSON：{error}；原文：{}",
                    response.text
                ))
            })?;
        // host 以调用方给定为准（AI 写错归属会让草案挂到别的系统下）。
        draft.host_system_id = host_system_id.to_string();
        if draft.rule_text.trim().is_empty() {
            return Err(Adm4Error::validation(
                "custom 草案的 rule_text 为空（规则文本是机制的本体，AI 起草不完整即拒）",
            ));
        }
        if draft.rationale.trim().is_empty() {
            return Err(Adm4Error::validation(
                "custom 草案的 rationale 为空（设计理由必填，AI 起草不完整即拒）",
            ));
        }
        if draft.effects.is_empty() {
            return Err(Adm4Error::validation(
                "custom 草案不含任何效果（无效果的机制不成立）",
            ));
        }
        for (index, effect) in draft.effects.iter().enumerate() {
            let is_custom = effect.get("effect").and_then(|tag| tag.as_str()) == Some("custom");
            if !is_custom {
                continue;
            }
            for key in ["given", "when", "then"] {
                let blank = effect
                    .get(key)
                    .and_then(|value| value.as_str())
                    .is_none_or(|text| text.trim().is_empty());
                if blank {
                    return Err(Adm4Error::validation(format!(
                        "custom 草案第 {} 条效果的 GWT 段 {key} 为空（三段验收语义缺一不可）",
                        index + 1
                    )));
                }
            }
        }
        Ok(draft)
    }

    /// 用户确认提案（唯一提交路径；AI 不可代调用——本方法仅由 UI/CLI 的用户动作触发）。
    /// `overrides` 为例外下钻：整表确认的同时替换若干行/格，
    /// 改动摘要（哪些行/格/参数从什么改成什么）记入 transcript（D10）。
    pub fn confirm_proposal(
        engine: &mut AuthoringEngine,
        proposal: &InterviewProposal,
        overrides: Option<ParameterValues>,
    ) -> Adm4Result<Vec<String>> {
        let confirm_note = match overrides.as_ref() {
            None => "确认".to_string(),
            Some(values) => {
                let row_key = table_row_key(engine, proposal)?;
                format!(
                    "确认（例外下钻：{}）",
                    describe_override_diff(row_key.as_deref(), &proposal.parameters, values)
                )
            }
        };
        engine.select_option(
            &proposal.decision_id,
            &proposal.option_id,
            Provenance::AiInterviewConfirmed,
        )?;
        let parameters = overrides.unwrap_or_else(|| proposal.parameters.clone());
        let problems = if matches!(parameters, ParameterValues::None) {
            Vec::new()
        } else {
            engine.set_parameters(&proposal.decision_id, parameters)?
        };
        engine.set_rationale(&proposal.decision_id, &proposal.rationale)?;
        engine.confirm_selection(&proposal.decision_id)?;
        let interview = engine_interview_mut(engine);
        interview.skipped_this_round.remove(&proposal.decision_id);
        interview.transcript.push(InterviewEntry {
            decision_id: proposal.decision_id.clone(),
            role: "user_confirm".into(),
            content: confirm_note,
            at: UtcTimestamp::now().to_iso8601(),
        });
        Ok(problems)
    }

    /// 用户拒绝提案：该点留在待办并标记「本轮跳过」——propose_next 不会立刻重提，
    /// 直到同层其余待办处理完只剩它（D11：AI 永不代提交，拒绝不产生任何选择）。
    pub fn reject_proposal(engine: &mut AuthoringEngine, decision_id: &str, note: &str) {
        let interview = engine_interview_mut(engine);
        interview.skipped_this_round.insert(decision_id.to_string());
        if interview.cursor.as_deref() == Some(decision_id) {
            interview.cursor = None;
        }
        interview.transcript.push(InterviewEntry {
            decision_id: decision_id.to_string(),
            role: "user_reject".into(),
            content: note.to_string(),
            at: UtcTimestamp::now().to_iso8601(),
        });
    }

    /// 查询分层访谈进度：当前层与各层「已确认/适用」计数（只读，不改状态）。
    pub fn progress(engine: &AuthoringEngine) -> InterviewProgress {
        let applicability = engine.applicability();
        let mut per_level: BTreeMap<DesignLevel, LevelProgress> = BTreeMap::new();
        for point in engine.space().graph.points() {
            // 与完成度分母同口径：未作答的非必做点不计入某层的 applicable，
            // 否则 `current_level` 会永远停在该层，而 `propose_next` 已返回 Complete。
            if !counts_toward_completeness(point, &applicability, &engine.state().selections) {
                continue;
            }
            let entry = per_level.entry(point.level).or_insert(LevelProgress {
                level: point.level,
                confirmed: 0,
                applicable: 0,
            });
            entry.applicable += 1;
            let confirmed = engine
                .state()
                .selections
                .get(&point.id)
                .is_some_and(|selection| selection.confirmed_by_user);
            if confirmed {
                entry.confirmed += 1;
            }
        }
        let levels: Vec<LevelProgress> = per_level.into_values().collect();
        let current_level = levels
            .iter()
            .find(|progress| progress.confirmed < progress.applicable)
            .map(|progress| progress.level);
        InterviewProgress {
            current_level,
            levels,
        }
    }
}

fn engine_interview_mut(engine: &mut AuthoringEngine) -> &mut crate::state::InterviewState {
    engine.interview_mut()
}

/// D9 选点：取待办中最低的 L 层，同层按拓扑序（pending 已是拓扑序）；
/// 本轮被拒绝的点排到同层最后——若同层全部被拒，则重提其中第一个
/// （更高层被 D9 层门挡住，此时重提是唯一可推进路径）。
fn select_layered_next(engine: &AuthoringEngine, pending: &[DecisionId]) -> Adm4Result<DecisionId> {
    let graph = &engine.space().graph;
    let mut leveled: Vec<(&DecisionId, DesignLevel)> = Vec::with_capacity(pending.len());
    for id in pending {
        leveled.push((id, graph.require_point(id)?.level));
    }
    let Some(current_level) = leveled.iter().map(|(_, level)| *level).min() else {
        return Err(Adm4Error::internal("访谈选点：待办清单为空"));
    };
    let layer: Vec<&DecisionId> = leveled
        .iter()
        .filter(|(_, level)| *level == current_level)
        .map(|(id, _)| *id)
        .collect();
    let skipped = &engine.state().interview.skipped_this_round;
    let chosen = layer
        .iter()
        .copied()
        .find(|id| !skipped.contains(*id))
        .or_else(|| layer.first().copied())
        .ok_or_else(|| Adm4Error::internal("访谈选点：当前层没有候选决策点"))?;
    Ok(chosen.clone())
}

/// 若提案选项是 Table 结构，返回其行键（下钻摘要按行键定位改动行）。
fn table_row_key(
    engine: &AuthoringEngine,
    proposal: &InterviewProposal,
) -> Adm4Result<Option<String>> {
    let point = engine.space().graph.require_point(&proposal.decision_id)?;
    let option = point.option(&proposal.option_id).ok_or_else(|| {
        Adm4Error::not_found(format!(
            "决策点 {} 无选项 {}",
            proposal.decision_id, proposal.option_id
        ))
    })?;
    Ok(match &option.parameter_schema {
        ParameterSchema::Table(table) => Some(table.row_key.clone()),
        _ => None,
    })
}

/// 生成例外下钻的可读摘要：逐行/逐格/逐参数列出「从什么改成什么」（D10）。
fn describe_override_diff(
    row_key: Option<&str>,
    base: &ParameterValues,
    overridden: &ParameterValues,
) -> String {
    let changes = match (base, overridden) {
        (ParameterValues::Rows { rows: base_rows }, ParameterValues::Rows { rows: new_rows }) => {
            diff_rows(row_key, base_rows, new_rows)
        }
        (
            ParameterValues::Cells { cells: base_cells },
            ParameterValues::Cells { cells: new_cells },
        ) => diff_cells(base_cells, new_cells),
        (
            ParameterValues::Scalars {
                entries: base_entries,
            },
            ParameterValues::Scalars {
                entries: new_entries,
            },
        ) => diff_scalars(base_entries, new_entries),
        (base_other, new_other) => vec![format!(
            "参数整体替换：{} → {}",
            shape_label(base_other),
            shape_label(new_other)
        )],
    };
    if changes.is_empty() {
        "与提案一致，无实际改动".to_string()
    } else {
        changes.join("；")
    }
}

fn diff_rows(
    row_key: Option<&str>,
    base_rows: &[BTreeMap<String, TypedValue>],
    new_rows: &[BTreeMap<String, TypedValue>],
) -> Vec<String> {
    let key_of = |index: usize, row: &BTreeMap<String, TypedValue>| -> String {
        row_key
            .and_then(|key| row.get(key))
            .map(TypedValue::render)
            .filter(|value| !value.is_empty())
            .unwrap_or_else(|| format!("第{}行", index + 1))
    };
    let base_map: BTreeMap<String, &BTreeMap<String, TypedValue>> = base_rows
        .iter()
        .enumerate()
        .map(|(index, row)| (key_of(index, row), row))
        .collect();
    let new_map: BTreeMap<String, &BTreeMap<String, TypedValue>> = new_rows
        .iter()
        .enumerate()
        .map(|(index, row)| (key_of(index, row), row))
        .collect();
    let mut changes = Vec::new();
    for (key, new_row) in &new_map {
        let Some(base_row) = base_map.get(key) else {
            changes.push(format!("新增行 {key}"));
            continue;
        };
        let mut columns: BTreeSet<&String> = base_row.keys().collect();
        columns.extend(new_row.keys());
        for column in columns {
            match (base_row.get(column), new_row.get(column)) {
                (Some(old), Some(new)) if old != new => changes.push(format!(
                    "行 {key} 列 {column}：{} → {}",
                    old.render(),
                    new.render()
                )),
                (Some(old), None) => {
                    changes.push(format!("行 {key} 删除列 {column}（原 {}）", old.render()));
                }
                (None, Some(new)) => {
                    changes.push(format!("行 {key} 新增列 {column}={}", new.render()));
                }
                _ => {}
            }
        }
    }
    for key in base_map.keys() {
        if !new_map.contains_key(key) {
            changes.push(format!("删除行 {key}"));
        }
    }
    changes
}

fn diff_cells(base_cells: &[MatrixCell], new_cells: &[MatrixCell]) -> Vec<String> {
    let base_map: BTreeMap<(&str, &str), &TypedValue> = base_cells
        .iter()
        .map(|cell| ((cell.row.as_str(), cell.col.as_str()), &cell.value))
        .collect();
    let new_map: BTreeMap<(&str, &str), &TypedValue> = new_cells
        .iter()
        .map(|cell| ((cell.row.as_str(), cell.col.as_str()), &cell.value))
        .collect();
    let mut changes = Vec::new();
    for ((row, col), new_value) in &new_map {
        match base_map.get(&(row, col)) {
            None => changes.push(format!("新增格 [{row} × {col}]={}", new_value.render())),
            Some(old_value) if old_value != new_value => changes.push(format!(
                "格 [{row} × {col}]：{} → {}",
                old_value.render(),
                new_value.render()
            )),
            _ => {}
        }
    }
    for (row, col) in base_map.keys() {
        if !new_map.contains_key(&(row, col)) {
            changes.push(format!("删除格 [{row} × {col}]"));
        }
    }
    changes
}

fn diff_scalars(
    base_entries: &BTreeMap<String, TypedValue>,
    new_entries: &BTreeMap<String, TypedValue>,
) -> Vec<String> {
    let mut changes = Vec::new();
    for (key, new_value) in new_entries {
        match base_entries.get(key) {
            None => changes.push(format!("新增参数 {key}={}", new_value.render())),
            Some(old_value) if old_value != new_value => changes.push(format!(
                "参数 {key}：{} → {}",
                old_value.render(),
                new_value.render()
            )),
            _ => {}
        }
    }
    for key in base_entries.keys() {
        if !new_entries.contains_key(key) {
            changes.push(format!("删除参数 {key}"));
        }
    }
    changes
}

fn shape_label(values: &ParameterValues) -> &'static str {
    match values {
        ParameterValues::None => "无参数",
        ParameterValues::Scalars { .. } => "标量参数",
        ParameterValues::Rows { .. } => "表行数据",
        ParameterValues::Cells { .. } => "矩阵格数据",
    }
}

fn build_request(engine: &AuthoringEngine, point: &DecisionPoint) -> Adm4Result<AiRequest> {
    let options_text = point
        .options
        .iter()
        .map(|option| {
            format!(
                "- id={} label={} implications={}{}",
                option.id,
                option.label,
                option.implications.join("；"),
                schema_hint(&option.parameter_schema)
            )
        })
        .collect::<Vec<_>>()
        .join("\n");
    let context = engine
        .state()
        .selections
        .values()
        .filter(|selection| selection.confirmed_by_user)
        .map(|selection| format!("{} => {}", selection.decision_id, selection.option_id))
        .collect::<Vec<_>>()
        .join("\n");
    Ok(AiRequest {
        purpose: "interview_proposal".into(),
        system_prompt: "你是游戏设计访谈助手。只能从给出的选项中提案，不得发明选项或参数字段。\
                        输出 JSON：{\"option_id\":..., \"rationale\":..., \"parameters\":可选}。\
                        表结构选项必须给出整表 rows，矩阵选项必须给出整表 cells。\
                        你提出的是建议，最终决定权在用户。"
            .into(),
        user_prompt: format!(
            "项目已确认决策：\n{context}\n\n当前决策点 {}（{}）：{}\n候选选项：\n{options_text}",
            point.id,
            point.level.label(),
            point.question
        ),
        expect_json: true,
    })
}

/// 选项参数结构提示（进提示词，让 AI 知道整表提案需要什么形状）。
fn schema_hint(schema: &ParameterSchema) -> String {
    match schema {
        ParameterSchema::None => String::new(),
        ParameterSchema::Scalar { fields } => format!(
            " 参数字段：{}",
            fields
                .iter()
                .map(|field| field.key.clone())
                .collect::<Vec<_>>()
                .join("/")
        ),
        ParameterSchema::Table(table) => format!(
            " 整表提案（parameters.rows，行键 {}，列：{}）",
            table.row_key,
            table
                .columns
                .iter()
                .map(|column| column.key.clone())
                .collect::<Vec<_>>()
                .join("/")
        ),
        ParameterSchema::Matrix(_) => " 整表提案（parameters.cells，逐格 row/col/value）".into(),
        // W7 §5.4 机械新增臂（T-W7-3a 编译豁免申报）：Graph/Curve 值以标量
        // `graph`/`curve` 键装 JSON 文本，访谈按标量参数提示——旧 schema 零行为改动。
        ParameterSchema::Graph(_) => " 参数字段：graph（GraphSpec 形态 JSON 文本）".into(),
        ParameterSchema::Curve(_) => " 参数字段：curve（CurveSpec 形态 JSON 文本）".into(),
    }
}

fn parse_proposal(point: &DecisionPoint, text: &str) -> Adm4Result<InterviewProposal> {
    let value: serde_json::Value = serde_json::from_str(text.trim()).map_err(|error| {
        Adm4Error::validation(format!("AI 提案不是合法 JSON：{error}；原文：{text}"))
    })?;
    let option_id = value
        .get("option_id")
        .and_then(|item| item.as_str())
        .ok_or_else(|| Adm4Error::validation("AI 提案缺少 option_id"))?
        .to_string();
    let option = point.option(&option_id).ok_or_else(|| {
        Adm4Error::validation(format!(
            "AI 提案的选项 {option_id} 不在决策点 {} 的选项集内（发明选项被拒绝）",
            point.id
        ))
    })?;
    let rationale = value
        .get("rationale")
        .and_then(|item| item.as_str())
        .unwrap_or("")
        .to_string();
    let parameters = match value.get("parameters") {
        None | Some(serde_json::Value::Null) => ParameterValues::None,
        Some(raw) => parse_parameters(raw)?,
    };
    check_parameter_shape(point, option, &parameters)?;
    Ok(InterviewProposal {
        decision_id: point.id.clone(),
        option_id,
        rationale,
        parameters,
    })
}

/// D12/R7：AI 提案参数必须与选项 parameter_schema 的结构对上；
/// 结构不符（如 Table 点给了标量）或发明字段/列 → Err 停止，不做静默修复。
fn check_parameter_shape(
    point: &DecisionPoint,
    option: &DecisionOption,
    parameters: &ParameterValues,
) -> Adm4Result<()> {
    match (&option.parameter_schema, parameters) {
        (ParameterSchema::None, ParameterValues::None) => Ok(()),
        (ParameterSchema::None, other) => Err(Adm4Error::validation(format!(
            "决策点 {} 选项 {} 无参数结构，AI 却给出了{}（非法输出即停）",
            point.id,
            option.id,
            shape_label(other)
        ))),
        // 结构层参数可留待用户填写，缺参不算非法输出。
        (ParameterSchema::Scalar { .. }, ParameterValues::None) => Ok(()),
        (ParameterSchema::Scalar { fields }, ParameterValues::Scalars { entries }) => {
            let known: BTreeSet<&str> = fields.iter().map(|field| field.key.as_str()).collect();
            let invented: Vec<&str> = entries
                .keys()
                .map(String::as_str)
                .filter(|key| !known.contains(key))
                .collect();
            if invented.is_empty() {
                Ok(())
            } else {
                Err(Adm4Error::validation(format!(
                    "决策点 {} 的 AI 提案发明了参数字段：{}（不在 parameter_schema 内）",
                    point.id,
                    invented.join(", ")
                )))
            }
        }
        (ParameterSchema::Scalar { .. }, other) => Err(Adm4Error::validation(format!(
            "决策点 {} 是标量参数结构，AI 却给出了{}（非法输出即停）",
            point.id,
            shape_label(other)
        ))),
        (ParameterSchema::Table(table), ParameterValues::Rows { rows }) => {
            let known: BTreeSet<&str> = table
                .columns
                .iter()
                .map(|column| column.key.as_str())
                .collect();
            for (index, row) in rows.iter().enumerate() {
                let invented: Vec<&str> = row
                    .keys()
                    .map(String::as_str)
                    .filter(|key| !known.contains(key))
                    .collect();
                if !invented.is_empty() {
                    return Err(Adm4Error::validation(format!(
                        "决策点 {} 的整表提案第 {} 行发明了列：{}（不在表结构内）",
                        point.id,
                        index + 1,
                        invented.join(", ")
                    )));
                }
            }
            Ok(())
        }
        (ParameterSchema::Table(_), other) => Err(Adm4Error::validation(format!(
            "决策点 {} 是表结构（L5/L6 整表提案），AI 必须给出整表 rows，实际给出{}（非法输出即停）",
            point.id,
            shape_label(other)
        ))),
        (ParameterSchema::Matrix(_), ParameterValues::Cells { .. }) => Ok(()),
        (ParameterSchema::Matrix(_), other) => Err(Adm4Error::validation(format!(
            "决策点 {} 是矩阵结构（L5/L6 整表提案），AI 必须给出整表 cells，实际给出{}（非法输出即停）",
            point.id,
            shape_label(other)
        ))),
        // W7 §5.4 机械新增臂（T-W7-3a 编译豁免申报）：Graph/Curve 值以标量键装
        // JSON 文本 → 与 Scalar 同形（None 留待用户填、Scalars 放行、其余形态拒绝）。
        (
            ParameterSchema::Graph(_) | ParameterSchema::Curve(_),
            ParameterValues::None | ParameterValues::Scalars { .. },
        ) => Ok(()),
        (ParameterSchema::Graph(_) | ParameterSchema::Curve(_), other) => {
            Err(Adm4Error::validation(format!(
                "决策点 {} 是图/曲线参数（标量 JSON 文本承载），AI 给出了{}（非法输出即停）",
                point.id,
                shape_label(other)
            )))
        }
    }
}

fn parse_parameters(raw: &serde_json::Value) -> Adm4Result<ParameterValues> {
    if let Some(rows) = raw.get("rows").and_then(|item| item.as_array()) {
        let mut parsed_rows = Vec::new();
        for row in rows {
            parsed_rows.push(parse_scalar_map(row)?);
        }
        return Ok(ParameterValues::Rows { rows: parsed_rows });
    }
    if let Some(cells) = raw.get("cells").and_then(|item| item.as_array()) {
        let mut parsed_cells = Vec::new();
        for cell in cells {
            let row = cell
                .get("row")
                .and_then(|item| item.as_str())
                .ok_or_else(|| Adm4Error::validation("矩阵格缺少 row"))?;
            let col = cell
                .get("col")
                .and_then(|item| item.as_str())
                .ok_or_else(|| Adm4Error::validation("矩阵格缺少 col"))?;
            let value = cell
                .get("value")
                .ok_or_else(|| Adm4Error::validation("矩阵格缺少 value"))?;
            parsed_cells.push(MatrixCell {
                row: row.into(),
                col: col.into(),
                value: parse_typed_value(value)?,
            });
        }
        return Ok(ParameterValues::Cells {
            cells: parsed_cells,
        });
    }
    if let Some(map) = raw.as_object() {
        let mut entries = BTreeMap::new();
        for (key, value) in map {
            entries.insert(key.clone(), parse_typed_value(value)?);
        }
        return Ok(ParameterValues::Scalars { entries });
    }
    Err(Adm4Error::validation("无法识别的参数格式"))
}

fn parse_scalar_map(raw: &serde_json::Value) -> Adm4Result<BTreeMap<String, TypedValue>> {
    let map = raw
        .as_object()
        .ok_or_else(|| Adm4Error::validation("表行必须是对象"))?;
    let mut entries = BTreeMap::new();
    for (key, value) in map {
        entries.insert(key.clone(), parse_typed_value(value)?);
    }
    Ok(entries)
}

fn parse_typed_value(raw: &serde_json::Value) -> Adm4Result<TypedValue> {
    match raw {
        serde_json::Value::Bool(value) => Ok(TypedValue::Bool(*value)),
        serde_json::Value::Number(number) => {
            if let Some(int_value) = number.as_i64() {
                Ok(TypedValue::Int(int_value))
            } else if let Some(float_value) = number.as_f64() {
                Ok(TypedValue::Float(float_value))
            } else {
                Err(Adm4Error::validation(format!(
                    "无法表示的数值参数：{number}"
                )))
            }
        }
        serde_json::Value::String(text) => Ok(TypedValue::Text(text.clone())),
        other => Err(Adm4Error::validation(format!("不支持的参数值：{other}"))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::AuthoringState;
    use adm4_ai::ScriptedProvider;
    use adm4_contracts::{CardinalityRange, ValueKind};
    use adm4_decision::{
        DecisionGraph, DepthProfile, GenreScope, PointRequirement, ScalarField, TableSchema,
    };
    use adm4_space::{DesignSpace, GenrePack};

    fn option(id: &str) -> DecisionOption {
        DecisionOption {
            id: id.into(),
            label: id.into(),
            ..Default::default()
        }
    }

    fn structural_point(
        id: &str,
        level: DesignLevel,
        options: Vec<DecisionOption>,
    ) -> DecisionPoint {
        DecisionPoint {
            id: id.into(),
            domain: "test".into(),
            level,
            genre_scope: GenreScope::Universal,
            question: format!("{id}？"),
            mda_layer: None,
            design_question: None,
            node_id: None,
            selection_mode: adm4_decision::SelectionMode::Single,
            requirement: PointRequirement::Unlocked,
            tier_gate: None,
            options,
            skin_fields: Vec::new(),
            evidence_slots: false,
        }
    }

    fn table_point(id: &str) -> DecisionPoint {
        let mut point = structural_point(id, DesignLevel::L5, Vec::new());
        point.options = vec![DecisionOption {
            id: "full_table".into(),
            label: "整表".into(),
            parameter_schema: ParameterSchema::Table(TableSchema {
                columns: vec![
                    ScalarField {
                        key: "id".into(),
                        kind: ValueKind::Text,
                        constraint: None,
                        required: true,
                        is_skin: false,
                    },
                    ScalarField {
                        key: "cost".into(),
                        kind: ValueKind::Int,
                        constraint: None,
                        required: true,
                        is_skin: false,
                    },
                ],
                row_key: "id".into(),
                cardinality_key: "test_rows".into(),
            }),
            ..Default::default()
        }];
        point
    }

    fn scalar_point(id: &str) -> DecisionPoint {
        let mut point = structural_point(id, DesignLevel::L0, Vec::new());
        point.options = vec![DecisionOption {
            id: "opt_scalar".into(),
            label: "带参数".into(),
            parameter_schema: ParameterSchema::Scalar {
                fields: vec![ScalarField {
                    key: "statement".into(),
                    kind: ValueKind::Text,
                    constraint: None,
                    required: false,
                    is_skin: false,
                }],
            },
            ..Default::default()
        }];
        point
    }

    fn engine_with(points: Vec<DecisionPoint>) -> AuthoringEngine {
        let space = DesignSpace {
            universal_version: "test".into(),
            pack: GenrePack {
                pack_id: "test_pack".into(),
                pack_version: "0.1.0".into(),
                display_name: "测试包".into(),
                reference_games: vec!["参考甲".into(), "参考乙".into(), "参考丙".into()],
                profile_points: Vec::new(),
                cardinality_expectations: [(
                    "test_rows".to_string(),
                    CardinalityRange { min: 1, max: 10 },
                )]
                .into_iter()
                .collect(),
                consistency_rules: Vec::new(),
                nodes: Vec::new(),
                decision_points: Vec::new(),
                system_refs: Vec::new(),
                core_nouns: Vec::new(),
            },
            graph: DecisionGraph::new(points).unwrap(),
            organization: adm4_decision::DesignOrganization::default(),
            system_instances: Vec::new(),
        };
        let state = AuthoringState::new(
            "测试项目",
            "test_pack",
            "0.1.0",
            DepthProfile::new(DesignLevel::L6).unwrap(),
        );
        AuthoringEngine::new(space, state).unwrap()
    }

    fn provider_with(responses: Vec<String>) -> ScriptedProvider {
        let provider = ScriptedProvider::new();
        provider.script("interview_proposal", responses);
        provider
    }

    fn proposal_json(option_id: &str) -> String {
        format!(r#"{{"option_id":"{option_id}","rationale":"测试理由"}}"#)
    }

    fn structural_turn(engine: &mut AuthoringEngine, provider: &ScriptedProvider) -> String {
        match InterviewService::propose_next(engine, provider).unwrap() {
            InterviewTurn::StructuralPoint(proposal) => {
                let id = proposal.decision_id.clone();
                InterviewService::confirm_proposal(engine, &proposal, None).unwrap();
                id
            }
            other => panic!("期待结构层提案，得到 {other:?}"),
        }
    }

    fn row(id: &str, cost: i64) -> BTreeMap<String, TypedValue> {
        [
            ("id".to_string(), TypedValue::Text(id.into())),
            ("cost".to_string(), TypedValue::Int(cost)),
        ]
        .into_iter()
        .collect()
    }

    /// a) D9 分层推进：拓扑序（字典序）里 L1 点排最前，但 L0 未确认完不得提案 L1。
    #[test]
    fn propose_next_advances_level_by_level() {
        let mut engine = engine_with(vec![
            structural_point("a_direction", DesignLevel::L1, vec![option("dir_x")]),
            structural_point("b_profile", DesignLevel::L0, vec![option("pro_x")]),
            structural_point("c_platform", DesignLevel::L0, vec![option("pla_x")]),
        ]);
        let provider = provider_with(vec![
            proposal_json("pro_x"),
            proposal_json("pla_x"),
            proposal_json("dir_x"),
        ]);

        let progress = InterviewService::progress(&engine);
        assert_eq!(progress.current_level, Some(DesignLevel::L0));

        assert_eq!(structural_turn(&mut engine, &provider), "b_profile");
        assert_eq!(
            InterviewService::progress(&engine).current_level,
            Some(DesignLevel::L0),
            "L0 还有未确认点，不得进入 L1"
        );
        assert_eq!(structural_turn(&mut engine, &provider), "c_platform");

        // L0 全确认后才轮到 L1。
        assert_eq!(
            InterviewService::progress(&engine).current_level,
            Some(DesignLevel::L1)
        );
        assert_eq!(structural_turn(&mut engine, &provider), "a_direction");

        assert!(matches!(
            InterviewService::propose_next(&mut engine, &provider).unwrap(),
            InterviewTurn::Complete
        ));
        let progress = InterviewService::progress(&engine);
        assert!(progress.is_complete());
        assert_eq!(
            progress.levels,
            vec![
                LevelProgress {
                    level: DesignLevel::L0,
                    confirmed: 2,
                    applicable: 2
                },
                LevelProgress {
                    level: DesignLevel::L1,
                    confirmed: 1,
                    applicable: 1
                },
            ]
        );
    }

    /// b) D10 整表提案 + overrides 例外下钻，transcript 含改动摘要。
    #[test]
    fn table_proposal_confirms_whole_table_and_logs_drilldown() {
        let mut engine = engine_with(vec![table_point("t_guards")]);
        let provider = provider_with(vec![
            r#"{"option_id":"full_table","rationale":"整表建议","parameters":{"rows":[{"id":"archer","cost":100},{"id":"mage","cost":150}]}}"#
                .into(),
        ]);
        let InterviewTurn::TableProposal(proposal) =
            InterviewService::propose_next(&mut engine, &provider).unwrap()
        else {
            panic!("L5 表结构点必须走整表提案");
        };
        // 例外下钻：改 archer 造价、新增 tank 行。
        let overrides = ParameterValues::Rows {
            rows: vec![row("archer", 120), row("mage", 150), row("tank", 90)],
        };
        let problems =
            InterviewService::confirm_proposal(&mut engine, &proposal, Some(overrides.clone()))
                .unwrap();
        assert!(problems.is_empty(), "{problems:?}");

        let selection = &engine.state().selections["t_guards"];
        assert!(selection.confirmed_by_user);
        assert_eq!(selection.parameters, overrides);

        let entry = engine
            .state()
            .interview
            .transcript
            .iter()
            .rev()
            .find(|entry| entry.role == "user_confirm")
            .unwrap();
        assert!(entry.content.contains("例外下钻"), "{}", entry.content);
        assert!(
            entry.content.contains("行 archer 列 cost：100 → 120"),
            "{}",
            entry.content
        );
        assert!(entry.content.contains("新增行 tank"), "{}", entry.content);
        assert!(!entry.content.contains("mage"), "未改的行不应出现在摘要中");
    }

    /// c) D11 拒绝后不立刻重提；同层只剩被拒点时才重提；拒绝不产生任何选择。
    #[test]
    fn rejected_point_is_not_immediately_reproposed() {
        let mut engine = engine_with(vec![
            structural_point("b_profile", DesignLevel::L0, vec![option("pro_x")]),
            structural_point("c_platform", DesignLevel::L0, vec![option("pla_x")]),
        ]);
        let provider = provider_with(vec![
            proposal_json("pro_x"),
            proposal_json("pla_x"),
            proposal_json("pro_x"),
        ]);
        let InterviewTurn::StructuralPoint(first) =
            InterviewService::propose_next(&mut engine, &provider).unwrap()
        else {
            panic!("期待结构层提案");
        };
        assert_eq!(first.decision_id, "b_profile");
        InterviewService::reject_proposal(&mut engine, "b_profile", "方向不对");

        // 拒绝不产生任何选择（AI 永不代提交）。
        assert!(!engine.state().selections.contains_key("b_profile"));

        // 下一提案必须是另一个待办点。
        let InterviewTurn::StructuralPoint(second) =
            InterviewService::propose_next(&mut engine, &provider).unwrap()
        else {
            panic!("期待结构层提案");
        };
        assert_eq!(second.decision_id, "c_platform");
        InterviewService::confirm_proposal(&mut engine, &second, None).unwrap();

        // 其余待办处理完只剩被拒点 → 允许重提。
        let InterviewTurn::StructuralPoint(third) =
            InterviewService::propose_next(&mut engine, &provider).unwrap()
        else {
            panic!("期待结构层提案");
        };
        assert_eq!(third.decision_id, "b_profile");
        assert!(
            engine
                .state()
                .interview
                .transcript
                .iter()
                .any(|entry| entry.role == "user_reject")
        );
    }

    /// d-1) D12 发明选项 → Err 即停，不留 ai_proposal 痕迹。
    #[test]
    fn invented_option_stops_with_error() {
        let mut engine = engine_with(vec![structural_point(
            "b_profile",
            DesignLevel::L0,
            vec![option("pro_x")],
        )]);
        let provider = provider_with(vec![proposal_json("ghost_option")]);
        let error = InterviewService::propose_next(&mut engine, &provider).unwrap_err();
        assert!(error.message.contains("发明选项"), "{}", error.message);
        assert!(engine.state().interview.transcript.is_empty());
        assert!(engine.state().interview.cursor.is_none());
    }

    /// d-2) D12 非 JSON → Err 即停。
    #[test]
    fn non_json_output_stops_with_error() {
        let mut engine = engine_with(vec![structural_point(
            "b_profile",
            DesignLevel::L0,
            vec![option("pro_x")],
        )]);
        let provider = provider_with(vec!["我觉得选 pro_x 挺好".into()]);
        let error = InterviewService::propose_next(&mut engine, &provider).unwrap_err();
        assert!(error.message.contains("JSON"), "{}", error.message);
        assert!(engine.state().interview.transcript.is_empty());
    }

    /// d-3) D12 参数结构与 parameter_schema 不符 → Err 即停。
    #[test]
    fn schema_mismatched_parameters_stop_with_error() {
        // Table 点给了标量参数。
        let mut engine = engine_with(vec![table_point("t_guards")]);
        let provider = provider_with(vec![
            r#"{"option_id":"full_table","rationale":"x","parameters":{"cost":100}}"#.into(),
        ]);
        let error = InterviewService::propose_next(&mut engine, &provider).unwrap_err();
        assert!(error.message.contains("rows"), "{}", error.message);

        // Table 点整表提案缺参数也不行。
        let mut engine = engine_with(vec![table_point("t_guards")]);
        let provider = provider_with(vec![r#"{"option_id":"full_table","rationale":"x"}"#.into()]);
        assert!(InterviewService::propose_next(&mut engine, &provider).is_err());
    }

    fn ammo(question: &str) -> PromptEntry {
        PromptEntry {
            id: "ammo_1".into(),
            domain: "sys.test".into(),
            question_zh: question.into(),
            follow_ups: vec!["对标哪款游戏？".into()],
            source_ref: "v2#test".into(),
        }
    }

    /// 机制访谈 e-1) 范围限定：只对 `<instance>.` 前缀的待办出提案，
    /// 弹药问句注入 user_prompt（scripted 通道可断言）。
    #[test]
    fn scoped_interview_stays_in_instance_namespace_and_injects_ammo() {
        let mut engine = engine_with(vec![
            structural_point("gear_main.tier", DesignLevel::L3, vec![option("t_light")]),
            structural_point("other.tier", DesignLevel::L3, vec![option("o_x")]),
        ]);
        let provider = ScriptedProvider::new();
        provider.script(
            PURPOSE_MECHANISM,
            vec![proposal_json("t_light"), proposal_json("t_light")],
        );
        let ammo_entries = vec![ammo("强化失败要不要掉级？")];
        let InterviewTurn::StructuralPoint(proposal) = InterviewService::propose_next_scoped(
            &mut engine,
            &provider,
            "gear_main",
            &ammo_entries,
        )
        .unwrap() else {
            panic!("期待结构层提案");
        };
        assert_eq!(proposal.decision_id, "gear_main.tier");
        // 弹药问句进了 AI 上下文（验收 3 的可断言性锚点）。
        let calls = provider.calls();
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].purpose, PURPOSE_MECHANISM);
        assert!(
            calls[0].user_prompt.contains("强化失败要不要掉级？"),
            "弹药问句应注入提示词：{}",
            calls[0].user_prompt
        );
        assert!(
            calls[0].user_prompt.contains("追问弹药"),
            "{}",
            calls[0].user_prompt
        );
        // 确认后该实例无待办 → Complete（other.tier 不在范围内）。
        InterviewService::confirm_proposal(&mut engine, &proposal, None).unwrap();
        assert!(matches!(
            InterviewService::propose_next_scoped(&mut engine, &provider, "gear_main", &[])
                .unwrap(),
            InterviewTurn::Complete
        ));
        assert!(
            !engine.state().selections.contains_key("other.tier"),
            "范围外的点不得被访谈触碰"
        );
    }

    /// 机制访谈 e-2) custom 草案起草：GWT 三段非空校验；缺段即 Err；host 以调用方为准。
    #[test]
    fn custom_draft_requires_three_part_gwt() {
        let engine = engine_with(vec![structural_point(
            "sys.combat",
            DesignLevel::L3,
            vec![option("combat_x")],
        )]);
        let provider = ScriptedProvider::new();
        provider.script(
            PURPOSE_MECHANISM_CUSTOM,
            vec![
                r#"{"host_system_id":"whatever","slug":"chain_strike","label_zh":"连锁打击","rule_text":"连续命中同一目标叠层","effects":[{"effect":"custom","verb":"stack_bonus","given":"同一目标被连续命中","when":"第三次命中结算","then":"追加一次伤害"}],"rationale":"奖励专注输出"}"#
                    .into(),
            ],
        );
        let draft = InterviewService::draft_custom_mechanic(
            &engine,
            &provider,
            "sys.combat",
            "想要连击奖励机制",
            &[],
        )
        .unwrap();
        assert_eq!(draft.host_system_id, "sys.combat", "host 以调用方给定为准");
        assert_eq!(draft.slug, "chain_strike");
        assert!(!draft.rule_text.is_empty());

        // GWT 缺段 → Err。
        let provider = ScriptedProvider::new();
        provider.script(
            PURPOSE_MECHANISM_CUSTOM,
            vec![
                r#"{"host_system_id":"sys.combat","slug":"x","label_zh":"某机制","rule_text":"规则","effects":[{"effect":"custom","verb":"v","given":"","when":"w","then":"t"}],"rationale":"理由"}"#
                    .into(),
            ],
        );
        let error =
            InterviewService::draft_custom_mechanic(&engine, &provider, "sys.combat", "想法", &[])
                .unwrap_err();
        assert!(error.message.contains("given"), "{}", error.message);

        // rule_text 留白 → Err。
        let provider = ScriptedProvider::new();
        provider.script(
            PURPOSE_MECHANISM_CUSTOM,
            vec![
                r#"{"host_system_id":"sys.combat","slug":"x","label_zh":"某机制","rule_text":"  ","effects":[{"effect":"custom","verb":"v","given":"g","when":"w","then":"t"}],"rationale":"理由"}"#
                    .into(),
            ],
        );
        let error =
            InterviewService::draft_custom_mechanic(&engine, &provider, "sys.combat", "想法", &[])
                .unwrap_err();
        assert!(error.message.contains("rule_text"), "{}", error.message);
    }

    /// d-3 补充) 标量结构点发明参数字段 → Err 即停。
    #[test]
    fn invented_scalar_field_stops_with_error() {
        let mut engine = engine_with(vec![scalar_point("a_profile")]);
        let provider = provider_with(vec![
            r#"{"option_id":"opt_scalar","rationale":"x","parameters":{"ghost_field":"值"}}"#
                .into(),
        ]);
        let error = InterviewService::propose_next(&mut engine, &provider).unwrap_err();
        assert!(
            error.message.contains("发明了参数字段"),
            "{}",
            error.message
        );
    }
}
