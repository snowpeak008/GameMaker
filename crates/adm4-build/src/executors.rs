//! P0 / P2 的真实执行器（G3 落地；P1/P3-P5 仍是诚实空实现）。
//!
//! - **P0 两条线派生**：读 Phase 1 的 C3/C4 产物 + `GameSpec` 真源 → 程序线契约 / 美术线契约 /
//!   资产表初版 / 对齐报告，全部过权威顺序校验后落成 P0 契约。引擎工程种子归 G4：
//!   契约里如实写 `engine_seed.status = pending_g4`，**不假装完成**——它的唯一消费者 P1
//!   本波仍诚实 Blocked，没有任何下游会被这份"部分成功"误导。
//! - **P2 资产批量生产**：风格硬门（`StyleReadiness::require_ready`）→ 预算人工门（R3）→
//!   逐资产生产（缓存命中零调用、真调用扣预算额度）→ 台账 → 基因表回填对账 →
//!   确定性一致性比对 → 干净才放行（Block 级漂移进修复队列并阻断）。

use crate::art::asset_producer::{
    AiImageAssetProducer, AssetProducer, ExternalToolProducer, ProduceRequest, build_prompt,
    select_producer,
};
use crate::art::budget::{AssetBudget, BudgetStatus};
use crate::art::cache::{CacheKeyInput, ProductionCache};
use crate::art::genome_backfill::{
    AssetProductionLedger, LedgerEntry, ProductionSource, assemble_record, backfill_genome,
    deterministic_drift_checks, used_by_from_alignment, verify_cardinality,
};
use crate::art::style_anchor::StyleAnchorStore;
use crate::governance::alignment::AlignmentReport;
use crate::governance::art_line::ArtContract;
use crate::governance::asset_registry::{AssetRegistry, NamingRules, NamingViolation};
use crate::governance::authority_order::{
    AuthorityOrderInput, AuthorityOrderReport, MarkdownDocument, validate_authority_order,
};
use crate::governance::program_line::ProgramContract;
use crate::program::derive::{DeriveInput, derive_two_lines};
use crate::runner::{BuildContext, StageExecutor};
use adm4_ai::ImageProvider;
use adm4_foundation::{
    Adm4Error, Adm4Result, UtcTimestamp, atomic_write, ensure_dir, read_json_file, write_json_file,
};
use adm4_pipeline::{ArtifactStore, CapabilitiesContract, ContentInventoryContract, StageStatus};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

fn now_iso() -> String {
    UtcTimestamp::now().to_iso8601()
}

/// 引擎工程种子的诚实状态（P0 契约字段；G4 落地后改为真种子）。
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct EngineSeedStatus {
    /// `pending_g4` = 本波未产（唯一消费者 P1 也未实现，无下游被误导）。
    pub status: String,
    pub note: String,
}

/// P0 落盘契约：两条线 + 命名权威 + 对齐 + 权威顺序自检。
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct TwoLineContract {
    pub schema_version: String,
    pub generated_at: String,
    pub engine_seed: EngineSeedStatus,
    pub program: ProgramContract,
    pub art: ArtContract,
    pub registry: AssetRegistry,
    pub alignment: AlignmentReport,
    pub authority: AuthorityOrderReport,
    pub naming_violations: Vec<NamingViolation>,
}

/// P0 执行器：两条线派生 + 对齐合流。
pub struct P0Executor {
    /// Phase 1 的产物仓（读 C3/C4；**只读**，P0 的产物写进构建仓）。
    design: ArtifactStore,
}

impl P0Executor {
    pub fn new(design: ArtifactStore) -> Self {
        Self { design }
    }
}

impl StageExecutor for P0Executor {
    fn stage_id(&self) -> &str {
        "P0"
    }

