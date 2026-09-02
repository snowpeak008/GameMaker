use crate::framework::StageStatus;
use crate::runner::RunnerContext;
use adm4_ai::AiRequest;
use adm4_contracts::{
    CardinalityDeclaration, CardinalityRange, EvidencePointer, MeasuredMetric, SpecRef,
};
use adm4_foundation::{Adm4Error, Adm4Result};
use adm4_spec::{AcceptanceScenario, EffectSpec, GameSpec, MechanicSpec};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CapabilityContract {
    pub id: String,
    /// AI 命名（仅命名；结构由机制确定性投影）。
    pub interface_name: String,
    pub data_structures: Vec<String>,
    pub source_refs: Vec<SpecRef>,
    pub scenarios: Vec<AcceptanceScenario>,
}

/// C4 契约：机制投影派生的能力契约 + GWT + 双向核对。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CapabilitiesContract {
    pub capabilities: Vec<CapabilityContract>,
    /// 覆盖率 = 真实命中数 / 机制总数（R1：带逐机制证据，禁止恒 1.0 硬编码）。
    pub coverage: MeasuredMetric,
    pub cardinality: CardinalityDeclaration,
    pub blockers: Vec<String>,
}

pub fn execute(ctx: &RunnerContext<'_>) -> Adm4Result<StageStatus> {
    let spec: GameSpec = ctx.store.read_contract("C0")?;
    if spec.mechanics.is_empty() {
        return Err(Adm4Error::validation("GameSpec 无任何机制，C4 无从派生"));
    }

    // 确定性投影：每条机制 → 能力契约 + GWT 场景（枚举投影，无 AI 发明）。
    let mut capabilities = Vec::new();
    for mechanic in &spec.mechanics {
        let interface_name = name_interface(ctx, mechanic)?;
        let scenario = project_scenario(mechanic)?;
        let data_structures = collect_data_structures(&spec, mechanic);
        capabilities.push(CapabilityContract {
            id: format!("cap_{}", mechanic.id),
            interface_name,
            data_structures,
            source_refs: vec![SpecRef::new(format!("mechanics/{}", mechanic.id))],
            scenarios: vec![scenario],
        });
    }

    // 双向核对（真核对，非橡皮图章）。
    let mut blockers = Vec::new();
    // 正向：每条 source_ref 真实存在。
    for capability in &capabilities {
        for source_ref in &capability.source_refs {
            if !spec.contains_ref(source_ref) {
                blockers.push(format!(
                    "能力 {} 的 source_ref {} 不存在于 GameSpec",
                    capability.id, source_ref.0
                ));
            }
        }
        if capability.scenarios.is_empty() {
            blockers.push(format!("能力 {} 没有可判定验收场景", capability.id));
        }
    }
    // 反向：每条机制被 ≥1 能力命中。
    let mut evidence = Vec::new();
    let mut covered = 0usize;
    for mechanic in &spec.mechanics {
        let path = format!("mechanics/{}", mechanic.id);
        let hit = capabilities.iter().find(|capability| {
            capability
                .source_refs
                .iter()
                .any(|source_ref| source_ref.0 == path)
        });
        match hit {
            Some(capability) => {
                covered += 1;
                evidence.push(EvidencePointer {
                    file: "C4/contract.json".into(),
                    path: path.clone(),
                    observed: format!("被能力 {} 覆盖", capability.id),
                });
            }
            None => blockers.push(format!(
                "CORE_MECHANIC_NOT_PLANNED: {path} 未被任何能力覆盖"
            )),
        }
    }
    if !blockers.is_empty() {
        return Err(Adm4Error::validation(format!(
            "C4 双向核对 {} 项 blocker：{}",
            blockers.len(),
            blockers.join("; ")
        )));
    }
    let coverage = MeasuredMetric::new(covered as f64 / spec.mechanics.len() as f64, evidence)?;

    let cardinality = CardinalityDeclaration {
        rule: "每条 L4 机制确定性投影为 1 个能力契约与 ≥1 个 GWT 场景".into(),
        produced: capabilities.len(),
        expected: CardinalityRange {
            min: spec.mechanics.len(),
            max: spec.mechanics.len(),
        },
        dropped: Vec::new(),
    };

    let contract = CapabilitiesContract {
        capabilities: capabilities.clone(),
        coverage,
        cardinality,
        blockers: Vec::new(),
    };
    let mut document = format!(
        "# C4 程序需求与架构\n\n- 能力契约：{} 个\n- 机制覆盖率：{:.0}%（逐机制证据见 contract.json）\n\n",
        capabilities.len(),
        contract.coverage.value() * 100.0
    );
    for capability in &capabilities {
        let scenario = &capability.scenarios[0];
        document.push_str(&format!(
            "## {}（`{}`）\n\n- 来源：{}\n- 数据结构：{}\n- 验收场景：\n  - Given {}\n  - When {}\n  - Then {}\n\n",
            capability.interface_name,
            capability.id,
            capability
                .source_refs
                .iter()
                .map(|source_ref| format!("`{}`", source_ref.0))
                .collect::<Vec<_>>()
                .join(" "),
            capability.data_structures.join(", "),
            scenario.given.join("；"),
            scenario.when.join("；"),
            scenario.then.join("；")
        ));
    }
    document.push_str("> 本文档由 contract.json 渲染，请勿手改。\n");
    ctx.store.write_stage("C4", &contract, &document)?;
    Ok(StageStatus::Succeeded)
}

