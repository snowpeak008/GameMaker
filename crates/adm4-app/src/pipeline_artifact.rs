//! 阶段产物只读查询：读 `content/pipeline/v{N}/{C0..C6}/{document.md,contract.json}`。
//!
//! 归宿定位（`docs/design/06` §4）：流水线视图「每卡状态/耗时/产物入口」里的**产物入口**。
//! 视图此前只能显示 `summary/status`，看不到该段究竟产出了什么。
//!
//! 缺文件的口径与 `deliverable.rs` 完全一致（present / complete / missing 三件套）：
//! 文件不在就如实标 `present=false` 并进 `missing`，**不**用空串或默认值兜底（R2）。
//! `document_text` 缺失时是 `None` 而不是 `Some("")`——空串会与「文件存在但内容为空」混同。

use adm4_foundation::{Adm4Error, Adm4Result, sha256_hex};
use adm4_pipeline::design_compile_registry;
use serde::{Deserialize, Serialize};
use std::path::Path;

/// 机器契约文件名（真相源）。
pub const CONTRACT_FILE: &str = "contract.json";
/// 渲染文档文件名（由契约渲染，不可手改）。
pub const DOCUMENT_FILE: &str = "document.md";

/// `document.md` 的预览上限（字节）。
///
/// 大文件策略：超过上限只回传前 256 KiB 的文本并置 `document_truncated=true`，
/// 但 `bytes` 与 `sha256` 恒为**整份文件**的真值——UI 既能低成本预览，
/// 又能看出「你看到的不是全文」，不会拿截断内容当完整产物去核对哈希。
pub const DOCUMENT_PREVIEW_LIMIT_BYTES: usize = 256 * 1024;

/// 一份产物文件的清点结果。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ArtifactFileView {
    /// 文件名（`document.md` / `contract.json`）。
    pub file_name: String,
    pub present: bool,
    /// 文件的绝对路径。缺失时同样给出**预期路径**（供用户去磁盘上排查），
    /// 这不是兜底：`present=false` 已经明说文件不在。
    pub path: String,
    /// 文件内容 sha256（缺失时空串）。
    pub sha256: String,
    /// 文件字节数（缺失时 0）。
    pub bytes: u64,
}

/// 一个流水线阶段的产物视图：双格式产物的存在性/路径/摘要 + 渲染文档预览。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StageArtifactView {
    pub archive_id: String,
    pub frozen_version: u32,
    pub stage_id: String,
    /// 两份产物都在才算齐备（口径同 `DeliverableSegment::present`）。
    pub complete: bool,
    /// 缺失的产物文件名（非空即缺产物，UI 必须照实提示，不许显示成空白详情）。
    pub missing: Vec<String>,
    pub document: ArtifactFileView,
    pub contract: ArtifactFileView,
    /// `document.md` 的文本内容；文件缺失 = `None`。
    pub document_text: Option<String>,
    /// 预览是否被截断（`true` 时 `document_text` 只是前 `preview_limit_bytes` 字节）。
    pub document_truncated: bool,
    pub preview_limit_bytes: u64,
}

impl StageArtifactView {
    /// 清点某冻结版本下单个阶段的产物。
    ///
    /// `pipeline_version_dir` 指向 `content/pipeline/v{N}`。阶段 id 必须是 C0-C6 之一
    /// （以 `design_compile_registry()` 为唯一真相源）：未知 id 直接 `NotFound`，
    /// 而不是"报告一个全缺的空视图"——那会把打错的阶段名伪装成"该段没跑"。
    pub fn build(
        pipeline_version_dir: &Path,
        archive_id: &str,
        frozen_version: u32,
        stage_id: &str,
    ) -> Adm4Result<Self> {
        let stage_id = require_design_stage(stage_id)?;
        let stage_dir = pipeline_version_dir.join(&stage_id);

        let (document, document_bytes) =
            read_artifact(&stage_dir.join(DOCUMENT_FILE), DOCUMENT_FILE)?;
        let (contract, _) = read_artifact(&stage_dir.join(CONTRACT_FILE), CONTRACT_FILE)?;

        let mut missing = Vec::new();
        if !document.present {
            missing.push(DOCUMENT_FILE.to_string());
        }
        if !contract.present {
            missing.push(CONTRACT_FILE.to_string());
        }

        let (document_text, document_truncated) = match document_bytes {
            Some(bytes) => {
                // R2：产物不是合法 UTF-8 就报错，不做 lossy 转换——静默替换字符会让
                // 用户以为文档正常，而实际内容已被外部改坏。
                let text = String::from_utf8(bytes).map_err(|error| {
                    Adm4Error::validation(format!(
                        "阶段 {stage_id} 的 {DOCUMENT_FILE} 不是合法 UTF-8 文本（产物疑似被外部改写）：{error}"
                    ))
                })?;
                let (preview, truncated) =
                    truncate_on_char_boundary(&text, DOCUMENT_PREVIEW_LIMIT_BYTES);
                (Some(preview), truncated)
            }
            None => (None, false),
        };

        Ok(Self {
            archive_id: archive_id.to_string(),
            frozen_version,
            stage_id,
            complete: missing.is_empty(),
            missing,
            document,
            contract,
            document_text,
            document_truncated,
            preview_limit_bytes: DOCUMENT_PREVIEW_LIMIT_BYTES as u64,
        })
    }
}

