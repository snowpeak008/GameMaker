//! 机制级 custom 一等入口（W7 定稿 §5.6，T-W7-2）。
//!
//! 设计要点：
//! - **草案与内建 L4 同信息密度**：归属系统必填（悬空即拒）、rule_text 必填、
//!   effects 走 EffectSpec 全集当场反序列化 + 悬空校验（由 [`EffectTemplateValidator`]
//!   注入——EffectSpec 类型在 adm4-spec，本 crate 不引它，校验实现在 adm4-app 门面层）、
//!   rationale 必填（随 Selection 进 C0 的 design_notes）。
//! - **落地为项目私有单选点**：`is_custom: true`，id 形如 `custom.<host>.<slug>`，
//!   选项恰一个、自动选中但**未确认**——确认是用户手势（AI 永不代确认）。
//! - **C0 零特殊分支**：合成点带 `spec_role=mechanic` + `system=<host>` 标签与
//!   `effects_template`，走全部既有编译链路；流水线侧由 `FrozenDesign::custom_points`
//!   增广设计空间（见 [`augment_space_with_points`]），pipeline crate 不感知 custom。
//! - **"老问题的新答案"由引擎统一注入**：每个既有 L3/L4 点的「自定义答案」占位选项
//!   由呈现层按 [`CUSTOM_ENTRY_OPTION_ID`] 注入（不改 pack 数据），选中即引导走
//!   `add_custom_mechanic` 流；占位选项本身不可被选择/确认（引擎入口硬拦）。

use adm4_contracts::TypedValue;
use adm4_decision::{
    DecisionGraph, DecisionOption, DecisionPoint, DesignLevel, MdaLayer, ParameterSchema,
    PointRequirement, ScalarField, Selection, SelectionMode,
};
use adm4_foundation::{Adm4Error, Adm4Result};
use adm4_space::DesignSpace;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// 呈现层统一注入的「自定义答案」占位选项 id。
///
/// 双下划线前缀保证不与任何 pack 声明的选项 id 冲突；它**不进决策图**（呈现层注入），
/// 引擎的 select/add_option 对它硬拦并引导走 `add_custom_mechanic`。
pub const CUSTOM_ENTRY_OPTION_ID: &str = "__custom_entry__";

/// 占位选项的展示名（CLI 与桌面端共用，不在两处各写一份文案）。
pub const CUSTOM_ENTRY_LABEL: &str = "自定义答案（创建项目私有机制）";

/// 占位选项的说明文案。
pub const CUSTOM_ENTRY_SUMMARY: &str = "预设选项都不是你要的机制时，从这里录入自定义机制草案（custom add）：\
     规则文本、效果（EffectSpec）与设计理由缺一不可，与内建机制同信息密度。";

/// 合成点上唯一选项的 id（引擎自动选中它；确认仍走用户手势）。
pub const CUSTOM_RULE_OPTION_ID: &str = "custom_rule";

/// 效果模板校验器：反序列化 EffectSpec 全集 + 悬空校验 + Custom 变体 GWT 三段非空。
///
/// EffectSpec 类型在 adm4-spec（本 crate 无该依赖），因此校验实现由上层注入：
/// `adm4_app::AppServices` 提供基于真 EffectSpec 的实现；authoring 单测可用桩。
/// 签名把它做成 `add_custom_mechanic` 的必传参数，调用方**无法**跳过效果校验。
pub trait EffectTemplateValidator {
    /// 校验第 `position` 个（1 起）效果模板；不合法（含悬空引用）即 Err。
    ///
    /// `space` 是**当前已增广**的空间（此前登记的 custom 机制可被 ModifyRule 指向）；
    /// `new_nouns` 是草案显式登记的新名词（引用它们不算悬空）。
    fn validate_template(
        &self,
        space: &DesignSpace,
        decision_id: &str,
        new_nouns: &[String],
        template: &serde_json::Value,
        position: usize,
    ) -> Adm4Result<()>;
}

