#![forbid(unsafe_code)]

use serde::{Deserialize, Serialize};

pub const CONTRACT_FAMILY: &str = "view";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskView {
    Design,
    Pipeline,
    Patch,
    Package,
    Logs,
    Sdk,
}

impl TaskView {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Design => "design",
            Self::Pipeline => "pipeline",
            Self::Patch => "patch",
            Self::Package => "package",
            Self::Logs => "logs",
            Self::Sdk => "sdk",
        }
    }
}
