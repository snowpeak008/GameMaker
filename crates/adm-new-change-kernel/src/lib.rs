#![forbid(unsafe_code)]

mod kernel;
mod spec_store;
mod workspace_change_set;

pub use kernel::*;
pub use spec_store::*;
pub use workspace_change_set::*;

pub const CRATE_NAME: &str = "adm-new-change-kernel";

pub fn crate_ready() -> bool {
    true
}
