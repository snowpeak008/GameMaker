pub use crate::effects::EffectSpec;
pub use crate::graph::GraphSpec;
use adm4_contracts::{MatrixCell, SpecRef, TypedValue, ValueConstraint, ValueKind};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

pub const SPEC_SCHEMA_VERSION: &str = "4.0.0";

/// 设计注记的角色（W7 定稿 §5.5）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum DesignNoteRole {
    /// 选择理由（选项 rationale）。
    #[default]
    Rationale,
    /// 设计者自由陈述。
    Statement,
}

/// 设计注记：rationale 进编译链（W7 定稿 §5.5）。
///
/// **纪律：注记只被携带与展示，永不被编译成结构**——GWT 仍只从结构化
/// 字段派生，保 I1 确定性守恒。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct DesignNote {
    #[serde(default)]
    pub source_decision: String,
    #[serde(default)]
    pub source_option: String,
    #[serde(default)]
    pub role: DesignNoteRole,
    #[serde(default)]
    pub text: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SpecIdentity {
    pub schema_version: String,
    pub project_id: String,
    /// 绑定冻结集内容哈希（可追溯）。
    pub frozen_hash: String,
}

/// L0-L2：项目意图。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct ProjectIntent {
    pub title: String,
    #[serde(default)]
    pub experience_promise: String,
    #[serde(default)]
    pub genre_structure: String,
    #[serde(default)]
    pub profile: BTreeMap<String, String>,
}

/// L3：系统组成。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SystemSpec {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub purpose: String,
    #[serde(default)]
    pub interfaces: Vec<String>,
    #[serde(default)]
    pub design_notes: Vec<DesignNote>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ConditionSpec {
    pub subject: String,
    pub predicate: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StateTransition {
    pub from: String,
    pub to: String,
    pub trigger: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StateMachineSpec {
    pub id: String,
    pub states: Vec<String>,
    pub initial: String,
    pub transitions: Vec<StateTransition>,
}

/// L4：机制规则（公式符号级）。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MechanicSpec {
    pub id: String,
    pub system_id: String,
    pub rule_text: String,
    #[serde(default)]
    pub preconditions: Vec<ConditionSpec>,
    pub effects: Vec<EffectSpec>,
    #[serde(default)]
    pub state_machine: Option<StateMachineSpec>,
    #[serde(default)]
    pub design_notes: Vec<DesignNote>,
}

/// 实体的视觉形态声明（C3 视觉白名单依据：未声明 = 不产美术资产）。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VisualForm {
    Sprite2d,
    Model3d,
    UiOnly,
    Invisible,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PropertySpec {
    pub key: String,
    pub kind: ValueKind,
    #[serde(default)]
    pub constraint: Option<ValueConstraint>,
}

/// L5：实体类型。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EntitySpec {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub visual_form: Option<VisualForm>,
    #[serde(default)]
    pub properties: Vec<PropertySpec>,
}

/// L5/L6：属性表或矩阵（列结构 + 行数据）。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TableSpec {
    pub id: String,
    pub columns: Vec<PropertySpec>,
    pub row_key: String,
    #[serde(default)]
    pub rows: Vec<BTreeMap<String, TypedValue>>,
    /// 矩阵型表的格数据（行/列/值）。
    #[serde(default)]
    pub cells: Vec<MatrixCell>,
    #[serde(default)]
    pub design_notes: Vec<DesignNote>,
}

/// L6：关卡/波次等内容数据。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ContentSpec {
    pub id: String,
    pub content_kind: String,
    pub data: serde_json::Value,
    #[serde(default)]
    pub design_notes: Vec<DesignNote>,
}

/// GWT 验收场景（C4 派生填充；Phase 2 P4 真机执行）。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AcceptanceScenario {
    pub id: String,
    pub capability_id: String,
    pub given: Vec<String>,
    pub when: Vec<String>,
    pub then: Vec<String>,
    #[serde(default)]
    pub source_refs: Vec<SpecRef>,
}

/// spec 元素 ← 决策路径 的追溯条目。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SpecSourceEntry {
    pub spec_path: SpecRef,
    pub decision_id: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GameSpec {
    pub identity: SpecIdentity,
    pub intent: ProjectIntent,
    pub systems: Vec<SystemSpec>,
    pub mechanics: Vec<MechanicSpec>,
    pub entities: Vec<EntitySpec>,
    pub tables: Vec<TableSpec>,
    pub content: Vec<ContentSpec>,
    /// L5/L6：图结构参数（W7 新增，旧档缺键可读）。
    #[serde(default)]
    pub graphs: Vec<GraphSpec>,
    #[serde(default)]
    pub acceptance: Vec<AcceptanceScenario>,
    pub source_map: Vec<SpecSourceEntry>,
}

impl GameSpec {
    /// 判断锚定路径是否存在于本 spec（R4 锚定校验）。
    pub fn contains_ref(&self, spec_ref: &SpecRef) -> bool {
        let path = spec_ref.0.as_str();
        let Some((section, id)) = path.split_once('/') else {
            return matches!(path, "intent" | "identity");
        };
        match section {
            "intent" => true,
            "systems" => self.systems.iter().any(|item| item.id == id),
            "mechanics" => self.mechanics.iter().any(|item| item.id == id),
            "entities" => self.entities.iter().any(|item| item.id == id),
            "tables" => self.tables.iter().any(|item| item.id == id),
            "content" => self.content.iter().any(|item| item.id == id),
            "graphs" => self.graphs.iter().any(|item| item.id == id),
            "acceptance" => self.acceptance.iter().any(|item| item.id == id),
            _ => false,
        }
    }

    pub fn all_ref_paths(&self) -> Vec<SpecRef> {
        let mut paths = vec![SpecRef::new("intent")];
        paths.extend(
            self.systems
                .iter()
                .map(|item| SpecRef::new(format!("systems/{}", item.id))),
        );
        paths.extend(
            self.mechanics
                .iter()
                .map(|item| SpecRef::new(format!("mechanics/{}", item.id))),
        );
        paths.extend(
            self.entities
                .iter()
                .map(|item| SpecRef::new(format!("entities/{}", item.id))),
        );
        paths.extend(
            self.tables
                .iter()
                .map(|item| SpecRef::new(format!("tables/{}", item.id))),
        );
        paths.extend(
            self.content
                .iter()
                .map(|item| SpecRef::new(format!("content/{}", item.id))),
        );
        paths.extend(
            self.graphs
                .iter()
                .map(|item| SpecRef::new(format!("graphs/{}", item.id))),
        );
        paths
    }
}
