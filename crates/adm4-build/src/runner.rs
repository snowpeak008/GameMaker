//! `Phase2Runner`：P0-P5 的运行骨架（册 10 §1「运行骨架层」）。
//!
//! 语义与 Phase 1 的 `PipelineRunner` **同构**——区间运行、断点续跑、协作式取消、人工门、
//! 强制重跑连带下游，一条不差。同构是刻意的：两段流水线在界面上是一张版图，用户不该在
//! C 段与 P 段之间面对两套「重跑到底会作废什么」的规则。
//!
//! ## 复用了什么、为什么有些没复用
//!
//! 直接复用 `adm4-pipeline` 的既有件：`ArtifactStore`（双格式产物 + 换皮扫描钩子）、
//! `PipelineRunState` / `StageRecord` / `StageStatus`（运行状态与断点续跑依据）、
//! `CancelSignal`（协作式取消原语）、`StageResetReport` / `RevokedConfirmation`（重置回执）、
//! `PipelineRunOutcome` / `PipelineRerunOutcome`（运行回执），以及
//! [`PipelineRunner::confirm_human_gate`]——人工门确认只认运行状态、不认 registry，
//! R3 的署名必填判定因此**没有第二份实现**。
//!
//! 没能复用的只有「区间推进」与「下游重置」两段：它们要按**本段 registry** 找位置与下游，
//! 而 `PipelineRunner` 的 registry 是写死的 C0-C6（`design_compile_registry()`），
//! 没有注入点。把它改成可注入会动到 Phase 1 的既有行为，不在本波范围内，因此这里按
//! 同一套算法对 P 段 registry 重写一遍，语义逐条对齐。
//!
//! ## 本波的执行器
//!
//! 全部是 [`PendingExecutor`]：如实返回 `Blocked` + 待哪一波实现，**绝不返回假成功**（R7）。

use crate::registry::{phase2_artifacts, validate_artifact_graph};
use adm4_foundation::{Adm4Error, Adm4ErrorKind, Adm4Result, UtcTimestamp};
use adm4_pipeline::{
    ArtifactStore, CancelSignal, PipelineRerunOutcome, PipelineRunOutcome, PipelineRunState,
    PipelineRunner, RevokedConfirmation, StageRecord, StageResetReport, StageSpec, StageStatus,
    phase2_registry,
};
use adm4_spec::GameSpec;
use std::collections::{BTreeMap, BTreeSet};

/// 阶段执行器的共享上下文。
///
/// 只给两样东西：**唯一真源**与**产物仓**。不给 AI provider——Phase 2 的治理段是确定性的，
/// 需要 AI 的生产段（G3/G4）会在自己的插件里按接缝取用，骨架不预埋 AI 通道。
pub struct BuildContext<'a> {
    /// 唯一权威真源（Phase 1 C0 的产物）。一切派生自它（D22）。
    pub spec: &'a GameSpec,
    /// Phase 2 的产物仓（与 Phase 1 的 `pipeline/v{N}` 分开，各自持有自己的 run_state）。
    pub store: &'a ArtifactStore,
}

/// 阶段执行器：插件的统一接口（册 10「弱代码耦合」的换点）。
///
/// 加一个能力 = 加一个实现并注册进来，骨架零改；插件之间不互相 import，只通过制品契约交换数据。
pub trait StageExecutor {
    /// 本执行器负责的阶段 id（必须在 registry 内）。
    fn stage_id(&self) -> &str;

    /// 执行本段。
    ///
    /// 返回 `Ok(Succeeded)` 之外的任何状态都会让运行停在本段；返回 `Err` 时由运行器按
    /// 错误类别落 `Blocked`（阻塞/AI 不可用）或 `Failed`（其余），与 Phase 1 同一套映射。
    fn execute(&self, ctx: &BuildContext<'_>) -> Adm4Result<StageStatus>;
}

/// 一段尚未实现的执行器登记：待哪一波、要补什么。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PendingStage {
    pub stage_id: &'static str,
    /// 负责实现它的波次（如 `G4`、`G3/G4`）。
    pub waves: &'static str,
    /// 要补的东西（照立项五册的落点写，后续波次照此接活）。
    pub detail: &'static str,
}

impl PendingStage {
    /// 诚实空实现的阻塞原因：说清「谁在什么时候补什么」，不写成一句「未实现」。
    pub fn blocked_reason(&self) -> String {
        format!(
            "待 {} 实现：{}（G1 只建治理骨架与插件框架，不做任何生产逻辑）",
            self.waves, self.detail
        )
    }
}

/// 尚未实现的 P 段登记表。
///
/// G3 已落地 P0（两条线派生与对齐合流）与 P2（资产生产通道 + 预算门 + 内容哈希缓存 +
/// 基因表回填）；G4a 已落地 P1（可玩切片 + 运行时清单 + 引擎指南 + 现场开发轮次）与
/// P0 的引擎工程种子。三段已从本表移除——本表只登记**还没有真实执行器**的段。
pub const PENDING_STAGES: [PendingStage; 3] = [
    PendingStage {
        stage_id: "P3",
        waves: "G4b",
        detail: "现场装配集成（资产接入 + 运行时加载路径闭环）",
    },
    PendingStage {
        stage_id: "P4",
        waves: "G5",
        detail: "运行证据捕获 + 机器预检 + 用户裁决 + 缺陷回写队列",
    },
    PendingStage {
        stage_id: "P5",
        waves: "G5",
        detail: "交付清单与确定性报告",
    },
];

