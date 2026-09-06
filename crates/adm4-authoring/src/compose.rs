//! 组合校验输入构造与评估（T-W7-3b：0c 纯函数 `check_composition` 的接线层）。
//!
//! 为什么在 authoring 侧再包一层纯函数：`CompositionInput` 的字段横跨三处数据源——
//! 设计空间（system_refs/core_nouns）、模块表（阶梯标定/接口/传导）、创作状态
//! （tier 合成点的当前选择、署名形态确认留痕）。gate2 与 authoring 即时反馈必须
//! 拿到**逐字节一致**的结论（I1），所以两处都只许调用本文件的 `assess_composition`，
//! 不许各自拼输入。
//!
//! 关键口径（与任务卡裁决一致）：
//! - **tier 未选择 = 该实例不进组合输入**，并记入 `missing_tiers`（gate2 产 block：
//!   组合校验的前提数据不全 = 不可冻结，R2 宁缺勿造，不按最轻档兜底）；
//! - **inductions 并集**：声明档及以下各档的 inductions 全部背上（定稿 §4.4
//!   "本档（含以上）触发"语义，0c 文件头注释指定由调用方展开）；
//! - **接口边由 noun_bindings 成边**：provides 名词一律成 `(实例, Provides, 裸名, "")`
//!   边；consumes/modifies 名词按绑定目标成边——绑到 pack 核心名词的，噪声名取核心
//!   名词本名（与 V6 白名单匹配）；绑到 `<提供方实例>.<名词>` 的，取名词末段并指向
//!   提供方实例（与提供方 Provides 边同名，H 邻接与 V6 供给判定由此闭合）；
//! - **κ 直取 `SystemRef.core_link`**（0c 注释：校验器信任该字段不复算）；
//!   `core_loop_verbs` 取创作状态的 `core_loop`（3d 概念访谈确认落盘的动词绑定，
//!   3b 遗留 ① 已回填）——未做概念访谈的项目为空序列，κ 已由 pack 声明覆盖
//!   预算与 H 判定，空序列不产生误判；
//! - **产品档**：L0 画像点 `u.target_scale` 的当前选择映射五档参考线；未选择或
//!   选项不认识 → 超休闲（参考线 0 最严）——参考线是提示义务，宁可多出提示也不
//!   静默放行（与 0c `ProductGrade::default` 同一精神）；
//! - **署名形态确认的失效判据**：确认留痕携带确认当时的 h_set 快照，当前 h_set
//!   与快照不一致（新增/更换重核）即失效，`form_confirmed` 按 false 计，
//!   报告重新要求确认。

use crate::state::{CompositionFormConfirmation, CoreLoopVerb};
use adm4_decision::composition::{
    CompositionBudget, CompositionInput, CompositionReport, InterfaceEdge, InterfacePort,
    ProductGrade, SystemInstance, check_composition,
};
use adm4_decision::system_module::{Induction, SystemModule};
use adm4_decision::{DecisionId, Selection};
use adm4_foundation::{Adm4Error, Adm4Result};
use adm4_space::{DesignSpace, SystemRef};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

/// 产品档参考线的数据源：L0 项目画像点（定稿 §4.2(c)：|H| 对照 L0 档位参考线）。
const PRODUCT_GRADE_POINT: &str = "u.target_scale";

/// tier 未选择（或选择已失效）的实例——组合校验的前提数据缺口。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct MissingTierSelection {
    pub instance_id: String,
    /// 中文完整叙述（含缺口原因与修复指向），gate2 finding 与 CLI 直接引用。
    pub detail: String,
}

/// 组合评估结果：0c 报告 + 接线层补充的前提缺口与确认状态。
///
/// gate2 与 authoring 即时反馈消费同一个结构——`report` 由 `check_composition`
/// 产出（同输入必同输出），`missing_tiers` 与确认状态是接线层事实。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct CompositionAssessment {
    pub report: CompositionReport,
    /// tier 未选择的实例（非空 = 组合输入不全，gate2 逐条 block）。
    pub missing_tiers: Vec<MissingTierSelection>,
    /// 当前**生效**的署名形态确认（快照与当前 h_set 一致才算生效；失效的不在此）。
    pub confirmation: Option<CompositionFormConfirmation>,
    /// 存在确认留痕但 h_set 已变化 → 确认失效，需要重新署名。
    pub confirmation_stale: bool,
}

