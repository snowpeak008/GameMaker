use crate::{AdmError, AdmErrorKind, AdmResult};
use std::path::{Component, Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SafePath {
    root: PathBuf,
    absolute: PathBuf,
}

impl SafePath {
    pub fn new(root: impl AsRef<Path>, candidate: impl AsRef<Path>) -> AdmResult<Self> {
        let root = absolute_normalized(root.as_ref())?;
        let absolute = ensure_within_root(&root, candidate)?;
        Ok(Self { root, absolute })
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn absolute(&self) -> &Path {
        &self.absolute
    }

    pub fn relative(&self) -> AdmResult<&Path> {
        self.absolute.strip_prefix(&self.root).map_err(|error| {
            AdmError::new(AdmErrorKind::PathEscape, error.to_string())
                .with_context(format!("root={}", self.root.display()))
                .with_context(format!("path={}", self.absolute.display()))
        })
    }
}

pub fn ensure_within_root(
    root: impl AsRef<Path>,
    candidate: impl AsRef<Path>,
) -> AdmResult<PathBuf> {
    let root = absolute_normalized(root.as_ref())?;
    let candidate = candidate.as_ref();
    let candidate = if candidate.is_absolute() {
        normalize_lexical(candidate)
    } else {
        normalize_lexical(root.join(candidate))
    };

    if !candidate.starts_with(&root) {
        return Err(
            AdmError::new(AdmErrorKind::PathEscape, "path escapes the configured root")
                .with_context(format!("root={}", root.display()))
                .with_context(format!("path={}", candidate.display())),
        );
    }

    Ok(candidate)
}

fn absolute_normalized(path: &Path) -> AdmResult<PathBuf> {
    let absolute = if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir()?.join(path)
    };
    Ok(normalize_lexical(absolute))
}

fn normalize_lexical(path: impl AsRef<Path>) -> PathBuf {
    let mut out = PathBuf::new();
    for component in path.as_ref().components() {
        match component {
            Component::Prefix(prefix) => out.push(prefix.as_os_str()),
            Component::RootDir => out.push(component.as_os_str()),
            Component::CurDir => {}
            Component::ParentDir => {
                out.pop();
            }
            Component::Normal(part) => out.push(part),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn safe_path_rejects_parent_escape() {
        let root = std::env::temp_dir().join("adm_safe_path_root");
        let error = ensure_within_root(&root, "..").expect_err("escape must be rejected");
        assert_eq!(error.kind(), &AdmErrorKind::PathEscape);
    }

    #[test]
    fn safe_path_allows_child() {
        let root = std::env::temp_dir().join("adm_safe_path_root");
        let child = ensure_within_root(&root, "child/file.txt").expect("child path");
        assert!(child.ends_with("child/file.txt"));
    }
}