/// 确定性 GWT 投影：preconditions → Given；rule_text → When；effects（封闭枚举）→ Then。
///
/// W7 新增 9 变体（第 2 层 8 个 + Custom）的需求渲染属波 1（T-W7-1b/1c）；
/// 未交付前遇到即返回结构化 Err（§0 C4 未交付臂纪律：无 `_` 臂、禁 todo!()）。
fn project_scenario(mechanic: &MechanicSpec) -> Adm4Result<AcceptanceScenario> {
    let given = if mechanic.preconditions.is_empty() {
        vec![format!("系统 {} 处于就绪状态", mechanic.system_id)]
    } else {
        mechanic
            .preconditions
            .iter()
            .map(|condition| format!("{} {}", condition.subject, condition.predicate))
            .collect()
    };
    let undelivered = |variant: &str| {
        Err(Adm4Error::blocked(format!(
            "效果变体 {variant} 的需求渲染未交付（W7 波 1 实现）"
        )))
    };
    let then = mechanic
        .effects
        .iter()
        .map(|effect| match effect {
            EffectSpec::ModifyProperty {
                entity,
                property,
                formula,
            } => Ok(format!("实体 {entity} 的 {property} 按公式 {formula} 变化")),
            EffectSpec::SpawnEntity { entity } => Ok(format!("生成实体 {entity}")),
            EffectSpec::DespawnEntity { entity } => Ok(format!("移除实体 {entity}")),
            EffectSpec::ChangeState { machine, to_state } => {
                Ok(format!("状态机 {machine} 进入 {to_state}"))
            }
            EffectSpec::GrantResource { resource, formula } => {
                Ok(format!("资源 {resource} 按 {formula} 增加"))
            }
            EffectSpec::ConsumeResource { resource, formula } => {
                Ok(format!("资源 {resource} 按 {formula} 消耗"))
            }
            EffectSpec::EmitSignal { signal } => Ok(format!("发出信号 {signal}")),
            EffectSpec::Displace { .. } => undelivered("Displace"),
            EffectSpec::AreaApply { .. } => undelivered("AreaApply"),
            EffectSpec::Attach { .. } => undelivered("Attach"),
            EffectSpec::Detach { .. } => undelivered("Detach"),
            EffectSpec::Schedule { .. } => undelivered("Schedule"),
            EffectSpec::ModifyRule { .. } => undelivered("ModifyRule"),
            EffectSpec::DrawFromPool { .. } => undelivered("DrawFromPool"),
            EffectSpec::RollCheck { .. } => undelivered("RollCheck"),
            EffectSpec::Custom { .. } => undelivered("Custom"),
        })
        .collect::<Adm4Result<Vec<String>>>()?;
    Ok(AcceptanceScenario {
        id: format!("scenario_{}", mechanic.id),
        capability_id: format!("cap_{}", mechanic.id),
        given,
        when: vec![mechanic.rule_text.clone()],
        then,
        source_refs: vec![SpecRef::new(format!("mechanics/{}", mechanic.id))],
    })
}

fn collect_data_structures(spec: &GameSpec, mechanic: &MechanicSpec) -> Vec<String> {
    let mut structures = Vec::new();
    for effect in &mechanic.effects {
        let entity = match effect {
            EffectSpec::ModifyProperty { entity, .. }
            | EffectSpec::SpawnEntity { entity }
            | EffectSpec::DespawnEntity { entity } => Some(entity),
            EffectSpec::ChangeState { .. }
            | EffectSpec::GrantResource { .. }
            | EffectSpec::ConsumeResource { .. }
            | EffectSpec::EmitSignal { .. } => None,
            // W7 波 1 补真实收集（含 AreaApply/Schedule/RollCheck 嵌套 inner 的递归收集）
            EffectSpec::Displace { .. }
            | EffectSpec::AreaApply { .. }
            | EffectSpec::Attach { .. }
            | EffectSpec::Detach { .. }
            | EffectSpec::Schedule { .. }
            | EffectSpec::ModifyRule { .. }
            | EffectSpec::DrawFromPool { .. }
            | EffectSpec::RollCheck { .. }
            | EffectSpec::Custom { .. } => None,
        };
        if let Some(entity) = entity
            && let Some(found) = spec
                .entities
                .iter()
                .find(|candidate| &candidate.id == entity)
        {
            let name = format!("{}Data", camel(&found.name));
            if !structures.contains(&name) {
                structures.push(name);
            }
        }
    }
    structures
}