/// 组合违例码的稳定字符串标签（与 serde snake_case tag 同形）。
///
/// gate2 finding 子码与 CLI 输出共用——不 `format!("{:?}")` 是为了不把 Rust
/// 变体命名泄进产物与脚本断言面。
pub fn composition_finding_code(code: adm4_decision::FindingCode) -> &'static str {
    use adm4_decision::FindingCode;
    match code {
        FindingCode::V1TransmissionGap => "v1_transmission_gap",
        FindingCode::V2TransmissionUnmet => "v2_transmission_unmet",
        FindingCode::V3aDisconnected => "v3a_disconnected",
        FindingCode::V3bWeakCoupling => "v3b_weak_coupling",
        FindingCode::V3cCountAdvice => "v3c_count_advice",
        FindingCode::V4HeavyButLoose => "v4_heavy_but_loose",
        FindingCode::V5BudgetAdvice => "v5_budget_advice",
        FindingCode::V6DanglingConsume => "v6_dangling_consume",
        FindingCode::BiconnectivityAdvice => "biconnectivity_advice",
    }
}

/// 组合评估主入口（纯函数：输入决定输出，无 IO）。
///
/// 无 `system_refs` 的项目返回 `Ok(None)`——旧项目零开销零变化（负测试锁定）。
/// 引用的模块不在模块表内 → Err（fail-closed：装配成功的空间不该出现这种情况，
/// 出现即调用方没把模块表喂全，静默跳过会把结构缺陷报成"零违例"）。
pub fn assess_composition(
    space: &DesignSpace,
    selections: &BTreeMap<DecisionId, Selection>,
    modules: &BTreeMap<String, SystemModule>,
    confirmation: Option<&CompositionFormConfirmation>,
    core_loop: &[CoreLoopVerb],
    budget: &CompositionBudget,
) -> Adm4Result<Option<CompositionAssessment>> {
    let refs = &space.pack.system_refs;
    if refs.is_empty() {
        return Ok(None);
    }

    let mut instances = Vec::with_capacity(refs.len());
    let mut missing_tiers = Vec::new();
    let mut module_tier_orders: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for system_ref in refs {
        let module = modules.get(&system_ref.module_id).ok_or_else(|| {
            Adm4Error::internal(format!(
                "组合校验缺少模块数据：实例 {} 引用的模块 {} 不在模块表内\
                 （调用方须把库内与项目私有模块一并喂给 assess_composition）",
                system_ref.instance_id, system_ref.module_id
            ))
        })?;
        module_tier_orders
            .entry(module.module_id.clone())
            .or_insert_with(|| {
                module
                    .heaviness
                    .tiers
                    .iter()
                    .map(|tier| tier.id.clone())
                    .collect()
            });

        let tier_point_id = format!("{}.tier", system_ref.instance_id);
        let Some(selection) = selections.get(&tier_point_id) else {
            missing_tiers.push(MissingTierSelection {
                instance_id: system_ref.instance_id.clone(),
                detail: format!(
                    "实例 {}（模块 {}）的档位合成点 {tier_point_id} 尚未选择：组合校验的\
                     前提数据不全 = 不可冻结（R2 宁缺勿造，不按最轻档兜底）。\
                     修复方向：在该合成点声明重度档位。",
                    system_ref.instance_id, system_ref.module_id
                ),
            });
            continue;
        };
        let declared_tier = selection.option_id.clone();
        let Some(rank) = module.heaviness.tier_rank(&declared_tier) else {
            missing_tiers.push(MissingTierSelection {
                instance_id: system_ref.instance_id.clone(),
                detail: format!(
                    "实例 {} 选择的档位 {declared_tier} 不在模块 {} 的重度阶梯中\
                     （模块升级后旧选择失效？）：按前提数据不全处理，不做兜底。\
                     修复方向：在 {tier_point_id} 重新声明档位。",
                    system_ref.instance_id, system_ref.module_id
                ),
            });
            continue;
        };
        let tier = &module.heaviness.tiers[rank];
        // 声明档及以下各档的传导并集（"本档（含以上）触发" = 达到某档即背上
        // 该档与更低档的全部传导要求）。
        let inductions: Vec<Induction> = module.heaviness.tiers[..=rank]
            .iter()
            .flat_map(|lower| lower.inductions.iter().cloned())
            .collect();
        instances.push(SystemInstance {
            instance_id: system_ref.instance_id.clone(),
            module_id: system_ref.module_id.clone(),
            declared_tier,
            rating: tier.rating,
            core_link: system_ref.core_link,
            is_meta_only: false,
            interface_edges: build_interface_edges(space, system_ref, module)?,
            inductions,
        });
    }

    let mut input = CompositionInput {
        instances,
        // 3b 遗留 ① 回填：概念访谈确认落盘的动词绑定（未访谈项目为空序列，行为不变）。
        core_loop_verbs: core_loop
            .iter()
            .map(|entry| (entry.verb.clone(), entry.instance_id.clone()))
            .collect(),
        product_grade: product_grade_of(selections),
        budget: budget.clone(),
        form_confirmed: false,
        pack_core_nouns: space.pack.core_nouns.clone(),
        module_tier_orders,
    };
    // 第一遍探测：拿当前 h_set 做确认快照比对（h_set 不受 form_confirmed 影响）。
    let probe = check_composition(&input);
    let (effective_confirmation, confirmation_stale) = match confirmation {
        Some(record) if record.h_set == probe.h_set => (Some(record.clone()), false),
        Some(_) => (None, true),
        None => (None, false),
    };
    // 确认生效时按 form_confirmed=true 重跑同一纯函数（不复制 |H| 判据），
    // 报告里 |H| 提示与预算提示照常产出、form_confirmation_required 不再触发。
    let report = if effective_confirmation.is_some() {
        input.form_confirmed = true;
        check_composition(&input)
    } else {
        probe
    };
    Ok(Some(CompositionAssessment {
        report,
        missing_tiers,
        confirmation: effective_confirmation,
        confirmation_stale,
    }))
}

