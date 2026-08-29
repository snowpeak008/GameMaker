use crate::production_readiness::build_production_readiness_report;
use adm_ai::{
    AiBudget, AiCapability, AiInterventionCriteria, AiOutputState, AiOutputValidator, AiProvider,
    AiProviderRouter, AiRetryPolicy, AiTaskJournal, AiTaskRecord, AiTaskRequest, AiTaskStatus,
    decide_ai_intervention,
};
use adm_assets::AssetPlan;
use adm_config::AiSettings;
use adm_design::{
    DesignEvaluation, DesignProject, GameDesignBrief, evaluate_design, render_design_document,
};
use adm_development::DevelopmentPlan;
use adm_foundation::{AdmError, AdmErrorKind, AdmResult, ArtifactId, ContentHash, RunId, StageId};
use adm_packaging::{
    GameBuildPlan, PackageManifest, validate_game_build_targets, validate_release_package,
};
use adm_pipeline::{
    ArtifactRecord, ArtifactRegistry, PipelineGraph, PipelineRunReport, PipelineRunState,
    PipelineRunner, PipelineStage, StageExecutor, StageRunResult, StageRunStatus,
};
use adm_sdk::{SdkKnowledgeBase, SdkTargetProfile};
use adm_validation::{ValidationIssue, ValidationReport, ValidationStatus};
use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct CorePipelineOutputs {
    pub brief_document: String,
    pub design_document: String,
    pub development_document: String,
    pub asset_document: String,
    pub sdk_document: String,
    pub build_targets_document: String,
    pub package_document: String,
    pub acceptance_matrix_document: String,
    pub scenario_test_plan_document: String,
    pub runtime_validation_document: String,
    pub production_readiness_document: String,
    pub validation: ValidationReport,
    pub pipeline_report: PipelineRunReport,
    pub artifact_registry: ArtifactRegistry,
    pub run_state: PipelineRunState,
    pub ai_journal: AiTaskJournal,
    pub devflow_pipeline_report: PipelineRunReport,
    pub devflow_run_state: PipelineRunState,
    pub devflow_step_documents: Vec<DevflowStepDocument>,
}

#[derive(Debug, Clone, Default)]
pub struct CorePipelineServices;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DevflowStepDocument {
    pub step_id: String,
    pub relative_path: PathBuf,
    pub content: String,
}

pub const DEVFLOW_RUN_REPORT_PATH: &str = "pipeline/devflow_run_report.adm";
pub const DEVFLOW_RUN_STATE_PATH: &str = "pipeline/devflow_run_state.adm";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DevFlowStepSpec {
    pub step_id: &'static str,
    pub group: &'static str,
    pub title: &'static str,
    pub core_stage_id: &'static str,
    pub detail: &'static str,
}

const DEVFLOW_STEP_SPECS: [DevFlowStepSpec; 15] = [
    DevFlowStepSpec {
        step_id: "step00",
        group: "设计阶段",
        title: "Step00 创意收集",
        core_stage_id: "design",
        detail: "接收游戏创意包并形成初始项目画像。",
    },
    DevFlowStepSpec {
        step_id: "step01",
        group: "设计阶段",
        title: "Step01 玩法框架",
        core_stage_id: "design",
        detail: "抽取核心循环、系统边界和玩法框架。",
    },
    DevFlowStepSpec {
        step_id: "step02",
        group: "设计阶段",
        title: "Step02 设计冻结",
        core_stage_id: "design",
        detail: "冻结设计决策并生成后续开发输入。",
    },
    DevFlowStepSpec {
        step_id: "step03",
        group: "设计阶段",
        title: "Step03 程序需求",
        core_stage_id: "development",
        detail: "从冻结设计生成程序需求契约。",
    },
    DevFlowStepSpec {
        step_id: "step04",
        group: "设计阶段",
        title: "Step04 美术需求",
        core_stage_id: "assets",
        detail: "从冻结设计生成美术与资产需求契约。",
    },
    DevFlowStepSpec {
        step_id: "step05",
        group: "设计阶段",
        title: "Step05 程序评审",
        core_stage_id: "development",
        detail: "评审程序需求完整性、可执行性和风险。",
    },
    DevFlowStepSpec {
        step_id: "step06",
        group: "设计阶段",
        title: "Step06 美术评审",
        core_stage_id: "assets",
        detail: "评审美术需求覆盖、风格一致性和生产风险。",
    },
    DevFlowStepSpec {
        step_id: "step07",
        group: "风格确认",
        title: "Step07 美术风格",
        core_stage_id: "assets",
        detail: "生成、选择、确认或重生成美术风格方向。",
    },
    DevFlowStepSpec {
        step_id: "step08",
        group: "计划阶段",
        title: "Step08 程序计划",
        core_stage_id: "development",
        detail: "把设计与程序需求转为可执行开发计划。",
    },
    DevFlowStepSpec {
        step_id: "step09",
        group: "计划阶段",
        title: "Step09 美术计划",
        core_stage_id: "assets",
        detail: "把美术需求转为资产生产计划。",
    },
    DevFlowStepSpec {
        step_id: "step10",
        group: "计划阶段",
        title: "Step10 资源对齐",
        core_stage_id: "sdk",
        detail: "对齐资产、SDK、构建目标和交付契约。",
    },
    DevFlowStepSpec {
        step_id: "step11",
        group: "执行阶段",
        title: "Step11 程序执行",
        core_stage_id: "development",
        detail: "记录程序实现任务、状态与验证证据。",
    },
    DevFlowStepSpec {
        step_id: "step12",
        group: "执行阶段",
        title: "Step12 美术生产",
        core_stage_id: "assets",
        detail: "记录美术生产任务、资产状态与验收证据。",
    },
    DevFlowStepSpec {
        step_id: "step13",
        group: "执行阶段",
        title: "Step13 场景组装",
        core_stage_id: "packaging",
        detail: "组装场景、构建目标和可运行交付结构。",
    },
    DevFlowStepSpec {
        step_id: "step14",
        group: "执行阶段",
        title: "Step14 集成验证",
        core_stage_id: "packaging",
        detail: "执行集成验证、验收矩阵和交付前检查。",
    },
];

pub fn devflow_step_specs() -> &'static [DevFlowStepSpec] {
    &DEVFLOW_STEP_SPECS
}

pub fn core_stage_id_for_devflow_step(stage_id: &str) -> Option<&'static str> {
    match stage_id {
        "design" => Some("design"),
        "development" => Some("development"),
        "assets" => Some("assets"),
        "sdk" => Some("sdk"),
        "packaging" => Some("packaging"),
        _ => DEVFLOW_STEP_SPECS
            .iter()
            .find(|step| step.step_id == stage_id)
            .map(|step| step.core_stage_id),
    }
}

pub fn devflow_step_spec(stage_id: &str) -> Option<&'static DevFlowStepSpec> {
    DEVFLOW_STEP_SPECS
        .iter()
        .find(|step| step.step_id == stage_id)
}

impl CorePipelineServices {
    pub fn new() -> Self {
        Self
    }

    pub fn build<P: AiProvider>(
        &self,
        brief: GameDesignBrief,
        ai_provider: &P,
        ai_settings: &AiSettings,
    ) -> AdmResult<CorePipelineOutputs> {
        self.build_with_state(
            brief,
            ai_provider,
            ai_settings,
            PipelineRunState::new(RunId::generate()),
        )
    }

    pub fn build_with_state<P: AiProvider>(
        &self,
        brief: GameDesignBrief,
        ai_provider: &P,
        ai_settings: &AiSettings,
        mut state: PipelineRunState,
    ) -> AdmResult<CorePipelineOutputs> {
        let graph = default_core_pipeline_graph()?;
        state.validate_for_graph(&graph)?;
        let execution_order = graph.execution_order()?;
        let mut executor = CorePipelineStageExecutor::new(brief, ai_provider, ai_settings);
        for stage in &execution_order {
            if state.is_stage_completed(&stage.id) {
                executor.prepare_completed_stage(&stage.id)?;
            }
        }
        let pipeline_report =
            PipelineRunner::new(graph).run_serial_with_state(&mut executor, &mut state)?;
        executor.into_outputs(pipeline_report, state)
    }

    pub fn rewind_state_to_stage(
        &self,
        state: &mut PipelineRunState,
        stage_id: &StageId,
    ) -> AdmResult<Vec<StageId>> {
        let graph = default_core_pipeline_graph()?;
        state.rewind_to_stage(&graph, stage_id)
    }
}

struct CorePipelineStageExecutor<'a, P: AiProvider> {
    assembly: CorePipelineAssembly,
    ai_provider: &'a P,
    ai_settings: &'a AiSettings,
}

impl<'a, P: AiProvider> CorePipelineStageExecutor<'a, P> {
    fn new(brief: GameDesignBrief, ai_provider: &'a P, ai_settings: &'a AiSettings) -> Self {
        Self {
            assembly: CorePipelineAssembly::new(brief),
            ai_provider,
            ai_settings,
        }
    }

    fn into_outputs(
        self,
        pipeline_report: PipelineRunReport,
        run_state: PipelineRunState,
    ) -> AdmResult<CorePipelineOutputs> {
        self.assembly.into_outputs(
            pipeline_report,
            run_state,
            self.ai_provider,
            self.ai_settings,
        )
    }

    fn prepare_completed_stage(&mut self, stage_id: &StageId) -> AdmResult<()> {
        match stage_id.as_str() {
            "design" => self.assembly.materialize_completed_design_stage(),
            "development" => self.assembly.materialize_development_stage().map(|_| ()),
            "assets" => self.assembly.materialize_assets_stage().map(|_| ()),
            "sdk" => self.assembly.materialize_sdk_stage().map(|_| ()),
            "packaging" => self.assembly.materialize_packaging_stage().map(|_| ()),
            other => Err(AdmError::new(
                AdmErrorKind::Internal,
                format!("unknown completed core pipeline stage: {other}"),
            )),
        }
    }
}

impl<P: AiProvider> StageExecutor for CorePipelineStageExecutor<'_, P> {
    fn execute(&mut self, stage: &PipelineStage) -> AdmResult<StageRunResult> {
        match stage.id.as_str() {
            "design" => self
                .assembly
                .execute_design_stage(self.ai_provider, self.ai_settings),
            "development" => self.assembly.execute_development_stage(),
            "assets" => self.assembly.execute_assets_stage(),
            "sdk" => self.assembly.execute_sdk_stage(),
            "packaging" => self.assembly.execute_packaging_stage(),
            other => Ok(StageRunResult {
                stage_id: stage.id.clone(),
                status: StageRunStatus::Failed,
                artifacts: Vec::new(),
                message: format!("unknown core pipeline stage: {other}"),
            }),
        }
    }
}

#[derive(Debug, Clone)]
struct CorePipelineAssembly {
    project: DesignProject,
    evaluation: DesignEvaluation,
    brief_document: String,
    design_document: Option<String>,
    development_plan: Option<DevelopmentPlan>,
    development_document: Option<String>,
    asset_plan: Option<AssetPlan>,
    asset_document: Option<String>,
    sdk: Option<SdkKnowledgeBase>,
    sdk_document: Option<String>,
    build_targets_plan: Option<GameBuildPlan>,
    build_targets_document: Option<String>,
    package_manifest: Option<PackageManifest>,
    package_document: Option<String>,
    acceptance_matrix_document: Option<String>,
    scenario_test_plan_document: Option<String>,
    runtime_validation_document: Option<String>,
    production_readiness_document: Option<String>,
    input_validation: Option<ValidationReport>,
    artifact_registry: ArtifactRegistry,
    ai_journal: Option<AiTaskJournal>,
}

impl CorePipelineAssembly {
    fn new(brief: GameDesignBrief) -> Self {
        let brief_document = render_brief_document(&brief);
        let project = brief.to_project();
        let evaluation = evaluate_design(&project);
        Self {
            project,
            evaluation,
            brief_document,
            design_document: None,
            development_plan: None,
            development_document: None,
            asset_plan: None,
            asset_document: None,
            sdk: None,
            sdk_document: None,
            build_targets_plan: None,
            build_targets_document: None,
            package_manifest: None,
            package_document: None,
            acceptance_matrix_document: None,
            scenario_test_plan_document: None,
            runtime_validation_document: None,
            production_readiness_document: None,
            input_validation: None,
            artifact_registry: ArtifactRegistry::new(),
            ai_journal: None,
        }
    }

    fn execute_design_stage<P: AiProvider>(
        &mut self,
        ai_provider: &P,
        ai_settings: &AiSettings,
    ) -> AdmResult<StageRunResult> {
        let artifact = self.materialize_design_stage()?;
        self.ai_journal = Some(build_ai_journal(
            &self.project.working_title,
            &self.evaluation,
            ai_provider,
            ai_settings,
        )?);
        Ok(success(
            StageId::new("design")?,
            vec![artifact],
            "Game Design completed",
        ))
    }

    fn execute_development_stage(&mut self) -> AdmResult<StageRunResult> {
        let artifact = self.materialize_development_stage()?;
        Ok(success(
            StageId::new("development")?,
            vec![artifact],
            "Development Plan completed",
        ))
    }