/// 查一段的待实现登记（呈现层拿它显示「这段在等谁」，不在 UI 里写死文案）。
pub fn pending_stage(stage_id: &str) -> Option<&'static PendingStage> {
    PENDING_STAGES
        .iter()
        .find(|stage| stage.stage_id == stage_id)
}

/// 诚实空实现执行器：如实报 `Blocked` 与原因。
///
/// 为什么不返回 `Succeeded`：一个什么都没做却报成功的段，会让下游按「上游已就绪」继续推进，
/// 最后在最难排查的地方炸掉——那正是 R7 禁止的假成功。
pub struct PendingExecutor {
    stage: &'static PendingStage,
}

impl PendingExecutor {
    pub fn new(stage: &'static PendingStage) -> Self {
        Self { stage }
    }
}

impl StageExecutor for PendingExecutor {
    fn stage_id(&self) -> &str {
        self.stage.stage_id
    }

    fn execute(&self, _ctx: &BuildContext<'_>) -> Adm4Result<StageStatus> {
        Ok(StageStatus::Blocked {
            reasons: vec![self.stage.blocked_reason()],
        })
    }
}

/// 尚未实现段的诚实空执行器（P3/P4/P5）。
pub fn pending_executors() -> Vec<Box<dyn StageExecutor>> {
    PENDING_STAGES
        .iter()
        .map(|stage| Box::new(PendingExecutor::new(stage)) as Box<dyn StageExecutor>)
        .collect()
}

/// 「已有真实现、但本装配没给它上下文」的诚实占位。
///
/// P0/P1/P2 的真实执行器要注入 Phase 1 产物仓 / 引擎后端与指南来源 / 风格门根 / 图像通道，
/// 这些只有门面（`AppServices`）拿得到。不带上下文的 `Phase2Runner::new()`（人工门确认、
/// 测试夹具用）对这三段装它——被跑到时如实说「装配没接线」，而不是假装还没实现或假装成功（R7）。
struct NotWiredExecutor {
    stage_id: &'static str,
    hint: &'static str,
}

impl StageExecutor for NotWiredExecutor {
    fn stage_id(&self) -> &str {
        self.stage_id
    }

    fn execute(&self, _ctx: &BuildContext<'_>) -> Adm4Result<StageStatus> {
        Ok(StageStatus::Blocked {
            reasons: vec![format!(
                "阶段 {} 的执行器未接线：{}（请通过 AppServices::build_run 运行——门面会注入真实上下文）",
                self.stage_id, self.hint
            )],
        })
    }
}

/// P0-P5 运行器。
pub struct Phase2Runner {
    registry: Vec<StageSpec>,
    executors: BTreeMap<String, Box<dyn StageExecutor>>,
}

impl Phase2Runner {
    /// 无上下文装配：未实现段 = 诚实空实现；已实现段（P0/P1/P2）= 未接线占位。
    ///
    /// 这个装配**跑不出任何成功段**，它存在只为两件事：人工门确认（只查运行状态，
    /// 不执行段）与测试夹具。真实运行走 [`Phase2Runner::with_executors`]（门面装配）。
    pub fn new() -> Self {
        let mut executors: BTreeMap<String, Box<dyn StageExecutor>> = pending_executors()
            .into_iter()
            .map(|executor| (executor.stage_id().to_string(), executor))
            .collect();
        executors.insert(
            "P0".to_string(),
            Box::new(NotWiredExecutor {
                stage_id: "P0",
                hint: "两条线派生要读 Phase 1 的 C3/C4 产物仓",
            }),
        );
        executors.insert(
            "P1".to_string(),
            Box::new(NotWiredExecutor {
                stage_id: "P1",
                hint: "可玩切片现场开发要注入引擎后端、引擎指南来源与工作区目录",
            }),
        );
        executors.insert(
            "P2".to_string(),
            Box::new(NotWiredExecutor {
                stage_id: "P2",
                hint: "资产生产要注入风格门产物根与图像生成通道",
            }),
        );
        Self {
            registry: phase2_registry(),
            executors,
        }
    }

    /// 注入执行器装配（后续波次替换插件、测试注入夹具都走这里）。
    ///
    /// 装配时**当场**校验三件事：每段有且只有一个执行器、没有 registry 之外的执行器、
    /// 制品依赖图与 registry 自洽。装错插件要在装配时炸，而不是跑到一半才发现某段没人管。
    pub fn with_executors(executors: Vec<Box<dyn StageExecutor>>) -> Adm4Result<Self> {
        let registry = phase2_registry();
        validate_artifact_graph(&registry, &phase2_artifacts())?;
        let known: BTreeSet<&str> = registry.iter().map(|stage| stage.id.as_str()).collect();
        let mut map: BTreeMap<String, Box<dyn StageExecutor>> = BTreeMap::new();
        for executor in executors {
            let stage_id = executor.stage_id().to_string();
            if !known.contains(stage_id.as_str()) {
                return Err(Adm4Error::not_found(format!(
                    "执行器声称负责阶段 {stage_id}，但它不在 Phase 2 registry 内"
                )));
            }
            if map.insert(stage_id.clone(), executor).is_some() {
                return Err(Adm4Error::conflict(format!(
                    "阶段 {stage_id} 注册了多个执行器：一段只能有一个负责人"
                )));
            }
        }
        for stage in &registry {
            if !map.contains_key(&stage.id) {
                return Err(Adm4Error::validation(format!(
                    "阶段 {} 没有注册执行器：插件装配不完整（宁可装不上，也不空跑一段）",
                    stage.id
                )));
            }
        }
        Ok(Self {
            registry,
            executors: map,
        })
    }

