//! 生产台账 + AssetGenome 回填（册 08 §4.3-4.4）。
//!
//! 台账是 godogen 硬要求的七字段记录（Name / Purpose / Runtime path / In-game size /
//! Cost / Fallback / Used by），外加追溯所需的提示词与指纹；基因表回填后与资产表对账
//! （path = 运行时加载 path，[`crate::governance::asset_genome`] 的确定性核对）。
//!
//! 一致性比对（vs 风格锚点）在这里只做**确定性可判**的部分：格式对账、提示词前缀合规。
//! 真视觉比对机器判不了——如实落 `visual_review_note`，不产一条"AI 看着挺像"的假结论（R1/R7）。

use crate::governance::alignment::AlignmentReport;
use crate::governance::art_line::{ArtContract, DriftCheck, DriftSeverity, drift_check_id};
use crate::governance::asset_genome::{AssetGenome, GenomeDrift, GenomeEntry};
use crate::governance::asset_registry::AssetRegistry;
use adm4_foundation::{Adm4Error, Adm4Result};
use serde::{Deserialize, Serialize};

/// 这张图从哪来。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProductionSource {
    /// 真实生成调用（花钱、占预算额度）。
    #[default]
    Ai,
    /// 内容哈希缓存命中（不花钱、不占额度）。
    Cache,
    /// 外部工具通道（本期不可达，留枚举位）。
    External,
}

/// 台账条目（godogen 七字段 + 追溯字段）。
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct LedgerEntry {
    pub asset_id: String,
    /// Name：实际文件名。
    pub file_name: String,
    /// Purpose。
    pub purpose: String,
    /// Runtime path（= 资产表登记的运行时加载路径，逐字相同）。
    pub runtime_path: String,
    /// In-game size：**实测**入游尺寸；本波未实测，如实 None（不抄申报尺寸，R1）。
    pub in_game_size: Option<crate::governance::AssetSize>,
    /// Cost：本资产消耗的真实生成调用数（缓存命中 = 0）。
    pub generation_calls: usize,
    pub source: ProductionSource,
    /// Fallback：本产线没有兜底资产（生成失败即停，R7）——写死这句而不是留空，
    /// godogen 要求这个字段**被回答**，「无」也是一种回答。
    pub fallback: String,
    /// Used by：程序线的依赖引用点（来自对齐报告的 unified）。
    pub used_by: Vec<String>,
    /// 生成它的完整提示词（重生成与一致性比对都要）。
    pub prompt: String,
    pub bytes_sha256: String,
    pub bytes: u64,
    pub media_type: String,
    pub provider_id: String,
    pub model: String,
    pub produced_at: String,
}

/// 生产台账（`P2` 契约主体之一）。
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct AssetProductionLedger {
    pub schema_version: String,
    pub entries: Vec<LedgerEntry>,
    /// 实测计数（R1）。
    pub cache_hits: usize,
    pub cache_misses: usize,
    pub generation_calls: usize,
}

impl AssetProductionLedger {
    pub fn new() -> Self {
        Self {
            schema_version: crate::governance::GOVERNANCE_SCHEMA_VERSION.to_string(),
            ..Self::default()
        }
    }

    pub fn entry(&self, asset_id: &str) -> Option<&LedgerEntry> {
        self.entries.iter().find(|entry| entry.asset_id == asset_id)
    }

    pub fn summary(&self) -> String {
        format!(
            "台账 {} 条：缓存命中 {} / 未命中 {}，真实生成调用 {} 次",
            self.entries.len(),
            self.cache_hits,
            self.cache_misses,
            self.generation_calls
        )
    }
}

/// 从台账回填基因表并与资产表对账。
///
/// 返回（基因表, 全部差异）。差异**如实带回**而不是内部吞掉——调用方（P2 执行器）
/// 决定差异是否阻断（本产线：任何差异都不放行装配）。
pub fn backfill_genome(
    ledger: &AssetProductionLedger,
    registry: &AssetRegistry,
) -> Adm4Result<(AssetGenome, Vec<GenomeDrift>)> {
    let mut genome = AssetGenome {
        schema_version: crate::governance::GOVERNANCE_SCHEMA_VERSION.to_string(),
        assets: Vec::new(),
    };
    for entry in &ledger.entries {
        genome.backfill(GenomeEntry {
            id: entry.asset_id.clone(),
            files: vec![entry.runtime_path.clone()],
            created_at: entry.produced_at.clone(),
            in_game_size: entry.in_game_size,
            used_by: entry.used_by.clone(),
        })?;
    }
    let drifts = genome.verify_runtime_paths(registry);
    Ok((genome, drifts))
}

