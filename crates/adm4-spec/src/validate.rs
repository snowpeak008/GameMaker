use crate::model::{GameSpec, SPEC_SCHEMA_VERSION};
use adm4_foundation::{Adm4Error, Adm4Result, ContentHash};
use std::collections::BTreeSet;

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
            if let crate::model::EffectSpec::ModifyProperty { entity, .. }
            | crate::model::EffectSpec::SpawnEntity { entity }
            | crate::model::EffectSpec::DespawnEntity { entity } = effect
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

/// 规范化内容哈希。
pub fn spec_content_hash(spec: &GameSpec) -> Adm4Result<String> {
    let value = serde_json::to_value(spec)
        .map_err(|error| Adm4Error::internal(format!("spec serialize failed: {error}")))?;
    Ok(ContentHash::of_canonical_json(&value)?.0)
}

#[cfg(test)]
mod tests {
    use super::*;
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
            }],
            entities: vec![EntitySpec {
                id: "enemy".into(),
                name: "敌人".into(),
                visual_form: Some(VisualForm::Sprite2d),
                properties: Vec::new(),
            }],
            tables: Vec::new(),
            content: Vec::new(),
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
}
