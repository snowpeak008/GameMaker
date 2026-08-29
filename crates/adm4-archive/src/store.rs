use crate::data_root::DataRoot;
use crate::fingerprint::content_fingerprint;
use adm4_foundation::{
    Adm4Error, Adm4Result, UtcTimestamp, ensure_dir, new_id, read_json_file, write_json_file,
};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

pub const ARCHIVE_FORMAT_VERSION: u32 = 1;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ArchiveManifest {
    pub format_version: u32,
    pub archive_id: String,
    pub project_name: String,
    pub created_at: String,
    pub updated_at: String,
    pub content_fingerprint: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DraftMeta {
    pub session_id: String,
    pub pid: u32,
    pub linked_archive: Option<String>,
    pub updated_at: String,
}

/// 存档仓：正式存档 CRUD + 草稿工作区 + 原子提交。
pub struct ArchiveStore {
    pub data_root: DataRoot,
}

impl ArchiveStore {
    pub fn new(data_root: DataRoot) -> Self {
        Self { data_root }
    }

    pub fn list_archives(&self) -> Adm4Result<Vec<ArchiveManifest>> {
        let dir = self.data_root.archives_dir();
        let mut manifests = Vec::new();
        let entries = fs::read_dir(&dir)
            .map_err(|error| Adm4Error::io(format!("read archives dir failed: {error}")))?;
        for entry in entries.flatten() {
            let manifest_path = entry.path().join("manifest.json");
            if manifest_path.is_file() {
                manifests.push(read_json_file::<ArchiveManifest>(&manifest_path)?);
            }
        }
        manifests.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));
        Ok(manifests)
    }

    pub fn manifest(&self, archive_id: &str) -> Adm4Result<ArchiveManifest> {
        let path = self.data_root.archive_dir(archive_id).join("manifest.json");
        if !path.is_file() {
            return Err(Adm4Error::not_found(format!(
                "存档 {archive_id} 不存在（可用 project list 查看现有项目）"
            )));
        }
        read_json_file(&path)
    }

    pub fn content_dir(&self, archive_id: &str) -> PathBuf {
        self.data_root.archive_dir(archive_id).join("content")
    }

    /// 创建草稿工作区（新项目或从正式存档 checkout）。
    pub fn create_draft(
        &self,
        session_id: &str,
        linked_archive: Option<&str>,
    ) -> Adm4Result<PathBuf> {
        let draft_dir = self.data_root.draft_dir(session_id);
        ensure_dir(&draft_dir)?;
        if let Some(archive_id) = linked_archive {
            let source = self.content_dir(archive_id);
            if source.is_dir() {
                copy_tree(&source, &draft_dir.join("content"))?;
            }
        } else {
            ensure_dir(&draft_dir.join("content"))?;
        }
        let meta = DraftMeta {
            session_id: session_id.to_string(),
            pid: std::process::id(),
            linked_archive: linked_archive.map(String::from),
            updated_at: UtcTimestamp::now().to_iso8601(),
        };
        write_json_file(&draft_dir.join("draft_meta.json"), &meta)?;
        Ok(draft_dir)
    }

    pub fn draft_content_dir(&self, session_id: &str) -> PathBuf {
        self.data_root.draft_dir(session_id).join("content")
    }

    /// 原子提交：草稿 content → 临时目录 → 校验指纹 → 替换正式目录。
    /// 返回（可能新建的）archive_id。
    pub fn commit_draft(
        &self,
        session_id: &str,
        project_name: &str,
        target_archive: Option<&str>,
    ) -> Adm4Result<String> {
        let draft_content = self.draft_content_dir(session_id);
        if !draft_content.is_dir() {
            return Err(Adm4Error::not_found(format!(
                "会话 {session_id} 无草稿工作区"
            )));
        }
        let archive_id = target_archive
            .map(String::from)
            .unwrap_or_else(|| new_id("archive"));
        let archive_dir = self.data_root.archive_dir(&archive_id);
        ensure_dir(&archive_dir)?;

        // 1. 复制草稿到临时目录。
        let staging = archive_dir.join(".staging_content");
        if staging.exists() {
            fs::remove_dir_all(&staging)
                .map_err(|error| Adm4Error::io(format!("clear staging failed: {error}")))?;
        }
        copy_tree(&draft_content, &staging)?;

        // 2. 校验指纹一致（复制无损）。
        let draft_fingerprint = content_fingerprint(&draft_content)?;
        let staging_fingerprint = content_fingerprint(&staging)?;
        if draft_fingerprint != staging_fingerprint {
            fs::remove_dir_all(&staging).ok();
            return Err(Adm4Error::internal(
                "staging fingerprint mismatch, commit aborted",
            ));
        }

        // 3. 替换正式 content（备份旧目录后 rename）。
        let content_dir = archive_dir.join("content");
        let backup = archive_dir.join(".backup_content");
        if backup.exists() {
            fs::remove_dir_all(&backup)
                .map_err(|error| Adm4Error::io(format!("clear backup failed: {error}")))?;
        }
        if content_dir.exists() {
            fs::rename(&content_dir, &backup)
                .map_err(|error| Adm4Error::io(format!("backup content failed: {error}")))?;
        }
        fs::rename(&staging, &content_dir)
            .map_err(|error| Adm4Error::io(format!("swap content failed: {error}")))?;
        if backup.exists() {
            fs::remove_dir_all(&backup).ok();
        }

        // 4. 写 manifest。
        let now = UtcTimestamp::now().to_iso8601();
        let created_at = self
            .manifest(&archive_id)
            .map(|manifest| manifest.created_at)
            .unwrap_or_else(|_| now.clone());
        let manifest = ArchiveManifest {
            format_version: ARCHIVE_FORMAT_VERSION,
            archive_id: archive_id.clone(),
            project_name: project_name.to_string(),
            created_at,
            updated_at: now,
            content_fingerprint: draft_fingerprint,
        };
        write_json_file(&archive_dir.join("manifest.json"), &manifest)?;
        Ok(archive_id)
    }

    /// 追加型产物（冻结/流水线）写入 content 后刷新 manifest 指纹。
    pub fn refresh_fingerprint(&self, archive_id: &str) -> Adm4Result<()> {
        let mut manifest = self.manifest(archive_id)?;
        manifest.content_fingerprint = content_fingerprint(&self.content_dir(archive_id))?;
        manifest.updated_at = UtcTimestamp::now().to_iso8601();
        write_json_file(
            &self.data_root.archive_dir(archive_id).join("manifest.json"),
            &manifest,
        )
    }

    /// 存档体检：manifest 存在、指纹一致。
    pub fn doctor(&self, archive_id: &str) -> Adm4Result<Vec<String>> {
        let mut problems = Vec::new();
        let manifest = match self.manifest(archive_id) {
            Ok(manifest) => manifest,
            Err(error) => {
                return Ok(vec![format!("manifest 不可读：{}", error.message)]);
            }
        };
        let actual = content_fingerprint(&self.content_dir(archive_id))?;
        if actual != manifest.content_fingerprint {
            problems.push(format!(
                "内容指纹不一致：manifest={} 实际={actual}",
                manifest.content_fingerprint
            ));
        }
        Ok(problems)
    }
}

fn copy_tree(source: &Path, target: &Path) -> Adm4Result<()> {
    ensure_dir(target)?;
    let entries = fs::read_dir(source)
        .map_err(|error| Adm4Error::io(format!("read {} failed: {error}", source.display())))?;
    for entry in entries.flatten() {
        let path = entry.path();
        let destination = target.join(entry.file_name());
        if path.is_dir() {
            copy_tree(&path, &destination)?;
        } else {
            fs::copy(&path, &destination).map_err(|error| {
                Adm4Error::io(format!("copy {} failed: {error}", path.display()))
            })?;
        }
    }
    Ok(())
}
