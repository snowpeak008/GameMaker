use crate::evidence::EvidenceCandidate;
use crate::model::{CertificationStatus, Confidence, Evidence, Template, TemplateAnswer};
use adm4_ai::{AiProvider, AiRequest};
use adm4_contracts::{MatrixCell, TypedValue};
use adm4_decision::{DecisionPoint, ParameterValues};
use adm4_foundation::{Adm4Error, Adm4Result, sha256_hex};
use std::collections::{BTreeMap, BTreeSet};

/// S2 映射调用的 purpose 标识（ScriptedProvider 脚本与调用日志按此键路由）。
pub const MAPPING_PURPOSE: &str = "template_mapping";

/// S2 AI 映射填答（设计决定 D6）：把 S1 的证据候选映射为逆向答卷。
pub struct MappingService;

impl MappingService {
    /// 把证据候选交给 AI 映射为 [`TemplateAnswer`] 列表，写入模板并推进 `Draft→Mapped`。
    ///
    /// 强制约束：
    /// - 每条答案必须挂至少一条 [`Evidence`]（含 source_url），缺证据整卷拒收（R1）；
    /// - 证据来源必须出自 `candidates` 的 source_url（禁止 AI 编造来源，宁缺勿造）；
    /// - AI 输出非 JSON、引用不存在的决策点/选项、重复映射同一决策点 → 直接 Err，
    ///   不做任何修复兜底（R7）；
    /// - 校验全部通过后才写入模板，失败时模板保持 Draft 原状。
    ///
    /// 返回映射条数。查不到证据的决策点由 AI 留空（不输出），缺口进 coverage。
    pub fn map_answers(
        provider: &dyn AiProvider,
        template: &mut Template,
        points: &[DecisionPoint],
        candidates: &[EvidenceCandidate],
    ) -> Adm4Result<usize> {
        if template.certification.status != CertificationStatus::Draft {
            return Err(Adm4Error::blocked(format!(
                "模板 {} 当前认证状态 {:?}，只有 Draft 可执行 AI 映射",
                template.template_id, template.certification.status
            )));
        }
        let request = build_mapping_request(template, points, candidates);
        let response = provider.invoke(&request)?;
        let answers = parse_answers(&response.text, points, candidates)?;
        let count = answers.len();
        template.answers = answers;
        // 记录本次映射会话的应答哈希：S3 用它与核验会话比对，识别「第二会话复读第一会话」（R3）。
        template.mapping_hash = sha256_hex(response.text.as_bytes());
        template.crosscheck_proof = None;
        template
            .certification
            .advance_to(CertificationStatus::Mapped)?;
        Ok(count)
    }
}

fn build_mapping_request(
    template: &Template,
    points: &[DecisionPoint],
    candidates: &[EvidenceCandidate],
) -> AiRequest {
    let points_text = points
        .iter()
        .map(|point| {
            let options = point
                .options
                .iter()
                .map(|option| format!("    - option_id={} label={}", option.id, option.label))
                .collect::<Vec<_>>()
                .join("\n");
            format!(
                "- decision_id={}（{}）：{}\n{options}",
                point.id,
                point.level.label(),
                point.question
            )
        })
        .collect::<Vec<_>>()
        .join("\n");
    let candidates_text = candidates
        .iter()
        .map(|candidate| {
            format!(
                "- source_url={} source_type={:?} title={} snippet={}",
                candidate.source_url, candidate.source_type, candidate.title, candidate.snippet
            )
        })
        .collect::<Vec<_>>()
        .join("\n");
    AiRequest {
        purpose: MAPPING_PURPOSE.into(),
        system_prompt: "你是游戏逆向答卷映射员。只依据给出的证据候选，把游戏在各决策点的\
                        实际做法映射为「选哪个选项」。只能引用给出的 decision_id/option_id 与\
                        证据 source_url，不得发明。查不到证据的决策点不要输出（宁缺勿造）。\
                        输出 JSON 数组，每项：{\"decision_id\":…, \"option_id\":…, \
                        \"evidence\":[{\"source_url\":…, \"quote\":…, \"confidence\":\"high|med|low\"}], \
                        \"parameters\":可选, \"notes\":可选}。"
            .into(),
        user_prompt: format!(
            "逆向目标游戏：{}\n\n决策点与候选选项：\n{points_text}\n\n证据候选：\n{candidates_text}",
            template.game_name
        ),
        expect_json: true,
    }
}

fn parse_answers(
    text: &str,
    points: &[DecisionPoint],
    candidates: &[EvidenceCandidate],
) -> Adm4Result<Vec<TemplateAnswer>> {
    let value: serde_json::Value = serde_json::from_str(text.trim()).map_err(|error| {
        Adm4Error::validation(format!("AI 映射输出不是合法 JSON：{error}；原文：{text}"))
    })?;
    let items = value
        .as_array()
        .ok_or_else(|| Adm4Error::validation("AI 映射输出必须是 JSON 数组（R7：非法输出即停）"))?;
    let mut seen = BTreeSet::new();
    let mut answers = Vec::new();
    for item in items {
        answers.push(parse_answer(item, points, candidates, &mut seen)?);
    }
    Ok(answers)
}

