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
// C3/C4 契约类型对外导出：Phase 2 的 P0 派生器要按类型读它们（设计文档契约集是
// P0 声明的消费制品）。只导出类型不导出执行器——派生器读产物，不重跑阶段。
pub use c3_content::{AssetSpec as C3AssetSpec, ContentInventoryContract, UiSpecEntry};
pub use c4_capabilities::{CapabilitiesContract, CapabilityContract as C4Capability};
pub use cancel::CancelSignal;
pub use framework::{
    ArtifactStore, HumanConfirmation, PipelineRunState, StageKind, StageRecord, StageSpec,
    StageStatus, design_compile_registry, phase2_registry,
};
pub use runner::{
    PipelineRerunOutcome, PipelineRunOutcome, PipelineRunner, RevokedConfirmation, RunnerContext,
    StageResetReport,
};
