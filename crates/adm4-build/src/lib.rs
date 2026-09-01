//! V4 Phase 2 构建产线：治理契约 + 制品注册 + `Phase2Runner` 插件框架。
//!
//! Phase 1 把冻结设计编译成文档集（C0-C6）；Phase 2 把它变成可运行的产物（P0-P5）。
//! 本 crate 是 Phase 2 的**地基**，三条边界写死在这里：
//!
//! - **不新造第二真源**（D22）：一切派生自 `GameSpec`；[`governance`] 里的对齐、回填、命名
//!   都是**校验与追溯**层，不是并行的状态树。
//! - **确定性优先**：对齐三要素核对、权威顺序校验、拓扑序推导全是确定性 Rust，不经 AI；
//!   同一份输入永远得到同一份结论（可直接作为回归夹具断言）。
//! - **接缝纪律**（D17）：具体引擎只在 [`engine`] 的后端实现里出现；治理、注册、运行骨架
//!   三层不认得任何一个具体引擎的名字。
//!
//! 本波（G1）的全部执行器都是[诚实空实现](runner::PendingExecutor)：如实返回 `Blocked`
//! 与「待哪一波补什么」，**绝不返回假成功**（R7）。

pub mod art;
pub mod engine;
mod executors;
pub mod governance;
pub mod program;
mod registry;
mod runner;

pub use executors::{
    BUDGET_FILE, EngineSeedStatus, P0Executor, P2Executor, TwoLineContract, load_budget,
    save_budget,
};
pub use registry::{
    ArtifactKind, StageArtifacts, phase2_artifacts, phase2_execution_order, producer_of,
    topological_order, validate_artifact_graph,
};
pub use runner::{
    BuildContext, PENDING_STAGES, PendingExecutor, PendingStage, Phase2Runner, StageExecutor,
    pending_executors, pending_stage,
};
