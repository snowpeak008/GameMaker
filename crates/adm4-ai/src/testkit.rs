use crate::image::{ImageArtifact, ImageProvider, ImageRequest};
use crate::provider::{AiCapability, AiProvider, AiRequest, AiResponse};
use adm4_foundation::{Adm4Error, Adm4Result, sha256_hex};
use std::collections::BTreeMap;
use std::sync::Mutex;

/// 确定性脚本 Provider：按 purpose 回放固定应答。测试/离线演示用，杜绝测试依赖真实网络。
pub struct ScriptedProvider {
    responses: Mutex<BTreeMap<String, Vec<String>>>,
}

impl ScriptedProvider {
    pub fn new() -> Self {
        Self {
            responses: Mutex::new(BTreeMap::new()),
        }
    }

    /// 为某 purpose 注册应答队列（按序弹出，弹尽复用最后一条）。
    ///
    /// 锁中毒（此前有线程持锁 panic）时取回内部数据继续注册，不再 panic：
    /// 队列本身是纯数据，中毒的锁不代表数据损坏。
    pub fn script(&self, purpose: &str, responses: Vec<String>) {
        let mut guard = match self.responses.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        };
        guard.insert(purpose.to_string(), responses);
    }
}

impl Default for ScriptedProvider {
    fn default() -> Self {
        Self::new()
    }
}

impl AiProvider for ScriptedProvider {
    fn id(&self) -> &str {
        "scripted"
    }

    fn capabilities(&self) -> &[AiCapability] {
        &[
            AiCapability::Text,
            AiCapability::Structured,
            AiCapability::Review,
        ]
    }

    fn invoke(&self, request: &AiRequest) -> Adm4Result<AiResponse> {
        let mut guard = self.responses.lock().map_err(|_| {
            Adm4Error::internal(
                "脚本 Provider 的应答队列锁已中毒（此前有线程 panic），无法回放应答",
            )
        })?;
        let queue = guard.get_mut(&request.purpose).ok_or_else(|| {
            Adm4Error::ai_unavailable(format!(
                "scripted provider has no response for purpose {}",
                request.purpose
            ))
        })?;
        let text = if queue.len() > 1 {
            queue.remove(0)
        } else {
            queue
                .first()
                .cloned()
                .ok_or_else(|| Adm4Error::ai_unavailable("scripted response queue empty"))?
        };
        Ok(AiResponse {
            text,
            provider_id: "scripted".into(),
            model: "scripted".into(),
        })
    }
}

/// 确定性脚本图像 Provider：按提示词与尺寸算出一张可显示的占位 PNG，零网络。
///
/// # 它是显式的测试替身，不是「生成失败时的兜底」
///
/// `id()` 恒为 `scripted_image`，而生成记录会把 provider id 落盘，所以任何一张由它产出的
/// 图在存档里都指名道姓写着「这是脚本占位图」。真实通道失败时**绝不**回落到它
/// （R7：生成失败就是失败）——那条路在 `adm4-app` 侧压根不存在：调用方要么拿到
/// 真实 Provider，要么拿到显式的 Err。
///
/// # 为什么产出真 PNG 而不是随便几个字节
///
/// 桌面端的风格网格要真把图画出来。若占位图不是合法 PNG，无图像 API 的开发者连界面
/// 走查都做不了（看到的是一片加载失败），风格门的交互就无法在离线环境验收。
/// PNG 编码在这里是手写的（存储型 deflate + CRC32 + adler32，约百行纯函数），
/// 因为任务卡禁止新增第三方依赖，而这点代码有确定性测试钉着。
/// 克隆共享同一份调用记录与失败脚本（`Arc`）：测试常把一个克隆 `Box` 进服务层、
/// 留原件断言"到底发了几次调用"——两者必须看见同一份账。
#[derive(Clone)]
pub struct ScriptedImageProvider {
    /// 已收到的请求（按序），供测试断言「服务层到底发了什么提示词、什么尺寸」。
    calls: std::sync::Arc<Mutex<Vec<ImageRequest>>>,
    /// 非 None 时每次生成都按该原因失败：用于验证 R7 的失败原样上抛路径。
    failure: std::sync::Arc<Mutex<Option<String>>>,
}

