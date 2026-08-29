use adm4_ai::HttpProviderConfig;
use adm4_archive::DataRoot;
use adm4_foundation::{Adm4Result, read_json_file, write_json_file};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct AppConfig {
    /// 设计空间根目录（相对 cwd 或绝对路径）。
    #[serde(default = "default_design_space_root")]
    pub design_space_root: String,
    /// 激活的 AI Provider 配置（无则 AI 相关功能 blocked）。
    #[serde(default)]
    pub ai_provider: Option<HttpProviderConfig>,
}

fn default_design_space_root() -> String {
    "knowledge/design_space".into()
}

pub fn load_config(data_root: &DataRoot) -> Adm4Result<AppConfig> {
    let path = data_root.config_dir().join("app.json");
    if !path.is_file() {
        return Ok(AppConfig {
            design_space_root: default_design_space_root(),
            ai_provider: None,
        });
    }
    read_json_file(&path)
}

pub fn save_config(data_root: &DataRoot, config: &AppConfig) -> Adm4Result<()> {
    write_json_file(&data_root.config_dir().join("app.json"), config)
}

/// named secrets（config/secrets.json）；env 引用不落盘。
pub fn load_named_secrets(data_root: &DataRoot) -> Adm4Result<BTreeMap<String, String>> {
    let path = data_root.config_dir().join("secrets.json");
    if !path.is_file() {
        return Ok(BTreeMap::new());
    }
    read_json_file(&path)
}
