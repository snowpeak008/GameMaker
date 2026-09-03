use crate::framework::StageStatus;
use crate::runner::RunnerContext;
use adm4_ai::AiRequest;
use adm4_contracts::{AnchoredNarrative, EvidencePointer, MeasuredMetric, SpecRef};
use adm4_foundation::{Adm4Error, Adm4Result};
use adm4_spec::{DesignNote, DesignNoteRole, GameSpec, GraphEntry, MechanicSpec, SystemSpec};
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
        let source_text = build_system_source_text(&spec, system, &mechanics);
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

/// 系统章节的 AI 叙述素材（W7 定稿 §5.5）：规格正文 + design_notes 原文 +
/// 图结构摘要（graphs 非空时）。注记与摘要只作背景素材进 user_prompt，
/// AI 约束提示词（system_prompt）不变。
fn build_system_source_text(
    spec: &GameSpec,
    system: &SystemSpec,
    mechanics: &[&MechanicSpec],
) -> String {
    let mut source_text = format!(
        "系统 {}：{}\n机制：\n{}",
        system.name,
        system.purpose,
        mechanics
            .iter()
            .map(|mechanic| format!("- {}", mechanic.rule_text))
            .collect::<Vec<_>>()
            .join("\n")
    );
    source_text.push_str(&render_design_notes(system, mechanics));
    source_text.push_str(&render_graph_summary(spec));
    source_text
}

/// 系统章节的设计注记原文块（W7 定稿 §5.5）：系统自身 + 该系统下机制的
/// design_notes 逐条列出（角色 + 来源决策/选项 + 原文）。全部为空 → 空串（不加空标题）。
fn render_design_notes(system: &SystemSpec, mechanics: &[&MechanicSpec]) -> String {
    let mut lines = Vec::new();
    for note in &system.design_notes {
        lines.push(render_note_line(note));
    }
    for mechanic in mechanics {
        for note in &mechanic.design_notes {
            lines.push(render_note_line(note));
        }
    }
    if lines.is_empty() {
        return String::new();
    }
    format!("\n设计注记（只作背景，不新增设计）：\n{}", lines.join("\n"))
}

fn render_note_line(note: &DesignNote) -> String {
    let role = match note.role {
        DesignNoteRole::Rationale => "理由",
        DesignNoteRole::Statement => "陈述",
    };
    format!(
        "- [{role}] {}（来源：{}/{}）",
        note.text, note.source_decision, note.source_option
    )
}