/// 机制级 custom 草案（`add_custom_mechanic` 的入参；CLI 的 `--file` JSON 即本结构）。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CustomMechanicDraft {
    /// 归属系统的 L3 决策点 id（必须存在于当前空间且已被选择——悬空即拒）。
    pub host_system_id: String,
    /// 机制短名（`[a-z0-9_]`），决策点 id = `custom.<host>.<slug>`。
    pub slug: String,
    /// 中文机制名（进选项 label 与 C0 的 rule_text 前缀）。
    pub label_zh: String,
    /// 规则文本（进选项 summary → C0 的 MechanicSpec::rule_text → C4 的 When）。
    pub rule_text: String,
    /// 效果清单：EffectSpec 的 JSON 形态（支持 `{param:KEY}` 占位符），
    /// 登记时当场反序列化 + 悬空校验（见 [`EffectTemplateValidator`]）。
    pub effects: Vec<serde_json::Value>,
    /// 标量参数值（可选）：键值直接落 Selection 参数，schema 由值类型推断
    /// （与既有 L4 参数形态一致——标量字段组）。
    #[serde(default)]
    pub parameters: Option<BTreeMap<String, TypedValue>>,
    /// 显式登记的新名词：effects 引用了尚不在 spec 可解析域内的实体/表名时必须在此
    /// 申报，否则悬空即拒（R2：不静默放行发明名词）。注意登记只解锁草案校验，
    /// 结构化引用（如 ModifyProperty 的 entity）仍须最终落在实体表里才能过 C0/C1。
    #[serde(default)]
    pub new_nouns: Vec<String>,
    /// 设计理由（必填；落 Selection::rationale → C0 design_notes → C2 叙述素材）。
    pub rationale: String,
}

/// 已登记的 custom 机制（创作状态持久化形态；旧存档无此字段 → 空 map，I2 守恒）。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CustomMechanicRecord {
    /// 合成决策点 id（`custom.<host>.<slug>`，也是 map 键）。
    pub decision_id: String,
    #[serde(flatten)]
    pub draft: CustomMechanicDraft,
    pub created_at: String,
}

/// custom 机制的决策点 id 口径（引擎与呈现层共用）。
pub fn custom_decision_id(host_system_id: &str, slug: &str) -> String {
    format!("custom.{host_system_id}.{slug}")
}

/// 校验草案（信息密度与内建 L4 同级；任一不满足即拒，不落半成品）。
/// 通过时返回合成决策点 id。
pub fn validate_draft(
    space: &DesignSpace,
    selections: &BTreeMap<String, Selection>,
    draft: &CustomMechanicDraft,
    validator: &dyn EffectTemplateValidator,
) -> Adm4Result<String> {
    let slug_ok = !draft.slug.trim().is_empty()
        && draft.slug.chars().all(|character| {
            character.is_ascii_lowercase() || character.is_ascii_digit() || character == '_'
        });
    if !slug_ok {
        return Err(Adm4Error::invalid_input(format!(
            "custom 机制 slug 非法（只接受小写字母/数字/下划线）：{:?}",
            draft.slug
        )));
    }
    if draft.label_zh.trim().is_empty() {
        return Err(Adm4Error::invalid_input(
            "custom 机制必须提供机制名（label_zh）",
        ));
    }
    if draft.rule_text.trim().is_empty() {
        return Err(Adm4Error::invalid_input(
            "custom 机制必须提供规则文本（rule_text）：与内建 L4 同信息密度，机制规则不可留白",
        ));
    }
    if draft.rationale.trim().is_empty() {
        return Err(Adm4Error::invalid_input(
            "custom 机制必须提供设计理由（rationale）：理由进 design_notes，是 C2 叙述与红队评审的素材",
        ));
    }
    // 归属系统：必须存在于当前空间、是 L3 系统点、且已被选择（悬空即拒）。
    let host = space.graph.point(&draft.host_system_id).ok_or_else(|| {
        Adm4Error::not_found(format!(
            "custom 机制的归属系统 {} 不在当前设计空间内（悬空即拒）",
            draft.host_system_id
        ))
    })?;
    if host.level != DesignLevel::L3 {
        return Err(Adm4Error::invalid_input(format!(
            "custom 机制的归属点 {} 不是 L3 系统点（level={:?}）：机制必须挂在系统之下",
            draft.host_system_id, host.level
        )));
    }
    if !selections.contains_key(&draft.host_system_id) {
        return Err(Adm4Error::conflict(format!(
            "custom 机制的归属系统 {} 尚未被选择：请先在该 L3 点选定系统结构",
            draft.host_system_id
        )));
    }
    let decision_id = custom_decision_id(&draft.host_system_id, &draft.slug);
    if space.graph.point(&decision_id).is_some() {
        return Err(Adm4Error::conflict(format!(
            "决策点 {decision_id} 已存在（slug 重复或与清单冲突），请换一个 slug"
        )));
    }
    if draft.effects.is_empty() {
        return Err(Adm4Error::invalid_input(
            "custom 机制必须至少声明一个效果（effects）：流水线不发明效果（R2）",
        ));
    }
    for (index, template) in draft.effects.iter().enumerate() {
        validator.validate_template(space, &decision_id, &draft.new_nouns, template, index + 1)?;
    }
    Ok(decision_id)
}

