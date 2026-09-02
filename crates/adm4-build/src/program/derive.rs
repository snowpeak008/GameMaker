//! P0 两条线派生器（册 07 §1-§4 的执行者）：GameSpec + C3/C4 契约 → 程序线契约 +
//! 美术线契约 + 资产表初版 + 对齐报告。
//!
//! **只映射不发明**（铁律①）：这里的每一条产出都能指回一条真源事实——
//! 系统来自 `spec.systems`，能力来自 C4 的机制投影，资产来自 C3 的视觉白名单产物，
//! 三要素来自实体声明与 C3 资产规格。真源里没有的事实**不编**：缺了就登记成
//! [`CoverageGap`]，由对齐层把「未知」判成待人工冲突（R2），而不是替设计者补一个默认值。
//!
//! **确定性**：无 AI、无随机、无时钟参与派生本体（`generated_at` 只进信封不参与内容），
//! 同一份输入永远产出同一份契约——e2e 直接以字节断言。

use crate::governance::alignment::{AlignmentReport, align};
use crate::governance::art_line::{ArtAsset, ArtContract, AssetCategory, VisualLanguage};
use crate::governance::asset_registry::{
    AssetLifecycleState, AssetRegistry, AssetRegistryEntry, StabilityLevel, lexical_variant,
};
use crate::governance::program_line::{
    AcceptanceBinding, AuthorityAssignment, CapabilityContract, ContractMethod,
    ProgramAssetDependency, ProgramContract, ProgramEntity, ProgramSystem,
};
use crate::governance::{
    ART_LINE, ContractEnvelope, CoverageGap, GapSeverity, PROGRAM_LINE, SpecTriple,
};
use adm4_contracts::SpecRef;
use adm4_foundation::{Adm4Error, Adm4Result};
use adm4_pipeline::{C3AssetSpec, CapabilitiesContract, ContentInventoryContract};
use adm4_spec::{EffectSpec, GameSpec, VisualForm};

/// P0 派生输入：唯一真源 + Phase 1 的两份设计文档契约。
///
/// C3/C4 是**读进来的产物**而不是重算的：Phase 2 重算一遍 C3 等于绕开 C3 的
/// 视觉白名单阻塞与基数人工门（R2/R6 都白设了）。
pub struct DeriveInput<'a> {
    pub spec: &'a GameSpec,
    pub content: &'a ContentInventoryContract,
    pub capabilities: &'a CapabilitiesContract,
    pub generated_at: &'a str,
}

/// P0 的四件核心产物（引擎工程种子不在此列——那是 G4 的活，见 [`super::super::runner`]）。
#[derive(Debug, Clone, PartialEq)]
pub struct TwoLineDerivation {
    pub program: ProgramContract,
    pub art: ArtContract,
    pub registry: AssetRegistry,
    pub alignment: AlignmentReport,
}

/// 资产 id：C3 的 `asset_{entity}` → 命名规范的 `UI_{entity}`（ui_only）或
/// `T_{entity}`（sprite2d 贴图）/ `SM_{entity}`（model3d 静态模型）。
///
/// 为什么不用 C3 的 id 原文：`asset_guard` 过不了命名权威的类型前缀校验
/// （册 07 §6 机制内置 `SM_/SK_/T_/M_/UI_/VFX_`），而命名权威是铁律②的单点锚定处——
/// 让 P0 派生出一批注定违规的名字，等于第一天就给自己造一堆废单。
/// 映射规则确定性且可逆（前缀 + 实体 id），追溯链不断。
fn asset_id_for(kind: &str, entity_id: &str) -> Adm4Result<String> {
    let subject = pascal_case(entity_id);
    match kind {
        "sprite2d" => Ok(format!("T_{subject}")),
        "model3d" => Ok(format!("SM_{subject}")),
        "ui" => Ok(format!("UI_{subject}")),
        other => Err(Adm4Error::validation(format!(
            "C3 资产 {entity_id} 的类型「{other}」不在映射表内：\
             不猜前缀（R2），请扩展映射规则"
        ))),
    }
}

