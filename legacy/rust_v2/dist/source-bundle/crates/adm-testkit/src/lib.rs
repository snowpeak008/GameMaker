#![forbid(unsafe_code)]

use adm_foundation::{AdmResult, SessionId};
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug)]
pub struct TestWorkspace {
    root: PathBuf,
}

impl TestWorkspace {
    pub fn create(name: &str) -> AdmResult<Self> {
        let root = std::env::temp_dir().join(format!(
            "adm_testkit_{name}_{}",
            SessionId::generate().as_str()
        ));
        fs::create_dir_all(&root)?;
        Ok(Self { root })
    }

    pub fn root(&self) -> &Path {
        &self.root
    }
}

impl Drop for TestWorkspace {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}