/// 由登记记录合成项目私有 L4 决策点（`is_custom: true`，单选、恰一个选项）。
///
/// domain/node/genre_scope 继承归属系统点：C0 的机制归属回落、C6 的装配聚合、
/// 左栏领域进度三处因此都把它当作该系统域内的普通机制点处理（零特殊分支）。
pub fn synthesize_point(record: &CustomMechanicRecord, host: &DecisionPoint) -> DecisionPoint {
    let draft = &record.draft;
    let parameter_schema = match &draft.parameters {
        None => ParameterSchema::None,
        Some(entries) if entries.is_empty() => ParameterSchema::None,
        Some(entries) => ParameterSchema::Scalar {
            fields: entries
                .iter()
                .map(|(key, value)| ScalarField {
                    key: key.clone(),
                    kind: value_kind_of(value),
                    constraint: None,
                    required: true,
                    // 文本参数按皮字段处理（命名/主题/文案类，R5 换皮门比对粒度）。
                    is_skin: matches!(value, TypedValue::Text(_)),
                })
                .collect(),
        },
    };
    let option = DecisionOption {
        id: CUSTOM_RULE_OPTION_ID.into(),
        label: draft.label_zh.clone(),
        summary: draft.rule_text.clone(),
        implications: Vec::new(),
        requires: Vec::new(),
        conflicts: Vec::new(),
        unlocks: Vec::new(),
        parameter_schema,
        is_custom: true,
        compiler_tags: BTreeMap::from([
            ("spec_role".to_string(), "mechanic".to_string()),
            ("system".to_string(), draft.host_system_id.clone()),
        ]),
        effects_template: draft.effects.clone(),
    };
    DecisionPoint {
        id: record.decision_id.clone(),
        domain: host.domain.clone(),
        level: DesignLevel::L4,
        genre_scope: host.genre_scope.clone(),
        question: format!("自定义机制「{}」的规则是什么？", draft.label_zh),
        mda_layer: Some(MdaLayer::Mechanics),
        design_question: None,
        node_id: host.node_id.clone(),
        selection_mode: SelectionMode::Single,
        requirement: PointRequirement::Unlocked,
        tier_gate: None,
        options: vec![option],
        skin_fields: Vec::new(),
        evidence_slots: false,
    }
}

fn value_kind_of(value: &TypedValue) -> adm4_contracts::ValueKind {
    match value {
        TypedValue::Bool(_) => adm4_contracts::ValueKind::Bool,
        TypedValue::Int(_) => adm4_contracts::ValueKind::Int,
        TypedValue::Float(_) => adm4_contracts::ValueKind::Float,
        TypedValue::Text(_) => adm4_contracts::ValueKind::Text,
    }
}

/// 用登记记录增广设计空间（创作侧：引擎构造与 add/remove 后重建）。
///
/// `base` 必须是**未增广**的原始空间，否则重复 id 会被 `DecisionGraph::new` 拒绝
/// （fail-closed，不静默去重）。
pub fn augment_space(
    base: &DesignSpace,
    records: &BTreeMap<String, CustomMechanicRecord>,
) -> Adm4Result<DesignSpace> {
    let points: Vec<DecisionPoint> = records
        .values()
        .map(|record| {
            let host = base
                .graph
                .point(&record.draft.host_system_id)
                .ok_or_else(|| {
                    Adm4Error::validation(format!(
                        "custom 机制 {} 的归属系统 {} 已不在设计空间内（清单变更？）",
                        record.decision_id, record.draft.host_system_id
                    ))
                })?;
            Ok(synthesize_point(record, host))
        })
        .collect::<Adm4Result<Vec<_>>>()?;
    augment_space_with_points(base, &points)
}

