use adm4_foundation::{Adm4Error, Adm4Result, UtcTimestamp};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
struct LockPayload {
    session_id: String,
    pid: u32,
    created_at: String,
}

/// 存档锁：同档单编辑。持有者 drop 时释放；陈旧锁（进程不存在）可强制清理。
#[derive(Debug)]
pub struct ArchiveLock {
    path: PathBuf,
    released: bool,
}

impl ArchiveLock {
    /// 获取锁；已被其它会话持有 → AlreadyLocked。
    pub fn acquire(archive_dir: &Path, session_id: &str) -> Adm4Result<Self> {
        let path = archive_dir.join(".lock");
        if path.exists() {
            let existing: Option<LockPayload> = fs::read_to_string(&path)
                .ok()
                .and_then(|text| serde_json::from_str(&text).ok());
            if let Some(payload) = existing
                && payload.session_id != session_id
            {
                return Err(Adm4Error::already_locked(format!(
                    "存档已被会话 {}（pid {}，{}）锁定",
                    payload.session_id, payload.pid, payload.created_at
                )));
            }
        }
        let payload = LockPayload {
            session_id: session_id.to_string(),
            pid: std::process::id(),
            created_at: UtcTimestamp::now().to_iso8601(),
        };
        let text = serde_json::to_string_pretty(&payload)
            .map_err(|error| Adm4Error::internal(format!("lock serialize failed: {error}")))?;
        fs::write(&path, text)
            .map_err(|error| Adm4Error::io(format!("write lock failed: {error}")))?;
        Ok(Self {
            path,
            released: false,
        })
    }

    /// 强制清理外部陈旧锁（用户显式操作）。
    pub fn force_clear(archive_dir: &Path) -> Adm4Result<()> {
        let path = archive_dir.join(".lock");
        if path.exists() {
            fs::remove_file(&path)
                .map_err(|error| Adm4Error::io(format!("clear lock failed: {error}")))?;
        }
        Ok(())
    }

    pub fn release(mut self) -> Adm4Result<()> {
        self.release_inner()
    }

    fn release_inner(&mut self) -> Adm4Result<()> {
        if !self.released {
            self.released = true;
            if self.path.exists() {
                fs::remove_file(&self.path)
                    .map_err(|error| Adm4Error::io(format!("release lock failed: {error}")))?;
            }
        }
        Ok(())
    }
}

impl Drop for ArchiveLock {
    fn drop(&mut self) {
        let _ = self.release_inner();
    }
}