/// 确定性一致性比对（vs 风格应用契约）：格式对账 + 提示词前缀合规。
///
/// 每个资产恰好一条 `DRIFT-{asset_id}`（G1 的 id 规则如此；多维发现合并进 detail）。
/// 判得出问题 → `Block`（回修复队列重生成）；确定性检查全过 → `Ok` 带证据；
/// 真视觉比对不在这里假装——见 [`VISUAL_REVIEW_NOTE`]。
pub fn deterministic_drift_checks(
    art: &ArtContract,
    ledger: &AssetProductionLedger,
    prompt_prefix: &str,
) -> Vec<DriftCheck> {
    let mut checks = Vec::new();
    for asset in &art.assets {
        let Some(entry) = ledger.entry(&asset.asset_id) else {
            // 没产出来的资产不在这里报——基因表对账（NotProduced）已经把账算破了，
            // 两处各报一遍只会让人修两次同一件事。
            continue;
        };
        let mut problems = Vec::new();
        let mut evidence = Vec::new();
        // 格式对账：申报 png 拿回 jpeg 是确定性可判的漂移。
        if let Some(declared) = asset.production_spec.format.as_deref() {
            let expected = format!("image/{declared}");
            if entry.media_type != expected {
                problems.push(format!(
                    "格式漂移：申报 {declared}，实际字节头是 {}",
                    entry.media_type
                ));
            } else {
                evidence.push(format!("格式一致：{}（字节头嗅探）", entry.media_type));
            }
        }
        // 提示词前缀合规：风格一致性的实际抓手。
        if entry.prompt.starts_with(prompt_prefix.trim()) {
            evidence.push("提示词以应用契约前缀起头".to_string());
        } else {
            problems.push(
                "提示词未以应用契约的 prompt_prefix 起头：风格约束没有进生成输入".to_string(),
            );
        }
        evidence.push(format!("产物指纹 sha256:{}", entry.bytes_sha256));
        let (severity, detail) = if problems.is_empty() {
            (
                DriftSeverity::Ok,
                "确定性比对通过（格式 / 提示词前缀）；视觉比对见 visual_review_note".to_string(),
            )
        } else {
            (DriftSeverity::Block, problems.join("；"))
        };
        checks.push(DriftCheck {
            check_id: drift_check_id(&asset.asset_id),
            asset_id: asset.asset_id.clone(),
            severity,
            detail,
            evidence,
        });
    }
    checks
}

/// 视觉比对的诚实边界（进 P2 契约原文，UI/CLI 原样呈现）。
pub const VISUAL_REVIEW_NOTE: &str = "真视觉一致性（构图/笔触/色调 vs 锚图）机器判不了：\
    确定性比对只覆盖格式与提示词前缀；视觉裁决需人工看图或后续接 VLM 评审通道（届时证据缓存）";

/// P2 落盘契约：台账 + 基因表 + 对账 + 漂移 + 修复队列。
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct AssetProductionRecord {
    pub schema_version: String,
    /// 锚定的风格锚点版本与契约哈希（这批资产按哪版风格产的）。
    pub anchor_version: u32,
    pub source_anchor_hash: String,
    pub ledger: AssetProductionLedger,
    pub genome: AssetGenome,
    /// 基因表 ↔ 资产表对账差异（必须为空才放行装配）。
    pub genome_drifts: Vec<GenomeDrift>,
    pub drift_checks: Vec<DriftCheck>,
    /// 需重生成的资产（Block 级漂移）。
    pub repair_queue: Vec<String>,
    pub visual_review_note: String,
    pub produced_at: String,
}

impl AssetProductionRecord {
    /// 是否可放行装配（P3 的准入口径）。
    pub fn clean(&self) -> bool {
        self.genome_drifts.is_empty() && self.repair_queue.is_empty()
    }
}

