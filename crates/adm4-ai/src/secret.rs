use adm4_foundation::{Adm4Error, Adm4Result};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fmt;

/// 密钥引用：`env:NAME` 或 `named:NAME`。原始密钥不进存档、不进日志。
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "ref", rename_all = "snake_case")]
pub enum SecretRef {
    Env { name: String },
    Named { name: String },
}

impl SecretRef {
    pub fn parse(text: &str) -> Adm4Result<Self> {
        if let Some(name) = text.strip_prefix("env:") {
            return Ok(Self::Env { name: name.into() });
        }
        if let Some(name) = text.strip_prefix("named:") {
            return Ok(Self::Named { name: name.into() });
        }
        Err(Adm4Error::invalid_input(format!(
            "secret ref must be env:NAME or named:NAME, got {text}"
        )))
    }

    /// 解析实际密钥；named 密钥从注入的表（config/secrets.json 内容）查找。
    pub fn resolve(&self, named_secrets: &BTreeMap<String, String>) -> Adm4Result<String> {
        match self {
            Self::Env { name } => std::env::var(name)
                .map_err(|_| Adm4Error::not_found(format!("environment variable {name} not set"))),
            Self::Named { name } => named_secrets
                .get(name)
                .cloned()
                .ok_or_else(|| Adm4Error::not_found(format!("named secret {name} not found"))),
        }
    }
}

impl fmt::Debug for SecretRef {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Env { name } => write!(formatter, "SecretRef::Env({name})"),
            Self::Named { name } => write!(formatter, "SecretRef::Named({name})"),
        }
    }
}