impl ScriptedImageProvider {
    pub fn new() -> Self {
        Self {
            calls: std::sync::Arc::new(Mutex::new(Vec::new())),
            failure: std::sync::Arc::new(Mutex::new(None)),
        }
    }

    /// 让后续每次生成都失败（模拟图像 API 报错），用于测试失败原样上抛。
    pub fn fail_with(&self, reason: impl Into<String>) {
        let mut guard = match self.failure.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        };
        *guard = Some(reason.into());
    }

    /// 已收到的请求（克隆快照）。
    pub fn calls(&self) -> Vec<ImageRequest> {
        match self.calls.lock() {
            Ok(guard) => guard.clone(),
            Err(poisoned) => poisoned.into_inner().clone(),
        }
    }
}

impl Default for ScriptedImageProvider {
    fn default() -> Self {
        Self::new()
    }
}

impl ImageProvider for ScriptedImageProvider {
    fn id(&self) -> &str {
        "scripted_image"
    }

    fn capabilities(&self) -> &[AiCapability] {
        &[AiCapability::Image]
    }

    fn generate(&self, request: &ImageRequest) -> Adm4Result<ImageArtifact> {
        request.validate()?;
        {
            let mut guard = self.calls.lock().map_err(|_| {
                Adm4Error::internal("脚本图像 Provider 的调用记录锁已中毒（此前有线程 panic）")
            })?;
            guard.push(request.clone());
        }
        let failure = match self.failure.lock() {
            Ok(guard) => guard.clone(),
            Err(poisoned) => poisoned.into_inner().clone(),
        };
        if let Some(reason) = failure {
            return Err(Adm4Error::ai_unavailable(reason));
        }
        let bytes = placeholder_png(request.width, request.height, &request.prompt)?;
        Ok(ImageArtifact {
            bytes,
            media_type: "image/png".to_string(),
            provider_id: "scripted_image".to_string(),
            model: "scripted_image".to_string(),
        })
    }
}

/// 由提示词确定性派生一张 `width`x`height` 的占位 PNG（8 位真彩、存储型 deflate）。
///
/// 同一提示词 + 同一尺寸 → 逐字节相同；不同提示词 → 配色不同（便于肉眼区分方向卡）。
fn placeholder_png(width: u32, height: u32, prompt: &str) -> Adm4Result<Vec<u8>> {
    // 上限不是性能优化而是防御：尺寸来自配置，写错一位（10240）就会让一次生成
    // 占掉几百 MB 内存。真实 API 也不接受这种尺寸，这里提前如实拒绝。
    const MAX_EDGE: u32 = 4096;
    if width > MAX_EDGE || height > MAX_EDGE {
        return Err(Adm4Error::invalid_input(format!(
            "占位图尺寸 {width}x{height} 超过 {MAX_EDGE} 上限：请检查图像通道的 size 配置"
        )));
    }
    let digest = sha256_hex(prompt.as_bytes());
    let seed = digest.as_bytes();
    let channel = |offset: usize| -> u8 {
        // 摘要是 ASCII 十六进制，取三段各自求和 → 稳定但随提示词发散的三通道。
        let mut value: u32 = 0;
        for index in 0..8 {
            value = value
                .wrapping_mul(31)
                .wrapping_add(u32::from(seed[(offset * 8 + index) % seed.len()]));
        }
        // 压到 96..=239：避免过暗（看不出图案）与过亮（看不出边框）。
        96 + (value % 144) as u8
    };
    let (red, green, blue) = (channel(0), channel(1), channel(2));
    let border = 4u32.min(width.min(height) / 8);

    let stride = width as usize * 3 + 1;
    let mut raw = Vec::with_capacity(stride * height as usize);
    for y in 0..height {
        raw.push(0); // 每行的 filter 类型：0 = None
        for x in 0..width {
            let on_border = border > 0
                && (x < border || y < border || x + border >= width || y + border >= height);
            let dark = ((x / 16) + (y / 16)) % 2 == 1;
            let (r, g, b) = if on_border {
                (255 - red, 255 - green, 255 - blue)
            } else if dark {
                (red / 2, green / 2, blue / 2)
            } else {
                (red, green, blue)
            };
            raw.extend_from_slice(&[r, g, b]);
        }
    }

    let mut png = Vec::new();
    png.extend_from_slice(&[0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A]);
    let mut header = Vec::with_capacity(13);
    header.extend_from_slice(&width.to_be_bytes());
    header.extend_from_slice(&height.to_be_bytes());
    header.extend_from_slice(&[8, 2, 0, 0, 0]); // 位深 8 / 真彩 / 无压缩变体 / 无滤波变体 / 非隔行
    push_png_chunk(&mut png, b"IHDR", &header);
    push_png_chunk(&mut png, b"IDAT", &zlib_stored(&raw));
    push_png_chunk(&mut png, b"IEND", &[]);
    Ok(png)
}

