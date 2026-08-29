//! V4 AI 层：Provider 抽象、OpenAI 兼容 HTTP、密钥引用、调用日志。
//!
//! 红线 R7：失败即 Err，接口不提供任何降级参数；语义失败不重试不兜底。

mod http;
mod provider;
mod secret;
mod testkit;

pub use http::{HttpProviderConfig, OpenAiCompatibleProvider, provider_presets};
pub use provider::{AiCapability, AiProvider, AiRequest, AiResponse, InvocationRecord};
pub use secret::SecretRef;
pub use testkit::ScriptedProvider;
