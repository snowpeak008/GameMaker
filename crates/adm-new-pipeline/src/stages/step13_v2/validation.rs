use std::collections::BTreeMap;
use std::path::Path;

use adm_new_foundation::{AdmError, AdmResult, io, sha256_hex};
use adm_new_game_spec::{AcceptanceScenario, GameSpec, canonicalize_game_spec};

use crate::stages::step11_v2::{Step11ExecutionReport, Step11ExecutionStatus};
use crate::stages::step12_v2::{Step12AssetProductionOutput, Step12Status};
use crate::stages::step13_v2::types::{
    AcceptanceScenarioResult, AccessibilityCheckResult, AutomationKind, PerformanceCheckResult,
    STEP13_EXECUTION_EVIDENCE_SCHEMA_VERSION, STEP13_V2_COMPILER_VERSION,
    ScenarioExecutionObservation, ScenarioExecutionStatus, Step13AcceptanceOutput,
    Step13ExecutionEvidence, Step13Status, Step13ValidationPolicy,
};

pub fn run_step13_acceptance_validation(
    spec: &GameSpec,
    step11: &Step11ExecutionReport,
    step12: &Step12AssetProductionOutput,
    policy: &Step13ValidationPolicy,
    out_dir: &Path,
) -> AdmResult<Step13AcceptanceOutput> {
    std::fs::create_dir_all(out_dir)?;
    let spec_hash = canonicalize_game_spec(spec)
        .map_err(|error| AdmError::new(format!("GameSpec hash failed: {error}")))?
        .content_hash;
    let build_hash = compute_step13_build_hash(step11, step12);
    let evidence = policy.execution_evidence.as_ref();
    let evidence_problem =
        evidence.and_then(|evidence| validate_execution_evidence(evidence, &build_hash));
    let scenario_results = spec
        .acceptance_scenarios
        .iter()
        .map(|(scenario_id, scenario)| {
            scenario_result(
                scenario_id.as_str(),
                scenario,
                spec,
                step11,
                step12,
                &spec_hash,
                &build_hash,
                evidence,
                evidence_problem.as_deref(),
            )
        })
        .collect::<Vec<_>>();
    let status = output_status(&scenario_results);
    let scenario_path = io::write_json_serializable(
        &out_dir.join("scenario_execution_results.json"),
        &scenario_results,
    )?;
    let performance_path = io::write_json_serializable(
        &out_dir.join("performance_validation_report.json"),
        &performance_report(&scenario_results),
    )?;
    let manual_path = io::write_json_serializable(
        &out_dir.join("manual_review_report.json"),
        &manual_review_report(&scenario_results),
    )?;
    let regression_path = io::write_json_serializable(
        &out_dir.join("regression_report.json"),
        &regression_report(&scenario_results, policy),
    )?;
    let request_path = io::write_json_serializable(
        &out_dir.join("scenario_execution_request.json"),
        &scenario_execution_request(spec, &spec_hash, &build_hash),
    )?;
    let output = Step13AcceptanceOutput {
        schema_version: "step13_acceptance_validation.v1".to_string(),
        compiler_version: STEP13_V2_COMPILER_VERSION.to_string(),
        status,
        spec_hash,
        build_hash,
        scenario_results,
        output_paths: BTreeMap::from([
            (
                "scenarioExecutionResults".to_string(),
                path_string(&scenario_path),
            ),
            (
                "performanceValidationReport".to_string(),
                path_string(&performance_path),
            ),
            ("manualReviewReport".to_string(), path_string(&manual_path)),
            (
                "regressionReport".to_string(),
                path_string(&regression_path),
            ),
            (
                "scenarioExecutionRequest".to_string(),
                path_string(&request_path),
            ),
        ]),
    };
    io::write_json_serializable(&out_dir.join("step13_acceptance_output.json"), &output)?;
    Ok(output)
}