    fn execute_assets_stage(&mut self) -> AdmResult<StageRunResult> {
        let artifact = self.materialize_assets_stage()?;
        Ok(success(
            StageId::new("assets")?,
            vec![artifact],
            "Asset Plan completed",
        ))
    }

    fn execute_sdk_stage(&mut self) -> AdmResult<StageRunResult> {
        let artifact = self.materialize_sdk_stage()?;
        Ok(success(
            StageId::new("sdk")?,
            vec![artifact],
            "SDK Review completed",
        ))
    }

    fn execute_packaging_stage(&mut self) -> AdmResult<StageRunResult> {
        let artifacts = self.materialize_packaging_stage()?;
        Ok(success(
            StageId::new("packaging")?,
            artifacts,
            "Packaging completed",
        ))
    }

    fn materialize_completed_design_stage(&mut self) -> AdmResult<()> {
        self.materialize_design_stage()?;
        if self.ai_journal.is_none() {
            self.ai_journal = Some(AiTaskJournal::default());
        }
        Ok(())
    }

    fn materialize_design_stage(&mut self) -> AdmResult<ArtifactId> {
        let brief_document = self.brief_document.clone();
        self.register_text_artifact(
            "artifact_project_brief",
            "design",
            "project/brief.adm",
            &brief_document,
        )?;
        let document = render_design_document(&self.project, &self.evaluation);
        let artifact = self.register_text_artifact(
            "artifact_design_project",
            "design",
            "design/project.adm",
            &document,
        )?;
        self.design_document = Some(document);
        Ok(artifact)
    }

    fn materialize_development_stage(&mut self) -> AdmResult<ArtifactId> {
        let plan =
            DevelopmentPlan::for_core_loop(&self.project.core_loop, Some("Unity".to_string()));
        let document = plan.render();
        let artifact = self.register_text_artifact(
            "artifact_development_plan",
            "development",
            "development/plan.adm",
            &document,
        )?;
        self.development_plan = Some(plan);
        self.development_document = Some(document);
        Ok(artifact)
    }

    fn materialize_assets_stage(&mut self) -> AdmResult<ArtifactId> {
        let plan = AssetPlan::for_core_loop(&self.project.genre, &self.project.core_loop);
        let document = plan.render();
        let artifact = self.register_text_artifact(
            "artifact_asset_plan",
            "assets",
            "assets/plan.adm",
            &document,
        )?;
        self.asset_plan = Some(plan);
        self.asset_document = Some(document);
        Ok(artifact)
    }

    fn materialize_sdk_stage(&mut self) -> AdmResult<ArtifactId> {
        let sdk =
            SdkKnowledgeBase::for_target(&SdkTargetProfile::new("Unity", "windows-desktop", true));
        let document = sdk.render();
        let artifact =
            self.register_text_artifact("artifact_sdk_index", "sdk", "sdk/index.adm", &document)?;
        self.sdk = Some(sdk);
        self.sdk_document = Some(document);
        Ok(artifact)
    }

    fn materialize_packaging_stage(&mut self) -> AdmResult<Vec<ArtifactId>> {
        let development_plan = required_ref(&self.development_plan, "development plan")?;
        let asset_plan = required_ref(&self.asset_plan, "asset plan")?;
        let sdk = required_ref(&self.sdk, "sdk knowledge base")?;
        let input_validation =
            validate_pipeline_inputs(&self.project, development_plan, asset_plan, sdk);
        let build_targets_plan =
            GameBuildPlan::windows_desktop_prototype(self.project.project_id.clone());
        let build_targets_document = build_targets_plan.render();
        let acceptance_matrix_document = render_acceptance_matrix(
            &self.project,
            development_plan,
            asset_plan,
            sdk,
            &build_targets_plan,
        );
        let scenario_test_plan_document = render_scenario_test_plan(
            &self.project,
            development_plan,
            asset_plan,
            &build_targets_plan,
        );
        let runtime_validation_document = render_runtime_validation_report(
            &self.project,
            development_plan,
            asset_plan,
            &build_targets_plan,
            &acceptance_matrix_document,
        );
        let production_readiness_document = build_production_readiness_report(
            &self.project,
            &self.evaluation,
            development_plan,
            asset_plan,
            sdk,
            &build_targets_plan,
            &input_validation,
            &acceptance_matrix_document,
            &scenario_test_plan_document,
            &runtime_validation_document,
        )
        .render();
        let build_targets_artifact = self.register_text_artifact(
            "artifact_game_build_targets",
            "packaging",
            "package/build_targets.adm",
            &build_targets_document,
        )?;
        let acceptance_matrix_artifact = self.register_text_artifact(
            "artifact_acceptance_matrix",
            "packaging",
            "validation/acceptance_matrix.adm",
            &acceptance_matrix_document,
        )?;
        let scenario_test_plan_artifact = self.register_text_artifact(
            "artifact_scenario_test_plan",
            "packaging",
            "validation/scenario_test_plan.adm",
            &scenario_test_plan_document,
        )?;
        let runtime_validation_artifact = self.register_text_artifact(
            "artifact_runtime_validation_report",
            "packaging",
            "validation/runtime_validation_report.adm",
            &runtime_validation_document,
        )?;
        let production_readiness_artifact = self.register_text_artifact(
            "artifact_production_readiness",
            "packaging",
            "validation/production_readiness.adm",
            &production_readiness_document,
        )?;
        let package_manifest = PackageManifest::new(
            self.project.project_id.clone(),
            "windows-desktop".to_string(),
            vec![
                "design/project.adm".to_string(),
                "development/plan.adm".to_string(),
                "assets/plan.adm".to_string(),
                "sdk/index.adm".to_string(),
                "package/build_targets.adm".to_string(),
            ],
        )
        .with_support_files(vec![
            "project/brief.adm".to_string(),
            "package/manifest.adm".to_string(),
            "validation/report.adm".to_string(),
            "validation/acceptance_matrix.adm".to_string(),
            "validation/scenario_test_plan.adm".to_string(),
            "validation/runtime_validation_report.adm".to_string(),
            "validation/production_readiness.adm".to_string(),
            "pipeline/run_report.adm".to_string(),
            "pipeline/run_state.adm".to_string(),
            DEVFLOW_RUN_REPORT_PATH.to_string(),
            DEVFLOW_RUN_STATE_PATH.to_string(),
            "pipeline/artifact_registry.adm".to_string(),
            "ai/journal.adm".to_string(),
        ]);
        let document = package_manifest.render();
        let manifest_artifact = self.register_text_artifact(
            "artifact_package_manifest",
            "packaging",
            "package/manifest.adm",
            &document,
        )?;
        self.input_validation = Some(input_validation);
        self.build_targets_plan = Some(build_targets_plan);
        self.build_targets_document = Some(build_targets_document);
        self.package_manifest = Some(package_manifest);
        self.package_document = Some(document);
        self.acceptance_matrix_document = Some(acceptance_matrix_document);
        self.scenario_test_plan_document = Some(scenario_test_plan_document);
        self.runtime_validation_document = Some(runtime_validation_document);
        self.production_readiness_document = Some(production_readiness_document);
        Ok(vec![
            build_targets_artifact,
            acceptance_matrix_artifact,
            scenario_test_plan_artifact,
            runtime_validation_artifact,
            production_readiness_artifact,
            manifest_artifact,
        ])
    }

    fn register_text_artifact(
        &mut self,
        artifact_id: &str,
        stage_id: &str,
        relative_path: &str,
        content: &str,
    ) -> AdmResult<ArtifactId> {
        let artifact_id = ArtifactId::new(artifact_id)?;
        self.artifact_registry.register(ArtifactRecord {
            artifact_id: artifact_id.clone(),
            stage_id: StageId::new(stage_id)?,
            relative_path: PathBuf::from(relative_path),
            content_hash: ContentHash::from_bytes(content.as_bytes()),
        })?;
        Ok(artifact_id)
    }

    fn into_outputs(
        self,
        pipeline_report: PipelineRunReport,
        run_state: PipelineRunState,
        ai_provider: &impl AiProvider,
        ai_settings: &AiSettings,
    ) -> AdmResult<CorePipelineOutputs> {
        let package_manifest = required(self.package_manifest, "package manifest")?;
        let build_targets_plan = required(self.build_targets_plan, "game build target plan")?;
        let input_validation = required(self.input_validation, "input validation")?;
        let build_targets_validation =
            validate_game_build_targets(&build_targets_plan, &self.artifact_registry);
        let package_validation = validate_release_package(
            &package_manifest,
            &self.artifact_registry,
            &run_state,
            &input_validation,
        );
        let validation = merge_validation_reports(vec![
            input_validation,
            build_targets_validation,
            package_validation,
        ]);
        let mut ai_journal = required(self.ai_journal, "ai journal")?;
        append_validation_review_to_journal(
            &mut ai_journal,
            &self.project.working_title,
            &validation,
            ai_provider,
            ai_settings,
        )?;

        let design_document = required(self.design_document, "design document")?;
        let development_document = required(self.development_document, "development document")?;
        let asset_document = required(self.asset_document, "asset document")?;
        let sdk_document = required(self.sdk_document, "sdk document")?;
        let build_targets_document =
            required(self.build_targets_document, "game build target document")?;
        let package_document = required(self.package_document, "package document")?;
        let acceptance_matrix_document = required(
            self.acceptance_matrix_document,
            "acceptance matrix document",
        )?;
        let scenario_test_plan_document = required(
            self.scenario_test_plan_document,
            "scenario test plan document",
        )?;
        let runtime_validation_document = required(
            self.runtime_validation_document,
            "runtime validation document",
        )?;
        let production_readiness_document = required(
            self.production_readiness_document,
            "production readiness document",
        )?;
        let (devflow_step_documents, devflow_pipeline_report, devflow_run_state) =
            run_devflow_pipeline(DevflowStepRenderInputs {
                project: &self.project,
                design_document: &design_document,
                development_document: &development_document,
                asset_document: &asset_document,
                sdk_document: &sdk_document,
                build_targets_document: &build_targets_document,
                package_document: &package_document,
                acceptance_matrix_document: &acceptance_matrix_document,
                scenario_test_plan_document: &scenario_test_plan_document,
                runtime_validation_document: &runtime_validation_document,
                production_readiness_document: &production_readiness_document,
                validation: &validation,
                pipeline_report: &pipeline_report,
                core_state: &run_state,
            })?;
        let mut artifact_registry = self.artifact_registry;
        for document in &devflow_step_documents {
            artifact_registry.register(ArtifactRecord {
                artifact_id: ArtifactId::new(format!("artifact_{}", document.step_id))?,
                stage_id: StageId::new(document.step_id.clone())?,
                relative_path: document.relative_path.clone(),
                content_hash: ContentHash::from_bytes(document.content.as_bytes()),
            })?;
        }

        Ok(CorePipelineOutputs {
            brief_document: self.brief_document,
            design_document,
            development_document,
            asset_document,
            sdk_document,
            build_targets_document,
            package_document,
            acceptance_matrix_document,
            scenario_test_plan_document,
            runtime_validation_document,
            production_readiness_document,
            validation,
            pipeline_report,
            artifact_registry,
            run_state,
            ai_journal,
            devflow_pipeline_report,
            devflow_run_state,
            devflow_step_documents,
        })
    }
}

pub fn render_brief_document(brief: &GameDesignBrief) -> String {
    let mut document = String::new();
    document.push_str("# Game Design Brief\n");
    document.push_str(&format!("title={}\n", brief.title));
    document.push_str(&format!("genre={}\n", brief.genre));
    document.push_str(&format!("player_promise={}\n", brief.player_promise));
    document.push_str("core_loop_steps=\n");
    for step in &brief.core_loop {
        document.push_str(&format!("- {step}\n"));
    }
    document
}

struct DevflowStepRenderInputs<'a> {
    project: &'a DesignProject,
    design_document: &'a str,
    development_document: &'a str,
    asset_document: &'a str,
    sdk_document: &'a str,
    build_targets_document: &'a str,
    package_document: &'a str,
    acceptance_matrix_document: &'a str,
    scenario_test_plan_document: &'a str,
    runtime_validation_document: &'a str,
    production_readiness_document: &'a str,
    validation: &'a ValidationReport,
    pipeline_report: &'a PipelineRunReport,
    core_state: &'a PipelineRunState,
}

fn run_devflow_pipeline(
    inputs: DevflowStepRenderInputs<'_>,
) -> AdmResult<(
    Vec<DevflowStepDocument>,
    PipelineRunReport,
    PipelineRunState,
)> {
    let graph = default_devflow_pipeline_graph()?;
    let mut state = PipelineRunState::new(RunId::generate());
    let mut executor = DevflowPipelineStageExecutor {
        inputs,
        documents: Vec::new(),
    };
    let report = PipelineRunner::new(graph).run_serial_with_state(&mut executor, &mut state)?;
    Ok((executor.documents, report, state))
}

struct DevflowPipelineStageExecutor<'a> {
    inputs: DevflowStepRenderInputs<'a>,
    documents: Vec<DevflowStepDocument>,
}

