//! 对齐合流层（册 07 §4）：两条线在这里重新汇合，**确定性核对，不生成任何资产**。
//!
//! 做三件事，全是可断言的机器判定：
//! 1. 程序线的每条资产依赖去美术线找对应资产，比对**帧数 / 尺寸 / 格式**三要素；
//! 2. 对不上 → `unresolved_conflicts`（标 `human_decision_required`，交人裁决，代码不替人选）；
//! 3. 美术有、程序无依赖 → `orphan_art_assets`；程序要、美术缺 → `missing_for_program`。
//!
//! **【优化】**（册 07 §8 已登记）：py 当年这层靠 AI 提示词 + 导入引擎驱动；V4 把三要素核对与
//! orphan/conflict 判定收回成确定性 Rust，只有「冲突怎么取舍」才交人工。本模块因此不引入任何
//! AI 接口——同一份输入永远得到同一份报告。

use super::SpecTriple;
use super::art_line::ArtContract;
use super::asset_registry::AssetRegistry;
use super::program_line::{ProgramAssetDependency, ProgramContract};
use adm4_contracts::SpecRef;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

/// 程序侧与美术侧钉在同一个 uid 上的资产。
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct UnifiedAsset {
    /// 统一标识 = 稳定 `asset_id`（铁律②的单点锚定，不另起一套 uid）。
    pub uid: String,
    /// 程序侧引用点（如 `hero_controller.idle_anim`）。
    pub program_ref: String,
    /// 美术侧标识（`asset_id`）。
    pub art_ref: String,
    /// 命名权威登记的文件名模式。
    pub naming_pattern: String,
    /// 双方一致的三要素。
    pub spec_triple: SpecTriple,
    /// 程序侧依赖的真源锚点（追溯用）。
    pub source_refs: Vec<SpecRef>,
}

/// 冲突类别。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConflictKind {
    /// 旧档/未分类（读到它说明报告是别的版本写的，按需重算）。
    #[default]
    Unknown,
    /// 三要素双方都声明了但对不上。
    SpecMismatch,
    /// 三要素至少一侧没声明——**未知即停**，不当成一致（R2）。
    UnknownSpec,
    /// 美术线有这个资产，但命名权威（资产表）里没登记。
    NamingAuthorityMissing,
}

/// 一条待人工裁决的冲突。
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Conflict {
    pub conflict_id: String,
    pub kind: ConflictKind,
    pub asset_id: String,
    pub program_ref: String,
    /// 程序侧要求（人类可读，未知项写「未声明」）。
    pub program_requires: String,
    /// 美术侧提供。
    pub art_provides: String,
    /// 逐项差异明细。
    pub differences: Vec<String>,
    /// 恒为 true：取舍归人，代码只负责把冲突摆出来。
    pub human_decision_required: bool,
}

/// 美术产出但程序没有依赖的资产。
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct OrphanArtAsset {
    pub asset_id: String,
    pub reason: String,
}

/// 程序需要但美术线没有的资产。
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct MissingForProgram {
    pub asset_id: String,
    pub program_ref: String,
    pub reason: String,
}

/// 对齐覆盖率计数（R1：报实测计数，不报「基本对齐」这类无证据结论）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct AlignmentCoverage {
    pub program_dependencies: usize,
    pub art_assets: usize,
    pub unified: usize,
    pub conflicts: usize,
    pub orphans: usize,
    pub missing: usize,
}

/// 对齐报告（`alignment_report.json`）。
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct AlignmentReport {
    pub unified_assets: Vec<UnifiedAsset>,
    pub unresolved_conflicts: Vec<Conflict>,
    pub orphan_art_assets: Vec<OrphanArtAsset>,
    pub missing_for_program: Vec<MissingForProgram>,
    pub coverage: AlignmentCoverage,
}

