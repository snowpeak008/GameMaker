//! V4 流水线：框架（registry/状态机/双格式产物/换皮扫描钩子）+ Phase 1 的 C0-C6 执行器。
//!
//! Phase 1 完成定义：C0-C6 契约齐全 + 两个人工门（C5 风格、C6 签收）通过。
//! Phase 2（P0-P5）仅保留 registry 边界，另行立项。

mod c0_compile;
mod c1_validation;
mod c2_gameplay;
mod c3_content;
mod c4_capabilities;
mod c5_style;
mod c6_plan;
mod cancel;
mod framework;
mod runner;

pub use c0_compile::compile_frozen_design;
pub use cancel::CancelSignal;
pub use framework::{
    ArtifactStore, PipelineRunState, StageKind, StageRecord, StageSpec, StageStatus,
    design_compile_registry, phase2_registry,
};
pub use runner::{
    PipelineRerunOutcome, PipelineRunOutcome, PipelineRunner, RevokedConfirmation, RunnerContext,
    StageResetReport,
};
