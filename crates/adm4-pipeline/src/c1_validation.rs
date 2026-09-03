use crate::framework::StageStatus;
use crate::runner::RunnerContext;
use adm4_ai::AiRequest;
use adm4_contracts::{CategoryEvidence, ReviewProof, verify_review_batch};
use adm4_foundation::{Adm4Error, Adm4Result, sha256_hex};
use adm4_spec::{EffectSpec, GameSpec, validate_game_spec};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

/// 红队发现的合法严重度枚举；缺失或超出枚举一律拒收（不得默认降级为 warning）。
const FINDING_SEVERITIES: [&str; 2] = ["blocker", "warning"];

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RedTeamFinding {
    pub id: String,
    pub severity: String,
    pub target: String,
    pub text: String,
}

/// C1 契约：静态验证 + AI 红队（携带工作量证明）。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ValidationContract {
    pub static_violations: Vec<String>,
    pub redteam_findings: Vec<RedTeamFinding>,
    pub proof: ReviewProof,
    /// 含 Custom 效果的机制清单——custom 是逃生舱口，列入红队必审
    /// （W7 定稿 §5.7 C1 小改；渲染进 C1 文档的待审节）。
    #[serde(default)]
    pub custom_review_targets: Vec<String>,
}

pub fn execute(ctx: &RunnerContext<'_>) -> Adm4Result<StageStatus> {
    let spec: GameSpec = ctx.store.read_contract("C0")?;

    // 机器规则：零违例硬门（spec 级校验 + W7 波 1 追加清单）。
    let mut static_violations: Vec<String> = validate_game_spec(&spec)
        .into_iter()
        .map(|violation| format!("[{}] {}", violation.code, violation.message))
        .collect();
    static_violations.extend(collect_extended_violations(&spec));
    if !static_violations.is_empty() {
        return Err(Adm4Error::validation(format!(
            "C1 静态验证 {} 项违例：{}",
            static_violations.len(),
            static_violations.join("; ")
        )));
    }

    // AI 红队（必需；失败 = blocked，R7 无兜底）。
    let upstream_count = spec.mechanics.len() + spec.systems.len();
    let request = AiRequest {
        purpose: "c1_redteam".into(),
        system_prompt: "你是对抗性规格评审员。逐条检查 GameSpec 的系统与机制，找出规则矛盾、\
                        不可实现点、数值缺口。输出 JSON：{\"findings\":[{\"id\":...,\
                        \"severity\":\"blocker|warning\",\"target\":\"mechanics/xxx\",\"text\":...}],\
                        \"per_category\":[{\"category\":...,\"checked\":...,\"conclusion\":...}]}。\
                        id 与 target 每条必填（缺一即拒收）；severity=blocker 的 text 必须写明理由，\
                        不得留空。"
            .into(),
        user_prompt: serde_json::to_string_pretty(&spec)
            .map_err(|error| Adm4Error::internal(format!("spec serialize failed: {error}")))?,
        expect_json: true,
    };
    let response = ctx.ai.invoke(&request)?;
    let value: serde_json::Value = serde_json::from_str(response.text.trim())
        .map_err(|error| Adm4Error::validation(format!("C1 红队产出不是合法 JSON：{error}")))?;
    let findings = parse_findings(&value)?;
    let per_category: Vec<CategoryEvidence> = value
        .get("per_category")
        .and_then(|items| items.as_array())
        .map(|items| {
            items
                .iter()
                .map(|item| CategoryEvidence {
                    category: item
                        .get("category")
                        .and_then(|category| category.as_str())
                        .unwrap_or("")
                        .into(),
                    checked: item
                        .get("checked")
                        .and_then(|checked| checked.as_str())
                        .unwrap_or("")
                        .into(),
                    conclusion: item
                        .get("conclusion")
                        .and_then(|conclusion| conclusion.as_str())
                        .unwrap_or("")
                        .into(),
                    evidence: Vec::new(),
                })
                .collect()
        })
        .unwrap_or_default();
    let proof = ReviewProof {
        reviewer: format!("{}:{}", response.provider_id, response.model),
        reviewed_count: upstream_count,
        upstream_count,
        content_hash: sha256_hex(response.text.as_bytes()),
        per_category_evidence: per_category,
    };
    // R3：C1 红队与冻结门第 4 道的红队是同一批决策的两次评审，内容哈希全同 →
    // 判橡皮图章（第二次评审复读第一次）→ 拒绝。冻结产物未带红队证明时退化为单份校验。
    match &ctx.frozen.red_team_proof {
        Some(freeze_proof) => {
            verify_review_batch(&[freeze_proof.clone(), proof.clone()]).map_err(|error| {
                Adm4Error::red_line(format!(
                    "R3: C1 红队与冻结门红队的评审批次校验未过：{}",
                    error.message
                ))
            })?
        }
        None => proof.verify()?,
    }

    let blockers: Vec<&RedTeamFinding> = findings
        .iter()
        .filter(|finding| finding.severity == "blocker")
        .collect();
    if !blockers.is_empty() {
        let detail: Vec<String> = blockers
            .iter()
            .map(|finding| format!("{}: {}", finding.target, finding.text))
            .collect();
        return Err(Adm4Error::blocked(format!(
            "C1 红队发现 {} 项 blocker（需回设计工具修改并重新冻结）：{}",
            blockers.len(),
            detail.join("; ")
        )));
    }

    let contract = ValidationContract {
        static_violations,
        redteam_findings: findings,
        proof,
        custom_review_targets: collect_custom_review_targets(&spec),
    };
    let mut document = format!(
        "# C1 验证与红队报告\n\n- 静态违例：0\n- 红队发现：{}（无 blocker）\n- 评审者：{}\n- 评审数量：{}/{}\n\n",
        contract.redteam_findings.len(),
        contract.proof.reviewer,
        contract.proof.reviewed_count,
        contract.proof.upstream_count
    );
    // custom 机制列入红队必审清单（W7 定稿 §5.7）：仅在存在 Custom 效果时渲染，
    // 无 custom 的项目文档字节不变（金样零漂移）。
    if !contract.custom_review_targets.is_empty() {
        document.push_str("## Custom 机制待审（红队必审）\n\n");
        for mechanic_id in &contract.custom_review_targets {
            document.push_str(&format!(
                "- `mechanics/{mechanic_id}`：含 Custom 效果（逃生舱口转录），红队须人工核对 GWT 模板与 rule_text 的一致性\n"
            ));
        }
        document.push('\n');
    }
    document.push_str("> 本文档由 contract.json 渲染，请勿手改。\n");
    ctx.store.write_stage("C1", &contract, &document)?;
    Ok(StageStatus::Succeeded)
}

