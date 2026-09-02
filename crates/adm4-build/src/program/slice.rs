//! 可玩切片抽取（册 09 §2）：从冻结 GameSpec + 程序线契约抽**最小可玩目标**，不塞完整商业范围。
//!
//! **只映射不发明**（铁律①）：每个切片字段都对应一条已有事实，抽取规则刻意简单——
//!
//! | 切片字段 | 事实来源 | 缺失时 |
//! |----------|----------|--------|
//! | `core_loop` | `GameSpec.intent.experience_promise` | `Err(Blocked)` 点名 |
//! | `primary_input` | 程序线契约里 `method == Command` 的**唯一**能力契约（id + inputs） | 0 条 `Err(Blocked)`；多条 `Err` 点名候选 |
//! | `player_feedback` | 程序线契约里 `method == Event` 的能力契约与事件 id | 登记 `CoverageGap`（Warning） |
//! | `scene` | `GameSpec.content` 里的**唯一**内容条目（L6 关卡/波次） | 0 条 `Err(Blocked)`；多条 `Err` 点名候选 |
//! | `success_or_fail_state` | `GameSpec.acceptance` 里的**唯一** GWT 场景的 `then` | 0 条 `Err(Blocked)`；多条 `Err` 点名候选 |
//! | `excluded_scope` | 主操作归属系统以外的全部程序系统 id | 允许为空 |
//!
//! 为什么"多于一个候选就停"而不是挑第一个：挑选就是设计决策，本模块无权替设计者做（R2）。
//! 为什么不做复杂推断：推断出来的事实无法指回一条 `SpecRef`，会在下游被当成发明（R4）。
//!
//! **确定性**：无 AI、无随机、无时钟；同一输入两次抽取的 serde 输出逐字节相等。

use crate::governance::program_line::{ContractMethod, ProgramContract};
use crate::governance::{CoverageGap, GapSeverity};
use adm4_contracts::SpecRef;
use adm4_foundation::{Adm4Error, Adm4Result};
use adm4_spec::GameSpec;
use serde::{Deserialize, Serialize};

/// 最小可玩切片（`playable_slice.json`）。
///
/// 字段与册 09 §2 的 JSON 形态一一对应；`anchors` 是本切片全部事实的真源锚点集合，
/// 下游叙述性产物（清单/durable docs）只能引用这里的事实。
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct PlayableSlice {
    /// 核心循环：来自项目意图的体验承诺。
    pub core_loop: String,
    /// 主操作：唯一命令型能力契约的 id 与输入数据结构。
    pub primary_input: Vec<String>,
    /// 玩家反馈：事件型能力契约与程序事件的 id。
    pub player_feedback: Vec<String>,
    /// 主场景：唯一内容条目的 id。
    pub scene: String,
    /// 成败状态：唯一验收场景的 `then` 断言。
    pub success_or_fail_state: String,
    /// 明确排除的范围：本切片不碰的程序系统 id。
    pub excluded_scope: Vec<String>,
    /// 真源锚点（R4）。
    pub anchors: Vec<SpecRef>,
}

impl PlayableSlice {
    /// 切片的硬性检查：核心循环 / 主操作 / 主场景 / 成败状态四项必须都有，且至少一条锚点。
    ///
    /// 抽取函数与旧档加载共用这一套检查，保证"抽出来的"与"读回来的"切片受同样约束。
    pub fn validate(&self) -> Adm4Result<()> {
        if self.core_loop.trim().is_empty() {
            return Err(Adm4Error::blocked(
                "可玩切片缺核心循环（core_loop）：GameSpec.intent.experience_promise 为空",
            ));
        }
        if self.primary_input.iter().all(|item| item.trim().is_empty()) {
            return Err(Adm4Error::blocked(
                "可玩切片缺主操作（primary_input）：程序线契约没有命令型能力契约",
            ));
        }
        if self.scene.trim().is_empty() {
            return Err(Adm4Error::blocked(
                "可玩切片缺主场景（scene）：GameSpec.content 没有内容条目",
            ));
        }
        if self.success_or_fail_state.trim().is_empty() {
            return Err(Adm4Error::blocked(
                "可玩切片缺成败状态（success_or_fail_state）：GameSpec.acceptance 没有验收场景",
            ));
        }
        if self.anchors.iter().all(|anchor| anchor.0.trim().is_empty()) {
            return Err(Adm4Error::validation(
                "可玩切片没有任何真源锚点：叙述性产物无锚即发明（R4）",
            ));
        }
        Ok(())
    }
}

