use crate::effects::EffectSpec;
use crate::graph::{GraphEntry, GraphSpec};
use crate::model::{GameSpec, SPEC_SCHEMA_VERSION};
use adm4_foundation::{Adm4Error, Adm4Result, ContentHash};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpecViolation {
    pub code: String,
    pub message: String,
}

/// GameSpec 结构校验：引用完整性 + source_map 全覆盖 + 公式非空。
pub fn validate_game_spec(spec: &GameSpec) -> Vec<SpecViolation> {
    let mut violations = Vec::new();
    if spec.identity.schema_version != SPEC_SCHEMA_VERSION {
        violations.push(SpecViolation {
            code: "schema_version".into(),
            message: format!(
                "schema_version {} != {SPEC_SCHEMA_VERSION}",
                spec.identity.schema_version
            ),
        });
    }
    if spec.identity.frozen_hash.is_empty() {
        violations.push(SpecViolation {
            code: "missing_frozen_hash".into(),
            message: "GameSpec 未绑定冻结集哈希".into(),
        });
    }

    let system_ids: BTreeSet<&str> = spec.systems.iter().map(|item| item.id.as_str()).collect();
    let entity_ids: BTreeSet<&str> = spec.entities.iter().map(|item| item.id.as_str()).collect();

    for mechanic in &spec.mechanics {
        if !system_ids.contains(mechanic.system_id.as_str()) {
            violations.push(SpecViolation {
                code: "mechanic_dangling_system".into(),
                message: format!(
                    "机制 {} 引用了不存在的系统 {}",
                    mechanic.id, mechanic.system_id
                ),
            });
        }
        if mechanic.rule_text.trim().is_empty() {
            violations.push(SpecViolation {
                code: "mechanic_empty_rule".into(),
                message: format!("机制 {} 的规则文本为空（L4 必须达公式符号级）", mechanic.id),
            });
        }
        if mechanic.effects.is_empty() {
            violations.push(SpecViolation {
                code: "mechanic_no_effects".into(),
                message: format!("机制 {} 没有任何效果", mechanic.id),
            });
        }
        for effect in &mechanic.effects {
            collect_custom_gwt_violations(&mechanic.id, effect, &mut violations);
            if let EffectSpec::ModifyProperty { entity, .. }
            | EffectSpec::SpawnEntity { entity }
            | EffectSpec::DespawnEntity { entity } = effect
            {
                // 允许引用单个实体 id 或实体类（entity_table 决策 id 前缀）。
                let class_prefix = format!("{entity}.");
                let known = entity_ids.contains(entity.as_str())
                    || entity_ids
                        .iter()
                        .any(|candidate| candidate.starts_with(&class_prefix));
                if !known {
                    violations.push(SpecViolation {
                        code: "effect_dangling_entity".into(),
                        message: format!("机制 {} 的效果引用了不存在的实体 {entity}", mechanic.id),
                    });
                }
            }
        }
    }

    for graph in &spec.graphs {
        validate_graph(graph, &mut violations);
    }

    // source_map 全覆盖：每个 spec 元素必须能追溯到决策 id。
    let mapped: BTreeSet<&str> = spec
        .source_map
        .iter()
        .map(|entry| entry.spec_path.0.as_str())
        .collect();
    for path in spec.all_ref_paths() {
        if !mapped.contains(path.0.as_str()) {
            violations.push(SpecViolation {
                code: "source_map_gap".into(),
                message: format!("spec 元素 {} 无法追溯到决策（source_map 缺口）", path.0),
            });
        }
    }
    for entry in &spec.source_map {
        if !spec.contains_ref(&entry.spec_path) {
            violations.push(SpecViolation {
                code: "source_map_dangling".into(),
                message: format!("source_map 引用了不存在的 spec 路径 {}", entry.spec_path.0),
            });
        }
    }

    violations
}

/// Custom 效果的 GWT 三段非空规则（递归遍历嵌套效果）。
///
/// 类型层不拦缺失（缺键旧档可读，serde default 兜底）；本规则在 spec 级
/// 校验入口生效，C0 按 R2 阻塞属波 1 的 C0 编译臂。
fn collect_custom_gwt_violations(
    mechanic_id: &str,
    effect: &EffectSpec,
    violations: &mut Vec<SpecViolation>,
) {
    match effect {
        EffectSpec::Custom {
            verb,
            given,
            when_,
            then,
            ..
        } => {
            if given.trim().is_empty() || when_.trim().is_empty() || then.trim().is_empty() {
                violations.push(SpecViolation {
                    code: "custom_gwt_incomplete".into(),
                    message: format!(
                        "机制 {mechanic_id} 的 Custom 效果（verb={verb}）GWT 三段模板不完整（given/when/then 必须全部非空）"
                    ),
                });
            }
        }
        EffectSpec::AreaApply { inner, .. } | EffectSpec::Schedule { inner, .. } => {
            for nested in inner {
                collect_custom_gwt_violations(mechanic_id, nested, violations);
            }
        }
        EffectSpec::RollCheck {
            on_success,
            on_failure,
            ..
        } => {
            for nested in on_success.iter().chain(on_failure.iter()) {
                collect_custom_gwt_violations(mechanic_id, nested, violations);
            }
        }
        _ => {}
    }
}

