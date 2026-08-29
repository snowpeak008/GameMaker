#![forbid(unsafe_code)]

pub mod error;
pub mod fs;
pub mod hash;
pub mod ids;
pub mod path;
pub mod time;

pub use error::{AdmError, AdmErrorKind, AdmResult};
pub use fs::{atomic_write, read_to_string, write_string};
pub use hash::ContentHash;
pub use ids::{ArchiveId, ArtifactId, ProjectId, ProviderId, RunId, SessionId, StageId, TaskId};
pub use path::{SafePath, ensure_within_root};
pub use time::UtcTimestamp;
