use crate::framework::StageStatus;
use crate::runner::RunnerContext;
use adm4_ai::AiRequest;
use adm4_contracts::{AnchoredNarrative, EvidencePointer, MeasuredMetric, SpecRef};
use adm4_foundation::{Adm4Error, Adm4Result};
use adm4_spec::GameSpec;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DocSection {
    pub id: String,
    pub title: String,
    pub narrative: AnchoredNarrative,
}

/// C2 契约：玩法设计文档（章节 → Spec 锚定表）。锚定覆盖率必须 100%（R4）。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GameplayDocContract {
    pub sections: Vec<DocSection>,
    pub anchor_coverage: MeasuredMetric,
}

pub fn execute(ctx: &RunnerContext<'_>) -> Adm4Result<StageStatus> {
    let spec: GameSpec = ctx.store.read_contract("C0")?;
    let mut sections = Vec::new();

    // 概览章节（锚定 intent）。
    sections.push(narrate_section(
        ctx,
        &spec,
        "overview",
        "游戏概览",
        &format!(
            "标题：{}\n体验承诺：{}\n品类结构：{}",
            spec.intent.title, spec.intent.experience_promise, spec.intent.genre_structure
        ),
        vec![SpecRef::new("intent")],
    )?);

    // 每个系统一章（锚定系统 + 该系统下机制）。
    for system in &spec.systems {
        let mechanics: Vec<_> = spec
            .mechanics
            .iter()
            .filter(|mechanic| mechanic.system_id == system.id)
            .collect();
        let mut anchors = vec![SpecRef::new(format!("systems/{}", system.id))];
        anchors.extend(
            mechanics
                .iter()
                .map(|mechanic| SpecRef::new(format!("mechanics/{}", mechanic.id))),
        );
        let source_text = format!(
            "系统 {}：{}\n机制：\n{}",
            system.name,
            system.purpose,
            mechanics
                .iter()
                .map(|mechanic| format!("- {}", mechanic.rule_text))
                .collect::<Vec<_>>()
                .join("\n")
        );
        sections.push(narrate_section(
            ctx,
            &spec,
            &format!("system_{}", system.id),
            &format!("系统：{}", system.name),
            &source_text,
            anchors,
        )?);
    }

    // 锚定覆盖核对：每个 mechanic/system 必须被 ≥1 章节锚定；每个锚定必须真实存在。
    let mut evidence = Vec::new();
    let mut uncovered = Vec::new();
    for path in spec
        .systems
        .iter()
        .map(|system| format!("systems/{}", system.id))
        .chain(
            spec.mechanics
                .iter()
                .map(|mechanic| format!("mechanics/{}", mechanic.id)),
        )
    {
        let covering = sections.iter().find(|section| {
            section
                .narrative
                .anchors
                .iter()
                .any(|anchor| anchor.0 == path)
        });
        match covering {
            Some(section) => evidence.push(EvidencePointer {
                file: "C2/contract.json".into(),
                path: path.clone(),
                observed: format!("章节 {} 锚定", section.id),
            }),
            None => uncovered.push(path),
        }
    }
    for section in &sections {
        for anchor in &section.narrative.anchors {
            if !spec.contains_ref(anchor) {
                return Err(Adm4Error::red_line(format!(
                    "R4: 章节 {} 锚定了不存在的 spec 路径 {}",
                    section.id, anchor.0
                )));
            }
        }
    }
    if !uncovered.is_empty() {
        return Err(Adm4Error::validation(format!(
            "C2 锚定覆盖率未达 100%，未覆盖：{}",
            uncovered.join(", ")
        )));
    }
    let total = (spec.systems.len() + spec.mechanics.len()) as f64;
    let anchor_coverage = MeasuredMetric::new(if total == 0.0 { 0.0 } else { 1.0 }, evidence)?;

    let contract = GameplayDocContract {
        sections: sections.clone(),
        anchor_coverage,
    };
    let mut document = format!("# {} · 玩法设计文档\n\n", spec.intent.title);
    for section in &sections {
        document.push_str(&format!(
            "## {}\n\n{}\n\n<sub>锚定：{}</sub>\n\n",
            section.title,
            section.narrative.text,
            section
                .narrative
                .anchors
                .iter()
                .map(|anchor| format!("`{}`", anchor.0))
                .collect::<Vec<_>>()
                .join(" ")
        ));
    }
    document.push_str("> 本文档由 contract.json 渲染，请勿手改。\n");
    ctx.store.write_stage("C2", &contract, &document)?;
    Ok(StageStatus::Succeeded)
}

/// 用 AI 生成一段叙述；产出必须是锚定叙述（R4），AI 不可用 = Err（R7）。
fn narrate_section(
    ctx: &RunnerContext<'_>,
    _spec: &GameSpec,
    id: &str,
    title: &str,
    source_text: &str,
    anchors: Vec<SpecRef>,
) -> Adm4Result<DocSection> {
    let request = AiRequest {
        purpose: "c2_narrative".into(),
        system_prompt: "你是玩法文档撰写者。只能基于给出的规格内容改写为流畅叙述，\
                        不得引入规格中不存在的设计。输出 JSON：{\"text\": ...}。"
            .into(),
        user_prompt: source_text.to_string(),
        expect_json: true,
    };
    let response = ctx.ai.invoke(&request)?;
    let value: serde_json::Value = serde_json::from_str(response.text.trim())
        .map_err(|error| Adm4Error::validation(format!("C2 叙述产出不是合法 JSON：{error}")))?;
    let text = value
        .get("text")
        .and_then(|text| text.as_str())
        .ok_or_else(|| Adm4Error::validation("C2 叙述缺少 text 字段"))?;
    Ok(DocSection {
        id: id.to_string(),
        title: title.to_string(),
        narrative: AnchoredNarrative::new(text, anchors)?,
    })
}
