#![forbid(unsafe_code)]

mod capability;
mod id;
mod spec;

pub use capability::*;
pub use id::{SpecId, SpecIdError};
pub use spec::*;

pub const CRATE_NAME: &str = "adm-new-game-spec";
pub const GAME_SPEC_SCHEMA_VERSION: &str = "2.0.0-alpha.1";

pub fn crate_ready() -> bool {
    true
}