/// `guard_tower` → `GuardTower`（命名骨架的 Subject 段）。
fn pascal_case(id: &str) -> String {
    id.split(['_', '-', ' '])
        .filter(|part| !part.is_empty())
        .map(|part| {
            let mut chars = part.chars();
            match chars.next() {
                Some(first) => first.to_uppercase().chain(chars).collect::<String>(),
                None => String::new(),
            }
        })
        .collect()
}

/// 运行时加载路径（相对隔离工程根）。资产表是命名权威：装配端（G4 P3）只认这里的路径。
fn runtime_path_for(asset_id: &str, format: &str) -> String {
    format!("GameAssets/{}.{format}", lexical_variant(asset_id))
}

/// 从 GameSpec + C3/C4 确定性派生两条线契约、资产表初版与对齐报告。
pub fn derive_two_lines(input: &DeriveInput<'_>) -> Adm4Result<TwoLineDerivation> {
    let frozen_hash = input.spec.identity.frozen_hash.as_str();
    if frozen_hash.trim().is_empty() {
        return Err(Adm4Error::validation(
            "GameSpec 没有冻结哈希：派生契约必须锚定一个确定的真源版本（D22）",
        ));
    }

    let program = derive_program_line(input, frozen_hash)?;
    let (art, registry) = derive_art_line(input, frozen_hash)?;

    // 自校验先于对齐：一份连自己都不自洽的契约没资格进合流层。
    program.validate()?;
    art.validate()?;
    registry.validate()?;

    let alignment = align(&program, &art, &registry);
    Ok(TwoLineDerivation {
        program,
        art,
        registry,
        alignment,
    })
}

