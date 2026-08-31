//! 图像生成通道：与文本 Provider 平级的第二条 AI 通道。
//!
//! 为什么与 [`crate::AiProvider`] 分开而不是塞进同一个 trait：文本调用的输入输出是字符串，
//! 图像调用的输出是**字节**（没有「文本应答」这回事），两者的错误面、超时量级、配置项
//! （尺寸）都不一样。合成一个 trait 会让每个实现都被迫为自己不支持的那一半返回
//! 「不支持」——那正是 R7 想避免的「接口上看着能用、真调用才知道不行」。
//!
//! 红线 R7 在本模块的落法：**生成失败就是失败**。没有任何降级参数，不产占位图冒充真图，
//! 不静默重试。占位图只由 [`crate::ScriptedImageProvider`] 产出，而它是显式的测试替身
//! （id 为 `scripted_image`，落盘记录里看得见），不会被误当成真实生成结果。

use crate::provider::AiCapability;
use adm4_foundation::{Adm4Error, Adm4Result};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::time::Duration;

/// 一次图像生成请求。
///
/// `width`/`height` 是**请求**尺寸：本层不解码图像，因此谁都不许声称它是实际尺寸
/// （见 [`ImageArtifact`] 为何不带尺寸字段）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ImageRequest {
    /// 调用意图标识（进调用日志与生成记录，与文本通道的 `AiRequest::purpose` 同款）。
    pub purpose: String,
    pub prompt: String,
    pub width: u32,
    pub height: u32,
}

impl ImageRequest {
    /// 入参自检：空提示词与零尺寸一律拒（R2：宁可报错也不发一次注定无意义的请求）。
    ///
    /// 两个实现都在 `generate` 开头调它，因此「打过去才报错」与「本地就报错」的判定
    /// 只有一份口径，脚本 Provider 与真实 Provider 的负例行为一致。
    pub fn validate(&self) -> Adm4Result<()> {
        if self.purpose.trim().is_empty() {
            return Err(Adm4Error::invalid_input(
                "图像生成请求缺 purpose：调用意图要进生成记录，缺了就无从追溯这张图是谁要的",
            ));
        }
        if self.prompt.trim().is_empty() {
            return Err(Adm4Error::invalid_input(
                "图像生成提示词为空：空提示词生成出来的图与设计无关，不发这次请求",
            ));
        }
        if self.width == 0 || self.height == 0 {
            return Err(Adm4Error::invalid_input(format!(
                "图像尺寸非法（{}x{}）：宽高都必须为正",
                self.width, self.height
            )));
        }
        Ok(())
    }
}

/// 生成出来的图像。
///
/// **刻意不带宽高**：本层不解码图像字节，写上尺寸就等于把「请求的尺寸」当成
/// 「实际拿到的尺寸」上报——模型返回别的尺寸时那是一句假话。需要请求尺寸的调用方
/// 自己从 [`ImageRequest`] 取（它才是那个事实的出处）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImageArtifact {
    pub bytes: Vec<u8>,
    /// 由字节头嗅探得出（[`sniff_image_media_type`]），不是配置里声称的格式。
    pub media_type: String,
    pub provider_id: String,
    pub model: String,
}

/// 图像 Provider：失败即 Err（R7：无降级参数、无占位兜底）。
pub trait ImageProvider: Send + Sync {
    fn id(&self) -> &str;
    fn capabilities(&self) -> &[AiCapability];
    fn generate(&self, request: &ImageRequest) -> Adm4Result<ImageArtifact>;
}

/// 图像通道配置（`config/app.json` 的 `image_provider` 段）。
///
/// 字段与文本通道的 [`crate::HttpProviderConfig`] 逐项对齐（同样的 `provider_id` /
/// `base_url` / `model` / `api_key_ref` / `timeout_secs` 语义），多出来的只有 `size`：
/// 图像 API 的尺寸是**服务端**参数，同一个模型往往只接受几种固定尺寸，因此它属于配置
/// 而不是每次调用现编。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HttpImageProviderConfig {
    pub provider_id: String,
    pub base_url: String,
    pub model: String,
    /// 密钥引用文本（env:NAME / named:NAME），解析后的密钥只存内存。
    pub api_key_ref: String,
    #[serde(default = "default_image_timeout_secs")]
    pub timeout_secs: u64,
    /// 生成尺寸，形如 `1024x1024`（OpenAI 兼容 images API 的 `size` 参数）。
    #[serde(default = "default_image_size")]
    pub size: String,
}

