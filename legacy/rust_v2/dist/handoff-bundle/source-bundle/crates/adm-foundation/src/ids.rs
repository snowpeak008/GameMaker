use crate::{AdmError, AdmResult};
use std::fmt::{Debug, Display, Formatter};
use std::str::FromStr;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

static ID_COUNTER: AtomicU64 = AtomicU64::new(1);

fn generated_token(prefix: &str) -> String {
    let millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis())
        .unwrap_or_default();
    let pid = std::process::id();
    let counter = ID_COUNTER.fetch_add(1, Ordering::Relaxed);
    format!("{prefix}_{millis}_{pid}_{counter}")
}

macro_rules! id_type {
    ($name:ident, $prefix:literal) => {
        #[derive(Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
        pub struct $name(String);

        impl $name {
            pub fn new(value: impl Into<String>) -> AdmResult<Self> {
                let value = value.into();
                if value.trim().is_empty() {
                    return Err(AdmError::invalid_input(concat!(
                        stringify!($name),
                        " cannot be empty"
                    )));
                }
                if value.contains(std::path::MAIN_SEPARATOR) || value.contains('/') {
                    return Err(AdmError::invalid_input(concat!(
                        stringify!($name),
                        " cannot contain path separators"
                    )));
                }
                Ok(Self(value))
            }

            pub fn generate() -> Self {
                Self(generated_token($prefix))
            }

            pub fn as_str(&self) -> &str {
                &self.0
            }

            pub fn into_string(self) -> String {
                self.0
            }
        }

        impl Debug for $name {
            fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
                f.debug_tuple(stringify!($name)).field(&self.0).finish()
            }
        }

        impl Display for $name {
            fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
                f.write_str(&self.0)
            }
        }

        impl FromStr for $name {
            type Err = AdmError;

            fn from_str(value: &str) -> Result<Self, Self::Err> {
                Self::new(value)
            }
        }
    };
}

id_type!(ProjectId, "project");
id_type!(ArchiveId, "archive");
id_type!(SessionId, "session");
id_type!(RunId, "run");
id_type!(StageId, "stage");
id_type!(TaskId, "task");
id_type!(ArtifactId, "artifact");
id_type!(ProviderId, "provider");

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generated_ids_are_non_empty_and_distinct() {
        let first = ProjectId::generate();
        let second = ProjectId::generate();
        assert!(!first.as_str().is_empty());
        assert_ne!(first, second);
    }

    #[test]
    fn ids_reject_path_separators() {
        assert!(ArchiveId::new("bad/name").is_err());
    }
}
