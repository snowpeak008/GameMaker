#![forbid(unsafe_code)]

mod canonical;
mod capability;
mod envelope;
mod id;
mod parse;
mod spec;
mod validation;

pub use canonical::{CanonicalGameSpec, CanonicalizationError, canonicalize_game_spec};
pub use capability::*;
pub use envelope::*;
pub use id::{SpecId, SpecIdError};
pub use parse::{GameSpecParseError, parse_game_spec};
pub use spec::*;
pub use validation::{
    SpecValidationIssue, SpecValidationReport, ValidationSeverity, validate_game_spec,
    validate_game_spec_for_envelope,
};

pub const CRATE_NAME: &str = "adm-new-game-spec";
pub const GAME_SPEC_SCHEMA_VERSION: &str = "2.0.0-alpha.1";

pub fn crate_ready() -> bool {
    true
}
