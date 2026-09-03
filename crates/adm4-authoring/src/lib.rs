//! V4 创作引擎：项目创作状态、双模式作业（手动 / AI 访谈分层确认）、
//! 模板预填与对照、冻结门五道、FrozenDesign。

mod custom;
mod engine;
mod freeze;
mod interview;
mod state;

pub use custom::{
    CUSTOM_ENTRY_LABEL, CUSTOM_ENTRY_OPTION_ID, CUSTOM_ENTRY_SUMMARY, CUSTOM_RULE_OPTION_ID,
    CustomMechanicDraft, CustomMechanicRecord, EffectTemplateValidator, augment_space_with_points,
    custom_decision_id,
};
pub use engine::{AuthoringEngine, PrefillReport, PrefillSkip, WorkbenchResetReport};
pub use freeze::{
    FreezeGateReport, FrozenDesign, GateFinding, GateResult, evaluate_freeze_gates, execute_freeze,
    run_red_team,
};
pub use interview::{
    InterviewProgress, InterviewProposal, InterviewService, InterviewTurn, LevelProgress,
};
pub use state::{
    AuthoringState, Finding, FindingDisposition, InterviewEntry, InterviewState, NaSignoff,
    RedTeamRecord, TemplateMode,
};
