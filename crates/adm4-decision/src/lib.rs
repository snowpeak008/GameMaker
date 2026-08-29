//! V4 决策模型：L0-L6 决策点、选项、参数表结构、Selection、适用性、DAG 校验、完成度，
//! 以及领域/节点两级内容组织（横向维度）与其进度聚合。

mod applicability;
mod completeness;
mod consistency;
mod graph;
mod organization;
mod types;

pub use applicability::{ApplicabilityMap, PointApplicability, compute_applicability};
pub use completeness::{
    CompletenessReport, MissingItem, compute_completeness, counts_toward_completeness,
    enumerate_axis, optional_unanswered, validate_option_parameters, validate_parameters,
    validate_selection_mode,
};
pub use consistency::{
    RowReference, RowReferenceViolation, check_row_references, row_reference_problems_by_decision,
};
pub use graph::{DecisionGraph, GraphViolation, validate_graph};
pub use organization::{
    DesignDomain, DesignNode, DesignOrganization, DomainProgress, NodeProgress,
    OrganizationProgress, OrganizationViolation, ProgressCounts, UNASSIGNED_DOMAIN_ID,
    UNASSIGNED_DOMAIN_NAME, UNASSIGNED_NODE_ID, UNASSIGNED_NODE_NAME, aggregate_progress,
    validate_organization,
};
pub use types::{
    AxisRef, DecisionId, DecisionOption, DecisionPoint, DepthProfile, DesignLevel, DomainId,
    GenrePackId, GenreScope, MatrixSchema, MdaLayer, NaJustification, NodeId, OptionId,
    OptionSelector, ParamPath, ParameterSchema, ParameterValues, PointRequirement, Provenance,
    ScalarField, SelectedOption, SelectedOptionRef, Selection, SelectionMode, TableSchema,
};