/// 图结构摘要（W7 定稿 §5.5）：graphs 非空时给每个图一行「节点/边/入口计数」，
/// 入口计数 = 入度 0 的节点数（按 from→to 计入度，与 GraphSpec 校验同口径），
/// 附声明的入口约束；只给统计不展开负载；graphs 为空 → 空串。
fn render_graph_summary(spec: &GameSpec) -> String {
    if spec.graphs.is_empty() {
        return String::new();
    }
    let lines: Vec<String> = spec
        .graphs
        .iter()
        .map(|graph| {
            let entry_count = graph
                .nodes
                .iter()
                .filter(|node| graph.edges.iter().all(|edge| edge.to != node.id))
                .count();
            let entry_constraint = match graph.entry {
                GraphEntry::Single => "单一入口",
                GraphEntry::Multiple => "多入口",
            };
            format!(
                "- 图 {}：节点 {} 个 / 边 {} 条 / 入口 {entry_count} 个（约束：{entry_constraint}）",
                graph.id,
                graph.nodes.len(),
                graph.edges.len()
            )
        })
        .collect();
    format!("\n图结构摘要：\n{}", lines.join("\n"))
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

#[cfg(test)]
mod tests {
    use super::*;
    use adm4_spec::{GraphNode, GraphSpec};

    fn system(notes: Vec<DesignNote>) -> SystemSpec {
        SystemSpec {
            id: "combat".into(),
            name: "战斗".into(),
            purpose: "克制关系".into(),
            interfaces: Vec::new(),
            design_notes: notes,
        }
    }

    fn mechanic(notes: Vec<DesignNote>) -> MechanicSpec {
        MechanicSpec {
            id: "damage".into(),
            system_id: "combat".into(),
            rule_text: "伤害 = 攻击 × 克制系数".into(),
            preconditions: Vec::new(),
            effects: vec![adm4_spec::EffectSpec::EmitSignal {
                signal: "hit".into(),
            }],
            state_machine: None,
            design_notes: notes,
        }
    }

    fn note(role: DesignNoteRole, text: &str) -> DesignNote {
        DesignNote {
            source_decision: "mech.damage".into(),
            source_option: "linear".into(),
            role,
            text: text.into(),
        }
    }

    fn spec_with(system: SystemSpec, mechanic: MechanicSpec, graphs: Vec<GraphSpec>) -> GameSpec {
        GameSpec {
            identity: adm4_spec::SpecIdentity {
                schema_version: adm4_spec::SPEC_SCHEMA_VERSION.into(),
                project_id: "p".into(),
                frozen_hash: "sha256:x".into(),
            },
            intent: Default::default(),
            systems: vec![system],
            mechanics: vec![mechanic],
            entities: Vec::new(),
            tables: Vec::new(),
            content: Vec::new(),
            graphs,
            acceptance: Vec::new(),
            source_map: Vec::new(),
        }
    }

    /// source_text 追加系统与机制的 design_notes 原文（角色标注 + 来源可追溯）。
    #[test]
    fn source_text_appends_design_notes() {
        let sys = system(vec![note(DesignNoteRole::Statement, "以小博大的压力曲线")]);
        let mech = mechanic(vec![note(
            DesignNoteRole::Rationale,
            "线性公式便于新手理解",
        )]);
        let spec = spec_with(sys.clone(), mech.clone(), Vec::new());
        let text = build_system_source_text(&spec, &spec.systems[0], &[&spec.mechanics[0]]);
        assert!(text.contains("设计注记"), "{text}");
        assert!(text.contains("[陈述] 以小博大的压力曲线"), "{text}");
        assert!(text.contains("[理由] 线性公式便于新手理解"), "{text}");
        assert!(text.contains("mech.damage/linear"), "{text}");
        // 规格正文仍在前（注记只是追加素材，不替换正文）。
        assert!(text.starts_with("系统 战斗："), "{text}");
    }

    /// 注记全空 → source_text 不出现空的「设计注记」标题（零噪音）。
    #[test]
    fn empty_notes_add_no_header() {
        let spec = spec_with(system(Vec::new()), mechanic(Vec::new()), Vec::new());
        let text = build_system_source_text(&spec, &spec.systems[0], &[&spec.mechanics[0]]);
        assert!(!text.contains("设计注记"), "{text}");
        assert!(!text.contains("图结构摘要"), "{text}");
    }

    /// graphs 非空 → source_text 追加图结构摘要（节点数/边数/入口约束）。
    #[test]
    fn source_text_appends_graph_summary_when_graphs_present() {
        let graph = GraphSpec {
            id: "talent_tree".into(),
            directed: true,
            acyclic: true,
            entry: adm4_spec::GraphEntry::Single,
            nodes: vec![
                GraphNode {
                    id: "root".into(),
                    payload: Default::default(),
                    is_skin_fields: Vec::new(),
                },
                GraphNode {
                    id: "leaf".into(),
                    payload: Default::default(),
                    is_skin_fields: Vec::new(),
                },
            ],
            edges: vec![adm4_spec::GraphEdge {
                from: "root".into(),
                to: "leaf".into(),
                payload: Default::default(),
            }],
            design_notes: Vec::new(),
        };
        let spec = spec_with(system(Vec::new()), mechanic(Vec::new()), vec![graph]);
        let text = build_system_source_text(&spec, &spec.systems[0], &[&spec.mechanics[0]]);
        assert!(text.contains("图结构摘要"), "{text}");
        // root→leaf 一条边：入度 0 的节点只有 root，入口计数 1。
        assert!(
            text.contains("图 talent_tree：节点 2 个 / 边 1 条 / 入口 1 个（约束：单一入口）"),
            "{text}"
        );
    }
}