/// 风险类别（册 09 §2 的封闭集合）。
///
/// `Unspecified` 是旧档/漏填的落点，不是合法类别：[`RiskSlicePlan::validate`] 见到即报错。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RiskKind {
    #[default]
    Unspecified,
    /// 手感
    Feel,
    /// 画面
    Visual,
    /// 技术
    Technical,
    /// 资产
    Asset,
}

impl RiskKind {
    /// 中文标签（清单与 durable docs 用）。
    pub fn label(self) -> &'static str {
        match self {
            Self::Unspecified => "未指定",
            Self::Feel => "手感",
            Self::Visual => "画面",
            Self::Technical => "技术",
            Self::Asset => "资产",
        }
    }
}

/// 风险验证方式（册 09 §2 的封闭集合）。每个风险必须有独立验证方式，`Unspecified` 不算。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VerifyMethod {
    #[default]
    Unspecified,
    /// 命令（构建/运行/检查命令的退出码与日志）
    Command,
    /// 截图
    Screenshot,
    /// 视频
    Video,
}

impl VerifyMethod {
    /// 中文标签（清单与 durable docs 用）。
    pub fn label(self) -> &'static str {
        match self {
            Self::Unspecified => "未指定",
            Self::Command => "命令",
            Self::Screenshot => "截图",
            Self::Video => "视频",
        }
    }
}

/// 一条风险切片：要验什么、怎么验、事实从哪来。
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct RiskSliceItem {
    pub risk: RiskKind,
    pub description: String,
    pub verify_by: VerifyMethod,
    pub anchors: Vec<SpecRef>,
}

/// 风险切片计划（`risk_slice_plan`）。
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct RiskSlicePlan {
    pub items: Vec<RiskSliceItem>,
}

impl RiskSlicePlan {
    /// 每条风险必须有类别、描述与验证方式——没有验证方式的风险等于没登记（R7：不许口头声称已验）。
    pub fn validate(&self) -> Adm4Result<()> {
        for (position, item) in self.items.iter().enumerate() {
            if item.risk == RiskKind::Unspecified {
                return Err(Adm4Error::validation(format!(
                    "风险切片第 {} 条未指定风险类别（手感/画面/技术/资产）",
                    position + 1
                )));
            }
            if item.description.trim().is_empty() {
                return Err(Adm4Error::validation(format!(
                    "风险切片第 {} 条（{}）没有描述",
                    position + 1,
                    item.risk.label()
                )));
            }
            if item.verify_by == VerifyMethod::Unspecified {
                return Err(Adm4Error::validation(format!(
                    "风险切片第 {} 条（{}）没有验证方式（命令/截图/视频）",
                    position + 1,
                    item.risk.label()
                )));
            }
        }
        Ok(())
    }
}

/// 抽取结果：切片 + 风险计划 + 抽不出来但不致命的事实缺口。
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct SliceExtraction {
    pub slice: PlayableSlice,
    pub risk_plan: RiskSlicePlan,
    pub gaps: Vec<CoverageGap>,
}

/// 从候选列表取唯一一条：0 条按缺事实阻塞，多条点名全部候选交人工裁决。
fn exactly_one<'a, T>(
    candidates: &'a [T],
    what: &str,
    missing_fact: &str,
    describe: impl Fn(&T) -> String,
) -> Adm4Result<&'a T> {
    match candidates {
        [] => Err(Adm4Error::blocked(format!(
            "可玩切片抽不出{what}：{missing_fact}"
        ))),
        [single] => Ok(single),
        many => Err(Adm4Error::blocked(format!(
            "可玩切片要求不超过 1 个{what}，现有 {} 个候选：{}（回设计侧收敛，本模块不替设计者挑）",
            many.len(),
            many.iter().map(describe).collect::<Vec<_>>().join(", ")
        ))),
    }
}

