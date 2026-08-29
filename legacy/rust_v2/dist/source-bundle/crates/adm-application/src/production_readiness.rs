use adm_assets::AssetPlan;
use adm_design::{DesignEvaluation, DesignProject};
use adm_development::DevelopmentPlan;
use adm_packaging::GameBuildPlan;
use adm_sdk::SdkKnowledgeBase;
use adm_validation::{ValidationReport, ValidationStatus};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProductionReadinessStatus {
    Ready,
    Warning,
    Blocked,
}

impl ProductionReadinessStatus {
    fn as_str(self) -> &'static str {
        match self {
            Self::Ready => "ready",
            Self::Warning => "warning",
            Self::Blocked => "blocked",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProductionReadinessCheck {
    pub check_id: String,
    pub status: ProductionReadinessStatus,
    pub expected: usize,
    pub actual: usize,
    pub artifact_refs: Vec<String>,
    pub detail: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProductionReadinessReport {
    pub project_id: String,
    pub title: String,
    pub overall_status: ProductionReadinessStatus,
    pub checks: Vec<ProductionReadinessCheck>,
    pub trace_artifacts: Vec<String>,
}

impl ProductionReadinessReport {
    pub fn render(&self) -> String {
        let ready_count = self
            .checks
            .iter()
            .filter(|check| check.status == ProductionReadinessStatus::Ready)
            .count();
        let warning_count = self
            .checks
            .iter()
            .filter(|check| check.status == ProductionReadinessStatus::Warning)
            .count();
        let blocking_count = self
            .checks
            .iter()
            .filter(|check| check.status == ProductionReadinessStatus::Blocked)
            .count();

        let mut document = String::from("# Production Readiness Report\n");
        document.push_str(&format!("project_id={}\n", self.project_id));
        document.push_str(&format!("title={}\n", sanitize_inline(&self.title)));
        document.push_str(&format!(
            "overall_status={}\n",
            self.overall_status.as_str()
        ));
        document.push_str(&format!("readiness_checks={}\n", self.checks.len()));
        document.push_str(&format!("ready_count={ready_count}\n"));
        document.push_str(&format!("warning_count={warning_count}\n"));
        document.push_str(&format!("blocking_count={blocking_count}\n"));
        document.push_str(&format!(
            "trace_artifacts={}\n",
            self.trace_artifacts.join(" | ")
        ));
        document.push_str("\n## Checks\n");
        for check in &self.checks {
            document.push_str(&format!(
                "- check_id={}; status={}; expected={}; actual={}; artifacts={}; detail={}\n",
                check.check_id,
                check.status.as_str(),
                check.expected,
                check.actual,
                check.artifact_refs.join(" | "),
                sanitize_inline(&check.detail)
            ));
        }
        document
    }
}

pub fn build_production_readiness_report(
    project: &DesignProject,
    evaluation: &DesignEvaluation,
    development_plan: &DevelopmentPlan,
    asset_plan: &AssetPlan,
    sdk: &SdkKnowledgeBase,
    build_targets_plan: &GameBuildPlan,
    input_validation: &ValidationReport,
    acceptance_matrix_document: &str,
    scenario_test_plan_document: &str,
    runtime_validation_document: &str,
) -> ProductionReadinessReport {
    let expected_loop_steps = project.core_loop.len();
    let acceptance = acceptance_summary(acceptance_matrix_document);
    let scenario_tests = acceptance_summary(scenario_test_plan_document);
    let runtime_validation = acceptance_summary(runtime_validation_document);
    let required_sdk_resources = sdk
        .resources
        .iter()
        .filter(|resource| resource.required_for_build)
        .count();
    let mechanic_feedback_tasks = asset_plan
        .tasks
        .iter()
        .filter(|task| task.pipeline_stage == "mechanic_feedback")
        .count();
    let build_required_artifacts = build_targets_plan
        .targets
        .iter()
        .map(|target| target.required_artifacts.len())
        .sum();

    let checks = vec![
        ProductionReadinessCheck {
            check_id: "design_quality".to_string(),
            status: if evaluation.requires_ai_intervention {
                ProductionReadinessStatus::Warning
            } else {
                ProductionReadinessStatus::Ready
            },
            expected: 75,
            actual: (evaluation.score.value.clamp(0.0, 1.0) * 100.0).round() as usize,
            artifact_refs: vec![
                "design/project.adm".to_string(),
                "ai/journal.adm".to_string(),
            ],
            detail: "design quality reaches the no-intervention threshold".to_string(),
        },
        count_check(
            "playable_scenario_coverage",
            expected_loop_steps,
            project.playable_scenarios.len(),
            vec!["design/project.adm"],
            "every core loop step has a playable scenario",
        ),
        count_check(
            "development_task_coverage",
            expected_loop_steps,
            development_plan.tasks.len(),
            vec!["development/plan.adm", "validation/acceptance_matrix.adm"],
            "every core loop step has an implementation task",
        ),
        count_check(
            "asset_feedback_coverage",
            expected_loop_steps,
            mechanic_feedback_tasks,
            vec!["assets/plan.adm", "validation/acceptance_matrix.adm"],
            "every core loop step has mechanic feedback assets",
        ),
        count_check(
            "sdk_build_readiness",
            1,
            required_sdk_resources,
            vec!["sdk/index.adm", "package/build_targets.adm"],
            "at least one build-required SDK resource supports delivery",
        ),
        count_check(
            "build_target_readiness",
            1,
            build_targets_plan.targets.len(),
            vec!["package/build_targets.adm"],
            "at least one game build target is declared",
        ),
        count_check(
            "build_artifact_contract",
            1,
            build_required_artifacts,
            vec![
                "package/build_targets.adm",
                "pipeline/artifact_registry.adm",
            ],
            "build targets list concrete required artifacts",
        ),
        ProductionReadinessCheck {
            check_id: "acceptance_trace_readiness".to_string(),
            status: if acceptance.rows >= expected_loop_steps
                && acceptance.rows > 0
                && acceptance.incomplete == 0
            {
                ProductionReadinessStatus::Ready
            } else if acceptance.rows > 0 {
                ProductionReadinessStatus::Warning
            } else {
                ProductionReadinessStatus::Blocked
            },
            expected: expected_loop_steps,
            actual: acceptance.ready,
            artifact_refs: vec!["validation/acceptance_matrix.adm".to_string()],
            detail: format!(
                "acceptance rows={}, ready={}, incomplete={}",
                acceptance.rows, acceptance.ready, acceptance.incomplete
            ),
        },
        ProductionReadinessCheck {
            check_id: "scenario_test_plan_readiness".to_string(),
            status: if scenario_tests.rows >= expected_loop_steps
                && scenario_tests.rows > 0
                && scenario_tests.incomplete == 0
            {
                ProductionReadinessStatus::Ready
            } else if scenario_tests.rows > 0 {
                ProductionReadinessStatus::Warning
            } else {
                ProductionReadinessStatus::Blocked
            },
            expected: expected_loop_steps,
            actual: scenario_tests.ready,
            artifact_refs: vec!["validation/scenario_test_plan.adm".to_string()],
            detail: format!(
                "scenario test rows={}, ready={}, incomplete={}",
                scenario_tests.rows, scenario_tests.ready, scenario_tests.incomplete
            ),
        },
        ProductionReadinessCheck {
            check_id: "runtime_validation_readiness".to_string(),
            status: if runtime_validation.rows >= expected_loop_steps
                && runtime_validation.rows > 0
                && runtime_validation.incomplete == 0
            {
                ProductionReadinessStatus::Ready
            } else if runtime_validation.rows > 0 {
                ProductionReadinessStatus::Warning
            } else {
                ProductionReadinessStatus::Blocked
            },
            expected: expected_loop_steps,
            actual: runtime_validation.ready,
            artifact_refs: vec!["validation/runtime_validation_report.adm".to_string()],
            detail: format!(
                "runtime validation rows={}, ready={}, incomplete={}",
                runtime_validation.rows, runtime_validation.ready, runtime_validation.incomplete
            ),
        },
        ProductionReadinessCheck {
            check_id: "validation_gate".to_string(),
            status: match input_validation.status {
                ValidationStatus::Passed => ProductionReadinessStatus::Ready,
                ValidationStatus::Warning => ProductionReadinessStatus::Warning,
                ValidationStatus::Failed => ProductionReadinessStatus::Blocked,
            },
            expected: 0,
            actual: input_validation.issues.len(),
            artifact_refs: vec!["validation/report.adm".to_string()],
            detail: "input validation must not block packaging".to_string(),
        },
    ];

    let overall_status = overall_status(&checks);
    ProductionReadinessReport {
        project_id: project.project_id.to_string(),
        title: project.working_title.clone(),
        overall_status,
        checks,
        trace_artifacts: vec![
            "project/brief.adm".to_string(),
            "design/project.adm".to_string(),
            "development/plan.adm".to_string(),
            "assets/plan.adm".to_string(),
            "sdk/index.adm".to_string(),
            "package/build_targets.adm".to_string(),
            "validation/acceptance_matrix.adm".to_string(),
            "validation/scenario_test_plan.adm".to_string(),
            "validation/runtime_validation_report.adm".to_string(),
        ],
    }
}

fn count_check(
    check_id: &str,
    expected: usize,
    actual: usize,
    artifact_refs: Vec<&str>,
    detail: &str,
) -> ProductionReadinessCheck {
    ProductionReadinessCheck {
        check_id: check_id.to_string(),
        status: if expected > 0 && actual >= expected {
            ProductionReadinessStatus::Ready
        } else if actual > 0 {
            ProductionReadinessStatus::Warning
        } else {
            ProductionReadinessStatus::Blocked
        },
        expected,
        actual,
        artifact_refs: artifact_refs.into_iter().map(ToString::to_string).collect(),
        detail: detail.to_string(),
    }
}

fn overall_status(checks: &[ProductionReadinessCheck]) -> ProductionReadinessStatus {
    if checks
        .iter()
        .any(|check| check.status == ProductionReadinessStatus::Blocked)
    {
        ProductionReadinessStatus::Blocked
    } else if checks
        .iter()
        .any(|check| check.status == ProductionReadinessStatus::Warning)
    {
        ProductionReadinessStatus::Warning
    } else {
        ProductionReadinessStatus::Ready
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct AcceptanceSummary {
    rows: usize,
    ready: usize,
    incomplete: usize,
}

fn acceptance_summary(document: &str) -> AcceptanceSummary {
    let mut rows = 0;
    let mut ready = 0;
    for line in document.lines() {
        let trimmed = line.trim();
        if !trimmed.starts_with("- ") {
            continue;
        }
        rows += 1;
        if trimmed
            .split("; ")
            .any(|part| part.trim().eq_ignore_ascii_case("status=ready"))
        {
            ready += 1;
        }
    }
    AcceptanceSummary {
        rows,
        ready,
        incomplete: rows.saturating_sub(ready),
    }
}

fn sanitize_inline(value: &str) -> String {
    value.replace(['\r', '\n', ';'], " ").trim().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use adm_design::GameDesignBrief;
    use adm_packaging::GameBuildPlan;

    fn complete_inputs() -> (
        DesignProject,
        DesignEvaluation,
        DevelopmentPlan,
        AssetPlan,
        SdkKnowledgeBase,
        GameBuildPlan,
        String,
    ) {
        let project = GameDesignBrief::new(
            "Ready Demo",
            "2D action",
            "Players read clear feedback after every major action",
            vec![
                "Scout".to_string(),
                "Act".to_string(),
                "Read feedback".to_string(),
            ],
        )
        .unwrap()
        .to_project();
        let evaluation = adm_design::evaluate_design(&project);
        let development = DevelopmentPlan::for_core_loop(&project.core_loop, Some("Unity".into()));
        let assets = AssetPlan::for_core_loop(&project.genre, &project.core_loop);
        let sdk = SdkKnowledgeBase::default_game_pipeline();
        let build_targets = GameBuildPlan::windows_desktop_prototype(project.project_id.clone());
        let acceptance = "# Acceptance Trace Matrix\n\
            - trace_id=trace_1; status=ready\n\
            - trace_id=trace_2; status=ready\n\
            - trace_id=trace_3; status=ready\n"
            .to_string();
        let scenario_tests = "# Scenario Test Plan\n\
            - test_id=test_1; status=ready\n\
            - test_id=test_2; status=ready\n\
            - test_id=test_3; status=ready\n"
            .to_string();
        let runtime_validation = "# Runtime Validation Report\n\
            - result_id=runtime_1; status=ready\n\
            - result_id=runtime_2; status=ready\n\
            - result_id=runtime_3; status=ready\n"
            .to_string();
        (
            project,
            evaluation,
            development,
            assets,
            sdk,
            build_targets,
            format!(
                "{acceptance}\n---SCENARIO_TESTS---\n{scenario_tests}\n---RUNTIME_VALIDATION---\n{runtime_validation}"
            ),
        )
    }

    #[test]
    fn production_readiness_reports_ready_for_complete_pipeline() {
        let (project, evaluation, development, assets, sdk, build_targets, validation_docs) =
            complete_inputs();
        let (acceptance, rest) = validation_docs
            .split_once("\n---SCENARIO_TESTS---\n")
            .expect("split validation docs");
        let (scenario_tests, runtime_validation) = rest
            .split_once("\n---RUNTIME_VALIDATION---\n")
            .expect("split runtime validation docs");
        let report = build_production_readiness_report(
            &project,
            &evaluation,
            &development,
            &assets,
            &sdk,
            &build_targets,
            &ValidationReport::passed(),
            acceptance,
            scenario_tests,
            runtime_validation,
        );
        let rendered = report.render();

        assert_eq!(report.overall_status, ProductionReadinessStatus::Ready);
        assert!(rendered.contains("overall_status=ready"));
        assert!(rendered.contains("check_id=acceptance_trace_readiness; status=ready"));
        assert!(rendered.contains("check_id=scenario_test_plan_readiness; status=ready"));
        assert!(rendered.contains("check_id=runtime_validation_readiness; status=ready"));
        assert!(rendered.contains("validation/acceptance_matrix.adm"));
        assert!(rendered.contains("validation/scenario_test_plan.adm"));
        assert!(rendered.contains("validation/runtime_validation_report.adm"));
    }

    #[test]
    fn production_readiness_blocks_when_acceptance_rows_are_missing() {
        let (project, evaluation, development, assets, sdk, build_targets, validation_docs) =
            complete_inputs();
        let (_, rest) = validation_docs
            .split_once("\n---SCENARIO_TESTS---\n")
            .expect("split validation docs");
        let (scenario_tests, runtime_validation) = rest
            .split_once("\n---RUNTIME_VALIDATION---\n")
            .expect("split runtime validation docs");
        let report = build_production_readiness_report(
            &project,
            &evaluation,
            &development,
            &assets,
            &sdk,
            &build_targets,
            &ValidationReport::passed(),
            "",
            scenario_tests,
            runtime_validation,
        );

        assert_eq!(report.overall_status, ProductionReadinessStatus::Blocked);
        assert!(report.checks.iter().any(|check| {
            check.check_id == "acceptance_trace_readiness"
                && check.status == ProductionReadinessStatus::Blocked
        }));
    }
}
