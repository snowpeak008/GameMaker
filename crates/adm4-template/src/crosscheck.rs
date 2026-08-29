use crate::model::{CertificationStatus, CrossCheckProof, Template};
use adm4_ai::{AiProvider, AiRequest};
use adm4_contracts::{CategoryEvidence, ReviewProof, verify_review_batch};
use adm4_decision::DecisionPoint;
use adm4_foundation::{Adm4Error, Adm4Result, UtcTimestamp, sha256_hex};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

/// S3 交叉核验调用的 purpose 标识（与 S2 的 [`crate::MAPPING_PURPOSE`] 互异，保证独立会话）。
pub const CROSSCHECK_PURPOSE: &str = "template_crosscheck";

/// 单条核验结论：一致 / 冲突（冲突 = 待人工，不采信任一方）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CrossCheckVerdict {
    Consistent,
    Conflict,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CrossCheckEntry {
    pub decision_id: String,
    pub verdict: CrossCheckVerdict,
    #[serde(default)]
    pub reason: String,
}

/// S3 逐条核验报告：覆盖答卷中的每个决策点（不多不少）。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct CrossCheckReport {
    pub entries: Vec<CrossCheckEntry>,
}

impl CrossCheckReport {
    /// 冲突条目的决策点 id（这些条目在模板中被标记为待人工）。
    pub fn conflict_ids(&self) -> Vec<String> {
        self.entries
            .iter()
            .filter(|entry| entry.verdict == CrossCheckVerdict::Conflict)
            .map(|entry| entry.decision_id.clone())
            .collect()
    }
}

/// S3 交叉核验（设计决定 D7）：用第二次**独立** AI 会话对照 S2 的映射结果。
pub struct CrossCheckService;

impl CrossCheckService {
    /// 对照映射结果并推进 `Mapped→CrossChecked`。
    ///
    /// 独立性：本方法重新构造全新的 [`AiRequest`]（purpose 与提示词均不同于 S2），
    /// 不携带首次映射会话的任何上下文。
    ///
    /// 结论落地：`Consistent` → `crosscheck_agreed=Some(true)`；`Conflict` →
    /// `Some(false)`（待人工——coverage 的 conflicted 清单据此呈现，人工审核前
    /// 不采信任一方）。AI 输出非 JSON、结论值非法、遗漏或多报决策点 → 直接 Err（R7），
    /// 模板保持 Mapped 原状。
    ///
    /// 空答卷（S2 宁缺勿造产出零条）无需 AI 复核，直接推进并返回空报告。
    pub fn cross_check(
        provider: &dyn AiProvider,
        template: &mut Template,
        points: &[DecisionPoint],
    ) -> Adm4Result<CrossCheckReport> {
        if template.certification.status != CertificationStatus::Mapped {
            return Err(Adm4Error::blocked(format!(
                "模板 {} 当前认证状态 {:?}，只有 Mapped 可执行交叉核验",
                template.template_id, template.certification.status
            )));
        }
        if template.answers.is_empty() {
            template
                .certification
                .advance_to(CertificationStatus::CrossChecked)?;
            return Ok(CrossCheckReport::default());
        }
        let request = build_crosscheck_request(template, points);
        let response = provider.invoke(&request)?;
        let crosscheck_hash = sha256_hex(response.text.as_bytes());
        verify_two_session_independence(template, &crosscheck_hash, &response.provider_id)?;
        let report = parse_report(&response.text, template)?;
        for answer in &mut template.answers {
            let agreed = report
                .entries
                .iter()
                .find(|entry| entry.decision_id == answer.decision_id)
                .map(|entry| entry.verdict == CrossCheckVerdict::Consistent);
            answer.crosscheck_agreed = agreed;
        }
        template
            .certification
            .advance_to(CertificationStatus::CrossChecked)?;
        // 机器证据：两会话哈希留档，事后可复核核验确实来自独立的第二会话（R3）。
        template.crosscheck_proof = Some(CrossCheckProof {
            mapping_hash: template.mapping_hash.clone(),
            crosscheck_hash,
            checked_count: report.entries.len(),
            checked_at: UtcTimestamp::now().to_iso8601(),
        });
        Ok(report)
    }
}