/// 阶段 id 白名单校验（C0-C6，来自流水线 registry）。
fn require_design_stage(stage_id: &str) -> Adm4Result<String> {
    let wanted = stage_id.trim();
    let registry = design_compile_registry();
    if registry.iter().any(|stage| stage.id == wanted) {
        return Ok(wanted.to_string());
    }
    let known: Vec<String> = registry.into_iter().map(|stage| stage.id).collect();
    Err(Adm4Error::not_found(format!(
        "未知流水线阶段「{stage_id}」：可查询的阶段为 {}",
        known.join(" / ")
    )))
}

/// 读一份产物：返回清点结果 + 文件字节（缺失时 `None`，不视为错误）。
fn read_artifact(path: &Path, file_name: &str) -> Adm4Result<(ArtifactFileView, Option<Vec<u8>>)> {
    let display_path = path.display().to_string();
    if !path.is_file() {
        return Ok((
            ArtifactFileView {
                file_name: file_name.to_string(),
                present: false,
                path: display_path,
                sha256: String::new(),
                bytes: 0,
            },
            None,
        ));
    }
    let bytes = std::fs::read(path)
        .map_err(|error| Adm4Error::io(format!("读取 {display_path} 失败：{error}")))?;
    Ok((
        ArtifactFileView {
            file_name: file_name.to_string(),
            present: true,
            path: display_path,
            sha256: sha256_hex(&bytes),
            bytes: bytes.len() as u64,
        },
        Some(bytes),
    ))
}

/// 按字符边界截断到不超过 `limit` 字节；返回（预览文本，是否截断）。
fn truncate_on_char_boundary(text: &str, limit: usize) -> (String, bool) {
    if text.len() <= limit {
        return (text.to_string(), false);
    }
    let mut end = limit;
    while end > 0 && !text.is_char_boundary(end) {
        end -= 1;
    }
    (text[..end].to_string(), true)
}

#[cfg(test)]
mod tests {
    use super::*;
    use adm4_foundation::Adm4ErrorKind;
    use std::path::PathBuf;

    fn scratch(tag: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("adm4_stage_artifact_{}_{tag}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        dir
    }

    fn write_stage(version_dir: &Path, stage: &str, document: &str, contract: &str) {
        let dir = version_dir.join(stage);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join(DOCUMENT_FILE), document).unwrap();
        std::fs::write(dir.join(CONTRACT_FILE), contract).unwrap();
    }