/// 图像生成比文本慢一个量级（几十秒是常态），因此默认超时比文本通道宽。
fn default_image_timeout_secs() -> u64 {
    300
}

fn default_image_size() -> String {
    "1024x1024".to_string()
}

impl HttpImageProviderConfig {
    /// 把配置里的 `size` 解析成宽高。
    ///
    /// 解析失败**直接报错**而不回落到默认尺寸：写错了尺寸却按 1024 跑，用户会拿到一批
    /// 尺寸不对的锚图而毫不知情（R2）。
    pub fn parse_size(&self) -> Adm4Result<(u32, u32)> {
        let text = self.size.trim();
        let (width, height) = text.split_once(['x', 'X', '*']).ok_or_else(|| {
            Adm4Error::invalid_input(format!("图像尺寸配置「{text}」格式不对：应形如 1024x1024"))
        })?;
        let parse = |value: &str, what: &str| -> Adm4Result<u32> {
            value.trim().parse::<u32>().map_err(|error| {
                Adm4Error::invalid_input(format!(
                    "图像尺寸配置「{text}」的{what}不是正整数：{error}"
                ))
            })
        };
        let width = parse(width, "宽")?;
        let height = parse(height, "高")?;
        if width == 0 || height == 0 {
            return Err(Adm4Error::invalid_input(format!(
                "图像尺寸配置「{text}」非法：宽高都必须为正"
            )));
        }
        Ok((width, height))
    }
}

/// 内置图像 preset（供配置面板/CLI 套用后按实际改）。
pub fn image_provider_presets() -> Vec<HttpImageProviderConfig> {
    vec![
        HttpImageProviderConfig {
            provider_id: "openai_images".into(),
            base_url: "https://api.openai.com/v1".into(),
            model: "gpt-image-1".into(),
            api_key_ref: "env:OPENAI_API_KEY".into(),
            timeout_secs: 300,
            size: "1024x1024".into(),
        },
        HttpImageProviderConfig {
            provider_id: "local_openai_images".into(),
            base_url: "http://127.0.0.1:1234/v1".into(),
            model: "local-image-model".into(),
            api_key_ref: "env:LOCAL_OPENAI_API_KEY".into(),
            timeout_secs: 600,
            size: "512x512".into(),
        },
    ]
}

/// 按字节头判定图像格式。
///
/// 为什么必须嗅探而不信配置：落盘要给文件起名（`.png` / `.jpg`），而桌面端按扩展名
/// 加载。信「配置说是 PNG」的话，模型换成 JPEG 输出时会落一个内容是 JPEG 的 `.png`，
/// 界面上就是一张打不开的图，而日志里一切正常。认不出的格式**报错**而不是落
/// `.bin`：一张下游打不开的锚图不是资产，是一个静默故障。
pub fn sniff_image_media_type(bytes: &[u8]) -> Adm4Result<&'static str> {
    const PNG: &[u8] = b"\x89PNG\r\n\x1a\n";
    if bytes.starts_with(PNG) {
        return Ok("image/png");
    }
    if bytes.starts_with(&[0xFF, 0xD8, 0xFF]) {
        return Ok("image/jpeg");
    }
    if bytes.len() >= 12 && bytes.starts_with(b"RIFF") && &bytes[8..12] == b"WEBP" {
        return Ok("image/webp");
    }
    Err(Adm4Error::validation(format!(
        "返回的图像字节不是 PNG/JPEG/WEBP（前 {} 字节无法识别）：\
         无法确定落盘扩展名，按失败处理而不落一个下游打不开的文件",
        bytes.len().min(12)
    )))
}

/// 图像 media type → 落盘扩展名（不含点）。认不出的 media type 报错。
pub fn media_type_extension(media_type: &str) -> Adm4Result<&'static str> {
    match media_type {
        "image/png" => Ok("png"),
        "image/jpeg" => Ok("jpg"),
        "image/webp" => Ok("webp"),
        other => Err(Adm4Error::validation(format!(
            "未知图像 media type「{other}」：无法确定落盘扩展名"
        ))),
    }
}