impl StageExecutor for DevflowPipelineStageExecutor<'_> {
    fn execute(&mut self, stage: &PipelineStage) -> AdmResult<StageRunResult> {
        let step = devflow_step_spec(stage.id.as_str()).ok_or_else(|| {
            AdmError::validation(format!("unknown devflow step stage: {}", stage.id))
        })?;
        let document = DevflowStepDocument {
            step_id: step.step_id.to_string(),
            relative_path: PathBuf::from(format!("pipeline/{}/stage.adm", step.step_id)),
            content: render_devflow_step_document(step, &self.inputs),
        };
        let artifact_path = document.relative_path.to_string_lossy().replace('\\', "/");
        self.documents.push(document);
        let artifact_id = ArtifactId::new(format!("artifact_{}", step.step_id))?;
        let (status, core_status) = devflow_executor_core_status(step, &self.inputs);

        Ok(StageRunResult {
            stage_id: stage.id.clone(),
            status,
            artifacts: vec![artifact_id],
            message: format!(
                "{}，mode=rust_devflow_executor_v1，core_stage={}，core_status={}，artifact={}",
                step.detail, step.core_stage_id, core_status, artifact_path
            ),
        })
    }
}

fn devflow_executor_core_status(
    step: &DevFlowStepSpec,
    inputs: &DevflowStepRenderInputs<'_>,
) -> (StageRunStatus, String) {
    if let Some(core_result) = inputs
        .pipeline_report
        .results
        .iter()
        .rev()
        .find(|result| result.stage_id.as_str() == step.core_stage_id)
    {
        return (
            core_result.status.clone(),
            format!("{:?}", core_result.status),
        );
    }
    if inputs
        .core_state
        .completed_stages
        .iter()
        .any(|stage| stage.as_str() == step.core_stage_id)
    {
        return (
            StageRunStatus::Succeeded,
            "completed_from_core_state".to_string(),
        );
    }
    (
        StageRunStatus::Failed,
        "not_completed_in_core_state".to_string(),
    )
}

fn render_devflow_step_document(
    step: &DevFlowStepSpec,
    inputs: &DevflowStepRenderInputs<'_>,
) -> String {
    let source_documents = source_documents_for_devflow_step(step.step_id);
    let representative_excerpt = representative_excerpt_for_devflow_step(step.step_id, inputs);
    let contract_output = render_devflow_step_contract_output(step.step_id, inputs);
    let structured_content = render_devflow_structured_stage_content(step.step_id, inputs);
    let acceptance_checklist = render_devflow_acceptance_checklist(step.step_id, inputs);
    let downstream_inputs = render_devflow_downstream_inputs(step.step_id, inputs);
    let mut document = format!(
        "# {}\nstep_id={}\ngroup={}\ncore_stage_id={}\nexecution_mode=rust_devflow_executor_v1\nstatus=generated_by_rust_step_executor\nproject_id={}\nproject_title={}\nvalidation_status={:?}\npipeline_status={:?}\nsource_documents={}\n\n## Step Contract\n{}\n\n## Rust Native Contract Output\n{}\n\n## Structured Stage Content\n{}\n\n## Acceptance Checklist\n{}\n\n## Downstream Inputs\n{}\n\n## Representative Output\n{}\n",
        step.title,
        step.step_id,
        step.group,
        step.core_stage_id,
        inputs.project.project_id,
        sanitize_trace_value(&inputs.project.working_title),
        inputs.validation.status,
        inputs.pipeline_report.status(),
        source_documents.join(" | "),
        step.detail,
        contract_output,
        structured_content,
        acceptance_checklist,
        downstream_inputs,
        representative_excerpt
    );
    document.push_str("\n## Core Loop\n");
    document.push_str(&format!(
        "genre={}\ncore_loop={}\nscenario_count={}\n",
        sanitize_trace_value(&inputs.project.genre),
        sanitize_trace_value(&inputs.project.core_loop.join(" | ")),
        inputs.project.playable_scenarios.len()
    ));
    document
}

fn render_devflow_structured_stage_content(
    step_id: &str,
    inputs: &DevflowStepRenderInputs<'_>,
) -> String {
    let project = inputs.project;
    match step_id {
        "step00" => {
            let platform = find_document_value(inputs.build_targets_document, "platform")
                .unwrap_or_else(|| "windows-desktop".to_string());
            let profile = find_document_value(inputs.build_targets_document, "profile")
                .unwrap_or_else(|| "playable-prototype".to_string());
            format!(
                "### Project Profile\nworking_title={}\ngenre={}\nproject_id={}\n\n### Creative Input Summary\ncore_loop_step_count={}\ndesign_pillar_count={}\nplayable_scenario_count={}\n{}\n\n### Platform And Business Model\nplatform={}\nprototype_profile={}\nbusiness_model=human_approved_design_document_pipeline\n\n### Core Experience Promise\npromise={}\n",
                sanitize_trace_value(&project.working_title),
                sanitize_trace_value(&project.genre),
                project.project_id,
                project.core_loop.len(),
                project.design_pillars.len(),
                project.playable_scenarios.len(),
                render_core_loop_items(project, 6),
                sanitize_trace_value(&platform),
                sanitize_trace_value(&profile),
                sanitize_trace_value(&project.player_promise)
            )
        }
        "step01" => format!(
            "### Core Loop\ncore_loop_step_count={}\n{}\n\n### Gameplay Systems\nmechanic_count={}\n{}\n\n### Player Actions\n{}\n\n### Feedback Structure\n{}\n\n### System Boundaries\nstate_source=design/project.adm\nscenario_count={}\nvalidation_boundary=static_contract_probe_without_unity_playmode\n",
            project.core_loop.len(),
            render_core_loop_items(project, 8),
            project.gameplay_mechanics.len(),
            render_mechanic_summaries(project, 8),
            render_player_action_summaries(project, 8),
            render_feedback_summaries(project, 8),
            project.playable_scenarios.len()
        ),
        "step02" => format!(
            "### Frozen Decisions\nfreeze_status={:?}\nworking_title={}\ngenre={}\nplayer_promise={}\n{}\n\n### Open Questions\n{}\n\n### Risk List\nrisk_count={}\n{}\n\n### Development Inputs\nprogram_input=design/project.adm\nart_input=design/project.adm\nvalidation_status={:?}\n",
            inputs.validation.status,
            sanitize_trace_value(&project.working_title),
            sanitize_trace_value(&project.genre),
            sanitize_trace_value(&project.player_promise),
            render_design_pillar_summaries(project, 6),
            render_open_question_summary(inputs.validation),
            project.acceptance_risks.len(),
            render_risk_summaries(project, 8),
            inputs.validation.status
        ),
        "step03" => format!(
            "### Program Capabilities\nrequirement_task_count={}\ndata_contract_rows={}\ntest_rows={}\ntelemetry_rows={}\n\n### System Requirements\n{}\n\n### Task List\n{}\n\n### Acceptance Probes\n{}\n\n### Dependency Source\nsource=design/project.adm\ncore_stage=development\n",
            count_prefixed_lines(inputs.development_document, "- "),
            count_lines_containing(inputs.development_document, "data_contracts="),
            count_lines_containing(inputs.development_document, "tests="),
            count_lines_containing(inputs.development_document, "telemetry="),
            first_lines_containing(inputs.development_document, "data_contracts=", 5),
            first_prefixed_lines(inputs.development_document, "- ", 5),
            render_scenario_probe_summaries(project, 6)
        ),
        "step04" => format!(
            "### Asset Categories\nasset_task_count={}\nmechanic_feedback_rows={}\nsource_mechanic_rows={}\n\n### Visual Requirements\n{}\n\n### Production Risks\n{}\n\n### Acceptance Scope\n{}\n\n### Dependency Source\nsource=design/project.adm | development/plan.adm\ncore_stage=assets\n",
            count_prefixed_lines(inputs.asset_document, "- "),
            count_lines_containing(inputs.asset_document, "stage=mechanic_feedback"),
            count_lines_containing(inputs.asset_document, "source_mechanic="),
            first_lines_containing(inputs.asset_document, "kind=", 6),
            first_lines_containing(inputs.asset_document, "risk_controls=", 6),
            first_lines_containing(inputs.asset_document, "acceptance=", 6)
        ),
        "step05" => format!(
            "### Program Requirement Review\nreview_status={:?}\nchecked_task_rows={}\ndata_contract_rows={}\ntest_rows={}\n\n### Blocking Items\nblocking_issue_count={}\n{}\n\n### Warning Items\nwarning_issue_count={}\n{}\n\n### Correction Suggestions\n{}\n",
            inputs.validation.status,
            count_prefixed_lines(inputs.development_document, "- "),
            count_lines_containing(inputs.development_document, "data_contracts="),
            count_lines_containing(inputs.development_document, "tests="),
            validation_issue_count(inputs.validation, ValidationStatus::Failed),
            render_validation_issue_summaries(inputs.validation, ValidationStatus::Failed),
            validation_issue_count(inputs.validation, ValidationStatus::Warning),
            render_validation_issue_summaries(inputs.validation, ValidationStatus::Warning),
            render_review_correction_summary(inputs.validation, "program_requirements_ready")
        ),
        "step06" => format!(
            "### Art Requirement Review\nreview_status={:?}\nchecked_asset_rows={}\nmechanic_feedback_rows={}\ncoverage_rows={}\n\n### Style Consistency Risks\n{}\n\n### Missing Asset Items\n{}\n\n### Correction Suggestions\n{}\n",
            inputs.validation.status,
            count_prefixed_lines(inputs.asset_document, "- "),
            count_lines_containing(inputs.asset_document, "stage=mechanic_feedback"),
            count_lines_containing(inputs.asset_document, "source_mechanic="),
            render_validation_issue_summaries(inputs.validation, ValidationStatus::Warning),
            missing_art_item_summary(inputs),
            render_review_correction_summary(inputs.validation, "art_requirements_ready")
        ),
        "step07" => format!(
            "### Style Direction Candidates\nstyle_source=assets/plan.adm\nstyle_candidate_rows={}\nvisual_style_rows={}\n{}\n\n### Confirmation State\nconfirmation_required=true\nconfirmation_record=pipeline/step07_style_confirmation.adm\nregeneration_path=edit_prompt_and_rerun_step07\n\n### Style Acceptance Basis\n{}\n",
            count_lines_containing(inputs.asset_document, "stage="),
            count_lines_containing(inputs.asset_document, "kind=visual_style"),
            first_lines_containing(inputs.asset_document, "kind=visual_style", 4),
            first_lines_containing(inputs.asset_document, "validation=", 5)
        ),
        "step08" => format!(
            "### Program Execution Plan\nplan_rows={}\nvalidation_test_rows={}\ntelemetry_rows={}\n\n### Milestones\n{}\n\n### Validation Plan\n{}\n\n### Risk Controls\n{}\n",
            count_prefixed_lines(inputs.development_document, "- "),
            count_lines_containing(inputs.development_document, "tests="),
            count_lines_containing(inputs.development_document, "telemetry="),
            first_lines_containing(inputs.development_document, "milestone=", 6),
            first_lines_containing(inputs.development_document, "validation=", 6),
            first_lines_containing(inputs.development_document, "risk_controls=", 6)
        ),
        "step09" => format!(
            "### Art Production Plan\nasset_plan_rows={}\nfeedback_asset_rows={}\nvalidation_rows={}\n\n### Production Tasks\n{}\n\n### Acceptance Scope\n{}\n\n### Risk Controls\n{}\n",
            count_prefixed_lines(inputs.asset_document, "- "),
            count_lines_containing(inputs.asset_document, "mechanic_feedback"),
            count_lines_containing(inputs.asset_document, "validation="),
            first_prefixed_lines(inputs.asset_document, "- ", 6),
            first_lines_containing(inputs.asset_document, "acceptance=", 6),
            first_lines_containing(inputs.asset_document, "risk_controls=", 6)
        ),
        "step10" => format!(
            "### Asset And SDK Alignment\nsdk_resource_rows={}\nbuild_target_rows={}\nrequired_for_build_rows={}\n\n### SDK Resources\n{}\n\n### Build Targets\n{}\n\n### Required Package Inputs\n{}\n",
            count_prefixed_lines(inputs.sdk_document, "- "),
            count_lines_containing(inputs.build_targets_document, "target_id="),
            count_lines_containing(inputs.sdk_document, "required_for_build=true"),
            first_prefixed_lines(inputs.sdk_document, "- ", 6),
            first_lines_containing(inputs.build_targets_document, "target_id=", 4),
            first_prefixed_lines(inputs.build_targets_document, "- ", 10)
        ),
        "step11" => format!(
            "### Program Execution Record\nimplemented_task_rows={}\nruntime_probe_rows={}\ntelemetry_rows={}\n\n### Implemented Tasks\n{}\n\n### Runtime Probes\n{}\n\n### Execution Evidence\nsource=development/plan.adm | validation/runtime_validation_report.adm\n",
            count_prefixed_lines(inputs.development_document, "- "),
            count_lines_containing(inputs.runtime_validation_document, "result_id="),
            count_lines_containing(inputs.development_document, "telemetry="),
            first_prefixed_lines(inputs.development_document, "- ", 6),
            first_lines_containing(inputs.runtime_validation_document, "result_id=", 6)
        ),
        "step12" => format!(
            "### Art Production Record\nproduced_asset_rows={}\nasset_validation_rows={}\nmechanic_feedback_rows={}\n\n### Produced Assets\n{}\n\n### Validation Evidence\n{}\n\n### Production Risk Controls\n{}\n",
            count_prefixed_lines(inputs.asset_document, "- "),
            count_lines_containing(inputs.asset_document, "validation="),
            count_lines_containing(inputs.asset_document, "stage=mechanic_feedback"),
            first_prefixed_lines(inputs.asset_document, "- ", 6),
            first_lines_containing(inputs.asset_document, "validation=", 6),
            first_lines_containing(inputs.asset_document, "risk_controls=", 6)
        ),
        "step13" => format!(
            "### Scene Assembly\nbuild_target_rows={}\npackage_entry_rows={}\npackage_support_rows={}\n\n### Build Targets\n{}\n\n### Package Entries\n{}\n\n### Support Files\n{}\n",
            count_lines_containing(inputs.build_targets_document, "target_id="),
            count_manifest_section_items(inputs.package_document, "entries="),
            count_manifest_section_items(inputs.package_document, "support_files="),
            first_lines_containing(inputs.build_targets_document, "target_id=", 4),
            manifest_section_excerpt(inputs.package_document, "entries=", 8),
            manifest_section_excerpt(inputs.package_document, "support_files=", 10)
        ),
        "step14" => format!(
            "### Integration Validation\nacceptance_trace_rows={}\nscenario_test_rows={}\nruntime_result_rows={}\nproduction_readiness={}\n\n### Acceptance Matrix\n{}\n\n### Scenario Tests\n{}\n\n### Runtime Results\n{}\n\n### Production Readiness\n{}\n",
            count_lines_containing(inputs.acceptance_matrix_document, "trace_id="),
            count_lines_containing(inputs.scenario_test_plan_document, "test_id="),
            count_lines_containing(inputs.runtime_validation_document, "result_id="),
            find_document_value(inputs.production_readiness_document, "overall_status")
                .unwrap_or_else(|| "unknown".to_string()),
            first_lines_containing(inputs.acceptance_matrix_document, "trace_id=", 6),
            first_lines_containing(inputs.scenario_test_plan_document, "test_id=", 6),
            first_lines_containing(inputs.runtime_validation_document, "result_id=", 6),
            excerpt_lines(inputs.production_readiness_document, 8)
        ),
        _ => render_generic_structured_stage_content(step_id, inputs),
    }
}