    #[test]
    fn present_stage_reports_paths_digests_and_document_text() {
        let root = scratch("present");
        let vdir = root.join("pipeline").join("v1");
        write_stage(&vdir, "C2", "# C2 玩法设计文档\n正文一行", r#"{"ok":true}"#);

        let view = StageArtifactView::build(&vdir, "arc-1", 1, "C2").unwrap();
        assert_eq!(view.stage_id, "C2");
        assert_eq!(view.frozen_version, 1);
        assert!(view.complete);
        assert!(view.missing.is_empty());
        assert!(view.document.present && view.contract.present);
        assert_eq!(view.document.file_name, DOCUMENT_FILE);
        assert_eq!(view.contract.file_name, CONTRACT_FILE);
        assert!(
            view.document.path.ends_with(DOCUMENT_FILE),
            "{}",
            view.document.path
        );
        assert_eq!(
            view.document.bytes as usize,
            "# C2 玩法设计文档\n正文一行".len()
        );
        assert!(!view.document.sha256.is_empty() && !view.contract.sha256.is_empty());
        assert_eq!(
            view.document_text.as_deref(),
            Some("# C2 玩法设计文档\n正文一行")
        );
        assert!(!view.document_truncated);
        assert_eq!(
            view.preview_limit_bytes,
            DOCUMENT_PREVIEW_LIMIT_BYTES as u64
        );

        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn missing_files_are_reported_not_defaulted() {
        let root = scratch("missing");
        let vdir = root.join("pipeline").join("v1"); // 流水线从未跑

        let view = StageArtifactView::build(&vdir, "arc-2", 1, "C6").unwrap();
        assert!(!view.complete);
        assert_eq!(view.missing, vec![DOCUMENT_FILE, CONTRACT_FILE]);
        assert!(!view.document.present && !view.contract.present);
        // 缺文件必须是「空摘要 + None 文本」，绝不是空串正文冒充「文档为空」。
        assert!(view.document.sha256.is_empty());
        assert_eq!(view.document.bytes, 0);
        assert_eq!(view.document_text, None);
        assert!(!view.document_truncated);
        // 预期路径照实给出，供用户去磁盘核对。
        assert!(view.document.path.contains("C6"), "{}", view.document.path);
    }

    #[test]
    fn half_written_stage_is_incomplete_but_still_previews_what_exists() {
        let root = scratch("half");
        let vdir = root.join("pipeline").join("v3");
        let dir = vdir.join("C4");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join(DOCUMENT_FILE), "# C4\n").unwrap();

        let view = StageArtifactView::build(&vdir, "arc-3", 3, "C4").unwrap();
        assert!(!view.complete, "缺 contract.json 不算齐备");
        assert_eq!(view.missing, vec![CONTRACT_FILE]);
        assert!(view.document.present);
        assert_eq!(view.document_text.as_deref(), Some("# C4\n"));

        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn oversized_document_is_truncated_on_char_boundary_and_flagged() {
        let root = scratch("big");
        let vdir = root.join("pipeline").join("v1");
        // 多字节字符铺满，确保截断点落在字符中间，验证不会切出非法 UTF-8。
        let huge = "设".repeat(DOCUMENT_PREVIEW_LIMIT_BYTES);
        write_stage(&vdir, "C3", &huge, "{}");

        let view = StageArtifactView::build(&vdir, "arc-4", 1, "C3").unwrap();
        assert!(view.complete);
        assert!(view.document_truncated, "超限文档必须显式标记截断");
        let preview = view.document_text.expect("预览文本");
        assert!(preview.len() <= DOCUMENT_PREVIEW_LIMIT_BYTES);
        assert!(preview.starts_with('设'), "截断不得破坏首字符");
        assert!(
            preview.len() > DOCUMENT_PREVIEW_LIMIT_BYTES - 4,
            "应尽量取满上限"
        );
        // 摘要与字节数是整份文件的真值，不是预览的。
        assert_eq!(view.document.bytes as usize, huge.len());
        assert_eq!(view.document.sha256, sha256_hex(huge.as_bytes()));

        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn non_utf8_document_is_an_error_not_a_lossy_preview() {
        let root = scratch("badenc");
        let vdir = root.join("pipeline").join("v1");
        let dir = vdir.join("C1");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join(DOCUMENT_FILE), [0xff, 0xfe, 0x00]).unwrap();
        std::fs::write(dir.join(CONTRACT_FILE), "{}").unwrap();

        let error = StageArtifactView::build(&vdir, "arc-5", 1, "C1").unwrap_err();
        assert_eq!(error.kind, Adm4ErrorKind::Validation);
        assert!(error.message.contains("UTF-8"), "{}", error.message);

        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn unknown_stage_id_is_rejected_instead_of_reported_as_missing() {
        let root = scratch("unknown");
        let vdir = root.join("pipeline").join("v1");
        for bad in ["C7", "P0", "c2", "../C0", ""] {
            let error = StageArtifactView::build(&vdir, "arc-6", 1, bad)
                .expect_err("未知阶段 id 必须报错，不能伪装成「该段没跑」");
            assert_eq!(error.kind, Adm4ErrorKind::NotFound, "阶段 id {bad}");
        }
        // 合法 id 允许两侧空白（CLI 参数常带空格）。
        assert_eq!(
            StageArtifactView::build(&vdir, "arc-6", 1, " C0 ")
                .unwrap()
                .stage_id,
            "C0"
        );
    }
}