fn scenario_result(
    scenario_id: &str,
    scenario: &AcceptanceScenario,
    spec: &GameSpec,
    step11: &Step11ExecutionReport,
    step12: &Step12AssetProductionOutput,
    spec_hash: &str,
    build_hash: &str,
    evidence: Option<&Step13ExecutionEvidence>,
    evidence_problem: Option<&str>,
) -> AcceptanceScenarioResult {
    let action_ids = scenario
        .when
        .iter()
        .map(|action| action.action.to_string())
        .collect::<Vec<_>>();
    let manual = is_manual_review_scenario(scenario);
    let scenario_observation =
        evidence.and_then(|evidence| evidence.scenario_observations.get(scenario_id));
    let performance_checks = performance_checks(scenario, spec, evidence);
    let accessibility_checks = accessibility_checks(spec, evidence);
    let failure_reason = failure_reason(
        scenario,
        &action_ids,
        step11,
        step12,
        scenario_observation,
        evidence,
        evidence_problem,
        &performance_checks,
        &accessibility_checks,
    );
    let status = if failure_reason.is_some() {
        ScenarioExecutionStatus::Failed
    } else if manual
        && !evidence.is_some_and(|evidence| evidence.completed_manual_reviews.contains(scenario_id))
    {
        ScenarioExecutionStatus::ManualReviewRequired
    } else {
        ScenarioExecutionStatus::Passed
    };
    let log_hash = scenario_observation
        .map(|observation| observation.log_hash.clone())
        .unwrap_or_else(|| {
            sha256_hex(
                format!("{scenario_id}:{:?}:{build_hash}:{failure_reason:?}", status).as_bytes(),
            )
        });
    AcceptanceScenarioResult {
        scenario_id: scenario_id.to_string(),
        summary: scenario.summary.clone(),
        automation_kind: if manual {
            AutomationKind::ManualReview
        } else {
            AutomationKind::Automated
        },
        status,
        action_ids,
        spec_hash: spec_hash.to_string(),
        build_hash: build_hash.to_string(),
        log_hash,
        performance_checks,
        accessibility_checks,
        failure_reason,
    }
}

fn failure_reason(
    scenario: &AcceptanceScenario,
    action_ids: &[String],
    step11: &Step11ExecutionReport,
    step12: &Step12AssetProductionOutput,
    scenario_observation: Option<&ScenarioExecutionObservation>,
    evidence: Option<&Step13ExecutionEvidence>,
    evidence_problem: Option<&str>,
    performance_checks: &[PerformanceCheckResult],
    accessibility_checks: &[AccessibilityCheckResult],
) -> Option<String> {
    if step11.status != Step11ExecutionStatus::Success || !step11.correction_queue.is_empty() {
        return Some("Step11 code execution has unresolved failures".to_string());
    }
    if step12.status != Step12Status::Success || !step12.correction_queue.is_empty() {
        return Some("Step12 asset production has unresolved failures".to_string());
    }
    if let Some(problem) = evidence_problem {
        return Some(problem.to_string());
    }
    if !is_manual_review_scenario(scenario) {
        let Some(observation) = scenario_observation else {
            return Some("scenario execution evidence is missing".to_string());
        };
        if observation.status == ScenarioExecutionStatus::ManualReviewRequired {
            return Some(
                "automated scenario execution evidence cannot claim manual review status"
                    .to_string(),
            );
        }
        if observation.status == ScenarioExecutionStatus::Failed {
            return Some(
                observation
                    .failure_reason
                    .clone()
                    .unwrap_or_else(|| "scenario executor reported failure".to_string()),
            );
        }
        if let Some(action) = action_ids
            .iter()
            .find(|action| !observation.executed_action_ids.contains(action))
        {
            return Some(format!(
                "scenario execution evidence did not execute required action '{action}'"
            ));
        }
    }
    if let Some(action) = action_ids.iter().find(|action| {
        evidence.is_some_and(|evidence| evidence.disabled_action_ids.contains(action.as_str()))
    }) {
        return Some(format!(
            "required action '{action}' is disabled in the tested build"
        ));
    }
    if evidence.is_some_and(|evidence| !evidence.missing_asset_ids.is_empty())
        && scenario.asset_validation_required
    {
        return Some("tested build is missing assets required by this scenario".to_string());
    }
    if performance_checks.iter().any(|check| !check.passed) {
        return Some("performance budget check failed or lacks an observation".to_string());
    }
    if accessibility_checks.iter().any(|check| !check.passed) {
        return Some("accessibility requirement check failed".to_string());
    }
    None
}

fn performance_checks(
    scenario: &AcceptanceScenario,
    spec: &GameSpec,
    evidence: Option<&Step13ExecutionEvidence>,
) -> Vec<PerformanceCheckResult> {
    if scenario.performance_budget_refs.is_empty() {
        return Vec::new();
    }
    scenario
        .performance_budget_refs
        .iter()
        .map(|budget_id| {
            let limit = spec.technical.performance_budgets.get(budget_id.as_str());
            let observed = limit.and_then(|_| {
                evidence.and_then(|evidence| {
                    evidence
                        .performance_observations
                        .get(budget_id.as_str())
                        .copied()
                })
            });
            PerformanceCheckResult {
                budget_id: budget_id.to_string(),
                limit: limit.copied().unwrap_or_default(),
                observed,
                passed: limit.is_some_and(|limit| observed.is_some_and(|value| value <= *limit)),
            }
        })
        .collect()
}

fn accessibility_checks(
    spec: &GameSpec,
    evidence: Option<&Step13ExecutionEvidence>,
) -> Vec<AccessibilityCheckResult> {
    spec.technical
        .accessibility_requirements
        .iter()
        .map(|requirement| {
            let observed = evidence.and_then(|evidence| {
                evidence
                    .accessibility_observations
                    .get(requirement.as_str())
                    .copied()
            });
            AccessibilityCheckResult {
                requirement: requirement.clone(),
                observed,
                passed: observed.unwrap_or(false),
            }
        })
        .collect()
}