/// 写一个 PNG chunk：长度（大端）+ 类型 + 数据 + CRC32（类型与数据一起算）。
fn push_png_chunk(output: &mut Vec<u8>, kind: &[u8; 4], data: &[u8]) {
    output.extend_from_slice(&(data.len() as u32).to_be_bytes());
    output.extend_from_slice(kind);
    output.extend_from_slice(data);
    let mut crc_input = Vec::with_capacity(4 + data.len());
    crc_input.extend_from_slice(kind);
    crc_input.extend_from_slice(data);
    output.extend_from_slice(&crc32(&crc_input).to_be_bytes());
}

/// zlib 流（存储型 deflate）：无需压缩算法即可产出合法 PNG。
fn zlib_stored(raw: &[u8]) -> Vec<u8> {
    let mut stream = vec![0x78, 0x01]; // CMF/FLG：deflate 32K 窗口，(0x7801 % 31 == 0)
    if raw.is_empty() {
        stream.extend_from_slice(&[0x01, 0x00, 0x00, 0xFF, 0xFF]);
    } else {
        let mut offset = 0usize;
        while offset < raw.len() {
            let length = (raw.len() - offset).min(0xFFFF);
            let final_block = offset + length >= raw.len();
            stream.push(u8::from(final_block)); // BFINAL + BTYPE=00（存储）
            stream.extend_from_slice(&(length as u16).to_le_bytes());
            stream.extend_from_slice(&(!(length as u16)).to_le_bytes());
            stream.extend_from_slice(&raw[offset..offset + length]);
            offset += length;
        }
    }
    stream.extend_from_slice(&adler32(raw).to_be_bytes());
    stream
}

fn crc32(bytes: &[u8]) -> u32 {
    let mut crc = 0xFFFF_FFFFu32;
    for byte in bytes {
        crc ^= u32::from(*byte);
        for _ in 0..8 {
            crc = if crc & 1 == 1 {
                (crc >> 1) ^ 0xEDB8_8320
            } else {
                crc >> 1
            };
        }
    }
    !crc
}