/// AI 只负责接口命名（锚定机制；不可用 = Err，R7）。
fn name_interface(ctx: &RunnerContext<'_>, mechanic: &MechanicSpec) -> Adm4Result<String> {
    let request = AiRequest {
        purpose: "c4_interface_naming".into(),
        system_prompt: "你是程序接口命名者。给机制起一个英文 PascalCase 接口名，\
                        输出 JSON：{\"interface_name\": ...}。"
            .into(),
        user_prompt: format!("机制 {}：{}", mechanic.id, mechanic.rule_text),
        expect_json: true,
    };
    let response = ctx.ai.invoke(&request)?;
    let value: serde_json::Value = serde_json::from_str(response.text.trim())
        .map_err(|error| Adm4Error::validation(format!("C4 命名产出不是合法 JSON：{error}")))?;
    let name = value
        .get("interface_name")
        .and_then(|name| name.as_str())
        .filter(|name| !name.trim().is_empty())
        .ok_or_else(|| Adm4Error::validation("C4 命名缺少 interface_name"))?;
    Ok(name.to_string())
}

fn camel(text: &str) -> String {
    text.split(|character: char| !character.is_alphanumeric())
        .filter(|part| !part.is_empty())
        .map(|part| {
            let mut characters = part.chars();
            match characters.next() {
                Some(first) => first.to_uppercase().collect::<String>() + characters.as_str(),
                None => String::new(),
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use adm4_foundation::Adm4ErrorKind;
    use std::collections::BTreeMap;

    fn mechanic_with(effects: Vec<EffectSpec>) -> MechanicSpec {
        MechanicSpec {
            id: "m1".into(),
            system_id: "s1".into(),
            rule_text: "规则".into(),
            preconditions: Vec::new(),
            effects,
            state_machine: None,
            design_notes: Vec::new(),
        }
    }

    /// 锁定：波 1 交付真渲染前，Displace 必须走结构化"未交付"Err，
    /// 不许悄悄糊假渲染（§0 C4 未交付臂纪律）。
    #[test]
    fn displace_projection_is_honest_undelivered_err() {
        let mechanic = mechanic_with(vec![EffectSpec::Displace {
            subject: "enemy".into(),
            direction_expr: "away".into(),
            distance_expr: "3".into(),
            duration_expr: "0.5".into(),
        }]);
        let error = project_scenario(&mechanic).expect_err("Displace 渲染未交付应 Err");
        assert_eq!(error.kind, Adm4ErrorKind::Blocked);
        assert!(
            error.message.contains("Displace") && error.message.contains("未交付"),
            "{}",
            error.message
        );
    }

    /// 锁定：ModifyRule 同上。
    #[test]
    fn modify_rule_projection_is_honest_undelivered_err() {
        let mechanic = mechanic_with(vec![EffectSpec::ModifyRule {
            target_rule: "damage_formula".into(),
            patch: Default::default(),
            priority: 0,
        }]);
        let error = project_scenario(&mechanic).expect_err("ModifyRule 渲染未交付应 Err");
        assert_eq!(error.kind, Adm4ErrorKind::Blocked);
        assert!(
            error.message.contains("ModifyRule") && error.message.contains("未交付"),
            "{}",
            error.message
        );
    }

    /// 锁定：Custom 同上——即使带了完整 GWT 模板，转录投影也属波 1，不许提前糊。
    #[test]
    fn custom_projection_is_honest_undelivered_err() {
        let mechanic = mechanic_with(vec![EffectSpec::Custom {
            verb: "merge".into(),
            operands: BTreeMap::new(),
            given: "g".into(),
            when_: "w".into(),
            then: "t".into(),
        }]);
        let error = project_scenario(&mechanic).expect_err("Custom 渲染未交付应 Err");
        assert_eq!(error.kind, Adm4ErrorKind::Blocked);
        assert!(
            error.message.contains("Custom") && error.message.contains("未交付"),
            "{}",
            error.message
        );
    }

    /// 混入一个新变体就整体 Err：旧 7 变体不因同机制的未交付变体被静默丢弃。
    #[test]
    fn mixed_effects_fail_whole_projection() {
        let mechanic = mechanic_with(vec![
            EffectSpec::SpawnEntity {
                entity: "guard".into(),
            },
            EffectSpec::Schedule {
                timing: Default::default(),
                amount_expr: "2".into(),
                unit: Default::default(),
                inner: Vec::new(),
            },
        ]);
        let error = project_scenario(&mechanic).expect_err("含 Schedule 的机制应整体 Err");
        assert!(error.message.contains("Schedule"), "{}", error.message);
    }

    /// 旧 7 变体投影不受本卡影响（基线守恒）。
    #[test]
    fn legacy_seven_variants_still_project() {
        let mechanic = mechanic_with(vec![
            EffectSpec::ModifyProperty {
                entity: "enemy".into(),
                property: "hp".into(),
                formula: "hp - damage".into(),
            },
            EffectSpec::EmitSignal {
                signal: "hit".into(),
            },
        ]);
        let scenario = project_scenario(&mechanic).expect("旧变体应正常投影");
        assert_eq!(scenario.then.len(), 2);
        assert!(scenario.then[0].contains("hp"));
        assert!(scenario.then[1].contains("hit"));
    }
}