/// 用现成决策点增广设计空间（流水线侧：`FrozenDesign::custom_points` 直接进图）。
///
/// pipeline crate 因此完全不感知 custom——C0-C6 拿到的空间里 custom 点就是普通 L4 点。
pub fn augment_space_with_points(
    base: &DesignSpace,
    custom_points: &[DecisionPoint],
) -> Adm4Result<DesignSpace> {
    if custom_points.is_empty() {
        return Ok(base.clone());
    }
    let mut points = base.graph.points().to_vec();
    points.extend(custom_points.iter().cloned());
    Ok(DesignSpace {
        universal_version: base.universal_version.clone(),
        pack: base.pack.clone(),
        graph: DecisionGraph::new(points).map_err(|error| {
            Adm4Error::validation(format!("custom 决策点并入设计空间失败：{}", error.message))
        })?,
        organization: base.organization.clone(),
        system_instances: base.system_instances.clone(),
    })
}

/// 效果模板 JSON 里是否有 `modify_rule` 指向 `rule_id`（递归遍历嵌套容器）。
///
/// 用于 `remove_custom_mechanic` 的引用守卫：删掉被别的 custom 机制 ModifyRule
/// 指向的机制，会让那条机制在 C1 悬空。按裸 JSON 走查（不反序列化 EffectSpec，
/// 本 crate 无该类型），键名口径与 adm4-spec 的 serde 形态一致。
pub fn template_references_rule(template: &serde_json::Value, rule_id: &str) -> bool {
    match template {
        serde_json::Value::Object(map) => {
            let is_hit = map.get("effect").and_then(|tag| tag.as_str()) == Some("modify_rule")
                && map.get("target_rule").and_then(|target| target.as_str()) == Some(rule_id);
            is_hit
                || map
                    .values()
                    .any(|value| template_references_rule(value, rule_id))
        }
        serde_json::Value::Array(items) => items
            .iter()
            .any(|item| template_references_rule(item, rule_id)),
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use adm4_decision::GenreScope;

    /// 单测桩：不做 EffectSpec 反序列化（真校验在 adm4-app 层，有 e2e 钉住）。
    pub(crate) struct AcceptAllValidator;
    impl EffectTemplateValidator for AcceptAllValidator {
        fn validate_template(
            &self,
            _space: &DesignSpace,
            _decision_id: &str,
            _new_nouns: &[String],
            _template: &serde_json::Value,
            _position: usize,
        ) -> Adm4Result<()> {
            Ok(())
        }
    }

    fn draft() -> CustomMechanicDraft {
        CustomMechanicDraft {
            host_system_id: "sys.combat".into(),
            slug: "auto_target".into(),
            label_zh: "可编程索敌".into(),
            rule_text: "条件-动作规则决定索敌".into(),
            effects: vec![serde_json::json!({
                "effect": "custom", "verb": "target_select",
                "given": "存在多个敌人", "when": "规则求值", "then": "选出目标"
            })],
            parameters: None,
            new_nouns: Vec::new(),
            rationale: "让玩家自己写索敌逻辑".into(),
        }
    }

    fn host_point() -> DecisionPoint {
        DecisionPoint {
            id: "sys.combat".into(),
            domain: "combat".into(),
            level: DesignLevel::L3,
            genre_scope: GenreScope::Pack("p".into()),
            question: "q".into(),
            mda_layer: None,
            design_question: None,
            node_id: Some("node_combat".into()),
            selection_mode: SelectionMode::Single,
            requirement: PointRequirement::Unlocked,
            tier_gate: None,
            options: vec![DecisionOption {
                id: "melee".into(),
                label: "近战".into(),
                ..Default::default()
            }],
            skin_fields: Vec::new(),
            evidence_slots: false,
        }
    }

    /// validate_draft 的信息密度关卡：空 rule_text / 未选归属系统 / 空 effects 逐一拒绝，
    /// 全部补齐后放行并返回合成决策点 id。
    #[test]
    fn validate_draft_rejects_thin_drafts_and_accepts_complete_ones() {
        let space = DesignSpace {
            universal_version: "test".into(),
            pack: adm4_space::GenrePack {
                pack_id: "p".into(),
                pack_version: "0.1.0".into(),
                display_name: "测试包".into(),
                reference_games: Vec::new(),
                profile_points: Vec::new(),
                cardinality_expectations: BTreeMap::new(),
                consistency_rules: Vec::new(),
                nodes: Vec::new(),
                decision_points: Vec::new(),
                system_refs: Vec::new(),
                core_nouns: Vec::new(),
            },
            graph: DecisionGraph::new(vec![host_point()]).expect("测试图装配失败"),
            organization: Default::default(),
            system_instances: Vec::new(),
        };
        let mut selections = BTreeMap::new();

        // 归属系统尚未选择 → 拒。
        let error = validate_draft(&space, &selections, &draft(), &AcceptAllValidator)
            .expect_err("未选归属系统应拒绝");
        assert!(error.message.contains("尚未被选择"), "{}", error.message);

        selections.insert(
            "sys.combat".to_string(),
            Selection {
                decision_id: "sys.combat".into(),
                option_id: "melee".into(),
                parameters: adm4_decision::ParameterValues::None,
                rationale: String::new(),
                provenance: adm4_decision::Provenance::UserManual,
                confirmed_by_user: true,
                template_original: None,
                additional_options: Vec::new(),
                primary_option: None,
            },
        );

        let mut blank_rule = draft();
        blank_rule.rule_text = "  ".into();
        let error = validate_draft(&space, &selections, &blank_rule, &AcceptAllValidator)
            .expect_err("空 rule_text 应拒绝");
        assert!(error.message.contains("rule_text"), "{}", error.message);

        let mut no_effects = draft();
        no_effects.effects.clear();
        let error = validate_draft(&space, &selections, &no_effects, &AcceptAllValidator)
            .expect_err("空 effects 应拒绝");
        assert!(error.message.contains("效果"), "{}", error.message);

        let decision_id = validate_draft(&space, &selections, &draft(), &AcceptAllValidator)
            .expect("完整草案应放行");
        assert_eq!(decision_id, "custom.sys.combat.auto_target");
    }

    #[test]
    fn decision_id_shape_is_stable() {
        assert_eq!(
            custom_decision_id("ld.combat_system", "auto_target"),
            "custom.ld.combat_system.auto_target"
        );
    }

    #[test]
    fn synthesized_point_inherits_host_organization() {
        let record = CustomMechanicRecord {
            decision_id: "custom.sys.combat.auto_target".into(),
            draft: draft(),
            created_at: "t".into(),
        };
        let point = synthesize_point(&record, &host_point());
        assert_eq!(point.domain, "combat");
        assert_eq!(point.node_id.as_deref(), Some("node_combat"));
        assert_eq!(point.level, DesignLevel::L4);
        assert_eq!(point.options.len(), 1);
        let option = &point.options[0];
        assert!(option.is_custom);
        assert_eq!(
            option.compiler_tags.get("spec_role").map(String::as_str),
            Some("mechanic")
        );
        assert_eq!(
            option.compiler_tags.get("system").map(String::as_str),
            Some("sys.combat")
        );
        assert_eq!(option.summary, "条件-动作规则决定索敌");
        assert_eq!(option.effects_template.len(), 1);
    }

    /// 记录的 serde 形态：draft 打平（flatten），存档里没有嵌套 draft 键。
    #[test]
    fn record_serde_flattens_draft() {
        let record = CustomMechanicRecord {
            decision_id: "custom.sys.combat.auto_target".into(),
            draft: draft(),
            created_at: "2026-09-03T00:00:00Z".into(),
        };
        let json = serde_json::to_string(&record).unwrap();
        assert!(json.contains(r#""host_system_id":"sys.combat""#), "{json}");
        assert!(!json.contains(r#""draft""#), "{json}");
        let back: CustomMechanicRecord = serde_json::from_str(&json).unwrap();
        assert_eq!(back, record);
    }

    #[test]
    fn modify_rule_reference_walker_recurses_into_nested_effects() {
        let flat = serde_json::json!({
            "effect": "modify_rule", "target_rule": "custom.s.a", "patch": {"patch": "disable"}
        });
        assert!(template_references_rule(&flat, "custom.s.a"));
        assert!(!template_references_rule(&flat, "custom.s.b"));
        let nested = serde_json::json!({
            "effect": "schedule", "timing": "periodic",
            "inner": [{ "effect": "modify_rule", "target_rule": "custom.s.a" }]
        });
        assert!(template_references_rule(&nested, "custom.s.a"));
        let unrelated = serde_json::json!({ "effect": "emit_signal", "signal": "custom.s.a" });
        assert!(!template_references_rule(&unrelated, "custom.s.a"));
    }
}
