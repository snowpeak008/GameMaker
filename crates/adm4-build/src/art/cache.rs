//! 内容哈希缓存（册 08 §4.3）：同「规格 + 最终提示词 + 尺寸 + provider/model」命中则复用，
//! 不重生（图像生成花钱；缓存命中不占预算额度）。
//!
//! 键是**内容哈希**而不是资产 id：提示词或风格契约变了，键随之变化，旧图自然失效——
//! 不需要任何显式失效逻辑，也就没有「忘了失效」这种 bug。

use adm4_foundation::{Adm4Error, Adm4Result, atomic_write, sha256_hex};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// 缓存键的输入面（谁变了都必须导致重新生成）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CacheKeyInput<'a> {
    pub asset_id: &'a str,
    /// 实际使用的完整提示词（含风格前缀与用途约束）。
    pub prompt: &'a str,
    pub width: u32,
    pub height: u32,
    pub provider_id: &'a str,
    pub model: &'a str,
}

impl CacheKeyInput<'_> {
    /// 内容哈希键（规范化 JSON 的 sha256 十六进制前 32 位；碰撞概率对本用途可忽略）。
    ///
    /// 键要当文件名用：去掉 `sha256_hex` 的 `sha256:` 方案前缀（Windows 文件名禁冒号），
    /// 只留十六进制部分。
    pub fn key(&self) -> Adm4Result<String> {
        let json = serde_json::to_string(self)
            .map_err(|error| Adm4Error::internal(format!("缓存键序列化失败：{error}")))?;
        let digest = sha256_hex(json.as_bytes());
        let hex = digest.strip_prefix("sha256:").unwrap_or(&digest);
        Ok(hex.chars().take(32).collect())
    }
}

/// 缓存命中的记录（进台账：这张图是从缓存来的，不是新生成的）。
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct CachedImage {
    pub key: String,
    pub media_type: String,
    pub provider_id: String,
    pub model: String,
    /// 字节的 sha256（取用时校验：缓存文件被外部改动过就当未命中重新生成）。
    pub bytes_sha256: String,
}

/// 磁盘缓存：`<root>/<key>.bin` + `<root>/<key>.json`（元数据）。
pub struct ProductionCache {
    root: PathBuf,
}

impl ProductionCache {
    pub fn new(root: PathBuf) -> Self {
        Self { root }
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    fn bytes_path(&self, key: &str) -> PathBuf {
        self.root.join(format!("{key}.bin"))
    }

    fn meta_path(&self, key: &str) -> PathBuf {
        self.root.join(format!("{key}.json"))
    }

    /// 查缓存。命中且完好 → 字节 + 元数据；未命中 / 元数据坏 / 指纹不符 → None。
    ///
    /// 指纹不符按未命中处理而不是报错：缓存是**优化**，坏了重新生成即可；
    /// 把一份被外部改过的缓存文件当真图用才是事故。
    pub fn lookup(&self, key: &str) -> Adm4Result<Option<(Vec<u8>, CachedImage)>> {
        let bytes_path = self.bytes_path(key);
        let meta_path = self.meta_path(key);
        if !bytes_path.is_file() || !meta_path.is_file() {
            return Ok(None);
        }
        let bytes = std::fs::read(&bytes_path).map_err(|error| {
            Adm4Error::io(format!("读缓存 {} 失败：{error}", bytes_path.display()))
        })?;
        let meta: CachedImage = match adm4_foundation::read_json_file(&meta_path) {
            Ok(meta) => meta,
            Err(_) => return Ok(None),
        };
        if sha256_hex(&bytes) != meta.bytes_sha256 {
            return Ok(None);
        }
        Ok(Some((bytes, meta)))
    }

    /// 写缓存（生成成功后调用；原子写，写坏了下次按未命中走）。
    pub fn store(
        &self,
        key: &str,
        bytes: &[u8],
        media_type: &str,
        provider_id: &str,
        model: &str,
    ) -> Adm4Result<CachedImage> {
        let meta = CachedImage {
            key: key.to_string(),
            media_type: media_type.to_string(),
            provider_id: provider_id.to_string(),
            model: model.to_string(),
            bytes_sha256: sha256_hex(bytes),
        };
        atomic_write(&self.bytes_path(key), bytes)?;
        adm4_foundation::write_json_file(&self.meta_path(key), &meta)?;
        Ok(meta)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(case: &str) -> ProductionCache {
        let root = std::env::temp_dir().join(format!("adm4_cache_{case}_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).expect("建缓存目录");
        ProductionCache::new(root)
    }

    fn key_input<'a>(prompt: &'a str) -> CacheKeyInput<'a> {
        CacheKeyInput {
            asset_id: "T_Guard",
            prompt,
            width: 1024,
            height: 1024,
            provider_id: "scripted_image",
            model: "scripted",
        }
    }

    /// 键的敏感面：提示词 / 尺寸 / 模型任一变化 → 键变化；同输入 → 键稳定。
    #[test]
    fn cache_key_changes_with_any_input_change() {
        let base = key_input("风格前缀 守卫立绘").key().expect("键");
        assert_eq!(base, key_input("风格前缀 守卫立绘").key().unwrap());
        assert_ne!(base, key_input("别的提示词").key().unwrap());
        let mut wider = key_input("风格前缀 守卫立绘");
        wider.width = 512;
        assert_ne!(base, wider.key().unwrap());
        let mut other_model = key_input("风格前缀 守卫立绘");
        other_model.model = "gpt-image-1";
        assert_ne!(base, other_model.key().unwrap());
    }

    /// 存取往返 + 指纹自校验：被外部改过的缓存按未命中处理，不当真图用。
    #[test]
    fn lookup_round_trips_and_rejects_tampered_bytes() {
        let cache = scratch("roundtrip");
        let key = key_input("提示词").key().unwrap();
        assert!(cache.lookup(&key).expect("查询").is_none(), "冷缓存未命中");

        cache
            .store(
                &key,
                b"png-bytes",
                "image/png",
                "scripted_image",
                "scripted",
            )
            .expect("写缓存");
        let (bytes, meta) = cache.lookup(&key).expect("查询").expect("命中");
        assert_eq!(bytes, b"png-bytes");
        assert_eq!(meta.media_type, "image/png");

        // 篡改字节：指纹不符 → 视同未命中（重新生成），绝不把坏图当真图。
        std::fs::write(cache.bytes_path(&key), b"tampered").expect("篡改");
        assert!(cache.lookup(&key).expect("查询").is_none());

        std::fs::remove_dir_all(cache.root()).ok();
    }
}