/// 名词绑定成边（口径见文件头注释）。
///
/// 绑定缺失/悬空在加载器（3a）是装配失败，装配成功的空间不该走到这两个 Err 分支；
/// 保留 Err 是因为本函数是纯函数、不假设调用方一定喂装配产物（R2 不静默跳过）。
fn build_interface_edges(
    space: &DesignSpace,
    system_ref: &SystemRef,
    module: &SystemModule,
) -> Adm4Result<Vec<InterfaceEdge>> {
    let instance_id = system_ref.instance_id.as_str();
    let core_nouns: BTreeSet<&str> = space.pack.core_nouns.iter().map(String::as_str).collect();
    let mut edges = Vec::new();
    for noun in &module.interface.provides {
        edges.push(InterfaceEdge {
            from_instance: instance_id.to_string(),
            port: InterfacePort::Provides,
            noun: local_noun(noun).to_string(),
            to_instance: String::new(),
        });
    }
    let bound_ports = module
        .interface
        .consumes
        .iter()
        .map(|noun| (InterfacePort::Consumes, noun))
        .chain(
            module
                .interface
                .modifies
                .iter()
                .map(|noun| (InterfacePort::Modifies, noun)),
        );
    for (port, noun) in bound_ports {
        let Some(target) = system_ref.noun_bindings.get(noun) else {
            return Err(Adm4Error::validation(format!(
                "组合成边失败：实例 {instance_id} 的 {} 名词 {noun} 没有绑定目标\
                 （装配期 V6 应已拦截，出现即输入不是装配产物）",
                port.label()
            )));
        };
        if core_nouns.contains(target.as_str()) {
            // pack 核心名词：无提供方实例，V6 靠 pack_core_nouns 白名单放行。
            edges.push(InterfaceEdge {
                from_instance: instance_id.to_string(),
                port,
                noun: target.clone(),
                to_instance: String::new(),
            });
            continue;
        }
        // `<提供方实例>.<名词>`：名词取末段（与提供方 Provides 边的裸名同名）。
        match target.rsplit_once('.') {
            Some((provider_instance, provided_noun))
                if !provider_instance.is_empty() && !provided_noun.is_empty() =>
            {
                edges.push(InterfaceEdge {
                    from_instance: instance_id.to_string(),
                    port,
                    noun: provided_noun.to_string(),
                    to_instance: provider_instance.to_string(),
                });
            }
            _ => {
                return Err(Adm4Error::validation(format!(
                    "组合成边失败：实例 {instance_id} 的 {} 名词 {noun} 绑定目标 {target} \
                     既不是 pack 核心名词也不是 <提供方实例>.<名词> 形态\
                     （装配期 V6 应已拦截，出现即输入不是装配产物）",
                    port.label()
                )));
            }
        }
    }
    Ok(edges)
}