/// GraphSpec 结构校验：边端点存在 + acyclic 声明则成环检查 + entry 约束。
fn validate_graph(graph: &GraphSpec, violations: &mut Vec<SpecViolation>) {
    let node_ids: BTreeSet<&str> = graph.nodes.iter().map(|node| node.id.as_str()).collect();

    let mut has_dangling = false;
    for edge in &graph.edges {
        for endpoint in [edge.from.as_str(), edge.to.as_str()] {
            if !node_ids.contains(endpoint) {
                has_dangling = true;
                violations.push(SpecViolation {
                    code: "graph_dangling_edge".into(),
                    message: format!(
                        "图 {} 的边 {}→{} 引用了未声明节点 {endpoint}",
                        graph.id, edge.from, edge.to
                    ),
                });
            }
        }
    }
    // 悬空边已单独报告；后续结构检查只在端点闭合时有意义。
    if has_dangling {
        return;
    }

    if graph.acyclic && has_cycle(graph) {
        violations.push(SpecViolation {
            code: "graph_cycle_in_acyclic".into(),
            message: format!("图 {} 声明 acyclic=true 但存在环", graph.id),
        });
    }

    if graph.entry == GraphEntry::Single {
        let mut indegree: BTreeMap<&str, usize> = graph
            .nodes
            .iter()
            .map(|node| (node.id.as_str(), 0))
            .collect();
        for edge in &graph.edges {
            if let Some(count) = indegree.get_mut(edge.to.as_str()) {
                *count += 1;
            }
        }
        let entry_count = indegree.values().filter(|count| **count == 0).count();
        if entry_count != 1 {
            violations.push(SpecViolation {
                code: "graph_entry_violation".into(),
                message: format!(
                    "图 {} 声明 entry=single 但入度 0 节点有 {entry_count} 个（要求恰 1）",
                    graph.id
                ),
            });
        }
    }
}

/// 成环检测（Kahn 拓扑排序：消不完节点即有环）。
///
/// 无向图（directed=false）声明 acyclic 时按有向读法检查——无向成环
/// 语义由波 1 的编译臂按模块口径细化，本层先保守只查有向环。
fn has_cycle(graph: &GraphSpec) -> bool {
    let mut indegree: BTreeMap<&str, usize> = graph
        .nodes
        .iter()
        .map(|node| (node.id.as_str(), 0))
        .collect();
    let mut adjacency: BTreeMap<&str, Vec<&str>> = BTreeMap::new();
    for edge in &graph.edges {
        adjacency
            .entry(edge.from.as_str())
            .or_default()
            .push(edge.to.as_str());
        if let Some(count) = indegree.get_mut(edge.to.as_str()) {
            *count += 1;
        }
    }
    let mut queue: Vec<&str> = indegree
        .iter()
        .filter(|(_, count)| **count == 0)
        .map(|(id, _)| *id)
        .collect();
    let mut visited = 0usize;
    while let Some(current) = queue.pop() {
        visited += 1;
        if let Some(next_nodes) = adjacency.get(current) {
            for next in next_nodes {
                if let Some(count) = indegree.get_mut(next) {
                    *count -= 1;
                    if *count == 0 {
                        queue.push(next);
                    }
                }
            }
        }
    }
    visited != graph.nodes.len()
}

