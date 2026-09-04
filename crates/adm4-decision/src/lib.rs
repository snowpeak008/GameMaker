//! V4 决策模型：L0-L6 决策点、选项、参数表结构、Selection、适用性、DAG 校验、完成度，
//! 以及领域/节点两级内容组织（横向维度）与其进度聚合；
//! W7 起新增系统模块类型底座（`system_module`，知识层一等资产）。

mod applicability;
mod completeness;
pub mod composition;
mod consistency;
mod graph;
mod organization;
pub mod system_module;
mod types;

pub use applicability::{ApplicabilityMap, PointApplicability, compute_applicability};
pub use completeness::{
    CompletenessReport, MissingItem, compute_completeness, counts_toward_completeness,
    enumerate_axis, optional_unanswered, validate_option_parameters, validate_parameters,
    validate_selection_mode,
};
pub use composition::{
    CompositionBudget, CompositionFinding, CompositionInput, CompositionReport, FindingCode,
    InterfaceEdge, InterfacePort, ProductGrade, SystemInstance, check_composition,
    derive_core_link,
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
pub use system_module::{
    ConsistencyRule, ConsistencyRuleKind, CoreLink, FiveAxisRating, HeavinessBand, HeavinessLadder,
    HeavinessTier, Induction, InductionTarget, MdaMapping, NounDecl, NounId, NounKind, PromptEntry,
    PromptLibrary, SystemInterface, SystemModule, TierId,
};
pub use types::{
    AxisRef, CurveSchema, DecisionId, DecisionOption, DecisionPoint, DepthProfile, DesignLevel,
    DomainId, GenrePackId, GenreScope, GraphEntryConstraint, GraphSchema, MatrixSchema, MdaLayer,
    NaJustification, NodeId, OptionId, OptionSelector, ParamPath, ParameterSchema, ParameterValues,
    PointRequirement, Provenance, ScalarField, SelectedOption, SelectedOptionRef, Selection,
    SelectionMode, TableSchema, validate_graph_value,
};
