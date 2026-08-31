//! 程序线契约（册 07 §1）：`GameSpec` → 系统 / 能力契约 / 实体 / 事件 / 权属 / 验收 / 资产依赖。
//!
//! 程序线**只映射不发明**（铁律①）：每条事实都要么锚定一条 `SpecRef`，要么登记成 gap。
//! 本模块只定义契约形态与确定性校验，派生器（从 GameSpec 真正算出这份契约）属后续波次。

use super::{ContractEnvelope, PROGRAM_LINE, SpecTriple, require_non_blank};
use adm4_contracts::SpecRef;
use adm4_foundation::{Adm4Error, Adm4Result};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

/// 跨系统通信方式（py 协议 §4.2 的封闭集合）。
///
/// `Unknown` 是**旧档/漏填**的落点，不是一种合法方式：[`ProgramContract::validate`] 见到它就报错。
/// 有这么一个显式变体，才能把「没写」和「写了 query」区分开（R2：未知不许伪装成某个具体值）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ContractMethod {
    #[default]
    Unknown,
    Query,
    Command,
    Event,
}

/// 程序系统：实现面的系统划分，必须记得住自己从哪个设计系统来。
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct ProgramSystem {
    pub system_id: String,
    pub name: String,
    pub responsibility: String,
    /// 源设计系统（`SpecRef`，形如 `systems/<id>`）。
    pub source_refs: Vec<SpecRef>,
}

/// 能力契约：跨系统通信的唯一注册处（后续阶段只能绑定，不能另发明一条）。
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct CapabilityContract {
    pub capability_id: String,
    pub source_system: String,
    pub target_system: String,
    pub method: ContractMethod,
    pub inputs: Vec<String>,
    pub outputs: Vec<String>,
    pub errors: Vec<String>,
    pub source_refs: Vec<SpecRef>,
}

/// 程序实体：实现面的数据形状（本身不构成权属）。
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct ProgramEntity {
    pub entity_id: String,
    pub entity_name: String,
    pub owner_system: String,
    pub properties: Vec<String>,
    pub source_refs: Vec<SpecRef>,
}

/// 事件：必须绑定一条已注册的能力契约。
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct ProgramEvent {
    pub event_id: String,
    pub capability_id: String,
    pub payload: Vec<String>,
    pub source_refs: Vec<SpecRef>,
}

/// 权属：一条可变事实只能有一个写入者系统。
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct AuthorityAssignment {
    pub authority_id: String,
    /// 被托管的可变事实（实体字段/资源等）。
    pub mutable_fact: String,
    pub owner_system: String,
    pub source_refs: Vec<SpecRef>,
}

/// 验收绑定：把 `GameSpec.acceptance` 的 GWT 场景钉到能力契约上。
///
/// 只做绑定不复制场景正文——GWT 的真源在 `GameSpec` 里，抄一份过来就是第二真源（D22）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct AcceptanceBinding {
    pub acceptance_id: String,
    pub capability_id: String,
    /// 指向 `GameSpec.acceptance` 的锚（形如 `acceptance/<id>`）。
    pub scenario_ref: SpecRef,
}

/// `SpecRef` 没有（也不该有）`Default`——一个空锚点不是合法锚点。
/// 这里手写默认值只为满足旧档兼容所需的 `#[serde(default)]`：读出来是空串，
/// 随即被 [`ProgramContract::validate`] 判为「验收绑定缺锚点」而拒收。
impl Default for AcceptanceBinding {
    fn default() -> Self {
        Self {
            acceptance_id: String::new(),
            capability_id: String::new(),
            scenario_ref: SpecRef::new(String::new()),
        }
    }
}

/// 程序线对美术资产的依赖：对齐层的**程序侧输入**。
///
/// `asset_id` 是美术线的稳定标识（铁律②单点锚定）：程序线只能引用它，不能自己起名。
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct ProgramAssetDependency {
    /// 程序侧引用点（如 `hero_controller.idle_anim`），进对齐报告的 `program_ref`。
    pub dependency_id: String,
    pub owner_system: String,
    pub asset_id: String,
    /// 程序侧要求的三要素（帧/尺寸/格式）。
    pub required_spec: SpecTriple,
    pub source_refs: Vec<SpecRef>,
}

