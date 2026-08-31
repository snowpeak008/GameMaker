//! V4 AI 层：Provider 抽象、OpenAI 兼容 HTTP、密钥引用、调用日志。
//!
//! 两条平级通道：文本（[`AiProvider`]，chat completions）与图像（[`ImageProvider`]，
//! images API）。图像单列成一条通道而不是塞进文本 trait，理由见 [`image`] 模块文档。
//!
//! 红线 R7：失败即 Err，接口不提供任何降级参数；语义失败不重试不兜底。
//! 图像通道尤其如此——生成失败就是失败，绝不产占位图冒充真图（占位图只由显式的
//! [`ScriptedImageProvider`] 产出，且它的 provider id 会随生成记录落盘）。

mod http;
mod image;
mod provider;
mod secret;
mod testkit;

pub use http::{HttpProviderConfig, OpenAiCompatibleProvider, provider_presets};
pub use image::{
    HttpImageProviderConfig, ImageArtifact, ImageProvider, ImageRequest,
    OpenAiCompatibleImageProvider, image_provider_presets, media_type_extension,
    sniff_image_media_type,
};
pub use provider::{AiCapability, AiProvider, AiRequest, AiResponse, InvocationRecord};
pub use secret::SecretRef;
pub use testkit::{ScriptedImageProvider, ScriptedProvider};
