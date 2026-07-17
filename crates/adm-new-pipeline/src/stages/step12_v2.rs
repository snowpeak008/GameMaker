mod image_support;
mod production;
mod types;

pub use production::{
    discover_asset_bindings_from_workspace, run_step12_asset_production,
    run_step12_asset_production_with_vlm, validate_asset_binding_graph,
};
pub use types::{
    AssetBindingReference, AssetBindingValidationReport, AssetProductionPolicy,
    ConfirmationStrategy, DeterministicHeadlessAssetLoader, EngineAssetLoader, EngineLoadProbe,
    ProducedAssetRecord, STEP12_V2_COMPILER_VERSION, Step12AssetProductionOutput,
    Step12CorrectionQueueItem, Step12Status, WorkspaceReferenceAssetLoader,
};