    pub fn registry(&self) -> &[StageSpec] {
        &self.registry
    }

    /// 顺序执行 [from, to] 区间内未成功的阶段；遇 Failed/Blocked/WaitingHuman 停止。
    pub fn run_range(
        &self,
        ctx: &BuildContext<'_>,
        from: &str,
        to: &str,
    ) -> Adm4Result<PipelineRunState> {
        Ok(self
            .run_range_with_cancel(ctx, from, to, &CancelSignal::never())?
            .state)
    }

    /// `run_range` 的可取消变体：每段开始前检查取消信号（协作式，不打断段内工作）。
    ///
    /// 被取消时该段记为 `Pending`（未运行）而不是 `Failed`——用户主动停止不是失败；
    /// 已完成段的成功状态与产物原样保留，下次照旧断点续跑。取消判定排在「落 Running」
    /// 之前，避免进程在两次写盘之间死掉时留下一个永远「正在跑」的幽灵段（同 Phase 1）。
    pub fn run_range_with_cancel(
        &self,
        ctx: &BuildContext<'_>,
        from: &str,
        to: &str,
        cancel: &CancelSignal,
    ) -> Adm4Result<PipelineRunOutcome> {
        let mut state = self.bind_run_state(ctx)?;
        let (start, end) = self.range_bounds(from, to)?;
        let mut cancelled_at = None;
        for stage in &self.registry[start..=end] {
            if state.is_succeeded(&stage.id) {
                continue;
            }
            if cancel.is_cancelled() {
                state
                    .stages
                    .insert(stage.id.clone(), pending_record(&stage.id));
                ctx.store.save_run_state(&state)?;
                cancelled_at = Some(stage.id.clone());
                break;
            }
            let unmet: Vec<String> = stage
                .depends_on
                .iter()
                .filter(|dependency| !state.is_succeeded(dependency))
                .cloned()
                .collect();
            if !unmet.is_empty() {
                state.stages.insert(
                    stage.id.clone(),
                    blocked_record(&stage.id, vec![format!("依赖未完成：{}", unmet.join(", "))]),
                );
                ctx.store.save_run_state(&state)?;
                break;
            }
            let started_at = now_iso();
            state.stages.insert(
                stage.id.clone(),
                StageRecord {
                    stage_id: stage.id.clone(),
                    status: StageStatus::Running,
                    contract_hash: String::new(),
                    started_at: started_at.clone(),
                    finished_at: String::new(),
                    human_confirmation: None,
                },
            );
            ctx.store.save_run_state(&state)?;

            let status = match self.execute_stage(ctx, &stage.id) {
                Ok(status) => status,
                Err(error) => match error.kind {
                    // R7：阻塞与 AI 不可用是「没做成」，不是「做坏了」——落 Blocked 带原因。
                    Adm4ErrorKind::AiUnavailable | Adm4ErrorKind::Blocked => StageStatus::Blocked {
                        reasons: vec![error.message.clone()],
                    },
                    _ => StageStatus::Failed {
                        reasons: vec![error.message.clone()],
                    },
                },
            };
            let stop = !matches!(status, StageStatus::Succeeded);
            state.stages.insert(
                stage.id.clone(),
                StageRecord {
                    stage_id: stage.id.clone(),
                    status,
                    contract_hash: String::new(),
                    started_at,
                    finished_at: now_iso(),
                    human_confirmation: None,
                },
            );
            ctx.store.save_run_state(&state)?;
            if stop {
                break;
            }
        }
        Ok(PipelineRunOutcome {
            state,
            cancelled_at,
        })
    }

    /// 强制重跑：先重置 `from` 及其**全部下游**（状态 + 产物 + 人工门署名），再从 `from` 跑到 `to`。
    ///
    /// 参数合法性与真源绑定在**动手删任何东西之前**校验：区间写错不该毁掉已有产物。
    pub fn rerun_from(
        &self,
        ctx: &BuildContext<'_>,
        from: &str,
        to: &str,
        cancel: &CancelSignal,
    ) -> Adm4Result<PipelineRerunOutcome> {
        self.bind_run_state(ctx)?;
        self.range_bounds(from, to)?;
        let reset = self.reset_from(ctx.store, from)?;
        let outcome = self.run_range_with_cancel(ctx, from, to, cancel)?;
        Ok(PipelineRerunOutcome {
            reset,
            state: outcome.state,
            cancelled_at: outcome.cancelled_at,
        })
    }

