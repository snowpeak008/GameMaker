//! 组合访谈（T-W7-3d ②）：composition_report 有 blocks/advices 时，AI 用人话
//! 解释违例（传导链/连通缺陷讲清楚）并给出**结构化修复选项**。
//!
//! 纪律与逐点访谈一致：AI 只提案，选定修复选项并执行是用户手势（D11）。
//! 本文件是纯函数（请求构造 + 解析 + 校验），执行落在 `AppServices`：
//! - `tier_change`（升档 X/降档 Y）→ 走既有 `<instance>.tier` select 链路；
//! - `confirm_form`（|H| 情形）→ 引导用户走署名确认 `compose_confirm_form`
//!   （AI 不能代签——选项只携带指路信息，执行入口强制要求 `--signer`）；
//! - `replace_system` / `add_binding`（换系统/改绑定）→ 结构化呈现修复方向，
//!   执行须走系统清单变更通道（概念访谈重confirm 或手改 refs），本卡不自动执行。
//!
//! AI 越界（发明实例 id/档位 id/选项种类）即 Err 不吞（R7）。

use crate::compose::CompositionAssessment;
use adm4_ai::{AiProvider, AiRequest};
use adm4_decision::system_module::SystemModule;
use adm4_foundation::{Adm4Error, Adm4Result};
use adm4_space::DesignSpace;
use compose_fix_support::instance_tier_orders;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// 组合访谈 AI 调用的 purpose 键。
pub const PURPOSE_COMPOSITION: &str = "interview_composition";

/// 修复动作的种类（闭集：AI 输出不在此内 → Err）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum FixActionKind {
    /// 升/降某实例档位（执行 = 改 tier 合成点选择，走既有 select 链路）。
    #[default]
    TierChange,
    /// |H| 超线情形：引导用户走署名形态确认（AI 不能代签）。
    ConfirmForm,
    /// 换系统（结构化建议；执行走系统清单变更通道，本卡不自动执行）。
    ReplaceSystem,
    /// 添加接口边 / 改名词绑定（结构化建议；执行走 refs 变更通道）。
    AddBinding,
}

impl FixActionKind {
    pub fn label(&self) -> &'static str {
        match self {
            Self::TierChange => "改档位",
            Self::ConfirmForm => "署名形态确认",
            Self::ReplaceSystem => "换系统",
            Self::AddBinding => "改绑定",
        }
    }
}

/// 一个结构化修复选项。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct CompositionFixOption {
    /// 选项 id（提案内唯一，执行入口按它定位）。
    pub option_id: String,
    pub kind: FixActionKind,
    /// 目标实例（tier_change/replace_system/add_binding 必填并校验存在）。
    pub instance_id: String,
    /// tier_change 的目标档位（校验在该实例模块阶梯内）。
    pub to_tier: String,
    /// add_binding 的名词与目标（呈现用；replace_system 时 to_tier 为空、
    /// detail 里说明替换方向）。
    pub binding_noun: String,
    pub binding_target: String,
    /// 人话说明：这个选项做什么、为什么能消除违例、代价是什么。
    pub detail: String,
}

/// 组合访谈提案：违例的人话解释 + 修复选项清单。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct CompositionFixProposal {
    /// 人话解释（AI 把传导链/连通缺陷讲清楚——违例码到设计语言的翻译）。
    pub explanation: String,
    pub options: Vec<CompositionFixOption>,
}

/// 组合访谈提案入口（纯函数）：评估结果 → AI 解释 + 修复选项。
///
/// 评估无 blocks、无 advices、也不要求形态确认时直接 Err（没有可访谈的违例，
/// 空谈会产生幻觉选项）；AI 输出越界（发明实例/档位/种类）即 Err。
pub fn propose_composition_fix(
    space: &DesignSpace,
    modules: &BTreeMap<String, SystemModule>,
    assessment: &CompositionAssessment,
    provider: &dyn AiProvider,
) -> Adm4Result<CompositionFixProposal> {
    let report = &assessment.report;
    if report.blocks.is_empty()
        && report.advices.is_empty()
        && !report.form_confirmation_required
        && assessment.missing_tiers.is_empty()
    {
        return Err(Adm4Error::conflict(
            "当前组合零违例零提示，无可访谈内容（组合访谈只在有 blocks/advices/确认待办时进行）",
        ));
    }
    let request = build_fix_request(space, modules, assessment);
    let response = provider.invoke(&request)?;
    parse_fix_proposal(space, modules, &response.text)
}