fn adler32(bytes: &[u8]) -> u32 {
    let mut low = 1u32;
    let mut high = 0u32;
    for byte in bytes {
        low = (low + u32::from(*byte)) % 65521;
        high = (high + low) % 65521;
    }
    (high << 16) | low
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request(prompt: &str, width: u32, height: u32) -> ImageRequest {
        ImageRequest {
            purpose: "style_anchor_preview".into(),
            prompt: prompt.into(),
            width,
            height,
        }
    }

    /// 确定性：同输入逐字节相同；不同提示词的字节必须不同（否则四张方向卡长一个样）。
    #[test]
    fn scripted_image_is_deterministic_and_prompt_sensitive() {
        let provider = ScriptedImageProvider::new();
        let first = provider
            .generate(&request("清晰量产", 48, 32))
            .expect("生成");
        let again = provider
            .generate(&request("清晰量产", 48, 32))
            .expect("生成");
        let other = provider
            .generate(&request("电影写实", 48, 32))
            .expect("生成");
        assert_eq!(first.bytes, again.bytes);
        assert_ne!(first.bytes, other.bytes);
        assert_eq!(first.media_type, "image/png");
        assert_eq!(first.provider_id, "scripted_image");
        assert_eq!(provider.id(), "scripted_image");
        assert_eq!(provider.capabilities(), &[AiCapability::Image]);
        // 尺寸不同也必须不同（尺寸进了 IHDR）。
        let smaller = provider
            .generate(&request("清晰量产", 24, 32))
            .expect("生成");
        assert_ne!(first.bytes, smaller.bytes);
    }

    /// 产出必须是结构合法的 PNG：签名 + IHDR 里的宽高 + IEND 收尾。
    #[test]
    fn scripted_image_bytes_are_a_structurally_valid_png() {
        let provider = ScriptedImageProvider::new();
        let artifact = provider
            .generate(&request("高对比街机", 70, 40))
            .expect("生成");
        let bytes = &artifact.bytes;
        assert_eq!(
            &bytes[..8],
            &[0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A]
        );
        assert_eq!(&bytes[12..16], b"IHDR");
        assert_eq!(
            u32::from_be_bytes([bytes[16], bytes[17], bytes[18], bytes[19]]),
            70
        );
        assert_eq!(
            u32::from_be_bytes([bytes[20], bytes[21], bytes[22], bytes[23]]),
            40
        );
        assert_eq!(&bytes[bytes.len() - 8..bytes.len() - 4], b"IEND");
        // 嗅探器（真实通道用的同一份）也必须认得它。
        assert_eq!(
            crate::image::sniff_image_media_type(bytes).expect("应认作 PNG"),
            "image/png"
        );
        // 每个 chunk 的 CRC 逐个复核（编码器自身的回归钉子）。
        let mut cursor = 8usize;
        let mut seen_end = false;
        while cursor + 12 <= bytes.len() {
            let length = u32::from_be_bytes([
                bytes[cursor],
                bytes[cursor + 1],
                bytes[cursor + 2],
                bytes[cursor + 3],
            ]) as usize;
            let body_start = cursor + 4;
            let body_end = body_start + 4 + length;
            let expected = u32::from_be_bytes([
                bytes[body_end],
                bytes[body_end + 1],
                bytes[body_end + 2],
                bytes[body_end + 3],
            ]);
            assert_eq!(crc32(&bytes[body_start..body_end]), expected);
            if &bytes[body_start..body_start + 4] == b"IEND" {
                seen_end = true;
            }
            cursor = body_end + 4;
        }
        assert!(seen_end, "必须以 IEND chunk 收尾");
        assert_eq!(cursor, bytes.len(), "chunk 边界必须正好覆盖整份字节");
    }

    /// 多块存储型 deflate：单块上限 65535，大图必须切多块且仍然合法。
    #[test]
    fn multi_block_stored_deflate_stays_valid() {
        // 每行 200*3+1 = 601 字节，200 行 = 120200 字节 → 必然切成 2 块。
        let bytes = placeholder_png(200, 200, "多块检查").expect("生成");
        assert_eq!(
            crate::image::sniff_image_media_type(&bytes).expect("PNG"),
            "image/png"
        );
        let raw_len = 200usize * 3 * 200 + 200;
        assert!(raw_len > 0xFFFF, "本例必须触发多块路径");
        // adler32 的参考值（zlib 规范定义）：空输入为 1。
        assert_eq!(adler32(&[]), 1);
        assert_eq!(crc32(b"IEND"), 0xAE42_6082);
    }

    #[test]
    fn scripted_image_records_calls_and_propagates_failure_verbatim() {
        let provider = ScriptedImageProvider::new();
        provider
            .generate(&request("清晰量产", 16, 16))
            .expect("生成");
        provider
            .generate(&request("概念绘画", 16, 16))
            .expect("生成");
        let calls = provider.calls();
        assert_eq!(calls.len(), 2);
        assert_eq!(calls[0].prompt, "清晰量产");
        assert_eq!(calls[1].prompt, "概念绘画");

        // R7：失败原因原样上抛，不产占位图冒充成功。
        provider.fail_with("图像 API 返回 429：额度耗尽");
        let error = provider
            .generate(&request("清晰量产", 16, 16))
            .expect_err("应失败");
        assert_eq!(error.kind, adm4_foundation::Adm4ErrorKind::AiUnavailable);
        assert!(error.message.contains("额度耗尽"), "{}", error.message);
    }

    /// 入参非法在两个实现里是同一份判定（与真实 Provider 共用 `ImageRequest::validate`）。
    #[test]
    fn scripted_image_rejects_invalid_requests() {
        let provider = ScriptedImageProvider::new();
        assert!(provider.generate(&request("   ", 16, 16)).is_err());
        assert!(provider.generate(&request("清晰量产", 0, 16)).is_err());
        // 超大尺寸如实拒绝（防止配置写错一位就吃掉几百 MB）。
        assert!(placeholder_png(8192, 8192, "过大").is_err());
    }
}
