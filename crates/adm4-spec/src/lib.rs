//! V4 GameSpec（schema 4.0.0）：C0 由 FrozenDesign 确定性编译产生，
//! 是流水线下游一切派生的唯一输入。

mod model;
mod validate;

pub use model::{
    AcceptanceScenario, ConditionSpec, ContentSpec, EffectSpec, EntitySpec, GameSpec, MechanicSpec,
    ProjectIntent, PropertySpec, SPEC_SCHEMA_VERSION, SpecIdentity, SpecSourceEntry,
    StateMachineSpec, StateTransition, SystemSpec, TableSpec, VisualForm,
};
pub use validate::{SpecViolation, spec_content_hash, validate_game_spec};
