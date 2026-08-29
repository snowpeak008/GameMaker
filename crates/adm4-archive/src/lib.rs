//! V4 存档层：数据根、正式存档、存档锁、草稿工作区、原子保存、内容指纹、导出导入包。
//!
//! 语义（继承二版词汇表）：编辑永远发生在草稿工作区；显式保存动作把草稿事务性
//! 提交为正式存档；打开正式存档编辑时持锁，同档单编辑。

mod data_root;
mod fingerprint;
mod lock;
mod package;
mod store;

pub use data_root::DataRoot;
pub use fingerprint::content_fingerprint;
pub use lock::ArchiveLock;
pub use package::{PACKAGE_MAGIC, export_package, import_package};
pub use store::{ArchiveManifest, ArchiveStore, DraftMeta};