/// 规范化内容哈希。
pub fn spec_content_hash(spec: &GameSpec) -> Adm4Result<String> {
    let value = serde_json::to_value(spec)
        .map_err(|error| Adm4Error::internal(format!("spec serialize failed: {error}")))?;
    Ok(ContentHash::of_canonical_json(&value)?.0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graph::{GraphEdge, GraphNode};
    use crate::model::*;
    use adm4_contracts::SpecRef;

    fn minimal_spec() -> GameSpec {
        GameSpec {
            identity: SpecIdentity {
                schema_version: SPEC_SCHEMA_VERSION.into(),
                project_id: "p1".into(),
                frozen_hash: "sha256:abc".into(),
            },
            intent: ProjectIntent {
                title: "测试".into(),
                ..Default::default()
            },
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
                rule_text: "伤害 = 基础 × 系数".into(),
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
            source_map: vec![
                SpecSourceEntry {
                    spec_path: SpecRef::new("intent"),
                    decision_id: "u.title".into(),
                },
                SpecSourceEntry {
                    spec_path: SpecRef::new("systems/combat"),
                    decision_id: "ld.systems".into(),
                },
                SpecSourceEntry {
                    spec_path: SpecRef::new("mechanics/damage"),
                    decision_id: "ld.damage".into(),
                },
                SpecSourceEntry {
                    spec_path: SpecRef::new("entities/enemy"),
                    decision_id: "ld.enemies".into(),
                },
            ],
        }
    }

    #[test]
    fn valid_minimal_spec_passes() {
        assert!(validate_game_spec(&minimal_spec()).is_empty());
    }

    #[test]
    fn source_map_gap_detected() {
        let mut spec = minimal_spec();
        spec.source_map.pop();
        assert!(
            validate_game_spec(&spec)
                .iter()
                .any(|violation| violation.code == "source_map_gap")
        );
    }

    #[test]
    fn dangling_entity_detected() {
        let mut spec = minimal_spec();
        spec.entities.clear();
        assert!(
            validate_game_spec(&spec)
                .iter()
                .any(|violation| violation.code == "effect_dangling_entity")
        );
    }

    #[test]
    fn hash_is_deterministic() {
        assert_eq!(
            spec_content_hash(&minimal_spec()).unwrap(),
            spec_content_hash(&minimal_spec()).unwrap()
        );
    }

    fn node(id: &str) -> GraphNode {
        GraphNode {
            id: id.into(),
            payload: Default::default(),
            is_skin_fields: Vec::new(),
        }
    }

    fn edge(from: &str, to: &str) -> GraphEdge {
        GraphEdge {
            from: from.into(),
            to: to.into(),
            payload: Default::default(),
        }
    }

    fn graph(id: &str, acyclic: bool, entry: GraphEntry) -> GraphSpec {
        GraphSpec {
            id: id.into(),
            directed: true,
            acyclic,
            entry,
            nodes: vec![node("a"), node("b"), node("c")],
            edges: vec![edge("a", "b"), edge("b", "c")],
            design_notes: Vec::new(),
        }
    }

    fn spec_with_graph(graph: GraphSpec) -> GameSpec {
        let mut spec = minimal_spec();
        spec.source_map.push(SpecSourceEntry {
            spec_path: SpecRef::new(format!("graphs/{}", graph.id)),
            decision_id: "ld.graph".into(),
        });
        spec.graphs.push(graph);
        spec
    }

    /// 合法图（有向无环、单入口）通过全部校验。
    #[test]
    fn valid_graph_passes() {
        let spec = spec_with_graph(graph("map", true, GraphEntry::Single));
        assert!(validate_game_spec(&spec).is_empty());
    }

    #[test]
    fn graph_dangling_edge_detected() {
        let mut bad = graph("map", false, GraphEntry::Multiple);
        bad.edges.push(edge("c", "ghost"));
        let violations = validate_game_spec(&spec_with_graph(bad));
        assert!(violations.iter().any(|v| v.code == "graph_dangling_edge"));
    }

    /// 假 acyclic：声明无环但实际成环。
    #[test]
    fn graph_false_acyclic_detected() {
        let mut bad = graph("map", true, GraphEntry::Multiple);
        bad.edges.push(edge("c", "a"));
        let violations = validate_game_spec(&spec_with_graph(bad));
        assert!(
            violations
                .iter()
                .any(|v| v.code == "graph_cycle_in_acyclic")
        );
        // 同构图声明 acyclic=false 则放行（对话 hub 回环合法）。
        let mut cyclic_ok = graph("dialogue", false, GraphEntry::Multiple);
        cyclic_ok.edges.push(edge("c", "a"));
        assert!(validate_game_spec(&spec_with_graph(cyclic_ok)).is_empty());
    }

    /// entry=Single 但入度 0 节点不为 1（孤立节点使入口数为 2）。
    #[test]
    fn graph_entry_violation_detected() {
        let mut bad = graph("map", true, GraphEntry::Single);
        bad.nodes.push(node("orphan"));
        let violations = validate_game_spec(&spec_with_graph(bad));
        assert!(violations.iter().any(|v| v.code == "graph_entry_violation"));
    }

    /// graphs 进 all_ref_paths / contains_ref（R4 锚定闭合）。
    #[test]
    fn graphs_in_ref_paths() {
        let spec = spec_with_graph(graph("map", true, GraphEntry::Single));
        assert!(
            spec.all_ref_paths()
                .iter()
                .any(|path| path.0 == "graphs/map")
        );
        assert!(spec.contains_ref(&SpecRef::new("graphs/map")));
        assert!(!spec.contains_ref(&SpecRef::new("graphs/ghost")));
        // 缺 source_map 条目时报缺口——graphs 已纳入追溯分母。
        let mut gap = spec;
        gap.source_map.pop();
        assert!(
            validate_game_spec(&gap)
                .iter()
                .any(|v| v.code == "source_map_gap")
        );
    }

    /// design_notes 缺键可读（旧档五挂点均无该键）+ 带值往返。
    #[test]
    fn design_notes_missing_keys_readable() {
        let raw = r#"{"id":"combat","name":"战斗"}"#;
        let system: SystemSpec = serde_json::from_str(raw).unwrap();
        assert!(system.design_notes.is_empty());

        let noted = r#"{"id":"combat","name":"战斗","design_notes":[{"source_decision":"d1","source_option":"o1","role":"statement","text":"变异器设计说明"}]}"#;
        let system: SystemSpec = serde_json::from_str(noted).unwrap();
        assert_eq!(system.design_notes.len(), 1);
        assert_eq!(system.design_notes[0].role, DesignNoteRole::Statement);
        let rejson = serde_json::to_string(&system).unwrap();
        let back: SystemSpec = serde_json::from_str(&rejson).unwrap();
        assert_eq!(system, back);

        // 其余四挂点同样缺键可读。
        let table: TableSpec =
            serde_json::from_str(r#"{"id":"t","columns":[],"row_key":"k"}"#).unwrap();
        assert!(table.design_notes.is_empty());
        let content: ContentSpec =
            serde_json::from_str(r#"{"id":"c","content_kind":"wave","data":{}}"#).unwrap();
        assert!(content.design_notes.is_empty());
        let graph: GraphSpec = serde_json::from_str(r#"{"id":"g"}"#).unwrap();
        assert!(graph.design_notes.is_empty());
        let mechanic: MechanicSpec =
            serde_json::from_str(r#"{"id":"m","system_id":"s","rule_text":"r","effects":[]}"#)
                .unwrap();
        assert!(mechanic.design_notes.is_empty());
    }

    /// 旧 GameSpec 整档 JSON（无 graphs/design_notes 键）反序列化 + 校验语义不变。
    #[test]
    fn legacy_game_spec_json_still_valid() {
        let raw = r#"{
            "identity": {"schema_version": "4.0.0", "project_id": "p1", "frozen_hash": "sha256:abc"},
            "intent": {"title": "测试"},
            "systems": [{"id": "combat", "name": "战斗"}],
            "mechanics": [{"id": "damage", "system_id": "combat", "rule_text": "伤害 = 基础 × 系数",
                "effects": [{"effect": "modify_property", "entity": "enemy", "property": "hp", "formula": "hp - damage"}]}],
            "entities": [{"id": "enemy", "name": "敌人", "visual_form": "sprite2d"}],
            "tables": [],
            "content": [],
            "source_map": [
                {"spec_path": "intent", "decision_id": "u.title"},
                {"spec_path": "systems/combat", "decision_id": "ld.systems"},
                {"spec_path": "mechanics/damage", "decision_id": "ld.damage"},
                {"spec_path": "entities/enemy", "decision_id": "ld.enemies"}
            ]
        }"#;
        let spec: GameSpec = serde_json::from_str(raw).unwrap();
        assert!(spec.graphs.is_empty());
        assert!(validate_game_spec(&spec).is_empty());
    }

    /// Custom GWT 三段非空的 spec 级校验：缺段被拦、嵌套内的 Custom 也被拦、齐全放行。
    #[test]
    fn custom_gwt_incomplete_detected() {
        let mut spec = minimal_spec();
        spec.mechanics[0].effects.push(EffectSpec::Custom {
            verb: "merge".into(),
            operands: Default::default(),
            given: "g".into(),
            when_: "w".into(),
            then: String::new(),
        });
        assert!(
            validate_game_spec(&spec)
                .iter()
                .any(|v| v.code == "custom_gwt_incomplete")
        );

        // 嵌套在 Schedule 内的不完整 Custom 同样被拦。
        let mut nested = minimal_spec();
        nested.mechanics[0].effects.push(EffectSpec::Schedule {
            timing: Default::default(),
            amount_expr: "1".into(),
            unit: Default::default(),
            inner: vec![EffectSpec::Custom {
                verb: "merge".into(),
                operands: Default::default(),
                given: String::new(),
                when_: "w".into(),
                then: "t".into(),
            }],
        });
        assert!(
            validate_game_spec(&nested)
                .iter()
                .any(|v| v.code == "custom_gwt_incomplete")
        );

        // 三段齐全放行。
        let mut ok = minimal_spec();
        ok.mechanics[0].effects.push(EffectSpec::Custom {
            verb: "merge".into(),
            operands: Default::default(),
            given: "两个同级单位相邻".into(),
            when_: "拖拽合成".into(),
            then: "生成高一级单位".into(),
        });
        assert!(validate_game_spec(&ok).is_empty());
    }
}