/// 程序线：系统/能力/实体/事件/权属/验收绑定/资产依赖，全部从真源与 C4 映射。
fn derive_program_line(input: &DeriveInput<'_>, frozen_hash: &str) -> Adm4Result<ProgramContract> {
    let spec = input.spec;
    let mut envelope = ContractEnvelope::new(PROGRAM_LINE, input.generated_at, frozen_hash);
    envelope.derivation_policy = vec![
        "只映射不发明：系统/实体来自 GameSpec，能力契约来自 C4 机制投影".to_string(),
        "缺失事实登记 coverage_gaps，不用散文填补（铁律①）".to_string(),
    ];

    if spec.systems.is_empty() {
        return Err(Adm4Error::blocked(
            "GameSpec 没有任何系统：程序线无从划分（回设计工作台补玩法系统再冻结）",
        ));
    }
    let systems: Vec<ProgramSystem> = spec
        .systems
        .iter()
        .map(|system| ProgramSystem {
            system_id: system.id.clone(),
            name: system.name.clone(),
            responsibility: system.purpose.clone(),
            source_refs: vec![SpecRef::new(format!("systems/{}", system.id))],
        })
        .collect();

    // 机制归属系统：能力契约的源/目标系统按机制的 system_id 落；C4 只给了机制 → 能力
    // 的投影（id 形如 cap_{mechanic}），这里把它接回程序线的系统图。
    let mechanic_system = |mechanic_id: &str| -> Adm4Result<String> {
        spec.mechanics
            .iter()
            .find(|mechanic| mechanic.id == mechanic_id)
            .map(|mechanic| mechanic.system_id.clone())
            .ok_or_else(|| {
                Adm4Error::validation(format!(
                    "C4 能力指向的机制 {mechanic_id} 不在 GameSpec 内：设计文档契约集与真源不同版"
                ))
            })
    };

    let mut capabilities = Vec::new();
    let mut acceptance = Vec::new();
    for capability in &input.capabilities.capabilities {
        let Some(mechanic_id) = capability.id.strip_prefix("cap_") else {
            return Err(Adm4Error::validation(format!(
                "C4 能力 id「{}」不符合机制投影命名（cap_<机制>）：无法回接程序线",
                capability.id
            )));
        };
        let owner = mechanic_system(mechanic_id)?;
        // 机制效果里的 EmitSignal 是跨系统事件的证据；其余效果按命令（改状态）归类。
        let mechanic = spec
            .mechanics
            .iter()
            .find(|item| item.id == mechanic_id)
            .ok_or_else(|| {
                Adm4Error::validation(format!("机制 {mechanic_id} 消失于两次读取之间"))
            })?;
        let method = if mechanic
            .effects
            .iter()
            .any(|effect| matches!(effect, EffectSpec::EmitSignal { .. }))
        {
            ContractMethod::Event
        } else {
            ContractMethod::Command
        };
        capabilities.push(CapabilityContract {
            capability_id: capability.id.clone(),
            source_system: owner.clone(),
            target_system: owner.clone(),
            method,
            inputs: capability.data_structures.clone(),
            outputs: Vec::new(),
            errors: Vec::new(),
            source_refs: capability.source_refs.clone(),
        });
        for scenario in &capability.scenarios {
            acceptance.push(AcceptanceBinding {
                acceptance_id: format!("acc_{}", scenario.id),
                capability_id: capability.id.clone(),
                scenario_ref: SpecRef::new(format!("mechanics/{mechanic_id}")),
            });
        }
    }

    // 实体归属：GameSpec 的实体没有 owner 字段，按「哪个系统的机制效果改它」推；
    // 推不出唯一归属就落第一个声明的系统并登记 gap——不编造一个「实体管理系统」。
    let mut gaps: Vec<CoverageGap> = Vec::new();
    let mut entities = Vec::new();
    let mut authority = Vec::new();
    for entity in &spec.entities {
        let writers: Vec<&str> = spec
            .mechanics
            .iter()
            .filter(|mechanic| {
                mechanic.effects.iter().any(|effect| match effect {
                    EffectSpec::ModifyProperty { entity: target, .. }
                    | EffectSpec::SpawnEntity { entity: target }
                    | EffectSpec::DespawnEntity { entity: target } => target == &entity.id,
                    _ => false,
                })
            })
            .map(|mechanic| mechanic.system_id.as_str())
            .collect();
        let owner = match writers.first() {
            Some(first) => (*first).to_string(),
            None => {
                gaps.push(CoverageGap {
                    gap_id: format!("gap_entity_owner_{}", entity.id),
                    missing_fact: format!("实体 {} 没有任何机制写它，推不出归属系统", entity.id),
                    required_by: format!("entities/{}", entity.id),
                    severity: GapSeverity::Warning,
                });
                systems[0].system_id.clone()
            }
        };
        // 多个系统写同一实体是权属冲突的苗头：登记 gap（Warning），权属仍单点落第一个写者。
        let distinct: std::collections::BTreeSet<&str> = writers.iter().copied().collect();
        if distinct.len() > 1 {
            gaps.push(CoverageGap {
                gap_id: format!("gap_entity_multi_writer_{}", entity.id),
                missing_fact: format!(
                    "实体 {} 被多个系统的机制写入（{}）：真源未声明唯一写者",
                    entity.id,
                    distinct.into_iter().collect::<Vec<_>>().join(", ")
                ),
                required_by: format!("entities/{}", entity.id),
                severity: GapSeverity::Warning,
            });
        }
        entities.push(ProgramEntity {
            entity_id: entity.id.clone(),
            entity_name: entity.name.clone(),
            owner_system: owner.clone(),
            properties: entity
                .properties
                .iter()
                .map(|property| property.key.clone())
                .collect(),
            source_refs: vec![SpecRef::new(format!("entities/{}", entity.id))],
        });
        authority.push(AuthorityAssignment {
            authority_id: format!("auth_{}", entity.id),
            mutable_fact: entity.id.clone(),
            owner_system: owner,
            source_refs: vec![SpecRef::new(format!("entities/{}", entity.id))],
        });
    }

    // 程序侧资产依赖：C3 白名单产物（sprite2d/model3d）+ UI 清单 → 依赖条目。
    // 三要素在真源里只有格式可推（sprite2d→png）；帧数/尺寸真源没说，就如实留 None——
    // 对齐层会把它判成 UnknownSpec 冲突交人工（R2），这正是「缺规格要人补」的正确出口。
    let mut asset_dependencies = Vec::new();
    for asset in &input.content.assets {
        let owner = entity_owner(&entities, &asset.entity_id)
            .unwrap_or_else(|| systems[0].system_id.clone());
        asset_dependencies.push(ProgramAssetDependency {
            dependency_id: format!("render.{}", asset.entity_id),
            owner_system: owner,
            asset_id: asset_id_for(&asset.asset_kind, &asset.entity_id)?,
            required_spec: default_visual_triple(&asset.asset_kind),
            source_refs: vec![SpecRef::new(format!("entities/{}", asset.entity_id))],
        });
    }
    for ui in &input.content.ui_entries {
        let owner =
            entity_owner(&entities, &ui.entity_id).unwrap_or_else(|| systems[0].system_id.clone());
        asset_dependencies.push(ProgramAssetDependency {
            dependency_id: format!("ui.{}", ui.entity_id),
            owner_system: owner,
            asset_id: asset_id_for("ui", &ui.entity_id)?,
            required_spec: default_visual_triple("ui"),
            source_refs: vec![SpecRef::new(format!("entities/{}", ui.entity_id))],
        });
    }

    envelope.coverage_gaps = gaps;
    Ok(ProgramContract {
        envelope,
        systems,
        capabilities,
        entities,
        events: Vec::new(),
        authority,
        acceptance,
        asset_dependencies,
    })
}

