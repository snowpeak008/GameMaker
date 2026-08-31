//! V4 应用编排层：项目生命周期、创作、冻结、流水线、逆向、日志。
//! GUI/CLI 只调用本层，不含业务规则。

mod change;
mod config;
mod deliverable;
mod pipeline_artifact;
mod runlog;
mod sdk;
mod services;

/// 设计阶段风格门的契约与只读投影。
///
/// 类型定义在 `adm4-build`（那里才是 Phase 2 美术线的家），经门面**再导出**：
/// CLI/GUI 只认 `adm4-app` 这一道门（`lib.rs` 顶部那句「GUI/CLI 只调用本层」），
/// 不必为了几个结构体再挂一条 `adm4-build` 依赖。
pub use adm4_build::art::style_anchor::{
    ANCHOR_SET_FILE, APPLICATION_CONTRACT_FILE, CONFIRMATION_FILE, FIT_REPORT_FILE, MAX_DIRECTIONS,
    MIN_DIRECTIONS, PROMPT_SUMMARY_CHARS, SELECTED_ANCHOR_ROLE,
    STYLE_APPLICATION_CONTRACT_NOT_APPROVED, STYLE_SECTION, StyleAnchorImage, StyleAnchorSet,
    StyleApplicationContract, StyleConfirmation, StyleConstraint, StyleDirection,
    StyleDirectionStatus, StyleFitEntry, StyleFitReport, StyleFitRisk, StyleGateStatus,
    StyleGenerationItem, StyleGenerationOptions, StyleGenerationRound, StyleLockOutcome,
    StylePreview, StyleReadiness, StyleRoundKind, StyleSession, StyleUsage, style_presets,
};
pub use change::{ChangeLog, ChangeRequest, ChangeStatus};
pub use config::{
    AppConfig, list_named_secret_names, load_config, load_named_secrets, save_config,
    save_named_secret,
};
pub use deliverable::{DeliverableManifest, DeliverableSegment};
pub use pipeline_artifact::{
    ArtifactFileView, CONTRACT_FILE, DOCUMENT_FILE, DOCUMENT_PREVIEW_LIMIT_BYTES, StageArtifactView,
};
pub use runlog::{RunLog, RunLogEntry};
pub use sdk::{SdkKnowledgeBase, SdkRecord, SdkReviewStatus, SdkSnapshot};
pub use services::{
    AI_INVOKE_CHECK_PURPOSE, AiDoctorReport, AiInvokeCheckReport, AppServices, BuildStageView,
    DecisionOptionView, DecisionPointView, ExemptionView, GateSummary, InterviewTurnDto,
    MissingByDomain, MissingEntry, NodeRiskNote, ProfileField, ProfileOption, ProjectDoctorReport,
    ProjectProfile, RedTeamFinding, RedTeamSummary, RowReferenceIssue, TemplateCompareEntry,
    TemplateComparison, TemplateExportReport, WorkbenchOverview, WorkbenchRisk, WorkbenchSummary,
    WorkbenchValidation,
};
