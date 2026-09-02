//! P0 / P1 / P2 的真实执行器（P3-P5 仍是诚实空实现）。
//!
//! - **P0 两条线派生**：读 Phase 1 的 C3/C4 产物 + `GameSpec` 真源 → 程序线契约 / 美术线契约 /
//!   资产表初版 / 对齐报告，全部过权威顺序校验后落成 P0 契约。引擎工程种子（G4a）由
//!   真源派生：`engine_id` 来自配置（未配置即 `"none"`，不写死任何引擎），工程目录名由
//!   项目 id 派生；派生不出来就如实写 `engine_seed.status = blocked` 并阻塞。
//! - **P1 可玩切片现场开发**：读 P0 契约 → 确定性抽切片 → 渲运行时清单 → 取引擎指南 →
//!   durable docs + 轮次日志落到工作区 → 引擎预检（未就绪**不跑**开发）→ 开/建工程 →
//!   现场开发一轮 → 轮次进日志。四制品经 `write_stage("P1", ..)` 落盘；Blocked 时产物
//!   照落（产物即证据）。
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
use crate::engine::{DevContext, DevRoundStatus, EngineBackend, EngineProjectSeed, SliceTask};
use crate::governance::CoverageGap;
use crate::governance::alignment::AlignmentReport;
use crate::governance::art_line::ArtContract;
use crate::governance::asset_registry::{AssetRegistry, NamingRules, NamingViolation};
use crate::governance::authority_order::{
    AuthorityOrderInput, AuthorityOrderReport, MarkdownDocument, validate_authority_order,
};
use crate::governance::program_line::ProgramContract;
use crate::program::derive::{DeriveInput, derive_two_lines};
use crate::program::dev_round::{DevRoundLog, RoundStatus, render_durable_docs};
use crate::program::engine_guide::{EngineGuide, EngineGuideSource};
use crate::program::manifest::{RuntimeManifest, render_runtime_manifest};
use crate::program::slice::{PlayableSlice, RiskSlicePlan, extract_playable_slice};
use crate::runner::{BuildContext, StageExecutor};
use adm4_ai::ImageProvider;
use adm4_contracts::SpecRef;
use adm4_foundation::{
    Adm4Error, Adm4Result, UtcTimestamp, atomic_write, ensure_dir, read_json_file, write_json_file,
};
use adm4_pipeline::{ArtifactStore, CapabilitiesContract, ContentInventoryContract, StageStatus};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

fn now_iso() -> String {
    UtcTimestamp::now().to_iso8601()
}

/// 未配置引擎时种子上登记的后端标识（只是字符串占位，不指向任何具体引擎）。
pub const DEFAULT_ENGINE_ID: &str = "none";

/// P1 工作区在构建仓段目录下的相对位置（durable docs / 轮次日志 / 引擎工程都落这里）。
pub const P1_WORKSPACE_DIR: &str = "workspace";

/// 引擎工程种子的状态包装（P0 契约字段）。
///
/// 类型保留是为了读旧档：G3 产的契约里 `status = pending_g4` 且没有 `seed`，
/// 用 `#[serde(default)]` 照样能读进来，`seed` 落 `None`；P1 读到 `None` 时如实阻塞指路重跑 P0。
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct EngineSeedStatus {
    /// `produced` = 真种子已派生（见 `seed`）；`pending_g4` = G3 旧档（未产）；`blocked` = 派生失败。
    pub status: String,
    pub note: String,
    /// 真种子；旧档缺键即 `None`。
    pub seed: Option<EngineProjectSeed>,
}

/// 由项目 id 派生 snake_case 的工程目录名（只保留 ASCII 字母数字，其余折成下划线）。
///
/// 目录名要进文件系统与 Mock 的 `ensure_within_root`，因此不能含分隔符/点号/空白；
/// 派生不出一个非空名字就 `Err`——工程目录名没有"合理默认值"（R2）。
pub fn derive_project_dir_name(project_id: &str) -> Adm4Result<String> {
    let mut name = String::new();
    let mut last_was_separator = true;
    for ch in project_id.trim().chars() {
        if ch.is_ascii_alphanumeric() {
            name.extend(ch.to_lowercase());
            last_was_separator = false;
        } else if !last_was_separator {
            name.push('_');
            last_was_separator = true;
        }
    }
    let name = name.trim_end_matches('_').to_string();
    if name.is_empty() {
        return Err(Adm4Error::blocked(format!(
            "无法从项目 id「{project_id}」派生引擎工程目录名：不含任何 ASCII 字母或数字（请在真源里给项目一个可作目录名的 id）"
        )));
    }
    Ok(format!("{name}_playable"))
}