/// 程序线机器契约（`program_contract.json`）。
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct ProgramContract {
    pub envelope: ContractEnvelope,
    pub systems: Vec<ProgramSystem>,
    pub capabilities: Vec<CapabilityContract>,
    pub entities: Vec<ProgramEntity>,
    pub events: Vec<ProgramEvent>,
    pub authority: Vec<AuthorityAssignment>,
    pub acceptance: Vec<AcceptanceBinding>,
    pub asset_dependencies: Vec<ProgramAssetDependency>,
}

impl ProgramContract {
    /// 确定性自校验（不经 AI）：id 齐备且唯一、引用闭合、方式已声明。
    ///
    /// 对照 py 协议 §8 的硬阻塞项：无系统的程序契约、无源/目标系统的能力契约、
    /// 悬空的事件/权属绑定——这些在这里全部变成 `Err`，而不是一条警告。
    pub fn validate(&self) -> Adm4Result<()> {
        self.envelope.validate(PROGRAM_LINE)?;
        if self.systems.is_empty() {
            return Err(Adm4Error::validation(
                "程序线契约没有任何系统：无系统的程序契约不可用于下游派生",
            ));
        }
        let mut system_ids = BTreeSet::new();
        for system in &self.systems {
            require_non_blank(&system.system_id, "程序系统 system_id")?;
            if !system_ids.insert(system.system_id.as_str()) {
                return Err(Adm4Error::validation(format!(
                    "程序系统 id 重复：{}（系统 id 是稳定标识，不得撞名）",
                    system.system_id
                )));
            }
        }
        let known_system = |id: &str, what: &str| -> Adm4Result<()> {
            if system_ids.contains(id) {
                return Ok(());
            }
            Err(Adm4Error::validation(format!(
                "{what} 引用了未注册的系统 {id}"
            )))
        };

        let mut capability_ids = BTreeSet::new();
        for capability in &self.capabilities {
            require_non_blank(&capability.capability_id, "能力契约 capability_id")?;
            if !capability_ids.insert(capability.capability_id.as_str()) {
                return Err(Adm4Error::validation(format!(
                    "能力契约 id 重复：{}",
                    capability.capability_id
                )));
            }
            if capability.method == ContractMethod::Unknown {
                return Err(Adm4Error::validation(format!(
                    "能力契约 {} 未声明通信方式（query/command/event）",
                    capability.capability_id
                )));
            }
            known_system(
                &capability.source_system,
                &format!("能力契约 {} 的源系统", capability.capability_id),
            )?;
            known_system(
                &capability.target_system,
                &format!("能力契约 {} 的目标系统", capability.capability_id),
            )?;
        }

        for entity in &self.entities {
            require_non_blank(&entity.entity_id, "程序实体 entity_id")?;
            known_system(
                &entity.owner_system,
                &format!("程序实体 {} 的归属系统", entity.entity_id),
            )?;
        }
        for event in &self.events {
            require_non_blank(&event.event_id, "程序事件 event_id")?;
            if !capability_ids.contains(event.capability_id.as_str()) {
                return Err(Adm4Error::validation(format!(
                    "程序事件 {} 绑定了未注册的能力契约 {}",
                    event.event_id, event.capability_id
                )));
            }
        }
        let mut owned_facts = BTreeSet::new();
        for authority in &self.authority {
            require_non_blank(&authority.authority_id, "权属条目 authority_id")?;
            known_system(
                &authority.owner_system,
                &format!("权属条目 {} 的归属系统", authority.authority_id),
            )?;
            if !owned_facts.insert(authority.mutable_fact.as_str()) {
                return Err(Adm4Error::validation(format!(
                    "可变事实 {} 被登记了多个写入者：权属必须单点归属",
                    authority.mutable_fact
                )));
            }
        }
        for binding in &self.acceptance {
            require_non_blank(&binding.acceptance_id, "验收绑定 acceptance_id")?;
            require_non_blank(
                &binding.scenario_ref.0,
                &format!("验收绑定 {} 的真源锚点", binding.acceptance_id),
            )?;
            if !capability_ids.contains(binding.capability_id.as_str()) {
                return Err(Adm4Error::validation(format!(
                    "验收绑定 {} 指向未注册的能力契约 {}",
                    binding.acceptance_id, binding.capability_id
                )));
            }
        }
        let mut dependency_ids = BTreeSet::new();
        for dependency in &self.asset_dependencies {
            require_non_blank(&dependency.dependency_id, "资产依赖 dependency_id")?;
            require_non_blank(
                &dependency.asset_id,
                &format!("资产依赖 {} 的 asset_id", dependency.dependency_id),
            )?;
            if !dependency_ids.insert(dependency.dependency_id.as_str()) {
                return Err(Adm4Error::validation(format!(
                    "资产依赖 id 重复：{}",
                    dependency.dependency_id
                )));
            }
            known_system(
                &dependency.owner_system,
                &format!("资产依赖 {} 的归属系统", dependency.dependency_id),
            )?;
        }
        Ok(())
    }

