use crate::cancel::CancelSignal;
use crate::framework::{
    ArtifactStore, HumanConfirmation, PipelineRunState, StageRecord, StageSpec, StageStatus,
    design_compile_registry, now_iso,
};
use crate::{
    c0_compile, c1_validation, c2_gameplay, c3_content, c4_capabilities, c5_style, c6_plan,
};
use adm4_ai::AiProvider;
use adm4_authoring::FrozenDesign;
use adm4_foundation::{Adm4Error, Adm4ErrorKind, Adm4Result};
use adm4_space::DesignSpace;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

/// 阶段执行器共享上下文。
pub struct RunnerContext<'a> {
    pub frozen: &'a FrozenDesign,
    pub space: &'a DesignSpace,
    pub ai: &'a dyn AiProvider,
    pub store: &'a ArtifactStore,
}

/// 一次区间运行的结果。
///
/// 为什么不只返回 `PipelineRunState`：被用户取消与「跑到人工门停下」在状态里长得一样
/// （当前段都不是 Succeeded），上层没法区分，也就无法如实落日志。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PipelineRunOutcome {
    pub state: PipelineRunState,
    /// `Some(段 id)` = 运行在该段开始前被用户取消，该段记为「未运行」（Pending）。
    #[serde(default)]
    pub cancelled_at: Option<String>,
}

/// 被重置作废的人工门确认。
///
/// R3：旧署名不得为新产物背书。重跑范围内已通过的 C5/C6 确认必须连同产物一起失效，
/// 并把原署名留在报告与日志里（谁在什么时候签的、被哪次重跑作废，可追溯）。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RevokedConfirmation {
    pub stage_id: String,
    pub actor: String,
    pub at: String,
}

/// 重置报告：一次「目标段 + 全部下游」失效操作的如实清单。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StageResetReport {
    /// 重置起点（用户指定要重跑的段）。
    pub target: String,
    /// 实际被重置的段（含目标段，registry 顺序）。
    pub reset_stages: Vec<String>,
    /// 被作废的人工门确认（需重新确认）。
    pub revoked_confirmations: Vec<RevokedConfirmation>,
    /// 确有产物被删除的段（原本没产物的段不列，不虚报）。
    pub cleared_artifacts: Vec<String>,
}

impl StageResetReport {
    /// 一行摘要（日志与 CLI/GUI 提示共用；与 `WorkbenchResetReport::summary` /
    /// `TemplateExportReport::summary` 同一套口径）。
    ///
    /// 放在报告类型上而不是各呈现层各拼一份：三处（服务层日志、CLI、桌面状态栏）
    /// 曾各写一遍同样的 format!，改一处就与另两处对不上，用户在日志与界面里看到
    /// 两个说法。
    pub fn summary(&self) -> String {
        format!(
            "重置 {} 段（{}），清空产物 {} 段，作废人工门确认 {} 处",
            self.reset_stages.len(),
            self.reset_stages.join("/"),
            self.cleared_artifacts.len(),
            self.revoked_confirmations.len()
        )
    }
}

/// 强制重跑结果：先失效（`reset`）再运行（`state` / `cancelled_at`）。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PipelineRerunOutcome {
    pub reset: StageResetReport,
    pub state: PipelineRunState,
    #[serde(default)]
    pub cancelled_at: Option<String>,
}

/// C0-C6 运行器：依赖检查、断点续跑、人工门确认。
pub struct PipelineRunner {
    registry: Vec<StageSpec>,
}

impl PipelineRunner {
    pub fn new() -> Self {
        Self {
            registry: design_compile_registry(),
        }
    }

    pub fn registry(&self) -> &[StageSpec] {
        &self.registry
    }

    /// 顺序执行 [from, to] 区间内未成功的阶段；遇 Failed/Blocked/WaitingHuman 停止。
    ///
    /// 行为与 T13 基线完全一致（内部走永不取消的信号），既有调用方无需改动。
    pub fn run_range(
        &self,
        ctx: &RunnerContext<'_>,
        from: &str,
        to: &str,
    ) -> Adm4Result<PipelineRunState> {
        Ok(self
            .run_range_with_cancel(ctx, from, to, &CancelSignal::never())?
            .state)
    }

