//! V4 决策模型：L0-L6 决策点、选项、参数表结构、Selection、适用性、DAG 校验、完成度。

mod applicability;
mod completeness;
mod consistency;
mod graph;
mod types;

pub use applicability::{ApplicabilityMap, PointApplicability, compute_applicability};
pub use completeness::{
    CompletenessReport, MissingItem, compute_completeness, enumerate_axis, validate_parameters,
};
pub use consistency::{
    RowReference, RowReferenceViolation, check_row_references, row_reference_problems_by_decision,
};
pub use graph::{DecisionGraph, GraphViolation, validate_graph};
pub use types::{
    AxisRef, DecisionId, DecisionOption, DecisionPoint, DepthProfile, DesignLevel, DomainId,
    GenrePackId, GenreScope, MatrixSchema, MdaLayer, NaJustification, OptionId, OptionSelector,
    ParamPath, ParameterSchema, ParameterValues, PointRequirement, Provenance, ScalarField,
    Selection, TableSchema,
};