fn parse_answer(
    item: &serde_json::Value,
    points: &[DecisionPoint],
    candidates: &[EvidenceCandidate],
    seen: &mut BTreeSet<String>,
) -> Adm4Result<TemplateAnswer> {
    let object = item
        .as_object()
        .ok_or_else(|| Adm4Error::validation("AI 映射条目必须是 JSON 对象"))?;
    let decision_id = require_string(object, "decision_id", "AI 映射条目缺少 decision_id")?;
    let point = points
        .iter()
        .find(|point| point.id == decision_id)
        .ok_or_else(|| {
            Adm4Error::validation(format!(
                "AI 映射引用了不存在的决策点 {decision_id}（R7：非法输出直接报错）"
            ))
        })?;
    if !seen.insert(decision_id.clone()) {
        return Err(Adm4Error::validation(format!(
            "AI 映射重复输出了决策点 {decision_id}"
        )));
    }
    let option_id = require_string(object, "option_id", "AI 映射条目缺少 option_id")?;
    if point.option(&option_id).is_none() {
        return Err(Adm4Error::validation(format!(
            "AI 映射的选项 {option_id} 不在决策点 {decision_id} 的选项集内（发明选项被拒绝）"
        )));
    }
    let evidence_items = object
        .get("evidence")
        .and_then(|raw| raw.as_array())
        .filter(|list| !list.is_empty())
        .ok_or_else(|| {
            Adm4Error::red_line(format!(
                "决策点 {decision_id} 的答案没有携带任何证据（R1：无证据即整条拒收）"
            ))
        })?;
    let mut evidence = Vec::new();
    for raw in evidence_items {
        evidence.push(parse_evidence(raw, candidates, &decision_id)?);
    }
    let parameters = match object.get("parameters") {
        None | Some(serde_json::Value::Null) => ParameterValues::None,
        Some(raw) => parse_parameters(raw)?,
    };
    let notes = object
        .get("notes")
        .and_then(|raw| raw.as_str())
        .unwrap_or("")
        .to_string();
    Ok(TemplateAnswer {
        decision_id,
        option_id,
        parameters,
        evidence,
        notes,
        crosscheck_agreed: None,
        // S2 映射当前只产出单选答案（AI 提示词不含多选契约）；多选答卷来自批量迁移通道。
        additional_options: Vec::new(),
        primary_option: None,
    })
}

fn parse_evidence(
    raw: &serde_json::Value,
    candidates: &[EvidenceCandidate],
    decision_id: &str,
) -> Adm4Result<Evidence> {
    let object = raw
        .as_object()
        .ok_or_else(|| Adm4Error::validation("证据条目必须是 JSON 对象"))?;
    let source_url = require_string(object, "source_url", "证据缺少 source_url（R1：宁缺勿造）")?;
    if source_url.trim().is_empty() {
        return Err(Adm4Error::red_line(format!(
            "决策点 {decision_id} 的证据 source_url 为空（R1：宁缺勿造）"
        )));
    }
    let candidate = candidates
        .iter()
        .find(|candidate| candidate.source_url == source_url)
        .ok_or_else(|| {
            Adm4Error::validation(format!(
                "决策点 {decision_id} 的证据来源 {source_url} 不在检索候选集内（禁止编造来源）"
            ))
        })?;
    let quote = object
        .get("quote")
        .and_then(|value| value.as_str())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        // 引文缺省用候选快照的 snippet（同源同内容，非编造）。
        .unwrap_or_else(|| candidate.snippet.clone());
    let confidence = match object.get("confidence").and_then(|value| value.as_str()) {
        Some("high") => Confidence::High,
        Some("med") => Confidence::Med,
        Some("low") => Confidence::Low,
        other => {
            return Err(Adm4Error::validation(format!(
                "决策点 {decision_id} 的证据 confidence 非法（应为 high|med|low，得到 {other:?}）"
            )));
        }
    };
    Ok(Evidence {
        source_url,
        quote,
        // 来源类型以检索候选为准，AI 不得改写。
        source_type: candidate.source_type,
        confidence,
    })
}

fn require_string(
    object: &serde_json::Map<String, serde_json::Value>,
    key: &str,
    message: &str,
) -> Adm4Result<String> {
    object
        .get(key)
        .and_then(|value| value.as_str())
        .map(str::to_string)
        .ok_or_else(|| Adm4Error::validation(message.to_string()))
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
            } else {
                number
                    .as_f64()
                    .map(TypedValue::Float)
                    .ok_or_else(|| Adm4Error::validation(format!("无法解析的数值：{number}")))
            }
        }
        serde_json::Value::String(text) => Ok(TypedValue::Text(text.clone())),
        other => Err(Adm4Error::validation(format!("不支持的参数值：{other}"))),
    }
}
