//! EffectSpec：机制效果的封闭枚举，三层确定性等级（W7 定稿 §5.3）。
//!
//! - 第 1 层 封闭核心（7，**旧 tag 与字段一个不动**，I2 serde 旧档守恒）：全语义投影。
//! - 第 2 层 受控扩展（8，W7 新增）：模式投影，每变体一个固定渲染函数。
//! - 第 3 层 逃生舱口（1，Custom）：转录投影，只誊写设计者自己写的 GWT 验收模板。

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// AreaApply 的空间查询形状（封闭枚举）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum AreaKind {
    #[default]
    Circle,
    Cone,
    Line,
    Grid,
}

/// Schedule 的时序模式（封闭枚举）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum ScheduleTiming {
    /// 延迟一次性触发。
    #[default]
    Delayed,
    /// 持续期内逐步生效。
    OverTime,
    /// 周期性重复触发。
    Periodic,
}

/// Schedule 的时间单位（封闭枚举）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum ScheduleUnit {
    #[default]
    Seconds,
    Turns,
    Ticks,
}

/// ModifyRule 的封闭补丁操作集——规则修改器只引用已存在规则 + 封闭操作集，
/// 是转录不是发明（W7 定稿 §5.3）。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(tag = "patch", rename_all = "snake_case")]
pub enum RulePatch {
    /// 按表达式缩放目标规则的系数。
    ScaleCoefficient {
        #[serde(default)]
        expr: String,
    },
    /// 整体替换目标规则的公式。
    ReplaceFormula {
        #[serde(default)]
        formula: String,
    },
    /// 禁用目标规则。
    #[default]
    Disable,
    /// 启用目标规则。
    Enable,
    /// 为目标规则追加前置条件。
    AddPrecondition {
        #[serde(default)]
        condition: String,
    },
}

/// 机制效果的封闭枚举——C4 确定性投影的前提。
///
/// 序列化保持单层 tag（`effect` 键，snake_case）。第 1 层旧 7 个变体的
/// tag 与字段**逐字节不动**（I2）；第 2/3 层新变体全部字段 `serde(default)`，
/// 保证缺键旧档可读。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "effect", rename_all = "snake_case")]
pub enum EffectSpec {
    // ===== 第 1 层 封闭核心（旧 7 变体，一个不动）=====
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

    // ===== 第 2 层 受控扩展（W7 新增 8 变体，模式投影）=====
    /// 位移：把 subject 沿方向表达式推动一段距离/时长。
    Displace {
        #[serde(default)]
        subject: String,
        #[serde(default)]
        direction_expr: String,
        #[serde(default)]
        distance_expr: String,
        #[serde(default)]
        duration_expr: String,
    },
    /// 空间查询 + 对命中目标施加内层效果（可嵌套）。
    AreaApply {
        #[serde(default)]
        area_kind: AreaKind,
        #[serde(default)]
        area_params: BTreeMap<String, String>,
        #[serde(default)]
        inner: Vec<EffectSpec>,
        #[serde(default)]
        target_filter: String,
    },
    /// 挂载修饰器。同目标多修饰器按 priority 结算（大者后算），
    /// 同序冲突用确定性 tie-break（按机制 id 字典序）——W7 定稿 §5.3 叠加序。
    Attach {
        #[serde(default)]
        modifier_id: String,
        #[serde(default)]
        target: String,
        #[serde(default)]
        duration_expr: String,
        #[serde(default)]
        priority: i32,
    },
    /// 卸载修饰器。
    Detach {
        #[serde(default)]
        modifier_id: String,
        #[serde(default)]
        target: String,
    },
    /// 时序包装：Delayed/OverTime/Periodic × Seconds/Turns/Ticks，内层效果可嵌套。
    Schedule {
        #[serde(default)]
        timing: ScheduleTiming,
        #[serde(default)]
        amount_expr: String,
        #[serde(default)]
        unit: ScheduleUnit,
        #[serde(default)]
        inner: Vec<EffectSpec>,
    },
    /// 规则修改器：target_rule 必须是 spec 内真实 mechanic id 或系统声明的
    /// RuleSlot 名词（悬空校验属波 1 C1）；patch 为封闭 RulePatch；
    /// priority 语义同 Attach。
    ModifyRule {
        #[serde(default)]
        target_rule: String,
        #[serde(default)]
        patch: RulePatch,
        #[serde(default)]
        priority: i32,
    },
    /// 抽取（draft/抽卡/抽牌）：从池表按规则抽 N 个到目的地。
    DrawFromPool {
        #[serde(default)]
        pool_table: String,
        #[serde(default)]
        draw_count_expr: String,
        #[serde(default)]
        draw_rule: String,
        #[serde(default)]
        destination: String,
    },
    /// 检定：公式 vs 难度表达式，按成败走不同效果分支（可嵌套）。
    RollCheck {
        #[serde(default)]
        formula: String,
        #[serde(default)]
        difficulty_expr: String,
        #[serde(default)]
        on_success: Vec<EffectSpec>,
        #[serde(default)]
        on_failure: Vec<EffectSpec>,
    },

