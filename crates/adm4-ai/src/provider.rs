use adm4_foundation::Adm4Result;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AiCapability {
    Text,
    Structured,
    Review,
    Image,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AiRequest {
    /// 调用意图标识（访谈提案/红队/叙述/命名/生图……），进 journal。
    pub purpose: String,
    pub system_prompt: String,
    pub user_prompt: String,
    /// 期望 JSON 输出时的提示（Provider 侧尽力，调用方仍须解析校验）。
    #[serde(default)]
    pub expect_json: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AiResponse {
    pub text: String,
    pub provider_id: String,
    pub model: String,
}

/// AI Provider：失败即 Err（R7：无降级参数、无静默兜底）。
pub trait AiProvider: Send + Sync {
    fn id(&self) -> &str;
    fn capabilities(&self) -> &[AiCapability];
    fn invoke(&self, request: &AiRequest) -> Adm4Result<AiResponse>;
}

/// 调用日志条目（不含密钥）。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct InvocationRecord {
    pub purpose: String,
    pub provider_id: String,
    pub model: String,
    pub prompt_chars: usize,
    pub response_chars: usize,
    pub succeeded: bool,
    pub error: Option<String>,
    pub at: String,
}