    /// `run_range` 的可取消变体：每段开始前检查取消信号（协作式，不打断段内 AI 调用）。
    ///
    /// 被取消时：停止推进、当前段写为「未运行」（`Pending`，**不是** `Failed`——用户主动
    /// 停止不是失败）、已完成段的成功状态与产物原样保留（下次运行照旧断点续跑）。
    pub fn run_range_with_cancel(
        &self,
        ctx: &RunnerContext<'_>,
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
            // 协作式取消只在段边界生效：此刻磁盘上是上一段的完整产物，本段尚未写入任何东西。
            //
            // 取消判定必须排在「落 Running」之前：反过来会先把本段写成 Running 再改回
            // Pending，一旦进程在两次写盘之间死掉，磁盘上就留下一个永远「正在跑」的幽灵段。
            if cancel.is_cancelled() {
                state.stages.insert(
                    stage.id.clone(),
                    StageRecord {
                        stage_id: stage.id.clone(),
                        // 用户取消 ≠ 阶段失败：记为未运行，避免污染 Failed 的语义。
                        status: StageStatus::Pending,
                        contract_hash: String::new(),
                        // 本段一步都没执行，没有开始时刻可言（耗时因此如实为未知）。
                        started_at: String::new(),
                        finished_at: now_iso(),
                        human_confirmation: None,
                    },
                );
                ctx.store.save_run_state(&state)?;
                cancelled_at = Some(stage.id.clone());
                break;
            }
            // 依赖检查。
            let unmet: Vec<String> = stage
                .depends_on
                .iter()
                .filter(|dependency| !state.is_succeeded(dependency))
                .cloned()
                .collect();
            if !unmet.is_empty() {
                state.stages.insert(
                    stage.id.clone(),
                    StageRecord {
                        stage_id: stage.id.clone(),
                        status: StageStatus::Blocked {
                            reasons: vec![format!("依赖未完成：{}", unmet.join(", "))],
                        },
                        contract_hash: String::new(),
                        started_at: String::new(),
                        finished_at: now_iso(),
                        human_confirmation: None,
                    },
                );
                ctx.store.save_run_state(&state)?;
                break;
            }
            // 本段真要跑了：先把 Running + 开始时刻落盘，再执行。
            //
            // 多这一次写盘换来两件事：① 长时段（C1-C5 都含 AI 调用）运行期间，别的进程
            // /线程读 run_state 能看到「C3 正在跑」而不是「C3 待运行」——桌面端把流水线
            // 放进工作线程后靠它显示进度；② 段的开始时刻在案，「耗时」才有归宿。
            let started_at = now_iso();
            state.stages.insert(
                stage.id.clone(),
                StageRecord {
                    stage_id: stage.id.clone(),
                    status: StageStatus::Running,
                    contract_hash: String::new(),
                    started_at: started_at.clone(),
                    // 还没结束，结束时刻留空（不拿开始时刻冒充）。
                    finished_at: String::new(),
                    human_confirmation: None,
                },
            );
            ctx.store.save_run_state(&state)?;

            let outcome = self.execute_stage(ctx, &stage.id);
            let record = match outcome {
                Ok(status) => StageRecord {
                    stage_id: stage.id.clone(),
                    status,
                    contract_hash: String::new(),
                    started_at: started_at.clone(),
                    finished_at: now_iso(),
                    human_confirmation: None,
                },
                Err(error) => {
                    // R7：AI 不可用与 R2 阻塞 → Blocked（携带原因）；其余 → Failed。
                    let status = match error.kind {
                        Adm4ErrorKind::AiUnavailable | Adm4ErrorKind::Blocked => {
                            StageStatus::Blocked {
                                reasons: vec![error.message.clone()],
                            }
                        }
                        _ => StageStatus::Failed {
                            reasons: vec![error.message.clone()],
                        },
                    };
                    StageRecord {
                        stage_id: stage.id.clone(),
                        status,
                        contract_hash: String::new(),
                        started_at: started_at.clone(),
                        finished_at: now_iso(),
                        human_confirmation: None,
                    }
                }
            };
            let stop = !matches!(record.status, StageStatus::Succeeded);
            state.stages.insert(stage.id.clone(), record);
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
    /// 为什么必须连带下游：只重跑中间段而保留下游「已成功」，下游产物仍是按旧契约渲染的，
    /// 文档集会出现「C2 是新版、C4 引用旧版机制」的错版组合——这比不重跑更危险。
    ///
    /// 冻结哈希绑定与区间合法性在**动手删任何东西之前**校验：参数写错不该毁掉已有产物。
    pub fn rerun_from(
        &self,
        ctx: &RunnerContext<'_>,
        from: &str,
        to: &str,
        cancel: &CancelSignal,
    ) -> Adm4Result<PipelineRerunOutcome> {
        // 两个前置校验只为「早失败」：命中任一条就直接上抛，不进入下面的重置。
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

    /// 重置目标段及其全部下游：删除运行状态记录（人工门确认随之作废）并清空已落盘产物。
    ///
    /// 只做失效不做运行，因此可以独立验证「重置到底作废了什么」。
    ///
    /// 顺序是**先落状态、后删产物**：删除中途若失败（如 Windows 文件被占用），
    /// 磁盘上会是「状态已作废 + 残留旧产物」——重跑会覆盖残留产物，是安全的一侧。
    /// 反过来先删产物再落状态，一旦失败就会留下「状态称成功、产物已消失」的谎报。
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
    /// 两种推导取**并集**（更保守的一方）：① registry 顺序上位于目标段之后的全部段；
    /// ② `depends_on` 的传递闭包。当前 C0-C6 的 registry 已是拓扑序，两者结果相同；
    /// 取并集是为了将来插入乱序段时不会漏掉真实下游。
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

    /// 载入运行状态并与当前冻结版本对绑（首跑写入、错版拒绝）。
    fn bind_run_state(&self, ctx: &RunnerContext<'_>) -> Adm4Result<PipelineRunState> {
        let mut state = ctx.store.load_run_state()?;
        if state.frozen_hash.is_empty() {
            state.frozen_hash = ctx.frozen.content_hash.clone();
        } else if state.frozen_hash != ctx.frozen.content_hash {
            return Err(Adm4Error::conflict(format!(
                "运行状态绑定冻结哈希 {}，与当前 {} 不一致（新冻结版本需新的流水线目录）",
                state.frozen_hash, ctx.frozen.content_hash
            )));
        }
        Ok(state)
    }

    /// 解析 [from, to] 在 registry 中的下标区间。
    fn range_bounds(&self, from: &str, to: &str) -> Adm4Result<(usize, usize)> {
        let position = |wanted: &str| {
            self.registry
                .iter()
                .position(|stage| stage.id == wanted)
                .ok_or_else(|| Adm4Error::not_found(format!("未知阶段 {wanted}")))
        };
        let start = position(from)?;
        let end = position(to)?;
        if start > end {
            return Err(Adm4Error::invalid_input(format!("区间非法：{from} > {to}")));
        }
        Ok((start, end))
    }

    fn execute_stage(&self, ctx: &RunnerContext<'_>, stage_id: &str) -> Adm4Result<StageStatus> {
        match stage_id {
            "C0" => {
                let spec = c0_compile::compile_frozen_design(ctx.frozen, ctx.space)?;
                let document = c0_compile_document(&spec);
                ctx.store.write_stage("C0", &spec, &document)?;
                Ok(StageStatus::Succeeded)
            }
            "C1" => c1_validation::execute(ctx),
            "C2" => c2_gameplay::execute(ctx),
            "C3" => c3_content::execute(ctx),
            "C4" => c4_capabilities::execute(ctx),
            "C5" => c5_style::execute(ctx),
            "C6" => c6_plan::execute(ctx),
            other => Err(Adm4Error::not_found(format!("未知阶段 {other}"))),
        }
    }

    /// 人工门确认（C5 风格 / C6 签收 / 基数确认）。
    pub fn confirm_human_gate(
        &self,
        store: &ArtifactStore,
        stage_id: &str,
        actor: &str,
        note: &str,
    ) -> Adm4Result<PipelineRunState> {
        // R3：人工门必须留下署名，匿名确认等于没有评审。
        let actor = actor.trim();
        if actor.is_empty() {
            return Err(Adm4Error::invalid_input(format!(
                "阶段 {stage_id} 的人工确认必须署名确认人（R3 评审工作量证明）"
            )));
        }
        let mut state = store.load_run_state()?;
        let record = state
            .stages
            .get_mut(stage_id)
            .ok_or_else(|| Adm4Error::not_found(format!("阶段 {stage_id} 尚未运行")))?;
        let StageStatus::WaitingHuman { gate } = record.status.clone() else {
            return Err(Adm4Error::conflict(format!(
                "阶段 {stage_id} 不在等待人工确认状态"
            )));
        };
        record.status = StageStatus::Succeeded;
        record.human_confirmation = Some(HumanConfirmation {
            actor: actor.to_string(),
            note: format!("[{gate}] {note}"),
            at: now_iso(),
        });
        record.finished_at = now_iso();
        store.save_run_state(&state)?;
        Ok(state)
    }
}

impl Default for PipelineRunner {
    fn default() -> Self {
        Self::new()
    }
}

fn c0_compile_document(spec: &adm4_spec::GameSpec) -> String {
    format!(
        "# C0 规格编译报告\n\n- 项目：{}\n- 冻结哈希：`{}`\n- 系统 {} 个 / 机制 {} 个 / 实体 {} 个 / 表 {} 张 / 内容 {} 项\n- source_map 条目：{}\n\n> 本文档由 contract.json 渲染，请勿手改。\n",
        spec.intent.title,
        spec.identity.frozen_hash,
        spec.systems.len(),
        spec.mechanics.len(),
        spec.entities.len(),
        spec.tables.len(),
        spec.content.len(),
        spec.source_map.len()
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::framework::StageRecord;
    use adm4_contracts::SkinScanner;

    fn waiting_store(case: &str) -> ArtifactStore {
        let root = std::env::temp_dir().join(format!(
            "adm4_runner_{case}_{}_{}",
            std::process::id(),
            now_iso().replace([':', '.', '-'], "")
        ));
        std::fs::create_dir_all(&root).expect("create temp pipeline root");
        let store = ArtifactStore::new(root, SkinScanner::default());
        let mut state = PipelineRunState::default();
        state.stages.insert(
            "C5".into(),
            StageRecord {
                stage_id: "C5".into(),
                status: StageStatus::WaitingHuman {
                    gate: "style_confirm".into(),
                },
                contract_hash: String::new(),
                started_at: now_iso(),
                finished_at: now_iso(),
                human_confirmation: None,
            },
        );
        store.save_run_state(&state).expect("save run state");
        store
    }

    #[test]
    fn human_gate_rejects_blank_actor() {
        let store = waiting_store("blank_actor");
        let error = PipelineRunner::new()
            .confirm_human_gate(&store, "C5", "   ", "风格方向确认")
            .unwrap_err();
        assert_eq!(error.kind, Adm4ErrorKind::InvalidInput);
        assert!(error.message.contains("署名"), "{}", error.message);
        std::fs::remove_dir_all(&store.root).ok();
    }

    #[test]
    fn human_gate_records_trimmed_actor() {
        let store = waiting_store("named_actor");
        let state = PipelineRunner::new()
            .confirm_human_gate(&store, "C5", " 评审员甲 ", "风格方向确认")
            .expect("signed confirmation accepted");
        assert!(state.is_succeeded("C5"));
        let confirmation = state.stages["C5"]
            .human_confirmation
            .clone()
            .expect("confirmation recorded");
        assert_eq!(confirmation.actor, "评审员甲");
        std::fs::remove_dir_all(&store.root).ok();
    }

    /// 人工门确认只改结束时刻，开始时刻原样保留——否则「卡在人工门多久」算不出来。
    #[test]
    fn human_gate_confirmation_keeps_started_at() {
        let store = waiting_store("gate_started_at");
        let before = store
            .load_run_state()
            .expect("load run state")
            .stages
            .get("C5")
            .map(|record| record.started_at.clone())
            .expect("C5 记录应在案");
        assert!(!before.is_empty());

        let state = PipelineRunner::new()
            .confirm_human_gate(&store, "C5", "评审员甲", "风格方向确认")
            .expect("signed confirmation accepted");
        let record = &state.stages["C5"];
        assert_eq!(record.started_at, before, "确认不得覆盖开始时刻");
        assert!(!record.finished_at.is_empty());
        assert!(
            record.duration_seconds().is_some(),
            "两个时刻齐备后耗时必须算得出来"
        );

        std::fs::remove_dir_all(&store.root).ok();
    }

    // ------------------------------------------------------------------
    // 强制重跑：下游连带重置
    // ------------------------------------------------------------------

    fn empty_store(case: &str) -> ArtifactStore {
        let root = std::env::temp_dir().join(format!(
            "adm4_runner_{case}_{}_{}",
            std::process::id(),
            now_iso().replace([':', '.', '-'], "")
        ));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).expect("create temp pipeline root");
        ArtifactStore::new(root, SkinScanner::default())
    }

    /// 造一个「C0-C6 全绿、C5/C6 已署名、七段产物齐备」的完工现场。
    fn finished_store(case: &str) -> ArtifactStore {
        let store = empty_store(case);
        let mut state = PipelineRunState {
            frozen_hash: "sha256:fake".into(),
            ..Default::default()
        };
        for stage_id in ["C0", "C1", "C2", "C3", "C4", "C5", "C6"] {
            store
                .write_stage(
                    stage_id,
                    &serde_json::json!({"stage": stage_id}),
                    &format!("# {stage_id}\n正文"),
                )
                .expect("write stage artifacts");
            let human_confirmation = matches!(stage_id, "C5" | "C6").then(|| HumanConfirmation {
                actor: format!("{stage_id} 评审员"),
                note: "确认".into(),
                at: now_iso(),
            });
            state.stages.insert(
                stage_id.to_string(),
                StageRecord {
                    stage_id: stage_id.to_string(),
                    status: StageStatus::Succeeded,
                    contract_hash: String::new(),
                    started_at: now_iso(),
                    finished_at: now_iso(),
                    human_confirmation,
                },
            );
        }
        store.save_run_state(&state).expect("save run state");
        store
    }

    #[test]
    fn downstream_stages_cover_target_and_everything_after_it() {
        let runner = PipelineRunner::new();
        assert_eq!(
            runner.downstream_stages("C2").expect("known stage"),
            vec!["C2", "C3", "C4", "C5", "C6"]
        );
        assert_eq!(
            runner.downstream_stages("C6").expect("last stage"),
            vec!["C6"]
        );
        assert_eq!(
            runner.downstream_stages("C0").expect("first stage"),
            vec!["C0", "C1", "C2", "C3", "C4", "C5", "C6"]
        );
        assert_eq!(
            runner
                .downstream_stages("P0")
                .expect_err("Phase 2 段不在 C 段 registry 内")
                .kind,
            Adm4ErrorKind::NotFound
        );
    }

    #[test]
    fn reset_from_invalidates_downstream_state_artifacts_and_signatures() {
        let store = finished_store("reset_downstream");
        let report = PipelineRunner::new()
            .reset_from(&store, "C2")
            .expect("reset succeeds");

        assert_eq!(report.target, "C2");
        assert_eq!(report.reset_stages, vec!["C2", "C3", "C4", "C5", "C6"]);
        assert_eq!(report.cleared_artifacts, vec!["C2", "C3", "C4", "C5", "C6"]);
        // R3：范围内的人工门署名一并作废，原署名留档可追溯。
        let revoked: Vec<(&str, &str)> = report
            .revoked_confirmations
            .iter()
            .map(|item| (item.stage_id.as_str(), item.actor.as_str()))
            .collect();
        assert_eq!(
            revoked,
            vec![("C5", "C5 评审员"), ("C6", "C6 评审员")],
            "C5/C6 的旧署名不许为重跑后的新产物背书"
        );

        // 运行状态：上游保留成功，重置范围内全部回到未运行。
        let state = store.load_run_state().expect("reload run state");
        for kept in ["C0", "C1"] {
            assert!(
                state.is_succeeded(kept),
                "{kept} 不在重置范围内，应保持成功"
            );
            assert!(store.root.join(kept).join("document.md").is_file());
        }
        for reset in ["C2", "C3", "C4", "C5", "C6"] {
            assert_eq!(
                state.stage_status(reset),
                StageStatus::Pending,
                "{reset} 重置后应为未运行"
            );
            assert!(
                !store.root.join(reset).exists(),
                "{reset} 的旧产物必须删除，否则下游会读到错版契约"
            );
        }
        assert_eq!(state.frozen_hash, "sha256:fake", "重置不解绑冻结版本");

        std::fs::remove_dir_all(&store.root).ok();
    }

    #[test]
    fn reset_from_reports_only_stages_that_actually_had_artifacts() {
        let store = empty_store("reset_partial");
        let mut state = PipelineRunState::default();
        state.stages.insert(
            "C5".into(),
            StageRecord {
                stage_id: "C5".into(),
                status: StageStatus::Succeeded,
                contract_hash: String::new(),
                started_at: now_iso(),
                finished_at: now_iso(),
                human_confirmation: None,
            },
        );
        store.save_run_state(&state).expect("save run state");
        store
            .write_stage("C5", &serde_json::json!({"stage": "C5"}), "# C5")
            .expect("write C5");

        let report = PipelineRunner::new()
            .reset_from(&store, "C5")
            .expect("reset succeeds");
        assert_eq!(report.reset_stages, vec!["C5", "C6"]);
        assert_eq!(
            report.cleared_artifacts,
            vec!["C5"],
            "C6 从未产出过产物，不虚报为已清空"
        );
        assert!(report.revoked_confirmations.is_empty(), "无署名可作废");

        std::fs::remove_dir_all(&store.root).ok();
    }

    #[test]
    fn reset_from_rejects_unknown_stage_without_touching_anything() {
        let store = finished_store("reset_unknown");
        let error = PipelineRunner::new()
            .reset_from(&store, "C9")
            .expect_err("未知阶段必须被拒");
        assert_eq!(error.kind, Adm4ErrorKind::NotFound);
        let state = store.load_run_state().expect("reload run state");
        for stage_id in ["C0", "C1", "C2", "C3", "C4", "C5", "C6"] {
            assert!(state.is_succeeded(stage_id), "{stage_id} 不应被误伤");
            assert!(store.root.join(stage_id).join("contract.json").is_file());
        }
        std::fs::remove_dir_all(&store.root).ok();
    }
}
