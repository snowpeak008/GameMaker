//! 文档集交付打包：把 C0-C6 流水线产物汇总成一份带 sha256 与完整性标记的交付清单。
//!
//! 归宿定位（`docs/plan/05` §2.2）：二版「打包阶段」在四版的**文档集交付**落点。
//! 本期只做设计文档集的清点与校验（读既有 `content/pipeline/v{N}/{C0..C6}/` 产物）；
//! 游戏构建 / 引擎工程 / 运行时验证仍属 Phase 2（P0-P5），保留占位不在此实现。
//! `.adm4proj` 整包导出/导入复用既有 `AppServices::export_project`/`import_project`。
//!
//! 落盘：项目内 `content/deliverable/v{N}/manifest.json`（事务外补写 + refresh_fingerprint）。
//! 缺段不静默——manifest 显式列出 `missing_segments` 且 `complete=false`（R2/R6 口径）。

use adm4_foundation::{Adm4Error, Adm4Result, sha256_hex};
use serde::{Deserialize, Serialize};
use std::path::Path;

/// 设计文档集的段（对齐流水线 C0-C6 阶段 id，顺序即交付顺序）。
const DESIGN_STAGES: [&str; 7] = ["C0", "C1", "C2", "C3", "C4", "C5", "C6"];

/// 交付清单中的一段文档产物。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DeliverableSegment {
    /// 阶段 id（C0..C6）。
    pub stage_id: String,
    /// 该段是否齐备（contract.json 与 document.md 都在）。
    pub present: bool,
    /// 渲染文档 document.md 的 sha256（缺失时空串）。
    #[serde(default)]
    pub document_sha256: String,
    /// 机器契约 contract.json 的 sha256（缺失时空串）。
    #[serde(default)]
    pub contract_sha256: String,
    /// 渲染文档字节数（缺失时 0）。
    #[serde(default)]
    pub document_bytes: u64,
}

/// 文档集交付清单：一次冻结版本下 C0-C6 全段的清点结果。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DeliverableManifest {
    pub archive_id: String,
    /// 交付对应的冻结版本。
    pub frozen_version: u32,
    /// 生成时间 ISO8601。
    pub generated_at: String,
    /// 是否完整（七段齐备）。
    pub complete: bool,
    /// 缺失的段（按 C0..C6 顺序）。
    #[serde(default)]
    pub missing_segments: Vec<String>,
    /// 逐段清点结果（恒含 7 段，缺段 present=false）。
    pub segments: Vec<DeliverableSegment>,
}

impl DeliverableManifest {
    /// 清点一个冻结版本的流水线产物目录，产出交付清单。
    ///
    /// `pipeline_version_dir` 指向 `content/pipeline/v{N}`；每段读取
    /// `{stage}/document.md` 与 `{stage}/contract.json`，两者都在才算 `present`。
    /// 目录整体不存在（流水线从未跑）= 七段全缺、`complete=false`，不报错。
    pub fn build(
        pipeline_version_dir: &Path,
        archive_id: &str,
        frozen_version: u32,
        generated_at: &str,
    ) -> Adm4Result<Self> {
        let mut segments = Vec::with_capacity(DESIGN_STAGES.len());
        let mut missing_segments = Vec::new();
        for stage_id in DESIGN_STAGES {
            let stage_dir = pipeline_version_dir.join(stage_id);
            let document = file_digest(&stage_dir.join("document.md"))?;
            let contract = file_digest(&stage_dir.join("contract.json"))?;
            let present = document.is_some() && contract.is_some();
            if !present {
                missing_segments.push(stage_id.to_string());
            }
            let (document_sha256, document_bytes) = document.unwrap_or_default();
            let contract_sha256 = contract.map(|(sha, _)| sha).unwrap_or_default();
            segments.push(DeliverableSegment {
                stage_id: stage_id.to_string(),
                present,
                document_sha256,
                contract_sha256,
                document_bytes,
            });
        }
        Ok(Self {
            archive_id: archive_id.to_string(),
            frozen_version,
            generated_at: generated_at.to_string(),
            complete: missing_segments.is_empty(),
            missing_segments,
            segments,
        })
    }
}