fn render_devflow_acceptance_checklist(
    step_id: &str,
    inputs: &DevflowStepRenderInputs<'_>,
) -> String {
    let project = inputs.project;
    let checklist = match step_id {
        "step00" => vec![
            (
                "working_title_present",
                !project.working_title.trim().is_empty(),
            ),
            ("genre_present", !project.genre.trim().is_empty()),
            (
                "player_promise_present",
                !project.player_promise.trim().is_empty(),
            ),
            ("core_loop_present", !project.core_loop.is_empty()),
        ],
        "step01" => vec![
            ("core_loop_has_steps", !project.core_loop.is_empty()),
            (
                "gameplay_mechanics_derived",
                !project.gameplay_mechanics.is_empty(),
            ),
            (
                "playable_scenarios_derived",
                !project.playable_scenarios.is_empty(),
            ),
            (
                "feedback_structure_present",
                project
                    .gameplay_mechanics
                    .iter()
                    .any(|mechanic| !mechanic.feedback.trim().is_empty()),
            ),
        ],
        "step02" => vec![
            ("design_pillars_present", !project.design_pillars.is_empty()),
            (
                "acceptance_risks_present",
                !project.acceptance_risks.is_empty(),
            ),
            (
                "freeze_not_failed",
                inputs.validation.status != ValidationStatus::Failed,
            ),
            ("downstream_sources_declared", true),
        ],
        "step03" => vec![
            (
                "program_tasks_present",
                count_prefixed_lines(inputs.development_document, "- ") > 0,
            ),
            (
                "data_contracts_present",
                count_lines_containing(inputs.development_document, "data_contracts=") > 0,
            ),
            (
                "tests_present",
                count_lines_containing(inputs.development_document, "tests=") > 0,
            ),
            (
                "telemetry_present",
                count_lines_containing(inputs.development_document, "telemetry=") > 0,
            ),
        ],
        "step04" => vec![
            (
                "asset_tasks_present",
                count_prefixed_lines(inputs.asset_document, "- ") > 0,
            ),
            (
                "mechanic_feedback_present",
                count_lines_containing(inputs.asset_document, "stage=mechanic_feedback") > 0,
            ),
            (
                "asset_acceptance_present",
                count_lines_containing(inputs.asset_document, "acceptance=") > 0,
            ),
            (
                "asset_validation_present",
                count_lines_containing(inputs.asset_document, "validation=") > 0,
            ),
        ],
        "step05" => vec![
            (
                "program_review_has_subject",
                count_prefixed_lines(inputs.development_document, "- ") > 0,
            ),
            (
                "program_review_not_failed",
                inputs.validation.status != ValidationStatus::Failed,
            ),
            (
                "program_review_reports_blockers",
                validation_issue_count(inputs.validation, ValidationStatus::Failed)
                    == count_lines_containing(inputs.development_document, "status=blocked"),
            ),
            ("program_corrections_declared", true),
        ],
        "step06" => vec![
            (
                "art_review_has_subject",
                count_prefixed_lines(inputs.asset_document, "- ") > 0,
            ),
            (
                "art_review_not_failed",
                inputs.validation.status != ValidationStatus::Failed,
            ),
            (
                "mechanic_coverage_present",
                count_lines_containing(inputs.asset_document, "source_mechanic=") > 0,
            ),
            ("art_corrections_declared", true),
        ],
        "step07" => vec![
            (
                "style_candidates_present",
                count_lines_containing(inputs.asset_document, "kind=visual_style") > 0,
            ),
            ("style_confirmation_path_declared", true),
            (
                "style_validation_present",
                count_lines_containing(inputs.asset_document, "validation=") > 0,
            ),
            ("style_regeneration_path_declared", true),
        ],
        "step08" => vec![
            (
                "program_plan_rows_present",
                count_prefixed_lines(inputs.development_document, "- ") > 0,
            ),
            (
                "program_validation_tests_present",
                count_lines_containing(inputs.development_document, "tests=") > 0,
            ),
            (
                "program_risk_controls_present",
                count_lines_containing(inputs.development_document, "risk_controls=") > 0,
            ),
            ("program_plan_downstream_declared", true),
        ],
        "step09" => vec![
            (
                "art_plan_rows_present",
                count_prefixed_lines(inputs.asset_document, "- ") > 0,
            ),
            (
                "feedback_asset_rows_present",
                count_lines_containing(inputs.asset_document, "mechanic_feedback") > 0,
            ),
            (
                "art_acceptance_present",
                count_lines_containing(inputs.asset_document, "acceptance=") > 0,
            ),
            ("art_plan_downstream_declared", true),
        ],
        "step10" => vec![
            (
                "sdk_resources_present",
                count_prefixed_lines(inputs.sdk_document, "- ") > 0,
            ),
            (
                "build_targets_present",
                count_lines_containing(inputs.build_targets_document, "target_id=") > 0,
            ),
            (
                "build_required_sdk_present",
                count_lines_containing(inputs.sdk_document, "required_for_build=true") > 0,
            ),
            ("alignment_downstream_declared", true),
        ],
        "step11" => vec![
            (
                "implemented_program_rows_present",
                count_prefixed_lines(inputs.development_document, "- ") > 0,
            ),
            (
                "runtime_probe_rows_present",
                count_lines_containing(inputs.runtime_validation_document, "result_id=") > 0,
            ),
            (
                "telemetry_rows_present",
                count_lines_containing(inputs.development_document, "telemetry=") > 0,
            ),
            ("program_execution_evidence_declared", true),
        ],
        "step12" => vec![
            (
                "produced_asset_rows_present",
                count_prefixed_lines(inputs.asset_document, "- ") > 0,
            ),
            (
                "asset_validation_rows_present",
                count_lines_containing(inputs.asset_document, "validation=") > 0,
            ),
            (
                "mechanic_feedback_assets_present",
                count_lines_containing(inputs.asset_document, "stage=mechanic_feedback") > 0,
            ),
            ("art_execution_evidence_declared", true),
        ],
        "step13" => vec![
            (
                "scene_build_targets_present",
                count_lines_containing(inputs.build_targets_document, "target_id=") > 0,
            ),
            (
                "package_entries_present",
                count_manifest_section_items(inputs.package_document, "entries=") > 0,
            ),
            (
                "package_support_files_present",
                count_manifest_section_items(inputs.package_document, "support_files=") > 0,
            ),
            ("scene_assembly_downstream_declared", true),
        ],
        "step14" => vec![
            (
                "acceptance_trace_rows_present",
                count_lines_containing(inputs.acceptance_matrix_document, "trace_id=") > 0,
            ),
            (
                "scenario_test_rows_present",
                count_lines_containing(inputs.scenario_test_plan_document, "test_id=") > 0,
            ),
            (
                "runtime_result_rows_present",
                count_lines_containing(inputs.runtime_validation_document, "result_id=") > 0,
            ),
            (
                "production_readiness_present",
                find_document_value(inputs.production_readiness_document, "overall_status")
                    .is_some(),
            ),
        ],
        _ => vec![
            ("contract_kind_declared", true),
            ("source_documents_declared", true),
            ("representative_output_present", true),
        ],
    };
    render_checklist_rows(checklist)
}

fn render_devflow_downstream_inputs(step_id: &str, inputs: &DevflowStepRenderInputs<'_>) -> String {
    match step_id {
        "step00" => format!(
            "handoff_to=step01\nprovided_artifacts=project/brief.adm | design/project.adm\nrequired_fields=working_title | genre | player_promise | core_loop\ncore_loop_steps={}\n",
            inputs.project.core_loop.len()
        ),
        "step01" => format!(
            "handoff_to=step02\nprovided_artifacts=design/project.adm\nrequired_fields=core_loop | gameplay_mechanics | playable_scenarios | feedback_structure\nmechanic_count={}\nscenario_count={}\n",
            inputs.project.gameplay_mechanics.len(),
            inputs.project.playable_scenarios.len()
        ),
        "step02" => format!(
            "handoff_to=step03 | step04\nprovided_artifacts=design/project.adm\nrequired_fields=frozen_pillars | frozen_risks | playable_scenarios\nfreeze_status={:?}\n",
            inputs.validation.status
        ),
        "step03" => format!(
            "handoff_to=step05 | step08\nprovided_artifacts=development/plan.adm\nrequired_fields=program_tasks | data_contracts | tests | telemetry\nprogram_task_rows={}\n",
            count_prefixed_lines(inputs.development_document, "- ")
        ),
        "step04" => format!(
            "handoff_to=step06 | step07 | step09\nprovided_artifacts=assets/plan.adm\nrequired_fields=asset_tasks | validation | risk_controls | acceptance\nasset_task_rows={}\n",
            count_prefixed_lines(inputs.asset_document, "- ")
        ),
        "step05" => format!(
            "handoff_to=step08\nprovided_artifacts=development/plan.adm | pipeline/step05/stage.adm\nrequired_fields=review_status | blocking_items | correction_suggestions\nreview_status={:?}\n",
            inputs.validation.status
        ),
        "step06" => format!(
            "handoff_to=step07 | step09\nprovided_artifacts=assets/plan.adm | pipeline/step06/stage.adm\nrequired_fields=review_status | style_risks | missing_asset_items\nreview_status={:?}\n",
            inputs.validation.status
        ),
        "step07" => format!(
            "handoff_to=step09 | step10\nprovided_artifacts=assets/plan.adm | pipeline/step07/stage.adm\nrequired_fields=style_candidates | confirmation_record | regeneration_prompt\nstyle_candidate_rows={}\n",
            count_lines_containing(inputs.asset_document, "stage=")
        ),
        "step08" => format!(
            "handoff_to=step10 | step11\nprovided_artifacts=development/plan.adm | pipeline/step08/stage.adm\nrequired_fields=program_plan_rows | validation_tests | risk_controls\nprogram_plan_rows={}\n",
            count_prefixed_lines(inputs.development_document, "- ")
        ),
        "step09" => format!(
            "handoff_to=step10 | step12\nprovided_artifacts=assets/plan.adm | pipeline/step09/stage.adm\nrequired_fields=asset_plan_rows | feedback_assets | art_acceptance\nasset_plan_rows={}\n",
            count_prefixed_lines(inputs.asset_document, "- ")
        ),
        "step10" => format!(
            "handoff_to=step11 | step12 | step13\nprovided_artifacts=sdk/index.adm | package/build_targets.adm | pipeline/step10/stage.adm\nrequired_fields=sdk_resources | build_targets | required_artifacts\nbuild_target_rows={}\n",
            count_lines_containing(inputs.build_targets_document, "target_id=")
        ),
        "step11" => format!(
            "handoff_to=step13 | step14\nprovided_artifacts=development/plan.adm | validation/runtime_validation_report.adm | pipeline/step11/stage.adm\nrequired_fields=implemented_tasks | runtime_probes | telemetry\nruntime_result_rows={}\n",
            count_lines_containing(inputs.runtime_validation_document, "result_id=")
        ),
        "step12" => format!(
            "handoff_to=step13 | step14\nprovided_artifacts=assets/plan.adm | pipeline/step12/stage.adm\nrequired_fields=produced_assets | asset_validation | feedback_assets\nproduced_asset_rows={}\n",
            count_prefixed_lines(inputs.asset_document, "- ")
        ),
        "step13" => format!(
            "handoff_to=step14\nprovided_artifacts=package/manifest.adm | package/build_targets.adm | pipeline/step13/stage.adm\nrequired_fields=build_targets | package_entries | support_files\npackage_support_rows={}\n",
            count_manifest_section_items(inputs.package_document, "support_files=")
        ),
        "step14" => format!(
            "handoff_to=packaging\nprovided_artifacts=validation/acceptance_matrix.adm | validation/scenario_test_plan.adm | validation/runtime_validation_report.adm | validation/production_readiness.adm | pipeline/step14/stage.adm\nrequired_fields=acceptance_traces | scenario_tests | runtime_results | production_readiness\nproduction_readiness={}\n",
            find_document_value(inputs.production_readiness_document, "overall_status")
                .unwrap_or_else(|| "unknown".to_string())
        ),
        _ => {
            let next_step = next_devflow_step_id(step_id).unwrap_or("none");
            let source_documents = source_documents_for_devflow_step(step_id);
            format!(
                "handoff_to={}\nprovided_artifacts=pipeline/{}/stage.adm\nsource_documents={}\n",
                next_step,
                step_id,
                source_documents.join(" | ")
            )
        }
    }
}