/// OpenAI 兼容 images API Provider（阻塞式）。
///
/// 走 `POST {base_url}/images/generations`，优先请求 `b64_json`（一次往返拿到字节）；
/// 服务端只给 `url` 时再取一次 URL 的字节——两条路都拿不到就是失败，不产任何占位图。
pub struct OpenAiCompatibleImageProvider {
    config: HttpImageProviderConfig,
    api_key: String,
    capabilities: Vec<AiCapability>,
    client: reqwest::blocking::Client,
}

impl OpenAiCompatibleImageProvider {
    pub fn new(config: HttpImageProviderConfig, api_key: String) -> Adm4Result<Self> {
        // 尺寸在构造时就校验：等到生成时才发现配置写错，用户已经等了一次超时。
        config.parse_size()?;
        let client = reqwest::blocking::Client::builder()
            .timeout(Duration::from_secs(config.timeout_secs))
            .build()
            .map_err(|error| Adm4Error::internal(format!("图像 http client 构建失败：{error}")))?;
        Ok(Self {
            config,
            api_key,
            capabilities: vec![AiCapability::Image],
            client,
        })
    }

    /// 配置声明的生成尺寸（构造时已校验，这里不会失败）。
    pub fn configured_size(&self) -> Adm4Result<(u32, u32)> {
        self.config.parse_size()
    }

    fn fetch_url_bytes(&self, url: &str) -> Adm4Result<Vec<u8>> {
        let response = self.client.get(url).send().map_err(|error| {
            Adm4Error::ai_unavailable(format!("取图像 URL 失败（{url}）：{error}"))
        })?;
        let status = response.status();
        if !status.is_success() {
            return Err(Adm4Error::ai_unavailable(format!(
                "取图像 URL 返回状态 {status}（{url}）"
            )));
        }
        let bytes = response.bytes().map_err(|error| {
            Adm4Error::ai_unavailable(format!("读图像 URL 响应体失败（{url}）：{error}"))
        })?;
        Ok(bytes.to_vec())
    }
}

impl ImageProvider for OpenAiCompatibleImageProvider {
    fn id(&self) -> &str {
        &self.config.provider_id
    }

    fn capabilities(&self) -> &[AiCapability] {
        &self.capabilities
    }

    fn generate(&self, request: &ImageRequest) -> Adm4Result<ImageArtifact> {
        request.validate()?;
        let url = format!(
            "{}/images/generations",
            self.config.base_url.trim_end_matches('/')
        );
        let body = json!({
            "model": self.config.model,
            "prompt": request.prompt,
            "n": 1,
            "size": format!("{}x{}", request.width, request.height),
            "response_format": "b64_json",
        });
        let response = self
            .client
            .post(&url)
            .bearer_auth(&self.api_key)
            .json(&body)
            .send()
            .map_err(|error| Adm4Error::ai_unavailable(format!("图像生成请求失败：{error}")))?;
        let status = response.status();
        let payload: serde_json::Value = response
            .json()
            .map_err(|error| Adm4Error::ai_unavailable(format!("图像生成响应解析失败：{error}")))?;
        if !status.is_success() {
            return Err(Adm4Error::ai_unavailable(format!(
                "图像 Provider 返回状态 {status}：{}",
                payload
                    .get("error")
                    .and_then(|error| error.get("message"))
                    .and_then(|message| message.as_str())
                    .unwrap_or("（响应里没有 error.message）")
            )));
        }
        let first = payload
            .get("data")
            .and_then(|data| data.get(0))
            .ok_or_else(|| Adm4Error::ai_unavailable("图像生成响应缺 data[0]：没有拿到任何图像"))?;
        let bytes = match first.get("b64_json").and_then(|value| value.as_str()) {
            Some(encoded) => decode_base64(encoded)?,
            None => {
                let image_url = first
                    .get("url")
                    .and_then(|value| value.as_str())
                    .ok_or_else(|| {
                        Adm4Error::ai_unavailable(
                            "图像生成响应的 data[0] 既无 b64_json 也无 url：拿不到图像字节",
                        )
                    })?;
                self.fetch_url_bytes(image_url)?
            }
        };
        if bytes.is_empty() {
            return Err(Adm4Error::ai_unavailable(
                "图像生成返回了 0 字节：调用链通但产出不可用，按失败处理（不美化）",
            ));
        }
        let media_type = sniff_image_media_type(&bytes)?;
        Ok(ImageArtifact {
            bytes,
            media_type: media_type.to_string(),
            provider_id: self.config.provider_id.clone(),
            model: self.config.model.clone(),
        })
    }
}