impl AlignmentReport {
    /// 是否可放行下游：无冲突、无缺失。
    ///
    /// orphan **不**阻断：美术多产了一个图不影响程序跑起来，但必须在报告里可见
    /// （它通常意味着某条程序依赖被漏声明了，属于要人看一眼的线索而非硬错）。
    pub fn is_clean(&self) -> bool {
        self.unresolved_conflicts.is_empty() && self.missing_for_program.is_empty()
    }

    /// 一行摘要（服务层日志 / CLI / 桌面状态栏共用一份口径）。
    pub fn summary(&self) -> String {
        format!(
            "对齐 {} 项（程序依赖 {} 条 / 美术资产 {} 个），冲突 {} 条，孤儿资产 {} 个，程序侧缺件 {} 个",
            self.coverage.unified,
            self.coverage.program_dependencies,
            self.coverage.art_assets,
            self.coverage.conflicts,
            self.coverage.orphans,
            self.coverage.missing
        )
    }
}

/// 三要素比对结果。
enum TripleVerdict {
    Match,
    /// 至少一侧未声明（连同已发现的差异一起带出来）。
    Unknown(Vec<String>),
    /// 双方都声明了但不同。
    Mismatch(Vec<String>),
}

/// 比对帧数 / 尺寸 / 格式。
///
/// 「一侧未声明」优先判为 `Unknown`：既然连要求是什么都不知道，就谈不上「一致」也谈不上
/// 「不一致」——只能停下来问人（R2 未知即停）。
fn compare_triple(required: &SpecTriple, provided: &SpecTriple) -> TripleVerdict {
    let mut unknown = Vec::new();
    let mut mismatch = Vec::new();

    match (required.frames, provided.frames) {
        (Some(left), Some(right)) if left != right => {
            mismatch.push(format!("帧数：程序要 {left}，美术给 {right}"));
        }
        (Some(_), Some(_)) => {}
        (left, right) => unknown.push(format!(
            "帧数未声明（程序侧 {}，美术侧 {}）",
            describe_frames(left),
            describe_frames(right)
        )),
    }
    match (required.size, provided.size) {
        (Some(left), Some(right)) if left != right => {
            mismatch.push(format!("尺寸：程序要 {left}，美术给 {right}"));
        }
        (Some(_), Some(_)) => {}
        (left, right) => unknown.push(format!(
            "尺寸未声明（程序侧 {}，美术侧 {}）",
            describe_size(left),
            describe_size(right)
        )),
    }
    match (required.format.as_deref(), provided.format.as_deref()) {
        (Some(left), Some(right)) if left != right => {
            mismatch.push(format!("格式：程序要 {left}，美术给 {right}"));
        }
        (Some(_), Some(_)) => {}
        (left, right) => unknown.push(format!(
            "格式未声明（程序侧 {}，美术侧 {}）",
            describe_text(left),
            describe_text(right)
        )),
    }

    if !unknown.is_empty() {
        unknown.extend(mismatch);
        return TripleVerdict::Unknown(unknown);
    }
    if !mismatch.is_empty() {
        return TripleVerdict::Mismatch(mismatch);
    }
    TripleVerdict::Match
}

fn describe_frames(value: Option<u32>) -> String {
    match value {
        Some(count) => count.to_string(),
        None => "未声明".to_string(),
    }
}

fn describe_size(value: Option<super::AssetSize>) -> String {
    match value {
        Some(size) => size.to_string(),
        None => "未声明".to_string(),
    }
}

fn describe_text(value: Option<&str>) -> String {
    match value {
        Some(text) => text.to_string(),
        None => "未声明".to_string(),
    }
}