fn render_devflow_step_contract_output(
    step_id: &str,
    inputs: &DevflowStepRenderInputs<'_>,
) -> String {
    match step_id {
        "step00" => format!(
            "contract_kind=idea_intake\nworking_title={}\ngenre={}\nplayer_promise={}\ncore_loop_count={}\n",
            sanitize_trace_value(&inputs.project.working_title),
            sanitize_trace_value(&inputs.project.genre),
            sanitize_trace_value(&inputs.project.player_promise),
            inputs.project.core_loop.len()
        ),
        "step01" => format!(
            "contract_kind=gameplay_framework\nmechanic_count={}\nscenario_count={}\ncore_loop={}\n",
            inputs.project.gameplay_mechanics.len(),
            inputs.project.playable_scenarios.len(),
            sanitize_trace_value(&inputs.project.core_loop.join(" | "))
        ),
        "step02" => format!(
            "contract_kind=design_freeze\nfreeze_status={:?}\ndesign_pillars={}\nacceptance_risks={}\n",
            inputs.validation.status,
            inputs.project.design_pillars.len(),
            inputs.project.acceptance_risks.len()
        ),
        "step03" => format!(
            "contract_kind=program_requirements_contract\ndevelopment_task_count={}\ndata_contract_rows={}\ntest_rows={}\n",
            count_prefixed_lines(inputs.development_document, "- "),
            count_lines_containing(inputs.development_document, "data_contracts="),
            count_lines_containing(inputs.development_document, "tests=")
        ),
        "step04" => format!(
            "contract_kind=art_requirements_contract\nasset_task_count={}\nmechanic_feedback_rows={}\nsource_mechanic_rows={}\n",
            count_prefixed_lines(inputs.asset_document, "- "),
            count_lines_containing(inputs.asset_document, "stage=mechanic_feedback"),
            count_lines_containing(inputs.asset_document, "source_mechanic=")
        ),
        "step05" => format!(
            "contract_kind=program_review\nreview_status={:?}\nchecked_task_rows={}\nblocking_issue_count={}\n",
            inputs.validation.status,
            count_prefixed_lines(inputs.development_document, "- "),
            count_lines_containing(inputs.development_document, "status=blocked")
        ),
        "step06" => format!(
            "contract_kind=art_review\nreview_status={:?}\nchecked_asset_rows={}\ncoverage_rows={}\n",
            inputs.validation.status,
            count_prefixed_lines(inputs.asset_document, "- "),
            count_lines_containing(inputs.asset_document, "source_mechanic=")
        ),
        "step07" => format!(
            "contract_kind=art_style_confirmation\nstyle_confirmation_required=true\nstyle_source=assets/plan.adm\nstyle_candidate_rows={}\n",
            count_lines_containing(inputs.asset_document, "stage=")
        ),
        "step08" => format!(
            "contract_kind=program_execution_plan\nplan_rows={}\nvalidation_test_rows={}\n",
            count_prefixed_lines(inputs.development_document, "- "),
            count_lines_containing(inputs.development_document, "tests=")
        ),
        "step09" => format!(
            "contract_kind=art_production_plan\nasset_plan_rows={}\nfeedback_asset_rows={}\n",
            count_prefixed_lines(inputs.asset_document, "- "),
            count_lines_containing(inputs.asset_document, "mechanic_feedback")
        ),
        "step10" => format!(
            "contract_kind=asset_alignment\nsdk_resource_rows={}\nbuild_target_rows={}\nrequired_for_build_rows={}\n",
            count_prefixed_lines(inputs.sdk_document, "- "),
            count_lines_containing(inputs.build_targets_document, "target_id="),
            count_lines_containing(inputs.sdk_document, "required_for_build=true")
        ),
        "step11" => format!(
            "contract_kind=program_execution_record\nimplemented_task_rows={}\nruntime_probe_rows={}\n",
            count_prefixed_lines(inputs.development_document, "- "),
            count_lines_containing(inputs.runtime_validation_document, "result_id=")
        ),
        "step12" => format!(
            "contract_kind=art_production_record\nproduced_asset_rows={}\nasset_validation_rows={}\n",
            count_prefixed_lines(inputs.asset_document, "- "),
            count_lines_containing(inputs.asset_document, "status=")
        ),
        "step13" => format!(
            "contract_kind=scene_assembly\nbuild_target_rows={}\npackage_entry_rows={}\npackage_support_rows={}\n",
            count_lines_containing(inputs.build_targets_document, "target_id="),
            count_manifest_section_items(inputs.package_document, "entries="),
            count_manifest_section_items(inputs.package_document, "support_files=")
        ),
        "step14" => format!(
            "contract_kind=integration_validation\nacceptance_trace_rows={}\nscenario_test_rows={}\nruntime_result_rows={}\nproduction_readiness={}\n",
            count_lines_containing(inputs.acceptance_matrix_document, "trace_id="),
            count_lines_containing(inputs.scenario_test_plan_document, "test_id="),
            count_lines_containing(inputs.runtime_validation_document, "result_id="),
            find_document_value(inputs.production_readiness_document, "overall_status")
                .unwrap_or_else(|| "unknown".to_string())
        ),
        _ => "contract_kind=unknown\n".to_string(),
    }
}

fn source_documents_for_devflow_step(step_id: &str) -> Vec<&'static str> {
    match step_id {
        "step00" => vec!["project/brief.adm", "design/project.adm"],
        "step01" | "step02" => vec!["design/project.adm"],
        "step03" | "step05" | "step08" | "step11" => {
            vec!["design/project.adm", "development/plan.adm"]
        }
        "step04" | "step06" | "step07" | "step09" | "step12" => {
            vec!["design/project.adm", "assets/plan.adm"]
        }
        "step10" => vec![
            "development/plan.adm",
            "assets/plan.adm",
            "sdk/index.adm",
            "package/build_targets.adm",
        ],
        "step13" => vec!["package/build_targets.adm", "package/manifest.adm"],
        "step14" => vec![
            "validation/acceptance_matrix.adm",
            "validation/scenario_test_plan.adm",
            "validation/runtime_validation_report.adm",
            "validation/production_readiness.adm",
        ],
        _ => vec!["pipeline/run_report.adm"],
    }
}

fn representative_excerpt_for_devflow_step(
    step_id: &str,
    inputs: &DevflowStepRenderInputs<'_>,
) -> String {
    let source = match step_id {
        "step00" | "step01" | "step02" => inputs.design_document,
        "step03" | "step05" | "step08" | "step11" => inputs.development_document,
        "step04" | "step06" | "step07" | "step09" | "step12" => inputs.asset_document,
        "step10" => inputs.sdk_document,
        "step13" => inputs.build_targets_document,
        "step14" => {
            return format!(
                "{}\n{}\n{}\n{}",
                excerpt_lines(inputs.acceptance_matrix_document, 5),
                excerpt_lines(inputs.scenario_test_plan_document, 5),
                excerpt_lines(inputs.runtime_validation_document, 5),
                excerpt_lines(inputs.production_readiness_document, 5)
            );
        }
        _ => "",
    };
    excerpt_lines(source, 12)
}

fn excerpt_lines(text: &str, limit: usize) -> String {
    text.lines()
        .take(limit)
        .map(sanitize_trace_value)
        .collect::<Vec<_>>()
        .join("\n")
}

fn count_prefixed_lines(text: &str, prefix: &str) -> usize {
    text.lines()
        .filter(|line| line.trim_start().starts_with(prefix))
        .count()
}

fn count_lines_containing(text: &str, pattern: &str) -> usize {
    text.lines().filter(|line| line.contains(pattern)).count()
}

fn count_manifest_section_items(text: &str, section_header: &str) -> usize {
    let mut in_section = false;
    let mut count = 0;
    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed == section_header {
            in_section = true;
            continue;
        }
        if in_section && trimmed.ends_with('=') {
            break;
        }
        if in_section && trimmed.starts_with("- ") {
            count += 1;
        }
    }
    count
}

fn manifest_section_excerpt(text: &str, section_header: &str, limit: usize) -> String {
    let mut in_section = false;
    let mut rows = Vec::new();
    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed == section_header {
            in_section = true;
            continue;
        }
        if in_section && trimmed.ends_with('=') {
            break;
        }
        if in_section && trimmed.starts_with("- ") {
            rows.push(sanitize_trace_value(trimmed));
            if rows.len() >= limit {
                break;
            }
        }
    }
    non_empty_rows(rows)
}

fn find_document_value(text: &str, key: &str) -> Option<String> {
    let prefix = format!("{key}=");
    text.lines()
        .find_map(|line| line.trim().strip_prefix(&prefix).map(str::to_string))
}

fn render_core_loop_items(project: &DesignProject, limit: usize) -> String {
    let rows = project
        .core_loop
        .iter()
        .enumerate()
        .take(limit)
        .map(|(index, step)| {
            format!(
                "- core_loop_step_{}={}",
                index + 1,
                sanitize_trace_value(step)
            )
        })
        .collect::<Vec<_>>();
    non_empty_rows(rows)
}

fn render_mechanic_summaries(project: &DesignProject, limit: usize) -> String {
    let rows = project
        .gameplay_mechanics
        .iter()
        .take(limit)
        .map(|mechanic| {
            format!(
                "- mechanic={}; action={}; feedback={}",
                sanitize_trace_value(&mechanic.name),
                sanitize_trace_value(&mechanic.player_action),
                sanitize_trace_value(&mechanic.feedback)
            )
        })
        .collect::<Vec<_>>();
    non_empty_rows(rows)
}

fn render_player_action_summaries(project: &DesignProject, limit: usize) -> String {
    let rows = project
        .gameplay_mechanics
        .iter()
        .take(limit)
        .map(|mechanic| {
            format!(
                "- {} => {}",
                sanitize_trace_value(&mechanic.name),
                sanitize_trace_value(&mechanic.player_action)
            )
        })
        .collect::<Vec<_>>();
    non_empty_rows(rows)
}

fn render_feedback_summaries(project: &DesignProject, limit: usize) -> String {
    let rows = project
        .gameplay_mechanics
        .iter()
        .take(limit)
        .map(|mechanic| {
            format!(
                "- {} => {}",
                sanitize_trace_value(&mechanic.name),
                sanitize_trace_value(&mechanic.feedback)
            )
        })
        .collect::<Vec<_>>();
    non_empty_rows(rows)
}

fn render_design_pillar_summaries(project: &DesignProject, limit: usize) -> String {
    let rows = project
        .design_pillars
        .iter()
        .take(limit)
        .map(|pillar| {
            format!(
                "- pillar={}; rationale={}",
                sanitize_trace_value(&pillar.name),
                sanitize_trace_value(&pillar.rationale)
            )
        })
        .collect::<Vec<_>>();
    non_empty_rows(rows)
}

fn render_risk_summaries(project: &DesignProject, limit: usize) -> String {
    let rows = project
        .acceptance_risks
        .iter()
        .take(limit)
        .map(|risk| {
            format!(
                "- risk={}; mitigation={}",
                sanitize_trace_value(&risk.risk),
                sanitize_trace_value(&risk.mitigation)
            )
        })
        .collect::<Vec<_>>();
    non_empty_rows(rows)
}