fn entity_owner(entities: &[ProgramEntity], entity_id: &str) -> Option<String> {
    entities
        .iter()
        .find(|entity| entity.entity_id == entity_id)
        .map(|entity| entity.owner_system.clone())
}

/// 双线共用的默认三要素约定：**这是 P0 派生规则的一部分而不是编造的事实**——
/// 2D 产线统一 1 帧静帧 / 1024x1024 / png（与图像通道的生成尺寸一致），写进两条线
/// 因此对齐必然一致；真源日后声明了帧数/尺寸，派生器改从真源取，这个约定即退役。
fn default_visual_triple(kind: &str) -> SpecTriple {
    match kind {
        "sprite2d" | "ui" => {
            SpecTriple::full(1, crate::governance::AssetSize::new(1024, 1024), "png")
        }
        // model3d 的格式/尺寸真源没说，本产线也还没有 3D 通道：如实全 None，
        // 对齐层判 UnknownSpec 交人工——不编一个 fbx 出来。
        _ => SpecTriple::default(),
    }
}

/// 美术线 + 资产表：从 C3 白名单产物映射，风格约束引用锚点集（不复制其内容）。
fn derive_art_line(
    input: &DeriveInput<'_>,
    frozen_hash: &str,
) -> Adm4Result<(ArtContract, AssetRegistry)> {
    let mut envelope = ContractEnvelope::new(ART_LINE, input.generated_at, frozen_hash);
    envelope.derivation_policy = vec![
        "资产清单来自 C3 视觉白名单（无 visual_form 不产，R2）".to_string(),
        "风格来自设计阶段锁定的 style_anchor_set，本线只引用不重造（册 08）".to_string(),
    ];

    let mut assets = Vec::new();
    let mut entries = Vec::new();
    let mut push_asset = |asset_id: String,
                          name: String,
                          category: AssetCategory,
                          purpose: String,
                          spec_triple: SpecTriple,
                          description: String,
                          source_ref: SpecRef| {
        let format = spec_triple
            .format
            .clone()
            .unwrap_or_else(|| "png".to_string());
        let naming_pattern = format!("{}.{format}", lexical_variant(&asset_id));
        entries.push(AssetRegistryEntry {
            asset_id: asset_id.clone(),
            naming_pattern: naming_pattern.clone(),
            runtime_path: runtime_path_for(&asset_id, &format),
            state: AssetLifecycleState::Draft,
            stability: StabilityLevel::Experimental,
            source_refs: vec![source_ref.clone()],
        });
        assets.push(ArtAsset {
            asset_id,
            name,
            category,
            purpose,
            production_spec: spec_triple,
            naming_pattern,
            required_readability: "缩放至游戏内尺寸后轮廓可辨".to_string(),
            forbidden_visuals: Vec::new(),
            acceptance_checks: vec!["与风格锚点同一视觉语言".to_string()],
            source_refs: vec![source_ref],
            art_rule: description,
        });
    };

    for asset in &input.content.assets {
        let entity_name = entity_display_name(input.spec, &asset.entity_id);
        push_asset(
            asset_id_for(&asset.asset_kind, &asset.entity_id)?,
            entity_name.clone(),
            match asset.asset_kind.as_str() {
                "sprite2d" => AssetCategory::Illustration,
                _ => AssetCategory::Model,
            },
            format!("{entity_name} 的游戏内呈现"),
            default_visual_triple(&asset.asset_kind),
            asset.description.text.clone(),
            SpecRef::new(format!("entities/{}", asset.entity_id)),
        );
    }
    for ui in &input.content.ui_entries {
        let entity_name = entity_display_name(input.spec, &ui.entity_id);
        push_asset(
            asset_id_for("ui", &ui.entity_id)?,
            entity_name,
            AssetCategory::Ui,
            ui.purpose.clone(),
            default_visual_triple("ui"),
            ui.purpose.clone(),
            SpecRef::new(format!("entities/{}", ui.entity_id)),
        );
    }

    if assets.is_empty() {
        // 一个可见实体都没有的游戏画不出来：这是设计侧的事实缺失，不是美术线的默认状态。
        envelope.coverage_gaps.push(CoverageGap {
            gap_id: "gap_no_visible_entities".to_string(),
            missing_fact: "C3 白名单没有任何可见实体（sprite2d/model3d/ui_only 均为零）"
                .to_string(),
            required_by: "art_line".to_string(),
            severity: GapSeverity::Blocking,
        });
    }

    let art = ArtContract {
        envelope,
        visual_language: VisualLanguage {
            tokens: Vec::new(),
            palette: Vec::new(),
            forbidden_motifs: Vec::new(),
            // 只引用锚点集的位置；palette 等实际约束由 P2 从应用契约现读（不抄一份过来）。
            style_anchor_ref: "style/anchors".to_string(),
        },
        assets,
        visual_states: Vec::new(),
        ux_signal_bindings: Vec::new(),
        drift_checks: Vec::new(),
    };
    let registry = AssetRegistry {
        schema_version: crate::governance::GOVERNANCE_SCHEMA_VERSION.to_string(),
        entries,
    };
    Ok((art, registry))
}

