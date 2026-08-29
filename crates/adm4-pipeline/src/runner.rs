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

/// 阶段执行器共享上下文。
pub struct RunnerContext<'a> {
    pub frozen: &'a FrozenDesign,
    pub space: &'a DesignSpace,
    pub ai: &'a dyn AiProvider,
    pub store: &'a ArtifactStore,
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
    pub fn run_range(
        &self,
        ctx: &RunnerContext<'_>,
        from: &str,
        to: &str,
    ) -> Adm4Result<PipelineRunState> {
        let mut state = ctx.store.load_run_state()?;
        if state.frozen_hash.is_empty() {
            state.frozen_hash = ctx.frozen.content_hash.clone();
        } else if state.frozen_hash != ctx.frozen.content_hash {
            return Err(Adm4Error::conflict(format!(
                "运行状态绑定冻结哈希 {}，与当前 {} 不一致（新冻结版本需新的流水线目录）",
                state.frozen_hash, ctx.frozen.content_hash
            )));
        }
        let ids: Vec<String> = self.registry.iter().map(|stage| stage.id.clone()).collect();
        let start = ids
            .iter()
            .position(|id| id == from)
            .ok_or_else(|| Adm4Error::not_found(format!("未知阶段 {from}")))?;
        let end = ids
            .iter()
            .position(|id| id == to)
            .ok_or_else(|| Adm4Error::not_found(format!("未知阶段 {to}")))?;
        if start > end {
            return Err(Adm4Error::invalid_input(format!("区间非法：{from} > {to}")));
        }
        for stage in &self.registry[start..=end] {
            if state.is_succeeded(&stage.id) {
                continue;
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
                        finished_at: now_iso(),
                        human_confirmation: None,
                    },
                );
                ctx.store.save_run_state(&state)?;
                break;
            }
            let outcome = self.execute_stage(ctx, &stage.id);
            let record = match outcome {
                Ok(status) => StageRecord {
                    stage_id: stage.id.clone(),
                    status,
                    contract_hash: String::new(),
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
        Ok(state)
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
}