fn build_fix_request(
    space: &DesignSpace,
    modules: &BTreeMap<String, SystemModule>,
    assessment: &CompositionAssessment,
) -> AiRequest {
    let report = &assessment.report;
    let mut findings = Vec::new();
    for finding in &report.blocks {
        findings.push(format!("[BLOCK] {}：{}", finding.subject, finding.detail));
    }
    for finding in &report.advices {
        findings.push(format!("[ADVICE] {}：{}", finding.subject, finding.detail));
    }
    for missing in &assessment.missing_tiers {
        findings.push(format!(
            "[缺档] {}：{}",
            missing.instance_id, missing.detail
        ));
    }
    if report.form_confirmation_required {
        findings.push(format!(
            "[CONFIRM-REQUIRED] |H|={} 超参考线，需一次性署名形态确认（重核：{}）",
            report.h_set.len(),
            report.h_set.join("、")
        ));
    }
    let instances = instance_tier_orders(space, modules)
        .into_iter()
        .map(|(instance_id, module_id, tiers)| {
            format!(
                "- 实例 {instance_id}（模块 {module_id}）档位阶梯：{}",
                tiers.join(" < ")
            )
        })
        .collect::<Vec<_>>()
        .join("\n");
    AiRequest {
        purpose: PURPOSE_COMPOSITION.into(),
        system_prompt: "你是游戏设计访谈助手，做组合校验的违例解释与修复建议。\
             先用设计语言（人话）把违例讲清楚：传导链哪里断了、哪个重核游离、\
             为什么这是结构缺陷。再给结构化修复选项，kind 只能是：\
             tier_change（升/降某实例档位，to_tier 必须在该实例阶梯内）、\
             confirm_form（|H| 超线时引导用户署名确认——你不能代签）、\
             replace_system（换系统方向建议）、add_binding（改绑定/加接口边建议）。\
             实例 id 与档位 id 只能用给出的，不得发明。\
             输出 JSON：{\"explanation\":..., \"options\":[{\"option_id\":..., \
             \"kind\":..., \"instance_id\":..., \"to_tier\":..., \"binding_noun\":..., \
             \"binding_target\":..., \"detail\":...}]}。\
             你提出的是建议，最终决定权在用户。"
            .into(),
        user_prompt: format!(
            "当前组合的违例与提示：\n{}\n\n组合内实例与档位阶梯：\n{instances}",
            findings.join("\n")
        ),
        expect_json: true,
    }
}

fn parse_fix_proposal(
    space: &DesignSpace,
    modules: &BTreeMap<String, SystemModule>,
    text: &str,
) -> Adm4Result<CompositionFixProposal> {
    let value: serde_json::Value = serde_json::from_str(text.trim()).map_err(|error| {
        Adm4Error::validation(format!("组合修复提案不是合法 JSON：{error}；原文：{text}"))
    })?;
    let proposal: CompositionFixProposal = serde_json::from_value(value).map_err(|error| {
        Adm4Error::validation(format!(
            "组合修复提案 JSON 结构不符（kind 只接受 tier_change/confirm_form/\
             replace_system/add_binding）：{error}"
        ))
    })?;
    if proposal.explanation.trim().is_empty() {
        return Err(Adm4Error::validation(
            "组合修复提案缺少 explanation（人话解释违例是组合访谈的目的，不可留白）",
        ));
    }
    if proposal.options.is_empty() {
        return Err(Adm4Error::validation(
            "组合修复提案不含任何选项（无选项的解释请走 compose report，访谈提案必须可操作）",
        ));
    }
    let tier_orders: BTreeMap<String, Vec<String>> = instance_tier_orders(space, modules)
        .into_iter()
        .map(|(instance_id, _, tiers)| (instance_id, tiers))
        .collect();
    let mut seen_ids = std::collections::BTreeSet::new();
    for option in &proposal.options {
        if option.option_id.trim().is_empty() || !seen_ids.insert(option.option_id.as_str()) {
            return Err(Adm4Error::validation(format!(
                "组合修复选项 id 非法或重复：{:?}（执行入口按 option_id 定位，必须唯一非空）",
                option.option_id
            )));
        }
        match option.kind {
            FixActionKind::TierChange => {
                let Some(tiers) = tier_orders.get(&option.instance_id) else {
                    return Err(Adm4Error::validation(format!(
                        "修复选项 {} 指向的实例 {} 不在组合内（发明实例 id 被拒绝）",
                        option.option_id, option.instance_id
                    )));
                };
                if !tiers.iter().any(|tier| tier == &option.to_tier) {
                    return Err(Adm4Error::validation(format!(
                        "修复选项 {} 的目标档位 {} 不在实例 {} 的阶梯内（发明档位 id 被拒绝；\
                         可选：{}）",
                        option.option_id,
                        option.to_tier,
                        option.instance_id,
                        tiers.join("、")
                    )));
                }
            }
            FixActionKind::ConfirmForm => {
                // 无需实例/档位；执行入口强制 --signer（AI 不能代签在服务层复核）。
            }
            FixActionKind::ReplaceSystem | FixActionKind::AddBinding => {
                if !tier_orders.contains_key(&option.instance_id) {
                    return Err(Adm4Error::validation(format!(
                        "修复选项 {} 指向的实例 {} 不在组合内（发明实例 id 被拒绝）",
                        option.option_id, option.instance_id
                    )));
                }
            }
        }
        if option.detail.trim().is_empty() {
            return Err(Adm4Error::validation(format!(
                "修复选项 {} 缺少 detail 说明（用户要凭它做选择，不可留白）",
                option.option_id
            )));
        }
    }
    Ok(proposal)
}