/// R3 两会话互异校验：S2 映射与 S3 核验的应答内容哈希全同 → 第二会话在复读第一
/// 会话，核验不成立 → 拒绝（不改模板状态）。
///
/// 在调用侧把两次会话包装成同批 [`ReviewProof`] 交给 [`verify_review_batch`]：
/// 逐条覆盖的答卷条数即评审数量，`per_category_evidence` 记录各会话查了什么。
fn verify_two_session_independence(
    template: &Template,
    crosscheck_hash: &str,
    provider_id: &str,
) -> Adm4Result<()> {
    if template.mapping_hash.is_empty() {
        return Err(Adm4Error::red_line(format!(
            "模板 {} 没有 S2 映射会话哈希，无法证明 S3 是独立的第二会话（R3）；请重跑 S2 映射后再核验",
            template.template_id
        )));
    }
    let count = template.answers.len();
    let batch = [
        ReviewProof {
            reviewer: format!("s2_mapping:{}", template.template_id),
            reviewed_count: count,
            upstream_count: count,
            content_hash: template.mapping_hash.clone(),
            per_category_evidence: vec![CategoryEvidence {
                category: "s2_mapping".into(),
                checked: format!("证据候选 → 逆向答卷 {count} 条"),
                conclusion: "映射会话产出答卷".into(),
                evidence: Vec::new(),
            }],
        },
        ReviewProof {
            reviewer: format!("s3_crosscheck:{provider_id}"),
            reviewed_count: count,
            upstream_count: count,
            content_hash: crosscheck_hash.to_string(),
            per_category_evidence: vec![CategoryEvidence {
                category: "s3_crosscheck".into(),
                checked: format!("逐条对照答卷 {count} 条与其证据"),
                conclusion: "核验会话产出逐条结论".into(),
                evidence: Vec::new(),
            }],
        },
    ];
    verify_review_batch(&batch).map_err(|error| {
        Adm4Error::red_line(format!(
            "R3: S3 交叉核验与 S2 映射的会话应答完全相同（第二会话复读第一会话，判橡皮图章）：{}",
            error.message
        ))
    })
}

fn build_crosscheck_request(template: &Template, points: &[DecisionPoint]) -> AiRequest {
    let answers_text = template
        .answers
        .iter()
        .map(|answer| {
            let question = points
                .iter()
                .find(|point| point.id == answer.decision_id)
                .map(|point| point.question.clone())
                .unwrap_or_default();
            let evidence = answer
                .evidence
                .iter()
                .map(|item| format!("    - {}：{}", item.source_url, item.quote))
                .collect::<Vec<_>>()
                .join("\n");
            format!(
                "- decision_id={}（{question}）选了 option_id={}，证据：\n{evidence}",
                answer.decision_id, answer.option_id
            )
        })
        .collect::<Vec<_>>()
        .join("\n");
    AiRequest {
        purpose: CROSSCHECK_PURPOSE.into(),
        system_prompt: "你是独立复核员，与首次映射不共享任何上下文。逐条对照给出的答案与其\
                        证据：证据能支撑所选选项判 consistent，证据不支撑、互相矛盾或与你\
                        对该游戏的公开事实认知冲突判 conflict。必须覆盖每个 decision_id，\
                        不得增删。输出 JSON 数组，每项：{\"decision_id\":…, \
                        \"verdict\":\"consistent|conflict\", \"reason\":…}。"
            .into(),
        user_prompt: format!(
            "逆向目标游戏：{}\n\n待核验答卷：\n{answers_text}",
            template.game_name
        ),
        expect_json: true,
    }
}

fn parse_report(text: &str, template: &Template) -> Adm4Result<CrossCheckReport> {
    let value: serde_json::Value = serde_json::from_str(text.trim()).map_err(|error| {
        Adm4Error::validation(format!("AI 核验输出不是合法 JSON：{error}；原文：{text}"))
    })?;
    let items = value
        .as_array()
        .ok_or_else(|| Adm4Error::validation("AI 核验输出必须是 JSON 数组（R7：非法输出即停）"))?;
    let mut seen = BTreeSet::new();
    let mut entries = Vec::new();
    for item in items {
        let object = item
            .as_object()
            .ok_or_else(|| Adm4Error::validation("核验条目必须是 JSON 对象"))?;
        let decision_id = object
            .get("decision_id")
            .and_then(|raw| raw.as_str())
            .ok_or_else(|| Adm4Error::validation("核验条目缺少 decision_id"))?
            .to_string();
        if !template
            .answers
            .iter()
            .any(|answer| answer.decision_id == decision_id)
        {
            return Err(Adm4Error::validation(format!(
                "核验报告引用了答卷之外的决策点 {decision_id}（R7：非法输出直接报错）"
            )));
        }
        if !seen.insert(decision_id.clone()) {
            return Err(Adm4Error::validation(format!(
                "核验报告重复输出了决策点 {decision_id}"
            )));
        }
        let verdict = match object.get("verdict").and_then(|raw| raw.as_str()) {
            Some("consistent") => CrossCheckVerdict::Consistent,
            Some("conflict") => CrossCheckVerdict::Conflict,
            other => {
                return Err(Adm4Error::validation(format!(
                    "决策点 {decision_id} 的核验结论非法（应为 consistent|conflict，得到 {other:?}）"
                )));
            }
        };
        let reason = object
            .get("reason")
            .and_then(|raw| raw.as_str())
            .unwrap_or("")
            .to_string();
        entries.push(CrossCheckEntry {
            decision_id,
            verdict,
            reason,
        });
    }
    for answer in &template.answers {
        if !seen.contains(&answer.decision_id) {
            return Err(Adm4Error::validation(format!(
                "核验报告缺少决策点 {} 的结论（必须逐条覆盖，R7）",
                answer.decision_id
            )));
        }
    }
    Ok(CrossCheckReport { entries })
}
