//! V4 应用编排层：项目生命周期、创作、冻结、流水线、逆向、日志。
//! GUI/CLI 只调用本层，不含业务规则。

mod change;
mod config;
mod deliverable;
mod pipeline_artifact;
mod runlog;
mod sdk;
mod services;

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
    AI_INVOKE_CHECK_PURPOSE, AiDoctorReport, AiInvokeCheckReport, AppServices, DecisionOptionView,
    DecisionPointView, ExemptionView, GateSummary, InterviewTurnDto, MissingByDomain, MissingEntry,
    NodeRiskNote, ProfileField, ProfileOption, ProjectDoctorReport, ProjectProfile, RedTeamFinding,
    RedTeamSummary, RowReferenceIssue, TemplateCompareEntry, TemplateComparison,
    TemplateExportReport, WorkbenchOverview, WorkbenchRisk, WorkbenchSummary, WorkbenchValidation,
};
