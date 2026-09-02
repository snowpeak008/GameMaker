use adm4_ai::{HttpImageProviderConfig, HttpProviderConfig};
use adm4_archive::DataRoot;
use adm4_foundation::{Adm4Error, Adm4Result, read_json_file, write_json_file};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct AppConfig {
    /// 设计空间根目录（相对 cwd 或绝对路径）。
    #[serde(default = "default_design_space_root")]
    pub design_space_root: String,
    /// 激活的 AI Provider 配置（无则 AI 相关功能 blocked）。
    #[serde(default)]
    pub ai_provider: Option<HttpProviderConfig>,
    /// 激活的**图像**Provider 配置（无则风格门的生成入口显式 blocked，不产占位图）。
    ///
    /// 与 `ai_provider` 分开配置而不是复用它：图像 API 的 base_url、模型名、超时量级
    /// 与文本 API 都不一样（同一个厂商也是两个不同的 endpoint 与两套模型名）。
    /// 合成一个字段会逼用户在文本可用时假装图像也可用——那正是 R7 关心的误报。
    #[serde(default)]
    pub image_provider: Option<HttpImageProviderConfig>,
    /// 激活的引擎后端配置（无则 P1 的引擎预检如实 Blocked：未配置引擎，不跑现场开发）。
    ///
    /// 只登记一个字符串 `id`：门面按它挑后端实现，治理层不认得任何具体引擎（D17）。
    /// 本波没有任何真实后端实现可挑，配置了也只会得到「后端 id 无对应实现」的诚实阻塞。
    #[serde(default)]
    pub engine_backend: Option<EngineBackendConfig>,
}

/// 引擎后端配置：只有后端标识。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct EngineBackendConfig {
    /// 后端标识（写进 P0 种子的 `engine_id`，与后端实现的 `id()` 对应）。
    pub id: String,
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
            image_provider: None,
            engine_backend: None,
        });
    }
    read_json_file(&path)
}

pub fn save_config(data_root: &DataRoot, config: &AppConfig) -> Adm4Result<()> {
    write_json_file(&data_root.config_dir().join("app.json"), config)
}

/// named secrets（config/secrets.json）；env 引用不落盘。
pub fn load_named_secrets(data_root: &DataRoot) -> Adm4Result<BTreeMap<String, String>> {
    read_json_file_or_default(&secrets_path(data_root))
}

/// 写入一条 named secret（`config/secrets.json`），保留同文件里的其它条目。
///
/// 只有这一个写入通道：此前 V4 只能读，桌面端因此只能让用户手工维护 secrets.json
/// （二版有 `save-ai-secret`，四版零归宿）。
///
/// **密钥值不得出现在任何返回值、日志、报告或存档里**：本函数除了 `Ok(())` 什么都不回，
/// 调用方拿不到可以顺手打印的东西；名字与长度这类元信息由调用方自己从入参取。
/// 落点是数据根的 `config/`（不是项目存档内容树），因此密钥不进存档、不进导出包、
/// 不进内容指纹。
pub fn save_named_secret(data_root: &DataRoot, name: &str, value: &str) -> Adm4Result<()> {
    let name = name.trim();
    if name.is_empty() {
        return Err(Adm4Error::invalid_input(
            "密钥名不能为空（配置里以 named:<名字> 引用）",
        ));
    }
    if name.contains(char::is_whitespace) {
        return Err(Adm4Error::invalid_input(format!(
            "密钥名「{name}」不能含空白字符（named:<名字> 引用按整名匹配）"
        )));
    }
    // 空值是最容易被忽略的错配：写进去之后「已配置」为真而调用必然 401。
    if value.is_empty() {
        return Err(Adm4Error::invalid_input(format!(
            "密钥 {name} 的值为空——空密钥会让配置看着可用而实际调用必然失败"
        )));
    }
    let mut secrets = load_named_secrets(data_root)?;
    secrets.insert(name.to_string(), value.to_string());
    write_json_file(&secrets_path(data_root), &secrets)
}

/// 已登记的 named secret 名字（**不含值**）：面板/CLI 展示「配了哪些密钥」用。
pub fn list_named_secret_names(data_root: &DataRoot) -> Adm4Result<Vec<String>> {
    Ok(load_named_secrets(data_root)?.into_keys().collect())
}

fn secrets_path(data_root: &DataRoot) -> PathBuf {
    data_root.config_dir().join("secrets.json")
}

fn read_json_file_or_default(path: &Path) -> Adm4Result<BTreeMap<String, String>> {
    if !path.is_file() {
        return Ok(BTreeMap::new());
    }
    read_json_file(path)
}
