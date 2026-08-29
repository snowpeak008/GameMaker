use adm4_foundation::{Adm4Error, Adm4Result, sha256_hex};
use std::fs;
use std::path::Path;

/// 内容指纹：content/ 树全部文件按相对路径排序，逐文件 sha256 后连接再 sha256。
pub fn content_fingerprint(content_dir: &Path) -> Adm4Result<String> {
    let mut files = Vec::new();
    collect_files(content_dir, content_dir, &mut files)?;
    files.sort();
    let mut combined = String::new();
    for relative in &files {
        let bytes = fs::read(content_dir.join(relative)).map_err(|error| {
            Adm4Error::io(format!("read {relative} for fingerprint failed: {error}"))
        })?;
        combined.push_str(relative);
        combined.push(':');
        combined.push_str(&sha256_hex(&bytes));
        combined.push('\n');
    }
    Ok(sha256_hex(combined.as_bytes()))
}

fn collect_files(root: &Path, current: &Path, out: &mut Vec<String>) -> Adm4Result<()> {
    if !current.exists() {
        return Ok(());
    }
    let entries = fs::read_dir(current).map_err(|error| {
        Adm4Error::io(format!("read dir {} failed: {error}", current.display()))
    })?;
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_files(root, &path, out)?;
        } else if let Ok(relative) = path.strip_prefix(root) {
            out.push(relative.to_string_lossy().replace('\\', "/"));
        }
    }
    Ok(())
}