/// 从真源派生引擎工程种子（确定性；锚定 `identity.project_id` 与 `intent`）。
pub fn derive_engine_seed(
    spec: &adm4_spec::GameSpec,
    engine_id: &str,
) -> Adm4Result<EngineProjectSeed> {
    let engine_id = engine_id.trim();
    if engine_id.is_empty() {
        return Err(Adm4Error::blocked(
            "引擎后端标识为空：未配置引擎时请显式使用 \"none\"，不得留空",
        ));
    }
    let project_dir_name = derive_project_dir_name(&spec.identity.project_id)?;
    let notes = if engine_id == DEFAULT_ENGINE_ID {
        "未配置引擎后端：种子只记录工程目录名与锚点，P1 预检将如实阻塞".to_string()
    } else {
        format!("引擎后端 {engine_id} 由配置指定；seed_kind 与 required_tools 由该后端解释")
    };
    Ok(EngineProjectSeed {
        engine_id: engine_id.to_string(),
        project_dir_name,
        seed_kind: "empty_project".to_string(),
        required_tools: Vec::new(),
        notes,
        // 锚点形态与 `GameSpec::contains_ref` 认的一致：顶层段只有 `identity` / `intent` 两个。
        anchors: vec![SpecRef::new("identity"), SpecRef::new("intent")],
    })
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
    /// 种子上登记的引擎后端标识（来自配置；未配置即 [`DEFAULT_ENGINE_ID`]）。
    engine_id: String,
}

impl P0Executor {
    /// 未配置引擎的装配：种子 `engine_id = "none"`。
    pub fn new(design: ArtifactStore) -> Self {
        Self::with_engine_id(design, DEFAULT_ENGINE_ID)
    }