/// W7 波 1 追加的 C1 校验清单（定稿 §5.7 C1 小改）：
/// - graphs 引用闭合：验收场景 source_refs 指向的 `graphs/<id>` 必须真实存在
///   （source_map 悬空已由 spec 级校验兜住，这里补验收面的闭合）；
/// - ModifyRule target 存在性复检：target_rule 必须是 spec 内真实机制 id
///   （C0/GameSpec 校验层拦截的兜底复检，嵌套效果内同查）；
/// - Custom GWT 模板非空复检：C0 按 R2 拦截的兜底（嵌套效果内同查）。
fn collect_extended_violations(spec: &GameSpec) -> Vec<String> {
    let mut violations = Vec::new();
    for scenario in &spec.acceptance {
        for source_ref in &scenario.source_refs {
            if source_ref.0.starts_with("graphs/") && !spec.contains_ref(source_ref) {
                violations.push(format!(
                    "[c1_graph_ref_dangling] 验收场景 {} 引用了不存在的图 {}",
                    scenario.id, source_ref.0
                ));
            }
        }
    }
    let mechanic_ids: BTreeSet<&str> = spec
        .mechanics
        .iter()
        .map(|mechanic| mechanic.id.as_str())
        .collect();
    for mechanic in &spec.mechanics {
        for effect in &mechanic.effects {
            check_effect_recursively(&mechanic.id, effect, &mechanic_ids, &mut violations);
        }
    }
    violations
}