fn render_scenario_probe_summaries(project: &DesignProject, limit: usize) -> String {
    let rows = project
        .playable_scenarios
        .iter()
        .take(limit)
        .map(|scenario| {
            format!(
                "- scenario_id={}; validation_probe={}; success_state={}",
                sanitize_trace_value(&scenario.scenario_id),
                sanitize_trace_value(&scenario.validation_probe),
                sanitize_trace_value(&scenario.success_state)
            )
        })
        .collect::<Vec<_>>();
    non_empty_rows(rows)
}

fn render_open_question_summary(report: &ValidationReport) -> String {
    if report.issues.is_empty() {
        "- none=validation_passed_without_open_questions".to_string()
    } else {
        non_empty_rows(
            report
                .issues
                .iter()
                .take(8)
                .map(|issue| {
                    format!(
                        "- status={:?}; code={}; question={}",
                        issue.status,
                        sanitize_trace_value(&issue.code),
                        sanitize_trace_value(&issue.message)
                    )
                })
                .collect::<Vec<_>>(),
        )
    }
}

fn first_lines_containing(text: &str, pattern: &str, limit: usize) -> String {
    non_empty_rows(
        text.lines()
            .filter(|line| line.contains(pattern))
            .take(limit)
            .map(sanitize_trace_value)
            .collect::<Vec<_>>(),
    )
}

fn first_prefixed_lines(text: &str, prefix: &str, limit: usize) -> String {
    non_empty_rows(
        text.lines()
            .filter(|line| line.trim_start().starts_with(prefix))
            .take(limit)
            .map(sanitize_trace_value)
            .collect::<Vec<_>>(),
    )
}

fn validation_issue_count(report: &ValidationReport, status: ValidationStatus) -> usize {
    report
        .issues
        .iter()
        .filter(|issue| issue.status == status)
        .count()
}

fn render_validation_issue_summaries(
    report: &ValidationReport,
    status: ValidationStatus,
) -> String {
    non_empty_rows(
        report
            .issues
            .iter()
            .filter(|issue| issue.status == status)
            .take(8)
            .map(|issue| {
                format!(
                    "- code={}; message={}",
                    sanitize_trace_value(&issue.code),
                    sanitize_trace_value(&issue.message)
                )
            })
            .collect::<Vec<_>>(),
    )
}

fn render_review_correction_summary(report: &ValidationReport, ready_message: &str) -> String {
    if report.issues.is_empty() {
        format!("- action=none; status={ready_message}")
    } else {
        non_empty_rows(
            report
                .issues
                .iter()
                .take(8)
                .map(|issue| {
                    format!(
                        "- action=resolve; code={}; status={:?}; note={}",
                        sanitize_trace_value(&issue.code),
                        issue.status,
                        sanitize_trace_value(&issue.message)
                    )
                })
                .collect::<Vec<_>>(),
        )
    }
}

fn missing_art_item_summary(inputs: &DevflowStepRenderInputs<'_>) -> String {
    let expected = inputs.project.core_loop.len();
    let actual = count_lines_containing(inputs.asset_document, "stage=mechanic_feedback");
    if actual >= expected {
        "- none=all_core_loop_feedback_assets_declared".to_string()
    } else {
        format!(
            "- missing_feedback_asset_rows={}; expected={}; actual={}",
            expected - actual,
            expected,
            actual
        )
    }
}

fn render_generic_structured_stage_content(
    step_id: &str,
    inputs: &DevflowStepRenderInputs<'_>,
) -> String {
    let source_documents = source_documents_for_devflow_step(step_id);
    format!(
        "### Stage Summary\nstep_id={}\nsource_documents={}\nrepresentative_line_count={}\n\n### Representative Content\n{}\n",
        sanitize_trace_value(step_id),
        source_documents.join(" | "),
        representative_excerpt_for_devflow_step(step_id, inputs)
            .lines()
            .count(),
        representative_excerpt_for_devflow_step(step_id, inputs)
    )
}

fn render_checklist_rows(rows: Vec<(&'static str, bool)>) -> String {
    rows.into_iter()
        .map(|(label, passed)| format!("- [{}] {}", if passed { "x" } else { " " }, label))
        .collect::<Vec<_>>()
        .join("\n")
}

fn next_devflow_step_id(step_id: &str) -> Option<&'static str> {
    let specs = devflow_step_specs();
    specs
        .iter()
        .position(|step| step.step_id == step_id)
        .and_then(|index| specs.get(index + 1))
        .map(|step| step.step_id)
}

fn non_empty_rows(rows: Vec<String>) -> String {
    if rows.is_empty() {
        "- none".to_string()
    } else {
        rows.join("\n")
    }
}

fn default_devflow_pipeline_graph() -> AdmResult<PipelineGraph> {
    let mut stages = Vec::new();
    let mut previous_stage_id = None;
    for step in devflow_step_specs() {
        let stage_id = StageId::new(step.step_id)?;
        let mut stage = PipelineStage::new(stage_id.clone(), step.title)?;
        if let Some(previous) = previous_stage_id {
            stage = stage.depends_on(previous);
        }
        previous_stage_id = Some(stage_id);
        stages.push(stage);
    }
    PipelineGraph::new(stages)
}

fn default_core_pipeline_graph() -> AdmResult<PipelineGraph> {
    PipelineGraph::new(vec![
        PipelineStage::new(StageId::new("design")?, "Game Design")?,
        PipelineStage::new(StageId::new("development")?, "Development Plan")?
            .depends_on(StageId::new("design")?),
        PipelineStage::new(StageId::new("assets")?, "Asset Plan")?
            .depends_on(StageId::new("design")?),
        PipelineStage::new(StageId::new("sdk")?, "SDK Review")?
            .depends_on(StageId::new("development")?),
        PipelineStage::new(StageId::new("packaging")?, "Packaging")?
            .depends_on(StageId::new("development")?)
            .depends_on(StageId::new("assets")?)
            .depends_on(StageId::new("sdk")?),
    ])
}

fn render_acceptance_matrix(
    project: &DesignProject,
    development_plan: &DevelopmentPlan,
    asset_plan: &AssetPlan,
    sdk: &SdkKnowledgeBase,
    build_targets_plan: &GameBuildPlan,
) -> String {
    let build_targets = build_targets_plan
        .targets
        .iter()
        .map(|target| target.target_id.as_str())
        .collect::<Vec<_>>()
        .join(" | ");
    let build_sdk_resources = sdk
        .resources
        .iter()
        .filter(|resource| resource.required_for_build)
        .map(|resource| resource.sdk_name.as_str())
        .collect::<Vec<_>>();
    let sdk_resources = if build_sdk_resources.is_empty() {
        sdk.resources
            .iter()
            .map(|resource| resource.sdk_name.as_str())
            .collect::<Vec<_>>()
            .join(" | ")
    } else {
        build_sdk_resources.join(" | ")
    };
    let mut document = String::from("# Acceptance Trace Matrix\n");
    document.push_str(&format!("project_id={}\n", project.project_id));
    document.push_str(&format!("title={}\n", project.working_title));
    document.push_str(&format!("rows={}\n", development_plan.tasks.len()));
    document.push_str("artifacts=design/project.adm | development/plan.adm | assets/plan.adm | sdk/index.adm | package/build_targets.adm\n");
    for (index, task) in development_plan.tasks.iter().enumerate() {
        let scenario = project
            .playable_scenarios
            .iter()
            .find(|scenario| scenario.scenario_id == task.scenario_id);
        let asset = asset_plan
            .tasks
            .iter()
            .find(|asset| {
                asset.source_mechanic == task.source_mechanic
                    && asset.pipeline_stage == "mechanic_feedback"
            })
            .or_else(|| {
                asset_plan
                    .tasks
                    .iter()
                    .find(|asset| asset.source_mechanic == task.source_mechanic)
            });
        let validation_probe = scenario
            .map(|scenario| scenario.validation_probe.as_str())
            .unwrap_or("missing_validation_probe");
        let asset_task_id = asset
            .map(|asset| asset.task_id.to_string())
            .unwrap_or_else(|| "missing".to_string());
        let status = if scenario.is_some()
            && asset.is_some()
            && !sdk_resources.is_empty()
            && !build_targets.is_empty()
        {
            "ready"
        } else {
            "incomplete"
        };
        document.push_str(&format!(
            "- trace_id=trace_core_loop_step_{}; scenario_id={}; source_mechanic={}; development_task_id={}; asset_task_id={}; sdk_resources={}; build_targets={}; validation_probe={}; status={}\n",
            index + 1,
            task.scenario_id,
            task.source_mechanic,
            task.task_id,
            asset_task_id,
            sdk_resources,
            build_targets,
            validation_probe,
            status
        ));
    }
    document
}

fn render_scenario_test_plan(
    project: &DesignProject,
    development_plan: &DevelopmentPlan,
    asset_plan: &AssetPlan,
    build_targets_plan: &GameBuildPlan,
) -> String {
    let build_targets = build_targets_plan
        .targets
        .iter()
        .map(|target| target.target_id.as_str())
        .collect::<Vec<_>>()
        .join(" | ");
    let mut document = String::from("# Scenario Test Plan\n");
    document.push_str(&format!("project_id={}\n", project.project_id));
    document.push_str(&format!(
        "title={}\n",
        sanitize_trace_value(&project.working_title)
    ));
    document.push_str(&format!("scenarios={}\n", project.playable_scenarios.len()));
    document.push_str("artifacts=design/project.adm | development/plan.adm | assets/plan.adm | validation/acceptance_matrix.adm | package/build_targets.adm\n");
    for scenario in &project.playable_scenarios {
        let task = development_plan
            .tasks
            .iter()
            .find(|task| task.scenario_id == scenario.scenario_id);
        let source_mechanic = task
            .map(|task| task.source_mechanic.as_str())
            .unwrap_or("missing_mechanic");
        let asset = asset_plan
            .tasks
            .iter()
            .find(|asset| {
                asset.source_mechanic == source_mechanic
                    && asset.pipeline_stage == "mechanic_feedback"
            })
            .or_else(|| {
                asset_plan
                    .tasks
                    .iter()
                    .find(|asset| asset.source_mechanic == source_mechanic)
            });
        let telemetry = task
            .map(|task| task.telemetry_events.join(" | "))
            .unwrap_or_else(|| "missing_telemetry".to_string());
        let status = if task.is_some() && asset.is_some() && !build_targets.is_empty() {
            "ready"
        } else {
            "incomplete"
        };
        document.push_str(&format!(
            "- test_id=test_{}; scenario_id={}; source_mechanic={}; development_task_id={}; asset_task_id={}; test_type=playable_smoke; setup={}; steps={}; expected_success={}; expected_failure={}; validation_probe={}; telemetry={}; build_targets={}; status={}\n",
            sanitize_identifier(&scenario.scenario_id),
            sanitize_trace_value(&scenario.scenario_id),
            sanitize_trace_value(source_mechanic),
            task.map(|task| task.task_id.to_string()).unwrap_or_else(|| "missing".to_string()),
            asset.map(|asset| asset.task_id.to_string()).unwrap_or_else(|| "missing".to_string()),
            sanitize_trace_value(&scenario.entry_condition),
            sanitize_trace_value(&scenario.critical_path.join(" | ")),
            sanitize_trace_value(&scenario.success_state),
            sanitize_trace_value(&scenario.failure_state),
            sanitize_trace_value(&scenario.validation_probe),
            sanitize_trace_value(&telemetry),
            sanitize_trace_value(&build_targets),
            status
        ));
    }
    document
}

