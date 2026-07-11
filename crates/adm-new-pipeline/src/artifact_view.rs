use std::fs;
use std::path::Path;

use adm_new_foundation::{AdmResult, file_manifest};
use serde::{Deserialize, Serialize};

const PREVIEW_CHAR_LIMIT: usize = 4_096;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PipelineArtifactRecord {
    pub relative_path: String,
    pub name: String,
    pub size_bytes: u64,
    pub content_type: String,
    #[serde(default)]
    pub content_preview: String,
    #[serde(default)]
    pub is_binary: bool,
}

pub(crate) fn scan_stage_artifacts(
    artifact_root: &Path,
    stage_dir: &Path,
) -> AdmResult<Vec<PipelineArtifactRecord>> {
    let stage_name = stage_dir
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or_default();
    file_manifest(stage_dir)?
        .into_iter()
        .map(|entry| {
            let path = stage_dir.join(&entry.path);
            let bytes = fs::read(&path)?;
            let content_type = content_type_for_path(&path).to_string();
            let text = String::from_utf8(bytes).ok();
            let content_preview = text
                .as_deref()
                .filter(|_| is_text_content_type(&content_type))
                .map(|value| value.chars().take(PREVIEW_CHAR_LIMIT).collect())
                .unwrap_or_default();
            let is_binary = !is_text_content_type(&content_type) || text.is_none();
            let relative_path = path
                .strip_prefix(artifact_root)
                .map(|value| value.to_string_lossy().replace('\\', "/"))
                .unwrap_or_else(|_| format!("{stage_name}/{}", entry.path));
            Ok(PipelineArtifactRecord {
                name: path
                    .file_name()
                    .and_then(|value| value.to_str())
                    .unwrap_or_default()
                    .to_string(),
                relative_path,
                size_bytes: entry.size_bytes,
                content_type,
                content_preview,
                is_binary,
            })
        })
        .collect()
}

pub(crate) fn content_type_for_path(path: &Path) -> &'static str {
    match path
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase()
        .as_str()
    {
        "json" => "application/json",
        "md" | "markdown" => "text/markdown",
        "txt" | "log" => "text/plain",
        "html" => "text/html",
        "css" => "text/css",
        "js" => "text/javascript",
        "csv" => "text/csv",
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "svg" => "image/svg+xml",
        "wav" => "audio/wav",
        "mp3" => "audio/mpeg",
        "pdf" => "application/pdf",
        _ => "application/octet-stream",
    }
}

fn is_text_content_type(content_type: &str) -> bool {
    content_type.starts_with("text/")
        || matches!(content_type, "application/json" | "image/svg+xml")
}