    /// 重置目标段及其全部下游：删运行状态记录（人工门署名随之作废）并清空已落盘产物。
    ///
    /// 顺序是**先落状态、后删产物**：删除中途失败时磁盘上是「状态已作废 + 残留旧产物」，
    /// 重跑会覆盖残留产物，是安全的一侧；反过来会留下「状态称成功、产物已消失」的谎报。
    pub fn reset_from(
        &self,
        store: &ArtifactStore,
        stage_id: &str,
    ) -> Adm4Result<StageResetReport> {
        let reset_stages = self.downstream_stages(stage_id)?;
        let mut state = store.load_run_state()?;
        let mut revoked_confirmations = Vec::new();
        for target in &reset_stages {
            if let Some(record) = state.stages.remove(target)
                && let Some(confirmation) = record.human_confirmation
            {
                revoked_confirmations.push(RevokedConfirmation {
                    stage_id: target.clone(),
                    actor: confirmation.actor,
                    at: confirmation.at,
                });
            }
        }
        store.save_run_state(&state)?;

        let mut cleared_artifacts = Vec::new();
        for target in &reset_stages {
            if store.clear_stage(target)? {
                cleared_artifacts.push(target.clone());
            }
        }
        Ok(StageResetReport {
            target: stage_id.to_string(),
            reset_stages,
            revoked_confirmations,
            cleared_artifacts,
        })
    }

    /// 目标段及其全部下游段（registry 顺序）。
    ///
    /// 取两种推导的**并集**（更保守的一方）：① registry 顺序上位于目标段之后的全部段；
    /// ② `depends_on` 的传递闭包。当前 P0-P5 已是拓扑序，两者结果相同；取并集是为了
    /// 将来插入乱序段时不会漏掉真实下游（与 Phase 1 同一口径）。
    pub fn downstream_stages(&self, stage_id: &str) -> Adm4Result<Vec<String>> {
        let index = self
            .registry
            .iter()
            .position(|stage| stage.id == stage_id)
            .ok_or_else(|| Adm4Error::not_found(format!("未知阶段 {stage_id}")))?;
        let mut closure: BTreeSet<String> = BTreeSet::new();
        closure.insert(stage_id.to_string());
        loop {
            let mut grew = false;
            for stage in &self.registry {
                if closure.contains(&stage.id) {
                    continue;
                }
                if stage
                    .depends_on
                    .iter()
                    .any(|dependency| closure.contains(dependency))
                {
                    closure.insert(stage.id.clone());
                    grew = true;
                }
            }
            if !grew {
                break;
            }
        }
        Ok(self
            .registry
            .iter()
            .enumerate()
            .filter(|(position, stage)| *position >= index || closure.contains(&stage.id))
            .map(|(_, stage)| stage.id.clone())
            .collect())
    }

    /// 人工门确认（署名必填，R3）。
    ///
    /// 直接转给 Phase 1 的实现：它只认运行状态、不认 registry，两段流水线的人工门
    /// 因此共用同一份判定（署名非空、只改结束时刻、原开始时刻保留）。
    pub fn confirm_human_gate(
        &self,
        store: &ArtifactStore,
        stage_id: &str,
        actor: &str,
        note: &str,
    ) -> Adm4Result<PipelineRunState> {
        // 阶段是否属于本段版图由这里把关：Phase 1 的实现只查运行状态，
        // 不加这一道就能拿 P 段运行器去确认一个 C 段的门。
        if !self.registry.iter().any(|stage| stage.id == stage_id) {
            return Err(Adm4Error::not_found(format!(
                "未知阶段 {stage_id}（Phase 2 版图为 P0-P5）"
            )));
        }
        PipelineRunner::new().confirm_human_gate(store, stage_id, actor, note)
    }

    /// 载入运行状态并与真源版本对绑（首跑写入、错版拒绝）。
    fn bind_run_state(&self, ctx: &BuildContext<'_>) -> Adm4Result<PipelineRunState> {
        let mut state = ctx.store.load_run_state()?;
        let current = ctx.spec.identity.frozen_hash.as_str();
        if current.is_empty() {
            return Err(Adm4Error::validation(
                "GameSpec 没有冻结哈希：Phase 2 必须锚定一个确定的冻结版本才能开跑（D22）",
            ));
        }
        if state.frozen_hash.is_empty() {
            state.frozen_hash = current.to_string();
        } else if state.frozen_hash != current {
            return Err(Adm4Error::conflict(format!(
                "构建运行状态绑定冻结哈希 {}，与当前 {current} 不一致（新冻结版本需新的构建目录）",
                state.frozen_hash
            )));
        }
        Ok(state)
    }

    fn range_bounds(&self, from: &str, to: &str) -> Adm4Result<(usize, usize)> {
        let position = |wanted: &str| {
            self.registry
                .iter()
                .position(|stage| stage.id == wanted)
                .ok_or_else(|| {
                    Adm4Error::not_found(format!("未知阶段 {wanted}（Phase 2 版图为 P0-P5）"))
                })
        };
        let start = position(from)?;
        let end = position(to)?;
        if start > end {
            return Err(Adm4Error::invalid_input(format!("区间非法：{from} > {to}")));
        }
        Ok((start, end))
    }

    fn execute_stage(&self, ctx: &BuildContext<'_>, stage_id: &str) -> Adm4Result<StageStatus> {
        let executor = self
            .executors
            .get(stage_id)
            .ok_or_else(|| Adm4Error::not_found(format!("阶段 {stage_id} 没有注册执行器")))?;
        executor.execute(ctx)
    }
}