fn render_runtime_validation_report(
    project: &DesignProject,
    development_plan: &DevelopmentPlan,
    asset_plan: &AssetPlan,
    build_targets_plan: &GameBuildPlan,
    acceptance_matrix_document: &str,
) -> String {
    let build_targets = build_targets_plan
        .targets
        .iter()
        .map(|target| target.target_id.as_str())
        .collect::<Vec<_>>()
        .join(" | ");
    let mut document = String::from("# Runtime Validation Report\n");
    document.push_str(&format!("project_id={}\n", project.project_id));
    document.push_str(&format!(
        "title={}\n",
        sanitize_trace_value(&project.working_title)
    ));
    document.push_str("execution_mode=deterministic_runtime_probe\n");
    document.push_str(&format!("rows={}\n", project.playable_scenarios.len()));
    document.push_str("artifacts=validation/acceptance_matrix.adm | validation/scenario_test_plan.adm | development/plan.adm | assets/plan.adm | package/build_targets.adm\n");
    for scenario in &project.playable_scenarios {
        let task = development_plan
            .tasks
            .iter()
            .find(|task| task.scenario_id == scenario.scenario_id);
        let source_mechanic = task
            .map(|task| task.source_mechanic.as_str())
            .unwrap_or("missing_mechanic");
        let asset = asset_plan
            .tasks
            .iter()
            .find(|asset| {
                asset.source_mechanic == source_mechanic
                    && asset.pipeline_stage == "mechanic_feedback"
            })
            .or_else(|| {
                asset_plan
                    .tasks
                    .iter()
                    .find(|asset| asset.source_mechanic == source_mechanic)
            });
        let telemetry_events = task
            .map(|task| task.telemetry_events.clone())
            .unwrap_or_default();
        let telemetry_start = telemetry_events
            .first()
            .map(String::as_str)
            .unwrap_or("missing_telemetry_start");
        let telemetry_complete = telemetry_events
            .last()
            .map(String::as_str)
            .unwrap_or("missing_telemetry_complete");
        let acceptance_trace =
            find_acceptance_trace_id(acceptance_matrix_document, &scenario.scenario_id)
                .unwrap_or_else(|| "missing_acceptance_trace".to_string());
        let status = if task.is_some()
            && asset.is_some()
            && !build_targets.is_empty()
            && acceptance_trace != "missing_acceptance_trace"
            && telemetry_start != "missing_telemetry_start"
            && telemetry_complete != "missing_telemetry_complete"
        {
            "ready"
        } else {
            "incomplete"
        };
        document.push_str(&format!(
            "- result_id=runtime_{}; scenario_id={}; test_id=test_{}; acceptance_trace_id={}; validation_probe={}; telemetry_start={}; telemetry_complete={}; expected_runtime_state={}; failure_guard={}; build_targets={}; evidence=static_runtime_contract; status={}\n",
            sanitize_identifier(&scenario.scenario_id),
            sanitize_trace_value(&scenario.scenario_id),
            sanitize_identifier(&scenario.scenario_id),
            sanitize_trace_value(&acceptance_trace),
            sanitize_trace_value(&scenario.validation_probe),
            sanitize_trace_value(telemetry_start),
            sanitize_trace_value(telemetry_complete),
            sanitize_trace_value(&scenario.success_state),
            sanitize_trace_value(&scenario.failure_state),
            sanitize_trace_value(&build_targets),
            status
        ));
    }
    document
}

fn find_acceptance_trace_id(document: &str, scenario_id: &str) -> Option<String> {
    for line in document.lines() {
        let trimmed = line.trim();
        let Some(entry) = trimmed.strip_prefix("- ") else {
            continue;
        };
        let mut trace_id = None;
        let mut scenario_matches = false;
        for part in entry.split(';') {
            let Some((key, value)) = part.trim().split_once('=') else {
                continue;
            };
            match key.trim() {
                "trace_id" => trace_id = Some(value.trim().to_string()),
                "scenario_id" => scenario_matches = value.trim() == scenario_id,
                _ => {}
            }
        }
        if scenario_matches {
            return trace_id;
        }
    }
    None
}

fn sanitize_identifier(value: &str) -> String {
    value
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || ch == '_' || ch == '-' {
                ch
            } else {
                '_'
            }
        })
        .collect::<String>()
        .trim_matches('_')
        .to_string()
}

fn sanitize_trace_value(value: &str) -> String {
    value.replace(['\r', '\n', ';'], " ").trim().to_string()
}

fn build_ai_journal<P: AiProvider>(
    title: &str,
    evaluation: &DesignEvaluation,
    ai_provider: &P,
    ai_settings: &AiSettings,
) -> AdmResult<AiTaskJournal> {
    let mut journal = AiTaskJournal::default();
    let criteria = AiInterventionCriteria::new(
        ai_settings.intervention_policy.min_quality_score_percent,
        ai_settings.intervention_policy.on_quality_gap,
        ai_settings.intervention_policy.on_missing_content,
        ai_settings.intervention_policy.review_after_generation,
    )?;
    let decision = decide_ai_intervention(
        score_to_percent(evaluation.score.value),
        &evaluation.missing_topics,
        &criteria,
    )?;
    if !decision.required {
        return Ok(journal);
    }
    let request = AiTaskRequest::new(
        decision.capability.clone(),
        format!(
            "补全游戏设计：{}，缺失项：{}",
            title,
            evaluation.missing_topics.join(",")
        ),
        decision.reason_summary(),
    )?;
    let mut record = run_ai_request(request, ai_provider, ai_settings)?;
    validate_ai_record_output(&mut record)?;
    journal.push(record);
    Ok(journal)
}

fn append_validation_review_to_journal<P: AiProvider>(
    journal: &mut AiTaskJournal,
    title: &str,
    validation: &ValidationReport,
    ai_provider: &P,
    ai_settings: &AiSettings,
) -> AdmResult<()> {
    if validation.status == ValidationStatus::Passed
        || !ai_settings.intervention_policy.review_after_generation
    {
        return Ok(());
    }
    let issue_summary = validation
        .issues
        .iter()
        .map(|issue| format!("{:?}:{}:{}", issue.status, issue.code, issue.message))
        .collect::<Vec<_>>()
        .join(" | ");
    let request = AiTaskRequest::new(
        AiCapability::TextGeneration,
        format!(
            "Review pipeline validation issues for {title} and propose concrete fixes. Issues: {issue_summary}"
        ),
        format!(
            "validation={:?}; issue_count={}",
            validation.status,
            validation.issues.len()
        ),
    )?;
    let mut record = run_ai_request(request, ai_provider, ai_settings)?;
    validate_ai_record_output(&mut record)?;
    journal.push(record);
    Ok(())
}

fn run_ai_request<P: AiProvider>(
    request: AiTaskRequest,
    ai_provider: &P,
    ai_settings: &AiSettings,
) -> AdmResult<AiTaskRecord> {
    let router = AiProviderRouter::new(vec![ai_provider]);
    let mut budget = AiBudget::new(ai_settings.default_budget_units);
    let retry_policy = AiRetryPolicy::new(ai_settings.retry_policy.max_attempts)?;
    Ok(router.run_with_budget_and_policy(request, &mut budget, 1, retry_policy))
}

fn validate_ai_record_output(record: &mut AiTaskRecord) -> AdmResult<()> {
    if let Some(result) = record.result.take() {
        let validated = result.validate(&AiOutputValidator::strict_default());
        record.status = match validated.output_state {
            AiOutputState::Validated => AiTaskStatus::Accepted,
            AiOutputState::Rejected => AiTaskStatus::Rejected,
            _ => AiTaskStatus::Completed,
        };
        record.result = Some(if record.status == AiTaskStatus::Accepted {
            validated.accept()?
        } else {
            validated
        });
    }
    Ok(())
}

fn score_to_percent(value: f32) -> u8 {
    (value.clamp(0.0, 1.0) * 100.0).round() as u8
}

fn validate_pipeline_inputs(
    project: &DesignProject,
    development_plan: &DevelopmentPlan,
    asset_plan: &AssetPlan,
    sdk: &SdkKnowledgeBase,
) -> ValidationReport {
    let mut issues = Vec::new();
    if development_plan.tasks.is_empty() {
        issues.push(ValidationIssue {
            status: ValidationStatus::Failed,
            code: "development.empty".to_string(),
            message: "development plan has no tasks".to_string(),
        });
    }
    if asset_plan.tasks.is_empty() {
        issues.push(ValidationIssue {
            status: ValidationStatus::Failed,
            code: "assets.empty".to_string(),
            message: "asset plan has no tasks".to_string(),
        });
    }
    for task in &development_plan.tasks {
        if !project
            .playable_scenarios
            .iter()
            .any(|scenario| scenario.scenario_id == task.scenario_id)
        {
            issues.push(ValidationIssue {
                status: ValidationStatus::Failed,
                code: "design.scenario.missing".to_string(),
                message: format!(
                    "development task {} references missing scenario {}",
                    task.task_id, task.scenario_id
                ),
            });
        }
        if !asset_plan
            .tasks
            .iter()
            .any(|asset| asset.source_mechanic == task.source_mechanic)
        {
            issues.push(ValidationIssue {
                status: ValidationStatus::Failed,
                code: "assets.mechanic_feedback.missing".to_string(),
                message: format!(
                    "asset plan has no feedback coverage for {}",
                    task.source_mechanic
                ),
            });
        }
    }
    if sdk.resources.is_empty() {
        issues.push(ValidationIssue {
            status: ValidationStatus::Warning,
            code: "sdk.empty".to_string(),
            message: "sdk knowledge base has no resources".to_string(),
        });
    }
    merge_validation_reports(vec![
        ValidationReport::from_issues(issues),
        sdk.validate_for_target(&SdkTargetProfile::new("Unity", "windows-desktop", true)),
    ])
}

fn merge_validation_reports(reports: Vec<ValidationReport>) -> ValidationReport {
    ValidationReport::from_issues(
        reports
            .into_iter()
            .flat_map(|report| report.issues)
            .collect(),
    )
}

fn success(
    stage_id: StageId,
    artifacts: Vec<ArtifactId>,
    message: impl Into<String>,
) -> StageRunResult {
    StageRunResult {
        stage_id,
        status: StageRunStatus::Succeeded,
        artifacts,
        message: message.into(),
    }
}

fn required<T>(value: Option<T>, name: &str) -> AdmResult<T> {
    value.ok_or_else(|| {
        AdmError::new(
            AdmErrorKind::Internal,
            format!("core pipeline missing {name}"),
        )
    })
}

