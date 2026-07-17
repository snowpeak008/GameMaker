use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use adm_new_design::anti_overfit::{
    apply_capability_mutation, capability_mutation_suite, permute_display_labels,
};
use adm_new_design::data_loader::{DesignDataLoader, DomainDocument};
use adm_new_design::decision_graph::{
    CapabilityDecisionGraph, CapabilityDecisionGraphCompiler, DecisionCoverage,
};
use adm_new_foundation::source_root::SourceProjectRoot;
use adm_new_foundation::{AdmError, AdmResult, io, sha256_hex};
use adm_new_game_spec::{
    GameSpec, ProductEnvelope, ProductionScale, SpecId, SpecKind, SpecRef, ValidationSeverity,
    canonicalize_game_spec, parse_game_spec, validate_game_spec, validate_game_spec_for_envelope,
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

pub const GAME_SPEC_V2_STEPS_COMPILER_VERSION: &str = "game_spec_v2_steps_00_06.v1";
const STEP_CONTRACT_VERSION: &str = "game_spec_step_contract.v1";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StepGateStatus {
    Passed,
    Blocked,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct StepGateIssue {
    pub code: String,
    pub severity: String,
    pub path: String,
    pub message: String,
    pub suggestion: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AntiOverfitEvidence {
    pub label_permutation_status_stable: bool,
    pub capability_mutation_changed_hash: bool,
    pub capability_mutation_kind: String,
    #[serde(default)]
    pub capability_mutation_axis_count: usize,
    #[serde(default)]
    pub capability_mutation_changed_axes: Vec<String>,
    #[serde(default)]
    pub capability_mutation_failed_axes: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct StepCompileReport {
    pub step_id: String,
    pub contract_version: String,
    pub compiler_version: String,
    pub source_hash: String,
    pub status: StepGateStatus,
    #[serde(default)]
    pub issues: Vec<StepGateIssue>,
    #[serde(default)]
    pub outputs: Value,
    #[serde(default)]
    pub trace_refs: Vec<SpecRef>,
    pub anti_overfit: AntiOverfitEvidence,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct FrozenGameSpecBundle {
    pub compiler_version: String,
    pub source_hash: String,
    pub semantic_hash: String,
    pub game_spec_hash: String,
    pub spec: GameSpec,
    pub reports: Vec<StepCompileReport>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Step00_06Run {
    pub status: StepGateStatus,
    pub source_hash: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub final_semantic_hash: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub frozen: Option<FrozenGameSpecBundle>,
    pub reports: Vec<StepCompileReport>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct GateOutcome {
    status: StepGateStatus,
    issues: Vec<StepGateIssue>,
    outputs: Value,
    trace_refs: Vec<SpecRef>,
}

#[derive(Debug, Clone)]
pub struct Step00_06Compiler {
    supported_envelope: ProductEnvelope,
    decision_domains: Result<Vec<DomainDocument>, String>,
}

impl Default for Step00_06Compiler {
    fn default() -> Self {
        Self {
            supported_envelope: ProductEnvelope {
                scene_scale: ProductionScale::Medium,
                system_complexity: ProductionScale::Medium,
                asset_scale: ProductionScale::Medium,
                content_volume: ProductionScale::Medium,
            },
            decision_domains: load_default_decision_domains(),
        }
    }
}

impl Step00_06Compiler {
    pub fn new(supported_envelope: ProductEnvelope) -> Self {
        Self {
            supported_envelope,
            decision_domains: load_default_decision_domains(),
        }
    }

    pub fn with_decision_domains(
        supported_envelope: ProductEnvelope,
        decision_domains: Vec<DomainDocument>,
    ) -> Self {
        Self {
            supported_envelope,
            decision_domains: Ok(decision_domains),
        }
    }

    pub fn from_design_data_dir(
        supported_envelope: ProductEnvelope,
        project_root: impl AsRef<Path>,
        design_data_dir: impl AsRef<Path>,
    ) -> AdmResult<Self> {
        let decision_domains =
            DesignDataLoader::from_design_data_dir(project_root.as_ref(), design_data_dir.as_ref())
                .load_domains()?;
        Ok(Self::with_decision_domains(
            supported_envelope,
            decision_domains,
        ))
    }

    pub fn compile_json(&self, input: &str) -> Step00_06Run {
        let source_hash = sha256_hex(input.as_bytes());
        let spec = match parse_game_spec(input) {
            Ok(spec) => spec,
            Err(error) => {
                return Step00_06Run {
                    status: StepGateStatus::Blocked,
                    source_hash: source_hash.clone(),
                    final_semantic_hash: None,
                    frozen: None,
                    reports: vec![parse_blocked_report(&source_hash, error.to_string())],
                };
            }
        };
        self.compile_spec(spec)
    }

    pub fn compile_spec(&self, spec: GameSpec) -> Step00_06Run {
        let source_hash = source_hash_for_spec(&spec);
        let gates: [(&str, GateFn); 7] = [
            ("00", gate_step00_input_contract),
            ("01", gate_step01_intent),
            ("02", gate_step02_capabilities),
            ("03", gate_step03_core_loop),
            ("04", gate_step04_reference_closure),
            ("05", gate_step05_scenario_coverage),
            ("06", gate_step06_freeze),
        ];
        let mut reports = Vec::new();
        for (step_id, gate) in gates {
            let report = self.run_gate(step_id, gate, &spec, &source_hash);
            let blocked = report.status == StepGateStatus::Blocked;
            reports.push(report);
            if blocked {
                return Step00_06Run {
                    status: StepGateStatus::Blocked,
                    source_hash,
                    final_semantic_hash: None,
                    frozen: None,
                    reports,
                };
            }
        }
        let canonical =
            canonicalize_game_spec(&spec).expect("Step06 gate already verified canonicalization");
        let frozen = FrozenGameSpecBundle {
            compiler_version: GAME_SPEC_V2_STEPS_COMPILER_VERSION.to_string(),
            source_hash: source_hash.clone(),
            semantic_hash: canonical.content_hash.clone(),
            game_spec_hash: canonical.content_hash.clone(),
            spec,
            reports: reports.clone(),
        };
        Step00_06Run {
            status: StepGateStatus::Passed,
            source_hash,
            final_semantic_hash: Some(canonical.content_hash),
            frozen: Some(frozen),
            reports,
        }
    }

    pub fn write_outputs(&self, out_dir: &Path, run: &Step00_06Run) -> AdmResult<Value> {
        std::fs::create_dir_all(out_dir)?;
        let mut reports = Vec::new();
        for report in &run.reports {
            let path = io::write_json_serializable(
                &out_dir.join(format!("step{}_contract_report.json", report.step_id)),
                report,
            )?;
            reports.push(json!({
                "stepId": report.step_id,
                "status": report.status,
                "path": path.to_string_lossy().replace('\\', "/"),
            }));
        }
        let summary_path =
            io::write_json_serializable(&out_dir.join("step00_06_run_summary.json"), run)?;
        let mut output = json!({
            "status": run.status,
            "sourceHash": run.source_hash,
            "summaryPath": summary_path.to_string_lossy().replace('\\', "/"),
            "reports": reports,
        });
        if let Some(frozen) = &run.frozen {
            let spec_path = io::write_json_serializable(
                &out_dir.join("r1_frozen_game_spec.json"),
                &frozen.spec,
            )?;
            let meta_path = io::write_json_serializable(
                &out_dir.join("r1_frozen_game_spec_meta.json"),
                frozen,
            )?;
            if let Some(output) = output.as_object_mut() {
                output.insert(
                    "frozenGameSpecPath".to_string(),
                    json!(spec_path.to_string_lossy().replace('\\', "/")),
                );
                output.insert(
                    "frozenGameSpecMetaPath".to_string(),
                    json!(meta_path.to_string_lossy().replace('\\', "/")),
                );
                output.insert("semanticHash".to_string(), json!(frozen.semantic_hash));
            }
        }
        Ok(output)
    }

    fn run_gate(
        &self,
        step_id: &str,
        gate: GateFn,
        spec: &GameSpec,
        source_hash: &str,
    ) -> StepCompileReport {
        let context = GateContext {
            decision_domains: &self.decision_domains,
        };
        let outcome = gate(spec, &self.supported_envelope, &context);
        let anti_overfit =
            anti_overfit_evidence(spec, &self.supported_envelope, &context, gate, &outcome);
        StepCompileReport {
            step_id: step_id.to_string(),
            contract_version: STEP_CONTRACT_VERSION.to_string(),
            compiler_version: GAME_SPEC_V2_STEPS_COMPILER_VERSION.to_string(),
            source_hash: source_hash.to_string(),
            status: outcome.status,
            issues: outcome.issues,
            outputs: outcome.outputs,
            trace_refs: outcome.trace_refs,
            anti_overfit,
        }
    }
}

fn load_default_decision_domains() -> Result<Vec<DomainDocument>, String> {
    let source_root = SourceProjectRoot::discover(env!("CARGO_MANIFEST_DIR"))
        .map_err(|error| format!("decision graph source root unavailable: {error}"))?;
    DesignDataLoader::new(source_root.path())
        .load_domains()
        .map_err(|error| format!("decision graph domains unavailable: {error}"))
}

#[derive(Debug, Clone, Copy)]
struct GateContext<'a> {
    decision_domains: &'a Result<Vec<DomainDocument>, String>,
}

type GateFn = fn(&GameSpec, &ProductEnvelope, &GateContext<'_>) -> GateOutcome;

fn gate_step00_input_contract(
    spec: &GameSpec,
    _supported: &ProductEnvelope,
    _context: &GateContext<'_>,
) -> GateOutcome {
    let mut issues = Vec::new();
    if spec.identity.schema_version.trim().is_empty() {
        issues.push(issue(
            "STEP00_SCHEMA_VERSION_EMPTY",
            "/identity/schemaVersion",
            "GameSpec schema version is required.",
            "Write the active GameSpec schema version before compiling.",
        ));
    }
    if spec.intent.title.trim().is_empty() {
        issues.push(issue(
            "STEP00_TITLE_EMPTY",
            "/intent/title",
            "GameSpec title is required.",
            "Provide a concrete game title or working title.",
        ));
    }
    if spec.intent.experience_promises.is_empty() {
        issues.push(issue(
            "STEP00_PROMISES_EMPTY",
            "/intent/experiencePromises",
            "At least one experience promise is required.",
            "Add an explicit promise that later steps can trace.",
        ));
    }
    outcome(
        issues,
        json!({
            "projectId": spec.identity.project_id,
            "promiseCount": spec.intent.experience_promises.len(),
            "entityCount": spec.entities.len(),
            "actionCount": spec.actions.len(),
            "scenarioCount": spec.acceptance_scenarios.len(),
        }),
        intent_refs(spec),
    )
}

fn gate_step01_intent(
    spec: &GameSpec,
    _supported: &ProductEnvelope,
    _context: &GateContext<'_>,
) -> GateOutcome {
    let mut issues = Vec::new();
    let must_have = normalized_scope_set(&spec.intent.scope.must_have);
    let wont_have = normalized_scope_set(&spec.intent.scope.wont_have);
    let contradictions = must_have
        .intersection(&wont_have)
        .cloned()
        .collect::<Vec<_>>();
    if !contradictions.is_empty() {
        issues.push(issue(
            "STEP01_SCOPE_CONTRADICTION",
            "/intent/scope",
            format!("Scope contains contradictory entries: {contradictions:?}"),
            "Remove the same requirement from either mustHave or wontHave.",
        ));
    }
    for (promise_id, promise) in &spec.intent.experience_promises {
        if promise.statement.trim().is_empty() {
            issues.push(issue(
                "STEP01_PROMISE_EMPTY",
                format!("/intent/experiencePromises/{promise_id}/statement"),
                "Experience promise statement is empty.",
                "Write a concrete player-visible promise.",
            ));
        }
    }
    outcome(
        issues,
        json!({
            "scopeMustHaveCount": spec.intent.scope.must_have.len(),
            "scopeWontHaveCount": spec.intent.scope.wont_have.len(),
            "contradictionCount": contradictions.len(),
        }),
        intent_refs(spec),
    )
}

fn gate_step02_capabilities(
    spec: &GameSpec,
    _supported: &ProductEnvelope,
    context: &GateContext<'_>,
) -> GateOutcome {
    let evidence = compile_step02_decision_graph(spec, context);
    let issues = evidence
        .as_ref()
        .err()
        .map(|error| vec![error.clone()])
        .unwrap_or_default();
    let outputs = evidence
        .as_ref()
        .map(step02_decision_graph_outputs)
        .unwrap_or_else(|_| {
            json!({
                "decisionGraphStatus": "blocked",
                "capabilityEvidence": [],
                "activeNodeCount": 0,
                "activationEvidenceCount": 0,
            })
        });
    outcome(
        issues,
        outputs,
        evidence
            .as_ref()
            .map(step02_decision_graph_refs)
            .unwrap_or_default(),
    )
}

fn compile_step02_decision_graph(
    spec: &GameSpec,
    context: &GateContext<'_>,
) -> Result<CapabilityDecisionGraph, StepGateIssue> {
    let domains = context.decision_domains.as_ref().map_err(|error| {
        issue(
            "STEP02_DECISION_GRAPH_UNAVAILABLE",
            "/capabilities",
            format!("A03 decision graph domains are unavailable: {error}"),
            "Provide the packaged knowledge/design_data domains before freezing Step02.",
        )
    })?;
    let graph = CapabilityDecisionGraphCompiler
        .compile(spec, domains, &DecisionCoverage::default())
        .map_err(|error| {
            issue(
                "STEP02_DECISION_GRAPH_EVIDENCE_INVALID",
                error.path,
                format!("A03 decision graph evidence is invalid: {}", error.message),
                error.suggestion,
            )
        })?;
    graph.validate_activation_evidence().map_err(|error| {
        issue(
            "STEP02_DECISION_GRAPH_EVIDENCE_INVALID",
            error.path,
            format!(
                "A03 decision graph activation evidence is invalid: {}",
                error.message
            ),
            error.suggestion,
        )
    })?;
    if graph.active_nodes.is_empty() {
        return Err(issue(
            "STEP02_DECISION_GRAPH_EMPTY",
            "/capabilities",
            "A03 decision graph produced no active capability nodes.",
            "Verify the GameSpec capability profile and domain activation policy before freezing Step02.",
        ));
    }
    Ok(graph)
}

fn step02_decision_graph_outputs(graph: &CapabilityDecisionGraph) -> Value {
    let capability_evidence = graph
        .active_nodes
        .iter()
        .flat_map(|node| {
            node.activation_reasons.iter().map(move |reason| {
                json!({
                    "nodeId": node.node_id,
                    "domainId": node.domain_id,
                    "predicateId": reason.predicate_id,
                    "sourcePath": reason.source_path,
                    "operator": reason.operator,
                    "expected": reason.expected,
                    "actual": reason.actual,
                })
            })
        })
        .collect::<Vec<_>>();
    let covered_paths = capability_evidence
        .iter()
        .filter_map(|item| item.get("sourcePath").and_then(Value::as_str))
        .collect::<BTreeSet<_>>();
    json!({
        "decisionGraphStatus": "passed",
        "activeNodeCount": graph.active_nodes.len(),
        "activationEvidenceCount": capability_evidence.len(),
        "coveredCapabilityPaths": covered_paths,
        "capabilityEvidence": capability_evidence,
        "decisionGraph": graph,
    })
}

fn step02_decision_graph_refs(graph: &CapabilityDecisionGraph) -> Vec<SpecRef> {
    let mut refs = graph
        .active_nodes
        .iter()
        .flat_map(|node| node.activation_reasons.iter())
        .filter_map(|reason| capability_ref_from_source_path(&reason.source_path))
        .collect::<Vec<_>>();
    refs.sort();
    refs.dedup();
    refs
}

fn capability_ref_from_source_path(source_path: &str) -> Option<SpecRef> {
    let relative = source_path.strip_prefix("/capabilities/")?;
    let id = relative.split('/').next()?.trim();
    if id.is_empty() {
        return None;
    }
    Some(SpecRef {
        kind: SpecKind::Capability,
        id: SpecId::new(id).ok()?,
        path: Some(source_path.to_string()),
    })
}

fn gate_step03_core_loop(
    spec: &GameSpec,
    _supported: &ProductEnvelope,
    _context: &GateContext<'_>,
) -> GateOutcome {
    let mut issues = Vec::new();
    if spec.actions.is_empty() {
        issues.push(issue(
            "STEP03_ACTIONS_EMPTY",
            "/actions",
            "Core loop has no actions.",
            "Declare at least one action that can be invoked by an acceptance scenario.",
        ));
    }
    if spec.state_machines.is_empty() {
        issues.push(issue(
            "STEP03_STATE_MACHINES_EMPTY",
            "/stateMachines",
            "Core loop has no state machine.",
            "Declare at least one state machine for playable progression.",
        ));
    }
    for promise_id in spec.intent.experience_promises.keys() {
        if scenarios_for_promise(spec, promise_id).is_empty() {
            issues.push(issue(
                "STEP03_PROMISE_NOT_TRACEABLE",
                format!("/intent/experiencePromises/{promise_id}"),
                format!("Promise {promise_id} does not trace to an executable scenario."),
                "Add a trace link from the promise to a scenario or invoked action.",
            ));
        }
    }
    outcome(
        issues,
        json!({
            "actionCount": spec.actions.len(),
            "stateMachineCount": spec.state_machines.len(),
            "traceLinkCount": spec.trace_links.len(),
        }),
        trace_refs_from_links(spec),
    )
}

fn gate_step04_reference_closure(
    spec: &GameSpec,
    _supported: &ProductEnvelope,
    _context: &GateContext<'_>,
) -> GateOutcome {
    let validation = validate_game_spec(spec);
    let issues = validation
        .issues
        .into_iter()
        .filter(|issue| issue.severity == ValidationSeverity::Error)
        .map(validation_issue)
        .collect::<Vec<_>>();
    outcome(
        issues,
        json!({
            "entityCount": spec.entities.len(),
            "componentCount": spec.components.len(),
            "relationshipCount": spec.relationships.len(),
            "resourceCount": spec.resources.len(),
            "spaceCount": spec.spaces.len(),
        }),
        trace_refs_from_links(spec),
    )
}

fn gate_step05_scenario_coverage(
    spec: &GameSpec,
    _supported: &ProductEnvelope,
    _context: &GateContext<'_>,
) -> GateOutcome {
    let mut issues = Vec::new();
    let mut coverage = BTreeMap::new();
    for promise_id in spec.intent.experience_promises.keys() {
        let scenario_ids = scenarios_for_promise(spec, promise_id);
        let positive = scenario_ids
            .iter()
            .filter(|id| {
                spec.acceptance_scenarios
                    .get(*id)
                    .is_some_and(|scenario| !scenario.failure_case)
            })
            .count();
        let negative = scenario_ids
            .iter()
            .filter(|id| {
                spec.acceptance_scenarios
                    .get(*id)
                    .is_some_and(|scenario| scenario.failure_case)
            })
            .count();
        coverage.insert(
            promise_id.to_string(),
            json!({
                "positiveScenarioCount": positive,
                "negativeScenarioCount": negative,
                "scenarioIds": scenario_ids,
            }),
        );
        if positive == 0 || negative == 0 {
            issues.push(issue(
                "STEP05_PROMISE_SCENARIO_POLARITY_INCOMPLETE",
                format!("/intent/experiencePromises/{promise_id}"),
                format!(
                    "Promise {promise_id} requires at least one positive and one negative scenario."
                ),
                "Trace both a success path and a failure/rejection path to this promise.",
            ));
        }
    }
    outcome(
        issues,
        json!({ "promiseScenarioCoverage": coverage }),
        trace_refs_from_links(spec),
    )
}

fn gate_step06_freeze(
    spec: &GameSpec,
    supported: &ProductEnvelope,
    _context: &GateContext<'_>,
) -> GateOutcome {
    let mut issues = validate_game_spec_for_envelope(spec, supported)
        .issues
        .into_iter()
        .filter(|issue| issue.severity == ValidationSeverity::Error)
        .map(validation_issue)
        .collect::<Vec<_>>();
    let first = canonicalize_game_spec(spec);
    let second = canonicalize_game_spec(spec);
    match (first, second) {
        (Ok(first), Ok(second)) if first.content_hash == second.content_hash => {
            let frozen = issues.is_empty();
            outcome(
                issues,
                json!({
                    "frozen": frozen,
                    "semanticHash": first.content_hash,
                    "supportedEnvelope": supported,
                    "requestedEnvelope": spec.technical.product_envelope,
                }),
                trace_refs_from_links(spec),
            )
        }
        (Ok(first), Ok(second)) => {
            issues.push(issue(
                "STEP06_HASH_UNSTABLE",
                "/",
                format!(
                    "GameSpec canonical hash is unstable: {} vs {}",
                    first.content_hash, second.content_hash
                ),
                "Remove non-deterministic fields or map ordering from the specification.",
            ));
            outcome(
                issues,
                json!({ "frozen": false }),
                trace_refs_from_links(spec),
            )
        }
        (Err(error), _) | (_, Err(error)) => {
            issues.push(issue(
                "STEP06_CANONICALIZATION_FAILED",
                "/",
                format!("GameSpec canonicalization failed: {error}"),
                "Fix strict serialization issues before freezing.",
            ));
            outcome(
                issues,
                json!({ "frozen": false }),
                trace_refs_from_links(spec),
            )
        }
    }
}

fn parse_blocked_report(source_hash: &str, message: String) -> StepCompileReport {
    StepCompileReport {
        step_id: "00".to_string(),
        contract_version: STEP_CONTRACT_VERSION.to_string(),
        compiler_version: GAME_SPEC_V2_STEPS_COMPILER_VERSION.to_string(),
        source_hash: source_hash.to_string(),
        status: StepGateStatus::Blocked,
        issues: vec![issue(
            "STEP00_PARSE_FAILED",
            "/",
            message,
            "Provide strict JSON conforming to the GameSpec schema.",
        )],
        outputs: json!({ "parsed": false }),
        trace_refs: Vec::new(),
        anti_overfit: AntiOverfitEvidence {
            label_permutation_status_stable: false,
            capability_mutation_changed_hash: false,
            capability_mutation_kind: "not_run_parse_failed".to_string(),
            capability_mutation_axis_count: 0,
            capability_mutation_changed_axes: Vec::new(),
            capability_mutation_failed_axes: Vec::new(),
        },
    }
}

fn anti_overfit_evidence(
    spec: &GameSpec,
    supported: &ProductEnvelope,
    context: &GateContext<'_>,
    gate: GateFn,
    baseline: &GateOutcome,
) -> AntiOverfitEvidence {
    let label_status_stable = permute_display_labels(spec, "permuted_design_label")
        .map(|permuted| gate(&permuted, supported, context).status == baseline.status)
        .unwrap_or(false);
    let (axis_count, changed_axes, failed_axes) = capability_mutation_hash_evidence(spec);
    let capability_changed =
        axis_count > 0 && changed_axes.len() == axis_count && failed_axes.is_empty();
    AntiOverfitEvidence {
        label_permutation_status_stable: label_status_stable,
        capability_mutation_changed_hash: capability_changed,
        capability_mutation_kind: "multi_axis_capability_suite".to_string(),
        capability_mutation_axis_count: axis_count,
        capability_mutation_changed_axes: changed_axes,
        capability_mutation_failed_axes: failed_axes,
    }
}

fn capability_mutation_hash_evidence(spec: &GameSpec) -> (usize, Vec<String>, Vec<String>) {
    let original_hash = canonicalize_game_spec(spec)
        .map(|canonical| canonical.content_hash)
        .ok();
    let mut changed_axes = Vec::new();
    let mut failed_axes = Vec::new();
    let suite = capability_mutation_suite(spec);
    for case in &suite {
        let changed = original_hash.as_ref().is_some_and(|original_hash| {
            apply_capability_mutation(spec, case.mutation)
                .ok()
                .and_then(|mutated| canonicalize_game_spec(&mutated).ok())
                .is_some_and(|mutated| mutated.content_hash != *original_hash)
        });
        if changed {
            changed_axes.push(case.axis.to_string());
        } else {
            failed_axes.push(case.axis.to_string());
        }
    }
    (suite.len(), changed_axes, failed_axes)
}

fn outcome(issues: Vec<StepGateIssue>, outputs: Value, trace_refs: Vec<SpecRef>) -> GateOutcome {
    GateOutcome {
        status: if issues.is_empty() {
            StepGateStatus::Passed
        } else {
            StepGateStatus::Blocked
        },
        issues,
        outputs,
        trace_refs,
    }
}

fn issue(
    code: impl Into<String>,
    path: impl Into<String>,
    message: impl Into<String>,
    suggestion: impl Into<String>,
) -> StepGateIssue {
    StepGateIssue {
        code: code.into(),
        severity: "error".to_string(),
        path: path.into(),
        message: message.into(),
        suggestion: suggestion.into(),
    }
}

fn validation_issue(issue: adm_new_game_spec::SpecValidationIssue) -> StepGateIssue {
    StepGateIssue {
        code: issue.code,
        severity: "error".to_string(),
        path: issue.path,
        message: issue.message,
        suggestion: issue.suggestion,
    }
}

fn source_hash_for_spec(spec: &GameSpec) -> String {
    canonicalize_game_spec(spec)
        .map(|canonical| canonical.content_hash)
        .unwrap_or_else(|_| sha256_hex(serde_json::to_vec(spec).unwrap_or_default().as_slice()))
}

fn normalized_scope_set(values: &[String]) -> BTreeSet<String> {
    values
        .iter()
        .map(|value| value.trim().to_ascii_lowercase())
        .filter(|value| !value.is_empty())
        .collect()
}

fn intent_refs(spec: &GameSpec) -> Vec<SpecRef> {
    spec.intent
        .experience_promises
        .keys()
        .map(|id| SpecRef {
            kind: SpecKind::Intent,
            id: id.clone(),
            path: None,
        })
        .collect()
}

fn trace_refs_from_links(spec: &GameSpec) -> Vec<SpecRef> {
    let mut refs = Vec::new();
    for trace in spec.trace_links.values() {
        refs.push(trace.source.clone());
        refs.push(trace.target.clone());
    }
    refs.sort();
    refs.dedup();
    refs
}

fn scenarios_for_promise(spec: &GameSpec, promise_id: &SpecId) -> BTreeSet<SpecId> {
    let mut scenarios = BTreeSet::new();
    let mut actions = BTreeSet::new();
    for trace in spec.trace_links.values() {
        if trace.source.kind != SpecKind::Intent || trace.source.id != *promise_id {
            continue;
        }
        match trace.target.kind {
            SpecKind::Scenario => {
                if spec.acceptance_scenarios.contains_key(&trace.target.id) {
                    scenarios.insert(trace.target.id.clone());
                }
            }
            SpecKind::Action => {
                if spec.actions.contains_key(&trace.target.id) {
                    actions.insert(trace.target.id.clone());
                }
            }
            _ => {}
        }
    }
    for (scenario_id, scenario) in &spec.acceptance_scenarios {
        if scenario
            .when
            .iter()
            .any(|invocation| actions.contains(&invocation.action))
        {
            scenarios.insert(scenario_id.clone());
        }
    }
    scenarios
}

pub fn read_fixture(path: &Path) -> AdmResult<GameSpec> {
    let input = std::fs::read_to_string(path)
        .map_err(|error| AdmError::new(format!("failed to read GameSpec fixture: {error}")))?;
    parse_game_spec(&input).map_err(|error| AdmError::new(error.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn r1_fixture() -> GameSpec {
        parse_game_spec(include_str!(
            "../../../testdata/game_spec/r1c0_micro_ecodome_lane_guard_frozen.json"
        ))
        .expect("R1-C0 fixture should parse")
    }

    #[test]
    fn r1c0_fixture_passes_step00_06_and_writes_outputs() {
        let compiler = Step00_06Compiler::default();
        let run = compiler.compile_spec(r1_fixture());

        assert_eq!(run.status, StepGateStatus::Passed);
        assert_eq!(run.reports.len(), 7);
        assert!(run.final_semantic_hash.is_some());
        assert!(
            run.reports
                .iter()
                .all(|report| report.anti_overfit.label_permutation_status_stable)
        );
        assert!(
            run.reports
                .iter()
                .all(|report| report.anti_overfit.capability_mutation_changed_hash)
        );
        assert!(run.reports.iter().all(|report| {
            report.anti_overfit.capability_mutation_axis_count == 16
                && report.anti_overfit.capability_mutation_changed_axes.len() == 16
                && report
                    .anti_overfit
                    .capability_mutation_failed_axes
                    .is_empty()
        }));

        let root = std::env::temp_dir()
            .join(adm_new_foundation::new_stable_id("game_spec_v2_steps").unwrap());
        let output = compiler.write_outputs(&root, &run).unwrap();

        assert!(Path::new(output["frozenGameSpecPath"].as_str().unwrap()).exists());
        assert!(Path::new(output["frozenGameSpecMetaPath"].as_str().unwrap()).exists());
        assert!(root.join("step06_contract_report.json").exists());
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn repeated_compile_is_semantically_stable() {
        let compiler = Step00_06Compiler::default();
        let first = compiler.compile_spec(r1_fixture());
        let second = compiler.compile_spec(r1_fixture());

        assert_eq!(first.final_semantic_hash, second.final_semantic_hash);
        assert_eq!(first.source_hash, second.source_hash);
    }

    #[test]
    fn step02_consumes_a03_decision_graph_activation_evidence() {
        let compiler = Step00_06Compiler::default();
        let run = compiler.compile_spec(r1_fixture());

        assert_eq!(run.status, StepGateStatus::Passed);
        let step02 = run
            .reports
            .iter()
            .find(|report| report.step_id == "02")
            .expect("Step02 report must exist");
        assert_eq!(step02.status, StepGateStatus::Passed);
        assert_eq!(step02.outputs["decisionGraphStatus"], "passed");
        assert!(step02.outputs.get("capabilityReasons").is_none());
        let evidence = step02.outputs["capabilityEvidence"]
            .as_array()
            .expect("Step02 must expose activation evidence");
        assert!(!evidence.is_empty());
        assert!(evidence.iter().all(|item| {
            item["predicateId"]
                .as_str()
                .is_some_and(|value| !value.is_empty())
                && item["sourcePath"]
                    .as_str()
                    .is_some_and(|value| value.starts_with("/capabilities/"))
                && item["operator"]
                    .as_str()
                    .is_some_and(|value| !value.is_empty())
                && item["expected"]
                    .as_array()
                    .is_some_and(|items| !items.is_empty())
                && item["actual"]
                    .as_str()
                    .is_some_and(|value| !value.is_empty())
        }));
        assert!(step02.trace_refs.iter().any(|reference| {
            reference
                .path
                .as_deref()
                .is_some_and(|path| path.starts_with("/capabilities/"))
        }));
    }

    #[test]
    fn step02_blocks_when_a03_decision_domains_are_missing() {
        let compiler = Step00_06Compiler::with_decision_domains(medium_envelope(), Vec::new());

        let run = compiler.compile_spec(r1_fixture());

        assert_eq!(run.status, StepGateStatus::Blocked);
        let last = run.reports.last().unwrap();
        assert_eq!(last.step_id, "02");
        assert!(last.issues.iter().any(|issue| {
            issue.code == "STEP02_DECISION_GRAPH_EVIDENCE_INVALID"
                || issue.code == "STEP02_DECISION_GRAPH_UNAVAILABLE"
        }));
        assert!(
            !last
                .issues
                .iter()
                .any(|issue| issue.code == "STEP02_CAPABILITY_REASON_EMPTY")
        );
    }

    #[test]
    fn upstream_scope_error_blocks_at_step01() {
        let compiler = Step00_06Compiler::default();
        let mut spec = r1_fixture();
        spec.intent.scope.must_have.push("network play".to_string());
        spec.intent.scope.wont_have.push("network play".to_string());

        let run = compiler.compile_spec(spec);

        assert_eq!(run.status, StepGateStatus::Blocked);
        assert_eq!(run.reports.last().unwrap().step_id, "01");
        assert_eq!(run.reports.len(), 2);
        assert_eq!(
            run.reports.last().unwrap().issues[0].code,
            "STEP01_SCOPE_CONTRADICTION"
        );
    }

    #[test]
    fn excessive_envelope_blocks_at_step06() {
        let compiler = Step00_06Compiler::default();
        let mut spec = r1_fixture();
        spec.technical.product_envelope.system_complexity = ProductionScale::Large;

        let run = compiler.compile_spec(spec);

        assert_eq!(run.status, StepGateStatus::Blocked);
        assert_eq!(run.reports.last().unwrap().step_id, "06");
        assert!(
            run.reports
                .last()
                .unwrap()
                .issues
                .iter()
                .any(|issue| issue.code == "SPEC_ENVELOPE_EXCEEDED")
        );
    }

    #[test]
    fn missing_negative_scenario_blocks_at_step05() {
        let compiler = Step00_06Compiler::default();
        let mut spec = r1_fixture();
        for scenario in spec.acceptance_scenarios.values_mut() {
            scenario.failure_case = false;
        }

        let run = compiler.compile_spec(spec);

        assert_eq!(run.status, StepGateStatus::Blocked);
        assert_eq!(run.reports.last().unwrap().step_id, "05");
        assert!(
            run.reports
                .last()
                .unwrap()
                .issues
                .iter()
                .any(|issue| issue.code == "STEP05_PROMISE_SCENARIO_POLARITY_INCOMPLETE")
        );
    }

    fn medium_envelope() -> ProductEnvelope {
        ProductEnvelope {
            scene_scale: ProductionScale::Medium,
            system_complexity: ProductionScale::Medium,
            asset_scale: ProductionScale::Medium,
            content_volume: ProductionScale::Medium,
        }
    }
}