    // ===== 第 3 层 逃生舱口（转录投影）=====
    /// 自定义效果：verb + 类型化 operands + **必填 GWT 三段模板**。
    /// 类型层不拦缺失（缺键旧档可读）；spec 级校验要求三段非空，
    /// C0 按 R2 阻塞属波 1。
    Custom {
        #[serde(default)]
        verb: String,
        #[serde(default)]
        operands: BTreeMap<String, String>,
        #[serde(default)]
        given: String,
        #[serde(default, rename = "when")]
        when_: String,
        #[serde(default)]
        then: String,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    fn roundtrip(effect: &EffectSpec) -> EffectSpec {
        let json = serde_json::to_string(effect).unwrap();
        serde_json::from_str(&json).unwrap()
    }

    /// I2 铁证：旧 7 变体的原始 JSON 反序列化 + 再序列化逐字节不变。
    #[test]
    fn old_seven_tags_byte_stable() {
        let fixtures = [
            r#"{"effect":"modify_property","entity":"enemy","property":"hp","formula":"hp - damage"}"#,
            r#"{"effect":"spawn_entity","entity":"guard"}"#,
            r#"{"effect":"despawn_entity","entity":"guard"}"#,
            r#"{"effect":"change_state","machine":"door","to_state":"open"}"#,
            r#"{"effect":"grant_resource","resource":"gold","formula":"10 * wave"}"#,
            r#"{"effect":"consume_resource","resource":"mana","formula":"cost"}"#,
            r#"{"effect":"emit_signal","signal":"wave_cleared"}"#,
        ];
        for raw in fixtures {
            let parsed: EffectSpec = serde_json::from_str(raw).unwrap();
            let reserialized = serde_json::to_string(&parsed).unwrap();
            assert_eq!(raw, reserialized, "旧 tag 序列化形态漂移：{raw}");
        }
    }

    #[test]
    fn displace_roundtrip() {
        let effect = EffectSpec::Displace {
            subject: "enemy".into(),
            direction_expr: "away_from(caster)".into(),
            distance_expr: "3".into(),
            duration_expr: "0.5".into(),
        };
        assert_eq!(effect, roundtrip(&effect));
        let json = serde_json::to_string(&effect).unwrap();
        assert!(json.contains(r#""effect":"displace""#));
    }

    #[test]
    fn area_apply_roundtrip() {
        let effect = EffectSpec::AreaApply {
            area_kind: AreaKind::Cone,
            area_params: BTreeMap::from([("angle".to_string(), "60".to_string())]),
            inner: vec![EffectSpec::EmitSignal {
                signal: "hit".into(),
            }],
            target_filter: "faction != caster.faction".into(),
        };
        assert_eq!(effect, roundtrip(&effect));
        let json = serde_json::to_string(&effect).unwrap();
        assert!(json.contains(r#""effect":"area_apply""#) && json.contains(r#""cone""#));
    }

    #[test]
    fn attach_detach_roundtrip() {
        let attach = EffectSpec::Attach {
            modifier_id: "strength_buff".into(),
            target: "self".into(),
            duration_expr: "3_turns".into(),
            priority: 10,
        };
        let detach = EffectSpec::Detach {
            modifier_id: "strength_buff".into(),
            target: "self".into(),
        };
        assert_eq!(attach, roundtrip(&attach));
        assert_eq!(detach, roundtrip(&detach));
        let json = serde_json::to_string(&attach).unwrap();
        assert!(json.contains(r#""priority":10"#));
    }

    #[test]
    fn schedule_roundtrip() {
        let effect = EffectSpec::Schedule {
            timing: ScheduleTiming::Periodic,
            amount_expr: "2".into(),
            unit: ScheduleUnit::Turns,
            inner: vec![EffectSpec::GrantResource {
                resource: "gold".into(),
                formula: "5".into(),
            }],
        };
        assert_eq!(effect, roundtrip(&effect));
        let json = serde_json::to_string(&effect).unwrap();
        assert!(json.contains(r#""periodic""#) && json.contains(r#""turns""#));
    }

    #[test]
    fn modify_rule_all_patches_roundtrip() {
        let patches = [
            RulePatch::ScaleCoefficient {
                expr: "x * 2".into(),
            },
            RulePatch::ReplaceFormula {
                formula: "base + bonus".into(),
            },
            RulePatch::Disable,
            RulePatch::Enable,
            RulePatch::AddPrecondition {
                condition: "hp > 0".into(),
            },
        ];
        for patch in patches {
            let effect = EffectSpec::ModifyRule {
                target_rule: "damage_formula".into(),
                patch,
                priority: -1,
            };
            assert_eq!(effect, roundtrip(&effect));
        }
    }

    #[test]
    fn draw_from_pool_roundtrip() {
        let effect = EffectSpec::DrawFromPool {
            pool_table: "card_pool".into(),
            draw_count_expr: "3".into(),
            draw_rule: "weighted_by_rarity".into(),
            destination: "hand".into(),
        };
        assert_eq!(effect, roundtrip(&effect));
    }

    #[test]
    fn roll_check_roundtrip() {
        let effect = EffectSpec::RollCheck {
            formula: "d20 + perception".into(),
            difficulty_expr: "12".into(),
            on_success: vec![EffectSpec::EmitSignal {
                signal: "spotted".into(),
            }],
            on_failure: vec![EffectSpec::ConsumeResource {
                resource: "time".into(),
                formula: "1".into(),
            }],
        };
        assert_eq!(effect, roundtrip(&effect));
    }

    /// Custom 的 serde 字段名是 `when`（非 Rust 字段名 `when_`）。
    #[test]
    fn custom_roundtrip_uses_when_field_name() {
        let effect = EffectSpec::Custom {
            verb: "merge".into(),
            operands: BTreeMap::from([("left".to_string(), "unit_a".to_string())]),
            given: "两个同级单位相邻".into(),
            when_: "玩家拖拽其一到另一之上".into(),
            then: "合成一个高一级单位".into(),
        };
        assert_eq!(effect, roundtrip(&effect));
        let json = serde_json::to_string(&effect).unwrap();
        assert!(json.contains(r#""when":"#) && !json.contains("when_"));
    }

    /// 缺键旧档可读：新变体全部字段 serde default。
    #[test]
    fn new_variants_tolerate_missing_keys() {
        let bare_tags = [
            r#"{"effect":"displace"}"#,
            r#"{"effect":"area_apply"}"#,
            r#"{"effect":"attach"}"#,
            r#"{"effect":"detach"}"#,
            r#"{"effect":"schedule"}"#,
            r#"{"effect":"modify_rule"}"#,
            r#"{"effect":"draw_from_pool"}"#,
            r#"{"effect":"roll_check"}"#,
            r#"{"effect":"custom"}"#,
        ];
        for raw in bare_tags {
            let parsed: Result<EffectSpec, _> = serde_json::from_str(raw);
            assert!(parsed.is_ok(), "裸 tag 应可反序列化：{raw}");
        }
    }

    /// Custom 缺 then 仍可反序列化（校验拦截属波 1 的 C0）。
    #[test]
    fn custom_missing_then_still_deserializes() {
        let raw = r#"{"effect":"custom","verb":"merge","given":"g","when":"w"}"#;
        let parsed: EffectSpec = serde_json::from_str(raw).unwrap();
        match parsed {
            EffectSpec::Custom { then, when_, .. } => {
                assert!(then.is_empty());
                assert_eq!(when_, "w");
            }
            other => panic!("期望 Custom，得到 {other:?}"),
        }
    }

    /// 三层嵌套（AreaApply 套 Schedule 套 ModifyProperty）往返。
    #[test]
    fn nested_effects_roundtrip() {
        let effect = EffectSpec::AreaApply {
            area_kind: AreaKind::Circle,
            area_params: BTreeMap::from([("radius".to_string(), "5".to_string())]),
            inner: vec![EffectSpec::Schedule {
                timing: ScheduleTiming::OverTime,
                amount_expr: "3".into(),
                unit: ScheduleUnit::Seconds,
                inner: vec![EffectSpec::ModifyProperty {
                    entity: "enemy".into(),
                    property: "hp".into(),
                    formula: "hp - burn".into(),
                }],
            }],
            target_filter: String::new(),
        };
        assert_eq!(effect, roundtrip(&effect));
    }
}