/// 汇总出一个 P2 契约（并从 Block 级漂移推导修复队列）。
pub fn assemble_record(
    anchor_version: u32,
    source_anchor_hash: &str,
    ledger: AssetProductionLedger,
    genome: AssetGenome,
    genome_drifts: Vec<GenomeDrift>,
    drift_checks: Vec<DriftCheck>,
    produced_at: &str,
) -> AssetProductionRecord {
    let repair_queue: Vec<String> = drift_checks
        .iter()
        .filter(|check| check.severity == DriftSeverity::Block)
        .map(|check| check.asset_id.clone())
        .collect();
    AssetProductionRecord {
        schema_version: crate::governance::GOVERNANCE_SCHEMA_VERSION.to_string(),
        anchor_version,
        source_anchor_hash: source_anchor_hash.to_string(),
        ledger,
        genome,
        genome_drifts,
        drift_checks,
        repair_queue,
        visual_review_note: VISUAL_REVIEW_NOTE.to_string(),
        produced_at: produced_at.to_string(),
    }
}

/// 台账的 Used-by 回填：从对齐报告把「谁在用这个资产」接回来。
pub fn used_by_from_alignment(alignment: &AlignmentReport, asset_id: &str) -> Vec<String> {
    alignment
        .unified_assets
        .iter()
        .filter(|unified| unified.uid == asset_id)
        .map(|unified| unified.program_ref.clone())
        .collect()
}

