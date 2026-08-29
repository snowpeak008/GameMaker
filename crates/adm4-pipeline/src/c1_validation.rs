use crate::framework::StageStatus;
use crate::runner::RunnerContext;
use adm4_ai::AiRequest;
use adm4_contracts::{CategoryEvidence, ReviewProof, verify_review_batch};
use adm4_foundation::{Adm4Error, Adm4Result, sha256_hex};
use adm4_spec::{GameSpec, validate_game_spec};
use serde::{Deserialize, Serialize};

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
}

pub fn execute(ctx: &RunnerContext<'_>) -> Adm4Result<StageStatus> {
    let spec: GameSpec = ctx.store.read_contract("C0")?;

    // 机器规则：零违例硬门。
    let static_violations: Vec<String> = validate_game_spec(&spec)
        .into_iter()
        .map(|violation| format!("[{}] {}", violation.code, violation.message))
        .collect();
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
    };
    let document = format!(
        "# C1 验证与红队报告\n\n- 静态违例：0\n- 红队发现：{}（无 blocker）\n- 评审者：{}\n- 评审数量：{}/{}\n\n> 本文档由 contract.json 渲染，请勿手改。\n",
        contract.redteam_findings.len(),
        contract.proof.reviewer,
        contract.proof.reviewed_count,
        contract.proof.upstream_count
    );
    ctx.store.write_stage("C1", &contract, &document)?;
    Ok(StageStatus::Succeeded)
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
}