/// 内部支撑：从空间 + 模块表导出（实例, 模块, 档位序）清单。
///
/// 单独成模块是为了让本文件保持「请求构造/解析校验」的单一职责，
/// 同时给 lib 内其它调用方（services 侧执行校验）复用同一份口径。
pub(crate) mod compose_fix_support {
    use adm4_decision::system_module::SystemModule;
    use adm4_space::DesignSpace;
    use std::collections::BTreeMap;

    /// 组合内每个实例的 (instance_id, module_id, 档位 id 有序表)。
    /// 引用了表内没有的模块的实例跳过（评估层已 fail-closed 点名，这里是呈现辅助）。
    pub(crate) fn instance_tier_orders(
        space: &DesignSpace,
        modules: &BTreeMap<String, SystemModule>,
    ) -> Vec<(String, String, Vec<String>)> {
        space
            .pack
            .system_refs
            .iter()
            .filter_map(|reference| {
                modules.get(&reference.module_id).map(|module| {
                    (
                        reference.instance_id.clone(),
                        reference.module_id.clone(),
                        module
                            .heaviness
                            .tiers
                            .iter()
                            .map(|tier| tier.id.clone())
                            .collect(),
                    )
                })
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::compose::assess_composition;
    use adm4_ai::ScriptedProvider;
    use adm4_decision::system_module::{
        CoreLink, FiveAxisRating, HeavinessLadder, HeavinessTier, Induction, InductionTarget,
        MdaMapping, NounDecl, NounKind, SystemInterface,
    };
    use adm4_decision::{
        DecisionGraph, DesignOrganization, ParameterValues, Provenance, Selection,
    };
    use adm4_space::{GenrePack, SystemRef};

    fn tier(id: &str, weight: u8, inductions: Vec<Induction>) -> HeavinessTier {
        HeavinessTier {
            id: id.into(),
            label_zh: id.into(),
            rating: FiveAxisRating {
                m: weight,
                d: weight,
                c: weight,
                p: weight,
                o: weight,
            },
            p_floor: 0,
            interface_floor: 0,
            activates: Vec::new(),
            inductions,
            summary: String::new(),
        }
    }

    /// 生产者模块：重档传导要求 sys.sink ≥ deep。
    fn producer_module() -> SystemModule {
        SystemModule {
            module_id: "sys.producer".into(),
            semver: "1.0.0".into(),
            label_zh: "生产".into(),
            summary: String::new(),
            nouns: vec![NounDecl {
                id: "output".into(),
                kind: NounKind::Resource,
                label_zh: "产出".into(),
                summary: String::new(),
            }],
            interface: SystemInterface {
                provides: vec!["output".into()],
                consumes: Vec::new(),
                modifies: Vec::new(),
            },
            mda: MdaMapping::default(),
            heaviness: HeavinessLadder {
                tiers: vec![
                    tier("light", 1, Vec::new()),
                    tier(
                        "heavy",
                        2,
                        vec![Induction {
                            when_tier: "heavy".into(),
                            target: InductionTarget::Module("sys.sink".into()),
                            min_tier: "deep".into(),
                            reason: "重档产出必须有深度承接".into(),
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

    /// 承接模块：shallow < deep 两档。
    fn sink_module() -> SystemModule {
        SystemModule {
            module_id: "sys.sink".into(),
            semver: "1.0.0".into(),
            label_zh: "承接".into(),
            summary: String::new(),
            nouns: vec![NounDecl {
                id: "slot".into(),
                kind: NounKind::Resource,
                label_zh: "槽".into(),
                summary: String::new(),
            }],
            interface: SystemInterface {
                provides: vec!["slot".into()],
                consumes: vec!["sys.producer.output".into()],
                modifies: Vec::new(),
            },
            mda: MdaMapping::default(),
            heaviness: HeavinessLadder {
                tiers: vec![tier("shallow", 1, Vec::new()), tier("deep", 2, Vec::new())],
            },
            decision_points: Vec::new(),
            cardinality_expectations: BTreeMap::new(),
            consistency_rules: Vec::new(),
            skin_fields: Vec::new(),
        }
    }

    fn modules() -> BTreeMap<String, SystemModule> {
        [
            ("sys.producer".to_string(), producer_module()),
            ("sys.sink".to_string(), sink_module()),
        ]
        .into_iter()
        .collect()
    }

    fn space() -> DesignSpace {
        DesignSpace {
            universal_version: "test".into(),
            pack: GenrePack {
                pack_id: "fix_test".into(),
                pack_version: "0.1.0".into(),
                display_name: "组合访谈测试包".into(),
                reference_games: vec!["虚构甲".into(), "虚构乙".into(), "虚构丙".into()],
                profile_points: Vec::new(),
                cardinality_expectations: Default::default(),
                consistency_rules: Vec::new(),
                nodes: Vec::new(),
                decision_points: Vec::new(),
                system_refs: vec![
                    SystemRef {
                        instance_id: "producer_main".into(),
                        module_id: "sys.producer".into(),
                        version_req: String::new(),
                        allowed_tiers: Vec::new(),
                        noun_bindings: BTreeMap::new(),
                        core_link: CoreLink::Core,
                    },
                    SystemRef {
                        instance_id: "sink_main".into(),
                        module_id: "sys.sink".into(),
                        version_req: String::new(),
                        allowed_tiers: Vec::new(),
                        noun_bindings: [(
                            "sys.producer.output".to_string(),
                            "producer_main.output".to_string(),
                        )]
                        .into_iter()
                        .collect(),
                        core_link: CoreLink::Weak,
                    },
                ],
                core_nouns: Vec::new(),
            },
            graph: DecisionGraph::new(Vec::new()).expect("空图应可构造"),
            organization: DesignOrganization::new(Vec::new(), Vec::new()),
            system_instances: Vec::new(),
        }
    }

    fn select(selections: &mut BTreeMap<String, Selection>, decision_id: &str, option: &str) {
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

    /// 造一个 V2 违例评估：producer heavy + sink shallow（传导要求 deep）。
    fn violating_assessment() -> CompositionAssessment {
        let mut selections = BTreeMap::new();
        select(&mut selections, "producer_main.tier", "heavy");
        select(&mut selections, "sink_main.tier", "shallow");
        assess_composition(&space(), &selections, &modules(), None, &[])
            .unwrap()
            .unwrap()
    }

    fn scripted(response: &str) -> ScriptedProvider {
        let provider = ScriptedProvider::new();
        provider.script(PURPOSE_COMPOSITION, vec![response.to_string()]);
        provider
    }

    /// 正常路径：违例进提示词，AI 给升档选项，解析通过。
    #[test]
    fn proposes_fix_options_for_v2_violation() {
        let assessment = violating_assessment();
        assert!(!assessment.report.blocks.is_empty(), "夹具必须真有违例");
        let provider = scripted(
            r#"{"explanation":"生产系统开了重档，产出洪流需要深度承接，但承接系统只有浅档——传导链在 sink_main 断了。","options":[{"option_id":"upgrade_sink","kind":"tier_change","instance_id":"sink_main","to_tier":"deep","detail":"把承接升到 deep 档，直接满足传导要求；代价是承接系统复杂度上升。"},{"option_id":"downgrade_producer","kind":"tier_change","instance_id":"producer_main","to_tier":"light","detail":"把生产降回轻档，传导要求消失；代价是产出深度变浅。"}]}"#,
        );
        let proposal =
            propose_composition_fix(&space(), &modules(), &assessment, &provider).unwrap();
        assert!(proposal.explanation.contains("传导链"));
        assert_eq!(proposal.options.len(), 2);
        assert_eq!(proposal.options[0].kind, FixActionKind::TierChange);
        // 违例文本进了提示词（AI 有据可讲）。
        let calls = provider.calls();
        assert_eq!(calls[0].purpose, PURPOSE_COMPOSITION);
        assert!(calls[0].user_prompt.contains("sys.sink"), "违例应进提示词");
        assert!(
            calls[0].user_prompt.contains("shallow < deep"),
            "档位阶梯应进提示词：{}",
            calls[0].user_prompt
        );
    }

    /// 验收 5 负测试：发明实例 id / 档位 id → Err。
    #[test]
    fn invented_instance_or_tier_is_rejected() {
        let assessment = violating_assessment();
        let provider = scripted(
            r#"{"explanation":"解释","options":[{"option_id":"x","kind":"tier_change","instance_id":"ghost_instance","to_tier":"deep","detail":"d"}]}"#,
        );
        let error =
            propose_composition_fix(&space(), &modules(), &assessment, &provider).unwrap_err();
        assert!(
            error.message.contains("ghost_instance"),
            "{}",
            error.message
        );

        let provider = scripted(
            r#"{"explanation":"解释","options":[{"option_id":"x","kind":"tier_change","instance_id":"sink_main","to_tier":"bottomless","detail":"d"}]}"#,
        );
        let error =
            propose_composition_fix(&space(), &modules(), &assessment, &provider).unwrap_err();
        assert!(error.message.contains("bottomless"), "{}", error.message);

        // 发明选项种类：serde 反序列化直接拒。
        let provider = scripted(
            r#"{"explanation":"解释","options":[{"option_id":"x","kind":"wave_hands","instance_id":"sink_main","detail":"d"}]}"#,
        );
        let error =
            propose_composition_fix(&space(), &modules(), &assessment, &provider).unwrap_err();
        assert!(error.message.contains("kind"), "{}", error.message);
    }

    /// 零违例时拒绝空谈。
    #[test]
    fn clean_composition_refuses_interview() {
        let mut selections = BTreeMap::new();
        select(&mut selections, "producer_main.tier", "light");
        select(&mut selections, "sink_main.tier", "shallow");
        let assessment = assess_composition(&space(), &selections, &modules(), None, &[])
            .unwrap()
            .unwrap();
        // light 档 W5 不入 H、无传导：零违例零提示。
        assert!(assessment.report.blocks.is_empty());
        let provider = scripted(r#"{"explanation":"x","options":[]}"#);
        let error =
            propose_composition_fix(&space(), &modules(), &assessment, &provider).unwrap_err();
        assert!(error.message.contains("无可访谈"), "{}", error.message);
    }

    /// confirm_form 选项不要求实例/档位（|H| 情形的指路选项）。
    #[test]
    fn confirm_form_option_passes_without_instance() {
        let assessment = violating_assessment();
        let provider = scripted(
            r#"{"explanation":"解释","options":[{"option_id":"sign","kind":"confirm_form","detail":"|H| 超线：请署名确认超大玩法形态（compose confirm-form --signer 你的名字）。AI 不能代签。"}]}"#,
        );
        let proposal =
            propose_composition_fix(&space(), &modules(), &assessment, &provider).unwrap();
        assert_eq!(proposal.options[0].kind, FixActionKind::ConfirmForm);
    }

    /// explanation/detail 留白 → Err（访谈产物必须可读可选）。
    #[test]
    fn blank_explanation_or_detail_is_rejected() {
        let assessment = violating_assessment();
        let provider = scripted(
            r#"{"explanation":"","options":[{"option_id":"x","kind":"confirm_form","detail":"d"}]}"#,
        );
        let error =
            propose_composition_fix(&space(), &modules(), &assessment, &provider).unwrap_err();
        assert!(error.message.contains("explanation"), "{}", error.message);

        let provider = scripted(
            r#"{"explanation":"讲清楚了","options":[{"option_id":"x","kind":"confirm_form","detail":"  "}]}"#,
        );
        let error =
            propose_composition_fix(&space(), &modules(), &assessment, &provider).unwrap_err();
        assert!(error.message.contains("detail"), "{}", error.message);
    }
}
