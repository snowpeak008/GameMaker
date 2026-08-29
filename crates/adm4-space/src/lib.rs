//! V4 设计空间清单输入门：通用层 + 品类包的加载、校验与交叉验证。
//!
//! 决策点与选项内容由用户提供（E4 输入门），本 crate 只负责 schema、加载与校验。

mod loader;
mod model;
mod validate;

pub use loader::{DesignSpaceRoot, load_design_space, load_pack_file};
pub use model::{ConsistencyRule, ConsistencyRuleKind, DesignSpace, GenrePack, UniversalLayer};
pub use validate::{SpaceViolation, validate_design_space};