fn push_anchor(anchors: &mut Vec<SpecRef>, anchor: SpecRef) {
    if !anchors.contains(&anchor) {
        anchors.push(anchor);
    }
}

/// 从 GameSpec + 程序线契约确定性抽取最小可玩切片与风险计划。
///
/// 规则见模块头的映射表；抽不出核心循环/主操作/主场景/成败状态任一项即 `Err(Blocked)`，
/// 候选多于一个即 `Err` 点名候选。结果最后再过一遍 [`PlayableSlice::validate`] 与
/// [`RiskSlicePlan::validate`]，保证抽取器与校验器口径一致。
pub fn extract_playable_slice(
    spec: &GameSpec,
    program: &ProgramContract,
) -> Adm4Result<SliceExtraction> {
    let mut anchors: Vec<SpecRef> = Vec::new();
    let mut gaps: Vec<CoverageGap> = Vec::new();

    let core_loop = spec.intent.experience_promise.trim();
    if core_loop.is_empty() {
        return Err(Adm4Error::blocked(
            "可玩切片抽不出核心循环：GameSpec.intent.experience_promise 为空（回设计工作台补体验承诺）",
        ));
    }
    push_anchor(&mut anchors, SpecRef::new("intent"));

    let commands: Vec<_> = program
        .capabilities
        .iter()
        .filter(|capability| capability.method == ContractMethod::Command)
        .collect();
    let primary = exactly_one(
        &commands,
        "主操作",
        "程序线契约没有 method == command 的能力契约",
        |capability| capability.capability_id.clone(),
    )?;
    let mut primary_input = vec![primary.capability_id.clone()];
    primary_input.extend(primary.inputs.iter().cloned());
    for anchor in &primary.source_refs {
        push_anchor(&mut anchors, anchor.clone());
    }

    let mut player_feedback: Vec<String> = Vec::new();
    for capability in &program.capabilities {
        if capability.method == ContractMethod::Event {
            player_feedback.push(capability.capability_id.clone());
            for anchor in &capability.source_refs {
                push_anchor(&mut anchors, anchor.clone());
            }
        }
    }
    for event in &program.events {
        player_feedback.push(event.event_id.clone());
        for anchor in &event.source_refs {
            push_anchor(&mut anchors, anchor.clone());
        }
    }
    if player_feedback.is_empty() {
        gaps.push(CoverageGap {
            gap_id: "gap_slice_player_feedback".to_string(),
            missing_fact: "程序线契约没有事件型能力契约或程序事件：玩家反馈无事实可映射"
                .to_string(),
            required_by: "playable_slice/player_feedback".to_string(),
            severity: GapSeverity::Warning,
        });
    }

    let scene_entry = exactly_one(
        &spec.content,
        "主场景",
        "GameSpec.content 没有任何内容条目（L6 关卡/波次）",
        |content| format!("{}({})", content.id, content.content_kind),
    )?;
    let scene = scene_entry.id.clone();
    push_anchor(&mut anchors, SpecRef::new(format!("content/{scene}")));

    let scenario = exactly_one(
        &spec.acceptance,
        "成败状态",
        "GameSpec.acceptance 没有任何 GWT 验收场景",
        |scenario| scenario.id.clone(),
    )?;
    let success_or_fail_state = scenario.then.join("；");
    if success_or_fail_state.trim().is_empty() {
        return Err(Adm4Error::blocked(format!(
            "可玩切片抽不出成败状态：验收场景 {} 的 then 为空",
            scenario.id
        )));
    }
    push_anchor(
        &mut anchors,
        SpecRef::new(format!("acceptance/{}", scenario.id)),
    );

    let excluded_scope: Vec<String> = program
        .systems
        .iter()
        .filter(|system| system.system_id != primary.source_system)
        .map(|system| system.system_id.clone())
        .collect();
    for system in &program.systems {
        if system.system_id == primary.source_system {
            for anchor in &system.source_refs {
                push_anchor(&mut anchors, anchor.clone());
            }
        }
    }

    let slice = PlayableSlice {
        core_loop: core_loop.to_string(),
        primary_input,
        player_feedback,
        scene,
        success_or_fail_state,
        excluded_scope,
        anchors,
    };

    let mut items = vec![
        RiskSliceItem {
            risk: RiskKind::Feel,
            description: format!("主操作 {} 的手感", primary.capability_id),
            verify_by: VerifyMethod::Video,
            anchors: primary.source_refs.clone(),
        },
        RiskSliceItem {
            risk: RiskKind::Technical,
            description: format!("主场景 {} 可构建、可运行、可进入成败状态", scene_entry.id),
            verify_by: VerifyMethod::Command,
            anchors: vec![
                SpecRef::new(format!("content/{}", scene_entry.id)),
                SpecRef::new(format!("acceptance/{}", scenario.id)),
            ],
        },
    ];
    if program.asset_dependencies.is_empty() {
        gaps.push(CoverageGap {
            gap_id: "gap_slice_asset_dependencies".to_string(),
            missing_fact: "程序线契约没有资产依赖：画面/资产风险无事实可登记".to_string(),
            required_by: "risk_slice_plan".to_string(),
            severity: GapSeverity::Warning,
        });
    } else {
        let asset_ids: Vec<String> = program
            .asset_dependencies
            .iter()
            .map(|dependency| dependency.asset_id.clone())
            .collect();
        let mut asset_anchors: Vec<SpecRef> = Vec::new();
        for dependency in &program.asset_dependencies {
            for anchor in &dependency.source_refs {
                push_anchor(&mut asset_anchors, anchor.clone());
            }
        }
        items.push(RiskSliceItem {
            risk: RiskKind::Visual,
            description: format!("资产 {} 在主场景内可见且可辨", asset_ids.join(", ")),
            verify_by: VerifyMethod::Screenshot,
            anchors: asset_anchors.clone(),
        });
        items.push(RiskSliceItem {
            risk: RiskKind::Asset,
            description: format!("资产 {} 按资产表路径就位", asset_ids.join(", ")),
            verify_by: VerifyMethod::Command,
            anchors: asset_anchors,
        });
    }
    let risk_plan = RiskSlicePlan { items };

    slice.validate()?;
    risk_plan.validate()?;
    Ok(SliceExtraction {
        slice,
        risk_plan,
        gaps,
    })
}