fn entity_display_name(spec: &GameSpec, entity_id: &str) -> String {
    spec.entities
        .iter()
        .find(|entity| entity.id == entity_id)
        .map(|entity| entity.name.clone())
        .unwrap_or_else(|| entity_id.to_string())
}

/// C3 白名单条目的视觉形态断言（P2 白名单二次核对用：产前再验一遍，不信上游状态位）。
pub fn is_visual(spec: &GameSpec, entity_id: &str) -> bool {
    spec.entities
        .iter()
        .find(|entity| entity.id == entity_id)
        .and_then(|entity| entity.visual_form.as_ref())
        .is_some_and(|form| {
            matches!(
                form,
                VisualForm::Sprite2d | VisualForm::Model3d | VisualForm::UiOnly
            )
        })
}

/// 把 C3 的资产规格映射回派生资产 id（P2 生产时要按 C3 描述拼提示词）。
pub fn asset_id_of_c3(asset: &C3AssetSpec) -> Adm4Result<String> {
    asset_id_for(&asset.asset_kind, &asset.entity_id)
}

#[cfg(test)]
mod tests {
    use super::*;
    use adm4_contracts::{
        AnchoredNarrative, CardinalityDeclaration, CardinalityRange, EvidencePointer,
        MeasuredMetric,
    };
    use adm4_pipeline::{C3AssetSpec, C4Capability, UiSpecEntry};
    use adm4_spec::{
        AcceptanceScenario, EntitySpec, MechanicSpec, ProjectIntent, PropertySpec, SpecIdentity,
        SystemSpec,
    };