impl Default for Phase2Runner {
    fn default() -> Self {
        Self::new()
    }
}

fn pending_record(stage_id: &str) -> StageRecord {
    StageRecord {
        stage_id: stage_id.to_string(),
        status: StageStatus::Pending,
        contract_hash: String::new(),
        // 本段一步都没执行，没有开始时刻可言（耗时因此如实为未知）。
        started_at: String::new(),
        finished_at: now_iso(),
        human_confirmation: None,
    }
}

fn blocked_record(stage_id: &str, reasons: Vec<String>) -> StageRecord {
    StageRecord {
        stage_id: stage_id.to_string(),
        status: StageStatus::Blocked { reasons },
        contract_hash: String::new(),
        started_at: String::new(),
        finished_at: now_iso(),
        human_confirmation: None,
    }
}

/// 秒级 ISO-8601 UTC 时刻（与 Phase 1 的 `now_iso` 同一实现来源）。
fn now_iso() -> String {
    UtcTimestamp::now().to_iso8601()
}

#[cfg(test)]
mod tests {
    use super::*;
    use adm4_contracts::SkinScanner;
    use adm4_spec::{ProjectIntent, SpecIdentity};
    use std::sync::atomic::{AtomicUsize, Ordering};

    fn spec() -> GameSpec {
        GameSpec {
            identity: SpecIdentity {
                schema_version: "4.0.0".into(),
                project_id: "demo".into(),
                frozen_hash: "sha256:frozen".into(),
            },
            intent: ProjectIntent::default(),
            systems: Vec::new(),
            mechanics: Vec::new(),
            entities: Vec::new(),
            tables: Vec::new(),
            content: Vec::new(),
            graphs: Vec::new(),
            acceptance: Vec::new(),
            source_map: Vec::new(),
        }
    }

