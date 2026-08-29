//! V4 基础库：错误、ID、内容哈希、规范化 JSON、原子写、安全路径、时间戳。

mod error;
mod fs;
mod hash;
mod ids;
mod time;

pub use error::{Adm4Error, Adm4ErrorKind, Adm4Result};
pub use fs::{atomic_write, ensure_dir, ensure_within_root, read_json_file, write_json_file};
pub use hash::{ContentHash, canonical_json, sha256_hex};
pub use ids::{SessionId, new_id};
pub use time::UtcTimestamp;