    fn execute(&self, ctx: &BuildContext<'_>) -> Adm4Result<StageStatus> {
        // C3/C4 是消费的设计文档契约集：缺了如实指路，不就地重算（重算会绕开
        // C3 的白名单阻塞与基数人工门，R2/R6 都白设了）。
        let content: ContentInventoryContract =
            self.design.read_contract("C3").map_err(|error| {
                Adm4Error::blocked(format!(
                    "读不到 C3 内容与资产需求（{}）：请先跑 pipeline run <项目> --to C3",
                    error.message
                ))
            })?;
        let capabilities: CapabilitiesContract =
            self.design.read_contract("C4").map_err(|error| {
                Adm4Error::blocked(format!(
                    "读不到 C4 程序需求与架构（{}）：请先跑 pipeline run <项目> --to C4",
                    error.message
                ))
            })?;

        let generated_at = now_iso();
        let derivation = derive_two_lines(&DeriveInput {
            spec: ctx.spec,
            content: &content,
            capabilities: &capabilities,
            generated_at: &generated_at,
        })?;

        // 命名机制核对（禁止词根属品类包内容，BuildContext 不带包——本波只跑机制面，
        // 换皮词已由落盘钩子的 SkinScanner 兜住；见任务报告的如实登记）。
        let naming_violations = derivation
            .registry
            .naming_violations(&NamingRules::default());

        let mut reasons: Vec<String> = Vec::new();
        for gap in derivation.program.envelope.blocking_gaps() {
            reasons.push(format!("程序线阻塞级缺失：{}", gap.missing_fact));
        }
        for gap in derivation.art.envelope.blocking_gaps() {
            reasons.push(format!("美术线阻塞级缺失：{}", gap.missing_fact));
        }
        if !derivation.alignment.is_clean() {
            for conflict in &derivation.alignment.unresolved_conflicts {
                reasons.push(format!(
                    "对齐冲突 {}（{}）：{}",
                    conflict.conflict_id,
                    conflict.asset_id,
                    conflict.differences.join("；")
                ));
            }
            for missing in &derivation.alignment.missing_for_program {
                reasons.push(format!(
                    "程序侧缺件：{}（{}）",
                    missing.asset_id, missing.reason
                ));
            }
        }
        for violation in &naming_violations {
            reasons.push(format!(
                "命名违规 {}（{:?}）：{}",
                violation.asset_id, violation.code, violation.detail
            ));
        }

        let document =
            render_p0_document(&derivation.alignment, &derivation.program, &derivation.art);
        // 权威顺序自检：P0 自己渲染的 Markdown 也要过铁律③——渲染层不得携带契约里没有的事实。
        let authority = validate_authority_order(&AuthorityOrderInput {
            spec: ctx.spec,
            program: &derivation.program,
            art: &derivation.art,
            registry: &derivation.registry,
            markdown: &[MarkdownDocument::new("P0/document.md", &document)],
            naming: &NamingRules::default(),
        });
        if !authority.passed() {
            for finding in authority.blocking_findings() {
                reasons.push(format!(
                    "权威顺序违例 [{:?}] {}：{}",
                    finding.code, finding.subject, finding.detail
                ));
            }
        }

        let contract = TwoLineContract {
            schema_version: crate::governance::GOVERNANCE_SCHEMA_VERSION.to_string(),
            generated_at,
            engine_seed: EngineSeedStatus {
                status: "pending_g4".to_string(),
                note:
                    "引擎工程种子归 G4（册 09）：本波不产、不占位；其唯一消费者 P1 仍诚实 Blocked"
                        .to_string(),
            },
            program: derivation.program,
            art: derivation.art,
            registry: derivation.registry,
            alignment: derivation.alignment,
            authority,
            naming_violations,
        };
        // 产物先落盘再定状态：Blocked 时人要能打开报告看是哪几条冲突（产物即证据）。
        ctx.store.write_stage("P0", &contract, &document)?;

        if reasons.is_empty() {
            Ok(StageStatus::Succeeded)
        } else {
            Ok(StageStatus::Blocked { reasons })
        }
    }
}

fn render_p0_document(
    alignment: &AlignmentReport,
    program: &ProgramContract,
    art: &ArtContract,
) -> String {
    let mut document = format!(
        "# P0 两条线派生与对齐合流\n\n- 程序线：系统 {} 个 / 能力 {} 条 / 资产依赖 {} 条\n- 美术线：资产 {} 个\n- {}\n- 引擎工程种子：待 G4（本段如实未产）\n\n",
        program.systems.len(),
        program.capabilities.len(),
        program.asset_dependencies.len(),
        art.assets.len(),
        alignment.summary(),
    );
    if !alignment.unresolved_conflicts.is_empty() {
        document.push_str("## 待人工裁决的冲突\n\n");
        for conflict in &alignment.unresolved_conflicts {
            document.push_str(&format!(
                "- {}：程序要 {}，美术给 {}\n",
                conflict.conflict_id, conflict.program_requires, conflict.art_provides
            ));
        }
        document.push('\n');
    }
    document
}

/// P2 执行器：资产批量生产。
pub struct P2Executor {
    /// 风格门产物根（`content/style`）。
    style_root: PathBuf,
    /// 图像通道（None = 未配置；到 P2 才如实 Blocked，不在 P0 就拦住整条产线）。
    images: Option<Box<dyn ImageProvider>>,
    /// 未配置时的原因（原样呈现给用户）。
    image_hint: String,
    width: u32,
    height: u32,
}