    /// 本契约声明的全部标识（权威顺序校验器据此判断 Markdown 里的说法有没有契约背书）。
    pub fn declared_ids(&self) -> BTreeSet<String> {
        let mut ids = BTreeSet::new();
        ids.extend(self.systems.iter().map(|item| item.system_id.clone()));
        ids.extend(
            self.capabilities
                .iter()
                .map(|item| item.capability_id.clone()),
        );
        ids.extend(self.entities.iter().map(|item| item.entity_id.clone()));
        ids.extend(self.events.iter().map(|item| item.event_id.clone()));
        ids.extend(self.authority.iter().map(|item| item.authority_id.clone()));
        ids.extend(
            self.acceptance
                .iter()
                .map(|item| item.acceptance_id.clone()),
        );
        ids.extend(
            self.asset_dependencies
                .iter()
                .map(|item| item.dependency_id.clone()),
        );
        ids
    }

    /// 本契约用到的全部真源锚点（用于核对「下游是不是发明了真源里没有的事实」）。
    pub fn source_refs(&self) -> Vec<SpecRef> {
        let mut refs: Vec<SpecRef> = Vec::new();
        for system in &self.systems {
            refs.extend(system.source_refs.iter().cloned());
        }
        for capability in &self.capabilities {
            refs.extend(capability.source_refs.iter().cloned());
        }
        for entity in &self.entities {
            refs.extend(entity.source_refs.iter().cloned());
        }
        for event in &self.events {
            refs.extend(event.source_refs.iter().cloned());
        }
        for authority in &self.authority {
            refs.extend(authority.source_refs.iter().cloned());
        }
        for binding in &self.acceptance {
            refs.push(binding.scenario_ref.clone());
        }
        for dependency in &self.asset_dependencies {
            refs.extend(dependency.source_refs.iter().cloned());
        }
        refs
    }