    fn spec() -> GameSpec {
        GameSpec {
            identity: SpecIdentity {
                schema_version: "4.0.0".into(),
                project_id: "demo".into(),
                frozen_hash: "sha256:frozen".into(),
            },
            intent: ProjectIntent::default(),
            systems: vec![SystemSpec {
                id: "combat_system".into(),
                name: "战斗系统".into(),
                purpose: "结算克制伤害".into(),
                interfaces: Vec::new(),
                design_notes: Vec::new(),
            }],
            mechanics: vec![MechanicSpec {
                id: "counter_damage".into(),
                system_id: "combat_system".into(),
                rule_text: "克制系数放大伤害".into(),
                preconditions: Vec::new(),
                effects: vec![EffectSpec::ModifyProperty {
                    entity: "guard".into(),
                    property: "attack".into(),
                    formula: "attack * counter".into(),
                }],
                state_machine: None,
                design_notes: Vec::new(),
            }],
            entities: vec![
                EntitySpec {
                    id: "guard".into(),
                    name: "守卫".into(),
                    visual_form: Some(VisualForm::Sprite2d),
                    properties: vec![PropertySpec {
                        key: "attack".into(),
                        kind: adm4_contracts::ValueKind::Int,
                        constraint: None,
                    }],
                },
                EntitySpec {
                    id: "hud_panel".into(),
                    name: "指挥面板".into(),
                    visual_form: Some(VisualForm::UiOnly),
                    properties: Vec::new(),
                },
            ],
            tables: Vec::new(),
            content: Vec::new(),
            graphs: Vec::new(),
            acceptance: Vec::new(),
            source_map: Vec::new(),
        }
    }

    fn content() -> ContentInventoryContract {
        ContentInventoryContract {
            assets: vec![C3AssetSpec {
                id: "asset_guard".into(),
                entity_id: "guard".into(),
                asset_kind: "sprite2d".into(),
                description: AnchoredNarrative::new(
                    "扁平卡通守卫立绘",
                    vec![SpecRef::new("entities/guard")],
                )
                .expect("锚定叙述"),
            }],
            ui_entries: vec![UiSpecEntry {
                id: "ui_hud_panel".into(),
                entity_id: "hud_panel".into(),
                purpose: "指挥面板 的界面呈现".into(),
            }],
            non_visual_entities: Vec::new(),
            cardinality: CardinalityDeclaration {
                rule: "test".into(),
                produced: 1,
                expected: CardinalityRange { min: 0, max: 10 },
                dropped: Vec::new(),
            },
        }
    }

    fn capabilities() -> CapabilitiesContract {
        CapabilitiesContract {
            capabilities: vec![C4Capability {
                id: "cap_counter_damage".into(),
                interface_name: "CounterDamageService".into(),
                data_structures: vec!["guard".into()],
                source_refs: vec![SpecRef::new("mechanics/counter_damage")],
                scenarios: vec![AcceptanceScenario {
                    id: "acc_counter".into(),
                    capability_id: "cap_counter_damage".into(),
                    given: vec!["守卫在场".into()],
                    when: vec!["克制目标进入射程".into()],
                    then: vec!["伤害按系数放大".into()],
                    source_refs: Vec::new(),
                }],
            }],
            coverage: MeasuredMetric::new(
                1.0,
                vec![EvidencePointer {
                    file: "C4/contract.json".into(),
                    path: "mechanics/counter_damage".into(),
                    observed: "被能力 cap_counter_damage 覆盖".into(),
                }],
            )
            .expect("覆盖率"),
            cardinality: CardinalityDeclaration {
                rule: "test".into(),
                produced: 1,
                expected: CardinalityRange { min: 1, max: 1 },
                dropped: Vec::new(),
            },
            blockers: Vec::new(),
        }
    }

    fn derive() -> TwoLineDerivation {
        derive_two_lines(&DeriveInput {
            spec: &spec(),
            content: &content(),
            capabilities: &capabilities(),
            generated_at: "2026-08-31T00:00:00Z",
        })
        .expect("派生")
    }