/// 单个效果的 C1 复检（穷尽匹配无 `_` 臂；嵌套效果递归下探）。
fn check_effect_recursively(
    mechanic_id: &str,
    effect: &EffectSpec,
    mechanic_ids: &BTreeSet<&str>,
    violations: &mut Vec<String>,
) {
    match effect {
        EffectSpec::ModifyProperty { .. }
        | EffectSpec::SpawnEntity { .. }
        | EffectSpec::DespawnEntity { .. }
        | EffectSpec::ChangeState { .. }
        | EffectSpec::GrantResource { .. }
        | EffectSpec::ConsumeResource { .. }
        | EffectSpec::EmitSignal { .. }
        | EffectSpec::Displace { .. }
        | EffectSpec::Attach { .. }
        | EffectSpec::Detach { .. }
        | EffectSpec::DrawFromPool { .. } => {}
        EffectSpec::AreaApply { inner, .. } | EffectSpec::Schedule { inner, .. } => {
            for nested in inner {
                check_effect_recursively(mechanic_id, nested, mechanic_ids, violations);
            }
        }
        EffectSpec::RollCheck {
            on_success,
            on_failure,
            ..
        } => {
            for nested in on_success.iter().chain(on_failure.iter()) {
                check_effect_recursively(mechanic_id, nested, mechanic_ids, violations);
            }
        }
        EffectSpec::ModifyRule { target_rule, .. } => {
            if !mechanic_ids.contains(target_rule.as_str()) {
                violations.push(format!(
                    "[c1_modify_rule_dangling] 机制 {mechanic_id} 的 ModifyRule 目标 {target_rule} 不存在（须为 spec 内真实机制 id）"
                ));
            }
        }
        EffectSpec::Custom {
            verb,
            given,
            when_,
            then,
            ..
        } => {
            if given.trim().is_empty() || when_.trim().is_empty() || then.trim().is_empty() {
                violations.push(format!(
                    "[c1_custom_gwt_incomplete] 机制 {mechanic_id} 的 Custom 效果（verb={verb}）GWT 三段模板不完整（C0 拦截的兜底复检）"
                ));
            }
        }
    }
}

/// 收集含 Custom 效果的机制 id（含嵌套内的 Custom）——红队必审清单。
fn collect_custom_review_targets(spec: &GameSpec) -> Vec<String> {
    let mut targets = Vec::new();
    for mechanic in &spec.mechanics {
        if mechanic.effects.iter().any(effect_contains_custom) {
            targets.push(mechanic.id.clone());
        }
    }
    targets
}

fn effect_contains_custom(effect: &EffectSpec) -> bool {
    match effect {
        EffectSpec::Custom { .. } => true,
        EffectSpec::AreaApply { inner, .. } | EffectSpec::Schedule { inner, .. } => {
            inner.iter().any(effect_contains_custom)
        }
        EffectSpec::RollCheck {
            on_success,
            on_failure,
            ..
        } => on_success
            .iter()
            .chain(on_failure.iter())
            .any(effect_contains_custom),
        EffectSpec::ModifyProperty { .. }
        | EffectSpec::SpawnEntity { .. }
        | EffectSpec::DespawnEntity { .. }
        | EffectSpec::ChangeState { .. }
        | EffectSpec::GrantResource { .. }
        | EffectSpec::ConsumeResource { .. }
        | EffectSpec::EmitSignal { .. }
        | EffectSpec::Displace { .. }
        | EffectSpec::Attach { .. }
        | EffectSpec::Detach { .. }
        | EffectSpec::ModifyRule { .. }
        | EffectSpec::DrawFromPool { .. } => false,
    }
}

/// 解析红队发现清单（R2 未知即停）。
///
/// `findings` 键缺失 → Err：无法区分「AI 没查」与「查完零发现」，默认零发现会让
/// 红队门形同虚设；AI 确认无发现时必须显式输出空数组。
/// 单条发现的 `severity` 缺失或不在枚举内 → Err：默认成 warning 会把 blocker 静默降级。
/// `id`/`target` 缺失 → Err：定位不到规格位置的发现无法回设计工具处置。
/// blocker 的 `text` 缺失或空白 → Err：阻断流水线却说不出理由，用户无从修改。
fn parse_findings(value: &serde_json::Value) -> Adm4Result<Vec<RedTeamFinding>> {
    let items = value
        .get("findings")
        .ok_or_else(|| {
            Adm4Error::validation(
                "C1 红队产出缺少 findings 键（R2：缺输入即停；确认零发现请显式输出 \"findings\": []）",
            )
        })?
        .as_array()
        .ok_or_else(|| Adm4Error::validation("C1 红队产出的 findings 必须是数组"))?;
    let mut findings = Vec::with_capacity(items.len());
    for (index, item) in items.iter().enumerate() {
        let severity = item
            .get("severity")
            .and_then(|severity| severity.as_str())
            .map(str::trim)
            .filter(|severity| FINDING_SEVERITIES.contains(severity))
            .ok_or_else(|| {
                Adm4Error::validation(format!(
                    "C1 红队第 {} 条发现的 severity 缺失或非法（只接受 {}；禁止默认降级为 warning）",
                    index + 1,
                    FINDING_SEVERITIES.join("|")
                ))
            })?;
        let id = required_field(item, "id", index)?;
        let target = required_field(item, "target", index)?;
        let text = item
            .get("text")
            .and_then(|text| text.as_str())
            .map(str::trim)
            .unwrap_or("");
        if severity == "blocker" && text.is_empty() {
            return Err(Adm4Error::validation(format!(
                "C1 红队第 {} 条发现（{id}）是 blocker 但缺少 text：blocker 会阻断流水线，必须给出可读理由",
                index + 1
            )));
        }
        findings.push(RedTeamFinding {
            id,
            severity: severity.into(),
            target,
            text: text.into(),
        });
    }
    Ok(findings)
}