/// 对齐合流：程序线 × 美术线 × 命名权威 → 报告。
///
/// 纯函数、无 IO、无 AI：同一份输入永远产出同一份报告（可作为回归夹具直接断言）。
pub fn align(
    program: &ProgramContract,
    art: &ArtContract,
    registry: &AssetRegistry,
) -> AlignmentReport {
    let mut report = AlignmentReport::default();
    let mut referenced: BTreeSet<&str> = BTreeSet::new();

    for dependency in &program.asset_dependencies {
        referenced.insert(dependency.asset_id.as_str());
        let Some(asset) = art.asset(&dependency.asset_id) else {
            report.missing_for_program.push(MissingForProgram {
                asset_id: dependency.asset_id.clone(),
                program_ref: dependency.dependency_id.clone(),
                reason: "程序线声明了依赖，美术线没有对应资产".to_string(),
            });
            continue;
        };
        // 命名权威缺登记：即使两侧规格一致，也拿不到文件名与运行时路径 → 仍是冲突。
        let Some(registered) = registry.entry(&dependency.asset_id) else {
            report.unresolved_conflicts.push(conflict(
                dependency,
                ConflictKind::NamingAuthorityMissing,
                &asset.production_spec,
                vec![format!(
                    "资产 {} 未在资产表登记：拿不到文件名与运行时加载路径",
                    dependency.asset_id
                )],
            ));
            continue;
        };
        match compare_triple(&dependency.required_spec, &asset.production_spec) {
            TripleVerdict::Match => report.unified_assets.push(UnifiedAsset {
                uid: dependency.asset_id.clone(),
                program_ref: dependency.dependency_id.clone(),
                art_ref: dependency.asset_id.clone(),
                naming_pattern: registered.naming_pattern.clone(),
                spec_triple: asset.production_spec.clone(),
                source_refs: dependency.source_refs.clone(),
            }),
            TripleVerdict::Unknown(differences) => {
                report.unresolved_conflicts.push(conflict(
                    dependency,
                    ConflictKind::UnknownSpec,
                    &asset.production_spec,
                    differences,
                ));
            }
            TripleVerdict::Mismatch(differences) => {
                report.unresolved_conflicts.push(conflict(
                    dependency,
                    ConflictKind::SpecMismatch,
                    &asset.production_spec,
                    differences,
                ));
            }
        }
    }

    for asset in &art.assets {
        if !referenced.contains(asset.asset_id.as_str()) {
            report.orphan_art_assets.push(OrphanArtAsset {
                asset_id: asset.asset_id.clone(),
                reason: "美术线登记了资产，程序线没有任何依赖引用它".to_string(),
            });
        }
    }

    report.coverage = AlignmentCoverage {
        program_dependencies: program.asset_dependencies.len(),
        art_assets: art.assets.len(),
        unified: report.unified_assets.len(),
        conflicts: report.unresolved_conflicts.len(),
        orphans: report.orphan_art_assets.len(),
        missing: report.missing_for_program.len(),
    };
    report
}

