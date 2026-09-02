//! Graph / Curve 参数的 GameSpec 侧数据类型（W7 定稿 §5.4）。
//!
//! schema 侧（`ParameterSchema` 的 Graph/Curve tag 分支）位于 adm4-decision，
//! 属并行卡协调范围，本模块只承载编译产物形态。

use crate::model::DesignNote;
use adm4_contracts::TypedValue;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// 图的入口约束：Single 要求入度 0 的节点恰好 1 个（如天赋树根）；
/// Multiple 不限（如对话 hub 图）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum GraphEntry {
    Single,
    /// 对话/图类模块默认多入口（W7 定稿指令 10：对话类默认 acyclic=false）。
    #[default]
    Multiple,
}

/// 图节点：负载为键值对；is_skin_fields 列出负载中属于「皮」
/// （命名/主题/文案）的键，供换皮门比对粒度使用。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GraphNode {
    pub id: String,
    #[serde(default)]
    pub payload: BTreeMap<String, TypedValue>,
    #[serde(default)]
    pub is_skin_fields: Vec<String>,
}

/// 图边：端点必须是本图已声明节点（validate 校验）。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GraphEdge {
    pub from: String,
    pub to: String,
    #[serde(default)]
    pub payload: BTreeMap<String, TypedValue>,
}

/// L5/L6：图结构参数（天赋树、肉鸽地图、对话图等）。
///
/// - `acyclic=true` 时 validate 做拓扑/成环检查；对话类默认 false
///   （极乐迪斯科对话大量回环 hub，声明 true 属天赋树/肉鸽地图类）。
/// - `entry=Single` 时 validate 要求入度 0 节点恰 1（对无向图退化为
///   「未被任何边指向的节点恰 1」，按 from→to 计入度）。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GraphSpec {
    pub id: String,
    #[serde(default)]
    pub directed: bool,
    #[serde(default)]
    pub acyclic: bool,
    #[serde(default)]
    pub entry: GraphEntry,
    #[serde(default)]
    pub nodes: Vec<GraphNode>,
    #[serde(default)]
    pub edges: Vec<GraphEdge>,
    #[serde(default)]
    pub design_notes: Vec<DesignNote>,
}

/// 曲线插值方式（封闭枚举）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum CurveInterpolation {
    #[default]
    Linear,
    Step,
    Cubic,
}

/// 曲线参数：单值 y=f(x) 的采样点序列（按 x 升序）。
///
/// **已知宽度缺陷（官方口径，W7 定稿 §5.4 如实保留）**：本类型是单值
/// y=f(x)，二维偏移序列（如 CS2 后坐力的每发 (dx,dy)）装不下，需退化为
/// 三列 Table（shot_index/dx/dy）——表达位仍有，语义降级如实声明；
/// 「音游谱面 = Curve」同理需逐案核。Curve 不进 GameSpec 新 section，
/// 由 C0 编译成两列 TableSpec + 插值注记，复用既有表通路（波 1）。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CurveSpec {
    pub id: String,
    #[serde(default)]
    pub interpolation: CurveInterpolation,
    /// (x, y) 采样点；元组序列化为 `[x, y]` 双元素数组。
    #[serde(default)]
    pub points: Vec<(f64, f64)>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn graph_spec_roundtrip_and_missing_keys() {
        let graph = GraphSpec {
            id: "talent_tree".into(),
            directed: true,
            acyclic: true,
            entry: GraphEntry::Single,
            nodes: vec![GraphNode {
                id: "root".into(),
                payload: BTreeMap::from([("cost".to_string(), TypedValue::Int(1))]),
                is_skin_fields: vec!["label".into()],
            }],
            edges: Vec::new(),
            design_notes: Vec::new(),
        };
        let json = serde_json::to_string(&graph).unwrap();
        let parsed: GraphSpec = serde_json::from_str(&json).unwrap();
        assert_eq!(graph, parsed);

        // 缺键可读：只有 id 的最小 JSON（对话类默认 acyclic=false、entry=multiple）。
        let minimal: GraphSpec = serde_json::from_str(r#"{"id":"dialogue"}"#).unwrap();
        assert!(!minimal.directed && !minimal.acyclic);
        assert_eq!(minimal.entry, GraphEntry::Multiple);
        assert!(minimal.nodes.is_empty() && minimal.edges.is_empty());
    }

    #[test]
    fn curve_spec_roundtrip() {
        let curve = CurveSpec {
            id: "xp_curve".into(),
            interpolation: CurveInterpolation::Cubic,
            points: vec![(0.0, 0.0), (1.0, 100.0), (2.0, 350.0)],
        };
        let json = serde_json::to_string(&curve).unwrap();
        let parsed: CurveSpec = serde_json::from_str(&json).unwrap();
        assert_eq!(curve, parsed);
        assert!(json.contains(r#""cubic""#));

        let minimal: CurveSpec = serde_json::from_str(r#"{"id":"c"}"#).unwrap();
        assert_eq!(minimal.interpolation, CurveInterpolation::Linear);
        assert!(minimal.points.is_empty());
    }
}