/// 标准 base64 解码（含 `=` 填充；忽略换行与空白）。
///
/// 为什么手写：`b64_json` 是 OpenAI 兼容图像 API 的主路径，而任务卡禁止新增第三方依赖。
/// 解码是纯函数、二十来行、有负例测试兜着，比引一个 crate 的成本低。
/// **严格**实现：出现非法字符、长度不对、填充位置不对一律 Err（不跳过、不截断）——
/// 一段解错的字节落成图片就是一张打不开的锚图。
fn decode_base64(encoded: &str) -> Adm4Result<Vec<u8>> {
    fn value_of(byte: u8) -> Option<u32> {
        match byte {
            b'A'..=b'Z' => Some(u32::from(byte - b'A')),
            b'a'..=b'z' => Some(u32::from(byte - b'a') + 26),
            b'0'..=b'9' => Some(u32::from(byte - b'0') + 52),
            b'+' => Some(62),
            b'/' => Some(63),
            _ => None,
        }
    }

    let cleaned: Vec<u8> = encoded
        .bytes()
        .filter(|byte| !byte.is_ascii_whitespace())
        .collect();
    let (payload, padding) = match cleaned.iter().position(|byte| *byte == b'=') {
        Some(index) => {
            let padding = cleaned.len() - index;
            if padding > 2 || cleaned[index..].iter().any(|byte| *byte != b'=') {
                return Err(Adm4Error::validation(
                    "base64 图像数据的填充位不合法（'=' 只允许出现在末尾且最多两个）",
                ));
            }
            (&cleaned[..index], padding)
        }
        None => (&cleaned[..], 0),
    };
    if (payload.len() + padding) % 4 != 0 {
        return Err(Adm4Error::validation(format!(
            "base64 图像数据长度 {} 不是 4 的倍数：数据被截断了",
            payload.len() + padding
        )));
    }
    let mut decoded = Vec::with_capacity(payload.len() / 4 * 3);
    let mut accumulator: u32 = 0;
    let mut collected = 0u32;
    for byte in payload {
        let value = value_of(*byte).ok_or_else(|| {
            Adm4Error::validation(format!(
                "base64 图像数据含非法字符 0x{byte:02x}：不跳过、不猜，直接判失败"
            ))
        })?;
        accumulator = (accumulator << 6) | value;
        collected += 1;
        if collected == 4 {
            decoded.push((accumulator >> 16) as u8);
            decoded.push((accumulator >> 8) as u8);
            decoded.push(accumulator as u8);
            accumulator = 0;
            collected = 0;
        }
    }
    match collected {
        0 => {}
        2 => decoded.push((accumulator >> 4) as u8),
        3 => {
            decoded.push((accumulator >> 10) as u8);
            decoded.push((accumulator >> 2) as u8);
        }
        // 上面的长度校验已排除这一支（尾组只剩 1 个字符时长度必然不是 4 的倍数），
        // 留着是 fail-closed：将来若放宽长度校验，这里仍旧不许猜一个字节出来。
        other => {
            return Err(Adm4Error::validation(format!(
                "base64 图像数据的尾组只有 {other} 个字符：这不是合法的 base64"
            )));
        }
    }
    Ok(decoded)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn image_request_rejects_blank_prompt_and_zero_size() {
        let request = |prompt: &str, width: u32, height: u32| ImageRequest {
            purpose: "style_anchor_preview".into(),
            prompt: prompt.into(),
            width,
            height,
        };
        assert!(request("清晰量产风格", 512, 512).validate().is_ok());
        assert!(request("   ", 512, 512).validate().is_err());
        assert!(request("清晰量产风格", 0, 512).validate().is_err());
        assert!(request("清晰量产风格", 512, 0).validate().is_err());

        let mut no_purpose = request("清晰量产风格", 512, 512);
        no_purpose.purpose = "  ".into();
        assert!(no_purpose.validate().is_err());
    }

    #[test]
    fn size_parses_and_rejects_garbage_without_falling_back() {
        let config = |size: &str| HttpImageProviderConfig {
            provider_id: "p".into(),
            base_url: "http://localhost/v1".into(),
            model: "m".into(),
            api_key_ref: "env:K".into(),
            timeout_secs: 300,
            size: size.into(),
        };
        assert_eq!(
            config("1024x1024").parse_size().expect("正例"),
            (1024, 1024)
        );
        assert_eq!(
            config(" 512X768 ").parse_size().expect("大写 X"),
            (512, 768)
        );
        assert_eq!(config("256*256").parse_size().expect("星号"), (256, 256));
        // 写错不许回落到默认尺寸（那会静默产出一批尺寸不对的锚图）。
        for bad in ["1024", "axb", "0x512", "512x0", "", "1024x"] {
            assert!(config(bad).parse_size().is_err(), "「{bad}」应被拒");
        }
    }

    #[test]
    fn presets_are_parseable_and_carry_image_capability_defaults() {
        let presets = image_provider_presets();
        assert_eq!(presets.len(), 2);
        for preset in &presets {
            assert!(preset.parse_size().is_ok(), "{}", preset.provider_id);
            assert!(preset.api_key_ref.starts_with("env:"));
        }
    }

    /// 旧档兼容：只有四个必填字段的配置照旧可读，超时与尺寸落默认值。
    #[test]
    fn legacy_config_without_timeout_and_size_parses() {
        let legacy = r#"{
          "provider_id": "openai_images",
          "base_url": "https://api.openai.com/v1",
          "model": "gpt-image-1",
          "api_key_ref": "env:OPENAI_API_KEY"
        }"#;
        let parsed: HttpImageProviderConfig =
            serde_json::from_str(legacy).expect("旧档图像配置应可解析");
        assert_eq!(parsed.timeout_secs, 300);
        assert_eq!(parsed.size, "1024x1024");
    }

    #[test]
    fn media_type_sniffing_covers_png_jpeg_webp_and_rejects_unknown() {
        let mut png = b"\x89PNG\r\n\x1a\n".to_vec();
        png.extend_from_slice(&[0u8; 8]);
        assert_eq!(sniff_image_media_type(&png).expect("PNG"), "image/png");
        assert_eq!(
            sniff_image_media_type(&[0xFF, 0xD8, 0xFF, 0xE0]).expect("JPEG"),
            "image/jpeg"
        );
        let mut webp = b"RIFF".to_vec();
        webp.extend_from_slice(&[0, 0, 0, 0]);
        webp.extend_from_slice(b"WEBP");
        assert_eq!(sniff_image_media_type(&webp).expect("WEBP"), "image/webp");
        // 认不出的格式必须报错（不落一个下游打不开的 .bin）。
        assert!(sniff_image_media_type(b"not an image").is_err());
        assert!(sniff_image_media_type(&[]).is_err());

        assert_eq!(media_type_extension("image/png").expect("png"), "png");
        assert_eq!(media_type_extension("image/jpeg").expect("jpg"), "jpg");
        assert_eq!(media_type_extension("image/webp").expect("webp"), "webp");
        assert!(media_type_extension("application/octet-stream").is_err());
    }

    #[test]
    fn base64_decodes_all_padding_lengths_and_ignores_whitespace() {
        // 三种填充长度各来一条（padding 0/1/2）。
        assert_eq!(decode_base64("YWJj").expect("无填充"), b"abc");
        assert_eq!(decode_base64("YWJjZA==").expect("两个填充"), b"abcd");
        assert_eq!(decode_base64("YWJjZGU=").expect("一个填充"), b"abcde");
        assert_eq!(decode_base64("").expect("空串"), Vec::<u8>::new());
        // 真实响应常带换行，空白一律忽略。
        assert_eq!(decode_base64("YWJj\n  ZA==\n").expect("含空白"), b"abcd");
        // PNG 头的 base64 往返。
        let png_head = b"\x89PNG\r\n\x1a\n";
        assert_eq!(
            decode_base64("iVBORw0KGgo=").expect("PNG 头"),
            png_head.to_vec()
        );
    }

    #[test]
    fn base64_rejects_illegal_input_instead_of_guessing() {
        // 非法字符（不跳过）。
        assert!(decode_base64("YW*j").is_err());
        // 长度不是 4 的倍数（数据被截断）。
        assert!(decode_base64("YWJjZ").is_err());
        // 填充数与有效字符数对不上（6 + 1 不是 4 的倍数）。
        assert!(decode_base64("YWJjZA=").is_err());
        // 填充在中间、或超过两个。
        assert!(decode_base64("YW=jZA==").is_err());
        assert!(decode_base64("YWJj====").is_err());
    }
}