    /// 本契约依赖的全部美术 `asset_id`（对齐层的程序侧口径）。
    pub fn required_asset_ids(&self) -> BTreeSet<String> {
        self.asset_dependencies
            .iter()
            .map(|dependency| dependency.asset_id.clone())
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::governance::AssetSize;

    fn sample() -> ProgramContract {
        ProgramContract {
            envelope: ContractEnvelope::new(PROGRAM_LINE, "2026-08-31T00:00:00Z", "sha256:frozen"),
            systems: vec![ProgramSystem {
                system_id: "combat_system".into(),
                name: "战斗系统".into(),
                responsibility: "结算克制伤害".into(),
                source_refs: vec![SpecRef::new("systems/combat")],
            }],
            capabilities: vec![CapabilityContract {
                capability_id: "cap_apply_damage".into(),
                source_system: "combat_system".into(),
                target_system: "combat_system".into(),
                method: ContractMethod::Command,
                inputs: vec!["attacker".into()],
                outputs: vec!["damage".into()],
                errors: vec!["target_missing".into()],
                source_refs: vec![SpecRef::new("mechanics/counter_damage")],
            }],
            entities: vec![ProgramEntity {
                entity_id: "guard".into(),
                entity_name: "守卫".into(),
                owner_system: "combat_system".into(),
                properties: vec!["hp".into()],
                source_refs: vec![SpecRef::new("entities/guard")],
            }],
            events: vec![ProgramEvent {
                event_id: "damage_applied".into(),
                capability_id: "cap_apply_damage".into(),
                payload: vec!["damage".into()],
                source_refs: vec![SpecRef::new("mechanics/counter_damage")],
            }],
            authority: vec![AuthorityAssignment {
                authority_id: "auth_guard_hp".into(),
                mutable_fact: "guard.hp".into(),
                owner_system: "combat_system".into(),
                source_refs: vec![SpecRef::new("entities/guard")],
            }],
            acceptance: vec![AcceptanceBinding {
                acceptance_id: "acc_counter".into(),
                capability_id: "cap_apply_damage".into(),
                scenario_ref: SpecRef::new("acceptance/counter"),
            }],
            asset_dependencies: vec![ProgramAssetDependency {
                dependency_id: "hero_controller.idle_anim".into(),
                owner_system: "combat_system".into(),
                asset_id: "UI_PlayerIdle".into(),
                required_spec: SpecTriple::full(8, AssetSize::new(256, 256), "png"),
                source_refs: vec![SpecRef::new("entities/guard")],
            }],
        }
    }

    #[test]
    fn program_contract_round_trips_through_json() {
        let contract = sample();
        let json = serde_json::to_string_pretty(&contract).expect("序列化");
        let back: ProgramContract = serde_json::from_str(&json).expect("反序列化");
        assert_eq!(back, contract);
        assert!(back.validate().is_ok());
    }

    /// 旧档兼容：只有信封与 systems 的历史契约必须能读出来，缺的段落是空表而不是解析失败。
    #[test]
    fn legacy_program_contract_without_new_sections_parses() {
        let legacy = r#"{
          "envelope": {
            "schema_version": "4.0.0",
            "consumer_line": "program",
            "source_frozen_hash": "sha256:old"
          },
          "systems": [{"system_id": "combat_system", "name": "战斗系统"}]
        }"#;
        let parsed: ProgramContract = serde_json::from_str(legacy).expect("旧档应可解析");
        assert_eq!(parsed.systems.len(), 1);
        assert!(parsed.systems[0].source_refs.is_empty());
        assert!(parsed.capabilities.is_empty());
        assert!(parsed.asset_dependencies.is_empty());
        assert!(parsed.validate().is_ok(), "旧档结构本身合法，只是段落为空");
    }

    #[test]
    fn validate_rejects_dangling_and_duplicate_references() {
        let mut contract = sample();
        contract.capabilities[0].target_system = "nowhere".into();
        assert!(
            contract.validate().unwrap_err().message.contains("nowhere"),
            "能力契约指向未注册系统必须被拒"
        );

        let mut contract = sample();
        contract.capabilities[0].method = ContractMethod::Unknown;
        assert!(
            contract
                .validate()
                .unwrap_err()
                .message
                .contains("未声明通信方式")
        );

        let mut contract = sample();
        contract.events[0].capability_id = "cap_missing".into();
        assert!(contract.validate().is_err(), "事件绑定悬空契约必须被拒");

        let mut contract = sample();
        let duplicate = contract.authority[0].clone();
        contract.authority.push(AuthorityAssignment {
            authority_id: "auth_other".into(),
            ..duplicate
        });
        assert!(
            contract
                .validate()
                .unwrap_err()
                .message
                .contains("单点归属"),
            "同一条可变事实登记两个写入者必须被拒"
        );

        let empty = ProgramContract {
            envelope: ContractEnvelope::new(PROGRAM_LINE, "now", "sha256:x"),
            ..ProgramContract::default()
        };
        assert!(empty.validate().is_err(), "无系统的程序契约不可放行");
    }

    #[test]
    fn declared_ids_and_source_refs_cover_every_section() {
        let contract = sample();
        let ids = contract.declared_ids();
        for wanted in [
            "combat_system",
            "cap_apply_damage",
            "guard",
            "damage_applied",
            "auth_guard_hp",
            "acc_counter",
            "hero_controller.idle_anim",
        ] {
            assert!(ids.contains(wanted), "declared_ids 应含 {wanted}");
        }
        let refs = contract.source_refs();
        assert!(refs.contains(&SpecRef::new("acceptance/counter")));
        assert_eq!(
            contract
                .required_asset_ids()
                .into_iter()
                .collect::<Vec<_>>(),
            vec!["UI_PlayerIdle".to_string()]
        );
    }
}