impl P2Executor {
    pub fn new(
        style_root: PathBuf,
        images: Option<Box<dyn ImageProvider>>,
        image_hint: String,
        width: u32,
        height: u32,
    ) -> Self {
        Self {
            style_root,
            images,
            image_hint,
            width,
            height,
        }
    }
}

/// 预算文件名（P2 与门面共用一份路径口径）。
pub const BUDGET_FILE: &str = "asset_budget.json";

/// 读构建仓里的预算（没有 = 还没申报）。
pub fn load_budget(store: &ArtifactStore) -> Adm4Result<Option<AssetBudget>> {
    let path = store.root.join(BUDGET_FILE);
    if !path.is_file() {
        return Ok(None);
    }
    Ok(Some(read_json_file(&path)?))
}

/// 写预算（原子写；执行器与门面的批准都走这里）。
pub fn save_budget(store: &ArtifactStore, budget: &AssetBudget) -> Adm4Result<()> {
    write_json_file(&store.root.join(BUDGET_FILE), budget)
}

impl StageExecutor for P2Executor {
    fn stage_id(&self) -> &str {
        "P2"
    }

    fn execute(&self, ctx: &BuildContext<'_>) -> Adm4Result<StageStatus> {
        // 上游契约：P0 的两条线（depends_on 已保证 P0 成功，这里读的是产物本体）。
        let p0: TwoLineContract = ctx.store.read_contract("P0").map_err(|error| {
            Adm4Error::blocked(format!("读不到 P0 两条线契约（{}）", error.message))
        })?;

        // 风格硬门：外部输入「风格锚点集」实读实校（不信状态位）。
        let style = StyleAnchorStore::new(self.style_root.clone());
        let readiness = style.readiness()?;
        readiness.require_ready()?;
        let anchor_version = style.latest_anchor_version()?;
        let anchor_set = style.load_anchor_set(anchor_version)?;
        let contract = style.load_application_contract(anchor_version)?;
        contract.matches(&anchor_set)?;

        // 预算人工门（R3）：首次到达自动申报并停下等署名；批准后才生产。
        let mut budget = match load_budget(ctx.store)? {
            Some(existing) => existing,
            None => {
                let declared = AssetBudget::declare(
                    p0.art
                        .assets
                        .iter()
                        .map(|asset| asset.asset_id.clone())
                        .collect(),
                    p0.art.assets.len(),
                )?;
                save_budget(ctx.store, &declared)?;
                return Ok(StageStatus::Blocked {
                    reasons: vec![format!(
                        "资产预算已申报待批准：{}（用 build budget-confirm <项目> <署名> <结论> 批准后重跑本段）",
                        declared.summary()
                    )],
                });
            }
        };
        if budget.status == BudgetStatus::Draft {
            return Ok(StageStatus::Blocked {
                reasons: vec![format!(
                    "资产预算未批准：{}（R3 首次付费确认）",
                    budget.summary()
                )],
            });
        }

        // 图像通道：到这一步才要求它在——前面的门都过了，缺的就只剩通道本身。
        let Some(images) = self.images.as_deref() else {
            return Ok(StageStatus::Blocked {
                reasons: vec![format!("图像生成通道不可用：{}", self.image_hint)],
            });
        };

        let ai = AiImageAssetProducer::new(images, &ctx.store.scanner, self.width, self.height);
        let external = ExternalToolProducer::default();
        let producers: [&dyn AssetProducer; 2] = [&ai, &external];
        let cache_root = ctx.store.root.join("asset_cache");
        ensure_dir(&cache_root)?;
        let cache = ProductionCache::new(cache_root);
        let assets_root = ctx.store.root.join("assets");

        let mut ledger = AssetProductionLedger::new();
        let produced_at = now_iso();
        for asset in &p0.art.assets {
            let registered = p0.registry.require_entry(&asset.asset_id)?;
            let producer = select_producer(&producers, asset)?;
            let request = ProduceRequest {
                asset,
                registered,
                contract: &contract,
            };
            let prompt = build_prompt(&request)?;
            let key = CacheKeyInput {
                asset_id: &asset.asset_id,
                prompt: &prompt,
                width: self.width,
                height: self.height,
                provider_id: images.id(),
                model: "configured",
            }
            .key()?;

            let (bytes, media_type, provider_id, model, source, calls) = match cache.lookup(&key)? {
                Some((bytes, meta)) => {
                    ledger.cache_hits += 1;
                    (
                        bytes,
                        meta.media_type,
                        meta.provider_id,
                        meta.model,
                        ProductionSource::Cache,
                        0usize,
                    )
                }
                None => {
                    // 真调用前问预算；额度不够先把预算状态落盘再停（下次续跑接得上）。
                    if let Err(error) = budget.authorize_call() {
                        save_budget(ctx.store, &budget)?;
                        return Ok(StageStatus::Blocked {
                            reasons: vec![error.message],
                        });
                    }
                    let produced = match producer.produce(&request) {
                        Ok(produced) => produced,
                        Err(error) => {
                            // 生产失败原样上抛（R7）；预算不为失败扣费。
                            save_budget(ctx.store, &budget)?;
                            return Err(error);
                        }
                    };
                    budget.consume_call()?;
                    ledger.cache_misses += 1;
                    ledger.generation_calls += 1;
                    cache.store(
                        &key,
                        &produced.bytes,
                        &produced.media_type,
                        &produced.provider_id,
                        &produced.model,
                    )?;
                    (
                        produced.bytes,
                        produced.media_type,
                        produced.provider_id,
                        produced.model,
                        ProductionSource::Ai,
                        1usize,
                    )
                }
            };

            // 落到暂存资产根（G4 的 P3 装配从这里取；路径 = 资产表登记的运行时加载路径）。
            let target = assets_root.join(&registered.runtime_path);
            if let Some(parent) = target.parent() {
                ensure_dir(parent)?;
            }
            atomic_write(&target, &bytes)?;

            let file_name = registered
                .runtime_path
                .rsplit(['/', '\\'])
                .next()
                .unwrap_or(registered.runtime_path.as_str())
                .to_string();
            ledger.entries.push(LedgerEntry {
                asset_id: asset.asset_id.clone(),
                file_name,
                purpose: asset.purpose.clone(),
                runtime_path: registered.runtime_path.clone(),
                in_game_size: None,
                generation_calls: calls,
                source,
                fallback: "无（生成失败即停，R7 禁兜底）".to_string(),
                used_by: used_by_from_alignment(&p0.alignment, &asset.asset_id),
                prompt,
                bytes_sha256: adm4_foundation::sha256_hex(&bytes),
                bytes: bytes.len() as u64,
                media_type,
                provider_id,
                model,
                produced_at: produced_at.clone(),
            });
        }
        save_budget(ctx.store, &budget)?;

        // 收口三连：基数对账（R6）→ 基因表回填对账 → 确定性一致性比对。
        verify_cardinality(&p0.art, &ledger)?;
        let (genome, genome_drifts) = backfill_genome(&ledger, &p0.registry)?;
        let drift_checks = deterministic_drift_checks(&p0.art, &ledger, &contract.prompt_prefix);
        let record = assemble_record(
            anchor_version,
            &contract.source_anchor_hash,
            ledger,
            genome,
            genome_drifts,
            drift_checks,
            &produced_at,
        );

        let document = format!(
            "# P2 资产批量生产\n\n- {}\n- {}\n- 风格锚点：v{}（契约哈希 {}）\n- 基因表对账差异：{} 条\n- 修复队列：{} 个\n\n> {}\n",
            record.ledger.summary(),
            budget.summary(),
            record.anchor_version,
            record.source_anchor_hash,
            record.genome_drifts.len(),
            record.repair_queue.len(),
            record.visual_review_note,
        );
        ctx.store.write_stage("P2", &record, &document)?;

        if record.clean() {
            Ok(StageStatus::Succeeded)
        } else {
            let mut reasons: Vec<String> = record
                .genome_drifts
                .iter()
                .map(|drift| {
                    format!(
                        "基因表对账 {:?}：{}（{}）",
                        drift.kind, drift.asset_id, drift.detail
                    )
                })
                .collect();
            reasons.extend(
                record
                    .repair_queue
                    .iter()
                    .map(|asset_id| format!("Block 级漂移：{asset_id} 需重生成（修复队列）")),
            );
            Ok(StageStatus::Blocked { reasons })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// P0 契约 serde 往返 + 旧档兼容（缺全部新键仍可读，engine_seed 落空态）。
    #[test]
    fn two_line_contract_round_trips_and_legacy_parses() {
        let contract = TwoLineContract {
            schema_version: "4.0.0".into(),
            generated_at: "now".into(),
            engine_seed: EngineSeedStatus {
                status: "pending_g4".into(),
                note: "待 G4".into(),
            },
            ..TwoLineContract::default()
        };
        let json = serde_json::to_string(&contract).expect("序列化");
        let back: TwoLineContract = serde_json::from_str(&json).expect("反序列化");
        assert_eq!(back, contract);

        let legacy: TwoLineContract = serde_json::from_str("{}").expect("旧档");
        assert!(legacy.engine_seed.status.is_empty());
    }
}