fn is_manual_review_scenario(scenario: &AcceptanceScenario) -> bool {
    scenario.manual_review_required
}

fn output_status(results: &[AcceptanceScenarioResult]) -> Step13Status {
    if results
        .iter()
        .any(|result| result.status == ScenarioExecutionStatus::Failed)
    {
        Step13Status::Failed
    } else if results
        .iter()
        .any(|result| result.status == ScenarioExecutionStatus::ManualReviewRequired)
    {
        Step13Status::WaitingManualReview
    } else {
        Step13Status::Passed
    }
}

fn performance_report(
    results: &[AcceptanceScenarioResult],
) -> BTreeMap<String, Vec<PerformanceCheckResult>> {
    results
        .iter()
        .filter(|result| !result.performance_checks.is_empty())
        .map(|result| {
            (
                result.scenario_id.clone(),
                result.performance_checks.clone(),
            )
        })
        .collect()
}

fn manual_review_report(results: &[AcceptanceScenarioResult]) -> BTreeMap<String, String> {
    results
        .iter()
        .filter(|result| result.automation_kind == AutomationKind::ManualReview)
        .map(|result| (result.scenario_id.clone(), format!("{:?}", result.status)))
        .collect()
}

fn regression_report(
    results: &[AcceptanceScenarioResult],
    policy: &Step13ValidationPolicy,
) -> BTreeMap<String, String> {
    let evidence = policy.execution_evidence.as_ref();
    BTreeMap::from([
        (
            "disabledActionCount".to_string(),
            evidence
                .map(|evidence| evidence.disabled_action_ids.len())
                .unwrap_or_default()
                .to_string(),
        ),
        (
            "missingAssetCount".to_string(),
            evidence
                .map(|evidence| evidence.missing_asset_ids.len())
                .unwrap_or_default()
                .to_string(),
        ),
        (
            "failedScenarioCount".to_string(),
            results
                .iter()
                .filter(|result| result.status == ScenarioExecutionStatus::Failed)
                .count()
                .to_string(),
        ),
    ])
}

pub fn compute_step13_build_hash(
    step11: &Step11ExecutionReport,
    step12: &Step12AssetProductionOutput,
) -> String {
    sha256_hex(
        format!(
            "{}:{}:{:?}",
            step11.ending_tree_hash, step12.source_asset_manifest_hash, step12.status
        )
        .as_bytes(),
    )
}

fn validate_execution_evidence(
    evidence: &Step13ExecutionEvidence,
    build_hash: &str,
) -> Option<String> {
    if evidence.schema_version != STEP13_EXECUTION_EVIDENCE_SCHEMA_VERSION {
        return Some(format!(
            "scenario execution evidence schema is unsupported: {}",
            evidence.schema_version
        ));
    }
    if evidence.build_hash != build_hash {
        return Some(
            "scenario execution evidence build hash does not match Step11/12 output".to_string(),
        );
    }
    if evidence.executor_id.trim().is_empty() {
        return Some("scenario execution evidence executor id is empty".to_string());
    }
    None
}

fn scenario_execution_request(
    spec: &GameSpec,
    spec_hash: &str,
    build_hash: &str,
) -> BTreeMap<String, serde_json::Value> {
    let scenarios = spec
        .acceptance_scenarios
        .iter()
        .map(|(id, scenario)| {
            (
                id.to_string(),
                serde_json::json!({
                    "summary": scenario.summary,
                    "manualReviewRequired": scenario.manual_review_required,
                    "performanceBudgetRefs": scenario.performance_budget_refs,
                    "assetValidationRequired": scenario.asset_validation_required,
                    "requiredActionIds": scenario.when.iter().map(|action| action.action.to_string()).collect::<Vec<_>>(),
                }),
            )
        })
        .collect::<BTreeMap<_, _>>();
    BTreeMap::from([
        (
            "schemaVersion".to_string(),
            serde_json::json!("step13_scenario_execution_request.v1"),
        ),
        ("specHash".to_string(), serde_json::json!(spec_hash)),
        ("buildHash".to_string(), serde_json::json!(build_hash)),
        (
            "expectedEvidenceSchema".to_string(),
            serde_json::json!(STEP13_EXECUTION_EVIDENCE_SCHEMA_VERSION),
        ),
        (
            "performanceBudgets".to_string(),
            serde_json::to_value(&spec.technical.performance_budgets).unwrap_or_default(),
        ),
        (
            "accessibilityRequirements".to_string(),
            serde_json::to_value(&spec.technical.accessibility_requirements).unwrap_or_default(),
        ),
        (
            "scenarios".to_string(),
            serde_json::to_value(scenarios).unwrap_or_default(),
        ),
    ])
}

fn path_string(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}