/// 取单条发现的必填定位字段；缺失或全空白即拒收（无法定位的发现不可处置）。
fn required_field(item: &serde_json::Value, key: &str, index: usize) -> Adm4Result<String> {
    item.get(key)
        .and_then(|value| value.as_str())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .ok_or_else(|| {
            Adm4Error::validation(format!(
                "C1 红队第 {} 条发现缺少 {key}：无法定位到具体规格位置的发现不可处置",
                index + 1
            ))
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_findings_key_is_rejected() {
        let value = serde_json::json!({ "per_category": [] });
        let error = parse_findings(&value).unwrap_err();
        assert!(error.message.contains("findings"), "{}", error.message);
    }

    #[test]
    fn explicit_empty_findings_is_accepted() {
        let value = serde_json::json!({ "findings": [] });
        assert!(parse_findings(&value).unwrap().is_empty());
    }

    #[test]
    fn missing_or_unknown_severity_is_rejected() {
        let missing = serde_json::json!({ "findings": [{ "id": "f1", "target": "mechanics/x", "text": "隐患" }] });
        assert!(
            parse_findings(&missing)
                .unwrap_err()
                .message
                .contains("severity")
        );
        let unknown = serde_json::json!({ "findings": [{ "id": "f1", "severity": "critical", "target": "mechanics/x", "text": "隐患" }] });
        assert!(
            parse_findings(&unknown)
                .unwrap_err()
                .message
                .contains("severity")
        );
    }

    #[test]
    fn blocker_severity_survives_parsing() {
        let value = serde_json::json!({
            "findings": [{ "id": "f1", "severity": "blocker", "target": "mechanics/x", "text": "矛盾" }]
        });
        let findings = parse_findings(&value).unwrap();
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].severity, "blocker");
        assert_eq!(findings[0].id, "f1");
        assert_eq!(findings[0].target, "mechanics/x");
    }

    #[test]
    fn missing_id_or_target_is_rejected_for_any_severity() {
        let no_id = serde_json::json!({
            "findings": [{ "severity": "warning", "target": "mechanics/x", "text": "隐患" }]
        });
        assert!(parse_findings(&no_id).unwrap_err().message.contains("id"));
        let no_target = serde_json::json!({
            "findings": [{ "id": "f1", "severity": "warning", "text": "隐患" }]
        });
        assert!(
            parse_findings(&no_target)
                .unwrap_err()
                .message
                .contains("target")
        );
        let blank_target = serde_json::json!({
            "findings": [{ "id": "f1", "severity": "blocker", "target": "   ", "text": "矛盾" }]
        });
        assert!(
            parse_findings(&blank_target)
                .unwrap_err()
                .message
                .contains("target")
        );
    }

    #[test]
    fn blocker_without_readable_text_is_rejected() {
        let missing = serde_json::json!({
            "findings": [{ "id": "f1", "severity": "blocker", "target": "mechanics/x" }]
        });
        let error = parse_findings(&missing).unwrap_err();
        assert!(error.message.contains("text"), "{}", error.message);
        let blank = serde_json::json!({
            "findings": [{ "id": "f1", "severity": "blocker", "target": "mechanics/x", "text": "  \n " }]
        });
        assert!(parse_findings(&blank).unwrap_err().message.contains("text"));
    }

    #[test]
    fn warning_may_omit_text() {
        let value = serde_json::json!({
            "findings": [{ "id": "w1", "severity": "warning", "target": "mechanics/x" }]
        });
        let findings = parse_findings(&value).unwrap();
        assert_eq!(findings.len(), 1);
        assert!(findings[0].text.is_empty());
    }

    // ===== W7 波 1 追加校验（T-W7-1b）=====

    use adm4_contracts::SpecRef;
    use adm4_spec::{
        AcceptanceScenario, EntitySpec, MechanicSpec, ProjectIntent, SPEC_SCHEMA_VERSION,
        SpecIdentity, SystemSpec, VisualForm,
    };

    fn minimal_spec() -> GameSpec {
        GameSpec {
            identity: SpecIdentity {
                schema_version: SPEC_SCHEMA_VERSION.into(),
                project_id: "p1".into(),
                frozen_hash: "sha256:abc".into(),
            },
            intent: ProjectIntent::default(),
            systems: vec![SystemSpec {
                id: "combat".into(),
                name: "战斗".into(),
                purpose: String::new(),
                interfaces: Vec::new(),
                design_notes: Vec::new(),
            }],
            mechanics: vec![MechanicSpec {
                id: "damage".into(),
                system_id: "combat".into(),
                rule_text: "伤害规则".into(),
                preconditions: Vec::new(),
                effects: vec![EffectSpec::ModifyProperty {
                    entity: "enemy".into(),
                    property: "hp".into(),
                    formula: "hp - damage".into(),
                }],
                state_machine: None,
                design_notes: Vec::new(),
            }],
            entities: vec![EntitySpec {
                id: "enemy".into(),
                name: "敌人".into(),
                visual_form: Some(VisualForm::Sprite2d),
                properties: Vec::new(),
            }],
            tables: Vec::new(),
            content: Vec::new(),
            graphs: Vec::new(),
            acceptance: Vec::new(),
            source_map: Vec::new(),
        }
    }

    /// ModifyRule target 悬空被复检拦截；指向真实机制 id 则放行。
    #[test]
    fn modify_rule_dangling_target_is_flagged() {
        let mut spec = minimal_spec();
        spec.mechanics[0].effects.push(EffectSpec::ModifyRule {
            target_rule: "ghost_rule".into(),
            patch: Default::default(),
            priority: 0,
        });
        let violations = collect_extended_violations(&spec);
        assert!(
            violations
                .iter()
                .any(|violation| violation.contains("c1_modify_rule_dangling")
                    && violation.contains("ghost_rule")),
            "{violations:?}"
        );

        // 指向真实机制 id（自指也算真实）则零违例。
        let mut ok = minimal_spec();
        ok.mechanics[0].effects.push(EffectSpec::ModifyRule {
            target_rule: "damage".into(),
            patch: Default::default(),
            priority: 0,
        });
        assert!(collect_extended_violations(&ok).is_empty());
    }

    /// 嵌套（Schedule 内层）的 ModifyRule 悬空同样被拦。
    #[test]
    fn nested_modify_rule_dangling_target_is_flagged() {
        let mut spec = minimal_spec();
        spec.mechanics[0].effects.push(EffectSpec::Schedule {
            timing: Default::default(),
            amount_expr: "1".into(),
            unit: Default::default(),
            inner: vec![EffectSpec::ModifyRule {
                target_rule: "ghost_rule".into(),
                patch: Default::default(),
                priority: 0,
            }],
        });
        assert!(
            collect_extended_violations(&spec)
                .iter()
                .any(|violation| violation.contains("c1_modify_rule_dangling"))
        );
    }

    /// Custom GWT 三段不全被复检拦截（C0 拦截的兜底）。
    #[test]
    fn custom_incomplete_gwt_is_flagged() {
        let mut spec = minimal_spec();
        spec.mechanics[0].effects.push(EffectSpec::Custom {
            verb: "merge".into(),
            operands: Default::default(),
            given: "g".into(),
            when_: String::new(),
            then: "t".into(),
        });
        assert!(
            collect_extended_violations(&spec)
                .iter()
                .any(|violation| violation.contains("c1_custom_gwt_incomplete"))
        );
    }

    /// 验收场景引用不存在的图被拦；引用存在的图放行。
    #[test]
    fn dangling_graph_ref_in_acceptance_is_flagged() {
        let mut spec = minimal_spec();
        spec.acceptance.push(AcceptanceScenario {
            id: "s1".into(),
            capability_id: "cap_damage".into(),
            given: vec!["g".into()],
            when: vec!["w".into()],
            then: vec!["t".into()],
            source_refs: vec![SpecRef::new("graphs/ghost_map")],
        });
        assert!(
            collect_extended_violations(&spec)
                .iter()
                .any(|violation| violation.contains("c1_graph_ref_dangling")
                    && violation.contains("ghost_map"))
        );
    }

    /// custom 机制（含嵌套内 Custom）进红队必审清单；无 custom 项目清单为空。
    #[test]
    fn custom_mechanics_enter_review_targets() {
        let spec = minimal_spec();
        assert!(collect_custom_review_targets(&spec).is_empty());

        let mut with_custom = minimal_spec();
        with_custom.mechanics[0].effects.push(EffectSpec::Schedule {
            timing: Default::default(),
            amount_expr: "1".into(),
            unit: Default::default(),
            inner: vec![EffectSpec::Custom {
                verb: "merge".into(),
                operands: Default::default(),
                given: "g".into(),
                when_: "w".into(),
                then: "t".into(),
            }],
        });
        assert_eq!(
            collect_custom_review_targets(&with_custom),
            vec!["damage".to_string()]
        );
    }
}