/// 基数对账（R6 在 P2 的收口）：台账条数必须等于美术线资产数。
///
/// 少产是缺件（有资产没产出来），多产是发明（台账里有美术线没有的东西），
/// 两个方向都不放行。
pub fn verify_cardinality(art: &ArtContract, ledger: &AssetProductionLedger) -> Adm4Result<()> {
    if ledger.entries.len() != art.assets.len() {
        return Err(Adm4Error::validation(format!(
            "R6 基数对账不符：美术线申报 {} 个资产，台账实产 {} 条（缺件与多产都不放行）",
            art.assets.len(),
            ledger.entries.len()
        )));
    }
    for entry in &ledger.entries {
        if art.asset(&entry.asset_id).is_none() {
            return Err(Adm4Error::validation(format!(
                "台账里的资产 {} 不在美术线契约内：生产端不得发明资产（铁律②）",
                entry.asset_id
            )));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::governance::art_line::{ArtAsset, AssetCategory, VisualLanguage};
    use crate::governance::asset_registry::{
        AssetLifecycleState, AssetRegistryEntry, StabilityLevel,
    };
    use crate::governance::{ART_LINE, AssetSize, ContractEnvelope, SpecTriple};
    use adm4_contracts::SpecRef;

    fn art() -> ArtContract {
        ArtContract {
            envelope: ContractEnvelope::new(ART_LINE, "now", "sha256:frozen"),
            visual_language: VisualLanguage::default(),
            assets: vec![ArtAsset {
                asset_id: "T_Guard".into(),
                name: "守卫".into(),
                category: AssetCategory::Illustration,
                purpose: "守卫的游戏内呈现".into(),
                production_spec: SpecTriple::full(1, AssetSize::new(1024, 1024), "png"),
                naming_pattern: "t_guard.png".into(),
                required_readability: "轮廓可辨".into(),
                forbidden_visuals: Vec::new(),
                acceptance_checks: Vec::new(),
                source_refs: vec![SpecRef::new("entities/guard")],
                art_rule: "扁平卡通守卫立绘".into(),
            }],
            visual_states: Vec::new(),
            ux_signal_bindings: Vec::new(),
            drift_checks: Vec::new(),
        }
    }

    fn registry() -> AssetRegistry {
        AssetRegistry {
            schema_version: "4.0.0".into(),
            entries: vec![AssetRegistryEntry {
                asset_id: "T_Guard".into(),
                naming_pattern: "t_guard.png".into(),
                runtime_path: "GameAssets/t_guard.png".into(),
                state: AssetLifecycleState::Draft,
                stability: StabilityLevel::Experimental,
                source_refs: Vec::new(),
            }],
        }
    }

    fn ledger_with(prompt: &str, media_type: &str) -> AssetProductionLedger {
        let mut ledger = AssetProductionLedger::new();
        ledger.entries.push(LedgerEntry {
            asset_id: "T_Guard".into(),
            file_name: "t_guard.png".into(),
            purpose: "守卫的游戏内呈现".into(),
            runtime_path: "GameAssets/t_guard.png".into(),
            in_game_size: None,
            generation_calls: 1,
            source: ProductionSource::Ai,
            fallback: "无（生成失败即停，R7）".into(),
            used_by: vec!["render.guard".into()],
            prompt: prompt.into(),
            bytes_sha256: "cafebabe".into(),
            bytes: 256,
            media_type: media_type.into(),
            provider_id: "scripted_image".into(),
            model: "scripted".into(),
            produced_at: "2026-08-31T00:00:00Z".into(),
        });
        ledger.generation_calls = 1;
        ledger.cache_misses = 1;
        ledger
    }

    /// 回填闭环：台账 → 基因表 → 与资产表对账零差异；path 逐字等于运行时加载 path。
    #[test]
    fn backfill_closes_the_runtime_path_loop() {
        let ledger = ledger_with("前缀 守卫", "image/png");
        let (genome, drifts) = backfill_genome(&ledger, &registry()).expect("回填");
        assert!(drifts.is_empty(), "{drifts:?}");
        assert_eq!(
            genome.entry("T_Guard").expect("在案").files,
            vec!["GameAssets/t_guard.png"]
        );
    }

    /// 产到别处（路径漂移）必须被对账检出。
    #[test]
    fn wrong_runtime_path_is_reported() {
        let mut ledger = ledger_with("前缀 守卫", "image/png");
        ledger.entries[0].runtime_path = "GameAssets/t_guard_v2.png".into();
        let (_, drifts) = backfill_genome(&ledger, &registry()).expect("回填");
        assert_eq!(drifts.len(), 1);
        assert_eq!(
            drifts[0].kind,
            crate::governance::asset_genome::GenomeDriftKind::RuntimePathMismatch
        );
    }

    /// 确定性比对：全过 → Ok 带证据；格式漂移 / 前缀缺失 → Block 进修复队列。
    #[test]
    fn deterministic_checks_pass_or_block_with_evidence() {
        let good =
            deterministic_drift_checks(&art(), &ledger_with("前缀 守卫", "image/png"), "前缀");
        assert_eq!(good.len(), 1);
        assert_eq!(good[0].severity, DriftSeverity::Ok);
        assert!(!good[0].evidence.is_empty(), "Ok 也要带证据（R1）");

        let bad_format =
            deterministic_drift_checks(&art(), &ledger_with("前缀 守卫", "image/jpeg"), "前缀");
        assert_eq!(bad_format[0].severity, DriftSeverity::Block);
        assert!(bad_format[0].detail.contains("格式漂移"));

        let bad_prefix = deterministic_drift_checks(
            &art(),
            &ledger_with("没有风格前缀的词", "image/png"),
            "前缀",
        );
        assert_eq!(bad_prefix[0].severity, DriftSeverity::Block);

        let record = assemble_record(
            1,
            "sha256:anchor",
            ledger_with("没有风格前缀的词", "image/png"),
            AssetGenome::default(),
            Vec::new(),
            bad_prefix,
            "now",
        );
        assert_eq!(record.repair_queue, vec!["T_Guard"]);
        assert!(!record.clean());
    }

    /// R6 基数对账：缺件与多产都不放行。
    #[test]
    fn cardinality_mismatch_fails_both_ways() {
        let empty = AssetProductionLedger::new();
        assert!(verify_cardinality(&art(), &empty).is_err(), "缺件");

        let mut invented = ledger_with("前缀 守卫", "image/png");
        invented.entries.push(LedgerEntry {
            asset_id: "T_Ghost".into(),
            ..invented.entries[0].clone()
        });
        assert!(verify_cardinality(&art(), &invented).is_err(), "多产/发明");

        assert!(verify_cardinality(&art(), &ledger_with("前缀 守卫", "image/png")).is_ok());
    }

    /// serde 往返 + 旧档兼容。
    #[test]
    fn record_round_trips_and_legacy_parses() {
        let record = assemble_record(
            1,
            "sha256:anchor",
            ledger_with("前缀 守卫", "image/png"),
            AssetGenome::default(),
            Vec::new(),
            Vec::new(),
            "now",
        );
        let json = serde_json::to_string(&record).expect("序列化");
        let back: AssetProductionRecord = serde_json::from_str(&json).expect("反序列化");
        assert_eq!(back, record);

        let legacy: AssetProductionRecord = serde_json::from_str("{}").expect("旧档");
        assert_eq!(legacy.anchor_version, 0);
        assert!(
            legacy.clean(),
            "空记录没有差异，但也没有台账——由基数对账去拦"
        );
    }
}
