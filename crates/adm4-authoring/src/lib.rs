//! V4 创作引擎：项目创作状态、双模式作业（手动 / AI 访谈分层确认）、
//! 模板预填与对照、冻结门五道、FrozenDesign。

mod engine;
mod freeze;
mod interview;
mod state;

pub use engine::AuthoringEngine;
pub use freeze::{
    FreezeGateReport, FrozenDesign, GateFinding, GateResult, evaluate_freeze_gates, execute_freeze,
    run_red_team,
};
pub use interview::{
    InterviewProgress, InterviewProposal, InterviewService, InterviewTurn, LevelProgress,
};
pub use state::{
    AuthoringState, Finding, FindingDisposition, InterviewEntry, InterviewState, RedTeamRecord,
    TemplateMode,
};