    /// 指定引擎后端标识的装配（门面按配置传入）。
    pub fn with_engine_id(design: ArtifactStore, engine_id: &str) -> Self {
        Self {
            design,
            engine_id: engine_id.to_string(),
        }
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

        // 引擎工程种子：派生失败是阻塞事实而非异常——契约照落、状态如实写 blocked。
        let engine_seed = match derive_engine_seed(ctx.spec, &self.engine_id) {
            Ok(seed) => EngineSeedStatus {
                status: "produced".to_string(),
                note: format!(
                    "引擎工程种子由真源派生：后端 {}，工程目录 {}",
                    seed.engine_id, seed.project_dir_name
                ),
                seed: Some(seed),
            },
            Err(error) => {
                reasons.push(format!("引擎工程种子派生失败：{}", error.message));
                EngineSeedStatus {
                    status: "blocked".to_string(),
                    note: error.message,
                    seed: None,
                }
            }
        };

        let document = render_p0_document(
            &derivation.alignment,
            &derivation.program,
            &derivation.art,
            &engine_seed,
        );
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
            engine_seed,
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
    engine_seed: &EngineSeedStatus,
) -> String {
    let seed_line = match &engine_seed.seed {
        Some(seed) => format!(
            "引擎工程种子：已派生（后端 {}，工程目录 {}）",
            seed.engine_id, seed.project_dir_name
        ),
        None => format!(
            "引擎工程种子：{}（{}）",
            engine_seed.status, engine_seed.note
        ),
    };
    let mut document = format!(
        "# P0 两条线派生与对齐合流\n\n- 程序线：系统 {} 个 / 能力 {} 条 / 资产依赖 {} 条\n- 美术线：资产 {} 个\n- {}\n- {}\n\n",
        program.systems.len(),
        program.capabilities.len(),
        program.asset_dependencies.len(),
        art.assets.len(),
        alignment.summary(),
        seed_line,
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

/// P1 落盘契约：切片 + 风险计划 + 运行时清单 + 指南（可缺）+ 轮次日志 + 缺口 + 所用种子。
///
/// `guide` 为 `None` 是一条如实记录（指南来源未提供，归引擎插件波次），不是"默认值"。
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct P1Contract {
    pub schema_version: String,
    pub generated_at: String,
    pub slice: PlayableSlice,
    pub risk_plan: RiskSlicePlan,
    pub manifest: RuntimeManifest,
    pub guide: Option<EngineGuide>,
    pub dev_round_log: DevRoundLog,
    pub gaps: Vec<CoverageGap>,
    pub engine_seed: EngineProjectSeed,
    /// 引擎预检结论原文（就绪与否都记；未就绪时 P1 不跑开发，只留证据）。
    pub preflight_detail: String,
    /// 本次运行的阻塞原因（与运行状态里的一致，方便只读契约的呈现层）。
    pub blocked_reasons: Vec<String>,
}

/// 后端轮次记录 → 轮次日志记录。
///
/// 两侧同名不同型是刻意的（后端不读时钟、不分配序号），这里是唯一的翻译点：
/// `index` 交给 `append_round` 重分配，时间戳由本层填。
pub fn engine_round_to_log_round(
    round: &crate::engine::DevRound,
    started_at: &str,
    finished_at: &str,
) -> crate::program::dev_round::DevRound {
    crate::program::dev_round::DevRound {
        index: round.index,
        started_at: started_at.to_string(),
        finished_at: finished_at.to_string(),
        commands: round.commands.clone(),
        failures: round.failures.clone(),
        repair_summary: round.repair_summary.clone(),
        status: match round.status {
            DevRoundStatus::InProgress => RoundStatus::Running,
            DevRoundStatus::Succeeded => RoundStatus::Succeeded,
            DevRoundStatus::Failed => RoundStatus::Failed,
            DevRoundStatus::Aborted => RoundStatus::Aborted,
        },
    }
}

/// P1 执行器：可玩切片 + 薄运行时清单 + 引擎指南 + 现场开发一轮。
pub struct P1Executor {
    backend: Box<dyn EngineBackend>,
    guide: Box<dyn EngineGuideSource>,
    /// P1 工作区父目录（门面传构建仓的 P1 段目录；工作区 = `<root>/workspace`）。
    workspace_root: PathBuf,
}

impl P1Executor {
    pub fn new(
        backend: Box<dyn EngineBackend>,
        guide: Box<dyn EngineGuideSource>,
        workspace_root: PathBuf,
    ) -> Self {
        Self {
            backend,
            guide,
            workspace_root,
        }
    }

    /// 工作区目录（durable docs / 轮次日志 / 引擎工程的父目录）。
    pub fn workspace_dir(&self) -> PathBuf {
        self.workspace_root.join(P1_WORKSPACE_DIR)
    }
}

impl StageExecutor for P1Executor {
    fn stage_id(&self) -> &str {
        "P1"
    }

    fn execute(&self, ctx: &BuildContext<'_>) -> Adm4Result<StageStatus> {
        let p0: TwoLineContract = ctx.store.read_contract("P0").map_err(|error| {
            Adm4Error::blocked(format!(
                "读不到 P0 两条线契约（{}）：请先跑 build run <项目> --to P0",
                error.message
            ))
        })?;
        let engine_seed = p0.engine_seed.seed.clone().ok_or_else(|| {
            Adm4Error::blocked(format!(
                "P0 契约里没有引擎工程种子（engine_seed.status = {}）：这是旧版 P0 产物，请 build rerun <项目> --from P0 重产",
                if p0.engine_seed.status.is_empty() {
                    "<空>"
                } else {
                    p0.engine_seed.status.as_str()
                }
            ))
        })?;

        let generated_at = now_iso();
        let mut reasons: Vec<String> = Vec::new();

        let extraction = extract_playable_slice(ctx.spec, &p0.program)?;
        let manifest = render_runtime_manifest(&extraction.slice, &extraction.risk_plan);
        let guide = match self.guide.guide() {
            Ok(guide) => Some(guide),
            Err(error) => {
                reasons.push(format!("引擎指南缺失：{}", error.message));
                None
            }
        };

        // durable docs 与轮次日志先落工作区：后面预检失败也要留下这轮的现场。
        let workspace = self.workspace_dir();
        ensure_dir(&workspace)?;
        let mut log = DevRoundLog::load(&workspace)?;
        log.durable = render_durable_docs(&extraction.slice, &manifest);
        log.durable.write_to(&workspace)?;
        write_json_file(
            &workspace.join(crate::program::manifest::RUNTIME_MANIFEST_FILE),
            &manifest,
        )?;
        write_json_file(
            &workspace.join(crate::program::manifest::PLAYABLE_SLICE_FILE),
            &extraction.slice,
        )?;
        if let Some(guide) = &guide {
            write_json_file(
                &workspace.join(crate::program::manifest::ENGINE_GUIDE_FILE),
                guide,
            )?;
        }
        log.save(&workspace)?;

        let preflight = self.backend.preflight()?;
        let preflight_detail = preflight.detail.clone();
        if preflight.ready {
            self.backend
                .open_or_create_project(&engine_seed, &workspace)?;
            let project_dir = workspace.join(&engine_seed.project_dir_name);
            let round_index = log.rounds.len() as u32;
            let task = SliceTask {
                slice_ref: format!("P1/{}", crate::program::manifest::PLAYABLE_SLICE_FILE),
                round_index,
                objective: manifest.goal.clone(),
                constraints: extraction.slice.excluded_scope.clone(),
            };
            let dev_ctx = DevContext {
                project_dir,
                manifest_path: workspace.join(crate::program::manifest::RUNTIME_MANIFEST_FILE),
                guide_path: workspace.join(crate::program::manifest::ENGINE_GUIDE_FILE),
                durable_dir: workspace.clone(),
            };
            let started_at = now_iso();
            let round = self.backend.agent_develop(&task, &dev_ctx)?;
            let finished_at = now_iso();
            let log_round = engine_round_to_log_round(&round, &started_at, &finished_at);
            let appended = log.append_round(log_round)?;
            log.save(&workspace)?;
            match round.status {
                DevRoundStatus::Succeeded => {}
                other => reasons.push(format!(
                    "第 {appended} 轮现场开发结局为 {other:?}：{}",
                    if round.failures.is_empty() {
                        round.repair_summary.clone()
                    } else {
                        round.failures.join("；")
                    }
                )),
            }
        } else {
            reasons.push(format!(
                "引擎后端 {} 未就绪，本轮不跑现场开发：{}",
                preflight.backend_id, preflight.detail
            ));
        }

        let contract = P1Contract {
            schema_version: crate::governance::GOVERNANCE_SCHEMA_VERSION.to_string(),
            generated_at,
            slice: extraction.slice,
            risk_plan: extraction.risk_plan,
            manifest,
            guide,
            dev_round_log: log,
            gaps: extraction.gaps,
            engine_seed,
            preflight_detail,
            blocked_reasons: reasons.clone(),
        };
        let document = render_p1_document(&contract, &workspace);
        ctx.store.write_stage("P1", &contract, &document)?;

        if reasons.is_empty() {
            Ok(StageStatus::Succeeded)
        } else {
            Ok(StageStatus::Blocked { reasons })
        }
    }
}

fn render_p1_document(contract: &P1Contract, workspace: &Path) -> String {
    let mut document = format!(
        "# P1 可玩切片现场开发\n\n- 场景：{}\n- 核心循环：{}\n- 主操作：{}\n- 成败状态：{}\n- 风险项：{} 条\n- 事实缺口：{} 条\n- 引擎指南：{}\n- 引擎后端：{}（预检：{}）\n- 开发轮次：{} 轮\n- 工作区：{}\n\n",
        contract.slice.scene,
        contract.slice.core_loop,
        contract.slice.primary_input.join(" / "),
        contract.slice.success_or_fail_state,
        contract.risk_plan.items.len(),
        contract.gaps.len(),
        if contract.guide.is_some() {
            "已提供"
        } else {
            "未提供（归引擎插件波次）"
        },
        contract.engine_seed.engine_id,
        contract.preflight_detail,
        contract.dev_round_log.rounds.len(),
        workspace.display(),
    );
    document.push_str(&contract.manifest.to_markdown());
    document.push('\n');
    if !contract.dev_round_log.rounds.is_empty() {
        document.push_str("## 开发轮次\n\n");
        for round in &contract.dev_round_log.rounds {
            document.push_str(&format!(
                "- 第 {} 轮：{}；命令 {} 条；失败 {} 条\n",
                round.index,
                round.status.label(),
                round.commands.len(),
                round.failures.len()
            ));
        }
        document.push('\n');
    }
    if !contract.blocked_reasons.is_empty() {
        document.push_str("## 阻塞原因\n\n");
        for reason in &contract.blocked_reasons {
            document.push_str(&format!("- {reason}\n"));
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
                status: "produced".into(),
                note: "已派生".into(),
                seed: Some(EngineProjectSeed {
                    engine_id: "none".into(),
                    project_dir_name: "demo_playable".into(),
                    ..EngineProjectSeed::default()
                }),
            },
            ..TwoLineContract::default()
        };
        let json = serde_json::to_string(&contract).expect("序列化");
        let back: TwoLineContract = serde_json::from_str(&json).expect("反序列化");
        assert_eq!(back, contract);

        let legacy: TwoLineContract = serde_json::from_str("{}").expect("旧档");
        assert!(legacy.engine_seed.status.is_empty());
        assert!(legacy.engine_seed.seed.is_none());
    }

    /// G3 产的 P0 契约（`pending_g4`、无 `seed` 键）仍可读，seed 落 None。
    #[test]
    fn legacy_pending_g4_seed_status_still_parses() {
        let json = r#"{"engine_seed":{"status":"pending_g4","note":"引擎工程种子归 G4"}}"#;
        let legacy: TwoLineContract = serde_json::from_str(json).expect("旧档");
        assert_eq!(legacy.engine_seed.status, "pending_g4");
        assert!(legacy.engine_seed.seed.is_none());
    }

    #[test]
    fn project_dir_name_is_snake_case_and_rejects_empty() {
        assert_eq!(
            derive_project_dir_name("Demo Project-2").expect("派生"),
            "demo_project_2_playable"
        );
        assert_eq!(
            derive_project_dir_name("  abc  ").expect("派生"),
            "abc_playable"
        );
        assert_eq!(
            derive_project_dir_name("../..")
                .expect_err("无字母数字")
                .kind,
            adm4_foundation::Adm4ErrorKind::Blocked
        );
        assert_eq!(
            derive_project_dir_name("项目")
                .expect_err("纯非 ASCII")
                .kind,
            adm4_foundation::Adm4ErrorKind::Blocked
        );
    }

    #[test]
    fn engine_seed_derivation_is_deterministic_and_anchored() {
        let spec = adm4_spec::GameSpec {
            identity: adm4_spec::SpecIdentity {
                schema_version: "4.0.0".into(),
                project_id: "demo".into(),
                frozen_hash: "sha256:frozen".into(),
            },
            intent: adm4_spec::ProjectIntent::default(),
            systems: Vec::new(),
            mechanics: Vec::new(),
            entities: Vec::new(),
            tables: Vec::new(),
            content: Vec::new(),
            acceptance: Vec::new(),
            source_map: Vec::new(),
        };
        let first = derive_engine_seed(&spec, "none").expect("派生");
        let second = derive_engine_seed(&spec, "none").expect("派生");
        assert_eq!(first, second);
        assert_eq!(first.engine_id, "none");
        assert_eq!(first.project_dir_name, "demo_playable");
        assert_eq!(
            first.anchors,
            vec![SpecRef::new("identity"), SpecRef::new("intent")]
        );
        assert!(
            derive_engine_seed(&spec, "  ").is_err(),
            "空 engine_id 拒绝"
        );
        let configured = derive_engine_seed(&spec, "engine_x").expect("派生");
        assert_eq!(configured.engine_id, "engine_x");
    }

    /// 后端轮次 → 日志轮次：四种状态一一对应，命令/失败原文不丢。
    #[test]
    fn engine_round_converts_to_log_round_faithfully() {
        for (from, to) in [
            (DevRoundStatus::InProgress, RoundStatus::Running),
            (DevRoundStatus::Succeeded, RoundStatus::Succeeded),
            (DevRoundStatus::Failed, RoundStatus::Failed),
            (DevRoundStatus::Aborted, RoundStatus::Aborted),
        ] {
            let round = crate::engine::DevRound {
                index: 3,
                commands: vec!["build".into()],
                failures: vec!["err".into()],
                repair_summary: "fix".into(),
                status: from,
            };
            let converted = engine_round_to_log_round(&round, "t0", "t1");
            assert_eq!(converted.status, to);
            assert_eq!(converted.index, 3);
            assert_eq!(converted.commands, vec!["build".to_string()]);
            assert_eq!(converted.failures, vec!["err".to_string()]);
            assert_eq!(converted.repair_summary, "fix");
            assert_eq!(converted.started_at, "t0");
            assert_eq!(converted.finished_at, "t1");
        }
    }

    #[test]
    fn p1_contract_round_trips_and_legacy_parses() {
        let contract = P1Contract {
            schema_version: "4.0.0".into(),
            preflight_detail: "未就绪".into(),
            blocked_reasons: vec!["x".into()],
            ..P1Contract::default()
        };
        let json = serde_json::to_string(&contract).expect("序列化");
        let back: P1Contract = serde_json::from_str(&json).expect("反序列化");
        assert_eq!(back, contract);
        let legacy: P1Contract = serde_json::from_str("{}").expect("旧档");
        assert!(legacy.guide.is_none());
        assert!(legacy.dev_round_log.rounds.is_empty());
    }

    // -----------------------------------------------------------------------
    // P1 执行器集成测试（T-G4a-3 裁决 2）：用恰好 1 主操作的切片夹具 + 手工写入的 P0 契约，
    // 在切片可抽出的前提下验证后端门控——Mock 就绪 / 预检未就绪 / 未配置引擎。
    // lane_defense 全链夹具天然派生 3 个主操作候选（设计侧未收敛），故不在 e2e 里靶它。
    // -----------------------------------------------------------------------

    mod p1_integration {
        use super::super::*;
        use crate::engine::{MockCall, MockEngineBackend, MockEngineScript, NotConfiguredBackend};
        use crate::program::engine_guide::NotProvidedGuide;
        use crate::program::slice::test_fixtures;
        use adm4_contracts::SkinScanner;
        use std::sync::Arc;

        struct Harness {
            root: PathBuf,
            store: ArtifactStore,
            spec: adm4_spec::GameSpec,
        }

        impl Harness {
            /// 在 temp_dir 建产物仓并写入带真种子的 P0 契约（`program` 用切片夹具）。
            fn new(tag: &str, engine_id: &str) -> Self {
                let root = std::env::temp_dir().join(format!(
                    "adm4_build_p1_{tag}_{}_{}",
                    std::process::id(),
                    now_iso().replace([':', '.'], "_")
                ));
                std::fs::remove_dir_all(&root).ok();
                let store = ArtifactStore::new(root.clone(), SkinScanner::default());
                let spec = test_fixtures::spec();
                let seed = derive_engine_seed(&spec, engine_id).expect("派生种子");
                let p0 = TwoLineContract {
                    schema_version: crate::governance::GOVERNANCE_SCHEMA_VERSION.to_string(),
                    generated_at: now_iso(),
                    engine_seed: EngineSeedStatus {
                        status: "produced".into(),
                        note: "测试写入".into(),
                        seed: Some(seed),
                    },
                    program: test_fixtures::program(),
                    ..TwoLineContract::default()
                };
                store.write_stage("P0", &p0, "").expect("写 P0 契约");
                Self { root, store, spec }
            }

            fn run(&self, backend: Box<dyn EngineBackend>, engine_id: &str) -> StageStatus {
                self.run_with_guide(backend, Box::new(NotProvidedGuide::new(engine_id)))
            }

            fn run_with_guide(
                &self,
                backend: Box<dyn EngineBackend>,
                guide: Box<dyn EngineGuideSource>,
            ) -> StageStatus {
                let executor = P1Executor::new(backend, guide, self.root.join("P1"));
                let ctx = BuildContext {
                    spec: &self.spec,
                    store: &self.store,
                };
                executor.execute(&ctx).expect("P1 执行不应 Err")
            }

            fn workspace(&self) -> PathBuf {
                self.root.join("P1").join(P1_WORKSPACE_DIR)
            }

            fn p1_contract(&self) -> P1Contract {
                self.store.read_contract("P1").expect("读 P1 契约")
            }
        }

        impl Drop for Harness {
            fn drop(&mut self) {
                std::fs::remove_dir_all(&self.root).ok();
            }
        }

        /// 测试用共享后端：`MockEngineBackend` 移交给执行器后仍要能读 `calls()`。
        struct SharedMock(Arc<MockEngineBackend>);

        impl EngineBackend for SharedMock {
            fn id(&self) -> &str {
                self.0.id()
            }
            fn preflight(&self) -> Adm4Result<crate::engine::EnginePreflight> {
                self.0.preflight()
            }
            fn open_or_create_project(
                &self,
                seed: &EngineProjectSeed,
                dir: &Path,
            ) -> Adm4Result<()> {
                self.0.open_or_create_project(seed, dir)
            }
            fn agent_develop(
                &self,
                task: &SliceTask,
                ctx: &DevContext,
            ) -> Adm4Result<crate::engine::DevRound> {
                self.0.agent_develop(task, ctx)
            }
            fn run_playmode(&self, project: &Path) -> Adm4Result<crate::engine::RunResult> {
                self.0.run_playmode(project)
            }
            fn capture_proof(&self, project: &Path) -> Adm4Result<crate::engine::ProofBundle> {
                self.0.capture_proof(project)
            }
        }

        /// 测试用指南来源：Mock 后端配一页最小指南，让成功链不被「指南缺失」阻塞。
        struct ScriptedGuide(EngineGuide);

        impl EngineGuideSource for ScriptedGuide {
            fn engine_id(&self) -> &str {
                &self.0.engine_id
            }
            fn guide(&self) -> Adm4Result<EngineGuide> {
                self.0.validate()?;
                Ok(self.0.clone())
            }
        }

        fn mock_guide() -> Box<dyn EngineGuideSource> {
            use crate::program::engine_guide::{GuideCommand, GuideSection};
            Box::new(ScriptedGuide(EngineGuide {
                engine_id: "mock_engine".into(),
                sections: vec![GuideSection {
                    title: "构建".into(),
                    pitfalls: vec!["回放后端只回放脚本，不编译".into()],
                    commands: vec![GuideCommand {
                        purpose: "回放一轮".into(),
                        command: "mock: build".into(),
                    }],
                }],
            }))
        }

        fn mock_engine(ready: bool) -> Arc<MockEngineBackend> {
            Arc::new(MockEngineBackend::new(
                "mock_engine",
                MockEngineScript {
                    preflight_ready: ready,
                    rounds: vec![crate::engine::DevRound {
                        index: 0,
                        commands: vec!["mock: build".into()],
                        failures: Vec::new(),
                        repair_summary: "一轮成功".into(),
                        status: DevRoundStatus::Succeeded,
                    }],
                    ..MockEngineScript::default()
                },
            ))
        }

        /// (a) Mock 就绪 → Succeeded；四份 durable docs + 轮次日志在盘；后端被调开/建工程 + 一轮开发。
        #[test]
        fn p1_runs_full_chain_with_ready_mock_engine() {
            let harness = Harness::new("ok", "mock_engine");
            let engine = mock_engine(true);
            let status =
                harness.run_with_guide(Box::new(SharedMock(Arc::clone(&engine))), mock_guide());
            assert_eq!(status, StageStatus::Succeeded, "{status:?}");

            let workspace = harness.workspace();
            for file in [
                crate::program::manifest::DURABLE_PLAN_FILE,
                crate::program::manifest::DURABLE_STRUCTURE_FILE,
                crate::program::manifest::DURABLE_ASSETS_FILE,
                crate::program::manifest::DURABLE_PROOF_FILE,
                crate::program::dev_round::DEV_ROUND_LOG_FILE,
                crate::program::manifest::PLAYABLE_SLICE_FILE,
                crate::program::manifest::RUNTIME_MANIFEST_FILE,
                crate::program::manifest::ENGINE_GUIDE_FILE,
            ] {
                assert!(workspace.join(file).is_file(), "工作区缺 {file}");
            }
            let p1 = harness.p1_contract();
            assert_eq!(p1.slice.primary_input, vec!["cap_place_guard", "guard"]);
            assert_eq!(p1.dev_round_log.rounds.len(), 1);
            assert_eq!(
                p1.dev_round_log.rounds[0].index, 1,
                "轮次序号由日志分配（从 1 起）"
            );
            assert!(p1.blocked_reasons.is_empty());
            assert_eq!(
                p1.guide.as_ref().map(|g| g.engine_id.as_str()),
                Some("mock_engine"),
                "指南随契约落盘"
            );
            assert_eq!(p1.engine_seed.engine_id, "mock_engine");
            assert!(
                workspace
                    .join(&p1.engine_seed.project_dir_name)
                    .join(crate::engine::SEED_FILE_NAME)
                    .is_file(),
                "Mock 后端应在工程目录写下种子文件"
            );

            let calls = engine.calls();
            assert!(calls.iter().any(|call| matches!(call, MockCall::Preflight)));
            assert!(
                calls.iter().any(|call| matches!(
                    call,
                    MockCall::OpenOrCreateProject { engine_id, .. } if engine_id == "mock_engine"
                )),
                "{calls:?}"
            );
            assert!(
                calls
                    .iter()
                    .any(|call| matches!(call, MockCall::AgentDevelop { round_index: 0, .. })),
                "{calls:?}"
            );
        }

        /// (b) 预检未就绪 → Blocked，detail 透传；不开/建工程、不跑现场开发；产物照落作证据。
        #[test]
        fn p1_blocks_when_preflight_not_ready_and_skips_development() {
            let harness = Harness::new("notready", "mock_engine");
            let engine = mock_engine(false);
            let status = harness.run(Box::new(SharedMock(Arc::clone(&engine))), "mock_engine");
            match &status {
                StageStatus::Blocked { reasons } => {
                    assert!(
                        reasons.iter().any(|r| r.contains("按脚本设为未就绪")),
                        "预检 detail 必须透传：{reasons:?}"
                    );
                    assert!(
                        reasons.iter().any(|r| r.contains("引擎指南缺失")),
                        "{reasons:?}"
                    );
                }
                other => panic!("P1 应 Blocked，实际 {other:?}"),
            }
            let calls = engine.calls();
            assert!(calls.iter().any(|call| matches!(call, MockCall::Preflight)));
            assert!(
                !calls
                    .iter()
                    .any(|call| matches!(call, MockCall::AgentDevelop { .. })),
                "预检未就绪不得跑现场开发：{calls:?}"
            );
            assert!(
                !calls
                    .iter()
                    .any(|call| matches!(call, MockCall::OpenOrCreateProject { .. })),
                "预检未就绪不得开/建工程：{calls:?}"
            );
            let p1 = harness.p1_contract();
            assert!(p1.dev_round_log.rounds.is_empty());
            assert!(
                p1.preflight_detail.contains("未就绪"),
                "{}",
                p1.preflight_detail
            );
            assert!(
                harness
                    .workspace()
                    .join(crate::program::manifest::DURABLE_PLAN_FILE)
                    .is_file()
            );
        }

        /// (c) 未配置引擎 → Blocked，原因原样含 NotConfigured 的 reason 文案与后端 id。
        #[test]
        fn p1_blocks_with_not_configured_reason_when_no_engine_configured() {
            let harness = Harness::new("noengine", DEFAULT_ENGINE_ID);
            let reason = "未配置 engine_backend：请在配置里指定引擎后端";
            let backend = NotConfiguredBackend::new(DEFAULT_ENGINE_ID, reason);
            let status = harness.run(Box::new(backend), DEFAULT_ENGINE_ID);
            match &status {
                StageStatus::Blocked { reasons } => {
                    assert!(
                        reasons.iter().any(|r| r.contains(reason)),
                        "未配置引擎的原因必须原样呈现：{reasons:?}"
                    );
                    assert!(
                        reasons.iter().any(|r| r.contains(DEFAULT_ENGINE_ID)),
                        "{reasons:?}"
                    );
                }
                other => panic!("P1 应 Blocked，实际 {other:?}"),
            }
            let p1 = harness.p1_contract();
            assert_eq!(p1.engine_seed.engine_id, DEFAULT_ENGINE_ID);
            assert_eq!(p1.preflight_detail, reason);
            assert!(p1.dev_round_log.rounds.is_empty());
        }
    }
}