#[cfg(test)]
pub(crate) mod test_fixtures {
    //! 供 program 子模块共用的最小夹具：一个恰好满足「1 主操作 / 1 主场景 / 1 成败状态」的输入。

    use crate::governance::program_line::{
        AuthorityAssignment, CapabilityContract, ContractMethod, ProgramAssetDependency,
        ProgramContract, ProgramEntity, ProgramEvent, ProgramSystem,
    };
    use crate::governance::{ContractEnvelope, PROGRAM_LINE, SpecTriple};
    use adm4_contracts::SpecRef;
    use adm4_spec::{
        AcceptanceScenario, ContentSpec, EffectSpec, EntitySpec, GameSpec, MechanicSpec,
        ProjectIntent, SpecIdentity, SystemSpec, VisualForm,
    };

    pub(crate) fn spec() -> GameSpec {
        GameSpec {
            identity: SpecIdentity {
                schema_version: "4.0.0".into(),
                project_id: "demo".into(),
                frozen_hash: "sha256:frozen".into(),
            },
            intent: ProjectIntent {
                title: "守塔".into(),
                experience_promise: "放置守卫拦截来袭波次".into(),
                genre_structure: "塔防".into(),
                profile: Default::default(),
            },
            systems: vec![
                SystemSpec {
                    id: "combat_system".into(),
                    name: "战斗系统".into(),
                    purpose: "结算克制伤害".into(),
                    interfaces: Vec::new(),
                    design_notes: Vec::new(),
                },
                SystemSpec {
                    id: "economy_system".into(),
                    name: "经济系统".into(),
                    purpose: "资源结算".into(),
                    interfaces: Vec::new(),
                    design_notes: Vec::new(),
                },
            ],
            mechanics: vec![MechanicSpec {
                id: "place_guard".into(),
                system_id: "combat_system".into(),
                rule_text: "在格位放置守卫".into(),
                preconditions: Vec::new(),
                effects: vec![EffectSpec::SpawnEntity {
                    entity: "guard".into(),
                }],
                state_machine: None,
                design_notes: Vec::new(),
            }],
            entities: vec![EntitySpec {
                id: "guard".into(),
                name: "守卫".into(),
                visual_form: Some(VisualForm::Sprite2d),
                properties: Vec::new(),
            }],
            tables: Vec::new(),
            content: vec![ContentSpec {
                id: "wave_1".into(),
                content_kind: "wave_schedule".into(),
                data: serde_json::json!({"waves": 3}),
                design_notes: Vec::new(),
            }],
            graphs: Vec::new(),
            acceptance: vec![AcceptanceScenario {
                id: "acc_survive".into(),
                capability_id: "cap_place_guard".into(),
                given: vec!["守卫在场".into()],
                when: vec!["波次结束".into()],
                then: vec!["基地存活即胜利".into(), "基地血量归零即失败".into()],
                source_refs: vec![SpecRef::new("mechanics/place_guard")],
            }],
            source_map: Vec::new(),
        }
    }