fn conflict(
    dependency: &ProgramAssetDependency,
    kind: ConflictKind,
    provided: &SpecTriple,
    differences: Vec<String>,
) -> Conflict {
    Conflict {
        conflict_id: format!("CONFLICT-{}", dependency.asset_id),
        kind,
        asset_id: dependency.asset_id.clone(),
        program_ref: dependency.dependency_id.clone(),
        program_requires: dependency.required_spec.describe(),
        art_provides: provided.describe(),
        differences,
        human_decision_required: true,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::governance::art_line::{ArtAsset, AssetCategory};
    use crate::governance::asset_registry::{
        AssetLifecycleState, AssetRegistryEntry, StabilityLevel,
    };
    use crate::governance::program_line::ProgramSystem;
    use crate::governance::{ART_LINE, AssetSize, ContractEnvelope, PROGRAM_LINE};

    fn program_with(dependencies: Vec<ProgramAssetDependency>) -> ProgramContract {
        ProgramContract {
            envelope: ContractEnvelope::new(PROGRAM_LINE, "now", "sha256:frozen"),
            systems: vec![ProgramSystem {
                system_id: "combat_system".into(),
                name: "战斗系统".into(),
                responsibility: "结算".into(),
                source_refs: vec![SpecRef::new("systems/combat")],
            }],
            asset_dependencies: dependencies,
            ..ProgramContract::default()
        }
    }

    fn dependency(asset_id: &str, spec: SpecTriple) -> ProgramAssetDependency {
        ProgramAssetDependency {
            dependency_id: format!("hero_controller.{}", asset_id.to_ascii_lowercase()),
            owner_system: "combat_system".into(),
            asset_id: asset_id.into(),
            required_spec: spec,
            source_refs: vec![SpecRef::new("entities/guard")],
        }
    }

    fn art_with(assets: Vec<ArtAsset>) -> ArtContract {
        ArtContract {
            envelope: ContractEnvelope::new(ART_LINE, "now", "sha256:frozen"),
            assets,
            ..ArtContract::default()
        }
    }

    fn asset(asset_id: &str, spec: SpecTriple) -> ArtAsset {
        ArtAsset {
            asset_id: asset_id.into(),
            name: asset_id.into(),
            category: AssetCategory::Animation,
            production_spec: spec,
            naming_pattern: format!("{}.png", asset_id.to_ascii_lowercase()),
            source_refs: vec![SpecRef::new("entities/guard")],
            ..ArtAsset::default()
        }
    }

    fn registry_with(ids: &[&str]) -> AssetRegistry {
        AssetRegistry {
            schema_version: "4.0.0".into(),
            entries: ids
                .iter()
                .map(|asset_id| AssetRegistryEntry {
                    asset_id: (*asset_id).to_string(),
                    naming_pattern: format!("{}_{{frame:03d}}.png", asset_id.to_ascii_lowercase()),
                    runtime_path: format!("Assets/{asset_id}.png"),
                    state: AssetLifecycleState::Approved,
                    stability: StabilityLevel::Stable,
                    source_refs: Vec::new(),
                })
                .collect(),
        }
    }

    #[test]
    fn matching_three_elements_produce_a_unified_asset() {
        let spec = SpecTriple::full(8, AssetSize::new(256, 256), "png");
        let report = align(
            &program_with(vec![dependency("UI_PlayerIdle", spec.clone())]),
            &art_with(vec![asset("UI_PlayerIdle", spec.clone())]),
            &registry_with(&["UI_PlayerIdle"]),
        );
        assert!(report.is_clean());
        assert_eq!(report.unified_assets.len(), 1);
        let unified = &report.unified_assets[0];
        assert_eq!(unified.uid, "UI_PlayerIdle");
        assert_eq!(unified.program_ref, "hero_controller.ui_playeridle");
        assert_eq!(unified.naming_pattern, "ui_playeridle_{frame:03d}.png");
        assert_eq!(unified.spec_triple, spec);
        assert_eq!(report.coverage.unified, 1);
        assert_eq!(report.coverage.conflicts, 0);

        // 报告本身是要落盘的契约：serde 往返必须无损。
        let json = serde_json::to_string_pretty(&report).expect("序列化");
        let back: AlignmentReport = serde_json::from_str(&json).expect("反序列化");
        assert_eq!(back, report);
    }

    /// py `AlignmentProtocol` 的原始例子：程序要 16 帧、美术给 8 帧 → 冲突交人工。
    #[test]
    fn frame_count_mismatch_becomes_an_unresolved_conflict() {
        let report = align(
            &program_with(vec![dependency(
                "VFX_BossDeath",
                SpecTriple::full(16, AssetSize::new(512, 512), "png"),
            )]),
            &art_with(vec![asset(
                "VFX_BossDeath",
                SpecTriple::full(8, AssetSize::new(512, 512), "png"),
            )]),
            &registry_with(&["VFX_BossDeath"]),
        );
        assert!(!report.is_clean());
        assert_eq!(report.unresolved_conflicts.len(), 1);
        let conflict = &report.unresolved_conflicts[0];
        assert_eq!(conflict.kind, ConflictKind::SpecMismatch);
        assert_eq!(conflict.conflict_id, "CONFLICT-VFX_BossDeath");
        assert!(conflict.human_decision_required, "取舍归人，代码不替人选");
        assert_eq!(conflict.differences, vec!["帧数：程序要 16，美术给 8"]);
        assert!(conflict.program_requires.contains("16 帧"));
        assert!(conflict.art_provides.contains("8 帧"));
    }

    /// 一侧没声明三要素时**不许**判为一致：未知即停（R2）。
    #[test]
    fn undeclared_element_is_a_conflict_not_a_match() {
        let report = align(
            &program_with(vec![dependency(
                "UI_Portrait",
                SpecTriple {
                    frames: Some(1),
                    size: None,
                    format: Some("png".into()),
                },
            )]),
            &art_with(vec![asset(
                "UI_Portrait",
                SpecTriple {
                    frames: Some(1),
                    size: Some(AssetSize::new(128, 128)),
                    format: Some("png".into()),
                },
            )]),
            &registry_with(&["UI_Portrait"]),
        );
        assert_eq!(report.unresolved_conflicts.len(), 1);
        assert_eq!(
            report.unresolved_conflicts[0].kind,
            ConflictKind::UnknownSpec
        );
        assert!(report.unified_assets.is_empty(), "未知不得计入已对齐");

        // 两侧都没声明同样不算一致（「都不知道」不是「一样」）。
        let both_unknown = align(
            &program_with(vec![dependency("UI_Portrait", SpecTriple::default())]),
            &art_with(vec![asset("UI_Portrait", SpecTriple::default())]),
            &registry_with(&["UI_Portrait"]),
        );
        assert_eq!(
            both_unknown.unresolved_conflicts[0].kind,
            ConflictKind::UnknownSpec
        );
    }

    #[test]
    fn orphan_and_missing_are_reported_on_their_own_lists() {
        let spec = SpecTriple::full(4, AssetSize::new(64, 64), "png");
        let report = align(
            &program_with(vec![dependency("UI_Needed", spec.clone())]),
            &art_with(vec![asset("UI_Extra", spec)]),
            &registry_with(&["UI_Extra"]),
        );
        assert_eq!(report.missing_for_program.len(), 1);
        assert_eq!(report.missing_for_program[0].asset_id, "UI_Needed");
        assert_eq!(report.orphan_art_assets.len(), 1);
        assert_eq!(report.orphan_art_assets[0].asset_id, "UI_Extra");
        assert!(!report.is_clean(), "程序侧缺件必须阻断");

        // 只有孤儿资产时：可见但不阻断（多产一张图不影响程序跑起来）。
        let only_orphan = align(
            &program_with(Vec::new()),
            &art_with(vec![asset(
                "UI_Extra",
                SpecTriple::full(4, AssetSize::new(64, 64), "png"),
            )]),
            &registry_with(&["UI_Extra"]),
        );
        assert_eq!(only_orphan.orphan_art_assets.len(), 1);
        assert!(only_orphan.is_clean());
        assert!(only_orphan.summary().contains("孤儿资产 1 个"));
    }

    /// 规格一致但没在命名权威登记：仍是冲突（拿不到文件名与运行时路径）。
    #[test]
    fn unregistered_asset_is_a_naming_authority_conflict() {
        let spec = SpecTriple::full(1, AssetSize::new(32, 32), "png");
        let report = align(
            &program_with(vec![dependency("UI_Ghost", spec.clone())]),
            &art_with(vec![asset("UI_Ghost", spec)]),
            &registry_with(&[]),
        );
        assert_eq!(
            report.unresolved_conflicts[0].kind,
            ConflictKind::NamingAuthorityMissing
        );
    }

    /// 报告顺序只取决于输入顺序：同一份输入两次对齐逐字节相同（可做回归夹具）。
    #[test]
    fn alignment_is_deterministic() {
        let spec = SpecTriple::full(2, AssetSize::new(16, 16), "png");
        let program = program_with(vec![
            dependency("UI_B", spec.clone()),
            dependency("UI_A", SpecTriple::default()),
        ]);
        let art = art_with(vec![asset("UI_A", spec.clone()), asset("UI_B", spec)]);
        let registry = registry_with(&["UI_A", "UI_B"]);
        let first = serde_json::to_string(&align(&program, &art, &registry)).expect("序列化");
        let second = serde_json::to_string(&align(&program, &art, &registry)).expect("序列化");
        assert_eq!(first, second);
    }
}
