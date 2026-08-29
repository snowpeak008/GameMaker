use adm4_contracts::{MatrixCell, SpecRef, TypedValue, ValueConstraint, ValueKind};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

pub const SPEC_SCHEMA_VERSION: &str = "4.0.0";

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
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ConditionSpec {
    pub subject: String,
    pub predicate: String,
}

/// 机制效果的封闭枚举——C4 确定性投影的前提。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "effect", rename_all = "snake_case")]
pub enum EffectSpec {
    ModifyProperty {
        entity: String,
        property: String,
        formula: String,
    },
    SpawnEntity {
        entity: String,
    },
    DespawnEntity {
        entity: String,
    },
    ChangeState {
        machine: String,
        to_state: String,
    },
    GrantResource {
        resource: String,
        formula: String,
    },
    ConsumeResource {
        resource: String,
        formula: String,
    },
    EmitSignal {
        signal: String,
    },
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
}

/// L6：关卡/波次等内容数据。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ContentSpec {
    pub id: String,
    pub content_kind: String,
    pub data: serde_json::Value,
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
        paths
    }
}
