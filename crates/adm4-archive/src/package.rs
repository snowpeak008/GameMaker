use adm4_foundation::{Adm4Error, Adm4Result, ensure_dir, ensure_within_root, sha256_hex};
use std::fs;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

pub const PACKAGE_MAGIC: &[u8; 12] = b"ADM4PROJ_V1\0";

/// 导出包（.adm4proj）：magic + 文件数 + 逐文件（路径长/路径/哈希长/哈希/内容长/内容）。
pub fn export_package(content_dir: &Path, output: &Path) -> Adm4Result<usize> {
    let mut files = Vec::new();
    collect(content_dir, content_dir, &mut files)?;
    files.sort();
    let mut writer = fs::File::create(output)
        .map_err(|error| Adm4Error::io(format!("create package failed: {error}")))?;
    writer
        .write_all(PACKAGE_MAGIC)
        .map_err(|error| Adm4Error::io(format!("write magic failed: {error}")))?;
    write_u64(&mut writer, files.len() as u64)?;
    for relative in &files {
        let bytes = fs::read(content_dir.join(relative))
            .map_err(|error| Adm4Error::io(format!("read {relative} failed: {error}")))?;
        let hash = sha256_hex(&bytes);
        write_chunk(&mut writer, relative.as_bytes())?;
        write_chunk(&mut writer, hash.as_bytes())?;
        write_chunk(&mut writer, &bytes)?;
    }
    Ok(files.len())
}

/// 导入包：逐文件校验哈希后落盘；任何校验失败即中止（不产生半成品树）。
pub fn import_package(package: &Path, target_content_dir: &Path) -> Adm4Result<usize> {
    let mut reader = fs::File::open(package)
        .map_err(|error| Adm4Error::io(format!("open package failed: {error}")))?;
    let mut magic = [0u8; 12];
    reader
        .read_exact(&mut magic)
        .map_err(|error| Adm4Error::validation(format!("read magic failed: {error}")))?;
    if &magic != PACKAGE_MAGIC {
        return Err(Adm4Error::validation(
            "不是合法的 .adm4proj 包（magic 不符）",
        ));
    }
    let count = read_u64(&mut reader)? as usize;
    let mut staged: Vec<(PathBuf, Vec<u8>)> = Vec::new();
    for _ in 0..count {
        let path_bytes = read_chunk(&mut reader)?;
        let hash_bytes = read_chunk(&mut reader)?;
        let content = read_chunk(&mut reader)?;
        let relative =
            String::from_utf8(path_bytes).map_err(|_| Adm4Error::validation("包内路径非 UTF-8"))?;
        let declared_hash =
            String::from_utf8(hash_bytes).map_err(|_| Adm4Error::validation("包内哈希非 UTF-8"))?;
        let actual_hash = sha256_hex(&content);
        if declared_hash != actual_hash {
            return Err(Adm4Error::validation(format!(
                "文件 {relative} 哈希不符：声明 {declared_hash}，实际 {actual_hash}"
            )));
        }
        let safe = ensure_within_root(Path::new(&relative))?;
        staged.push((safe, content));
    }
    for (relative, content) in &staged {
        let destination = target_content_dir.join(relative);
        if let Some(parent) = destination.parent() {
            ensure_dir(parent)?;
        }
        fs::write(&destination, content).map_err(|error| {
            Adm4Error::io(format!("write {} failed: {error}", destination.display()))
        })?;
    }
    Ok(staged.len())
}

fn collect(root: &Path, current: &Path, out: &mut Vec<String>) -> Adm4Result<()> {
    let entries = fs::read_dir(current)
        .map_err(|error| Adm4Error::io(format!("read dir failed: {error}")))?;
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect(root, &path, out)?;
        } else if let Ok(relative) = path.strip_prefix(root) {
            out.push(relative.to_string_lossy().replace('\\', "/"));
        }
    }
    Ok(())
}

fn write_u64(writer: &mut impl Write, value: u64) -> Adm4Result<()> {
    writer
        .write_all(&value.to_le_bytes())
        .map_err(|error| Adm4Error::io(format!("write u64 failed: {error}")))
}

fn read_u64(reader: &mut impl Read) -> Adm4Result<u64> {
    let mut buffer = [0u8; 8];
    reader
        .read_exact(&mut buffer)
        .map_err(|error| Adm4Error::validation(format!("read u64 failed: {error}")))?;
    Ok(u64::from_le_bytes(buffer))
}

fn write_chunk(writer: &mut impl Write, bytes: &[u8]) -> Adm4Result<()> {
    write_u64(writer, bytes.len() as u64)?;
    writer
        .write_all(bytes)
        .map_err(|error| Adm4Error::io(format!("write chunk failed: {error}")))
}

fn read_chunk(reader: &mut impl Read) -> Adm4Result<Vec<u8>> {
    let length = read_u64(reader)? as usize;
    if length > 512 * 1024 * 1024 {
        return Err(Adm4Error::validation("包内块超过 512MB 上限"));
    }
    let mut buffer = vec![0u8; length];
    reader
        .read_exact(&mut buffer)
        .map_err(|error| Adm4Error::validation(format!("read chunk failed: {error}")))?;
    Ok(buffer)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip_export_import() {
        let temp = std::env::temp_dir().join(format!("adm4_pkg_test_{}", std::process::id()));
        let source = temp.join("source");
        let target = temp.join("target");
        ensure_dir(&source.join("sub")).unwrap();
        fs::write(source.join("a.json"), b"{\"x\":1}").unwrap();
        fs::write(source.join("sub").join("b.txt"), b"hello").unwrap();
        let package = temp.join("test.adm4proj");
        let exported = export_package(&source, &package).unwrap();
        assert_eq!(exported, 2);
        let imported = import_package(&package, &target).unwrap();
        assert_eq!(imported, 2);
        assert_eq!(
            fs::read(target.join("sub").join("b.txt")).unwrap(),
            b"hello"
        );
        fs::remove_dir_all(&temp).ok();
    }
}