    /// 核心承诺：派生出的双线契约各自自洽、能过对齐层、且对齐干净（同源派生必然一致）。
    #[test]
    fn two_lines_derive_and_align_cleanly() {
        let derivation = derive();
        assert_eq!(derivation.program.systems.len(), 1);
        assert_eq!(derivation.program.capabilities.len(), 1);
        assert_eq!(derivation.program.asset_dependencies.len(), 2);
        assert_eq!(derivation.art.assets.len(), 2);
        assert_eq!(derivation.registry.entries.len(), 2);
        assert!(
            derivation.alignment.is_clean(),
            "同源派生的两条线必须天然对齐：{:?}",
            derivation.alignment.unresolved_conflicts
        );
        assert_eq!(derivation.alignment.coverage.unified, 2);

        // 命名权威：派生名必须过机制校验（类型前缀 + 分段 + id→文件名链）。
        let violations = derivation
            .registry
            .naming_violations(&crate::governance::asset_registry::NamingRules::default());
        assert!(violations.is_empty(), "{violations:?}");
        // 映射规则可读：sprite2d→T_、ui_only→UI_。
        assert!(derivation.registry.entry("T_Guard").is_some());
        assert!(derivation.registry.entry("UI_HudPanel").is_some());
    }

    /// 确定性：同一输入两次派生逐字节相同（时钟只进 generated_at，两次传同一值）。
    #[test]
    fn derivation_is_deterministic() {
        let first = serde_json::to_string(&derive().program).expect("序列化");
        let second = serde_json::to_string(&derive().program).expect("序列化");
        assert_eq!(first, second);
    }

    /// 只映射不发明：真源缺事实（实体没人写）→ 登记 gap，不编一个归属系统的新名字。
    #[test]
    fn missing_facts_become_gaps_not_inventions() {
        let mut lonely = spec();
        lonely.entities.push(EntitySpec {
            id: "decoration".into(),
            name: "装饰物".into(),
            visual_form: Some(VisualForm::Sprite2d),
            properties: Vec::new(),
        });
        let derivation = derive_two_lines(&DeriveInput {
            spec: &lonely,
            content: &content(),
            capabilities: &capabilities(),
            generated_at: "2026-08-31T00:00:00Z",
        })
        .expect("派生");
        let gap = derivation
            .program
            .envelope
            .coverage_gaps
            .iter()
            .find(|gap| gap.gap_id == "gap_entity_owner_decoration")
            .expect("无人写的实体必须登记 gap");
        assert_eq!(gap.severity, GapSeverity::Warning);
        // 归属落在已声明的系统上，而不是发明一个新系统。
        let entity = derivation
            .program
            .entities
            .iter()
            .find(|entity| entity.entity_id == "decoration")
            .expect("实体在案");
        assert_eq!(entity.owner_system, "combat_system");
    }

    /// C4 与真源不同版（能力指向不存在的机制）必须报错，不静默丢弃。
    #[test]
    fn stale_capabilities_contract_is_rejected() {
        let mut stale = capabilities();
        stale.capabilities[0].id = "cap_ghost_mechanic".into();
        let error = derive_two_lines(&DeriveInput {
            spec: &spec(),
            content: &content(),
            capabilities: &stale,
            generated_at: "2026-08-31T00:00:00Z",
        })
        .expect_err("指向不存在机制的能力必须被拒");
        assert!(
            error.message.contains("ghost_mechanic"),
            "{}",
            error.message
        );
    }

    /// 无冻结哈希 / 无系统：显式报错（Blocked/Validation），不产半份契约。
    #[test]
    fn missing_hash_or_systems_fail_closed() {
        let mut no_hash = spec();
        no_hash.identity.frozen_hash.clear();
        assert!(
            derive_two_lines(&DeriveInput {
                spec: &no_hash,
                content: &content(),
                capabilities: &capabilities(),
                generated_at: "now",
            })
            .is_err()
        );

        let mut no_systems = spec();
        no_systems.systems.clear();
        let error = derive_two_lines(&DeriveInput {
            spec: &no_systems,
            content: &content(),
            capabilities: &capabilities(),
            generated_at: "now",
        })
        .expect_err("无系统必须 Blocked");
        assert_eq!(error.kind, adm4_foundation::Adm4ErrorKind::Blocked);
    }

    /// 未知资产类型不猜前缀（R2）。
    #[test]
    fn unknown_asset_kind_is_rejected() {
        assert!(asset_id_for("audio", "boom").is_err());
        assert_eq!(
            asset_id_for("sprite2d", "guard_tower").unwrap(),
            "T_GuardTower"
        );
        assert_eq!(pascal_case("hud_panel"), "HudPanel");
    }
}