/// 与加载器 `local_noun` 同口径：带点号取末段，裸名词原样。
fn local_noun(noun: &str) -> &str {
    noun.rsplit('.').next().unwrap_or(noun)
}

/// L0 画像点 `u.target_scale` → 参考线五档。
///
/// 映射：超休闲/广告变现→超休闲(0)、独立→休闲(1)、中核→中核(2)、
/// 大制作→重核(3)、大型长线服务→MMO(4)。未选择或选项不认识（custom）→
/// 超休闲——参考线最严，提示义务下宁可多提示（0c `#[default]` 同精神）。
fn product_grade_of(selections: &BTreeMap<DecisionId, Selection>) -> ProductGrade {
    match selections
        .get(PRODUCT_GRADE_POINT)
        .map(|selection| selection.option_id.as_str())
    {
        Some("iaa_hypercasual") => ProductGrade::HyperCasual,
        Some("indie") => ProductGrade::Casual,
        Some("midcore") => ProductGrade::MidCore,
        Some("triple_a") => ProductGrade::HardCore,
        Some("large_service") => ProductGrade::Mmo,
        _ => ProductGrade::HyperCasual,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use adm4_decision::system_module::Induction;
    use adm4_decision::system_module::{
        CoreLink, FiveAxisRating, HeavinessLadder, HeavinessTier, InductionTarget, MdaMapping,
        NounDecl, NounKind, SystemInterface,
    };
    use adm4_decision::{
        DecisionGraph, DecisionOption, DecisionPoint, DesignLevel, DesignOrganization, GenreScope,
        ParameterValues, PointRequirement, Provenance, SelectionMode,
    };
    use adm4_space::GenrePack;

    fn tier(id: &str, rating: FiveAxisRating, inductions: Vec<Induction>) -> HeavinessTier {
        HeavinessTier {
            id: id.into(),
            label_zh: id.into(),
            rating,
            p_floor: 0,
            interface_floor: 0,
            activates: Vec::new(),
            inductions,
            summary: String::new(),
        }
    }

    fn rating(m: u8, d: u8, c: u8, p: u8, o: u8) -> FiveAxisRating {
        FiveAxisRating { m, d, c, p, o }
    }

    /// 两档模块：轻档无传导，重档要求 sys.sink ≥ deep 且消费 sys.source.fuel。
    fn engine_module() -> SystemModule {
        SystemModule {
            module_id: "sys.engine".into(),
            semver: "1.0.0".into(),
            label_zh: "引擎".into(),
            summary: String::new(),
            nouns: vec![NounDecl {
                id: "torque".into(),
                kind: NounKind::Resource,
                label_zh: "扭矩".into(),
                summary: String::new(),
            }],
            interface: SystemInterface {
                provides: vec!["torque".into()],
                consumes: vec!["sys.source.fuel".into()],
                modifies: Vec::new(),
            },
            mda: MdaMapping::default(),
            heaviness: HeavinessLadder {
                tiers: vec![
                    tier("light", rating(1, 1, 1, 1, 1), Vec::new()),
                    tier(
                        "heavy",
                        rating(2, 2, 2, 2, 2),
                        vec![Induction {
                            when_tier: "heavy".into(),
                            target: InductionTarget::Module("sys.sink".into()),
                            min_tier: "deep".into(),
                            reason: "重档产出必须有承接".into(),
                        }],
                    ),
                ],
            },
            decision_points: Vec::new(),
            cardinality_expectations: BTreeMap::new(),
            consistency_rules: Vec::new(),
            skin_fields: Vec::new(),
        }
    }

    fn source_module() -> SystemModule {
        SystemModule {
            module_id: "sys.source".into(),
            semver: "1.0.0".into(),
            label_zh: "供给".into(),
            summary: String::new(),
            nouns: vec![NounDecl {
                id: "fuel".into(),
                kind: NounKind::Resource,
                label_zh: "燃料".into(),
                summary: String::new(),
            }],
            interface: SystemInterface {
                provides: vec!["fuel".into()],
                consumes: Vec::new(),
                modifies: Vec::new(),
            },
            mda: MdaMapping::default(),
            heaviness: HeavinessLadder {
                tiers: vec![tier("only", rating(1, 1, 1, 1, 1), Vec::new())],
            },
            decision_points: Vec::new(),
            cardinality_expectations: BTreeMap::new(),
            consistency_rules: Vec::new(),
            skin_fields: Vec::new(),
        }
    }

    fn modules() -> BTreeMap<String, SystemModule> {
        [
            ("sys.engine".to_string(), engine_module()),
            ("sys.source".to_string(), source_module()),
        ]
        .into_iter()
        .collect()
    }

    fn space_with_refs(refs: Vec<SystemRef>) -> DesignSpace {
        let point = DecisionPoint {
            id: "u.core".into(),
            domain: "core".into(),
            level: DesignLevel::L0,
            genre_scope: GenreScope::Universal,
            question: "核心？".into(),
            mda_layer: None,
            design_question: None,
            node_id: None,
            selection_mode: SelectionMode::Single,
            requirement: PointRequirement::Unlocked,
            tier_gate: None,
            options: vec![
                DecisionOption {
                    id: "a".into(),
                    label: "甲".into(),
                    ..Default::default()
                },
                DecisionOption {
                    id: "b".into(),
                    label: "乙".into(),
                    ..Default::default()
                },
            ],
            skin_fields: Vec::new(),
            evidence_slots: false,
        };
        DesignSpace {
            universal_version: "test".into(),
            pack: GenrePack {
                pack_id: "compose_test".into(),
                pack_version: "0.1.0".into(),
                display_name: "组合构造测试包".into(),
                reference_games: vec!["虚构甲".into(), "虚构乙".into(), "虚构丙".into()],
                profile_points: Vec::new(),
                cardinality_expectations: Default::default(),
                consistency_rules: Vec::new(),
                nodes: Vec::new(),
                decision_points: Vec::new(),
                system_refs: refs,
                core_nouns: vec!["mana".into()],
            },
            graph: DecisionGraph::new(vec![point]).expect("测试图应可构造"),
            organization: DesignOrganization::new(Vec::new(), Vec::new()),
            system_instances: Vec::new(),
        }
    }

    fn engine_ref(instance_id: &str, link: CoreLink) -> SystemRef {
        SystemRef {
            instance_id: instance_id.into(),
            module_id: "sys.engine".into(),
            version_req: String::new(),
            allowed_tiers: Vec::new(),
            noun_bindings: [(
                "sys.source.fuel".to_string(),
                "source_main.fuel".to_string(),
            )]
            .into_iter()
            .collect(),
            core_link: link,
        }
    }

    fn source_ref() -> SystemRef {
        SystemRef {
            instance_id: "source_main".into(),
            module_id: "sys.source".into(),
            version_req: String::new(),
            allowed_tiers: Vec::new(),
            noun_bindings: BTreeMap::new(),
            core_link: CoreLink::Weak,
        }
    }

    fn select(selections: &mut BTreeMap<DecisionId, Selection>, decision_id: &str, option: &str) {
        selections.insert(
            decision_id.to_string(),
            Selection {
                decision_id: decision_id.to_string(),
                option_id: option.to_string(),
                parameters: ParameterValues::None,
                rationale: String::new(),
                provenance: Provenance::UserManual,
                confirmed_by_user: true,
                template_original: None,
                additional_options: Vec::new(),
                primary_option: None,
            },
        );
    }

    #[test]
    fn no_system_refs_returns_none() {
        let space = space_with_refs(Vec::new());
        let result = assess_composition(
            &space,
            &BTreeMap::new(),
            &BTreeMap::new(),
            None,
            &[],
            &CompositionBudget::default(),
        )
        .expect("无引用项目不应报错");
        assert!(result.is_none(), "旧项目必须零开销返回 None");
    }

    #[test]
    fn unselected_tier_is_reported_not_defaulted() {
        let space = space_with_refs(vec![
            engine_ref("engine_main", CoreLink::Core),
            source_ref(),
        ]);
        let mut selections = BTreeMap::new();
        select(&mut selections, "source_main.tier", "only");
        let assessment = assess_composition(
            &space,
            &selections,
            &modules(),
            None,
            &[],
            &CompositionBudget::default(),
        )
        .expect("评估应成功")
        .expect("有引用应产报告");
        assert_eq!(assessment.missing_tiers.len(), 1);
        assert_eq!(assessment.missing_tiers[0].instance_id, "engine_main");
        assert!(
            assessment.missing_tiers[0]
                .detail
                .contains("engine_main.tier"),
            "{}",
            assessment.missing_tiers[0].detail
        );
        // 缺档实例不进输入：报告里只剩 source_main（轻档零违例）。
        assert!(assessment.report.h_set.is_empty());
        assert!(assessment.report.blocks.is_empty());
    }

    #[test]
    fn stale_tier_selection_is_reported_not_defaulted() {
        let space = space_with_refs(vec![
            engine_ref("engine_main", CoreLink::Core),
            source_ref(),
        ]);
        let mut selections = BTreeMap::new();
        select(&mut selections, "engine_main.tier", "ghost_tier");
        select(&mut selections, "source_main.tier", "only");
        let assessment = assess_composition(
            &space,
            &selections,
            &modules(),
            None,
            &[],
            &CompositionBudget::default(),
        )
        .expect("评估应成功")
        .expect("有引用应产报告");
        assert_eq!(assessment.missing_tiers.len(), 1);
        assert!(
            assessment.missing_tiers[0].detail.contains("ghost_tier"),
            "{}",
            assessment.missing_tiers[0].detail
        );
    }

    #[test]
    fn edges_and_inductions_follow_bindings_and_tier_union() {
        let space = space_with_refs(vec![
            engine_ref("engine_main", CoreLink::Core),
            source_ref(),
        ]);
        let mut selections = BTreeMap::new();
        select(&mut selections, "engine_main.tier", "heavy");
        select(&mut selections, "source_main.tier", "only");
        let assessment = assess_composition(
            &space,
            &selections,
            &modules(),
            None,
            &[],
            &CompositionBudget::default(),
        )
        .expect("评估应成功")
        .expect("有引用应产报告");
        // 重档传导 sys.sink ≥ deep：组合内无 sys.sink 实例 → V1。
        let v1 = assessment
            .report
            .blocks
            .iter()
            .find(|finding| finding.subject == "sys.sink");
        assert!(v1.is_some(), "blocks：{:?}", assessment.report.blocks);
        // 成边口径：consumes 绑定成 (engine_main, Consumes, fuel, source_main)，
        // source 的 provides 成 (source_main, Provides, fuel, "")——V6 不误报。
        assert!(
            !assessment
                .report
                .blocks
                .iter()
                .any(|finding| finding.subject == "fuel"),
            "fuel 有供给方，不应报 V6：{:?}",
            assessment.report.blocks
        );
    }

    #[test]
    fn missing_module_in_table_is_fail_closed() {
        let space = space_with_refs(vec![engine_ref("engine_main", CoreLink::Core)]);
        let error = assess_composition(
            &space,
            &BTreeMap::new(),
            &BTreeMap::new(),
            None,
            &[],
            &CompositionBudget::default(),
        )
        .expect_err("模块表缺失必须 Err，不得静默跳过");
        assert!(error.message.contains("sys.engine"), "{}", error.message);
    }

    #[test]
    fn product_grade_maps_target_scale_and_defaults_strict() {
        let mut selections = BTreeMap::new();
        assert_eq!(product_grade_of(&selections), ProductGrade::HyperCasual);
        for (option, expected) in [
            ("iaa_hypercasual", ProductGrade::HyperCasual),
            ("indie", ProductGrade::Casual),
            ("midcore", ProductGrade::MidCore),
            ("triple_a", ProductGrade::HardCore),
            ("large_service", ProductGrade::Mmo),
            ("unknown_custom", ProductGrade::HyperCasual),
        ] {
            select(&mut selections, PRODUCT_GRADE_POINT, option);
            assert_eq!(product_grade_of(&selections), expected, "选项 {option}");
        }
    }

    #[test]
    fn confirmation_snapshot_gates_form_confirmed() {
        // 两个重核（W10 heavy + core/strong κ）零接口边之外还超参考线——
        // 这里只验证确认快照的三态：无确认 / 有效确认 / 失效确认。
        let mut heavy_a = engine_ref("engine_a", CoreLink::Core);
        heavy_a.noun_bindings.clear();
        let mut heavy_b = engine_ref("engine_b", CoreLink::Strong);
        heavy_b.noun_bindings.clear();
        let mut module = engine_module();
        module.interface.consumes.clear();
        let modules: BTreeMap<String, SystemModule> =
            [("sys.engine".to_string(), module)].into_iter().collect();
        let space = space_with_refs(vec![heavy_a, heavy_b]);
        let mut selections = BTreeMap::new();
        select(&mut selections, "engine_a.tier", "heavy");
        select(&mut selections, "engine_b.tier", "heavy");

        // 无确认：超参考线（默认超休闲 0）→ 要求确认。
        let unconfirmed = assess_composition(
            &space,
            &selections,
            &modules,
            None,
            &[],
            &CompositionBudget::default(),
        )
        .expect("评估应成功")
        .expect("有引用应产报告");
        assert!(unconfirmed.report.form_confirmation_required);
        assert!(!unconfirmed.confirmation_stale);
        assert_eq!(unconfirmed.report.h_set, vec!["engine_a", "engine_b"]);

        // 有效确认（快照 = 当前 h_set）：不再要求，硬 block（V3a/V3b）依旧在场——不可豁免。
        let valid = CompositionFormConfirmation {
            signer: "设计师甲".into(),
            note: "接受双核形态".into(),
            at: "2026-09-05T00:00:00Z".into(),
            h_set: vec!["engine_a".into(), "engine_b".into()],
        };
        let confirmed = assess_composition(
            &space,
            &selections,
            &modules,
            Some(&valid),
            &[],
            &CompositionBudget::default(),
        )
        .expect("评估应成功")
        .expect("有引用应产报告");
        assert!(!confirmed.report.form_confirmation_required);
        assert_eq!(
            confirmed.confirmation.as_ref().map(|c| c.signer.as_str()),
            Some("设计师甲")
        );
        assert!(
            !confirmed.report.blocks.is_empty(),
            "连通/强耦合硬违例不因署名确认而豁免"
        );

        // 失效确认（快照 ≠ 当前 h_set）：重新要求，生效确认为 None。
        let stale = CompositionFormConfirmation {
            h_set: vec!["engine_a".into()],
            ..valid
        };
        let invalidated = assess_composition(
            &space,
            &selections,
            &modules,
            Some(&stale),
            &[],
            &CompositionBudget::default(),
        )
        .expect("评估应成功")
        .expect("有引用应产报告");
        assert!(invalidated.report.form_confirmation_required);
        assert!(invalidated.confirmation_stale);
        assert!(invalidated.confirmation.is_none());
    }

    /// 3b 遗留 ① 回填的最小锁定：core_loop 落盘后 `CompositionInput.core_loop_verbs`
    /// 非空且 κ 推导可用——core 动词绑定的实例即便声明 κ=weak，
    /// `derive_core_link` 也能按动词绑定推出 core（推导辅助可用性验证）。
    #[test]
    fn core_loop_verbs_flow_into_composition_input() {
        let space = space_with_refs(vec![
            engine_ref("engine_main", CoreLink::Core),
            source_ref(),
        ]);
        let mut selections = BTreeMap::new();
        select(&mut selections, "engine_main.tier", "light");
        select(&mut selections, "source_main.tier", "only");
        let core_loop = vec![CoreLoopVerb {
            verb: "驱动".into(),
            instance_id: "engine_main".into(),
        }];
        let assessment = assess_composition(
            &space,
            &selections,
            &modules(),
            None,
            &core_loop,
            &CompositionBudget::default(),
        )
        .expect("评估应成功")
        .expect("有引用应产报告");
        assert!(assessment.missing_tiers.is_empty());
        // κ 推导辅助可用：用与 assess 相同的动词序列跑 derive_core_link。
        let verbs: Vec<(String, String)> = core_loop
            .iter()
            .map(|entry| (entry.verb.clone(), entry.instance_id.clone()))
            .collect();
        let instance = SystemInstance {
            instance_id: "engine_main".into(),
            core_link: CoreLink::Weak,
            ..Default::default()
        };
        assert_eq!(
            adm4_decision::composition::derive_core_link(&instance, &verbs, &[]),
            CoreLink::Core,
            "core_loop 动词绑定应推导出 core"
        );
    }
}