    fn scratch_store(case: &str) -> ArtifactStore {
        let root = std::env::temp_dir().join(format!(
            "adm4_build_runner_{case}_{}_{}",
            std::process::id(),
            now_iso().replace([':', '.', '-'], "")
        ));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).expect("create temp build root");
        ArtifactStore::new(root, SkinScanner::default())
    }

    /// 测试用执行器：按脚本返回状态，并记下自己被调了几次。
    struct ScriptedExecutor {
        stage_id: String,
        status: StageStatus,
        calls: std::sync::Arc<AtomicUsize>,
    }

    impl ScriptedExecutor {
        fn new(stage_id: &str, status: StageStatus) -> Self {
            Self {
                stage_id: stage_id.to_string(),
                status,
                calls: std::sync::Arc::new(AtomicUsize::new(0)),
            }
        }
    }

    impl StageExecutor for ScriptedExecutor {
        fn stage_id(&self) -> &str {
            &self.stage_id
        }

        fn execute(&self, _ctx: &BuildContext<'_>) -> Adm4Result<StageStatus> {
            self.calls.fetch_add(1, Ordering::SeqCst);
            Ok(self.status.clone())
        }
    }

    /// 全部段 id（registry 口径；夹具不再依赖 `PENDING_STAGES`——G3 起它只剩未实现段）。
    fn all_stage_ids() -> Vec<String> {
        phase2_registry()
            .into_iter()
            .map(|stage| stage.id)
            .collect()
    }

    fn all_succeed() -> Vec<Box<dyn StageExecutor>> {
        all_stage_ids()
            .into_iter()
            .map(|stage_id| {
                Box::new(ScriptedExecutor::new(&stage_id, StageStatus::Succeeded))
                    as Box<dyn StageExecutor>
            })
            .collect()
    }

    #[test]
    fn default_runner_covers_every_registry_stage_honestly() {
        let runner = Phase2Runner::new();
        assert_eq!(runner.registry().len(), 6);
        for stage in runner.registry() {
            assert!(
                runner.executors.contains_key(&stage.id),
                "{} 缺执行器",
                stage.id
            );
        }
        // G4a 后的登记口径：P0/P1/P2 已有真实现（不在待实现表），P3/P4/P5 仍登记在案。
        assert_eq!(PENDING_STAGES.len(), 3);
        for implemented in ["P0", "P1", "P2"] {
            assert!(
                pending_stage(implemented).is_none(),
                "{implemented} 已实现，不该再挂在待实现表上"
            );
        }
        for waiting in ["P3", "P4", "P5"] {
            assert!(
                pending_stage(waiting).is_some(),
                "{waiting} 尚未实现，必须有待实现登记"
            );
        }
    }

    /// 无上下文装配的核心承诺：P0 如实报「执行器未接线」，绝无假成功（R7）。
    /// 真实装配（P0Executor/P2Executor 注入真上下文）由 e2e 覆盖。
    #[test]
    fn honest_empty_plan_blocks_at_the_first_stage_with_a_reason() {
        let store = scratch_store("honest_blocked");
        let spec = spec();
        let ctx = BuildContext {
            spec: &spec,
            store: &store,
        };
        let state = Phase2Runner::new()
            .run_range(&ctx, "P0", "P5")
            .expect("诚实装配应能跑完流程（结论是 Blocked，不是错误）");

        match state.stage_status("P0") {
            StageStatus::Blocked { reasons } => {
                assert_eq!(reasons.len(), 1);
                assert!(reasons[0].contains("未接线"), "{}", reasons[0]);
                assert!(reasons[0].contains("AppServices"), "{}", reasons[0]);
            }
            other => panic!("P0 应 Blocked，实际 {other:?}"),
        }
        for later in ["P1", "P2", "P3", "P4", "P5"] {
            assert_eq!(
                state.stage_status(later),
                StageStatus::Pending,
                "{later} 在 P0 停下后不该被推进"
            );
        }
        assert_eq!(state.frozen_hash, "sha256:frozen", "运行状态绑定真源版本");
        std::fs::remove_dir_all(&store.root).ok();
    }

    #[test]
    fn range_bounds_reject_unknown_and_inverted_stages() {
        let store = scratch_store("range");
        let spec = spec();
        let ctx = BuildContext {
            spec: &spec,
            store: &store,
        };
        let runner = Phase2Runner::new();
        assert_eq!(
            runner.run_range(&ctx, "P0", "C6").unwrap_err().kind,
            Adm4ErrorKind::NotFound
        );
        assert_eq!(
            runner.run_range(&ctx, "P9", "P5").unwrap_err().kind,
            Adm4ErrorKind::NotFound
        );
        assert_eq!(
            runner.run_range(&ctx, "P3", "P1").unwrap_err().kind,
            Adm4ErrorKind::InvalidInput
        );
        std::fs::remove_dir_all(&store.root).ok();
    }

    /// 区间运行 + 断点续跑：已成功的段第二次跑直接跳过（执行器不会被再调一次）。
    #[test]
    fn succeeded_stages_are_skipped_on_resume() {
        let store = scratch_store("resume");
        let spec = spec();
        let ctx = BuildContext {
            spec: &spec,
            store: &store,
        };
        let p0 = ScriptedExecutor::new("P0", StageStatus::Succeeded);
        let p0_calls = std::sync::Arc::clone(&p0.calls);
        let p1 = ScriptedExecutor::new(
            "P1",
            StageStatus::Blocked {
                reasons: vec!["等资产".into()],
            },
        );
        let p1_calls = std::sync::Arc::clone(&p1.calls);
        let mut executors: Vec<Box<dyn StageExecutor>> = vec![Box::new(p0), Box::new(p1)];
        for stage_id in all_stage_ids().into_iter().skip(2) {
            executors.push(Box::new(ScriptedExecutor::new(
                &stage_id,
                StageStatus::Succeeded,
            )));
        }
        let runner = Phase2Runner::with_executors(executors).expect("装配");

        let state = runner.run_range(&ctx, "P0", "P1").expect("第一轮");
        assert!(state.is_succeeded("P0"));
        assert!(matches!(
            state.stage_status("P1"),
            StageStatus::Blocked { .. }
        ));
        assert_eq!(p0_calls.load(Ordering::SeqCst), 1);
        assert_eq!(p1_calls.load(Ordering::SeqCst), 1);

        let state = runner.run_range(&ctx, "P0", "P1").expect("第二轮续跑");
        assert!(state.is_succeeded("P0"));
        assert_eq!(
            p0_calls.load(Ordering::SeqCst),
            1,
            "已成功的段续跑时不该被重新执行"
        );
        assert_eq!(p1_calls.load(Ordering::SeqCst), 2, "未成功的段应重新尝试");
        std::fs::remove_dir_all(&store.root).ok();
    }

    /// 协作式取消：停在段边界，被取消的段记为「未运行」而非失败，已完成段原样保留。
    #[test]
    fn cancellation_stops_at_stage_boundary_without_marking_failure() {
        let store = scratch_store("cancel");
        let spec = spec();
        let ctx = BuildContext {
            spec: &spec,
            store: &store,
        };
        let runner = Phase2Runner::with_executors(all_succeed()).expect("装配");

        let cancel = CancelSignal::new();
        cancel.cancel();
        let outcome = runner
            .run_range_with_cancel(&ctx, "P0", "P5", &cancel)
            .expect("取消是正常结束，不是错误");
        assert_eq!(outcome.cancelled_at.as_deref(), Some("P0"));
        assert_eq!(outcome.state.stage_status("P0"), StageStatus::Pending);

        // 复位后续跑：P0 起照常推进到底。
        cancel.reset();
        let outcome = runner
            .run_range_with_cancel(&ctx, "P0", "P5", &cancel)
            .expect("续跑");
        assert!(outcome.cancelled_at.is_none());
        for stage_id in all_stage_ids() {
            assert!(outcome.state.is_succeeded(&stage_id), "{stage_id} 应成功");
        }
        std::fs::remove_dir_all(&store.root).ok();
    }

    /// 依赖未完成时本段 Blocked 并停下（不越过依赖硬跑）。
    #[test]
    fn unmet_dependency_blocks_the_stage() {
        let store = scratch_store("dependency");
        let spec = spec();
        let ctx = BuildContext {
            spec: &spec,
            store: &store,
        };
        let runner = Phase2Runner::with_executors(all_succeed()).expect("装配");
        // 直接从 P1 起跑：它依赖 P0，而 P0 从未成功。
        let state = runner.run_range(&ctx, "P1", "P5").expect("运行");
        match state.stage_status("P1") {
            StageStatus::Blocked { reasons } => {
                assert!(reasons[0].contains("依赖未完成"), "{}", reasons[0]);
                assert!(reasons[0].contains("P0"), "{}", reasons[0]);
            }
            other => panic!("P1 应因依赖未完成而 Blocked，实际 {other:?}"),
        }
        assert_eq!(state.stage_status("P2"), StageStatus::Pending);
        std::fs::remove_dir_all(&store.root).ok();
    }

    /// 人工门：署名必填（R3），确认后转成功；未在等待态的段不接受确认。
    #[test]
    fn human_gate_requires_a_signature_and_a_waiting_stage() {
        let store = scratch_store("human_gate");
        let spec = spec();
        let ctx = BuildContext {
            spec: &spec,
            store: &store,
        };
        let mut executors: Vec<Box<dyn StageExecutor>> = Vec::new();
        for stage_id in all_stage_ids() {
            let status = if stage_id == "P2" {
                StageStatus::WaitingHuman {
                    gate: "asset_budget".into(),
                }
            } else {
                StageStatus::Succeeded
            };
            executors.push(Box::new(ScriptedExecutor::new(&stage_id, status)));
        }
        let runner = Phase2Runner::with_executors(executors).expect("装配");

        let state = runner.run_range(&ctx, "P0", "P5").expect("运行到人工门");
        assert!(matches!(
            state.stage_status("P2"),
            StageStatus::WaitingHuman { .. }
        ));
        assert_eq!(
            state.stage_status("P3"),
            StageStatus::Pending,
            "门未过不推进"
        );

        assert_eq!(
            runner
                .confirm_human_gate(&store, "P2", "   ", "预算确认")
                .unwrap_err()
                .kind,
            Adm4ErrorKind::InvalidInput,
            "匿名确认等于没有评审（R3）"
        );
        assert_eq!(
            runner
                .confirm_human_gate(&store, "P0", "评审员甲", "乱确认")
                .unwrap_err()
                .kind,
            Adm4ErrorKind::Conflict,
            "不在等待态的段不接受确认"
        );
        assert_eq!(
            runner
                .confirm_human_gate(&store, "C5", "评审员甲", "跨段确认")
                .unwrap_err()
                .kind,
            Adm4ErrorKind::NotFound,
            "P 段运行器不得去确认 C 段的门"
        );

        let state = runner
            .confirm_human_gate(&store, "P2", " 评审员甲 ", "预算清单已逐条核对")
            .expect("署名确认");
        assert!(state.is_succeeded("P2"));
        let confirmation = state.stages["P2"]
            .human_confirmation
            .clone()
            .expect("署名在案");
        assert_eq!(confirmation.actor, "评审员甲");
        assert!(confirmation.note.contains("asset_budget"), "门名进备注");

        let state = runner.run_range(&ctx, "P0", "P5").expect("确认后续跑");
        assert!(state.is_succeeded("P5"));
        std::fs::remove_dir_all(&store.root).ok();
    }

    /// 强制重跑：连带作废下游状态、产物与人工门署名（R3：旧署名不为新产物背书）。
    #[test]
    fn rerun_invalidates_downstream_state_artifacts_and_signatures() {
        let store = scratch_store("rerun");
        let spec = spec();
        let ctx = BuildContext {
            spec: &spec,
            store: &store,
        };
        let runner = Phase2Runner::with_executors(all_succeed()).expect("装配");
        runner.run_range(&ctx, "P0", "P5").expect("首轮全绿");
        for stage_id in ["P2", "P3", "P4"] {
            store
                .write_stage(
                    stage_id,
                    &serde_json::json!({ "stage": stage_id }),
                    &format!("# {stage_id}\n正文"),
                )
                .expect("写产物");
        }
        // 给 P2 补一个人工门署名（重跑要连它一起作废）。
        let mut state = store.load_run_state().expect("读状态");
        if let Some(record) = state.stages.get_mut("P2") {
            record.human_confirmation = Some(adm4_pipeline::HumanConfirmation {
                actor: "预算评审员".into(),
                note: "首轮确认".into(),
                at: now_iso(),
            });
        }
        store.save_run_state(&state).expect("存状态");

        let outcome = runner
            .rerun_from(&ctx, "P2", "P5", &CancelSignal::never())
            .expect("重跑");
        assert_eq!(outcome.reset.target, "P2");
        assert_eq!(outcome.reset.reset_stages, vec!["P2", "P3", "P4", "P5"]);
        assert_eq!(outcome.reset.cleared_artifacts, vec!["P2", "P3", "P4"]);
        assert_eq!(outcome.reset.revoked_confirmations.len(), 1);
        assert_eq!(outcome.reset.revoked_confirmations[0].actor, "预算评审员");
        assert!(
            outcome.reset.summary().contains("重置 4 段"),
            "{}",
            outcome.reset.summary()
        );
        // 上游不受影响，重置范围内重新跑绿。
        assert!(outcome.state.is_succeeded("P0"));
        assert!(outcome.state.is_succeeded("P5"));
        assert!(
            outcome.state.stages["P2"].human_confirmation.is_none(),
            "旧署名不得随重跑复活"
        );

        // 参数写错时一份产物、一条状态都不许被动（校验排在重置之前）。
        assert_eq!(
            runner
                .rerun_from(&ctx, "P4", "P1", &CancelSignal::never())
                .unwrap_err()
                .kind,
            Adm4ErrorKind::InvalidInput
        );
        let after = store.load_run_state().expect("读状态");
        for stage_id in all_stage_ids() {
            assert!(
                after.is_succeeded(&stage_id),
                "{stage_id} 不该被非法重跑参数波及"
            );
        }
        std::fs::remove_dir_all(&store.root).ok();
    }

    #[test]
    fn downstream_stages_cover_target_and_everything_after_it() {
        let runner = Phase2Runner::new();
        assert_eq!(
            runner.downstream_stages("P2").expect("已知阶段"),
            vec!["P2", "P3", "P4", "P5"]
        );
        assert_eq!(runner.downstream_stages("P5").expect("末段"), vec!["P5"]);
        assert_eq!(
            runner
                .downstream_stages("C0")
                .expect_err("C 段不在 P 段版图内")
                .kind,
            Adm4ErrorKind::NotFound
        );
    }

    /// 换了冻结版本必须拒跑（同一个构建目录不得混两版真源的产物）。
    #[test]
    fn run_state_is_bound_to_the_frozen_source_version() {
        let store = scratch_store("bind");
        let spec = spec();
        let runner = Phase2Runner::new();
        runner
            .run_range(
                &BuildContext {
                    spec: &spec,
                    store: &store,
                },
                "P0",
                "P0",
            )
            .expect("首跑写入绑定");

        let mut other = spec.clone();
        other.identity.frozen_hash = "sha256:another".into();
        let error = runner
            .run_range(
                &BuildContext {
                    spec: &other,
                    store: &store,
                },
                "P0",
                "P0",
            )
            .unwrap_err();
        assert_eq!(error.kind, Adm4ErrorKind::Conflict);

        let mut unbound = spec;
        unbound.identity.frozen_hash.clear();
        let error = runner
            .run_range(
                &BuildContext {
                    spec: &unbound,
                    store: &store,
                },
                "P0",
                "P0",
            )
            .unwrap_err();
        assert_eq!(error.kind, Adm4ErrorKind::Validation);
        std::fs::remove_dir_all(&store.root).ok();
    }

    /// `Phase2Runner` 不实现 `Debug`（里面装着 trait 对象），测试取错走 `err()`。
    fn assembly_error(executors: Vec<Box<dyn StageExecutor>>) -> Adm4Error {
        Phase2Runner::with_executors(executors)
            .err()
            .expect("装配应失败")
    }

    #[test]
    fn executor_assembly_rejects_gaps_duplicates_and_strangers() {
        // 少一段。
        let partial: Vec<Box<dyn StageExecutor>> = vec![Box::new(ScriptedExecutor::new(
            "P0",
            StageStatus::Succeeded,
        ))];
        assert!(
            assembly_error(partial).message.contains("没有注册执行器"),
            "缺段必须在装配时炸，而不是跑到一半才发现某段没人管"
        );
        // 重复注册。
        let mut duplicated = all_succeed();
        duplicated.push(Box::new(ScriptedExecutor::new(
            "P0",
            StageStatus::Succeeded,
        )));
        assert_eq!(assembly_error(duplicated).kind, Adm4ErrorKind::Conflict);
        // registry 之外的阶段。
        let mut stranger = all_succeed();
        stranger.push(Box::new(ScriptedExecutor::new(
            "C0",
            StageStatus::Succeeded,
        )));
        assert_eq!(assembly_error(stranger).kind, Adm4ErrorKind::NotFound);
    }

    /// 执行器抛错时：阻塞类落 Blocked，其余落 Failed（与 Phase 1 同一套映射，R7）。
    #[test]
    fn executor_errors_map_to_blocked_or_failed() {
        struct Failing {
            stage_id: String,
            error: Adm4Error,
        }
        impl StageExecutor for Failing {
            fn stage_id(&self) -> &str {
                &self.stage_id
            }
            fn execute(&self, _ctx: &BuildContext<'_>) -> Adm4Result<StageStatus> {
                Err(self.error.clone())
            }
        }

        for (error, expect_blocked) in [
            (Adm4Error::blocked("环境缺失"), true),
            (Adm4Error::ai_unavailable("未配置 Provider"), true),
            (Adm4Error::io("磁盘写失败"), false),
        ] {
            let store = scratch_store("errors");
            let spec = spec();
            let ctx = BuildContext {
                spec: &spec,
                store: &store,
            };
            let mut executors: Vec<Box<dyn StageExecutor>> = vec![Box::new(Failing {
                stage_id: "P0".into(),
                error: error.clone(),
            })];
            for stage_id in all_stage_ids().into_iter().skip(1) {
                executors.push(Box::new(ScriptedExecutor::new(
                    &stage_id,
                    StageStatus::Succeeded,
                )));
            }
            let runner = Phase2Runner::with_executors(executors).expect("装配");
            let state = runner.run_range(&ctx, "P0", "P5").expect("运行");
            match (state.stage_status("P0"), expect_blocked) {
                (StageStatus::Blocked { reasons }, true) => {
                    assert_eq!(reasons, vec![error.message.clone()]);
                }
                (StageStatus::Failed { reasons }, false) => {
                    assert_eq!(reasons, vec![error.message.clone()]);
                }
                (other, _) => panic!("{error:?} 映射错误：{other:?}"),
            }
            std::fs::remove_dir_all(&store.root).ok();
        }
    }
}