    pub(crate) fn program() -> ProgramContract {
        ProgramContract {
            envelope: ContractEnvelope::new(PROGRAM_LINE, "2026-09-02T00:00:00Z", "sha256:frozen"),
            systems: vec![
                ProgramSystem {
                    system_id: "combat_system".into(),
                    name: "战斗系统".into(),
                    responsibility: "结算克制伤害".into(),
                    source_refs: vec![SpecRef::new("systems/combat_system")],
                },
                ProgramSystem {
                    system_id: "economy_system".into(),
                    name: "经济系统".into(),
                    responsibility: "资源结算".into(),
                    source_refs: vec![SpecRef::new("systems/economy_system")],
                },
            ],
            capabilities: vec![
                CapabilityContract {
                    capability_id: "cap_place_guard".into(),
                    source_system: "combat_system".into(),
                    target_system: "combat_system".into(),
                    method: ContractMethod::Command,
                    inputs: vec!["guard".into()],
                    outputs: Vec::new(),
                    errors: Vec::new(),
                    source_refs: vec![SpecRef::new("mechanics/place_guard")],
                },
                CapabilityContract {
                    capability_id: "cap_wave_cleared".into(),
                    source_system: "combat_system".into(),
                    target_system: "economy_system".into(),
                    method: ContractMethod::Event,
                    inputs: Vec::new(),
                    outputs: Vec::new(),
                    errors: Vec::new(),
                    source_refs: vec![SpecRef::new("mechanics/wave_cleared")],
                },
            ],
            entities: vec![ProgramEntity {
                entity_id: "guard".into(),
                entity_name: "守卫".into(),
                owner_system: "combat_system".into(),
                properties: Vec::new(),
                source_refs: vec![SpecRef::new("entities/guard")],
            }],
            events: vec![ProgramEvent {
                event_id: "wave_cleared".into(),
                capability_id: "cap_wave_cleared".into(),
                payload: Vec::new(),
                source_refs: vec![SpecRef::new("mechanics/wave_cleared")],
            }],
            authority: vec![AuthorityAssignment {
                authority_id: "auth_guard".into(),
                mutable_fact: "guard".into(),
                owner_system: "combat_system".into(),
                source_refs: vec![SpecRef::new("entities/guard")],
            }],
            acceptance: Vec::new(),
            asset_dependencies: vec![ProgramAssetDependency {
                dependency_id: "render.guard".into(),
                owner_system: "combat_system".into(),
                asset_id: "T_Guard".into(),
                required_spec: SpecTriple::default(),
                source_refs: vec![SpecRef::new("entities/guard")],
            }],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::test_fixtures::{program, spec};
    use super::*;
    use adm4_foundation::Adm4ErrorKind;
    use adm4_spec::{AcceptanceScenario, ContentSpec};

    /// 映射结果逐字段对得上事实来源；两个系统里只有主操作归属的那个进切片，另一个进排除范围。
    #[test]
    fn extraction_maps_existing_facts_only() {
        let extraction = extract_playable_slice(&spec(), &program()).expect("抽取");
        let slice = &extraction.slice;
        assert_eq!(slice.core_loop, "放置守卫拦截来袭波次");
        assert_eq!(slice.primary_input, vec!["cap_place_guard", "guard"]);
        assert_eq!(
            slice.player_feedback,
            vec!["cap_wave_cleared", "wave_cleared"]
        );
        assert_eq!(slice.scene, "wave_1");
        assert_eq!(
            slice.success_or_fail_state,
            "基地存活即胜利；基地血量归零即失败"
        );
        assert_eq!(slice.excluded_scope, vec!["economy_system"]);
        for wanted in [
            "intent",
            "mechanics/place_guard",
            "content/wave_1",
            "acceptance/acc_survive",
            "systems/combat_system",
        ] {
            assert!(
                slice.anchors.contains(&SpecRef::new(wanted)),
                "锚点应含 {wanted}：{:?}",
                slice.anchors
            );
        }
        assert!(extraction.gaps.is_empty(), "{:?}", extraction.gaps);
        // 四类风险齐备且各有验证方式。
        assert_eq!(extraction.risk_plan.items.len(), 4);
        assert!(extraction.risk_plan.validate().is_ok());
    }

    /// 验收 a：同一输入两次抽取的 serde 输出逐字节相等。
    #[test]
    fn extraction_is_byte_deterministic() {
        let first =
            serde_json::to_string(&extract_playable_slice(&spec(), &program()).expect("抽取"))
                .expect("序列化");
        let second =
            serde_json::to_string(&extract_playable_slice(&spec(), &program()).expect("抽取"))
                .expect("序列化");
        assert_eq!(first, second);
    }

    /// 验收 b：缺核心循环 / 主操作 / 成败状态任一 → Blocked，消息点名缺哪条。
    #[test]
    fn missing_core_facts_block_with_named_reason() {
        let mut no_loop = spec();
        no_loop.intent.experience_promise.clear();
        let error = extract_playable_slice(&no_loop, &program()).expect_err("缺核心循环");
        assert_eq!(error.kind, Adm4ErrorKind::Blocked);
        assert!(error.message.contains("核心循环"), "{}", error.message);

        let mut no_input = program();
        no_input
            .capabilities
            .retain(|c| c.method != ContractMethod::Command);
        let error = extract_playable_slice(&spec(), &no_input).expect_err("缺主操作");
        assert_eq!(error.kind, Adm4ErrorKind::Blocked);
        assert!(error.message.contains("主操作"), "{}", error.message);

        let mut no_state = spec();
        no_state.acceptance.clear();
        let error = extract_playable_slice(&no_state, &program()).expect_err("缺成败状态");
        assert_eq!(error.kind, Adm4ErrorKind::Blocked);
        assert!(error.message.contains("成败状态"), "{}", error.message);

        let mut no_scene = spec();
        no_scene.content.clear();
        let error = extract_playable_slice(&no_scene, &program()).expect_err("缺主场景");
        assert_eq!(error.kind, Adm4ErrorKind::Blocked);
        assert!(error.message.contains("主场景"), "{}", error.message);
    }

    /// 验收 b：多于 1 个主场景 / 主操作 / 成败状态 → Err 点名全部候选。
    #[test]
    fn surplus_candidates_are_named_not_picked() {
        let mut two_scenes = spec();
        two_scenes.content.push(ContentSpec {
            id: "wave_2".into(),
            content_kind: "wave_schedule".into(),
            data: serde_json::json!({}),
            design_notes: Vec::new(),
        });
        let error = extract_playable_slice(&two_scenes, &program()).expect_err("两个场景");
        assert!(
            error.message.contains("wave_1") && error.message.contains("wave_2"),
            "{}",
            error.message
        );

        let mut two_commands = program();
        let mut extra = two_commands.capabilities[0].clone();
        extra.capability_id = "cap_sell_guard".into();
        two_commands.capabilities.push(extra);
        let error = extract_playable_slice(&spec(), &two_commands).expect_err("两个主操作");
        assert_eq!(
            error.kind,
            Adm4ErrorKind::Blocked,
            "多候选属设计侧未收敛，按 R2 阻塞"
        );
        assert!(
            error.message.contains("cap_place_guard") && error.message.contains("cap_sell_guard"),
            "{}",
            error.message
        );

        let mut two_scenarios = spec();
        two_scenarios.acceptance.push(AcceptanceScenario {
            id: "acc_other".into(),
            capability_id: "cap_place_guard".into(),
            given: Vec::new(),
            when: Vec::new(),
            then: vec!["其它".into()],
            source_refs: Vec::new(),
        });
        let error = extract_playable_slice(&two_scenarios, &program()).expect_err("两个成败状态");
        assert!(
            error.message.contains("acc_survive") && error.message.contains("acc_other"),
            "{}",
            error.message
        );
    }

    /// 非致命缺口登记 gap 而不是报错：无事件型契约 → 反馈缺口；无资产依赖 → 画面/资产风险缺口。
    #[test]
    fn non_fatal_gaps_are_recorded() {
        let mut thin = program();
        thin.capabilities
            .retain(|c| c.method == ContractMethod::Command);
        thin.events.clear();
        thin.asset_dependencies.clear();
        let extraction = extract_playable_slice(&spec(), &thin).expect("抽取");
        assert!(extraction.slice.player_feedback.is_empty());
        let ids: Vec<&str> = extraction.gaps.iter().map(|g| g.gap_id.as_str()).collect();
        assert_eq!(
            ids,
            vec!["gap_slice_player_feedback", "gap_slice_asset_dependencies"]
        );
        assert!(
            extraction
                .gaps
                .iter()
                .all(|g| g.severity == GapSeverity::Warning)
        );
        assert_eq!(extraction.risk_plan.items.len(), 2);
    }

    /// 验收 c：validate 拦空值——切片四项与风险的 verify_by。
    #[test]
    fn validate_rejects_blank_fields_and_unspecified_verify() {
        let good = extract_playable_slice(&spec(), &program()).expect("抽取");
        assert!(good.slice.validate().is_ok());

        let mut slice = good.slice.clone();
        slice.scene = "  ".into();
        assert!(slice.validate().unwrap_err().message.contains("主场景"));
        let mut slice = good.slice.clone();
        slice.primary_input = vec![String::new()];
        assert!(slice.validate().unwrap_err().message.contains("主操作"));
        let mut slice = good.slice.clone();
        slice.anchors.clear();
        assert!(slice.validate().unwrap_err().message.contains("锚点"));

        let mut plan = good.risk_plan.clone();
        plan.items[0].verify_by = VerifyMethod::Unspecified;
        assert!(plan.validate().unwrap_err().message.contains("验证方式"));
        let mut plan = good.risk_plan.clone();
        plan.items[1].risk = RiskKind::Unspecified;
        assert!(plan.validate().unwrap_err().message.contains("风险类别"));
        assert!(PlayableSlice::default().validate().is_err());
    }

    /// 旧档兼容：缺键可读、枚举按 snake_case、缺 verify_by 落成 Unspecified 被 validate 拦下。
    #[test]
    fn slice_serde_round_trip_and_legacy_keys() {
        let extraction = extract_playable_slice(&spec(), &program()).expect("抽取");
        let json = serde_json::to_string_pretty(&extraction).expect("序列化");
        let back: SliceExtraction = serde_json::from_str(&json).expect("反序列化");
        assert_eq!(back, extraction);
        assert!(json.contains("\"verify_by\": \"video\""));

        let legacy: RiskSlicePlan =
            serde_json::from_str(r#"{"items":[{"risk":"feel","description":"手感"}]}"#)
                .expect("旧档可读");
        assert_eq!(legacy.items[0].verify_by, VerifyMethod::Unspecified);
        assert!(legacy.validate().is_err());
        let partial: PlayableSlice = serde_json::from_str(r#"{"scene":"wave_1"}"#).expect("旧档");
        assert!(partial.core_loop.is_empty());
    }
}