fn required_ref<'a, T>(value: &'a Option<T>, name: &str) -> AdmResult<&'a T> {
    value.as_ref().ok_or_else(|| {
        AdmError::new(
            AdmErrorKind::Internal,
            format!("core pipeline missing {name}"),
        )
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use adm_ai::{AiCapability, MockAiProvider};
    use adm_foundation::ProviderId;

    #[test]
    fn validation_review_does_not_run_for_passed_validation() {
        let provider = MockAiProvider::new(
            ProviderId::new("mock").unwrap(),
            vec![AiCapability::TextGeneration],
        );
        let mut journal = AiTaskJournal::default();

        append_validation_review_to_journal(
            &mut journal,
            "Passed Demo",
            &ValidationReport::passed(),
            &provider,
            &AiSettings::default(),
        )
        .expect("validation review");

        assert!(journal.records().is_empty());
    }

    #[test]
    fn validation_review_appends_ai_task_for_failed_validation() {
        let provider = MockAiProvider::new(
            ProviderId::new("mock").unwrap(),
            vec![AiCapability::TextGeneration],
        );
        let mut journal = AiTaskJournal::default();
        let validation = ValidationReport::from_issues(vec![ValidationIssue {
            status: ValidationStatus::Failed,
            code: "game_build.required_artifact.missing".to_string(),
            message: "missing build input".to_string(),
        }]);

        append_validation_review_to_journal(
            &mut journal,
            "Failed Demo",
            &validation,
            &provider,
            &AiSettings::default(),
        )
        .expect("validation review");

        assert_eq!(journal.records().len(), 1);
        let record = &journal.records()[0];
        assert_eq!(record.status, AiTaskStatus::Accepted);
        assert!(
            record
                .request
                .prompt
                .contains("game_build.required_artifact.missing")
        );
    }

    #[test]
    fn core_pipeline_builds_outputs_from_stage_executor() {
        let provider = MockAiProvider::new(
            ProviderId::new("mock").unwrap(),
            vec![AiCapability::TextGeneration],
        );
        let outputs = CorePipelineServices::new()
            .build(
                GameDesignBrief::new(
                    "Stage Executor Demo",
                    "2D action",
                    "玩家通过探索、战斗和反馈形成稳定成长目标",
                    vec![
                        "探索关卡".to_string(),
                        "解决战斗".to_string(),
                        "获得反馈".to_string(),
                    ],
                )
                .unwrap(),
                &provider,
                &AiSettings::default(),
            )
            .expect("outputs");

        assert!(outputs.design_document.contains("Stage Executor Demo"));
        assert!(outputs.design_document.contains("## Playable Scenarios"));
        assert!(
            outputs
                .design_document
                .contains("scenario_core_loop_step_1")
        );
        assert!(
            outputs
                .design_document
                .contains("scenario_core_loop_step_3")
        );
        assert!(
            outputs
                .development_document
                .contains("data_contracts=core_loop_step_1.request")
        );
        assert!(
            outputs
                .development_document
                .contains("tests=test_core_loop_step_1_state_delta")
        );
        assert!(
            outputs
                .asset_document
                .contains("source_mechanic=Core Loop Mechanic 3")
        );
        assert!(outputs.asset_document.contains("stage=mechanic_feedback"));
        assert!(
            outputs
                .sdk_document
                .contains("## Unity Build Automation SDK")
        );
        assert!(
            outputs
                .sdk_document
                .contains("target_platforms=windows-desktop")
        );
        assert!(outputs.sdk_document.contains("required_for_build=true"));
        assert_eq!(outputs.pipeline_report.results.len(), 5);
        assert_eq!(outputs.artifact_registry.records().len(), 26);
        assert_eq!(
            outputs
                .artifact_registry
                .by_stage(&StageId::new("packaging").unwrap())
                .len(),
            6
        );
        assert_eq!(
            outputs
                .artifact_registry
                .by_stage(&StageId::new("step14").unwrap())
                .len(),
            1
        );
        assert_eq!(outputs.devflow_step_documents.len(), 15);
        assert_eq!(outputs.devflow_pipeline_report.results.len(), 15);
        assert_eq!(outputs.devflow_run_state.completed_stages.len(), 15);
        for step_id in [
            "step00", "step01", "step02", "step03", "step04", "step05", "step06", "step07",
            "step08", "step09", "step10", "step11", "step12", "step13", "step14",
        ] {
            let document = outputs
                .devflow_step_documents
                .iter()
                .find(|document| document.step_id == step_id)
                .expect("devflow document");
            for section in [
                "## Step Contract",
                "## Rust Native Contract Output",
                "## Structured Stage Content",
                "## Acceptance Checklist",
                "## Downstream Inputs",
            ] {
                assert!(
                    document.content.contains(section),
                    "{step_id} missing {section}"
                );
            }
        }
        let step00 = outputs
            .devflow_step_documents
            .iter()
            .find(|document| document.step_id == "step00")
            .expect("step00");
        assert!(step00.content.contains("### Project Profile"));
        assert!(
            step00
                .content
                .contains("business_model=human_approved_design_document_pipeline")
        );
        assert!(step00.content.contains("handoff_to=step01"));
        let step01 = outputs
            .devflow_step_documents
            .iter()
            .find(|document| document.step_id == "step01")
            .expect("step01");
        assert!(step01.content.contains("### Gameplay Systems"));
        assert!(step01.content.contains("### Feedback Structure"));
        assert!(step01.content.contains("handoff_to=step02"));
        let step02 = outputs
            .devflow_step_documents
            .iter()
            .find(|document| document.step_id == "step02")
            .expect("step02");
        assert!(step02.content.contains("### Frozen Decisions"));
        assert!(step02.content.contains("handoff_to=step03 | step04"));
        let step03 = outputs
            .devflow_step_documents
            .iter()
            .find(|document| document.step_id == "step03")
            .expect("step03");
        assert!(step03.content.contains("### Program Capabilities"));
        assert!(
            step03
                .content
                .contains("data_contracts=core_loop_step_1.request")
        );
        assert!(step03.content.contains("handoff_to=step05 | step08"));
        let step04 = outputs
            .devflow_step_documents
            .iter()
            .find(|document| document.step_id == "step04")
            .expect("step04");
        assert!(step04.content.contains("### Asset Categories"));
        assert!(step04.content.contains("stage=mechanic_feedback"));
        assert!(
            step04
                .content
                .contains("handoff_to=step06 | step07 | step09")
        );
        let step05 = outputs
            .devflow_step_documents
            .iter()
            .find(|document| document.step_id == "step05")
            .expect("step05");
        assert!(step05.content.contains("### Program Requirement Review"));
        assert!(step05.content.contains("program_requirements_ready"));
        let step06 = outputs
            .devflow_step_documents
            .iter()
            .find(|document| document.step_id == "step06")
            .expect("step06");
        assert!(step06.content.contains("### Art Requirement Review"));
        assert!(
            step06
                .content
                .contains("all_core_loop_feedback_assets_declared")
        );
        let step07 = outputs
            .devflow_step_documents
            .iter()
            .find(|document| document.step_id == "step07")
            .expect("step07");
        assert!(step07.content.contains("### Style Direction Candidates"));
        assert!(
            step07
                .content
                .contains("confirmation_record=pipeline/step07_style_confirmation.adm")
        );
        assert!(step07.content.contains("handoff_to=step09 | step10"));
        let step08 = outputs
            .devflow_step_documents
            .iter()
            .find(|document| document.step_id == "step08")
            .expect("step08");
        assert!(step08.content.contains("### Program Execution Plan"));
        assert!(step08.content.contains("validation_test_rows="));
        assert!(step08.content.contains("handoff_to=step10 | step11"));
        let step09 = outputs
            .devflow_step_documents
            .iter()
            .find(|document| document.step_id == "step09")
            .expect("step09");
        assert!(step09.content.contains("### Art Production Plan"));
        assert!(step09.content.contains("feedback_asset_rows="));
        assert!(step09.content.contains("handoff_to=step10 | step12"));
        let step10 = outputs
            .devflow_step_documents
            .iter()
            .find(|document| document.step_id == "step10")
            .expect("step10");
        assert!(step10.content.contains("### Asset And SDK Alignment"));
        assert!(
            step10
                .content
                .contains("target_id=windows_desktop_playable")
        );
        assert!(
            step10
                .content
                .contains("handoff_to=step11 | step12 | step13")
        );
        let step11 = outputs
            .devflow_step_documents
            .iter()
            .find(|document| document.step_id == "step11")
            .expect("step11");
        assert!(step11.content.contains("### Program Execution Record"));
        assert!(step11.content.contains("runtime_probe_rows="));
        assert!(step11.content.contains("handoff_to=step13 | step14"));
        let step12 = outputs
            .devflow_step_documents
            .iter()
            .find(|document| document.step_id == "step12")
            .expect("step12");
        assert!(step12.content.contains("### Art Production Record"));
        assert!(step12.content.contains("mechanic_feedback_assets_present"));
        assert!(step12.content.contains("handoff_to=step13 | step14"));
        let step13 = outputs
            .devflow_step_documents
            .iter()
            .find(|document| document.step_id == "step13")
            .expect("step13");
        assert!(step13.content.contains("### Scene Assembly"));
        assert!(step13.content.contains("package_support_rows="));
        assert!(step13.content.contains("handoff_to=step14"));
        let step14 = outputs
            .devflow_step_documents
            .iter()
            .find(|document| document.step_id == "step14")
            .expect("step14");
        assert!(step14.content.contains("### Integration Validation"));
        assert!(step14.content.contains("production_readiness=ready"));
        assert!(step14.content.contains("handoff_to=packaging"));
        assert_eq!(
            outputs.devflow_pipeline_report.results[7].stage_id,
            StageId::new("step07").unwrap()
        );
        assert!(
            outputs.devflow_pipeline_report.results[7]
                .message
                .contains("mode=rust_devflow_executor_v1")
        );
        assert!(
            outputs.devflow_pipeline_report.results[14]
                .artifacts
                .iter()
                .any(|artifact| artifact.as_str() == "artifact_step14")
        );
        assert!(outputs.devflow_step_documents.iter().any(|document| {
            document
                .relative_path
                .ends_with("pipeline/step14/stage.adm")
                && document.content.contains("Step14 集成验证")
                && document
                    .content
                    .contains("execution_mode=rust_devflow_executor_v1")
                && document
                    .content
                    .contains("contract_kind=integration_validation")
        }));
        assert!(outputs.devflow_step_documents.iter().any(|document| {
            document.step_id == "step03"
                && document
                    .content
                    .contains("contract_kind=program_requirements_contract")
        }));
        assert!(outputs.devflow_step_documents.iter().any(|document| {
            document.step_id == "step07"
                && document
                    .content
                    .contains("contract_kind=art_style_confirmation")
        }));
        assert!(
            outputs
                .build_targets_document
                .contains("target_id=windows_desktop_playable")
        );
        assert!(
            outputs
                .package_document
                .contains("package/build_targets.adm")
        );
        assert!(
            outputs
                .package_document
                .contains("validation/acceptance_matrix.adm")
        );
        assert!(
            outputs
                .package_document
                .contains("validation/scenario_test_plan.adm")
        );
        assert!(
            outputs
                .package_document
                .contains("validation/runtime_validation_report.adm")
        );
        assert!(
            outputs
                .package_document
                .contains("validation/production_readiness.adm")
        );
        assert!(
            outputs
                .acceptance_matrix_document
                .contains("# Acceptance Trace Matrix")
        );
        assert!(
            outputs
                .acceptance_matrix_document
                .contains("source_mechanic=Core Loop Mechanic 1")
        );
        assert!(outputs.acceptance_matrix_document.contains("status=ready"));
        assert!(
            outputs
                .acceptance_matrix_document
                .contains("validation_probe=probe_core_loop_step_1_input_state_feedback")
        );
        assert!(
            outputs
                .scenario_test_plan_document
                .contains("# Scenario Test Plan")
        );
        assert!(
            outputs
                .scenario_test_plan_document
                .contains("test_id=test_scenario_core_loop_step_1")
        );
        assert!(
            outputs
                .scenario_test_plan_document
                .contains("telemetry=core_loop_step_1_started | core_loop_step_1_completed")
        );
        assert!(
            outputs
                .runtime_validation_document
                .contains("# Runtime Validation Report")
        );
        assert!(
            outputs
                .runtime_validation_document
                .contains("result_id=runtime_scenario_core_loop_step_1")
        );
        assert!(
            outputs
                .runtime_validation_document
                .contains("acceptance_trace_id=trace_core_loop_step_1")
        );
        assert!(
            outputs
                .runtime_validation_document
                .contains("telemetry_complete=core_loop_step_1_completed")
        );
        assert!(
            outputs
                .production_readiness_document
                .contains("# Production Readiness Report")
        );
        assert!(
            outputs
                .production_readiness_document
                .contains("check_id=scenario_test_plan_readiness; status=ready")
        );
        assert!(
            outputs
                .production_readiness_document
                .contains("check_id=runtime_validation_readiness; status=ready")
        );
        assert!(
            outputs
                .production_readiness_document
                .contains("overall_status=ready")
        );
        assert_eq!(outputs.validation.status, ValidationStatus::Passed);
    }

    #[test]
    fn devflow_step_specs_cover_step00_to_step14() {
        let specs = devflow_step_specs();

        assert_eq!(specs.len(), 15);
        assert_eq!(specs.first().map(|step| step.step_id), Some("step00"));
        assert_eq!(specs.last().map(|step| step.step_id), Some("step14"));
        assert_eq!(core_stage_id_for_devflow_step("step10"), Some("sdk"));
        assert_eq!(
            devflow_step_spec("step07").map(|step| step.group),
            Some("风格确认")
        );
        assert!(core_stage_id_for_devflow_step("unknown").is_none());
    }

    #[test]
    fn core_pipeline_resumes_after_completed_stage_prefix() {
        let provider = MockAiProvider::new(
            ProviderId::new("mock").unwrap(),
            vec![AiCapability::TextGeneration],
        );
        let mut state = PipelineRunState::new(RunId::new("run_resume_core").unwrap());
        state.complete_stage(StageId::new("design").unwrap(), "already done");
        state.complete_stage(StageId::new("development").unwrap(), "already done");
        state.fail("asset plan failed");

        let outputs = CorePipelineServices::new()
            .build_with_state(
                GameDesignBrief::new(
                    "Resume Demo",
                    "2D action",
                    "玩家通过探索、战斗和反馈形成稳定成长目标",
                    vec![
                        "探索关卡".to_string(),
                        "解决战斗".to_string(),
                        "获得反馈".to_string(),
                    ],
                )
                .unwrap(),
                &provider,
                &AiSettings::default(),
                state,
            )
            .expect("resumed outputs");

        assert_eq!(outputs.pipeline_report.results.len(), 3);
        assert_eq!(
            outputs.pipeline_report.results[0].stage_id,
            StageId::new("assets").unwrap()
        );
        assert_eq!(outputs.run_state.completed_stages.len(), 5);
        assert_eq!(outputs.artifact_registry.records().len(), 26);
        assert_eq!(outputs.validation.status, ValidationStatus::Passed);
    }

    #[test]
    fn core_pipeline_reruns_selected_stage_and_downstream_only() {
        let provider = MockAiProvider::new(
            ProviderId::new("mock").unwrap(),
            vec![AiCapability::TextGeneration],
        );
        let services = CorePipelineServices::new();
        let mut state = PipelineRunState::new(RunId::new("run_rerun_core").unwrap());
        for stage in ["design", "development", "assets", "sdk", "packaging"] {
            state.complete_stage(StageId::new(stage).unwrap(), "already done");
        }
        state.finish();

        let rewound = services
            .rewind_state_to_stage(&mut state, &StageId::new("development").unwrap())
            .expect("rewind");
        let outputs = services
            .build_with_state(
                GameDesignBrief::new(
                    "Rerun Demo",
                    "2D action",
                    "玩家通过探索、战斗和反馈形成稳定成长目标",
                    vec![
                        "探索关卡".to_string(),
                        "解决战斗".to_string(),
                        "获得反馈".to_string(),
                    ],
                )
                .unwrap(),
                &provider,
                &AiSettings::default(),
                state,
            )
            .expect("rerun outputs");

        assert_eq!(
            rewound,
            vec![
                StageId::new("development").unwrap(),
                StageId::new("sdk").unwrap(),
                StageId::new("packaging").unwrap()
            ]
        );
        assert_eq!(outputs.pipeline_report.results.len(), 3);
        assert_eq!(
            outputs
                .pipeline_report
                .results
                .iter()
                .map(|result| result.stage_id.as_str())
                .collect::<Vec<_>>(),
            vec!["development", "sdk", "packaging"]
        );
        assert_eq!(outputs.run_state.completed_stages.len(), 5);
        assert_eq!(outputs.artifact_registry.records().len(), 26);
        assert_eq!(outputs.validation.status, ValidationStatus::Passed);
    }
}
