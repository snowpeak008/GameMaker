//! V4 应用编排层：项目生命周期、创作、冻结、流水线、逆向、日志。
//! GUI/CLI 只调用本层，不含业务规则。

mod config;
mod runlog;
mod services;

pub use config::{AppConfig, load_config, load_named_secrets, save_config};
pub use runlog::{RunLog, RunLogEntry};
pub use services::{AppServices, InterviewTurnDto, TemplateCompareEntry, TemplateComparison};
