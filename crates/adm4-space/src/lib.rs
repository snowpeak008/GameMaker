//! V4 设计空间清单输入门：通用层 + 品类包的加载、校验与交叉验证。
//!
//! 决策点与选项内容由用户提供（E4 输入门），本 crate 只负责 schema、加载与校验。

mod loader;
mod model;
mod system_loader;
mod validate;

pub use loader::{
    DesignSpaceRoot, assemble_design_space, load_design_space, load_design_space_customized,
    load_design_space_with_modules, load_pack_file,
};
pub use model::{
    ConsistencyRule, ConsistencyRuleKind, DesignSpace, GenrePack, SystemInstanceInfo, SystemRef,
    UniversalLayer,
};
pub use system_loader::{
    SystemInstantiation, instantiate_system_refs, load_module_file, load_modules_from_dirs,
    semver_matches,
};
pub use validate::{SpaceViolation, validate_design_space};