/// 计算文件的 sha256 与字节数；文件不存在返回 `None`（不视为错误）。
fn file_digest(path: &Path) -> Adm4Result<Option<(String, u64)>> {
    if !path.is_file() {
        return Ok(None);
    }
    let bytes = std::fs::read(path)
        .map_err(|error| Adm4Error::internal(format!("读取 {} 失败：{error}", path.display())))?;
    let len = bytes.len() as u64;
    Ok(Some((sha256_hex(&bytes), len)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    /// 唯一临时目录（进程 id + 测试名，避免同二进制内多测试相互踩踏）。
    fn scratch(tag: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("adm4_deliverable_{}_{tag}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        dir
    }

    fn write_stage(version_dir: &Path, stage: &str, doc: &str, contract: &str) {
        let dir = version_dir.join(stage);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("document.md"), doc).unwrap();
        std::fs::write(dir.join("contract.json"), contract).unwrap();
    }

    #[test]
    fn full_pipeline_yields_complete_manifest() {
        let root = scratch("full");
        let vdir = root.join("pipeline").join("v1");
        for stage in DESIGN_STAGES {
            write_stage(&vdir, stage, &format!("# {stage}\n正文"), r#"{"ok":true}"#);
        }
        let manifest =
            DeliverableManifest::build(&vdir, "arc-1", 1, "2026-08-30T00:00:00Z").unwrap();
        assert!(manifest.complete);
        assert!(manifest.missing_segments.is_empty());
        assert_eq!(manifest.segments.len(), 7);
        assert!(manifest.segments.iter().all(|s| s.present));
        assert!(
            manifest
                .segments
                .iter()
                .all(|s| !s.document_sha256.is_empty())
        );
        assert!(manifest.segments.iter().all(|s| s.document_bytes > 0));
        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn missing_stage_marks_incomplete_and_lists_gap() {
        let root = scratch("gap");
        let vdir = root.join("pipeline").join("v2");
        // 只写 C0-C4，缺 C5/C6。
        for stage in ["C0", "C1", "C2", "C3", "C4"] {
            write_stage(&vdir, stage, "doc", "{}");
        }
        let manifest =
            DeliverableManifest::build(&vdir, "arc-2", 2, "2026-08-30T00:00:00Z").unwrap();
        assert!(!manifest.complete);
        assert_eq!(manifest.missing_segments, vec!["C5", "C6"]);
        let c6 = manifest
            .segments
            .iter()
            .find(|s| s.stage_id == "C6")
            .unwrap();
        assert!(!c6.present);
        assert!(c6.document_sha256.is_empty());
        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn half_written_stage_is_not_present() {
        let root = scratch("half");
        let vdir = root.join("pipeline").join("v1");
        // C0 只有 document.md，没有 contract.json。
        let c0 = vdir.join("C0");
        std::fs::create_dir_all(&c0).unwrap();
        std::fs::write(c0.join("document.md"), "doc").unwrap();
        let manifest = DeliverableManifest::build(&vdir, "arc-3", 1, "t").unwrap();
        let seg = &manifest.segments[0];
        assert_eq!(seg.stage_id, "C0");
        assert!(!seg.present, "缺 contract.json 不算齐备");
        assert!(manifest.missing_segments.contains(&"C0".to_string()));
        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn absent_pipeline_dir_yields_all_missing() {
        let root = scratch("absent");
        let vdir = root.join("pipeline").join("v1"); // 从未创建
        let manifest = DeliverableManifest::build(&vdir, "arc-4", 1, "t").unwrap();
        assert!(!manifest.complete);
        assert_eq!(manifest.missing_segments.len(), 7);
        assert!(manifest.segments.iter().all(|s| !s.present));
    }
}
