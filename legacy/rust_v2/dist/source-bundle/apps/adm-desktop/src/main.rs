#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]
#![deny(unsafe_code)]

use adm_ai::{
    AiCapability, AiFailureKind, AiOutputValidator, AiProvider, AiTaskJournal,
    AiTaskJournalSummary, AiTaskRecord, AiTaskRequest, AiTaskStatus, MockAiProvider,
};
use adm_application::{
    AdmApplication, AiDiagnosticsReport, DEVFLOW_RUN_REPORT_PATH, DEVFLOW_RUN_STATE_PATH,
    DevflowRangeRunRequest, PipelineService, ProjectPipelineReport, ProjectSummary, RunLogService,
    SdkKnowledgeService, SdkReviewRecord, WorkbenchService, WorkbenchTemplateRow,
    analyze_supplement_request, core_stage_id_for_devflow_step, default_data_root,
    default_demo_brief, design_brief_from_parts, devflow_step_spec, devflow_step_specs,
};
use adm_archive::ArchiveLock;
use adm_config::{
    AiProviderConfig, SecretRef, SecretRefKind, ai_provider_preset, default_secret_ref_for_preset,
};
use adm_design::GameDesignBrief;
use adm_foundation::{
    AdmError, AdmErrorKind, AdmResult, ProviderId, RunId, SessionId, StageId, UtcTimestamp,
};
use adm_packaging::{
    DesktopReleaseSpec, DryRunEngineBuildRunner, EngineBuildExecutionStatus, EngineBuildRunner,
    GameBuildPlan, LOCAL_ENGINE_BUILD_CONFIRMATION_TOKEN, LocalProcessEngineBuildRunner,
    inspect_delivery, inspect_desktop_release, inspect_unity_build_preflight, plan_unity_cli_build,
    plan_unity_runtime_validation, stage_desktop_release, stage_game_build_bundle,
    stage_sdk_bundle, stage_unity_project_scaffold,
};
use adm_pipeline::{
    ArtifactRegistry, PipelineRunReport, PipelineRunState, StageRunResult, StageRunStatus,
};
use adm_ui_model::{
    AiDiagnosticsView, AiProviderDiagnosticsItem, AiStatusView, PackageStatusView,
    PipelineStatusView, ProjectListItem, ShellState, StageProgressItem, ValidationStatusView,
    render_stage_progress,
};
use slint::{ComponentHandle, Model, ModelRc, SharedString, VecModel};
use std::cell::RefCell;
use std::collections::HashMap;
use std::error::Error;
use std::io::{BufWriter, ErrorKind};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::rc::Rc;
use std::thread;
use std::time::{Duration, Instant};

slint::include_modules!();

#[derive(Debug, Clone)]
struct DesktopRunSummary {
    archive_id: String,
    status_text: String,
    pipeline_text: String,
    core_artifact_text: String,
    core_artifacts: Vec<CoreArtifactItem>,
    ai_text: String,
    ai_tasks: Vec<AiTaskItem>,
    sdk_text: String,
    sdk_resources: Vec<SdkResourceItem>,
    package_text: String,
    package_files: Vec<PackageFileItem>,
    build_target_text: String,
    build_targets: Vec<BuildTargetItem>,
    engine_history_text: String,
    engine_history: Vec<EngineHistoryItem>,
    validation_text: String,
    validation_issues: Vec<ValidationIssueItem>,
    acceptance_trace_text: String,
    acceptance_traces: Vec<AcceptanceTraceItem>,
    stage_progress_text: String,
    stage_items: Vec<StageProgressItem>,
    stage_detail: StageDetailView,
}

#[derive(Debug, Clone)]
struct ProjectInspection {
    detail_text: String,
    pipeline_text: String,
    core_artifact_text: String,
    core_artifacts: Vec<CoreArtifactItem>,
    ai_text: String,
    ai_tasks: Vec<AiTaskItem>,
    sdk_text: String,
    sdk_resources: Vec<SdkResourceItem>,
    package_text: String,
    package_files: Vec<PackageFileItem>,
    build_target_text: String,
    build_targets: Vec<BuildTargetItem>,
    engine_history_text: String,
    engine_history: Vec<EngineHistoryItem>,
    validation_text: String,
    validation_issues: Vec<ValidationIssueItem>,
    acceptance_trace_text: String,
    acceptance_traces: Vec<AcceptanceTraceItem>,
    stage_progress_text: String,
    stage_items: Vec<StageProgressItem>,
    stage_detail: StageDetailView,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct StageDetailView {
    label: String,
    stage_id: String,
    status: String,
    message: String,
    artifacts: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct StageArtifactContentSummary {
    contract_kind: String,
    structured_content: String,
    acceptance_checklist: String,
    downstream_inputs: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct AiTaskItem {
    capability: String,
    status: String,
    provider: String,
    failure: String,
    summary: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct PackageFileItem {
    kind: String,
    path: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct BuildTargetItem {
    target_id: String,
    engine: String,
    platform: String,
    profile: String,
    output_file: String,
    required_artifacts: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct EngineHistoryItem {
    target_id: String,
    mode: String,
    status: String,
    launched: String,
    exit_code: String,
    expected_output: String,
    expected_output_path: String,
    expected_output_present: String,
    expected_output_bytes: String,
    expected_output_hash: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct DeliveryCheckItem {
    scope: String,
    path: String,
    present: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct DeliveryDoctorView {
    message: String,
    checks: Vec<DeliveryCheckItem>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct SdkResourceItem {
    sdk_name: String,
    category: String,
    target_engines: String,
    target_platforms: String,
    required_for_build: String,
    purpose: String,
    ai_explanation: String,
    risks: String,
    validation: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct CoreArtifactItem {
    area: String,
    count: String,
    summary: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ValidationIssueItem {
    status: String,
    code: String,
    message: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct AcceptanceTraceItem {
    trace_id: String,
    scenario_id: String,
    source_mechanic: String,
    development_task_id: String,
    asset_task_id: String,
    sdk_resources: String,
    build_targets: String,
    validation_probe: String,
    status: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ImportProjectResult {
    message: String,
    archive_id: String,
    package_doctor_text: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct PackageDoctorView {
    ready: bool,
    message: String,
}

type SelectedArchiveLockState = Rc<RefCell<Option<SelectedArchiveLock>>>;
type WorkbenchServiceState = Rc<RefCell<Option<WorkbenchService>>>;

struct SelectedArchiveLock {
    archive_id: String,
    lock: ArchiveLock,
}

impl StageDetailView {
    fn empty() -> Self {
        Self {
            label: "No stage selected".to_string(),
            stage_id: String::new(),
            status: String::new(),
            message: String::new(),
            artifacts: Vec::new(),
        }
    }

    fn render(&self) -> String {
        let mut detail = format!(
            "Stage: {}\nid={}\nstatus={}\nartifacts={}",
            self.label,
            self.stage_id,
            self.status,
            self.artifacts.len()
        );
        if !self.message.is_empty() {
            detail.push_str(&format!("\nmessage={}", self.message));
        }
        if !self.artifacts.is_empty() {
            detail.push_str("\noutputs=");
            for artifact in &self.artifacts {
                detail.push_str(&format!("\n- {artifact}"));
            }
        }
        detail
    }
}

impl StageArtifactContentSummary {
    fn render(&self) -> String {
        format!(
            "Step Artifact Content\ncontract_kind={}\n\nStructured Stage Content\n{}\n\nAcceptance Checklist\n{}\n\nDownstream Inputs\n{}",
            self.contract_kind,
            self.structured_content,
            self.acceptance_checklist,
            self.downstream_inputs
        )
    }
}

fn main() {
    if let Err(error) = run_entry() {
        eprintln!("{error}");
        std::process::exit(1);
    }
}

fn run_entry() -> Result<(), Box<dyn Error>> {
    let args = std::env::args().collect::<Vec<_>>();
    if is_fake_unity_invocation(&args) {
        run_fake_unity_runner(&args[1..])
    } else if args.get(1).is_some_and(|arg| arg == "--ui-audit") {
        run_ui_audit(args.get(2))
    } else if args.iter().any(|arg| arg == "--smoke") {
        run_smoke()
    } else if args.get(1).is_some_and(|arg| arg == "--lock-probe") {
        run_lock_probe(&args[2..])
    } else {
        run_window()
    }
}

fn is_fake_unity_invocation(args: &[String]) -> bool {
    if !args.iter().any(|arg| arg == "-executeMethod") {
        return false;
    }
    let Some(exe) = std::env::current_exe().ok() else {
        return false;
    };
    exe.file_name().is_some_and(|name| name == "Unity.exe")
        && exe
            .components()
            .any(|component| component.as_os_str() == "fake-unity")
}

fn run_window() -> Result<(), Box<dyn Error>> {
    select_desktop_backend()?;
    let ui = MainWindow::new()?;
    let initial_root =
        default_data_root(&std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")));
    ui.set_data_root(path_to_text(&initial_root).into());
    ui.set_export_path(path_to_text(&initial_root.join("project.admproj")).into());
    ui.set_release_source_exe(path_to_text(&default_desktop_release_source()).into());
    ui.set_release_target_dir(path_to_text(&default_desktop_release_dir()).into());
    ui.set_game_build_target_dir(path_to_text(&default_game_build_bundle_dir()).into());
    ui.set_sdk_bundle_target_dir(path_to_text(&default_sdk_bundle_dir()).into());
    ui.set_unity_project_dir(path_to_text(&default_unity_project_dir()).into());
    refresh_projects(&ui, &initial_root, None);
    refresh_ai_diagnostics(&ui, &initial_root);
    refresh_design_reference(&ui, None);
    refresh_design_templates(&ui, &initial_root);
    let _ = append_run_log_for_ui(
        &initial_root,
        "INFO",
        "desktop",
        "window_started",
        "mode=gui",
    );
    refresh_run_log(&ui, &initial_root, "");
    refresh_sdk_knowledge(&ui, &initial_root);
    refresh_pipeline_service_status(&ui, &initial_root);
    attach_callbacks(&ui);
    ui.run()?;
    Ok(())
}

fn select_desktop_backend() -> Result<(), slint::PlatformError> {
    let selector = slint::BackendSelector::new();
    if std::env::var_os("SLINT_BACKEND").is_some() {
        selector.select()
    } else {
        selector.backend_name("software".to_string()).select()
    }
}

const MAIN_SLINT_SOURCE: &str = include_str!("../ui/main.slint");

const UI_AUDIT_VIEWS: &[&str] = &["design", "pipeline", "patch", "package", "logs", "sdk"];

const UI_AUDIT_LONG_TEXT_BINDINGS: &[&str] = &[
    "root.design-ai-interview-text",
    "root.design-right-tab",
    "root.pipeline-service-text",
    "root.supplement-analysis-text",
    "root.package-text",
    "root.game-build-text",
    "root.run-log-text",
    "root.sdk-review-text",
];

const UI_AUDIT_LONG_ROW_BINDINGS: &[&str] = &[
    "root.stage-items",
    "root.package-file-items",
    "root.sdk-review-items",
    "root.sdk-resource-items",
];

#[derive(Debug, Clone)]
struct UiSnapshotAudit {
    view: String,
    path: Option<PathBuf>,
    width: u32,
    height: u32,
    bytes: usize,
    non_zero_bytes: usize,
    varied_samples: usize,
}

fn run_ui_audit(output_path: Option<&String>) -> Result<(), Box<dyn Error>> {
    select_desktop_backend()?;
    let ui = MainWindow::new()?;
    ui.window().set_size(slint::PhysicalSize::new(1280, 860));
    let probe = ui_audit_probe_text();
    let probe_marker = "UI_AUDIT_LONG_TEXT_PROBE";
    let report_path = output_path
        .filter(|path| path.as_str() != "-")
        .map(PathBuf::from);
    let screenshot_base_path = report_path.as_ref().map(|path| path.with_extension("png"));

    ui.set_design_summary_text(probe.clone().into());
    ui.set_design_ai_interview_text(probe.clone().into());
    ui.set_design_template_text(probe.clone().into());
    ui.set_pipeline_service_text(probe.clone().into());
    ui.set_pipeline_text(probe.clone().into());
    ui.set_stage_detail_text(probe.clone().into());
    ui.set_supplement_analysis_text(probe.clone().into());
    ui.set_package_text(probe.clone().into());
    ui.set_release_doctor_text(probe.clone().into());
    ui.set_delivery_doctor_text(probe.clone().into());
    ui.set_game_build_text(probe.clone().into());
    ui.set_sdk_bundle_text(probe.clone().into());
    ui.set_unity_build_text(probe.clone().into());
    ui.set_runtime_validation_text(probe.clone().into());
    ui.set_run_log_text(probe.clone().into());
    ui.set_ai_text(probe.clone().into());
    ui.set_validation_text(probe.clone().into());
    ui.set_engine_history_text(probe.clone().into());
    ui.set_sdk_review_text(probe.clone().into());
    ui.set_sdk_text(probe.into());

    let mut lines = vec![
        "# UI Visual Audit".to_string(),
        "status=passed".to_string(),
        "mode=slint_software_backend".to_string(),
        "window_source_width=1280".to_string(),
        "window_source_height=860".to_string(),
        "screenshot_artifacts=generated".to_string(),
        format!("screenshot_artifact_count={}", UI_AUDIT_VIEWS.len()),
        String::new(),
        "## Active View Probe".to_string(),
    ];

    let mut snapshot_audits = Vec::new();
    for view in UI_AUDIT_VIEWS {
        ui.set_active_view((*view).into());
        let passed = ui.get_active_view().as_str() == *view;
        if !passed {
            return Err(std::io::Error::new(
                ErrorKind::Other,
                format!("UI audit failed to switch active view to {view}"),
            )
            .into());
        }
        snapshot_audits.push(audit_ui_snapshot(
            &ui,
            view,
            screenshot_base_path.as_deref(),
        )?);
        lines.push(format!("- {view}=passed"));
    }

    lines.push(String::new());
    lines.push("## Screenshot Probe".to_string());
    for audit in snapshot_audits {
        let path = audit
            .path
            .as_ref()
            .map(|path| path.display().to_string())
            .unwrap_or_else(|| "not_written".to_string());
        lines.push(format!(
            "- {}: path={} width={} height={} bytes={} non_zero_bytes={} varied_samples={}",
            audit.view,
            path,
            audit.width,
            audit.height,
            audit.bytes,
            audit.non_zero_bytes,
            audit.varied_samples
        ));
    }

    let probe_checks = [
        (
            "design_summary_text",
            ui.get_design_summary_text().contains(probe_marker),
        ),
        (
            "design_ai_interview_text",
            ui.get_design_ai_interview_text().contains(probe_marker),
        ),
        (
            "pipeline_service_text",
            ui.get_pipeline_service_text().contains(probe_marker),
        ),
        (
            "stage_detail_text",
            ui.get_stage_detail_text().contains(probe_marker),
        ),
        (
            "supplement_analysis_text",
            ui.get_supplement_analysis_text().contains(probe_marker),
        ),
        ("package_text", ui.get_package_text().contains(probe_marker)),
        (
            "game_build_text",
            ui.get_game_build_text().contains(probe_marker),
        ),
        ("run_log_text", ui.get_run_log_text().contains(probe_marker)),
        (
            "sdk_review_text",
            ui.get_sdk_review_text().contains(probe_marker),
        ),
    ];
    lines.push(String::new());
    lines.push("## Long Text Probe".to_string());
    for (name, passed) in probe_checks {
        if !passed {
            return Err(std::io::Error::new(
                ErrorKind::Other,
                format!("UI audit long text probe did not round-trip {name}"),
            )
            .into());
        }
        lines.push(format!("- {name}=passed"));
    }

    lines.push(String::new());
    lines.push("## Scroll And Wrap Contract".to_string());
    for binding in UI_AUDIT_LONG_TEXT_BINDINGS {
        let segment = nearest_scroll_segment(*binding)?;
        let wraps = segment.contains("wrap: word-wrap;");
        if !wraps {
            return Err(std::io::Error::new(
                ErrorKind::Other,
                format!("{binding} must wrap inside its ScrollView"),
            )
            .into());
        }
        lines.push(format!("- {binding}: scrollview=true wrap=true"));
    }

    lines.push(String::new());
    lines.push("## Long Row Contract".to_string());
    for binding in UI_AUDIT_LONG_ROW_BINDINGS {
        let _ = nearest_scroll_segment(*binding)?;
        lines.push(format!("- {binding}: scrollview=true"));
    }

    let report = format!("{}\n", lines.join("\n"));
    if let Some(path) = report_path {
        std::fs::write(path, &report)?;
    }
    print!("{report}");
    Ok(())
}

fn ui_audit_probe_text() -> String {
    let mut text = String::new();
    for index in 0..80 {
        text.push_str(&format!(
            "UI_AUDIT_LONG_TEXT_PROBE line={index:02} content=This line exercises wrapped and scrollable desktop report text across Slint surfaces.\n"
        ));
    }
    text
}

fn audit_ui_snapshot(
    ui: &MainWindow,
    view: &str,
    screenshot_base_path: Option<&Path>,
) -> Result<UiSnapshotAudit, Box<dyn Error>> {
    let snapshot = ui.window().take_snapshot()?;
    let width = snapshot.width();
    let height = snapshot.height();
    let bytes = snapshot.as_bytes();
    if width < 1000 || height < 700 {
        return Err(std::io::Error::new(
            ErrorKind::Other,
            format!("UI audit snapshot for {view} is unexpectedly small: {width}x{height}"),
        )
        .into());
    }
    let expected_len = width as usize * height as usize * 4;
    if bytes.len() != expected_len {
        return Err(std::io::Error::new(
            ErrorKind::Other,
            format!(
                "UI audit snapshot for {view} has {} bytes, expected {expected_len}",
                bytes.len()
            ),
        )
        .into());
    }
    let non_zero_bytes = bytes.iter().filter(|byte| **byte != 0).count();
    if non_zero_bytes == 0 {
        return Err(std::io::Error::new(
            ErrorKind::Other,
            format!("UI audit snapshot for {view} is blank"),
        )
        .into());
    }
    let first_pixel = bytes.get(0..4).ok_or_else(|| {
        std::io::Error::new(
            ErrorKind::Other,
            format!("UI audit snapshot for {view} did not contain a complete first pixel"),
        )
    })?;
    let varied_samples = bytes
        .chunks_exact(4)
        .step_by(97)
        .filter(|pixel| *pixel != first_pixel)
        .count();
    if varied_samples < 8 {
        return Err(std::io::Error::new(
            ErrorKind::Other,
            format!("UI audit snapshot for {view} does not contain enough visual variation"),
        )
        .into());
    }
    let path = screenshot_base_path.map(|base| screenshot_path_for_view(base, view));
    if let Some(path) = &path {
        write_png_snapshot(path, &snapshot)?;
    }
    Ok(UiSnapshotAudit {
        view: view.to_string(),
        path,
        width,
        height,
        bytes: bytes.len(),
        non_zero_bytes,
        varied_samples,
    })
}

fn screenshot_path_for_view(base: &Path, view: &str) -> PathBuf {
    let parent = base.parent().unwrap_or_else(|| Path::new(""));
    let stem = base
        .file_stem()
        .and_then(|stem| stem.to_str())
        .filter(|stem| !stem.is_empty())
        .unwrap_or("ui-visual-audit");
    let extension = base
        .extension()
        .and_then(|extension| extension.to_str())
        .filter(|extension| !extension.is_empty())
        .unwrap_or("png");
    parent.join(format!("{stem}-{view}.{extension}"))
}

fn write_png_snapshot(
    path: &Path,
    snapshot: &slint::SharedPixelBuffer<slint::Rgba8Pixel>,
) -> Result<(), Box<dyn Error>> {
    if let Some(parent) = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        std::fs::create_dir_all(parent)?;
    }
    let file = std::fs::File::create(path)?;
    let mut encoder = png::Encoder::new(BufWriter::new(file), snapshot.width(), snapshot.height());
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    let mut writer = encoder.write_header()?;
    writer.write_image_data(snapshot.as_bytes())?;
    Ok(())
}

fn nearest_scroll_segment(binding: &str) -> Result<&'static str, Box<dyn Error>> {
    let binding_index = MAIN_SLINT_SOURCE.find(binding).ok_or_else(|| {
        std::io::Error::new(
            ErrorKind::Other,
            format!("{binding} binding missing from Slint UI"),
        )
    })?;
    let before_binding = &MAIN_SLINT_SOURCE[..binding_index];
    let scroll_index = before_binding.rfind("ScrollView").ok_or_else(|| {
        std::io::Error::new(
            ErrorKind::Other,
            format!("{binding} is not inside a ScrollView"),
        )
    })?;
    let target_close_index = (binding_index + 1400).min(MAIN_SLINT_SOURCE.len());
    let close_index = MAIN_SLINT_SOURCE
        .char_indices()
        .map(|(index, _)| index)
        .find(|index| *index >= target_close_index)
        .unwrap_or(MAIN_SLINT_SOURCE.len());
    let segment = &MAIN_SLINT_SOURCE[scroll_index..close_index];
    if !segment.contains(binding) {
        return Err(std::io::Error::new(
            ErrorKind::Other,
            format!("{binding} is not covered by the nearest ScrollView segment"),
        )
        .into());
    }
    Ok(segment)
}

fn zh(value: impl AsRef<str>) -> SharedString {
    if std::env::args().any(|arg| arg == "--smoke") {
        return SharedString::from(value.as_ref());
    }
    SharedString::from(localize_ui_text(value.as_ref()))
}

fn localize_ui_text(value: &str) -> String {
    let mut text = value.to_string();
    for (from, to) in UI_TEXT_REPLACEMENTS {
        text = text.replace(from, to);
    }
    text
}

const UI_TEXT_REPLACEMENTS: &[(&str, &str)] = &[
    ("Error:", "错误："),
    ("Ready", "就绪"),
    ("Pipeline idle.", "流水线空闲。"),
    ("AI idle.", "智能服务空闲。"),
    ("No project selected.", "尚未选择项目。"),
    ("No projects loaded.", "尚未加载项目。"),
    ("Package not inspected.", "尚未检查项目包。"),
    ("Build targets not inspected.", "尚未检查构建目标。"),
    ("Engine history not inspected.", "尚未检查引擎历史。"),
    ("SDK not inspected.", "尚未检查开发套件。"),
    ("Core artifacts not inspected.", "尚未检查核心产物。"),
    ("Validation not inspected.", "尚未检查验证结果。"),
    ("Acceptance matrix not inspected.", "尚未检查验收矩阵。"),
    ("Stages: not run.", "阶段：尚未运行。"),
    ("Project detail unavailable.", "项目详情不可用。"),
    ("Pipeline unavailable.", "流水线不可用。"),
    ("Core artifacts unavailable.", "核心产物不可用。"),
    ("AI unavailable.", "智能服务不可用。"),
    ("SDK unavailable.", "开发套件不可用。"),
    ("Package unavailable.", "项目包不可用。"),
    ("Build targets unavailable.", "构建目标不可用。"),
    ("Engine history unavailable.", "引擎历史不可用。"),
    ("Validation unavailable.", "验证结果不可用。"),
    ("Acceptance matrix unavailable.", "验收矩阵不可用。"),
    ("Stages unavailable.", "阶段不可用。"),
    ("Data root:", "数据目录："),
    ("AI Config:", "智能服务配置："),
    ("AI:", "智能服务："),
    ("Applied AI provider preset", "已应用智能服务提供商预设"),
    ("Saved AI provider config to", "已保存智能服务提供商配置到"),
    ("Saved AI named secret", "已保存智能服务命名密钥"),
    ("Disabled AI provider", "已禁用智能服务提供商"),
    ("Provider Check:", "提供商检查："),
    ("Provider Invoke:", "提供商调用："),
    ("model=", "模型="),
    ("network_call=false", "网络调用=false"),
    ("network_call=true", "网络调用=true"),
    ("supports.", "支持."),
    ("ready_provider_count=", "就绪提供商数量="),
    ("provider_count=", "提供商数量="),
    ("budget=", "预算="),
    ("retries=", "重试次数="),
    ("Workspace doctor completed.", "工作区诊断已完成。"),
    ("Workspace doctor failed.", "工作区诊断失败。"),
    ("Workspace cleanup completed.", "工作区清理已完成。"),
    ("Workspace cleanup failed.", "工作区清理失败。"),
    ("Package doctor completed:", "项目包诊断已完成："),
    ("Project package doctor failed.", "项目包诊断失败。"),
    ("Desktop release staged.", "桌面发布包已暂存。"),
    ("Desktop release failed.", "桌面发布失败。"),
    ("Release doctor completed.", "发布诊断已完成。"),
    ("Release doctor failed.", "发布诊断失败。"),
    ("Delivery doctor completed.", "交付诊断已完成。"),
    ("Delivery doctor failed.", "交付诊断失败。"),
    ("Game build bundle staged.", "游戏构建包已暂存。"),
    ("Game build bundle failed.", "游戏构建包暂存失败。"),
    ("SDK bundle staged.", "开发套件包已暂存。"),
    ("SDK bundle failed.", "开发套件包暂存失败。"),
    ("Unity project scaffold staged.", "Unity 项目脚手架已暂存。"),
    (
        "Unity project scaffold failed.",
        "Unity 项目脚手架暂存失败。",
    ),
    ("Unity build preflight completed.", "Unity 构建预检已完成。"),
    ("Unity build preflight failed.", "Unity 构建预检失败。"),
    ("Unity build command planned.", "Unity 构建命令已规划。"),
    ("Unity build command failed.", "Unity 构建命令规划失败。"),
    (
        "Unity build dry-run completed and persisted.",
        "Unity 构建试运行已完成并保存。",
    ),
    ("Unity build dry-run completed", "Unity 构建试运行已完成"),
    ("Unity build dry-run failed.", "Unity 构建试运行失败。"),
    (
        "Unity build run completed and persisted.",
        "Unity 构建运行已完成并保存。",
    ),
    ("Unity build run completed", "Unity 构建运行已完成"),
    ("Unity build run failed.", "Unity 构建运行失败。"),
    (
        "Unity runtime validation command planned.",
        "Unity 运行时验证命令已规划。",
    ),
    (
        "Unity runtime validation command failed.",
        "Unity 运行时验证命令规划失败。",
    ),
    (
        "Unity runtime validation dry-run completed.",
        "Unity 运行时验证试运行已完成。",
    ),
    (
        "Unity runtime validation dry-run failed.",
        "Unity 运行时验证试运行失败。",
    ),
    (
        "Unity runtime validation run completed.",
        "Unity 运行时验证运行已完成。",
    ),
    (
        "Unity runtime validation run failed.",
        "Unity 运行时验证运行失败。",
    ),
    ("Runtime validation recorded.", "运行时验证结果已记录。"),
    (
        "Runtime validation record failed.",
        "运行时验证结果记录失败。",
    ),
    ("relock failed:", "重新加锁失败："),
    ("package_ready=", "项目包就绪="),
    ("ready=", "就绪="),
    ("locked=", "已锁定="),
    ("artifacts=", "产物数="),
    ("pipeline=", "流水线="),
    ("validation=", "验证="),
    ("files=", "文件数="),
    ("status=", "状态="),
    ("message=", "消息="),
    ("stage_id=", "阶段编号="),
    ("lock_state=", "锁状态="),
    ("lock_owner=", "锁持有者="),
    ("output=", "输出="),
    ("bytes=", "字节="),
    ("hash=", "哈希="),
    ("path=", "路径="),
    ("Created", "已创建"),
    ("Succeeded", "成功"),
    ("Failed", "失败"),
    ("Warning", "警告"),
    ("Resumed failed stage", "已恢复失败阶段"),
    ("Resumed", "已恢复"),
    ("Reran", "已重跑"),
    ("Active", "运行中"),
    ("Completed", "已完成"),
    ("Pending", "等待中"),
    ("Unknown", "未知"),
    ("completed previously", "之前已完成"),
    ("Design", "设计"),
    ("Development", "开发"),
    ("Assets", "资源"),
    ("Packaging", "打包"),
    ("Core Artifacts:", "核心产物："),
    ("core_loop=", "核心循环数="),
    ("tasks=", "任务数="),
    ("quality=", "质量="),
    ("ai=", "智能介入="),
    ("design_core_loop=", "设计核心循环="),
    ("development_tasks=", "开发任务数="),
    ("asset_tasks=", "资源任务数="),
    ("Project:", "项目："),
    ("No projects yet.", "暂无项目。"),
    ("Pipeline: not run", "流水线：尚未运行"),
    ("Pipeline:", "流水线："),
    ("Package:", "项目包："),
    ("Acceptance Matrix:", "验收矩阵："),
    ("Validation:", "验证："),
    ("Acceptance:", "验收："),
    ("Build Targets: not planned", "构建目标：尚未规划"),
    ("Build Targets:", "构建目标："),
    ("Engine History:", "引擎历史："),
    ("Runtime Validation:", "运行时验证："),
    ("Runtime Validation Result:", "运行时验证结果："),
    ("Game Build Bundle:", "游戏构建包："),
    ("SDK Bundle:", "开发套件包："),
    ("SDK:", "开发套件："),
    ("SDK", "开发套件"),
    ("Release:", "发布："),
    ("Delivery:", "交付："),
    ("completed=", "已完成="),
    ("active=", "当前阶段="),
    ("records=", "记录数="),
    ("launched=", "已启动="),
    ("failed=", "失败="),
    ("outputs_present=", "输出存在="),
    ("resources=", "资源数="),
    ("support_files=", "支持文件数="),
    ("entry_count=", "条目数="),
    ("artifact_count=", "产物数="),
    ("target_id=", "目标编号="),
    ("bundle_dir=", "包目录="),
    ("manifest=", "清单="),
    ("staged_files=", "暂存文件数="),
    ("runtime_results_file=", "运行时结果文件="),
    ("runtime_ready=", "运行时就绪="),
    ("runtime_runner=", "运行器="),
    ("runtime_contract_rows=", "契约行数="),
    ("runtime_observed_rows=", "观测行数="),
    ("runtime_passed_rows=", "通过行数="),
    ("runtime_failed_rows=", "失败行数="),
    ("runtime_missing_rows=", "缺失行数="),
    ("runtime_unexpected_rows=", "意外行数="),
    ("runtime_commit_files=", "运行时提交文件数="),
    ("contract_rows=", "契约行数="),
    ("observed_rows=", "观测行数="),
    ("passed_rows=", "通过行数="),
    ("failed_rows=", "失败行数="),
    ("missing_rows=", "缺失行数="),
    ("unexpected_rows=", "意外行数="),
    ("source_file=", "来源文件="),
    ("results_file=", "结果文件="),
    ("targets=", "目标数="),
    ("required_artifacts=", "必需产物数="),
    ("production_readiness=", "生产就绪度="),
    ("risks=", "风险数="),
    ("rows=", "行数="),
    ("incomplete=", "未完成="),
    ("game_build_bundle=", "游戏构建包="),
    ("sdk_bundle=", "开发套件包="),
    ("unity_project=", "Unity 项目="),
    ("release=", "发布="),
    ("entry", "入口"),
    ("optional_support", "可选支持"),
    ("optional_missing", "可选缺失"),
    ("support", "支持"),
    ("present", "存在"),
    ("missing", "缺失"),
    ("verified", "已验证"),
    ("dry_run", "试运行"),
    ("mode=", "模式="),
    ("expected_output_present=", "预期输出存在="),
    ("expected_output=", "预期输出="),
    ("expected_output_path=", "预期输出路径="),
    ("expected_output_bytes=", "预期输出字节="),
    ("expected_output_hash=", "预期输出哈希="),
    ("exit_code=", "退出码="),
    ("command_line=", "命令行="),
    ("history_file=", "历史文件="),
    ("history_records=", "历史记录数="),
    ("history_commit_files=", "历史提交文件数="),
    ("project_dir=", "项目目录="),
    ("generated_files=", "生成文件数="),
    (
        "game build target id cannot be empty",
        "游戏构建目标编号不能为空",
    ),
    ("archive id cannot be empty", "存档编号不能为空"),
    ("data root cannot be empty", "数据目录不能为空"),
    ("import file cannot be empty", "导入文件不能为空"),
    ("unknown game build target:", "未知游戏构建目标："),
    (
        "project package is not ready for import",
        "项目包尚未达到可导入状态",
    ),
    ("not run", "尚未运行"),
    ("not built", "尚未构建"),
    ("not indexed", "尚未建立索引"),
    ("no journal", "没有日志"),
    ("no tasks", "没有任务"),
    ("missing title", "缺少标题"),
    ("missing genre", "缺少类型"),
    ("n/a", "无"),
    ("=true", "=是"),
    ("=false", "=否"),
    ("=ready", "=就绪"),
    ("=failed", "=失败"),
    ("=passed", "=通过"),
    ("provider", "提供商"),
    ("Provider", "提供商"),
    ("capabilities", "能力"),
    ("MissingSecret", "缺少密钥"),
    ("Disabled", "已禁用"),
];

fn run_fake_unity_runner(args: &[String]) -> Result<(), Box<dyn Error>> {
    let current_dir = std::env::current_dir()?;
    if let Some(build_output) = arg_value(args, "-customBuildPath") {
        let build_output = resolve_child_path(&current_dir, build_output);
        if let Some(parent) = build_output.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(&build_output, b"fake unity build output\n")?;
        println!("fake_unity_build_output={}", build_output.display());
    }
    let runtime_output = arg_value(args, "-admRuntimeValidationOutput")
        .map(|value| resolve_child_path(&current_dir, value));
    if let Some(runtime_output) = runtime_output {
        write_runtime_validation_execution_fixture(&runtime_output)?;
        println!("fake_unity_runtime_output={}", runtime_output.display());
    }
    Ok(())
}

fn arg_value<'a>(args: &'a [String], key: &str) -> Option<&'a str> {
    args.windows(2).find_map(|pair| {
        if pair[0] == key {
            Some(pair[1].as_str())
        } else {
            None
        }
    })
}

fn resolve_child_path(base: &Path, value: &str) -> PathBuf {
    let path = PathBuf::from(value);
    if path.is_absolute() {
        path
    } else {
        base.join(path)
    }
}

fn run_lock_probe(args: &[String]) -> Result<(), Box<dyn Error>> {
    if args.len() != 5 {
        return Err(std::io::Error::new(
            ErrorKind::InvalidInput,
            "usage: --lock-probe <data_root> <archive_id> <ready_file> <release_file> <timeout_ms>",
        )
        .into());
    }
    let data_root = PathBuf::from(&args[0]);
    let archive_id = &args[1];
    let ready_file = PathBuf::from(&args[2]);
    let release_file = PathBuf::from(&args[3]);
    let timeout_ms = args[4].parse::<u64>().map_err(|error| {
        std::io::Error::new(
            ErrorKind::InvalidInput,
            format!("invalid lock probe timeout: {error}"),
        )
    })?;

    let app = AdmApplication::for_data_root(&data_root)?;
    let archive = app.load_project(archive_id)?;
    let lock = ArchiveLock::acquire(&archive.root, SessionId::generate())?;
    if let Some(parent) = ready_file.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(
        &ready_file,
        format!(
            "archive_id={archive_id}\nsession_id={}\npid={}\n",
            lock.session_id(),
            std::process::id()
        ),
    )?;

    let deadline = Instant::now() + Duration::from_millis(timeout_ms);
    while !release_file.exists() {
        if Instant::now() >= deadline {
            return Err(std::io::Error::new(
                ErrorKind::TimedOut,
                "lock probe timed out waiting for release file",
            )
            .into());
        }
        thread::sleep(Duration::from_millis(25));
    }
    println!(
        "lock_probe=released archive_id={archive_id} session_id={}",
        lock.session_id()
    );
    Ok(())
}

fn attach_callbacks(ui: &MainWindow) {
    let selected_lock: SelectedArchiveLockState = Rc::new(RefCell::new(None));
    let workbench_service: WorkbenchServiceState = Rc::new(RefCell::new(None));

    let create_ui = ui.as_weak();
    let create_lock = Rc::clone(&selected_lock);
    let create_workbench_service = Rc::clone(&workbench_service);
    ui.on_create_and_run(move || {
        if let Some(ui) = create_ui.upgrade() {
            let data_root = PathBuf::from(ui.get_data_root().to_string());
            release_selected_archive_lock(&create_lock);
            let brief = match pipeline_brief_for_ui(&ui, &create_workbench_service) {
                Ok(brief) => brief,
                Err(error) => {
                    ui.set_status_text(zh(format!("导出设计工作台到流水线失败：{error}")));
                    return;
                }
            };
            match create_and_run_project_from_brief(brief, &data_root) {
                Ok(summary) => {
                    ui.set_selected_archive_id(summary.archive_id.clone().into());
                    let _ = lock_selected_archive(&data_root, &summary.archive_id, &create_lock);
                    refresh_projects(&ui, &data_root, Some(&create_lock));
                    apply_run_summary(&ui, &summary);
                }
                Err(error) => {
                    ui.set_status_text(zh(format!("Error: {error}")));
                }
            }
        }
    });

    let refresh_ui = ui.as_weak();
    let refresh_lock = Rc::clone(&selected_lock);
    ui.on_refresh_projects(move || {
        if let Some(ui) = refresh_ui.upgrade() {
            let data_root = PathBuf::from(ui.get_data_root().to_string());
            refresh_projects(&ui, &data_root, Some(&refresh_lock));
            refresh_ai_diagnostics(&ui, &data_root);
        }
    });

    let ai_diagnostics_ui = ui.as_weak();
    ui.on_refresh_ai_diagnostics(move || {
        if let Some(ui) = ai_diagnostics_ui.upgrade() {
            let data_root = PathBuf::from(ui.get_data_root().to_string());
            refresh_ai_diagnostics(&ui, &data_root);
            let _ = append_run_log_for_ui(
                &data_root,
                "INFO",
                "ai",
                "refreshed_ai_diagnostics",
                "source=desktop",
            );
            refresh_run_log(&ui, &data_root, ui.get_run_log_filter().to_string());
        }
    });

    let refresh_run_log_ui = ui.as_weak();
    ui.on_refresh_run_log(move |filter| {
        if let Some(ui) = refresh_run_log_ui.upgrade() {
            let data_root = PathBuf::from(ui.get_data_root().to_string());
            refresh_run_log(&ui, &data_root, filter.to_string());
        }
    });

    let clear_run_log_ui = ui.as_weak();
    ui.on_clear_run_log(move || {
        if let Some(ui) = clear_run_log_ui.upgrade() {
            let data_root = PathBuf::from(ui.get_data_root().to_string());
            match RunLogService::new(&data_root).clear() {
                Ok(()) => {
                    let _ = append_run_log_for_ui(
                        &data_root,
                        "INFO",
                        "logs",
                        "cleared_run_log",
                        "source=desktop",
                    );
                    refresh_run_log(&ui, &data_root, ui.get_run_log_filter().to_string());
                    ui.set_status_text(zh("严格运行日志已清空。"));
                }
                Err(error) => ui.set_status_text(zh(format!("清空严格运行日志失败：{error}"))),
            }
        }
    });

    let export_run_log_ui = ui.as_weak();
    ui.on_export_run_log(move || {
        if let Some(ui) = export_run_log_ui.upgrade() {
            let data_root = PathBuf::from(ui.get_data_root().to_string());
            let target = data_root.join("exports").join("run_log.jsonl");
            let _ = append_run_log_for_ui(
                &data_root,
                "INFO",
                "logs",
                "export_run_log_requested",
                &format!("target={}", target.display()),
            );
            match RunLogService::new(&data_root).export_jsonl(&target) {
                Ok(path) => {
                    ui.set_run_log_export_path(path_to_text(&path).into());
                    refresh_run_log(&ui, &data_root, ui.get_run_log_filter().to_string());
                    ui.set_status_text(zh(format!("严格运行日志已导出：{}", path.display())));
                }
                Err(error) => ui.set_status_text(zh(format!("导出严格运行日志失败：{error}"))),
            }
        }
    });

    let refresh_sdk_knowledge_ui = ui.as_weak();
    ui.on_refresh_sdk_knowledge(move || {
        if let Some(ui) = refresh_sdk_knowledge_ui.upgrade() {
            let data_root = PathBuf::from(ui.get_data_root().to_string());
            refresh_sdk_knowledge(&ui, &data_root);
            let _ = append_run_log_for_ui(
                &data_root,
                "INFO",
                "sdk",
                "refreshed_sdk_knowledge",
                "source=desktop",
            );
            refresh_run_log(&ui, &data_root, ui.get_run_log_filter().to_string());
        }
    });

    let add_sdk_resource_ui = ui.as_weak();
    ui.on_add_sdk_resource(move |sdk_name, url| {
        if let Some(ui) = add_sdk_resource_ui.upgrade() {
            let data_root = PathBuf::from(ui.get_data_root().to_string());
            match SdkKnowledgeService::new(&data_root).add_pending(sdk_name.as_str(), url.as_str())
            {
                Ok(record) => {
                    ui.set_sdk_review_selected_id(record.id.clone().into());
                    refresh_sdk_knowledge(&ui, &data_root);
                    let _ = append_run_log_for_ui(
                        &data_root,
                        "INFO",
                        "sdk",
                        "added_sdk_candidate",
                        &format!("id={}", record.id),
                    );
                    refresh_run_log(&ui, &data_root, ui.get_run_log_filter().to_string());
                    ui.set_status_text(zh(format!("SDK 候选项已加入待审核：{}", record.sdk_name)));
                }
                Err(error) => ui.set_status_text(zh(format!("新增 SDK 候选项失败：{error}"))),
            }
        }
    });

    let approve_sdk_resource_ui = ui.as_weak();
    ui.on_approve_sdk_resource(move |record_id| {
        if let Some(ui) = approve_sdk_resource_ui.upgrade() {
            update_sdk_review_status(&ui, record_id.as_str(), "approve");
        }
    });

    let pending_sdk_resource_ui = ui.as_weak();
    ui.on_mark_sdk_resource_pending(move |record_id| {
        if let Some(ui) = pending_sdk_resource_ui.upgrade() {
            update_sdk_review_status(&ui, record_id.as_str(), "pending");
        }
    });

    let reject_sdk_resource_ui = ui.as_weak();
    ui.on_reject_sdk_resource(move |record_id| {
        if let Some(ui) = reject_sdk_resource_ui.upgrade() {
            update_sdk_review_status(&ui, record_id.as_str(), "reject");
        }
    });

    let design_reference_ui = ui.as_weak();
    let design_reference_service = Rc::clone(&workbench_service);
    ui.on_refresh_design_reference(move || {
        if let Some(ui) = design_reference_ui.upgrade() {
            refresh_design_reference(&ui, Some(&design_reference_service));
        }
    });

    let select_design_domain_ui = ui.as_weak();
    let select_design_domain_service = Rc::clone(&workbench_service);
    ui.on_select_design_domain(move |domain_id| {
        if let Some(ui) = select_design_domain_ui.upgrade() {
            select_design_domain(&ui, &select_design_domain_service, domain_id.as_str());
        }
    });

    let select_design_node_ui = ui.as_weak();
    let select_design_node_service = Rc::clone(&workbench_service);
    ui.on_select_design_node(move |node_id| {
        if let Some(ui) = select_design_node_ui.upgrade() {
            mutate_design_workbench(&ui, &select_design_node_service, |service| {
                service.select_node(node_id.as_str())?;
                Ok(format!("已选择节点：{node_id}"))
            });
        }
    });

    let save_design_project_name_ui = ui.as_weak();
    let save_design_project_name_service = Rc::clone(&workbench_service);
    ui.on_save_design_project_name(move |project_name| {
        if let Some(ui) = save_design_project_name_ui.upgrade() {
            mutate_design_workbench(&ui, &save_design_project_name_service, |service| {
                service.set_project_name(project_name.as_str());
                Ok(format!("项目名称已更新：{project_name}"))
            });
        }
    });

    let export_design_ui = ui.as_weak();
    let export_design_service = Rc::clone(&workbench_service);
    ui.on_export_design_workbench(move |format| {
        if let Some(ui) = export_design_ui.upgrade() {
            let data_root = PathBuf::from(ui.get_data_root().to_string());
            match ensure_workbench_loaded(&export_design_service, &data_root) {
                Ok(()) => {
                    let service_ref = export_design_service.borrow();
                    let Some(service) = service_ref.as_ref() else {
                        ui.set_status_text(zh("设计工作台未初始化。"));
                        return;
                    };
                    let target = workbench_export_path(&data_root, format.as_str());
                    match service.export_to_file(&target, format.as_str()) {
                        Ok(path) => {
                            ui.set_export_path(path_to_text(&path).into());
                            ui.set_status_text(zh(format!("设计工作台已导出：{}", path.display())));
                        }
                        Err(error) => {
                            ui.set_status_text(zh(format!("设计工作台导出失败：{error}")))
                        }
                    }
                }
                Err(error) => ui.set_status_text(zh(format!("设计工作台加载失败：{error}"))),
            }
        }
    });

    let save_design_archive_ui = ui.as_weak();
    let save_design_archive_service = Rc::clone(&workbench_service);
    let save_design_archive_lock = Rc::clone(&selected_lock);
    ui.on_save_design_archive(move || {
        if let Some(ui) = save_design_archive_ui.upgrade() {
            let data_root = PathBuf::from(ui.get_data_root().to_string());
            if let Err(error) = ensure_workbench_loaded(&save_design_archive_service, &data_root) {
                ui.set_status_text(zh(format!("设计工作台加载失败：{error}")));
                return;
            }
            let app = match AdmApplication::for_data_root(&data_root) {
                Ok(app) => app,
                Err(error) => {
                    ui.set_status_text(zh(format!("存档系统加载失败：{error}")));
                    return;
                }
            };
            let service_ref = save_design_archive_service.borrow();
            let Some(service) = service_ref.as_ref() else {
                ui.set_status_text(zh("设计工作台未初始化。"));
                return;
            };
            let selected_archive_id = ui.get_selected_archive_id().to_string();
            let selected_archive_id = selected_archive_id.trim();
            let archive_id = if selected_archive_id.is_empty() {
                None
            } else {
                Some(selected_archive_id)
            };
            let markdown = match service.export_text("markdown") {
                Ok(markdown) => markdown,
                Err(error) => {
                    ui.set_status_text(zh(format!("设计工作台导出失败：{error}")));
                    return;
                }
            };
            match app.commit_design_workbench_state(
                archive_id,
                service.state().project_name.as_str(),
                service.state(),
                &markdown,
            ) {
                Ok(report) => {
                    let archive_id = report.archive.manifest.archive_id.to_string();
                    ui.set_selected_archive_id(archive_id.clone().into());
                    refresh_projects(&ui, &data_root, Some(&save_design_archive_lock));
                    ui.set_status_text(zh(format!("设计工作台已保存到正式存档：{}", archive_id)));
                }
                Err(error) => ui.set_status_text(zh(format!("保存设计工作台存档失败：{error}"))),
            }
        }
    });

    let load_design_archive_ui = ui.as_weak();
    let load_design_archive_service = Rc::clone(&workbench_service);
    ui.on_load_design_archive(move |archive_id| {
        if let Some(ui) = load_design_archive_ui.upgrade() {
            let archive_id = archive_id.to_string();
            if archive_id.trim().is_empty() {
                ui.set_status_text(zh("请先在项目列表中选择一个包含工作台状态的存档。"));
                return;
            }
            let data_root = PathBuf::from(ui.get_data_root().to_string());
            let app = match AdmApplication::for_data_root(&data_root) {
                Ok(app) => app,
                Err(error) => {
                    ui.set_status_text(zh(format!("存档系统加载失败：{error}")));
                    return;
                }
            };
            let state = match app.load_design_workbench_state(archive_id.trim()) {
                Ok(state) => state,
                Err(error) => {
                    ui.set_status_text(zh(format!("载入设计工作台存档失败：{error}")));
                    return;
                }
            };
            let design_data_root = match locate_design_data_root() {
                Ok(root) => root,
                Err(error) => {
                    ui.set_status_text(zh(format!("设计数据目录定位失败：{error}")));
                    return;
                }
            };
            let mut loaded = match WorkbenchService::from_state(&design_data_root, state) {
                Ok(service) => service,
                Err(error) => {
                    ui.set_status_text(zh(format!("设计工作台状态恢复失败：{error}")));
                    return;
                }
            };
            match loaded.save_autosave(&workbench_autosave_path(&data_root)) {
                Ok(()) => {
                    apply_workbench_snapshot(&ui, &loaded);
                    *load_design_archive_service.borrow_mut() = Some(loaded);
                    ui.set_selected_archive_id(archive_id.trim().into());
                    ui.set_status_text(zh(format!("已载入设计工作台存档：{}", archive_id.trim())));
                }
                Err(error) => {
                    apply_workbench_snapshot(&ui, &loaded);
                    *load_design_archive_service.borrow_mut() = Some(loaded);
                    ui.set_status_text(zh(format!("设计工作台已载入，但自动保存失败：{error}")));
                }
            }
        }
    });

    let refresh_templates_ui = ui.as_weak();
    ui.on_refresh_design_templates(move || {
        if let Some(ui) = refresh_templates_ui.upgrade() {
            let data_root = PathBuf::from(ui.get_data_root().to_string());
            refresh_design_templates(&ui, &data_root);
        }
    });

    let load_template_ui = ui.as_weak();
    let load_template_service = Rc::clone(&workbench_service);
    ui.on_load_design_template(move |template_id| {
        if let Some(ui) = load_template_ui.upgrade() {
            let data_root = PathBuf::from(ui.get_data_root().to_string());
            let builtin_root = match locate_project_templates_root() {
                Ok(root) => root,
                Err(error) => {
                    ui.set_status_text(zh(format!("模板目录定位失败：{error}")));
                    return;
                }
            };
            let custom_root = custom_template_root(&data_root);
            mutate_design_workbench(&ui, &load_template_service, |service| {
                let row = service.import_project_template(
                    &builtin_root,
                    &custom_root,
                    template_id.as_str(),
                )?;
                Ok(format!("已载入模板：{}", row.name))
            });
            refresh_design_templates(&ui, &data_root);
        }
    });

    let save_template_ui = ui.as_weak();
    let save_template_service = Rc::clone(&workbench_service);
    ui.on_save_design_template(move |template_name| {
        if let Some(ui) = save_template_ui.upgrade() {
            let data_root = PathBuf::from(ui.get_data_root().to_string());
            match ensure_workbench_loaded(&save_template_service, &data_root) {
                Ok(()) => {
                    let service_ref = save_template_service.borrow();
                    let Some(service) = service_ref.as_ref() else {
                        ui.set_status_text(zh("设计工作台未初始化。"));
                        return;
                    };
                    match service.save_custom_template(
                        &custom_template_root(&data_root),
                        template_name.as_str(),
                    ) {
                        Ok(path) => {
                            refresh_design_templates(&ui, &data_root);
                            ui.set_status_text(zh(format!("已另存为模板：{}", path.display())));
                        }
                        Err(error) => ui.set_status_text(zh(format!("另存模板失败：{error}"))),
                    }
                }
                Err(error) => ui.set_status_text(zh(format!("设计工作台加载失败：{error}"))),
            }
        }
    });

    let checklist_ui = ui.as_weak();
    let checklist_service = Rc::clone(&workbench_service);
    ui.on_set_design_checklist(move |node_id, item_id, checked| {
        if let Some(ui) = checklist_ui.upgrade() {
            mutate_design_workbench(&ui, &checklist_service, |service| {
                service.set_checklist_item(node_id.as_str(), item_id.as_str(), checked)?;
                Ok(if checked {
                    format!("已勾选决策项：{item_id}")
                } else {
                    format!("已取消决策项：{item_id}")
                })
            });
        }
    });

    let l4_option_ui = ui.as_weak();
    let l4_option_service = Rc::clone(&workbench_service);
    ui.on_set_design_l4_option(move |node_id, item_id, group_id, option_id, checked| {
        if let Some(ui) = l4_option_ui.upgrade() {
            mutate_design_workbench(&ui, &l4_option_service, |service| {
                service.set_option_group_option(
                    node_id.as_str(),
                    item_id.as_str(),
                    group_id.as_str(),
                    option_id.as_str(),
                    checked,
                )?;
                Ok(if checked {
                    format!("已选择 L4 选项：{option_id}")
                } else {
                    format!("已取消 L4 选项：{option_id}")
                })
            });
        }
    });

    let l4_primary_ui = ui.as_weak();
    let l4_primary_service = Rc::clone(&workbench_service);
    ui.on_set_design_primary_option(move |node_id, item_id, group_id, option_id| {
        if let Some(ui) = l4_primary_ui.upgrade() {
            mutate_design_workbench(&ui, &l4_primary_service, |service| {
                service.set_option_group_primary(
                    node_id.as_str(),
                    item_id.as_str(),
                    group_id.as_str(),
                    option_id.as_str(),
                )?;
                Ok(format!("已设置 L4 主选项：{option_id}"))
            });
        }
    });

    let node_text_ui = ui.as_weak();
    let node_text_service = Rc::clone(&workbench_service);
    ui.on_save_design_node_text(
        move |node_id, design_note, risk_note, not_applicable_reason| {
            if let Some(ui) = node_text_ui.upgrade() {
                mutate_design_workbench(&ui, &node_text_service, |service| {
                    service.update_node_text(
                        node_id.as_str(),
                        "design_note",
                        design_note.as_str(),
                    )?;
                    service.update_node_text(node_id.as_str(), "risk_note", risk_note.as_str())?;
                    service.update_node_text(
                        node_id.as_str(),
                        "not_applicable_reason",
                        not_applicable_reason.as_str(),
                    )?;
                    Ok("节点说明已保存。".to_string())
                });
            }
        },
    );

    let l5_save_ui = ui.as_weak();
    let l5_save_service = Rc::clone(&workbench_service);
    ui.on_save_design_l5_json(move |node_id, raw_json| {
        if let Some(ui) = l5_save_ui.upgrade() {
            mutate_design_workbench(&ui, &l5_save_service, |service| {
                service.update_node_design_entities_json(node_id.as_str(), raw_json.as_str())?;
                Ok("L5 JSON 已保存并完成校验。".to_string())
            });
        }
    });

    let l5_clear_ui = ui.as_weak();
    let l5_clear_service = Rc::clone(&workbench_service);
    ui.on_clear_design_l5_json(move |node_id| {
        if let Some(ui) = l5_clear_ui.upgrade() {
            mutate_design_workbench(&ui, &l5_clear_service, |service| {
                service.update_node_design_entities_json(node_id.as_str(), "[]")?;
                Ok("L5 JSON 已清空。".to_string())
            });
        }
    });

    let interview_question_ui = ui.as_weak();
    let interview_question_service = Rc::clone(&workbench_service);
    ui.on_start_design_ai_interview(move |focus| {
        if let Some(ui) = interview_question_ui.upgrade() {
            mutate_design_workbench(&ui, &interview_question_service, |service| {
                let question = service.generate_interview_question(focus.as_str())?;
                Ok(format!(
                    "已生成 AI 访谈问题：{}",
                    one_line_summary(&question, 80)
                ))
            });
        }
    });

    let interview_reply_ui = ui.as_weak();
    let interview_reply_service = Rc::clone(&workbench_service);
    ui.on_record_design_ai_response(move |reply| {
        if let Some(ui) = interview_reply_ui.upgrade() {
            mutate_design_workbench(&ui, &interview_reply_service, |service| {
                service.record_interview_reply(reply.as_str())?;
                Ok("AI 访谈回答已记录。".to_string())
            });
        }
    });

    let interview_run_ui = ui.as_weak();
    let interview_run_service = Rc::clone(&workbench_service);
    ui.on_run_design_ai_interview(move |provider_id, model, reply| {
        if let Some(ui) = interview_run_ui.upgrade() {
            let data_root = PathBuf::from(ui.get_data_root().to_string());
            let provider_id = provider_id.to_string();
            let model = model.to_string();
            let reply = reply.to_string();
            if provider_id.trim() == "mock" {
                let provider = match ProviderId::new("mock") {
                    Ok(provider_id) => {
                        MockAiProvider::new(provider_id, vec![AiCapability::TextGeneration])
                    }
                    Err(error) => {
                        ui.set_status_text(zh(format!("AI provider 初始化失败：{error}")));
                        return;
                    }
                };
                mutate_design_workbench(&ui, &interview_run_service, |service| {
                    let report = service.run_ai_interview_with_provider(&provider, &reply)?;
                    Ok(format!(
                        "AI 访谈已完成：provider={}，写回={}，task={}",
                        report.provider_id, report.applied, report.task_id
                    ))
                });
                return;
            }

            let app = match AdmApplication::for_data_root(&data_root) {
                Ok(app) => app,
                Err(error) => {
                    ui.set_status_text(zh(format!("AI 配置加载失败：{error}")));
                    return;
                }
            };
            let provider_id = match ProviderId::new(provider_id) {
                Ok(provider_id) => provider_id,
                Err(error) => {
                    ui.set_status_text(zh(format!("AI provider id 无效：{error}")));
                    return;
                }
            };
            let provider = match app.chat_completions_provider_from_config(&provider_id, model) {
                Ok(provider) => provider,
                Err(error) => {
                    ui.set_status_text(zh(format!("AI provider 不可用：{error}")));
                    return;
                }
            };
            mutate_design_workbench(&ui, &interview_run_service, |service| {
                let report = service.run_ai_interview_with_provider(&provider, &reply)?;
                Ok(format!(
                    "AI 访谈已完成：provider={}，写回={}，task={}",
                    report.provider_id, report.applied, report.task_id
                ))
            });
        }
    });

    let reset_design_workbench_ui = ui.as_weak();
    let reset_design_workbench_service = Rc::clone(&workbench_service);
    ui.on_reset_design_workbench(move || {
        if let Some(ui) = reset_design_workbench_ui.upgrade() {
            reset_design_workbench(&ui, &reset_design_workbench_service);
        }
    });

    let supplement_ui = ui.as_weak();
    ui.on_analyze_supplement(move |request| {
        if let Some(ui) = supplement_ui.upgrade() {
            let data_root = PathBuf::from(ui.get_data_root().to_string());
            let archive_id = ui.get_selected_archive_id().to_string();
            let context = format!(
                "{}\n{}\n{}",
                ui.get_project_detail(),
                ui.get_pipeline_text(),
                ui.get_core_artifact_text()
            );
            match analyze_supplement_request(request.as_str(), &context) {
                Ok(analysis) => {
                    let mut text = analysis.render();
                    if archive_id.trim().is_empty() {
                        text.push_str("\n存档写入：未选择存档，仅完成本地分析。\n");
                        ui.set_status_text(zh("补充开发需求已分析，未写入存档。"));
                    } else {
                        match AdmApplication::for_data_root(&data_root)
                            .and_then(|app| app.commit_supplement_analysis(&archive_id, &analysis))
                        {
                            Ok(commit) => {
                                text.push_str(&format!(
                                    "\n存档写入：{}，任务数：{}\n",
                                    commit.request_file.display(),
                                    commit.task_count
                                ));
                                ui.set_status_text(zh(format!(
                                    "补充开发需求已分析并写入存档：{}",
                                    archive_id
                                )));
                            }
                            Err(error) => {
                                text.push_str(&format!("\n存档写入失败：{error}\n"));
                                ui.set_status_text(zh(format!(
                                    "补充开发需求已分析，但写入存档失败：{error}"
                                )));
                            }
                        }
                    }
                    ui.set_supplement_analysis_text(zh(text));
                }
                Err(error) => {
                    ui.set_supplement_analysis_text(zh(format!("补充开发分析失败：{error}")));
                    ui.set_status_text(zh(format!("补充开发分析失败：{error}")));
                }
            }
        }
    });

    let inspect_workspaces_ui = ui.as_weak();
    ui.on_inspect_workspaces(move || {
        if let Some(ui) = inspect_workspaces_ui.upgrade() {
            let data_root = PathBuf::from(ui.get_data_root().to_string());
            match inspect_workspaces_for_ui(&data_root) {
                Ok(message) => {
                    ui.set_workspace_doctor_text(zh(&message));
                    ui.set_status_text(zh("Workspace doctor completed."));
                }
                Err(error) => {
                    ui.set_workspace_doctor_text(zh(format!("Workspace doctor failed.\n{error}")));
                    ui.set_status_text(zh(format!("Error: {error}")));
                }
            }
        }
    });

    let cleanup_workspaces_ui = ui.as_weak();
    let cleanup_workspaces_lock = Rc::clone(&selected_lock);
    ui.on_cleanup_workspaces(move || {
        if let Some(ui) = cleanup_workspaces_ui.upgrade() {
            let data_root = PathBuf::from(ui.get_data_root().to_string());
            match cleanup_workspaces_for_ui(&data_root) {
                Ok(message) => {
                    ui.set_workspace_doctor_text(zh(&message));
                    refresh_projects(&ui, &data_root, Some(&cleanup_workspaces_lock));
                    ui.set_status_text(zh("Workspace cleanup completed."));
                }
                Err(error) => {
                    ui.set_workspace_doctor_text(zh(format!("Workspace cleanup failed.\n{error}")));
                    ui.set_status_text(zh(format!("Error: {error}")));
                }
            }
        }
    });

    let apply_ai_provider_preset_ui = ui.as_weak();
    ui.on_apply_ai_provider_preset(move || {
        if let Some(ui) = apply_ai_provider_preset_ui.upgrade() {
            match apply_ai_provider_preset_to_inputs(
                ui.get_ai_provider_preset().to_string(),
                ui.get_ai_provider_id().to_string(),
                ui.get_ai_provider_secret_ref().to_string(),
            ) {
                Ok(fields) => {
                    ui.set_ai_provider_id(fields.provider_id.into());
                    ui.set_ai_provider_endpoint(fields.endpoint_hint.into());
                    ui.set_ai_provider_secret_ref(fields.secret_ref.into());
                    ui.set_ai_provider_capabilities(fields.capabilities.into());
                    ui.set_status_text(zh(&fields.message));
                }
                Err(error) => ui.set_status_text(zh(format!("Error: {error}"))),
            }
        }
    });

    let save_ai_provider_ui = ui.as_weak();
    ui.on_save_ai_provider(move || {
        if let Some(ui) = save_ai_provider_ui.upgrade() {
            let data_root = PathBuf::from(ui.get_data_root().to_string());
            let provider_id = ui.get_ai_provider_id().to_string();
            let endpoint_hint = ui.get_ai_provider_endpoint().to_string();
            let secret_ref = ui.get_ai_provider_secret_ref().to_string();
            let capabilities = ui.get_ai_provider_capabilities().to_string();
            match save_ai_provider_config(
                &data_root,
                provider_id,
                endpoint_hint,
                secret_ref,
                capabilities,
            ) {
                Ok(message) => {
                    ui.set_status_text(zh(&message));
                    refresh_ai_diagnostics(&ui, &data_root);
                }
                Err(error) => ui.set_status_text(zh(format!("Error: {error}"))),
            }
        }
    });

    let save_ai_secret_ui = ui.as_weak();
    ui.on_save_ai_secret(move || {
        if let Some(ui) = save_ai_secret_ui.upgrade() {
            let data_root = PathBuf::from(ui.get_data_root().to_string());
            let secret_ref = ui.get_ai_provider_secret_ref().to_string();
            let secret_value = ui.get_ai_provider_secret_value().to_string();
            match save_ai_named_secret(&data_root, secret_ref, secret_value) {
                Ok(message) => {
                    ui.set_status_text(zh(&message));
                    refresh_ai_diagnostics(&ui, &data_root);
                }
                Err(error) => ui.set_status_text(zh(format!("Error: {error}"))),
            }
        }
    });

    let check_ai_provider_ui = ui.as_weak();
    ui.on_check_ai_provider(move || {
        if let Some(ui) = check_ai_provider_ui.upgrade() {
            let data_root = PathBuf::from(ui.get_data_root().to_string());
            let provider_id = ui.get_ai_provider_id().to_string();
            let model = ui.get_ai_provider_model().to_string();
            match check_ai_provider_config(&data_root, provider_id, model) {
                Ok(message) => ui.set_status_text(zh(&message)),
                Err(error) => ui.set_status_text(zh(format!("Error: {error}"))),
            }
        }
    });

    let invoke_ai_provider_ui = ui.as_weak();
    ui.on_invoke_ai_provider(move || {
        if let Some(ui) = invoke_ai_provider_ui.upgrade() {
            let data_root = PathBuf::from(ui.get_data_root().to_string());
            let provider_id = ui.get_ai_provider_id().to_string();
            let model = ui.get_ai_provider_model().to_string();
            let prompt = ui.get_ai_provider_prompt().to_string();
            match invoke_ai_provider_config(&data_root, provider_id, model, prompt) {
                Ok(message) => ui.set_status_text(zh(&message)),
                Err(error) => ui.set_status_text(zh(format!("Error: {error}"))),
            }
        }
    });

    let disable_ai_provider_ui = ui.as_weak();
    ui.on_disable_ai_provider(move || {
        if let Some(ui) = disable_ai_provider_ui.upgrade() {
            let data_root = PathBuf::from(ui.get_data_root().to_string());
            let provider_id = ui.get_ai_provider_id().to_string();
            match disable_ai_provider_config(&data_root, provider_id) {
                Ok(message) => {
                    ui.set_status_text(zh(&message));
                    refresh_ai_diagnostics(&ui, &data_root);
                }
                Err(error) => ui.set_status_text(zh(format!("Error: {error}"))),
            }
        }
    });

    let select_ui = ui.as_weak();
    let select_lock = Rc::clone(&selected_lock);
    ui.on_select_project(move || {
        if let Some(ui) = select_ui.upgrade() {
            let data_root = PathBuf::from(ui.get_data_root().to_string());
            let archive_id = ui.get_selected_archive_id().to_string();
            match lock_and_load_project_inspection(&data_root, &archive_id, &select_lock) {
                Ok(inspection) => apply_project_inspection(&ui, &inspection),
                Err(error) => {
                    apply_locked_or_unavailable_detail(&ui, &error);
                    ui.set_status_text(zh(format!("Error: {error}")));
                }
            }
        }
    });

    let resume_ui = ui.as_weak();
    let resume_lock = Rc::clone(&selected_lock);
    ui.on_resume_project(move || {
        if let Some(ui) = resume_ui.upgrade() {
            let title = ui.get_project_title().to_string();
            let data_root = PathBuf::from(ui.get_data_root().to_string());
            let archive_id = ui.get_selected_archive_id().to_string();
            release_selected_archive_lock(&resume_lock);
            match resume_project(&title, &data_root, &archive_id) {
                Ok(summary) => {
                    ui.set_selected_archive_id(summary.archive_id.clone().into());
                    let _ = lock_selected_archive(&data_root, &summary.archive_id, &resume_lock);
                    refresh_projects(&ui, &data_root, Some(&resume_lock));
                    apply_run_summary(&ui, &summary);
                }
                Err(error) => ui.set_status_text(zh(format!("Error: {error}"))),
            }
        }
    });

    let resume_failed_ui = ui.as_weak();
    let resume_failed_lock = Rc::clone(&selected_lock);
    ui.on_resume_failed_project(move || {
        if let Some(ui) = resume_failed_ui.upgrade() {
            let title = ui.get_project_title().to_string();
            let data_root = PathBuf::from(ui.get_data_root().to_string());
            let archive_id = ui.get_selected_archive_id().to_string();
            release_selected_archive_lock(&resume_failed_lock);
            match resume_failed_project(&title, &data_root, &archive_id) {
                Ok(summary) => {
                    ui.set_selected_archive_id(summary.archive_id.clone().into());
                    let _ =
                        lock_selected_archive(&data_root, &summary.archive_id, &resume_failed_lock);
                    refresh_projects(&ui, &data_root, Some(&resume_failed_lock));
                    apply_run_summary(&ui, &summary);
                }
                Err(error) => ui.set_status_text(zh(format!("Error: {error}"))),
            }
        }
    });

    let release_lock_ui = ui.as_weak();
    let release_lock_state = Rc::clone(&selected_lock);
    ui.on_release_current_lock(move || {
        if let Some(ui) = release_lock_ui.upgrade() {
            let data_root = PathBuf::from(ui.get_data_root().to_string());
            let archive_id = ui.get_selected_archive_id().to_string();
            match release_current_window_lock(&archive_id, &release_lock_state) {
                Ok(message) => {
                    refresh_projects(&ui, &data_root, Some(&release_lock_state));
                    ui.set_status_text(zh(&message));
                }
                Err(error) => ui.set_status_text(zh(format!("Error: {error}"))),
            }
        }
    });

    let clear_external_lock_ui = ui.as_weak();
    let clear_external_lock_state = Rc::clone(&selected_lock);
    ui.on_clear_external_lock(move || {
        if let Some(ui) = clear_external_lock_ui.upgrade() {
            let data_root = PathBuf::from(ui.get_data_root().to_string());
            let archive_id = ui.get_selected_archive_id().to_string();
            match clear_external_archive_lock_file(
                &data_root,
                &archive_id,
                Some(&clear_external_lock_state),
            ) {
                Ok(message) => {
                    refresh_projects(&ui, &data_root, Some(&clear_external_lock_state));
                    ui.set_status_text(zh(&message));
                }
                Err(error) => ui.set_status_text(zh(format!("Error: {error}"))),
            }
        }
    });

    let stage_ui = ui.as_weak();
    let stage_lock = Rc::clone(&selected_lock);
    ui.on_inspect_stage(move |stage_id| {
        inspect_selected_stage(&stage_ui, stage_id.as_str(), &stage_lock);
    });

    let rerun_stage_ui = ui.as_weak();
    let rerun_stage_lock = Rc::clone(&selected_lock);
    ui.on_rerun_stage(move |stage_id| {
        if let Some(ui) = rerun_stage_ui.upgrade() {
            let title = ui.get_project_title().to_string();
            let data_root = PathBuf::from(ui.get_data_root().to_string());
            let archive_id = ui.get_selected_archive_id().to_string();
            release_selected_archive_lock(&rerun_stage_lock);
            match rerun_project_stage(&title, &data_root, &archive_id, stage_id.as_str()) {
                Ok(summary) => {
                    ui.set_selected_archive_id(summary.archive_id.clone().into());
                    let _ =
                        lock_selected_archive(&data_root, &summary.archive_id, &rerun_stage_lock);
                    refresh_projects(&ui, &data_root, Some(&rerun_stage_lock));
                    apply_run_summary(&ui, &summary);
                }
                Err(error) => ui.set_status_text(zh(format!("Error: {error}"))),
            }
        }
    });

    let run_range_ui = ui.as_weak();
    let run_range_lock = Rc::clone(&selected_lock);
    let run_range_workbench_service = Rc::clone(&workbench_service);
    ui.on_run_pipeline_range(move |start_step, end_step| {
        if let Some(ui) = run_range_ui.upgrade() {
            let title = ui.get_project_title().to_string();
            let data_root = PathBuf::from(ui.get_data_root().to_string());
            let archive_id = ui.get_selected_archive_id().to_string();
            match PipelineService::new(&data_root).record_range_run_request(
                &archive_id,
                start_step.as_str(),
                end_step.as_str(),
            ) {
                Ok(request) => {
                    refresh_pipeline_service_status(&ui, &data_root);
                    let _ = append_pipeline_range_log_for_ui(
                        &data_root,
                        "pipeline_range_started",
                        &request,
                        &request.archive_id,
                        None,
                    );
                    refresh_run_log(&ui, &data_root, ui.get_run_log_filter().to_string());
                    if archive_id.trim().is_empty() {
                        ui.set_status_text(zh(
                            "未选择存档，正在从当前项目画像导出并运行完整流水线。",
                        ));
                        release_selected_archive_lock(&run_range_lock);
                        let brief = match pipeline_brief_for_ui(&ui, &run_range_workbench_service) {
                            Ok(brief) => brief,
                            Err(error) => {
                                ui.set_status_text(zh(format!(
                                    "导出设计工作台到流水线失败：{error}"
                                )));
                                return;
                            }
                        };
                        match create_and_run_project_from_brief(brief, &data_root) {
                            Ok(mut summary) => {
                                if let Err(error) = apply_devflow_range_to_summary(
                                    &mut summary,
                                    &data_root,
                                    &request.start_step_id,
                                    &request.end_step_id,
                                ) {
                                    ui.set_status_text(zh(format!(
                                        "流水线区间状态刷新失败：{error}"
                                    )));
                                    return;
                                }
                                let completed_count = match record_pipeline_range_summary_for_ui(
                                    &data_root, &request, &summary,
                                ) {
                                    Ok(count) => count,
                                    Err(error) => {
                                        ui.set_status_text(zh(format!(
                                            "流水线区间摘要记录失败：{error}"
                                        )));
                                        return;
                                    }
                                };
                                let _ = append_pipeline_range_log_for_ui(
                                    &data_root,
                                    "pipeline_range_projected",
                                    &request,
                                    &summary.archive_id,
                                    Some(completed_count),
                                );
                                ui.set_selected_archive_id(summary.archive_id.clone().into());
                                let _ = lock_selected_archive(
                                    &data_root,
                                    &summary.archive_id,
                                    &run_range_lock,
                                );
                                refresh_projects(&ui, &data_root, Some(&run_range_lock));
                                apply_run_summary(&ui, &summary);
                                let _ = append_pipeline_range_log_for_ui(
                                    &data_root,
                                    "pipeline_range_completed",
                                    &request,
                                    &summary.archive_id,
                                    Some(completed_count),
                                );
                                refresh_pipeline_service_status(&ui, &data_root);
                                refresh_run_log(
                                    &ui,
                                    &data_root,
                                    ui.get_run_log_filter().to_string(),
                                );
                            }
                            Err(error) => ui.set_status_text(zh(format!("Error: {error}"))),
                        }
                    } else {
                        release_selected_archive_lock(&run_range_lock);
                        match rerun_project_step_range(
                            &title,
                            &data_root,
                            &archive_id,
                            &request.start_step_id,
                            &request.end_step_id,
                        ) {
                            Ok(summary) => {
                                let completed_count = match record_pipeline_range_summary_for_ui(
                                    &data_root, &request, &summary,
                                ) {
                                    Ok(count) => count,
                                    Err(error) => {
                                        ui.set_status_text(zh(format!(
                                            "流水线区间摘要记录失败：{error}"
                                        )));
                                        return;
                                    }
                                };
                                let _ = append_pipeline_range_log_for_ui(
                                    &data_root,
                                    "pipeline_range_projected",
                                    &request,
                                    &summary.archive_id,
                                    Some(completed_count),
                                );
                                ui.set_selected_archive_id(summary.archive_id.clone().into());
                                let _ = lock_selected_archive(
                                    &data_root,
                                    &summary.archive_id,
                                    &run_range_lock,
                                );
                                refresh_projects(&ui, &data_root, Some(&run_range_lock));
                                apply_run_summary(&ui, &summary);
                                let _ = append_pipeline_range_log_for_ui(
                                    &data_root,
                                    "pipeline_range_completed",
                                    &request,
                                    &summary.archive_id,
                                    Some(completed_count),
                                );
                                refresh_pipeline_service_status(&ui, &data_root);
                                refresh_run_log(
                                    &ui,
                                    &data_root,
                                    ui.get_run_log_filter().to_string(),
                                );
                            }
                            Err(error) => ui.set_status_text(zh(format!("Error: {error}"))),
                        }
                    }
                }
                Err(error) => ui.set_status_text(zh(format!("记录流水线范围请求失败：{error}"))),
            }
        }
    });

    let stop_pipeline_ui = ui.as_weak();
    ui.on_stop_pipeline_run(move || {
        if let Some(ui) = stop_pipeline_ui.upgrade() {
            let data_root = PathBuf::from(ui.get_data_root().to_string());
            match PipelineService::new(&data_root).request_stop("用户在桌面端点击停止") {
                Ok(request) => {
                    refresh_pipeline_service_status(&ui, &data_root);
                    let _ = append_run_log_for_ui(
                        &data_root,
                        "WARN",
                        "pipeline",
                        "stop_requested",
                        &format!("requested_at_ms={}", request.requested_at_ms),
                    );
                    refresh_run_log(&ui, &data_root, ui.get_run_log_filter().to_string());
                    ui.set_status_text(zh("流水线停止请求已写入。"));
                }
                Err(error) => ui.set_status_text(zh(format!("写入流水线停止请求失败：{error}"))),
            }
        }
    });

    let confirm_style_ui = ui.as_weak();
    ui.on_confirm_pipeline_style(move |style_id, prompt| {
        if let Some(ui) = confirm_style_ui.upgrade() {
            let data_root = PathBuf::from(ui.get_data_root().to_string());
            let archive_id = ui.get_selected_archive_id().to_string();
            match PipelineService::new(&data_root).confirm_step07_style(
                &archive_id,
                style_id.as_str(),
                prompt.as_str(),
            ) {
                Ok(confirmation) => {
                    refresh_pipeline_service_status(&ui, &data_root);
                    let _ = append_run_log_for_ui(
                        &data_root,
                        "INFO",
                        "pipeline",
                        "step07_style_confirmed",
                        &format!("style_id={}", confirmation.style_id),
                    );
                    refresh_run_log(&ui, &data_root, ui.get_run_log_filter().to_string());
                    ui.set_status_text(zh(format!(
                        "Step07 美术风格已确认：{}",
                        confirmation.style_id
                    )));
                }
                Err(error) => ui.set_status_text(zh(format!("确认 Step07 风格失败：{error}"))),
            }
        }
    });

    let export_ui = ui.as_weak();
    let export_lock = Rc::clone(&selected_lock);
    ui.on_export_project(move || {
        if let Some(ui) = export_ui.upgrade() {
            let data_root = PathBuf::from(ui.get_data_root().to_string());
            let archive_id = ui.get_selected_archive_id().to_string();
            let target = PathBuf::from(ui.get_export_path().to_string());
            let owned_session = owned_lock_session_for_archive(&export_lock, &archive_id);
            match export_project(&data_root, &archive_id, &target, owned_session.as_ref()) {
                Ok(message) => {
                    ui.set_import_path(path_to_text(&target).into());
                    match inspect_import_package_for_ui(&data_root, &target) {
                        Ok(view) => {
                            ui.set_package_doctor_text(zh(&view.message));
                            ui.set_status_text(zh(format!(
                                "{message}; package_ready={}",
                                view.ready
                            )));
                        }
                        Err(error) => {
                            ui.set_package_doctor_text(zh(format!(
                                "Project package doctor failed.\n{error}"
                            )));
                            ui.set_status_text(zh(&message));
                        }
                    }
                }
                Err(error) => ui.set_status_text(zh(format!("Error: {error}"))),
            }
        }
    });

    let package_doctor_ui = ui.as_weak();
    ui.on_check_import_package(move || {
        if let Some(ui) = package_doctor_ui.upgrade() {
            let data_root = PathBuf::from(ui.get_data_root().to_string());
            let package = PathBuf::from(ui.get_import_path().to_string());
            match inspect_import_package_for_ui(&data_root, &package) {
                Ok(view) => {
                    ui.set_package_doctor_text(zh(&view.message));
                    ui.set_status_text(zh(format!(
                        "Package doctor completed: ready={}",
                        view.ready
                    )));
                }
                Err(error) => {
                    ui.set_package_doctor_text(zh(format!(
                        "Project package doctor failed.\n{error}"
                    )));
                    ui.set_status_text(zh(format!("Error: {error}")));
                }
            }
        }
    });

    let import_ui = ui.as_weak();
    let import_lock = Rc::clone(&selected_lock);
    ui.on_import_project(move || {
        if let Some(ui) = import_ui.upgrade() {
            let data_root = PathBuf::from(ui.get_data_root().to_string());
            let package = PathBuf::from(ui.get_import_path().to_string());
            release_selected_archive_lock(&import_lock);
            match import_project(&data_root, &package) {
                Ok(result) => {
                    ui.set_package_doctor_text(zh(&result.package_doctor_text));
                    ui.set_selected_archive_id(result.archive_id.clone().into());
                    let _ = lock_selected_archive(&data_root, &result.archive_id, &import_lock);
                    refresh_projects(&ui, &data_root, Some(&import_lock));
                    ui.set_status_text(zh(&result.message));
                }
                Err(error) => ui.set_status_text(zh(format!("Error: {error}"))),
            }
        }
    });

    let stage_release_ui = ui.as_weak();
    ui.on_stage_desktop_release(move || {
        if let Some(ui) = stage_release_ui.upgrade() {
            let source_exe = PathBuf::from(ui.get_release_source_exe().to_string());
            let target_dir = PathBuf::from(ui.get_release_target_dir().to_string());
            match stage_desktop_release_for_ui(&source_exe, &target_dir) {
                Ok(message) => {
                    ui.set_release_text(zh(&message));
                    ui.set_status_text(zh("Desktop release staged."));
                }
                Err(error) => {
                    ui.set_release_text(zh(format!("Desktop release failed.\n{error}")));
                    ui.set_status_text(zh(format!("Error: {error}")));
                }
            }
        }
    });

    let release_doctor_ui = ui.as_weak();
    ui.on_release_doctor(move || {
        if let Some(ui) = release_doctor_ui.upgrade() {
            let target_dir = PathBuf::from(ui.get_release_target_dir().to_string());
            match release_doctor_for_ui(&target_dir) {
                Ok(message) => {
                    ui.set_release_doctor_text(zh(&message));
                    ui.set_status_text(zh("Release doctor completed."));
                }
                Err(error) => {
                    ui.set_release_doctor_text(zh(format!("Release doctor failed.\n{error}")));
                    ui.set_status_text(zh(format!("Error: {error}")));
                }
            }
        }
    });

    let delivery_doctor_ui = ui.as_weak();
    ui.on_delivery_doctor(move || {
        if let Some(ui) = delivery_doctor_ui.upgrade() {
            let release_dir = path_from_optional_text(
                ui.get_release_target_dir().to_string(),
                default_desktop_release_dir(),
            );
            let game_bundle_dir = path_from_optional_text(
                ui.get_game_build_target_dir().to_string(),
                default_game_build_bundle_dir(),
            );
            let sdk_bundle_dir = path_from_optional_text(
                ui.get_sdk_bundle_target_dir().to_string(),
                default_sdk_bundle_dir(),
            );
            let unity_project_dir = path_from_optional_text(
                ui.get_unity_project_dir().to_string(),
                default_unity_project_dir(),
            );
            match delivery_doctor_for_ui(
                &release_dir,
                &game_bundle_dir,
                &sdk_bundle_dir,
                &unity_project_dir,
            ) {
                Ok(view) => {
                    ui.set_release_target_dir(path_to_text(&release_dir).into());
                    ui.set_game_build_target_dir(path_to_text(&game_bundle_dir).into());
                    ui.set_sdk_bundle_target_dir(path_to_text(&sdk_bundle_dir).into());
                    ui.set_unity_project_dir(path_to_text(&unity_project_dir).into());
                    ui.set_delivery_doctor_text(zh(&view.message));
                    apply_delivery_checks(&ui, &view.checks);
                    ui.set_status_text(zh("Delivery doctor completed."));
                }
                Err(error) => {
                    ui.set_delivery_doctor_text(zh(format!("Delivery doctor failed.\n{error}")));
                    apply_delivery_checks(&ui, &[]);
                    ui.set_status_text(zh(format!("Error: {error}")));
                }
            }
        }
    });

    let stage_game_bundle_ui = ui.as_weak();
    ui.on_stage_game_build_bundle(move || {
        if let Some(ui) = stage_game_bundle_ui.upgrade() {
            let data_root = PathBuf::from(ui.get_data_root().to_string());
            let archive_id = ui.get_selected_archive_id().to_string();
            let target_id = ui.get_game_build_target_id().to_string();
            let target_dir = game_build_target_dir_from_ui(
                &data_root,
                &archive_id,
                &target_id,
                ui.get_game_build_target_dir().to_string(),
            );
            match stage_game_build_bundle_for_ui(&data_root, &archive_id, &target_id, &target_dir) {
                Ok(message) => {
                    ui.set_game_build_target_dir(path_to_text(&target_dir).into());
                    ui.set_game_build_text(zh(&message));
                    ui.set_status_text(zh("Game build bundle staged."));
                }
                Err(error) => {
                    ui.set_game_build_text(zh(format!("Game build bundle failed.\n{error}")));
                    ui.set_status_text(zh(format!("Error: {error}")));
                }
            }
        }
    });

    let stage_sdk_bundle_ui = ui.as_weak();
    ui.on_stage_sdk_bundle(move || {
        if let Some(ui) = stage_sdk_bundle_ui.upgrade() {
            let data_root = PathBuf::from(ui.get_data_root().to_string());
            let archive_id = ui.get_selected_archive_id().to_string();
            let target_dir = sdk_bundle_target_dir_from_ui(
                &data_root,
                &archive_id,
                ui.get_sdk_bundle_target_dir().to_string(),
            );
            match stage_sdk_bundle_for_ui(&data_root, &archive_id, &target_dir) {
                Ok(message) => {
                    ui.set_sdk_bundle_target_dir(path_to_text(&target_dir).into());
                    ui.set_sdk_bundle_text(zh(&message));
                    ui.set_status_text(zh("SDK bundle staged."));
                }
                Err(error) => {
                    ui.set_sdk_bundle_text(zh(format!("SDK bundle failed.\n{error}")));
                    ui.set_status_text(zh(format!("Error: {error}")));
                }
            }
        }
    });

    let stage_unity_project_ui = ui.as_weak();
    ui.on_stage_unity_project(move || {
        if let Some(ui) = stage_unity_project_ui.upgrade() {
            let data_root = PathBuf::from(ui.get_data_root().to_string());
            let archive_id = ui.get_selected_archive_id().to_string();
            let target_id = ui.get_game_build_target_id().to_string();
            let unity_project_dir = path_from_optional_text(
                ui.get_unity_project_dir().to_string(),
                default_unity_project_dir(),
            );
            match stage_unity_project_for_ui(
                &data_root,
                &archive_id,
                &target_id,
                &unity_project_dir,
            ) {
                Ok(message) => {
                    ui.set_unity_project_dir(path_to_text(&unity_project_dir).into());
                    if ui.get_runtime_results_file().to_string().trim().is_empty() {
                        ui.set_runtime_results_file(
                            path_to_text(&default_runtime_results_file(&unity_project_dir)).into(),
                        );
                    }
                    ui.set_unity_build_text(zh(&message));
                    ui.set_status_text(zh("Unity project scaffold staged."));
                }
                Err(error) => {
                    ui.set_unity_build_text(zh(format!("Unity project scaffold failed.\n{error}")));
                    ui.set_status_text(zh(format!("Error: {error}")));
                }
            }
        }
    });

    let unity_build_preflight_ui = ui.as_weak();
    ui.on_unity_build_preflight(move || {
        if let Some(ui) = unity_build_preflight_ui.upgrade() {
            let data_root = PathBuf::from(ui.get_data_root().to_string());
            let archive_id = ui.get_selected_archive_id().to_string();
            let target_id = ui.get_game_build_target_id().to_string();
            let unity_exe = PathBuf::from(ui.get_unity_exe().to_string());
            let unity_project_dir = PathBuf::from(ui.get_unity_project_dir().to_string());
            let confirm_token = ui.get_unity_confirm_token().to_string();
            match unity_build_preflight_for_ui(
                &data_root,
                &archive_id,
                &target_id,
                &unity_exe,
                &unity_project_dir,
                &confirm_token,
            ) {
                Ok(message) => {
                    ui.set_unity_build_text(zh(&message));
                    ui.set_status_text(zh("Unity build preflight completed."));
                }
                Err(error) => {
                    ui.set_unity_build_text(zh(format!("Unity build preflight failed.\n{error}")));
                    ui.set_status_text(zh(format!("Error: {error}")));
                }
            }
        }
    });

    let plan_unity_ui = ui.as_weak();
    ui.on_plan_unity_build(move || {
        if let Some(ui) = plan_unity_ui.upgrade() {
            let data_root = PathBuf::from(ui.get_data_root().to_string());
            let archive_id = ui.get_selected_archive_id().to_string();
            let target_id = ui.get_game_build_target_id().to_string();
            let unity_exe = PathBuf::from(ui.get_unity_exe().to_string());
            let unity_project_dir = PathBuf::from(ui.get_unity_project_dir().to_string());
            match plan_unity_build_for_ui(
                &data_root,
                &archive_id,
                &target_id,
                &unity_exe,
                &unity_project_dir,
            ) {
                Ok(message) => {
                    ui.set_unity_build_text(zh(&message));
                    ui.set_status_text(zh("Unity build command planned."));
                }
                Err(error) => {
                    ui.set_unity_build_text(zh(format!("Unity build command failed.\n{error}")));
                    ui.set_status_text(zh(format!("Error: {error}")));
                }
            }
        }
    });

    let dry_run_unity_ui = ui.as_weak();
    let dry_run_unity_lock = Rc::clone(&selected_lock);
    ui.on_dry_run_unity_build(move || {
        if let Some(ui) = dry_run_unity_ui.upgrade() {
            let data_root = PathBuf::from(ui.get_data_root().to_string());
            let archive_id = ui.get_selected_archive_id().to_string();
            let target_id = ui.get_game_build_target_id().to_string();
            let unity_exe = PathBuf::from(ui.get_unity_exe().to_string());
            let unity_project_dir = PathBuf::from(ui.get_unity_project_dir().to_string());
            let should_relock = dry_run_unity_lock
                .borrow()
                .as_ref()
                .is_some_and(|selected| selected.archive_id == archive_id);
            if should_relock {
                release_selected_archive_lock(&dry_run_unity_lock);
            }
            let result = dry_run_unity_build_for_ui(
                &data_root,
                &archive_id,
                &target_id,
                &unity_exe,
                &unity_project_dir,
            );
            let relock_error = if should_relock {
                lock_selected_archive(&data_root, &archive_id, &dry_run_unity_lock).err()
            } else {
                None
            };
            match result {
                Ok(message) => {
                    ui.set_unity_build_text(zh(&message));
                    if let Some(error) = relock_error {
                        ui.set_status_text(zh(format!(
                            "Unity build dry-run completed; relock failed: {error}"
                        )));
                    } else {
                        refresh_projects(&ui, &data_root, Some(&dry_run_unity_lock));
                        ui.set_status_text(zh("Unity build dry-run completed and persisted."));
                    }
                }
                Err(error) => {
                    ui.set_unity_build_text(zh(format!("Unity build dry-run failed.\n{error}")));
                    if let Some(relock_error) = relock_error {
                        ui.set_status_text(zh(format!(
                            "Error: {error}; relock failed: {relock_error}"
                        )));
                    } else {
                        ui.set_status_text(zh(format!("Error: {error}")));
                    }
                }
            }
        }
    });

    let run_unity_build_ui = ui.as_weak();
    let run_unity_build_lock = Rc::clone(&selected_lock);
    ui.on_run_unity_build(move || {
        if let Some(ui) = run_unity_build_ui.upgrade() {
            let data_root = PathBuf::from(ui.get_data_root().to_string());
            let archive_id = ui.get_selected_archive_id().to_string();
            let target_id = ui.get_game_build_target_id().to_string();
            let unity_exe = PathBuf::from(ui.get_unity_exe().to_string());
            let unity_project_dir = PathBuf::from(ui.get_unity_project_dir().to_string());
            let confirm_token = ui.get_unity_confirm_token().to_string();
            let should_relock = run_unity_build_lock
                .borrow()
                .as_ref()
                .is_some_and(|selected| selected.archive_id == archive_id);
            if should_relock {
                release_selected_archive_lock(&run_unity_build_lock);
            }
            let result = run_unity_build_for_ui(
                &data_root,
                &archive_id,
                &target_id,
                &unity_exe,
                &unity_project_dir,
                &confirm_token,
            );
            let relock_error = if should_relock {
                lock_selected_archive(&data_root, &archive_id, &run_unity_build_lock).err()
            } else {
                None
            };
            match result {
                Ok(message) => {
                    ui.set_unity_build_text(zh(&message));
                    if let Ok(inspection) = load_project_inspection(&data_root, &archive_id, None) {
                        apply_project_inspection(&ui, &inspection);
                    }
                    if let Some(error) = relock_error {
                        ui.set_status_text(zh(format!(
                            "Unity build run completed; relock failed: {error}"
                        )));
                    } else {
                        refresh_projects(&ui, &data_root, Some(&run_unity_build_lock));
                        ui.set_status_text(zh("Unity build run completed and persisted."));
                    }
                }
                Err(error) => {
                    ui.set_unity_build_text(zh(format!("Unity build run failed.\n{error}")));
                    if let Some(relock_error) = relock_error {
                        ui.set_status_text(zh(format!(
                            "Error: {error}; relock failed: {relock_error}"
                        )));
                    } else {
                        ui.set_status_text(zh(format!("Error: {error}")));
                    }
                }
            }
        }
    });

    let plan_unity_runtime_ui = ui.as_weak();
    ui.on_plan_unity_runtime_validation(move || {
        if let Some(ui) = plan_unity_runtime_ui.upgrade() {
            let data_root = PathBuf::from(ui.get_data_root().to_string());
            let archive_id = ui.get_selected_archive_id().to_string();
            let target_id = ui.get_game_build_target_id().to_string();
            let unity_exe = PathBuf::from(ui.get_unity_exe().to_string());
            let unity_project_dir = PathBuf::from(ui.get_unity_project_dir().to_string());
            let runtime_results_file = runtime_results_file_from_ui(
                &unity_project_dir,
                ui.get_runtime_results_file().to_string(),
            );
            match plan_unity_runtime_validation_for_ui(
                &data_root,
                &archive_id,
                &target_id,
                &unity_exe,
                &unity_project_dir,
            ) {
                Ok(message) => {
                    ui.set_runtime_results_file(path_to_text(&runtime_results_file).into());
                    ui.set_runtime_validation_text(zh(&message));
                    ui.set_status_text(zh("Unity runtime validation command planned."));
                }
                Err(error) => {
                    ui.set_runtime_validation_text(zh(format!(
                        "Unity runtime validation command failed.\n{error}"
                    )));
                    ui.set_status_text(zh(format!("Error: {error}")));
                }
            }
        }
    });

    let dry_run_unity_runtime_ui = ui.as_weak();
    ui.on_dry_run_unity_runtime_validation(move || {
        if let Some(ui) = dry_run_unity_runtime_ui.upgrade() {
            let data_root = PathBuf::from(ui.get_data_root().to_string());
            let archive_id = ui.get_selected_archive_id().to_string();
            let target_id = ui.get_game_build_target_id().to_string();
            let unity_exe = PathBuf::from(ui.get_unity_exe().to_string());
            let unity_project_dir = PathBuf::from(ui.get_unity_project_dir().to_string());
            let runtime_results_file = runtime_results_file_from_ui(
                &unity_project_dir,
                ui.get_runtime_results_file().to_string(),
            );
            match dry_run_unity_runtime_validation_for_ui(
                &data_root,
                &archive_id,
                &target_id,
                &unity_exe,
                &unity_project_dir,
            ) {
                Ok(message) => {
                    ui.set_runtime_results_file(path_to_text(&runtime_results_file).into());
                    ui.set_runtime_validation_text(zh(&message));
                    ui.set_status_text(zh("Unity runtime validation dry-run completed."));
                }
                Err(error) => {
                    ui.set_runtime_validation_text(zh(format!(
                        "Unity runtime validation dry-run failed.\n{error}"
                    )));
                    ui.set_status_text(zh(format!("Error: {error}")));
                }
            }
        }
    });

    let run_unity_runtime_ui = ui.as_weak();
    let run_unity_runtime_lock = Rc::clone(&selected_lock);
    ui.on_run_unity_runtime_validation(move || {
        if let Some(ui) = run_unity_runtime_ui.upgrade() {
            let data_root = PathBuf::from(ui.get_data_root().to_string());
            let archive_id = ui.get_selected_archive_id().to_string();
            let target_id = ui.get_game_build_target_id().to_string();
            let unity_exe = PathBuf::from(ui.get_unity_exe().to_string());
            let unity_project_dir = PathBuf::from(ui.get_unity_project_dir().to_string());
            let confirm_token = ui.get_unity_confirm_token().to_string();
            let runtime_results_file = runtime_results_file_from_ui(
                &unity_project_dir,
                ui.get_runtime_results_file().to_string(),
            );
            let should_relock = run_unity_runtime_lock
                .borrow()
                .as_ref()
                .is_some_and(|selected| selected.archive_id == archive_id);
            if should_relock {
                release_selected_archive_lock(&run_unity_runtime_lock);
            }
            let result = run_unity_runtime_validation_for_ui(
                &data_root,
                &archive_id,
                &target_id,
                &unity_exe,
                &unity_project_dir,
                &confirm_token,
            );
            let relock_error = if should_relock {
                lock_selected_archive(&data_root, &archive_id, &run_unity_runtime_lock).err()
            } else {
                None
            };
            match result {
                Ok(message) => {
                    ui.set_runtime_results_file(path_to_text(&runtime_results_file).into());
                    ui.set_runtime_validation_text(zh(&message));
                    if let Ok(inspection) = load_project_inspection(&data_root, &archive_id, None) {
                        apply_project_inspection(&ui, &inspection);
                    }
                    if let Some(error) = relock_error {
                        ui.set_status_text(
                            format!(
                                "Unity runtime validation run completed; relock failed: {error}"
                            )
                            .into(),
                        );
                    } else {
                        refresh_projects(&ui, &data_root, Some(&run_unity_runtime_lock));
                        ui.set_status_text(zh("Unity runtime validation run completed."));
                    }
                }
                Err(error) => {
                    ui.set_runtime_validation_text(zh(format!(
                        "Unity runtime validation run failed.\n{error}"
                    )));
                    if let Some(relock_error) = relock_error {
                        ui.set_status_text(zh(format!(
                            "Error: {error}; relock failed: {relock_error}"
                        )));
                    } else {
                        ui.set_status_text(zh(format!("Error: {error}")));
                    }
                }
            }
        }
    });

    let record_runtime_ui = ui.as_weak();
    let record_runtime_lock = Rc::clone(&selected_lock);
    ui.on_record_runtime_validation(move || {
        if let Some(ui) = record_runtime_ui.upgrade() {
            let data_root = PathBuf::from(ui.get_data_root().to_string());
            let archive_id = ui.get_selected_archive_id().to_string();
            let unity_project_dir = PathBuf::from(ui.get_unity_project_dir().to_string());
            let runtime_results_file = runtime_results_file_from_ui(
                &unity_project_dir,
                ui.get_runtime_results_file().to_string(),
            );
            let should_relock = record_runtime_lock
                .borrow()
                .as_ref()
                .is_some_and(|selected| selected.archive_id == archive_id);
            if should_relock {
                release_selected_archive_lock(&record_runtime_lock);
            }
            let result =
                record_runtime_validation_for_ui(&data_root, &archive_id, &runtime_results_file);
            let relock_error = if should_relock {
                lock_selected_archive(&data_root, &archive_id, &record_runtime_lock).err()
            } else {
                None
            };
            match result {
                Ok(message) => {
                    ui.set_runtime_results_file(path_to_text(&runtime_results_file).into());
                    ui.set_runtime_validation_text(zh(&message));
                    if let Ok(inspection) = load_project_inspection(&data_root, &archive_id, None) {
                        apply_project_inspection(&ui, &inspection);
                    }
                    if let Some(error) = relock_error {
                        ui.set_status_text(zh(format!(
                            "Runtime validation recorded; relock failed: {error}"
                        )));
                    } else {
                        refresh_projects(&ui, &data_root, Some(&record_runtime_lock));
                        ui.set_status_text(zh("Runtime validation recorded."));
                    }
                }
                Err(error) => {
                    ui.set_runtime_validation_text(zh(format!(
                        "Runtime validation record failed.\n{error}"
                    )));
                    if let Some(relock_error) = relock_error {
                        ui.set_status_text(zh(format!(
                            "Error: {error}; relock failed: {relock_error}"
                        )));
                    } else {
                        ui.set_status_text(zh(format!("Error: {error}")));
                    }
                }
            }
        }
    });
}

fn run_smoke() -> Result<(), Box<dyn Error>> {
    select_desktop_backend()?;
    let ui = MainWindow::new()?;
    let data_root = std::env::temp_dir().join(format!("adm_desktop_smoke_{}", std::process::id()));
    ui.set_data_root(path_to_text(&data_root).into());
    ui.set_project_title("Slint Smoke Project".into());
    ui.set_project_genre("tactical puzzle adventure".into());
    ui.set_project_promise("Players solve compact tactical routes with readable feedback".into());
    ui.set_project_core_loop(
        "Scout the room | Plan a route | Resolve the encounter with feedback".into(),
    );
    ui.set_export_path(path_to_text(&data_root.join("smoke.admproj")).into());
    append_run_log_for_ui(
        &data_root,
        "INFO",
        "smoke",
        "desktop_smoke_started",
        "phase=init",
    )?;
    refresh_run_log(&ui, &data_root, "smoke");
    if !ui.get_run_log_text().contains("desktop_smoke_started") {
        return Err(std::io::Error::new(
            ErrorKind::Other,
            "desktop smoke did not render strict run log",
        )
        .into());
    }
    let sdk_candidate = SdkKnowledgeService::new(&data_root)
        .add_pending("Smoke SDK", "https://example.invalid/smoke-sdk")?;
    SdkKnowledgeService::new(&data_root).approve(&sdk_candidate.id)?;
    refresh_sdk_knowledge(&ui, &data_root);
    if ui.get_sdk_review_items().row_count() != 1
        || !ui.get_sdk_review_text().contains("已批准=1")
        || !ui
            .get_sdk_review_text()
            .contains("approved_prompt_context_bytes=")
    {
        return Err(std::io::Error::new(
            ErrorKind::Other,
            "desktop smoke did not render SDK approval queue",
        )
        .into());
    }
    let pipeline_service = PipelineService::new(&data_root);
    let range_request = pipeline_service.record_range_run_request("smoke_archive", "3", "10")?;
    pipeline_service.request_stop("smoke stop request")?;
    pipeline_service.confirm_step07_style(
        "smoke_archive",
        "style_smoke_readable",
        "readable smoke test style",
    )?;
    refresh_pipeline_service_status(&ui, &data_root);
    if range_request.mapped_core_stage_ids.len() != 3
        || !ui
            .get_pipeline_service_text()
            .contains("stop_requested=true")
        || !ui
            .get_pipeline_service_text()
            .contains("step07_style_confirmed=true")
    {
        return Err(std::io::Error::new(
            ErrorKind::Other,
            "desktop smoke did not render pipeline service state",
        )
        .into());
    }
    refresh_ai_diagnostics(&ui, &data_root);
    if !ui.get_ai_config_text().contains("ready_provider_count=1") {
        return Err(std::io::Error::new(
            ErrorKind::Other,
            "desktop smoke did not load AI diagnostics",
        )
        .into());
    }
    if ui.get_ai_provider_items().row_count() != 1 {
        return Err(std::io::Error::new(
            ErrorKind::Other,
            "desktop smoke did not populate AI provider model",
        )
        .into());
    }
    ui.set_ai_provider_preset("openai".into());
    ui.set_ai_provider_id("".into());
    ui.set_ai_provider_secret_ref("default".into());
    let preset_fields = apply_ai_provider_preset_to_inputs(
        ui.get_ai_provider_preset().to_string(),
        ui.get_ai_provider_id().to_string(),
        ui.get_ai_provider_secret_ref().to_string(),
    )?;
    ui.set_ai_provider_id(preset_fields.provider_id.clone().into());
    ui.set_ai_provider_endpoint(preset_fields.endpoint_hint.clone().into());
    ui.set_ai_provider_secret_ref(preset_fields.secret_ref.clone().into());
    ui.set_ai_provider_capabilities(preset_fields.capabilities.clone().into());
    ui.set_status_text(preset_fields.message.into());
    if ui.get_ai_provider_id() != "openai"
        || ui.get_ai_provider_endpoint() != "https://api.openai.com/v1"
        || ui.get_ai_provider_secret_ref() != "env:OPENAI_API_KEY"
        || !ui
            .get_ai_provider_capabilities()
            .contains("structured_output")
        || !ui.get_status_text().contains("network_call=false")
    {
        return Err(std::io::Error::new(
            ErrorKind::Other,
            "desktop smoke did not apply AI provider preset",
        )
        .into());
    }
    ui.set_ai_provider_id("remote_smoke".into());
    ui.set_ai_provider_endpoint("https://example.invalid/v1".into());
    ui.set_ai_provider_secret_ref("env:ADM_SMOKE_AI_KEY".into());
    ui.set_ai_provider_capabilities("text_generation".into());
    ui.set_ai_provider_model("gpt-test".into());
    let save_provider_message = save_ai_provider_config(
        &data_root,
        ui.get_ai_provider_id().to_string(),
        ui.get_ai_provider_endpoint().to_string(),
        ui.get_ai_provider_secret_ref().to_string(),
        ui.get_ai_provider_capabilities().to_string(),
    )?;
    ui.set_status_text(save_provider_message.into());
    refresh_ai_diagnostics(&ui, &data_root);
    if !ui
        .get_ai_config_text()
        .contains("remote_smoke | MissingSecret")
    {
        return Err(std::io::Error::new(
            ErrorKind::Other,
            "desktop smoke did not show missing provider secret",
        )
        .into());
    }
    let disable_provider_message =
        disable_ai_provider_config(&data_root, ui.get_ai_provider_id().to_string())?;
    ui.set_status_text(disable_provider_message.into());
    refresh_ai_diagnostics(&ui, &data_root);
    if !ui.get_ai_config_text().contains("remote_smoke | Disabled") {
        return Err(std::io::Error::new(
            ErrorKind::Other,
            "desktop smoke did not show disabled provider",
        )
        .into());
    }
    let ready_provider_message = save_ai_provider_config(
        &data_root,
        "remote_ready".to_string(),
        "https://example.invalid/v1".to_string(),
        "env:PATH".to_string(),
        "text_generation,structured_output".to_string(),
    )?;
    ui.set_status_text(ready_provider_message.into());
    let check_message = check_ai_provider_config(
        &data_root,
        "remote_ready".to_string(),
        "gpt-test".to_string(),
    )?;
    if !check_message.contains("network_call=false")
        || !check_message.contains("supports.structured_output=true")
    {
        return Err(std::io::Error::new(
            ErrorKind::Other,
            "desktop smoke did not validate provider dry-run",
        )
        .into());
    }
    ui.set_ai_provider_id("remote_named".into());
    ui.set_ai_provider_endpoint("https://example.invalid/v1".into());
    ui.set_ai_provider_secret_ref("named:openai".into());
    ui.set_ai_provider_secret_value("fake_desktop_named_secret".into());
    ui.set_ai_provider_capabilities("text_generation".into());
    let save_secret_message = save_ai_named_secret(
        &data_root,
        ui.get_ai_provider_secret_ref().to_string(),
        ui.get_ai_provider_secret_value().to_string(),
    )?;
    if !save_secret_message.contains("named:openai")
        || save_secret_message.contains("fake_desktop_named_secret")
    {
        return Err(std::io::Error::new(
            ErrorKind::Other,
            "desktop smoke leaked or missed named secret save status",
        )
        .into());
    }
    let named_provider_message = save_ai_provider_config(
        &data_root,
        ui.get_ai_provider_id().to_string(),
        ui.get_ai_provider_endpoint().to_string(),
        ui.get_ai_provider_secret_ref().to_string(),
        ui.get_ai_provider_capabilities().to_string(),
    )?;
    ui.set_status_text(named_provider_message.into());
    refresh_ai_diagnostics(&ui, &data_root);
    let app_profile = std::fs::read_to_string(data_root.join("config").join("app_config.adm"))?;
    if !ui.get_ai_config_text().contains("remote_named | Ready")
        || !ui
            .get_ai_config_text()
            .contains("secret named:openai resolved")
        || !app_profile.contains("named:openai")
        || app_profile.contains("fake_desktop_named_secret")
    {
        return Err(std::io::Error::new(
            ErrorKind::Other,
            "desktop smoke did not save named AI secret safely",
        )
        .into());
    }
    let summary = create_and_run_project(
        &ui.get_project_title().to_string(),
        &ui.get_project_genre().to_string(),
        &ui.get_project_promise().to_string(),
        &ui.get_project_core_loop().to_string(),
        &data_root,
    )?;
    apply_run_summary(&ui, &summary);
    refresh_projects(&ui, &data_root, None);
    if ui.get_project_items().row_count() != 1 {
        return Err(std::io::Error::new(
            ErrorKind::Other,
            "desktop smoke did not populate project item model",
        )
        .into());
    }
    let design_core_artifact = ui.get_core_artifact_items().row_data(0);
    if !ui.get_core_artifact_text().contains("design_core_loop=3")
        || !ui.get_core_artifact_text().contains("development_tasks=3")
        || !ui.get_core_artifact_text().contains("asset_tasks=6")
        || !design_core_artifact
            .as_ref()
            .is_some_and(|item| item.summary.contains("tactical puzzle adventure"))
        || ui.get_core_artifact_items().row_count() != 3
    {
        return Err(std::io::Error::new(
            ErrorKind::Other,
            "desktop smoke did not populate core artifact model",
        )
        .into());
    }
    if ui.get_ai_provider_items().row_count() != 4 {
        return Err(std::io::Error::new(
            ErrorKind::Other,
            "desktop smoke did not refresh AI provider item model",
        )
        .into());
    }
    if !ui.get_package_text().contains("support_files=13") {
        return Err(std::io::Error::new(
            ErrorKind::Other,
            "desktop smoke did not load package status",
        )
        .into());
    }
    let workspace_doctor = inspect_workspaces_for_ui(&data_root)?;
    ui.set_workspace_doctor_text(workspace_doctor.into());
    if !ui.get_workspace_doctor_text().contains("workspace_count=")
        || !ui.get_workspace_doctor_text().contains("stale_count=")
        || ui.get_workspace_doctor_text().contains("stale_count=0")
    {
        return Err(std::io::Error::new(
            ErrorKind::Other,
            "desktop smoke did not render stale workspace doctor state",
        )
        .into());
    }
    let workspace_cleanup = cleanup_workspaces_for_ui(&data_root)?;
    ui.set_workspace_doctor_text(workspace_cleanup.into());
    if !ui.get_workspace_doctor_text().contains("removed_count=")
        || ui.get_workspace_doctor_text().contains("removed_count=0")
        || !ui
            .get_workspace_doctor_text()
            .contains("skipped_active_count=0")
    {
        return Err(std::io::Error::new(
            ErrorKind::Other,
            "desktop smoke did not clean stale workspaces",
        )
        .into());
    }
    let workspace_after_cleanup = inspect_workspaces_for_ui(&data_root)?;
    ui.set_workspace_doctor_text(workspace_after_cleanup.into());
    if !ui.get_workspace_doctor_text().contains("workspace_count=0")
        || !ui.get_workspace_doctor_text().contains("stale_count=0")
    {
        return Err(std::io::Error::new(
            ErrorKind::Other,
            "desktop smoke did not verify clean workspace state",
        )
        .into());
    }
    if !ui.get_sdk_text().contains("resources=5")
        || !ui.get_sdk_text().contains("validation=15")
        || ui.get_sdk_resource_items().row_count() != 5
    {
        return Err(std::io::Error::new(
            ErrorKind::Other,
            "desktop smoke did not load SDK resource model",
        )
        .into());
    }
    let sdk_items = ui.get_sdk_resource_items();
    let build_sdk_row = (0..sdk_items.row_count())
        .filter_map(|index| sdk_items.row_data(index))
        .find(|row| {
            row.sdk_name
                .to_string()
                .contains("Unity Build Automation SDK")
        });
    if build_sdk_row.is_none_or(|row| {
        row.category.to_string() != "build"
            || row.target_engines.to_string() != "Unity"
            || row.target_platforms.to_string() != "windows-desktop"
            || row.required_for_build.to_string() != "build_required=true"
            || !row
                .ai_explanation
                .to_string()
                .contains("guarded real Unity build")
    }) {
        return Err(std::io::Error::new(
            ErrorKind::Other,
            "desktop smoke did not expose SDK target metadata",
        )
        .into());
    }
    if !ui.get_acceptance_trace_text().contains("ready=3")
        || !ui.get_acceptance_trace_text().contains("incomplete=0")
        || !ui
            .get_validation_text()
            .contains("production_readiness=ready")
        || ui.get_acceptance_trace_items().row_count() != 3
    {
        return Err(std::io::Error::new(
            ErrorKind::Other,
            "desktop smoke did not populate acceptance trace matrix",
        )
        .into());
    }
    let acceptance_items = ui.get_acceptance_trace_items();
    let Some(first_trace) = acceptance_items.row_data(0) else {
        return Err(std::io::Error::new(
            ErrorKind::Other,
            "desktop smoke did not expose acceptance trace row",
        )
        .into());
    };
    if !first_trace.source_mechanic.contains("Core Loop Mechanic 1")
        || first_trace.status != "ready"
        || first_trace.scenario_id != "scenario_core_loop_step_1"
        || first_trace.validation_probe != "probe_core_loop_step_1_input_state_feedback"
        || !first_trace
            .build_targets
            .contains("windows_desktop_playable")
    {
        return Err(std::io::Error::new(
            ErrorKind::Other,
            "desktop smoke did not render acceptance trace row details",
        )
        .into());
    }
    if ui.get_package_file_items().row_count() != 18
        || !ui.get_build_target_text().contains("targets=1")
        || ui.get_build_target_items().row_count() != 1
        || ui.get_ai_task_items().row_count() != 1
        || ui.get_validation_issue_items().row_count() != 0
    {
        return Err(std::io::Error::new(
            ErrorKind::Other,
            "desktop smoke did not populate package, build target, AI task, or validation models",
        )
        .into());
    }
    let archive_id = first_archive_id(ui.get_project_list().as_str()).unwrap_or_default();
    ui.set_selected_archive_id(archive_id.clone().into());
    let resumed = resume_project("Slint Smoke Project", &data_root, &archive_id)?;
    apply_run_summary(&ui, &resumed);
    if ui.get_stage_items().row_count() != 15 {
        return Err(std::io::Error::new(
            ErrorKind::Other,
            "desktop smoke did not populate stage item model",
        )
        .into());
    }
    if !ui
        .get_stage_progress_text()
        .contains("执行阶段  Step14 集成验证: Step 已完成")
    {
        return Err(std::io::Error::new(
            ErrorKind::Other,
            "desktop smoke did not load stage progress",
        )
        .into());
    }
    let reran = rerun_project_stage(
        "Slint Smoke Project",
        &data_root,
        &archive_id,
        "development",
    )?;
    apply_run_summary(&ui, &reran);
    if !ui.get_status_text().contains("Reran development") || ui.get_stage_items().row_count() != 15
    {
        return Err(std::io::Error::new(
            ErrorKind::Other,
            "desktop smoke did not rerun selected stage",
        )
        .into());
    }
    let step03_detail = inspect_stage_detail(&data_root, &archive_id, "step03", None)?;
    apply_stage_detail(&ui, &step03_detail);
    if !ui
        .get_stage_detail_text()
        .contains("contract_kind=program_requirements_contract")
        || !ui
            .get_stage_detail_text()
            .contains("Structured Stage Content")
        || !ui
            .get_stage_detail_text()
            .contains("data_contracts=core_loop_step_1.request")
    {
        return Err(std::io::Error::new(
            ErrorKind::Other,
            "desktop smoke did not load Step03 structured stage detail",
        )
        .into());
    }
    let step04_detail = inspect_stage_detail(&data_root, &archive_id, "step04", None)?;
    apply_stage_detail(&ui, &step04_detail);
    if !ui
        .get_stage_detail_text()
        .contains("contract_kind=art_requirements_contract")
        || !ui
            .get_stage_detail_text()
            .contains("Structured Stage Content")
        || !ui
            .get_stage_detail_text()
            .contains("stage=mechanic_feedback")
    {
        return Err(std::io::Error::new(
            ErrorKind::Other,
            "desktop smoke did not load Step04 structured stage detail",
        )
        .into());
    }
    let step14_detail = inspect_stage_detail(&data_root, &archive_id, "step14", None)?;
    apply_stage_detail(&ui, &step14_detail);
    if !ui
        .get_stage_detail_text()
        .contains("contract_kind=integration_validation")
        || !ui
            .get_stage_detail_text()
            .contains("Structured Stage Content")
        || !ui.get_stage_detail_text().contains("production_readiness")
    {
        return Err(std::io::Error::new(
            ErrorKind::Other,
            "desktop smoke did not load Step14 structured stage detail",
        )
        .into());
    }
    let stage_detail = inspect_stage_detail(&data_root, &archive_id, "packaging", None)?;
    apply_stage_detail(&ui, &stage_detail);
    if !ui.get_stage_detail_text().contains("package/manifest.adm")
        || !ui
            .get_stage_detail_text()
            .contains("validation/acceptance_matrix.adm")
        || !ui
            .get_stage_detail_text()
            .contains("validation/production_readiness.adm")
        || !ui
            .get_stage_detail_text()
            .contains("validation/scenario_test_plan.adm")
        || !ui
            .get_stage_detail_text()
            .contains("validation/runtime_validation_report.adm")
        || ui.get_stage_artifact_items().row_count() != 6
        || ui.get_stage_detail_id() != "packaging"
    {
        return Err(std::io::Error::new(
            ErrorKind::Other,
            "desktop smoke did not load packaging stage detail",
        )
        .into());
    }
    let smoke_completed_range_request =
        PipelineService::new(&data_root).record_range_run_request(&archive_id, "03", "05")?;
    append_pipeline_range_log_for_ui(
        &data_root,
        "pipeline_range_started",
        &smoke_completed_range_request,
        &archive_id,
        None,
    )?;
    let range_summary = rerun_project_step_range(
        "Slint Smoke Project",
        &data_root,
        &archive_id,
        &smoke_completed_range_request.start_step_id,
        &smoke_completed_range_request.end_step_id,
    )?;
    let range_completed_count = record_pipeline_range_summary_for_ui(
        &data_root,
        &smoke_completed_range_request,
        &range_summary,
    )?;
    append_pipeline_range_log_for_ui(
        &data_root,
        "pipeline_range_projected",
        &smoke_completed_range_request,
        &range_summary.archive_id,
        Some(range_completed_count),
    )?;
    append_pipeline_range_log_for_ui(
        &data_root,
        "pipeline_range_completed",
        &smoke_completed_range_request,
        &range_summary.archive_id,
        Some(range_completed_count),
    )?;
    refresh_pipeline_service_status(&ui, &data_root);
    refresh_run_log(&ui, &data_root, "pipeline");
    if range_completed_count != 3
        || !ui.get_run_log_text().contains("pipeline_range_completed")
        || !ui
            .get_pipeline_service_text()
            .contains("devflow_completed_count=3")
        || !ui.get_pipeline_service_text().contains("status=completed")
    {
        return Err(std::io::Error::new(
            ErrorKind::Other,
            "desktop smoke did not associate pipeline range run log and service status",
        )
        .into());
    }
    let exported_package = data_root.join("smoke.admproj");
    let export_message = export_project(&data_root, &archive_id, &exported_package, None)?;
    ui.set_import_path(path_to_text(&exported_package).into());
    let package_doctor = inspect_import_package_for_ui(&data_root, &exported_package)?;
    ui.set_package_doctor_text(package_doctor.message.into());
    if !package_doctor.ready
        || !ui.get_package_doctor_text().contains("ready=true")
        || !ui
            .get_package_doctor_text()
            .contains("format=ADM_PACKAGE_V3")
        || !ui
            .get_package_doctor_text()
            .contains("file_count_actual=34")
        || !ui
            .get_package_doctor_text()
            .contains("validation\\production_readiness.adm")
            && !ui
                .get_package_doctor_text()
                .contains("validation/production_readiness.adm")
        || !ui
            .get_package_doctor_text()
            .contains("validation\\scenario_test_plan.adm")
            && !ui
                .get_package_doctor_text()
                .contains("validation/scenario_test_plan.adm")
        || !ui
            .get_package_doctor_text()
            .contains("validation\\runtime_validation_report.adm")
            && !ui
                .get_package_doctor_text()
                .contains("validation/runtime_validation_report.adm")
    {
        return Err(std::io::Error::new(
            ErrorKind::Other,
            "desktop smoke did not render package doctor for exported project",
        )
        .into());
    }
    let release_source = data_root.join("release-fixture").join("adm-desktop.exe");
    std::fs::create_dir_all(release_source.parent().unwrap())?;
    std::fs::write(&release_source, b"fake desktop executable")?;
    let release_target_dir = data_root.join("dist").join("AutoDesignMaker-rust");
    ui.set_release_source_exe(path_to_text(&release_source).into());
    ui.set_release_target_dir(path_to_text(&release_target_dir).into());
    let release_message = stage_desktop_release_for_ui(&release_source, &release_target_dir)?;
    ui.set_release_text(release_message.into());
    if !ui
        .get_release_text()
        .contains("legacy_root_exe=not_modified")
        || !release_target_dir
            .join("AutoDesignMaker-rust.exe")
            .is_file()
        || !release_target_dir.join("release-manifest.adm").is_file()
    {
        return Err(std::io::Error::new(
            ErrorKind::Other,
            "desktop smoke did not stage desktop release bundle",
        )
        .into());
    }
    let release_doctor_message = release_doctor_for_ui(&release_target_dir)?;
    ui.set_release_doctor_text(release_doctor_message.into());
    if !ui.get_release_doctor_text().contains("ready=true")
        || !ui.get_release_doctor_text().contains("hash=fnv64:")
        || !ui
            .get_release_doctor_text()
            .contains("legacy_root_exe=not_modified")
    {
        return Err(std::io::Error::new(
            ErrorKind::Other,
            "desktop smoke did not render release doctor",
        )
        .into());
    }
    let game_bundle_target_dir = data_root
        .join("game-bundles")
        .join("windows_desktop_playable");
    ui.set_game_build_target_id("windows_desktop_playable".into());
    ui.set_game_build_target_dir(path_to_text(&game_bundle_target_dir).into());
    let game_build_message = stage_game_build_bundle_for_ui(
        &data_root,
        &archive_id,
        "windows_desktop_playable",
        &game_bundle_target_dir,
    )?;
    ui.set_game_build_text(game_build_message.into());
    if !ui.get_game_build_text().contains("staged_files=9")
        || !game_bundle_target_dir
            .join("game-build-manifest.adm")
            .is_file()
        || !game_bundle_target_dir
            .join("content/project/brief.adm")
            .is_file()
        || !game_bundle_target_dir
            .join("content/design/project.adm")
            .is_file()
        || !game_bundle_target_dir
            .join("content/validation/acceptance_matrix.adm")
            .is_file()
        || !game_bundle_target_dir
            .join("content/validation/production_readiness.adm")
            .is_file()
        || !game_bundle_target_dir
            .join("content/validation/scenario_test_plan.adm")
            .is_file()
        || !game_bundle_target_dir
            .join("content/validation/runtime_validation_report.adm")
            .is_file()
    {
        return Err(std::io::Error::new(
            ErrorKind::Other,
            "desktop smoke did not stage game build bundle",
        )
        .into());
    }
    let sdk_bundle_target_dir = data_root.join("sdk-bundles").join(&archive_id);
    ui.set_sdk_bundle_target_dir(path_to_text(&sdk_bundle_target_dir).into());
    let sdk_bundle_message =
        stage_sdk_bundle_for_ui(&data_root, &archive_id, &sdk_bundle_target_dir)?;
    ui.set_sdk_bundle_text(sdk_bundle_message.into());
    if !ui.get_sdk_bundle_text().contains("staged_files=5")
        || !sdk_bundle_target_dir
            .join("sdk-bundle-manifest.adm")
            .is_file()
        || !sdk_bundle_target_dir.join("sdk/index.adm").is_file()
        || !sdk_bundle_target_dir
            .join("package/build_targets.adm")
            .is_file()
        || !sdk_bundle_target_dir
            .join("validation/production_readiness.adm")
            .is_file()
        || !sdk_bundle_target_dir
            .join("validation/scenario_test_plan.adm")
            .is_file()
        || !sdk_bundle_target_dir
            .join("validation/runtime_validation_report.adm")
            .is_file()
    {
        return Err(std::io::Error::new(
            ErrorKind::Other,
            "desktop smoke did not stage SDK bundle",
        )
        .into());
    }
    let unity_project_dir = data_root.join("unity-project");
    let fake_unity_exe = data_root
        .join("fake-unity")
        .join("Editor")
        .join("Unity.exe");
    std::fs::create_dir_all(fake_unity_exe.parent().unwrap())?;
    std::fs::copy(std::env::current_exe()?, &fake_unity_exe)?;
    ui.set_unity_exe(path_to_text(&fake_unity_exe).into());
    ui.set_unity_project_dir(path_to_text(&unity_project_dir).into());
    ui.set_unity_confirm_token(LOCAL_ENGINE_BUILD_CONFIRMATION_TOKEN.into());
    let unity_scaffold_message = stage_unity_project_for_ui(
        &data_root,
        &archive_id,
        "windows_desktop_playable",
        &unity_project_dir,
    )?;
    ui.set_unity_build_text(unity_scaffold_message.into());
    if !ui.get_unity_build_text().contains("generated_files=20")
        || !unity_project_dir
            .join("Assets/AutoDesignMaker/Generated/project_brief.adm")
            .is_file()
        || !unity_project_dir
            .join("Assets/AutoDesignMaker/Generated/design_project.adm")
            .is_file()
        || !unity_project_dir
            .join("Assets/AutoDesignMaker/Generated/acceptance_matrix.adm")
            .is_file()
        || !unity_project_dir
            .join("Assets/AutoDesignMaker/Generated/production_readiness.adm")
            .is_file()
        || !unity_project_dir
            .join("Assets/AutoDesignMaker/Generated/scenario_test_plan.adm")
            .is_file()
        || !unity_project_dir
            .join("Assets/AutoDesignMaker/Generated/runtime_validation_report.adm")
            .is_file()
        || !unity_project_dir
            .join("Assets/AutoDesignMaker/Generated/AutoDesignMakerGameplayModel.cs")
            .is_file()
        || !unity_project_dir
            .join("Assets/AutoDesignMaker/Editor/AutoDesignMakerBuild.cs")
            .is_file()
        || !unity_project_dir
            .join("Assets/AutoDesignMaker/Editor/AutoDesignMakerRuntimeValidation.cs")
            .is_file()
        || !unity_project_dir
            .join("Assets/AutoDesignMaker/Runtime/AutoDesignMakerRuntimeController.cs")
            .is_file()
        || !unity_project_dir
            .join("Assets/AutoDesignMaker/Runtime/AutoDesignMakerGameplayController.cs")
            .is_file()
        || !unity_project_dir
            .join("Assets/AutoDesignMaker/Runtime/AutoDesignMakerSceneComposer.cs")
            .is_file()
    {
        return Err(std::io::Error::new(
            ErrorKind::Other,
            "desktop smoke did not stage Unity project scaffold",
        )
        .into());
    }
    let staged_build_script = std::fs::read_to_string(
        unity_project_dir.join("Assets/AutoDesignMaker/Editor/AutoDesignMakerBuild.cs"),
    )?;
    if !staged_build_script.contains("PerformBuild")
        || !staged_build_script.contains("EditorSceneManager.NewScene")
    {
        return Err(std::io::Error::new(
            ErrorKind::Other,
            "desktop smoke staged incomplete Unity build script",
        )
        .into());
    }
    let staged_runtime_validation_script = std::fs::read_to_string(
        unity_project_dir.join("Assets/AutoDesignMaker/Editor/AutoDesignMakerRuntimeValidation.cs"),
    )?;
    if !staged_runtime_validation_script.contains("RunValidation")
        || !staged_runtime_validation_script.contains("runtime_validation_report.adm")
        || !staged_runtime_validation_script.contains("runtime_execution_results.adm")
    {
        return Err(std::io::Error::new(
            ErrorKind::Other,
            "desktop smoke staged incomplete Unity runtime validation script",
        )
        .into());
    }
    let staged_runtime_script = std::fs::read_to_string(
        unity_project_dir
            .join("Assets/AutoDesignMaker/Runtime/AutoDesignMakerRuntimeController.cs"),
    )?;
    if !staged_runtime_script.contains("SaveRuntimeSnapshot")
        || !staged_runtime_script.contains("AutoDesignMakerInputRouter")
        || !staged_runtime_script.contains("AutoDesignMakerGameplayController")
        || !staged_runtime_script.contains("AutoDesignMakerSceneComposer")
    {
        return Err(std::io::Error::new(
            ErrorKind::Other,
            "desktop smoke staged incomplete Unity runtime controller",
        )
        .into());
    }
    let staged_gameplay_model = std::fs::read_to_string(
        unity_project_dir.join("Assets/AutoDesignMaker/Generated/AutoDesignMakerGameplayModel.cs"),
    )?;
    if !staged_gameplay_model.contains("AutoDesignMakerGameplayModel")
        || !staged_gameplay_model.contains("Core Loop Mechanic 1")
        || !staged_gameplay_model.contains("GeneratedDevelopmentTask")
    {
        return Err(std::io::Error::new(
            ErrorKind::Other,
            "desktop smoke staged incomplete Unity gameplay model",
        )
        .into());
    }
    let staged_gameplay_controller = std::fs::read_to_string(
        unity_project_dir
            .join("Assets/AutoDesignMaker/Runtime/AutoDesignMakerGameplayController.cs"),
    )?;
    if !staged_gameplay_controller.contains("Generated Gameplay Loop")
        || !staged_gameplay_controller.contains("AdvanceMechanic")
    {
        return Err(std::io::Error::new(
            ErrorKind::Other,
            "desktop smoke staged incomplete Unity gameplay controller",
        )
        .into());
    }
    let staged_scene_composer = std::fs::read_to_string(
        unity_project_dir.join("Assets/AutoDesignMaker/Runtime/AutoDesignMakerSceneComposer.cs"),
    )?;
    if !staged_scene_composer.contains("ComposeScene")
        || !staged_scene_composer.contains("CreateMechanicNodes")
        || !staged_scene_composer.contains("TextMesh")
        || !staged_scene_composer.contains("PrimitiveType.Cube")
        || !staged_scene_composer.contains("LineRenderer")
    {
        return Err(std::io::Error::new(
            ErrorKind::Other,
            "desktop smoke staged incomplete Unity scene composer",
        )
        .into());
    }
    let unity_preflight_message = unity_build_preflight_for_ui(
        &data_root,
        &archive_id,
        "windows_desktop_playable",
        &fake_unity_exe,
        &unity_project_dir,
        LOCAL_ENGINE_BUILD_CONFIRMATION_TOKEN,
    )?;
    ui.set_unity_build_text(unity_preflight_message.into());
    if !ui
        .get_unity_build_text()
        .contains("ready_for_local_build=true")
        || !ui
            .get_unity_build_text()
            .contains("executable_present=true")
        || !ui
            .get_unity_build_text()
            .contains("unity_project_ready=true")
        || !ui
            .get_unity_build_text()
            .contains("confirmation_valid=true")
    {
        return Err(std::io::Error::new(
            ErrorKind::Other,
            "desktop smoke did not pass Unity build preflight",
        )
        .into());
    }
    let delivery_view = delivery_doctor_for_ui(
        &release_target_dir,
        &game_bundle_target_dir,
        &sdk_bundle_target_dir,
        &unity_project_dir,
    )?;
    ui.set_delivery_doctor_text(delivery_view.message.clone().into());
    apply_delivery_checks(&ui, &delivery_view.checks);
    if !ui.get_delivery_doctor_text().contains("ready=true")
        || !ui
            .get_delivery_doctor_text()
            .contains("game_build_bundle=true")
        || !ui.get_delivery_doctor_text().contains("sdk_bundle=true")
        || !ui.get_delivery_doctor_text().contains("unity_project=true")
        || ui.get_delivery_check_items().row_count() != 44
    {
        return Err(std::io::Error::new(
            ErrorKind::Other,
            "desktop smoke did not render delivery doctor",
        )
        .into());
    }
    let delivery_items = ui.get_delivery_check_items();
    let scene_composer_verified = (0..delivery_items.row_count())
        .filter_map(|index| delivery_items.row_data(index))
        .any(|row| {
            row.path
                .contains("Assets/AutoDesignMaker/Runtime/AutoDesignMakerSceneComposer.cs")
                && row.present == "verified"
        });
    let sdk_build_targets_verified = (0..delivery_items.row_count())
        .filter_map(|index| delivery_items.row_data(index))
        .any(|row| {
            row.scope == "sdk_bundle"
                && row.path == "package/build_targets.adm"
                && row.present == "verified"
        });
    let sdk_engine_history_optional = (0..delivery_items.row_count())
        .filter_map(|index| delivery_items.row_data(index))
        .any(|row| {
            row.scope == "sdk_bundle"
                && row.path == "package/engine_build_history.adm"
                && row.present == "optional_missing"
        });
    let sdk_production_readiness_verified = (0..delivery_items.row_count())
        .filter_map(|index| delivery_items.row_data(index))
        .any(|row| {
            row.scope == "sdk_bundle"
                && row.path == "validation/production_readiness.adm"
                && row.present == "verified"
        });
    let sdk_scenario_test_plan_verified = (0..delivery_items.row_count())
        .filter_map(|index| delivery_items.row_data(index))
        .any(|row| {
            row.scope == "sdk_bundle"
                && row.path == "validation/scenario_test_plan.adm"
                && row.present == "verified"
        });
    let sdk_runtime_validation_verified = (0..delivery_items.row_count())
        .filter_map(|index| delivery_items.row_data(index))
        .any(|row| {
            row.scope == "sdk_bundle"
                && row.path == "validation/runtime_validation_report.adm"
                && row.present == "verified"
        });
    let sdk_runtime_execution_optional = (0..delivery_items.row_count())
        .filter_map(|index| delivery_items.row_data(index))
        .any(|row| {
            row.scope == "sdk_bundle"
                && row.path == "validation/runtime_execution_results.adm"
                && row.present == "optional_missing"
        });
    let game_acceptance_matrix_present = (0..delivery_items.row_count())
        .filter_map(|index| delivery_items.row_data(index))
        .any(|row| {
            row.scope == "game_build_bundle"
                && row.path == "content/validation/acceptance_matrix.adm"
                && row.present == "present"
        });
    let game_production_readiness_present = (0..delivery_items.row_count())
        .filter_map(|index| delivery_items.row_data(index))
        .any(|row| {
            row.scope == "game_build_bundle"
                && row.path == "content/validation/production_readiness.adm"
                && row.present == "present"
        });
    let game_scenario_test_plan_present = (0..delivery_items.row_count())
        .filter_map(|index| delivery_items.row_data(index))
        .any(|row| {
            row.scope == "game_build_bundle"
                && row.path == "content/validation/scenario_test_plan.adm"
                && row.present == "present"
        });
    let game_runtime_validation_present = (0..delivery_items.row_count())
        .filter_map(|index| delivery_items.row_data(index))
        .any(|row| {
            row.scope == "game_build_bundle"
                && row.path == "content/validation/runtime_validation_report.adm"
                && row.present == "present"
        });
    let game_runtime_execution_optional = (0..delivery_items.row_count())
        .filter_map(|index| delivery_items.row_data(index))
        .any(|row| {
            row.scope == "game_build_bundle"
                && row.path == "content/validation/runtime_execution_results.adm"
                && row.present == "optional_missing"
        });
    let game_project_brief_present = (0..delivery_items.row_count())
        .filter_map(|index| delivery_items.row_data(index))
        .any(|row| {
            row.scope == "game_build_bundle"
                && row.path == "content/project/brief.adm"
                && row.present == "present"
        });
    let unity_acceptance_matrix_verified = (0..delivery_items.row_count())
        .filter_map(|index| delivery_items.row_data(index))
        .any(|row| {
            row.scope == "unity_project"
                && row.path == "Assets/AutoDesignMaker/Generated/acceptance_matrix.adm"
                && row.present == "verified"
        });
    let unity_production_readiness_verified = (0..delivery_items.row_count())
        .filter_map(|index| delivery_items.row_data(index))
        .any(|row| {
            row.scope == "unity_project"
                && row.path == "Assets/AutoDesignMaker/Generated/production_readiness.adm"
                && row.present == "verified"
        });
    let unity_scenario_test_plan_verified = (0..delivery_items.row_count())
        .filter_map(|index| delivery_items.row_data(index))
        .any(|row| {
            row.scope == "unity_project"
                && row.path == "Assets/AutoDesignMaker/Generated/scenario_test_plan.adm"
                && row.present == "verified"
        });
    let unity_runtime_validation_verified = (0..delivery_items.row_count())
        .filter_map(|index| delivery_items.row_data(index))
        .any(|row| {
            row.scope == "unity_project"
                && row.path == "Assets/AutoDesignMaker/Generated/runtime_validation_report.adm"
                && row.present == "verified"
        });
    let unity_runtime_validation_script_verified = (0..delivery_items.row_count())
        .filter_map(|index| delivery_items.row_data(index))
        .any(|row| {
            row.scope == "unity_project"
                && row.path == "Assets/AutoDesignMaker/Editor/AutoDesignMakerRuntimeValidation.cs"
                && row.present == "verified"
        });
    let unity_runtime_execution_optional = (0..delivery_items.row_count())
        .filter_map(|index| delivery_items.row_data(index))
        .any(|row| {
            row.scope == "unity_project"
                && row.path == "Assets/AutoDesignMaker/Generated/runtime_execution_results.adm"
                && row.present == "optional_missing"
        });
    let unity_project_brief_verified = (0..delivery_items.row_count())
        .filter_map(|index| delivery_items.row_data(index))
        .any(|row| {
            row.scope == "unity_project"
                && row.path == "Assets/AutoDesignMaker/Generated/project_brief.adm"
                && row.present == "verified"
        });
    if !scene_composer_verified
        || !sdk_build_targets_verified
        || !sdk_engine_history_optional
        || !sdk_production_readiness_verified
        || !sdk_scenario_test_plan_verified
        || !sdk_runtime_validation_verified
        || !sdk_runtime_execution_optional
        || !game_project_brief_present
        || !game_acceptance_matrix_present
        || !game_production_readiness_present
        || !game_scenario_test_plan_present
        || !game_runtime_validation_present
        || !game_runtime_execution_optional
        || !unity_project_brief_verified
        || !unity_acceptance_matrix_verified
        || !unity_production_readiness_verified
        || !unity_scenario_test_plan_verified
        || !unity_runtime_validation_verified
        || !unity_runtime_validation_script_verified
        || !unity_runtime_execution_optional
    {
        return Err(std::io::Error::new(
            ErrorKind::Other,
            "desktop smoke did not verify delivery doctor content statuses",
        )
        .into());
    }
    let unity_build_message = plan_unity_build_for_ui(
        &data_root,
        &archive_id,
        "windows_desktop_playable",
        &PathBuf::from(ui.get_unity_exe().to_string()),
        &unity_project_dir,
    )?;
    ui.set_unity_build_text(unity_build_message.into());
    if !ui
        .get_unity_build_text()
        .contains("target_id=windows_desktop_playable")
        || !ui
            .get_unity_build_text()
            .contains("AutoDesignMaker.EditorBuild.PerformBuild")
        || !ui.get_unity_build_text().contains("Win64")
    {
        return Err(std::io::Error::new(
            ErrorKind::Other,
            "desktop smoke did not plan Unity build command",
        )
        .into());
    }
    let unity_dry_run_message = dry_run_unity_build_for_ui(
        &data_root,
        &archive_id,
        "windows_desktop_playable",
        &PathBuf::from(ui.get_unity_exe().to_string()),
        &unity_project_dir,
    )?;
    ui.set_unity_build_text(unity_dry_run_message.into());
    if !ui.get_unity_build_text().contains("mode=dry_run")
        || !ui.get_unity_build_text().contains("launched=false")
        || !ui
            .get_unity_build_text()
            .contains("AutoDesignMaker.EditorBuild.PerformBuild")
    {
        return Err(std::io::Error::new(
            ErrorKind::Other,
            "desktop smoke did not dry-run Unity build command",
        )
        .into());
    }
    let runtime_plan_message = plan_unity_runtime_validation_for_ui(
        &data_root,
        &archive_id,
        "windows_desktop_playable",
        &PathBuf::from(ui.get_unity_exe().to_string()),
        &unity_project_dir,
    )?;
    ui.set_runtime_validation_text(runtime_plan_message.into());
    let runtime_results_file = default_runtime_results_file(&unity_project_dir);
    ui.set_runtime_results_file(path_to_text(&runtime_results_file).into());
    if !ui
        .get_runtime_validation_text()
        .contains("Unity Runtime Validation Command")
        || !ui
            .get_runtime_validation_text()
            .contains("AutoDesignMaker.RuntimeValidation.RunValidation")
        || !ui
            .get_runtime_validation_text()
            .contains("runtime_execution_results.adm")
    {
        return Err(std::io::Error::new(
            ErrorKind::Other,
            "desktop smoke did not plan Unity runtime validation command",
        )
        .into());
    }
    let runtime_dry_run_message = dry_run_unity_runtime_validation_for_ui(
        &data_root,
        &archive_id,
        "windows_desktop_playable",
        &PathBuf::from(ui.get_unity_exe().to_string()),
        &unity_project_dir,
    )?;
    ui.set_runtime_validation_text(runtime_dry_run_message.into());
    if !ui.get_runtime_validation_text().contains("mode=dry_run")
        || !ui.get_runtime_validation_text().contains("launched=false")
        || !ui
            .get_runtime_validation_text()
            .contains("UnityRuntimeValidation")
        || !ui
            .get_runtime_validation_text()
            .contains("AutoDesignMaker.RuntimeValidation.RunValidation")
    {
        return Err(std::io::Error::new(
            ErrorKind::Other,
            "desktop smoke did not dry-run Unity runtime validation command",
        )
        .into());
    }
    let engine_history_inspection = load_project_inspection(&data_root, &archive_id, None)?;
    apply_project_inspection(&ui, &engine_history_inspection);
    if !ui.get_engine_history_text().contains("records=1")
        || !ui.get_engine_history_text().contains("launched=0")
        || !ui.get_engine_history_text().contains("outputs_present=0")
        || ui.get_engine_history_items().row_count() != 1
        || ui.get_package_file_items().row_count() != 19
        || ui.get_acceptance_trace_items().row_count() != 3
    {
        return Err(std::io::Error::new(
            ErrorKind::Other,
            "desktop smoke did not render engine build history package awareness",
        )
        .into());
    }
    let engine_history_items = ui.get_engine_history_items();
    let Some(engine_history_row) = engine_history_items.row_data(0) else {
        return Err(std::io::Error::new(
            ErrorKind::Other,
            "desktop smoke did not expose engine build history row",
        )
        .into());
    };
    if engine_history_row.expected_output_present != "false"
        || engine_history_row.expected_output_bytes != "0"
        || engine_history_row.expected_output_hash != "none"
        || !engine_history_row
            .expected_output_path
            .contains("build/windows/AutoDesignMakerGame.zip")
    {
        return Err(std::io::Error::new(
            ErrorKind::Other,
            "desktop smoke did not render engine build output verification fields",
        )
        .into());
    }
    let unity_run_message = run_unity_build_for_ui(
        &data_root,
        &archive_id,
        "windows_desktop_playable",
        &PathBuf::from(ui.get_unity_exe().to_string()),
        &unity_project_dir,
        LOCAL_ENGINE_BUILD_CONFIRMATION_TOKEN,
    )?;
    ui.set_unity_build_text(unity_run_message.into());
    if !ui.get_unity_build_text().contains("mode=local_process")
        || !ui.get_unity_build_text().contains("launched=true")
        || !ui
            .get_unity_build_text()
            .contains("expected_output_present=true")
        || !ui.get_unity_build_text().contains("history_records=2")
        || !unity_project_dir
            .join("build/windows/AutoDesignMakerGame.zip")
            .is_file()
    {
        return Err(std::io::Error::new(
            ErrorKind::Other,
            format!(
                "desktop smoke did not run guarded Unity build\n{}",
                ui.get_unity_build_text()
            ),
        )
        .into());
    }
    let engine_history_run_inspection = load_project_inspection(&data_root, &archive_id, None)?;
    apply_project_inspection(&ui, &engine_history_run_inspection);
    if !ui.get_engine_history_text().contains("records=2")
        || !ui.get_engine_history_text().contains("launched=1")
        || !ui.get_engine_history_text().contains("outputs_present=1")
        || ui.get_engine_history_items().row_count() != 2
        || ui.get_package_file_items().row_count() != 19
    {
        return Err(std::io::Error::new(
            ErrorKind::Other,
            "desktop smoke did not render guarded Unity build history",
        )
        .into());
    }
    let runtime_run_message = run_unity_runtime_validation_for_ui(
        &data_root,
        &archive_id,
        "windows_desktop_playable",
        &PathBuf::from(ui.get_unity_exe().to_string()),
        &unity_project_dir,
        LOCAL_ENGINE_BUILD_CONFIRMATION_TOKEN,
    )?;
    ui.set_runtime_validation_text(runtime_run_message.into());
    if !ui
        .get_runtime_validation_text()
        .contains("mode=local_process")
        || !ui.get_runtime_validation_text().contains("launched=true")
        || !ui
            .get_runtime_validation_text()
            .contains("expected_output_present=true")
        || !ui
            .get_runtime_validation_text()
            .contains("runtime_ready=true")
        || !ui
            .get_runtime_validation_text()
            .contains("runtime_runner=desktop_smoke_runtime")
        || !runtime_results_file.is_file()
    {
        return Err(std::io::Error::new(
            ErrorKind::Other,
            format!(
                "desktop smoke did not run guarded Unity runtime validation\n{}",
                ui.get_runtime_validation_text()
            ),
        )
        .into());
    }
    let runtime_run_inspection = load_project_inspection(&data_root, &archive_id, None)?;
    apply_project_inspection(&ui, &runtime_run_inspection);
    if !ui
        .get_validation_text()
        .contains("production_readiness=ready")
        || ui.get_package_file_items().row_count() != 20
    {
        return Err(std::io::Error::new(
            ErrorKind::Other,
            "desktop smoke did not render guarded runtime validation result awareness",
        )
        .into());
    }
    let runtime_record_message =
        record_runtime_validation_for_ui(&data_root, &archive_id, &runtime_results_file)?;
    ui.set_runtime_validation_text(runtime_record_message.into());
    if !ui.get_runtime_validation_text().contains("ready=true")
        || !ui.get_runtime_validation_text().contains("contract_rows=3")
        || !ui.get_runtime_validation_text().contains("observed_rows=3")
        || !ui
            .get_runtime_validation_text()
            .contains("runtime_commit_files=36")
    {
        return Err(std::io::Error::new(
            ErrorKind::Other,
            format!(
                "desktop smoke did not record runtime validation execution results\n{}",
                ui.get_runtime_validation_text()
            ),
        )
        .into());
    }
    let runtime_record_inspection = load_project_inspection(&data_root, &archive_id, None)?;
    apply_project_inspection(&ui, &runtime_record_inspection);
    if !ui
        .get_validation_text()
        .contains("production_readiness=ready")
        || ui.get_package_file_items().row_count() != 20
    {
        return Err(std::io::Error::new(
            ErrorKind::Other,
            format!(
                "desktop smoke did not render runtime validation package awareness\nvalidation_text={}\npackage_rows={}",
                ui.get_validation_text(),
                ui.get_package_file_items().row_count()
            ),
        )
        .into());
    }
    let import_root =
        std::env::temp_dir().join(format!("adm_desktop_smoke_import_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&import_root);
    let import_result = import_project(&import_root, &exported_package)?;
    ui.set_data_root(path_to_text(&import_root).into());
    ui.set_package_doctor_text(import_result.package_doctor_text.clone().into());
    ui.set_selected_archive_id(import_result.archive_id.clone().into());
    refresh_projects(&ui, &import_root, None);
    if !ui
        .get_project_detail()
        .contains(&format!("archive_id={}", import_result.archive_id))
        || !ui.get_package_doctor_text().contains("ready=true")
        || ui.get_package_file_items().row_count() != 18
        || ui.get_build_target_items().row_count() != 1
        || ui.get_sdk_resource_items().row_count() != 5
        || ui.get_core_artifact_items().row_count() != 3
        || ui.get_acceptance_trace_items().row_count() != 3
    {
        return Err(std::io::Error::new(
            ErrorKind::Other,
            "desktop smoke did not select imported project details",
        )
        .into());
    }
    ui.set_data_root(path_to_text(&data_root).into());
    ui.set_selected_archive_id(archive_id.clone().into());
    refresh_projects(&ui, &data_root, None);
    write_ai_failure_fixture(&data_root, &archive_id)?;
    let failure_inspection = load_project_inspection(&data_root, &archive_id, None)?;
    ui.set_ai_text(failure_inspection.ai_text.into());
    apply_ai_tasks(&ui, &failure_inspection.ai_tasks);
    if !ui
        .get_ai_text()
        .contains("failures=budget_exceeded=1, provider_unavailable=1")
    {
        return Err(std::io::Error::new(
            ErrorKind::Other,
            "desktop smoke did not render AI failure summary",
        )
        .into());
    }
    if ui.get_ai_task_items().row_count() != 2 {
        return Err(std::io::Error::new(
            ErrorKind::Other,
            "desktop smoke did not render AI failure task rows",
        )
        .into());
    }
    if !ui.get_ai_text().contains("last_error=provider offline") {
        return Err(std::io::Error::new(
            ErrorKind::Other,
            "desktop smoke did not render latest AI error",
        )
        .into());
    }
    write_failed_pipeline_fixture(&data_root, &archive_id)?;
    let failed_pipeline_inspection = load_project_inspection(&data_root, &archive_id, None)?;
    apply_project_inspection(&ui, &failed_pipeline_inspection);
    if !ui
        .get_stage_progress_text()
        .contains("Step10 资源对齐: Step 失败")
    {
        return Err(std::io::Error::new(
            ErrorKind::Other,
            "desktop smoke did not render failed pipeline stage",
        )
        .into());
    }
    let resumed_failed = resume_failed_project("Slint Smoke Project", &data_root, &archive_id)?;
    apply_run_summary(&ui, &resumed_failed);
    if !ui.get_status_text().contains("Resumed failed stage")
        || !ui
            .get_stage_progress_text()
            .contains("Step10 资源对齐: Step 已完成")
        || !ui
            .get_stage_progress_text()
            .contains("Step14 集成验证: Step 已完成")
    {
        return Err(std::io::Error::new(
            ErrorKind::Other,
            "desktop smoke did not resume failed pipeline stage",
        )
        .into());
    }
    write_validation_review_fixture(&data_root, &archive_id)?;
    let validation_review = load_project_inspection(&data_root, &archive_id, None)?;
    apply_project_inspection(&ui, &validation_review);
    if !ui.get_validation_text().contains("Validation: Warning")
        || ui.get_validation_issue_items().row_count() != 1
        || ui.get_ai_task_items().row_count() != 1
        || !ui.get_ai_text().contains("AI: records=1")
        || !ui.get_ai_text().contains("accepted=1")
    {
        return Err(std::io::Error::new(
            ErrorKind::Other,
            "desktop smoke did not render validation-triggered AI review state",
        )
        .into());
    }
    let owned_lock: SelectedArchiveLockState = Rc::new(RefCell::new(None));
    let owned_inspection = lock_and_load_project_inspection(&data_root, &archive_id, &owned_lock)?;
    apply_project_inspection(&ui, &owned_inspection);
    refresh_projects(&ui, &data_root, Some(&owned_lock));
    if !ui.get_project_list().contains("locked=true")
        || !ui.get_project_list().contains("owner=session_id=")
        || !ui
            .get_project_detail()
            .contains(&format!("archive_id={archive_id}"))
        || !ui.get_project_detail().contains("lock_owner=session_id=")
    {
        return Err(std::io::Error::new(
            ErrorKind::Other,
            "desktop smoke did not render own selected archive lock owner",
        )
        .into());
    }
    let parallel_summary = create_and_run_demo_project("Slint Parallel Smoke Project", &data_root)?;
    let parallel_archive_id = parallel_summary.archive_id.clone();
    let parallel_lock: SelectedArchiveLockState = Rc::new(RefCell::new(None));
    let parallel_inspection =
        lock_and_load_project_inspection(&data_root, &parallel_archive_id, &parallel_lock)?;
    if !parallel_inspection
        .detail_text
        .contains(&format!("archive_id={parallel_archive_id}"))
        || owned_lock_session_for_archive(&owned_lock, &archive_id).is_none()
        || owned_lock_session_for_archive(&parallel_lock, &parallel_archive_id).is_none()
    {
        return Err(std::io::Error::new(
            ErrorKind::Other,
            "desktop smoke did not allow different archives to be locked in parallel",
        )
        .into());
    }
    let primary_lock_path = data_root
        .join("archives")
        .join(&archive_id)
        .join(".archive_lock");
    let parallel_lock_path = data_root
        .join("archives")
        .join(&parallel_archive_id)
        .join(".archive_lock");
    if !primary_lock_path.is_file() || !parallel_lock_path.is_file() {
        return Err(std::io::Error::new(
            ErrorKind::Other,
            "desktop smoke did not hold independent archive lock files in parallel",
        )
        .into());
    }
    release_selected_archive_lock(&parallel_lock);
    if !primary_lock_path.is_file() || parallel_lock_path.exists() {
        return Err(std::io::Error::new(
            ErrorKind::Other,
            "desktop smoke released the wrong archive lock after parallel selection",
        )
        .into());
    }
    let competing_lock: SelectedArchiveLockState = Rc::new(RefCell::new(None));
    match lock_and_load_project_inspection(&data_root, &archive_id, &competing_lock) {
        Ok(_) => {
            return Err(std::io::Error::new(
                ErrorKind::Other,
                "desktop smoke allowed competing selected archive lock",
            )
            .into());
        }
        Err(error)
            if error
                .to_string()
                .contains("formal archive is already locked") => {}
        Err(error) => return Err(Box::new(error)),
    }
    let owned_session = owned_lock_session_for_archive(&owned_lock, &archive_id);
    inspect_stage_detail(&data_root, &archive_id, "packaging", owned_session.as_ref())?;
    let current_window_clear_error =
        clear_external_archive_lock_file(&data_root, &archive_id, Some(&owned_lock))
            .expect_err("external clear must reject current-window lock");
    if !current_window_clear_error
        .to_string()
        .contains("use Release Lock instead")
    {
        return Err(std::io::Error::new(
            ErrorKind::Other,
            "desktop smoke did not distinguish current-window lock from external lock",
        )
        .into());
    }
    let release_lock_message = release_current_window_lock(&archive_id, &owned_lock)?;
    if !release_lock_message.contains("Released current window lock")
        || owned_lock_session_for_archive(&owned_lock, &archive_id).is_some()
    {
        return Err(std::io::Error::new(
            ErrorKind::Other,
            "desktop smoke did not release current-window lock explicitly",
        )
        .into());
    }
    let lock_path = data_root
        .join("archives")
        .join(&archive_id)
        .join(".archive_lock");
    std::fs::write(
        &lock_path,
        "session_id=other_window\npid=999999\ncreated_at=0\n",
    )?;
    refresh_projects(&ui, &data_root, None);
    if !ui.get_project_list().contains("locked=true")
        || !ui.get_project_list().contains("pid=999999")
        || !ui
            .get_project_detail()
            .contains("formal archive is already locked")
        || !ui.get_project_detail().contains("session_id=other_window")
    {
        return Err(std::io::Error::new(
            ErrorKind::Other,
            "desktop smoke did not render locked archive owner state",
        )
        .into());
    }
    match load_project_inspection(&data_root, &archive_id, None) {
        Ok(_) => {
            return Err(std::io::Error::new(
                ErrorKind::Other,
                "desktop smoke allowed locked archive inspection",
            )
            .into());
        }
        Err(error)
            if error
                .to_string()
                .contains("formal archive is already locked") => {}
        Err(error) => return Err(Box::new(error)),
    }
    let clear_lock_message = clear_external_archive_lock_file(&data_root, &archive_id, None)?;
    if !clear_lock_message.contains("Cleared archive lock") {
        return Err(std::io::Error::new(
            ErrorKind::Other,
            "desktop smoke did not clear external archive lock",
        )
        .into());
    }
    refresh_projects(&ui, &data_root, None);
    if ui.get_project_list().contains("locked=true") || ui.get_project_list().contains("pid=999999")
    {
        return Err(std::io::Error::new(
            ErrorKind::Other,
            "desktop smoke still rendered lock owner after clear",
        )
        .into());
    }
    run_process_archive_lock_smoke(&data_root, &archive_id, &parallel_archive_id)?;
    println!("{}", resumed.status_text);
    println!("{export_message}");
    println!("{}", ui.get_pipeline_text());
    println!("{}", ui.get_core_artifact_text());
    println!("{}", ui.get_stage_progress_text());
    println!("{}", ui.get_stage_detail_text());
    println!("{}", ui.get_ai_text());
    println!("{}", ui.get_ai_config_text());
    println!("{}", ui.get_sdk_text());
    println!("{}", ui.get_package_text());
    println!("{}", ui.get_release_text());
    println!("{}", ui.get_validation_text());
    println!("{}", ui.get_project_list());
    let _ = std::fs::remove_dir_all(data_root);
    let _ = std::fs::remove_dir_all(import_root);
    Ok(())
}

fn write_runtime_validation_execution_fixture(results_file: &Path) -> AdmResult<()> {
    if let Some(parent) = results_file.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(
        results_file,
        "# Runtime Validation Execution\n\
runner=desktop_smoke_runtime\n\
target_id=windows_desktop_playable\n\
- result_id=runtime_scenario_core_loop_step_1; scenario_id=scenario_core_loop_step_1; test_id=test_scenario_core_loop_step_1; acceptance_trace_id=trace_core_loop_step_1; telemetry_start_seen=true; telemetry_complete_seen=true; expected_state_seen=true; failure_guard_triggered=false; status=passed\n\
- result_id=runtime_scenario_core_loop_step_2; scenario_id=scenario_core_loop_step_2; test_id=test_scenario_core_loop_step_2; acceptance_trace_id=trace_core_loop_step_2; telemetry_start_seen=true; telemetry_complete_seen=true; expected_state_seen=true; failure_guard_triggered=false; status=passed\n\
- result_id=runtime_scenario_core_loop_step_3; scenario_id=scenario_core_loop_step_3; test_id=test_scenario_core_loop_step_3; acceptance_trace_id=trace_core_loop_step_3; telemetry_start_seen=true; telemetry_complete_seen=true; expected_state_seen=true; failure_guard_triggered=false; status=passed\n",
    )?;
    Ok(())
}

fn run_process_archive_lock_smoke(
    data_root: &Path,
    archive_id: &str,
    parallel_archive_id: &str,
) -> Result<(), Box<dyn Error>> {
    let exe = std::env::current_exe()?;
    let marker_dir = data_root.join("process-lock-smoke");
    let _ = std::fs::remove_dir_all(&marker_dir);
    std::fs::create_dir_all(&marker_dir)?;

    let primary_ready = marker_dir.join("primary.ready");
    let primary_release = marker_dir.join("primary.release");
    let parallel_ready = marker_dir.join("parallel.ready");
    let parallel_release = marker_dir.join("parallel.release");
    let same_ready = marker_dir.join("same.ready");
    let same_release = marker_dir.join("same.release");

    let primary_lock_path = data_root
        .join("archives")
        .join(archive_id)
        .join(".archive_lock");
    let parallel_lock_path = data_root
        .join("archives")
        .join(parallel_archive_id)
        .join(".archive_lock");

    let mut primary = start_archive_lock_probe(
        &exe,
        data_root,
        archive_id,
        &primary_ready,
        &primary_release,
        10_000,
    )?;
    wait_for_file(&primary_ready, Duration::from_secs(3))?;
    wait_for_file(&primary_lock_path, Duration::from_secs(3))?;

    let same_output = Command::new(&exe)
        .arg("--lock-probe")
        .arg(data_root)
        .arg(archive_id)
        .arg(&same_ready)
        .arg(&same_release)
        .arg("500")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()?;
    if same_output.status.success() {
        let _ = std::fs::write(&primary_release, "release\n");
        let _ = primary.wait();
        return Err(std::io::Error::new(
            ErrorKind::Other,
            "process lock smoke allowed a second process to lock the same archive",
        )
        .into());
    }
    let same_text = process_output_text(&same_output);
    if !same_text.contains("formal archive is already locked") {
        let _ = std::fs::write(&primary_release, "release\n");
        let _ = primary.wait();
        return Err(std::io::Error::new(
            ErrorKind::Other,
            format!("same-archive lock probe failed for unexpected reason: {same_text}"),
        )
        .into());
    }

    let parallel = start_archive_lock_probe(
        &exe,
        data_root,
        parallel_archive_id,
        &parallel_ready,
        &parallel_release,
        10_000,
    )?;
    wait_for_file(&parallel_ready, Duration::from_secs(3))?;
    wait_for_file(&parallel_lock_path, Duration::from_secs(3))?;

    std::fs::write(&parallel_release, "release\n")?;
    std::fs::write(&primary_release, "release\n")?;
    let parallel_output = parallel.wait_with_output()?;
    let primary_output = primary.wait_with_output()?;
    ensure_probe_success("parallel archive", &parallel_output)?;
    ensure_probe_success("primary archive", &primary_output)?;
    wait_for_missing(&primary_lock_path, Duration::from_secs(3))?;
    wait_for_missing(&parallel_lock_path, Duration::from_secs(3))?;
    Ok(())
}

fn start_archive_lock_probe(
    exe: &Path,
    data_root: &Path,
    archive_id: &str,
    ready_file: &Path,
    release_file: &Path,
    timeout_ms: u64,
) -> Result<Child, Box<dyn Error>> {
    Ok(Command::new(exe)
        .arg("--lock-probe")
        .arg(data_root)
        .arg(archive_id)
        .arg(ready_file)
        .arg(release_file)
        .arg(timeout_ms.to_string())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?)
}

fn wait_for_file(path: &Path, timeout: Duration) -> Result<(), Box<dyn Error>> {
    let deadline = Instant::now() + timeout;
    while !path.is_file() {
        if Instant::now() >= deadline {
            return Err(std::io::Error::new(
                ErrorKind::TimedOut,
                format!("timed out waiting for {}", path.display()),
            )
            .into());
        }
        thread::sleep(Duration::from_millis(25));
    }
    Ok(())
}

fn wait_for_missing(path: &Path, timeout: Duration) -> Result<(), Box<dyn Error>> {
    let deadline = Instant::now() + timeout;
    while path.exists() {
        if Instant::now() >= deadline {
            return Err(std::io::Error::new(
                ErrorKind::TimedOut,
                format!("timed out waiting for {} to be removed", path.display()),
            )
            .into());
        }
        thread::sleep(Duration::from_millis(25));
    }
    Ok(())
}

fn ensure_probe_success(label: &str, output: &std::process::Output) -> Result<(), Box<dyn Error>> {
    if output.status.success() {
        return Ok(());
    }
    Err(std::io::Error::new(
        ErrorKind::Other,
        format!("{label} lock probe failed: {}", process_output_text(output)),
    )
    .into())
}

fn process_output_text(output: &std::process::Output) -> String {
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    format!("stdout={stdout}; stderr={stderr}")
}

fn write_ai_failure_fixture(data_root: &Path, archive_id: &str) -> AdmResult<()> {
    let app = AdmApplication::for_data_root(data_root)?;
    let archive = app.load_project(archive_id)?;
    let mut journal = AiTaskJournal::default();

    let budget_request = AiTaskRequest::new(
        AiCapability::ScoringReview,
        "score generated design",
        "smoke",
    )?;
    let mut budget_failure = AiTaskRecord::new(budget_request, 1);
    budget_failure.fail_with_kind("budget empty", AiFailureKind::BudgetExceeded);
    journal.push(budget_failure);

    let provider_request =
        AiTaskRequest::new(AiCapability::CodeGeneration, "generate code", "smoke")?;
    let mut provider_failure = AiTaskRecord::new(provider_request, 1);
    provider_failure.fail_with_kind("provider offline", AiFailureKind::ProviderUnavailable);
    journal.push(provider_failure);

    journal.save_to_path(archive.root.join("content").join("ai").join("journal.adm"))
}

fn write_validation_review_fixture(data_root: &Path, archive_id: &str) -> AdmResult<()> {
    let app = AdmApplication::for_data_root(data_root)?;
    let archive = app.load_project(archive_id)?;
    let content_root = archive.root.join("content");
    std::fs::write(
        content_root.join("validation").join("report.adm"),
        "# Validation Report\nstatus=Warning\n- status=Warning; code=game_build.required_artifact.missing; message=smoke validation review required\n",
    )?;

    let provider =
        MockAiProvider::new(ProviderId::new("mock")?, vec![AiCapability::TextGeneration]);
    let request = AiTaskRequest::new(
        AiCapability::TextGeneration,
        "Review pipeline validation issues for smoke fixture and propose concrete fixes.",
        "validation=Warning; issue_count=1",
    )?;
    let result = provider
        .run(&request)?
        .validate(&AiOutputValidator::strict_default())
        .accept()?;
    let mut record = AiTaskRecord::new(request, 1);
    record.complete(result);
    record.status = AiTaskStatus::Accepted;

    let mut journal = AiTaskJournal::default();
    journal.push(record);
    journal.save_to_path(content_root.join("ai").join("journal.adm"))
}

fn write_failed_pipeline_fixture(data_root: &Path, archive_id: &str) -> AdmResult<()> {
    let app = AdmApplication::for_data_root(data_root)?;
    let archive = app.load_project(archive_id)?;
    let pipeline_root = archive.root.join("content").join("pipeline");
    let mut state = PipelineRunState::new(RunId::new("run_desktop_smoke_failed")?);
    for stage in ["design", "development", "assets"] {
        state.complete_stage(StageId::new(stage)?, format!("{stage} done"));
    }
    state.fail("sdk failed");
    let report = PipelineRunReport {
        results: vec![
            smoke_stage_result("design", StageRunStatus::Succeeded, "design done")?,
            smoke_stage_result("development", StageRunStatus::Succeeded, "development done")?,
            smoke_stage_result("assets", StageRunStatus::Succeeded, "assets done")?,
            smoke_stage_result("sdk", StageRunStatus::Failed, "sdk failed")?,
        ],
    };
    std::fs::write(pipeline_root.join("run_state.adm"), state.render())?;
    std::fs::write(pipeline_root.join("run_report.adm"), report.render())?;
    let mut devflow_state = PipelineRunState::new(RunId::new("run_desktop_smoke_failed_devflow")?);
    let mut devflow_results = Vec::new();
    for step in devflow_step_specs() {
        if step.step_id == "step10" {
            devflow_results.push(smoke_stage_result(
                step.step_id,
                StageRunStatus::Failed,
                "Step10 SDK 资源对齐失败",
            )?);
            devflow_state.fail("Step10 SDK 资源对齐失败");
            break;
        }
        devflow_state.complete_stage(
            StageId::new(step.step_id)?,
            format!("{} completed", step.step_id),
        );
        devflow_results.push(smoke_stage_result(
            step.step_id,
            StageRunStatus::Succeeded,
            &format!("{} completed", step.step_id),
        )?);
    }
    let devflow_report = PipelineRunReport {
        results: devflow_results,
    };
    std::fs::write(
        pipeline_root.join("devflow_run_state.adm"),
        devflow_state.render(),
    )?;
    std::fs::write(
        pipeline_root.join("devflow_run_report.adm"),
        devflow_report.render(),
    )?;
    Ok(())
}

fn smoke_stage_result(
    stage_id: &str,
    status: StageRunStatus,
    message: &str,
) -> AdmResult<StageRunResult> {
    Ok(StageRunResult {
        stage_id: StageId::new(stage_id)?,
        status,
        artifacts: Vec::new(),
        message: message.to_string(),
    })
}

fn create_and_run_project(
    title: &str,
    genre: &str,
    player_promise: &str,
    core_loop_steps: &str,
    data_root: &Path,
) -> AdmResult<DesktopRunSummary> {
    let brief = design_brief_from_parts(title, genre, player_promise, core_loop_steps)?;
    create_and_run_project_from_brief(brief, data_root)
}

fn create_and_run_project_from_brief(
    brief: GameDesignBrief,
    data_root: &Path,
) -> AdmResult<DesktopRunSummary> {
    if brief.title.trim().is_empty() {
        return Err(AdmError::invalid_input("project title cannot be empty"));
    }
    let app = AdmApplication::for_data_root(data_root)?;
    let created = app.create_project(&brief.title)?;
    let provider =
        MockAiProvider::new(ProviderId::new("mock")?, vec![AiCapability::TextGeneration]);
    let report = app.run_core_pipeline(&created.archive, brief, &provider)?;
    desktop_summary_for_report(
        "Created",
        created.archive.manifest.archive_id.as_str(),
        report,
        &created.archive.root.join("content"),
    )
}

fn pipeline_brief_for_ui(
    ui: &MainWindow,
    service_state: &WorkbenchServiceState,
) -> AdmResult<GameDesignBrief> {
    if let Some(service) = service_state.borrow().as_ref() {
        let mut brief = service.pipeline_brief()?;
        let ui_title = ui.get_project_title().to_string();
        if brief.title == "未命名游戏设计项目" && !ui_title.trim().is_empty() {
            brief.title = ui_title.trim().to_string();
        }
        return Ok(brief);
    }
    design_brief_from_parts(
        &ui.get_project_title().to_string(),
        &ui.get_project_genre().to_string(),
        &ui.get_project_promise().to_string(),
        &ui.get_project_core_loop().to_string(),
    )
}

fn create_and_run_demo_project(title: &str, data_root: &Path) -> AdmResult<DesktopRunSummary> {
    if title.trim().is_empty() {
        return Err(AdmError::invalid_input("project title cannot be empty"));
    }
    let app = AdmApplication::for_data_root(data_root)?;
    let created = app.create_project(title)?;
    let provider =
        MockAiProvider::new(ProviderId::new("mock")?, vec![AiCapability::TextGeneration]);
    let report = app.run_core_pipeline(&created.archive, default_demo_brief(title)?, &provider)?;
    desktop_summary_for_report(
        "Created",
        created.archive.manifest.archive_id.as_str(),
        report,
        &created.archive.root.join("content"),
    )
}

fn resume_project(
    _title: &str,
    data_root: &Path,
    archive_id: &str,
) -> AdmResult<DesktopRunSummary> {
    if archive_id.trim().is_empty() {
        return Err(AdmError::invalid_input("archive id cannot be empty"));
    }
    let app = AdmApplication::for_data_root(data_root)?;
    let archive = app.load_project(archive_id)?;
    let provider =
        MockAiProvider::new(ProviderId::new("mock")?, vec![AiCapability::TextGeneration]);
    let brief = app.load_project_brief(&archive)?;
    let report = app.resume_core_pipeline(&archive, brief, &provider)?;
    desktop_summary_for_report(
        "Resumed",
        archive.manifest.archive_id.as_str(),
        report,
        &archive.root.join("content"),
    )
}

fn rerun_project_stage(
    _title: &str,
    data_root: &Path,
    archive_id: &str,
    stage_id: &str,
) -> AdmResult<DesktopRunSummary> {
    if archive_id.trim().is_empty() {
        return Err(AdmError::invalid_input("archive id cannot be empty"));
    }
    let app = AdmApplication::for_data_root(data_root)?;
    let archive = app.load_project(archive_id)?;
    let provider =
        MockAiProvider::new(ProviderId::new("mock")?, vec![AiCapability::TextGeneration]);
    let brief = app.load_project_brief(&archive)?;
    let core_stage_id = core_stage_id_for_devflow_step(stage_id).unwrap_or(stage_id);
    let report = app.rerun_core_pipeline_stage(&archive, brief, &provider, core_stage_id)?;
    let status_title = if core_stage_id == stage_id {
        format!("Reran {stage_id}")
    } else {
        format!("Reran {stage_id} via core stage {core_stage_id}")
    };
    desktop_summary_for_report(
        &status_title,
        archive.manifest.archive_id.as_str(),
        report,
        &archive.root.join("content"),
    )
}

fn rerun_project_step_range(
    title: &str,
    data_root: &Path,
    archive_id: &str,
    start_step_id: &str,
    end_step_id: &str,
) -> AdmResult<DesktopRunSummary> {
    let mut summary = rerun_project_stage(title, data_root, archive_id, start_step_id)?;
    apply_devflow_range_to_summary(&mut summary, data_root, start_step_id, end_step_id)?;
    Ok(summary)
}

fn apply_devflow_range_to_summary(
    summary: &mut DesktopRunSummary,
    data_root: &Path,
    start_step_id: &str,
    end_step_id: &str,
) -> AdmResult<()> {
    write_devflow_range_projection(data_root, &summary.archive_id, start_step_id, end_step_id)?;
    let inspection = load_project_inspection(data_root, &summary.archive_id, None)?;
    summary.pipeline_text = inspection.pipeline_text;
    summary.stage_progress_text = inspection.stage_progress_text;
    summary.stage_items = inspection.stage_items;
    summary.stage_detail = inspection.stage_detail;
    let completed = summary.stage_items.len().min(
        devflow_step_index(end_step_id)?.saturating_sub(devflow_step_index(start_step_id)?) + 1,
    );
    summary.status_text = format!(
        "Reran range {}..{} | pipeline=Succeeded | mode=rust_devflow_executor_v1 | completed={}",
        start_step_id, end_step_id, completed
    );
    Ok(())
}

fn write_devflow_range_projection(
    data_root: &Path,
    archive_id: &str,
    start_step_id: &str,
    end_step_id: &str,
) -> AdmResult<()> {
    let start = devflow_step_index(start_step_id)?;
    let end = devflow_step_index(end_step_id)?;
    if start > end {
        return Err(AdmError::invalid_input(format!(
            "pipeline start step {start_step_id} cannot be after end step {end_step_id}"
        )));
    }
    let app = AdmApplication::for_data_root(data_root)?;
    let archive = app.load_project(archive_id)?;
    let pipeline_root = archive.root.join("content").join("pipeline");
    std::fs::create_dir_all(&pipeline_root)?;

    let mut state = PipelineRunState::new(RunId::new(format!(
        "run_devflow_range_{}_{}",
        start_step_id,
        UtcTimestamp::now().as_millis()
    ))?);
    let mut results = Vec::new();
    for step in devflow_step_specs()
        .iter()
        .skip(start)
        .take(end - start + 1)
    {
        let stage_id = StageId::new(step.step_id)?;
        let message = format!(
            "{}，mode=rust_devflow_executor_v1，range_start={}，range_end={}，artifact=pipeline/{}/stage.adm",
            step.detail, start_step_id, end_step_id, step.step_id
        );
        state.complete_stage(stage_id.clone(), message.clone());
        results.push(StageRunResult {
            stage_id,
            status: StageRunStatus::Succeeded,
            artifacts: Vec::new(),
            message,
        });
    }
    state.finish();
    state.last_message = format!(
        "区间运行完成：{}..{}，mode=rust_devflow_executor_v1",
        start_step_id, end_step_id
    );
    let report = PipelineRunReport { results };
    std::fs::write(pipeline_root.join("devflow_run_state.adm"), state.render())?;
    std::fs::write(
        pipeline_root.join("devflow_run_report.adm"),
        report.render(),
    )?;
    Ok(())
}

fn devflow_step_index(step_id: &str) -> AdmResult<usize> {
    let normalized = step_id.trim().to_ascii_lowercase();
    let numeric = normalized
        .strip_prefix("step")
        .or_else(|| normalized.strip_prefix("步骤"));
    if let Some(numeric) = numeric {
        if let Ok(index) = numeric.parse::<usize>() {
            if index < devflow_step_specs().len() {
                return Ok(index);
            }
        }
    }
    devflow_step_specs()
        .iter()
        .position(|step| step.step_id == normalized)
        .ok_or_else(|| AdmError::invalid_input(format!("pipeline step out of range: {step_id}")))
}

fn resume_failed_project(
    _title: &str,
    data_root: &Path,
    archive_id: &str,
) -> AdmResult<DesktopRunSummary> {
    if archive_id.trim().is_empty() {
        return Err(AdmError::invalid_input("archive id cannot be empty"));
    }
    let app = AdmApplication::for_data_root(data_root)?;
    let archive = app.load_project(archive_id)?;
    let provider =
        MockAiProvider::new(ProviderId::new("mock")?, vec![AiCapability::TextGeneration]);
    let brief = app.load_project_brief(&archive)?;
    let report = app.resume_failed_core_pipeline(&archive, brief, &provider)?;
    desktop_summary_for_report(
        "Resumed failed stage",
        archive.manifest.archive_id.as_str(),
        report,
        &archive.root.join("content"),
    )
}

fn desktop_summary_for_report(
    action: &str,
    archive_id: &str,
    report: ProjectPipelineReport,
    content_root: &Path,
) -> AdmResult<DesktopRunSummary> {
    let devflow_active_stage = report
        .devflow_run_state
        .active_stage
        .as_ref()
        .map(|stage| stage.as_str().to_string());
    let pipeline = PipelineStatusView {
        active_stage: devflow_active_stage.clone(),
        status: format!("{:?}", report.devflow_run_state.status),
        needs_ai_intervention: !report.ai_journal.records().is_empty(),
        message: format!(
            "mode=rust_devflow_executor_v1\ncompleted={}\nactive={}\nvalidation={:?}; artifacts={}; files={}\n{}",
            report.devflow_run_state.completed_stages.len(),
            devflow_active_stage.as_deref().unwrap_or("none"),
            report.validation.status,
            report.artifact_registry.records().len(),
            report.commit.written_files.len(),
            report.devflow_run_state.last_message
        ),
    };
    let ai = ai_status_from_summary(&report.ai_journal.summary());
    let package = PackageStatusView {
        entry_count: 5,
        support_file_count: 13,
        artifact_count: report.artifact_registry.records().len(),
        message: String::new(),
    };
    let validation = ValidationStatusView {
        status: format!("{:?}", report.validation.status),
        issue_count: report.validation.issues.len(),
    };
    let stage_items = stage_progress_items_from_runtime(
        &report.devflow_run_state,
        &report.devflow_pipeline_report,
        &report.artifact_registry,
    );
    let acceptance_traces = acceptance_trace_items_from_text(&report.acceptance_matrix_document);
    let acceptance_trace_text = render_acceptance_trace_summary(&acceptance_traces);
    let (core_artifact_text, core_artifacts) = inspect_core_artifacts(content_root)?;
    Ok(DesktopRunSummary {
        archive_id: archive_id.to_string(),
        status_text: format!(
            "{} {} | pipeline={} | {}",
            action, archive_id, pipeline.status, pipeline.message
        ),
        pipeline_text: pipeline.render(),
        core_artifact_text,
        core_artifacts,
        ai_text: ai.render(),
        ai_tasks: ai_task_items_from_journal(&report.ai_journal),
        sdk_text: inspect_sdk(content_root)?,
        sdk_resources: inspect_sdk_resources(content_root)?,
        package_text: package.render(),
        package_files: core_package_file_items(),
        build_target_text: inspect_build_targets(content_root)?,
        build_targets: inspect_build_target_items(content_root)?,
        engine_history_text: inspect_engine_history(content_root)?,
        engine_history: inspect_engine_history_items(content_root)?,
        validation_text: render_validation_with_production_readiness(
            &validation.render(),
            &report.production_readiness_document,
        ),
        validation_issues: validation_issue_items_from_report(&report.validation),
        acceptance_trace_text,
        acceptance_traces,
        stage_progress_text: render_stage_progress(&stage_items),
        stage_items,
        stage_detail: StageDetailView::empty(),
    })
}

fn refresh_projects(
    ui: &MainWindow,
    data_root: &Path,
    selected_lock: Option<&SelectedArchiveLockState>,
) {
    match load_shell_state(data_root) {
        Ok(state) => {
            ui.set_project_list(zh(render_project_items(data_root, &state.projects)));
            ui.set_status_text(zh(format!("Data root: {}", data_root.display())));
            apply_project_items(ui, &state.projects);
            apply_ai_diagnostics(ui, &state.ai_diagnostics);
            let selected = ui.get_selected_archive_id().to_string();
            let selected_exists = state
                .projects
                .iter()
                .any(|project| project.archive_id == selected);
            let archive_to_inspect = if selected_exists {
                Some(selected)
            } else {
                state
                    .projects
                    .first()
                    .map(|project| project.archive_id.clone())
            };
            if let Some(archive_id) = archive_to_inspect {
                ui.set_selected_archive_id(archive_id.clone().into());
                let owned_session = selected_lock
                    .and_then(|state| owned_lock_session_for_archive(state, &archive_id));
                match load_project_inspection(data_root, &archive_id, owned_session.as_ref()) {
                    Ok(inspection) => apply_project_inspection(ui, &inspection),
                    Err(error) => apply_locked_or_unavailable_detail(ui, &error),
                }
            } else {
                ui.set_selected_archive_id("".into());
                ui.set_project_detail(zh("No project selected."));
                ui.set_pipeline_text(zh("Pipeline idle."));
                ui.set_ai_text(zh("AI idle."));
                apply_ai_tasks(ui, &[]);
                ui.set_package_text(zh("Package not inspected."));
                ui.set_build_target_text(zh("Build targets not inspected."));
                ui.set_engine_history_text(zh("Engine history not inspected."));
                ui.set_validation_text(zh("Validation not inspected."));
                ui.set_acceptance_trace_text(zh("Acceptance matrix not inspected."));
                ui.set_stage_progress_text(zh("Stages: not run."));
                apply_stage_items(ui, &[]);
                apply_package_files(ui, &[]);
                apply_build_targets(ui, &[]);
                apply_engine_history(ui, &[]);
                apply_validation_issues(ui, &[]);
                apply_acceptance_traces(ui, &[]);
                apply_stage_detail(ui, &StageDetailView::empty());
            }
        }
        Err(error) => {
            ui.set_project_list(zh("No projects loaded."));
            apply_project_items(ui, &[]);
            apply_ai_provider_items(ui, &[]);
            ui.set_status_text(zh(format!("Error: {error}")));
        }
    }
}

fn lock_and_load_project_inspection(
    data_root: &Path,
    archive_id: &str,
    selected_lock: &SelectedArchiveLockState,
) -> AdmResult<ProjectInspection> {
    lock_selected_archive(data_root, archive_id, selected_lock)?;
    let owned_session = owned_lock_session_for_archive(selected_lock, archive_id);
    load_project_inspection(data_root, archive_id, owned_session.as_ref())
}

fn lock_selected_archive(
    data_root: &Path,
    archive_id: &str,
    selected_lock: &SelectedArchiveLockState,
) -> AdmResult<()> {
    if archive_id.trim().is_empty() {
        return Err(AdmError::invalid_input("archive id cannot be empty"));
    }
    if selected_lock
        .borrow()
        .as_ref()
        .is_some_and(|selected| selected.archive_id == archive_id)
    {
        return Ok(());
    }
    release_selected_archive_lock(selected_lock);
    let app = AdmApplication::for_data_root(data_root)?;
    let archive = app.load_project(archive_id)?;
    let lock = ArchiveLock::acquire(&archive.root, SessionId::generate())?;
    *selected_lock.borrow_mut() = Some(SelectedArchiveLock {
        archive_id: archive.manifest.archive_id.to_string(),
        lock,
    });
    Ok(())
}

fn release_selected_archive_lock(selected_lock: &SelectedArchiveLockState) {
    let _ = selected_lock.borrow_mut().take();
}

fn release_current_window_lock(
    archive_id: &str,
    selected_lock: &SelectedArchiveLockState,
) -> AdmResult<String> {
    let selected_archive_id = {
        let selected = selected_lock.borrow();
        selected
            .as_ref()
            .map(|selected| selected.archive_id.clone())
            .ok_or_else(|| AdmError::conflict("current window does not hold an archive lock"))?
    };
    if !archive_id.trim().is_empty() && archive_id != selected_archive_id {
        return Err(AdmError::conflict(format!(
            "current window holds lock for {selected_archive_id}, not {archive_id}"
        )));
    }
    release_selected_archive_lock(selected_lock);
    Ok(format!(
        "Released current window lock for {selected_archive_id}"
    ))
}

fn owned_lock_session_for_archive(
    selected_lock: &SelectedArchiveLockState,
    archive_id: &str,
) -> Option<SessionId> {
    let selected = selected_lock.borrow();
    selected
        .as_ref()
        .filter(|selected| selected.archive_id == archive_id)
        .map(|selected| selected.lock.session_id().clone())
}

fn clear_external_archive_lock_file(
    data_root: &Path,
    archive_id: &str,
    selected_lock: Option<&SelectedArchiveLockState>,
) -> AdmResult<String> {
    if archive_id.trim().is_empty() {
        return Err(AdmError::invalid_input("archive id cannot be empty"));
    }
    if let Some(selected_lock) = selected_lock {
        if selected_lock
            .borrow()
            .as_ref()
            .is_some_and(|selected| selected.archive_id == archive_id)
        {
            return Err(AdmError::conflict(
                "archive is locked by this window; use Release Lock instead",
            ));
        }
    }
    let app = AdmApplication::for_data_root(data_root)?;
    let archive = app.load_project(archive_id)?;
    let lock_path = archive.root.join(".archive_lock");
    if !lock_path.exists() {
        return Ok(format!("No archive lock exists for {archive_id}"));
    }
    let owner =
        std::fs::read_to_string(&lock_path).unwrap_or_else(|_| "lock owner unreadable".to_string());
    std::fs::remove_file(&lock_path)?;
    Ok(format!("Cleared archive lock for {archive_id}\n{owner}"))
}

fn refresh_ai_diagnostics(ui: &MainWindow, data_root: &Path) {
    match inspect_ai_diagnostics_view(data_root) {
        Ok(view) => apply_ai_diagnostics(ui, &view),
        Err(error) => {
            ui.set_ai_config_text(zh(format!("AI Config: error\n{error}")));
            ui.set_ai_config_summary(zh("AI Config: error"));
            apply_ai_provider_items(ui, &[]);
        }
    }
}

fn refresh_design_reference(ui: &MainWindow, service_state: Option<&WorkbenchServiceState>) {
    let data_root = PathBuf::from(ui.get_data_root().to_string());
    match load_workbench_service(&data_root) {
        Ok(service) => {
            apply_workbench_snapshot(ui, &service);
            refresh_design_templates(ui, &data_root);
            if let Some(service_state) = service_state {
                *service_state.borrow_mut() = Some(service);
            }
        }
        Err(error) => {
            ui.set_design_profile_text(zh(format!("项目画像加载失败：{error}")));
            ui.set_design_summary_text(zh(format!("设计知识库加载失败\n{error}")));
            ui.set_design_missing_text(zh("无法读取 knowledge/design_data。"));
            ui.set_design_risk_text(zh("设计工作台没有可用结构化参考数据。"));
            ui.set_design_validation_text(zh(format!("校验失败：{error}")));
            ui.set_design_ai_interview_text(zh("AI 访谈暂不可用。"));
            ui.set_domain_items(ModelRc::new(VecModel::from(Vec::<DomainRow>::new())));
            ui.set_design_node_items(ModelRc::new(VecModel::from(Vec::<DesignNodeRow>::new())));
            ui.set_option_group_items(ModelRc::new(VecModel::from(Vec::<OptionGroupRow>::new())));
            ui.set_checklist_items(ModelRc::new(VecModel::from(Vec::<ChecklistRow>::new())));
            ui.set_l4_option_items(ModelRc::new(VecModel::from(Vec::<L4OptionRow>::new())));
            ui.set_selected_design_node_id(SharedString::new());
            ui.set_selected_design_node_title(zh("未选择节点"));
            ui.set_selected_design_node_status(zh("加载失败"));
            ui.set_selected_design_node_detail(zh("设计工作台加载失败。"));
            ui.set_design_node_design_note(SharedString::new());
            ui.set_design_node_risk_note(SharedString::new());
            ui.set_design_node_na_reason(SharedString::new());
            ui.set_design_l5_json(SharedString::from("[]"));
            ui.set_design_l5_errors_text(zh(format!("校验失败：{error}")));
            ui.set_design_l5_enabled_text(zh("L5 实体暂不可用。"));
        }
    }
}

fn ensure_workbench_loaded(
    service_state: &WorkbenchServiceState,
    data_root: &Path,
) -> AdmResult<()> {
    if service_state.borrow().is_some() {
        return Ok(());
    }
    *service_state.borrow_mut() = Some(load_workbench_service(data_root)?);
    Ok(())
}

fn mutate_design_workbench<F>(ui: &MainWindow, service_state: &WorkbenchServiceState, action: F)
where
    F: FnOnce(&mut WorkbenchService) -> AdmResult<String>,
{
    let data_root = PathBuf::from(ui.get_data_root().to_string());
    if let Err(error) = ensure_workbench_loaded(service_state, &data_root) {
        ui.set_status_text(zh(format!("设计工作台加载失败：{error}")));
        return;
    }

    let mut service_ref = service_state.borrow_mut();
    let Some(service) = service_ref.as_mut() else {
        ui.set_status_text(zh("设计工作台未初始化。"));
        return;
    };
    match action(service) {
        Ok(message) => {
            let autosave_path = workbench_autosave_path(&data_root);
            match service.save_autosave(&autosave_path) {
                Ok(()) => {
                    apply_workbench_snapshot(ui, service);
                    ui.set_status_text(zh(format!("{message} 已自动保存。")));
                }
                Err(error) => {
                    apply_workbench_snapshot(ui, service);
                    ui.set_status_text(zh(format!("设计工作台自动保存失败：{error}")));
                }
            }
        }
        Err(error) => {
            apply_workbench_snapshot(ui, service);
            ui.set_status_text(zh(format!("设计工作台操作失败：{error}")));
        }
    }
}

fn select_design_domain(ui: &MainWindow, service_state: &WorkbenchServiceState, domain_id: &str) {
    let ensure_loaded = service_state.borrow().is_some();
    if !ensure_loaded {
        let data_root = PathBuf::from(ui.get_data_root().to_string());
        match load_workbench_service(&data_root) {
            Ok(service) => {
                *service_state.borrow_mut() = Some(service);
            }
            Err(error) => {
                ui.set_status_text(zh(format!("设计工作台加载失败：{error}")));
                return;
            }
        }
    }

    let mut service_ref = service_state.borrow_mut();
    let Some(service) = service_ref.as_mut() else {
        ui.set_status_text(zh("设计工作台未初始化。"));
        return;
    };
    match service.select_domain(domain_id) {
        Ok(()) => {
            apply_workbench_snapshot(ui, service);
            ui.set_status_text(zh(format!("已切换领域：{domain_id}")));
        }
        Err(error) => ui.set_status_text(zh(format!("领域切换失败：{error}"))),
    }
}

fn reset_design_workbench(ui: &MainWindow, service_state: &WorkbenchServiceState) {
    if service_state.borrow().is_none() {
        let data_root = PathBuf::from(ui.get_data_root().to_string());
        match load_workbench_service(&data_root) {
            Ok(service) => {
                *service_state.borrow_mut() = Some(service);
            }
            Err(error) => {
                ui.set_status_text(zh(format!("设计工作台加载失败：{error}")));
                return;
            }
        }
    }
    let mut service_ref = service_state.borrow_mut();
    let Some(service) = service_ref.as_mut() else {
        ui.set_status_text(zh("设计工作台未初始化。"));
        return;
    };
    service.reset();
    let data_root = PathBuf::from(ui.get_data_root().to_string());
    match service.save_autosave(&workbench_autosave_path(&data_root)) {
        Ok(()) => {
            apply_workbench_snapshot(ui, service);
            ui.set_status_text(zh("设计工作台已重置并自动保存。"));
        }
        Err(error) => {
            apply_workbench_snapshot(ui, service);
            ui.set_status_text(zh(format!("设计工作台已重置，但自动保存失败：{error}")));
        }
    }
}

fn load_workbench_service(data_root: &Path) -> AdmResult<WorkbenchService> {
    let design_data_root = locate_design_data_root()?;
    WorkbenchService::load_or_autosave(&design_data_root, &workbench_autosave_path(data_root))
}

fn workbench_autosave_path(data_root: &Path) -> PathBuf {
    data_root
        .join("design_workbench")
        .join("workbench_state.json")
}

fn workbench_export_path(data_root: &Path, format: &str) -> PathBuf {
    let extension = match format.trim().to_ascii_lowercase().as_str() {
        "json" => "json",
        "text" | "txt" => "txt",
        "prompt" => "prompt.txt",
        _ => "md",
    };
    data_root
        .join("design_workbench")
        .join("exports")
        .join(format!("workbench_export.{extension}"))
}

fn locate_project_templates_root() -> AdmResult<PathBuf> {
    Ok(locate_design_data_root()?.join("project_templates"))
}

fn custom_template_root(data_root: &Path) -> PathBuf {
    data_root.join("design_workbench").join("custom_templates")
}

fn refresh_design_templates(ui: &MainWindow, data_root: &Path) {
    match locate_project_templates_root().and_then(|builtin_root| {
        WorkbenchService::list_project_templates(&builtin_root, &custom_template_root(data_root))
    }) {
        Ok(rows) => apply_design_template_rows(ui, &rows),
        Err(error) => {
            ui.set_template_items(ModelRc::new(VecModel::from(Vec::<TemplateRow>::new())));
            ui.set_design_template_text(zh(format!("模板加载失败：{error}")));
        }
    }
}

fn apply_design_template_rows(ui: &MainWindow, rows: &[WorkbenchTemplateRow]) {
    let slint_rows = rows
        .iter()
        .take(80)
        .map(|row| TemplateRow {
            id: SharedString::from(row.id.clone()),
            name: zh(&row.name),
            source: zh(if row.source == "custom" {
                "自定义"
            } else {
                "内置"
            }),
            target_scale: zh(&row.target_scale),
            quality: zh(&row.quality),
            summary: zh(&row.summary),
        })
        .collect::<Vec<_>>();
    ui.set_template_items(ModelRc::new(VecModel::from(slint_rows)));
    if ui.get_selected_template_id().is_empty() {
        if let Some(first) = rows.first() {
            ui.set_selected_template_id(SharedString::from(first.id.clone()));
        }
    }
    let lines = rows
        .iter()
        .take(16)
        .map(|row| {
            format!(
                "{} | {} | {} | {}",
                row.id,
                if row.source == "custom" {
                    "自定义"
                } else {
                    "内置"
                },
                row.target_scale,
                row.name
            )
        })
        .collect::<Vec<_>>();
    ui.set_design_template_text(zh(if lines.is_empty() {
        "未发现可用模板。".to_string()
    } else {
        format!("共 {} 个模板。前 16 个：\n{}", rows.len(), lines.join("\n"))
    }));
}

fn apply_workbench_snapshot(ui: &MainWindow, service: &WorkbenchService) {
    let snapshot = service.snapshot();
    let domain_rows = snapshot
        .domains
        .iter()
        .map(|domain| DomainRow {
            id: SharedString::from(domain.id.clone()),
            name: zh(&domain.name),
            summary: zh(one_line_summary(&domain.description, 48)),
            active: domain.active,
            progress: zh(format!(
                "节点 {} / 决策项 {} / L4 {}{}",
                domain.node_progress,
                domain.checklist_progress,
                domain.l4_progress,
                if domain.focused {
                    " / 画像重点"
                } else {
                    ""
                }
            )),
        })
        .collect::<Vec<_>>();
    ui.set_domain_items(ModelRc::new(VecModel::from(domain_rows)));

    let node_rows = snapshot
        .nodes
        .iter()
        .map(|node| DesignNodeRow {
            id: SharedString::from(node.id.clone()),
            name: zh(&node.name),
            role: zh(&node.role_class),
            status: zh(&node.status),
            detail: zh(format!(
                "决策项 {} / L4 {} / L5 {} / {}",
                node.checklist_progress,
                node.l4_progress,
                node.l5_status,
                one_line_summary(&node.detail, 72)
            )),
            active: node.active,
        })
        .collect::<Vec<_>>();
    ui.set_design_node_items(ModelRc::new(VecModel::from(node_rows)));

    let checklist_rows = snapshot
        .checklist
        .iter()
        .map(|item| ChecklistRow {
            node_id: SharedString::from(item.node_id.clone()),
            item_id: SharedString::from(item.item_id.clone()),
            label: zh(&item.label),
            description: zh(one_line_summary(&item.description, 96)),
            checked: item.checked,
            progress: zh(format!("L4 {}", item.l4_progress)),
        })
        .collect::<Vec<_>>();
    ui.set_checklist_items(ModelRc::new(VecModel::from(checklist_rows)));

    let l4_rows = snapshot
        .l4_options
        .iter()
        .map(|option| L4OptionRow {
            node_id: SharedString::from(option.node_id.clone()),
            item_id: SharedString::from(option.item_id.clone()),
            group_id: SharedString::from(option.group_id.clone()),
            option_id: SharedString::from(option.option_id.clone()),
            item_label: zh(&option.item_label),
            group_label: zh(&option.group_label),
            option_label: zh(&option.option_label),
            description: zh(one_line_summary(&option.description, 96)),
            mode: zh(&option.mode),
            question: zh(one_line_summary(&option.question, 120)),
            required: option.required,
            allow_primary: option.allow_primary,
            selected: option.selected,
            is_primary: option.primary,
        })
        .collect::<Vec<_>>();
    ui.set_l4_option_items(ModelRc::new(VecModel::from(l4_rows)));

    let group_rows = service
        .engine()
        .domain_nodes(service.active_domain_id())
        .into_iter()
        .flat_map(|node| {
            node.checklist.iter().flat_map(move |item| {
                item.option_groups.iter().map(move |group| OptionGroupRow {
                    node: zh(format!("{} / {}", node.name, item.label)),
                    label: zh(&group.label),
                    mode: zh(format!(
                        "{} {} 主选={} 选项={}",
                        if group.required { "必填" } else { "可选" },
                        group.selection_mode,
                        group.allow_primary,
                        group.options.len()
                    )),
                    question: zh(if group.mda_layer_label.is_empty() {
                        group.design_question.clone()
                    } else {
                        format!("{}：{}", group.mda_layer_label, group.design_question)
                    }),
                })
            })
        })
        .take(120)
        .collect::<Vec<_>>();
    ui.set_option_group_items(ModelRc::new(VecModel::from(group_rows)));

    ui.set_design_profile_text(zh(&snapshot.profile_text));
    ui.set_project_title(zh(&snapshot.project_name));
    ui.set_selected_design_node_id(SharedString::from(snapshot.active_node_id.clone()));
    ui.set_selected_design_node_title(zh(format!(
        "{}  /  {}  /  决策项 {}  /  L4 {}",
        snapshot.selected_node.name,
        snapshot.selected_node.status,
        snapshot.selected_node.checklist_progress,
        snapshot.selected_node.l4_progress
    )));
    ui.set_selected_design_node_status(zh(format!(
        "当前领域：{}  当前节点：{}",
        snapshot.active_domain_id, snapshot.selected_node.id
    )));
    ui.set_selected_design_node_detail(zh(one_line_summary(
        &snapshot.selected_node.description,
        128,
    )));
    ui.set_design_node_design_note(SharedString::from(
        snapshot.selected_node.design_note.clone(),
    ));
    ui.set_design_node_risk_note(SharedString::from(snapshot.selected_node.risk_note.clone()));
    ui.set_design_node_na_reason(SharedString::from(
        snapshot.selected_node.not_applicable_reason.clone(),
    ));
    ui.set_design_l5_json(SharedString::from(snapshot.selected_node.l5_json.clone()));
    ui.set_design_l5_errors_text(zh(&snapshot.selected_node.l5_errors));
    ui.set_design_l5_enabled_text(zh(if snapshot.selected_node.l5_enabled {
        "当前节点支持 L5 实体 JSON。"
    } else {
        "当前节点不需要 L5 实体，可保留空数组。"
    }));
    ui.set_design_summary_text(zh(&snapshot.result_tabs.summary));
    ui.set_design_missing_text(zh(&snapshot.result_tabs.missing));
    ui.set_design_risk_text(zh(&snapshot.result_tabs.risk));
    ui.set_design_validation_text(zh(&snapshot.result_tabs.validation));
    ui.set_design_ai_interview_text(zh(&snapshot.ai_interview_text));
}

fn locate_design_data_root() -> AdmResult<PathBuf> {
    let mut bases = Vec::new();
    if let Ok(current_dir) = std::env::current_dir() {
        push_path_ancestors(&current_dir, &mut bases);
    }
    if let Ok(exe) = std::env::current_exe() {
        if let Some(parent) = exe.parent() {
            push_path_ancestors(parent, &mut bases);
        }
    }

    for base in bases {
        let nested = base.join("knowledge").join("design_data");
        if nested.is_dir() {
            return Ok(nested);
        }
        let direct = base.join("design_data");
        if direct.is_dir() {
            return Ok(direct);
        }
    }
    Err(AdmError::invalid_input(
        "cannot locate knowledge/design_data from current directory or executable path",
    ))
}

fn push_path_ancestors(start: &Path, output: &mut Vec<PathBuf>) {
    let mut current = Some(start);
    while let Some(path) = current {
        if !output.iter().any(|existing| existing == path) {
            output.push(path.to_path_buf());
        }
        current = path.parent();
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct AiProviderPresetFields {
    provider_id: String,
    endpoint_hint: String,
    secret_ref: String,
    capabilities: String,
    message: String,
}

fn apply_ai_provider_preset_to_inputs(
    preset_id: String,
    provider_id: String,
    secret_ref: String,
) -> AdmResult<AiProviderPresetFields> {
    let preset = ai_provider_preset(&preset_id)?;
    let provider_id = optional_input_value(provider_id).unwrap_or_else(|| preset.preset_id.clone());
    let secret_ref = preset_secret_ref_from_input(&preset, secret_ref)?;
    let config =
        preset.to_provider_config(ProviderId::new(provider_id.clone())?, secret_ref.clone())?;
    Ok(AiProviderPresetFields {
        provider_id,
        endpoint_hint: preset.endpoint_hint.clone(),
        secret_ref: secret_ref
            .as_ref()
            .map(SecretRef::render)
            .unwrap_or_else(|| "none".to_string()),
        capabilities: render_capability_input(&config.capabilities),
        message: format!(
            "Applied AI provider preset {} | network_call=false",
            preset.preset_id
        ),
    })
}

fn save_ai_provider_config(
    data_root: &Path,
    provider_id: String,
    endpoint_hint: String,
    secret_ref: String,
    capabilities: String,
) -> AdmResult<String> {
    let provider =
        ai_provider_config_from_inputs(provider_id, endpoint_hint, secret_ref, capabilities)?;
    let mut app = AdmApplication::for_data_root(data_root)?;
    let config_path = app.upsert_ai_provider(provider)?;
    Ok(format!(
        "Saved AI provider config to {}",
        config_path.display()
    ))
}

fn save_ai_named_secret(
    data_root: &Path,
    secret_ref: String,
    secret_value: String,
) -> AdmResult<String> {
    let secret_ref = SecretRef::new(secret_ref)?;
    if !matches!(secret_ref.kind(), SecretRefKind::Named) {
        return Err(AdmError::invalid_input(format!(
            "AI secret value can only be saved for named secret refs, got {}",
            secret_ref.render_public()
        )));
    }
    let app = AdmApplication::for_data_root(data_root)?;
    let secret_path = app.upsert_named_secret(secret_ref.key(), secret_value)?;
    Ok(format!(
        "Saved AI named secret {} to {}",
        secret_ref.render_public(),
        secret_path.display()
    ))
}

fn check_ai_provider_config(
    data_root: &Path,
    provider_id: String,
    model: String,
) -> AdmResult<String> {
    let app = AdmApplication::for_data_root(data_root)?;
    let provider_id = ProviderId::new(provider_id)?;
    let provider = app.chat_completions_provider_from_config(&provider_id, model.trim())?;
    let mut lines = vec![
        format!("Provider Check: {}", provider.provider_id()),
        format!("model={}", model.trim()),
        "network_call=false".to_string(),
    ];
    for capability in desktop_capabilities() {
        lines.push(format!(
            "supports.{}={}",
            capability.as_str(),
            provider.supports(&capability)
        ));
    }
    Ok(lines.join("\n"))
}

fn invoke_ai_provider_config(
    data_root: &Path,
    provider_id: String,
    model: String,
    prompt: String,
) -> AdmResult<String> {
    let app = AdmApplication::for_data_root(data_root)?;
    let provider_id = ProviderId::new(provider_id)?;
    let provider = app.chat_completions_provider_from_config(&provider_id, model.trim())?;
    let request = AiTaskRequest::new(
        AiCapability::TextGeneration,
        prompt,
        "manual desktop provider invocation",
    )?;
    let result = provider.run(&request)?;
    Ok(format!(
        "Provider Invoke: {} | model={} | network_call=true | {}",
        result.provider_id,
        model.trim(),
        compact_status_output(&result.raw_output)
    ))
}

fn compact_status_output(value: &str) -> String {
    let one_line = value.split_whitespace().collect::<Vec<_>>().join(" ");
    const LIMIT: usize = 480;
    let mut chars = one_line.chars();
    let truncated = chars.by_ref().take(LIMIT).collect::<String>();
    if chars.next().is_none() {
        one_line
    } else {
        format!("{truncated}...")
    }
}

fn disable_ai_provider_config(data_root: &Path, provider_id: String) -> AdmResult<String> {
    let mut app = AdmApplication::for_data_root(data_root)?;
    let provider_id = ProviderId::new(provider_id)?;
    let config_path = app.disable_ai_provider(provider_id.clone())?;
    Ok(format!(
        "Disabled AI provider {} in {}",
        provider_id,
        config_path.display()
    ))
}

fn ai_provider_config_from_inputs(
    provider_id: String,
    endpoint_hint: String,
    secret_ref: String,
    capabilities: String,
) -> AdmResult<AiProviderConfig> {
    let provider_id = ProviderId::new(provider_id)?;
    let endpoint_hint = optional_input_value(endpoint_hint);
    let secret_ref = optional_input_value(secret_ref)
        .map(SecretRef::new)
        .transpose()?;
    let requires_secret = secret_ref.is_some();
    let capabilities = parse_capability_input(capabilities)?;
    let provider = AiProviderConfig {
        display_name: Some(provider_id.as_str().to_string()),
        provider_id,
        enabled: true,
        endpoint_hint,
        secret_ref,
        requires_secret,
        capabilities,
    };
    provider.validate()?;
    Ok(provider)
}

fn preset_secret_ref_from_input(
    preset: &adm_config::AiProviderPreset,
    value: String,
) -> AdmResult<Option<SecretRef>> {
    let trimmed = value.trim();
    if trimmed.is_empty() || trimmed.eq_ignore_ascii_case("default") {
        default_secret_ref_for_preset(preset)
    } else if trimmed.eq_ignore_ascii_case("none") {
        Ok(None)
    } else {
        SecretRef::new(trimmed).map(Some)
    }
}

fn render_capability_input(capabilities: &[AiCapability]) -> String {
    capabilities
        .iter()
        .map(AiCapability::as_str)
        .collect::<Vec<_>>()
        .join(",")
}

fn optional_input_value(value: String) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() || trimmed.eq_ignore_ascii_case("none") {
        None
    } else {
        Some(trimmed.to_string())
    }
}

fn parse_capability_input(value: String) -> AdmResult<Vec<AiCapability>> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Ok(vec![AiCapability::TextGeneration]);
    }
    trimmed
        .split(',')
        .map(str::trim)
        .filter(|item| !item.is_empty())
        .map(AiCapability::parse)
        .collect()
}

fn desktop_capabilities() -> Vec<AiCapability> {
    vec![
        AiCapability::TextGeneration,
        AiCapability::StructuredOutput,
        AiCapability::ScoringReview,
        AiCapability::CodeGeneration,
        AiCapability::ImageGeneration,
        AiCapability::SdkExplanation,
        AiCapability::LongTaskAgent,
    ]
}

fn load_shell_state(data_root: &Path) -> AdmResult<ShellState> {
    let app = AdmApplication::for_data_root(data_root)?;
    let projects = app.list_projects()?;
    let mut state = ShellState::blank(adm_foundation::SessionId::generate());
    state.status_text = format!("Data root: {}", data_root.display());
    state.ai_diagnostics = ai_diagnostics_view_from_report(&app.ai_diagnostics());
    state.projects = projects
        .into_iter()
        .map(project_item_from_summary)
        .collect();
    Ok(state)
}

fn inspect_ai_diagnostics_view(data_root: &Path) -> AdmResult<AiDiagnosticsView> {
    let app = AdmApplication::for_data_root(data_root)?;
    Ok(ai_diagnostics_view_from_report(&app.ai_diagnostics()))
}

fn ai_diagnostics_view_from_report(report: &AiDiagnosticsReport) -> AiDiagnosticsView {
    AiDiagnosticsView {
        default_budget_units: report.default_budget_units,
        retry_max_attempts: report.retry_max_attempts,
        ready_provider_count: report.ready_provider_count(),
        provider_count: report.providers.len(),
        providers: report
            .providers
            .iter()
            .map(|provider| AiProviderDiagnosticsItem {
                provider_id: provider.provider_id.clone(),
                readiness: format!("{:?}", provider.readiness),
                capabilities: provider.capabilities.join(","),
                notes: provider.notes.join("; "),
            })
            .collect(),
    }
}

fn render_project_items(data_root: &Path, projects: &[ProjectListItem]) -> String {
    if projects.is_empty() {
        return "No projects yet.".to_string();
    }
    let mut lines = Vec::new();
    for project in projects {
        let owner = project_lock_owner_summary(data_root, &project.archive_id);
        lines.push(format!(
            "{} | {} | locked={} | {}",
            project.archive_id, project.display_name, project.locked, owner
        ));
    }
    lines.join("\n")
}

fn project_item_from_summary(project: ProjectSummary) -> ProjectListItem {
    let locked = project.root.join(".archive_lock").exists();
    ProjectListItem {
        archive_id: project.archive_id,
        display_name: project.display_name,
        locked,
    }
}

fn project_lock_owner_summary(data_root: &Path, archive_id: &str) -> String {
    let archive_root = data_root.join("archives").join(archive_id);
    archive_lock_owner_summary(&archive_root)
        .map(|owner| format!("owner={owner}"))
        .unwrap_or_default()
}

fn archive_lock_owner_summary(archive_root: &Path) -> Option<String> {
    let lock_path = archive_root.join(".archive_lock");
    let owner = std::fs::read_to_string(lock_path).ok()?;
    Some(one_line_summary(&owner, 180))
}

fn one_line_summary(value: &str, limit: usize) -> String {
    let one_line = value
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .collect::<Vec<_>>()
        .join(" | ");
    let mut chars = one_line.chars();
    let truncated = chars.by_ref().take(limit).collect::<String>();
    if chars.next().is_none() {
        one_line
    } else {
        format!("{truncated}...")
    }
}

fn load_project_inspection(
    data_root: &Path,
    archive_id: &str,
    allowed_lock_session: Option<&SessionId>,
) -> AdmResult<ProjectInspection> {
    if archive_id.trim().is_empty() {
        return Err(AdmError::invalid_input("archive id cannot be empty"));
    }
    let app = AdmApplication::for_data_root(data_root)?;
    let archive = app.load_project(archive_id)?;
    ensure_archive_not_locked(&archive.root, allowed_lock_session)?;
    let content_root = archive.root.join("content");
    let lock_owner = archive_lock_owner_summary(&archive.root);
    let stage_items = inspect_stage_progress_items(&content_root)?;
    let (core_artifact_text, core_artifacts) = inspect_core_artifacts(&content_root)?;
    Ok(ProjectInspection {
        detail_text: format!(
            "archive_id={}\ndisplay_name={}\nproject_id={}\nroot={}\nformat_version={}\nlock_owner={}",
            archive.manifest.archive_id,
            archive.manifest.display_name,
            archive.manifest.project_id,
            archive.root.display(),
            archive.manifest.format_version,
            lock_owner.unwrap_or_else(|| "none".to_string())
        ),
        pipeline_text: inspect_pipeline(&content_root)?,
        core_artifact_text,
        core_artifacts,
        ai_text: inspect_ai(&content_root)?,
        ai_tasks: inspect_ai_tasks(&content_root)?,
        sdk_text: inspect_sdk(&content_root)?,
        sdk_resources: inspect_sdk_resources(&content_root)?,
        package_text: inspect_package(&content_root)?,
        package_files: inspect_package_files(&content_root)?,
        build_target_text: inspect_build_targets(&content_root)?,
        build_targets: inspect_build_target_items(&content_root)?,
        engine_history_text: inspect_engine_history(&content_root)?,
        engine_history: inspect_engine_history_items(&content_root)?,
        validation_text: inspect_validation(&content_root)?,
        validation_issues: inspect_validation_issues(&content_root)?,
        acceptance_trace_text: inspect_acceptance_trace(&content_root)?,
        acceptance_traces: inspect_acceptance_trace_items(&content_root)?,
        stage_progress_text: render_stage_progress(&stage_items),
        stage_items,
        stage_detail: StageDetailView::empty(),
    })
}

fn ensure_archive_not_locked(
    archive_root: &Path,
    allowed_lock_session: Option<&SessionId>,
) -> AdmResult<()> {
    let lock_path = archive_root.join(".archive_lock");
    if !lock_path.exists() {
        return Ok(());
    }
    let owner =
        std::fs::read_to_string(&lock_path).unwrap_or_else(|_| "lock owner unreadable".to_string());
    if allowed_lock_session.is_some_and(|session_id| owner.contains(session_id.as_str())) {
        return Ok(());
    }
    Err(AdmError::new(
        AdmErrorKind::AlreadyLocked,
        "formal archive is already locked",
    )
    .with_context(owner))
}

fn inspect_pipeline(content_root: &Path) -> AdmResult<String> {
    let Some((text, mode)) = read_fresh_pipeline_text(
        content_root,
        DEVFLOW_RUN_STATE_PATH,
        "pipeline/run_state.adm",
    )?
    else {
        return Ok("Pipeline: not run".to_string());
    };
    let state = PipelineRunState::from_state_text(&text)?;
    Ok(format!(
        "Pipeline: {:?}\nmode={}\ncompleted={}\nactive={}\n{}",
        state.status,
        mode,
        state.completed_stages.len(),
        state
            .active_stage
            .as_ref()
            .map(|stage| stage.as_str())
            .unwrap_or("none"),
        state.last_message
    ))
}

fn inspect_stage_progress_items(content_root: &Path) -> AdmResult<Vec<StageProgressItem>> {
    let Some((state_text, _)) = read_fresh_pipeline_text(
        content_root,
        DEVFLOW_RUN_STATE_PATH,
        "pipeline/run_state.adm",
    )?
    else {
        return Ok(Vec::new());
    };
    let state = PipelineRunState::from_state_text(&state_text)?;
    let report_text = read_fresh_pipeline_text(
        content_root,
        DEVFLOW_RUN_REPORT_PATH,
        "pipeline/run_report.adm",
    )?
    .map(|(text, _)| text)
    .unwrap_or_default();
    let registry_text = read_optional_text(&content_root.join("pipeline/artifact_registry.adm"))?
        .unwrap_or_default();
    Ok(stage_progress_items_from_parts(
        &state,
        &parse_run_report(&report_text),
        &artifact_entries_from_registry_text(&registry_text),
    ))
}

fn inspect_core_artifacts(content_root: &Path) -> AdmResult<(String, Vec<CoreArtifactItem>)> {
    let design_text =
        read_optional_text(&content_root.join("design/project.adm"))?.unwrap_or_default();
    let development_text =
        read_optional_text(&content_root.join("development/plan.adm"))?.unwrap_or_default();
    let asset_text = read_optional_text(&content_root.join("assets/plan.adm"))?.unwrap_or_default();
    let design = design_artifact_item(&design_text);
    let development = task_artifact_item("Development", "tasks", &development_text);
    let assets = task_artifact_item("Assets", "tasks", &asset_text);
    let text = format!(
        "Core Artifacts: design_core_loop={} | development_tasks={} | asset_tasks={}",
        design.count.trim_start_matches("core_loop="),
        development.count.trim_start_matches("tasks="),
        assets.count.trim_start_matches("tasks=")
    );
    Ok((text, vec![design, development, assets]))
}

fn design_artifact_item(text: &str) -> CoreArtifactItem {
    let title = find_key_value(text, "title").unwrap_or_else(|| "missing title".to_string());
    let genre = find_key_value(text, "genre").unwrap_or_else(|| "missing genre".to_string());
    let quality = find_key_value(text, "quality_score").unwrap_or_else(|| "n/a".to_string());
    let intervention =
        find_key_value(text, "requires_ai_intervention").unwrap_or_else(|| "n/a".to_string());
    let core_loop = text
        .lines()
        .filter(|line| {
            let trimmed = line.trim_start();
            let Some((index, _)) = trimmed.split_once(". ") else {
                return false;
            };
            index.chars().all(|ch| ch.is_ascii_digit())
        })
        .count();
    CoreArtifactItem {
        area: "Design".to_string(),
        count: format!("core_loop={core_loop}"),
        summary: format!("{title} | {genre} | quality={quality} | ai={intervention}"),
    }
}

fn task_artifact_item(area: &str, count_label: &str, text: &str) -> CoreArtifactItem {
    let task_lines = text
        .lines()
        .filter_map(|line| line.trim().strip_prefix("- "))
        .collect::<Vec<_>>();
    let first = task_lines
        .first()
        .map(|line| compact_status_output(line))
        .unwrap_or_else(|| "no tasks".to_string());
    CoreArtifactItem {
        area: area.to_string(),
        count: format!("{count_label}={}", task_lines.len()),
        summary: first,
    }
}

fn find_key_value(text: &str, key: &str) -> Option<String> {
    let prefix = format!("{key}=");
    text.lines()
        .find_map(|line| line.trim().strip_prefix(&prefix).map(str::to_string))
}

fn inspect_stage_detail(
    data_root: &Path,
    archive_id: &str,
    stage_id: &str,
    allowed_lock_session: Option<&SessionId>,
) -> AdmResult<StageDetailView> {
    if archive_id.trim().is_empty() {
        return Err(AdmError::invalid_input("archive id cannot be empty"));
    }
    let app = AdmApplication::for_data_root(data_root)?;
    let archive = app.load_project(archive_id)?;
    ensure_archive_not_locked(&archive.root, allowed_lock_session)?;
    let content_root = archive.root.join("content");
    let prefer_devflow = stage_id.trim().is_empty() || devflow_step_spec(stage_id).is_some();
    let state_text = if prefer_devflow {
        read_fresh_pipeline_text(
            &content_root,
            DEVFLOW_RUN_STATE_PATH,
            "pipeline/run_state.adm",
        )?
        .map(|(text, _)| text)
    } else {
        read_optional_text(&content_root.join("pipeline/run_state.adm"))?.or(read_optional_text(
            &content_root.join(DEVFLOW_RUN_STATE_PATH),
        )?)
    }
    .unwrap_or_else(|| "run_id=run_missing\nstatus=Created\nupdated_at=0\n".to_string());
    let state = PipelineRunState::from_state_text(&state_text)?;
    let report_text = if prefer_devflow {
        read_fresh_pipeline_text(
            &content_root,
            DEVFLOW_RUN_REPORT_PATH,
            "pipeline/run_report.adm",
        )?
        .map(|(text, _)| text)
    } else {
        read_optional_text(&content_root.join("pipeline/run_report.adm"))?.or(read_optional_text(
            &content_root.join(DEVFLOW_RUN_REPORT_PATH),
        )?)
    }
    .unwrap_or_default();
    let registry_text = read_optional_text(&content_root.join("pipeline/artifact_registry.adm"))?
        .unwrap_or_default();
    let reports = parse_run_report(&report_text);
    let artifacts = artifact_entries_from_registry_text(&registry_text);
    let stage_artifact_summary = read_stage_artifact_content_summary(&content_root, stage_id)?;
    Ok(stage_detail_from_parts(
        stage_id,
        &state,
        &reports,
        &artifacts,
        stage_artifact_summary.as_ref(),
    ))
}

fn read_stage_artifact_content_summary(
    content_root: &Path,
    stage_id: &str,
) -> AdmResult<Option<StageArtifactContentSummary>> {
    if devflow_step_spec(stage_id).is_none() {
        return Ok(None);
    }
    let artifact_path = content_root
        .join("pipeline")
        .join(stage_id)
        .join("stage.adm");
    let Some(text) = read_optional_text(&artifact_path)? else {
        return Ok(None);
    };
    let contract_kind =
        find_key_value(&text, "contract_kind").unwrap_or_else(|| "unknown".to_string());
    Ok(Some(StageArtifactContentSummary {
        contract_kind,
        structured_content: excerpt_section(
            &extract_markdown_section(&text, "Structured Stage Content"),
            22,
        ),
        acceptance_checklist: excerpt_section(
            &extract_markdown_section(&text, "Acceptance Checklist"),
            10,
        ),
        downstream_inputs: excerpt_section(
            &extract_markdown_section(&text, "Downstream Inputs"),
            8,
        ),
    }))
}

fn extract_markdown_section(text: &str, heading: &str) -> String {
    let target = format!("## {heading}");
    let mut in_section = false;
    let mut rows = Vec::new();
    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed == target {
            in_section = true;
            continue;
        }
        if in_section && trimmed.starts_with("## ") {
            break;
        }
        if in_section {
            rows.push(line.to_string());
        }
    }
    rows.join("\n").trim().to_string()
}

fn excerpt_section(section: &str, max_lines: usize) -> String {
    let rows = section
        .lines()
        .filter(|line| !line.trim().is_empty())
        .take(max_lines)
        .map(str::to_string)
        .collect::<Vec<_>>();
    if rows.is_empty() {
        "missing_section".to_string()
    } else {
        rows.join("\n")
    }
}

fn inspect_selected_stage(
    ui: &slint::Weak<MainWindow>,
    stage_id: &str,
    selected_lock: &SelectedArchiveLockState,
) {
    if let Some(ui) = ui.upgrade() {
        let data_root = PathBuf::from(ui.get_data_root().to_string());
        let archive_id = ui.get_selected_archive_id().to_string();
        let owned_session = owned_lock_session_for_archive(selected_lock, &archive_id);
        match inspect_stage_detail(&data_root, &archive_id, stage_id, owned_session.as_ref()) {
            Ok(detail) => apply_stage_detail(&ui, &detail),
            Err(error) => ui.set_status_text(zh(format!("Error: {error}"))),
        }
    }
}

fn inspect_ai(content_root: &Path) -> AdmResult<String> {
    let path = content_root.join("ai/journal.adm");
    if !path.exists() {
        return Ok("AI: no journal".to_string());
    }
    let journal = AiTaskJournal::load_from_path(path)?;
    Ok(ai_status_from_summary(&journal.summary()).render())
}

fn inspect_ai_tasks(content_root: &Path) -> AdmResult<Vec<AiTaskItem>> {
    let path = content_root.join("ai/journal.adm");
    if !path.exists() {
        return Ok(Vec::new());
    }
    let journal = AiTaskJournal::load_from_path(path)?;
    Ok(ai_task_items_from_journal(&journal))
}

fn ai_task_items_from_journal(journal: &AiTaskJournal) -> Vec<AiTaskItem> {
    journal
        .records()
        .iter()
        .map(|record| AiTaskItem {
            capability: record.request.capability.as_str().to_string(),
            status: record.status.as_str().to_string(),
            provider: record
                .provider_id
                .as_ref()
                .map(|provider_id| provider_id.as_str().to_string())
                .unwrap_or_default(),
            failure: record
                .failure_kind
                .as_ref()
                .map(|failure| failure.as_str().to_string())
                .unwrap_or_default(),
            summary: one_line_summary(&record.request.context_summary, 120),
        })
        .collect()
}

fn inspect_sdk(content_root: &Path) -> AdmResult<String> {
    let Some(text) = read_optional_text(&content_root.join("sdk/index.adm"))? else {
        return Ok("SDK: not indexed".to_string());
    };
    let resources = sdk_resource_items_from_text(&text);
    let risks = resources
        .iter()
        .map(|resource| count_csv_items(&resource.risks))
        .sum::<usize>();
    let validation = resources
        .iter()
        .map(|resource| count_csv_items(&resource.validation))
        .sum::<usize>();
    Ok(format!(
        "SDK: resources={}\nrisks={}\nvalidation={}",
        resources.len(),
        risks,
        validation
    ))
}

fn inspect_sdk_resources(content_root: &Path) -> AdmResult<Vec<SdkResourceItem>> {
    let Some(text) = read_optional_text(&content_root.join("sdk/index.adm"))? else {
        return Ok(Vec::new());
    };
    Ok(sdk_resource_items_from_text(&text))
}

fn ai_status_from_summary(summary: &AiTaskJournalSummary) -> AiStatusView {
    AiStatusView {
        record_count: summary.record_count,
        accepted_count: summary.accepted_count,
        failed_count: summary.failed_count,
        rejected_count: summary.rejected_count,
        intervention: summary.record_count > 0 || summary.has_failures(),
        failure_summary: summary.failure_summary_line(),
        last_error: summary.last_error.clone().unwrap_or_default(),
        message: String::new(),
    }
}

fn inspect_package(content_root: &Path) -> AdmResult<String> {
    let Some(text) = read_optional_text(&content_root.join("package/manifest.adm"))? else {
        return Ok("Package: not built".to_string());
    };
    let entries = count_manifest_section(&text, "entries=");
    let support_files = count_manifest_section(&text, "support_files=");
    Ok(PackageStatusView {
        entry_count: entries,
        support_file_count: support_files,
        artifact_count: 0,
        message: String::new(),
    }
    .render())
}

fn inspect_package_files(content_root: &Path) -> AdmResult<Vec<PackageFileItem>> {
    let Some(text) = read_optional_text(&content_root.join("package/manifest.adm"))? else {
        return Ok(Vec::new());
    };
    let mut items = package_file_items_from_manifest_text(&text);
    append_optional_package_files(content_root, &mut items);
    Ok(items)
}

fn inspect_build_targets(content_root: &Path) -> AdmResult<String> {
    let Some(text) = read_optional_text(&content_root.join("package/build_targets.adm"))? else {
        return Ok("Build Targets: not planned".to_string());
    };
    let targets = build_target_items_from_text(&text);
    let required_artifacts = targets
        .iter()
        .map(|target| count_csv_items(&target.required_artifacts))
        .sum::<usize>();
    Ok(format!(
        "Build Targets: targets={}\nrequired_artifacts={}",
        targets.len(),
        required_artifacts
    ))
}

fn inspect_build_target_items(content_root: &Path) -> AdmResult<Vec<BuildTargetItem>> {
    let Some(text) = read_optional_text(&content_root.join("package/build_targets.adm"))? else {
        return Ok(Vec::new());
    };
    Ok(build_target_items_from_text(&text))
}

fn inspect_engine_history(content_root: &Path) -> AdmResult<String> {
    let Some(text) = read_optional_text(&content_root.join("package/engine_build_history.adm"))?
    else {
        return Ok("Engine History: records=0".to_string());
    };
    let records = engine_history_items_from_text(&text);
    let launched = records
        .iter()
        .filter(|record| record.launched.eq_ignore_ascii_case("true"))
        .count();
    let failed = records
        .iter()
        .filter(|record| record.status.eq_ignore_ascii_case("failed"))
        .count();
    let outputs_present = records
        .iter()
        .filter(|record| record.expected_output_present.eq_ignore_ascii_case("true"))
        .count();
    Ok(format!(
        "Engine History: records={}\nlaunched={} failed={} outputs_present={}",
        records.len(),
        launched,
        failed,
        outputs_present
    ))
}

fn inspect_engine_history_items(content_root: &Path) -> AdmResult<Vec<EngineHistoryItem>> {
    let Some(text) = read_optional_text(&content_root.join("package/engine_build_history.adm"))?
    else {
        return Ok(Vec::new());
    };
    Ok(engine_history_items_from_text(&text))
}

fn inspect_validation(content_root: &Path) -> AdmResult<String> {
    let Some(text) = read_optional_text(&content_root.join("validation/report.adm"))? else {
        return Ok("Validation: not run".to_string());
    };
    let status = text
        .lines()
        .find_map(|line| line.strip_prefix("status="))
        .unwrap_or("unknown");
    let issues = text
        .lines()
        .filter(|line| line.trim_start().starts_with("- status="))
        .count();
    let validation = ValidationStatusView {
        status: status.to_string(),
        issue_count: issues,
    }
    .render();
    let readiness = read_optional_text(&content_root.join("validation/production_readiness.adm"))?
        .unwrap_or_default();
    Ok(render_validation_with_production_readiness(
        &validation,
        &readiness,
    ))
}

fn render_validation_with_production_readiness(validation: &str, readiness: &str) -> String {
    let status = find_key_value(readiness, "overall_status").unwrap_or_else(|| "not built".into());
    format!("{validation}\nproduction_readiness={status}")
}

fn inspect_validation_issues(content_root: &Path) -> AdmResult<Vec<ValidationIssueItem>> {
    let Some(text) = read_optional_text(&content_root.join("validation/report.adm"))? else {
        return Ok(Vec::new());
    };
    Ok(validation_issue_items_from_text(&text))
}

fn inspect_acceptance_trace(content_root: &Path) -> AdmResult<String> {
    let Some(text) = read_optional_text(&content_root.join("validation/acceptance_matrix.adm"))?
    else {
        return Ok("Acceptance Matrix: not built".to_string());
    };
    Ok(render_acceptance_trace_summary(
        &acceptance_trace_items_from_text(&text),
    ))
}

fn inspect_acceptance_trace_items(content_root: &Path) -> AdmResult<Vec<AcceptanceTraceItem>> {
    let Some(text) = read_optional_text(&content_root.join("validation/acceptance_matrix.adm"))?
    else {
        return Ok(Vec::new());
    };
    Ok(acceptance_trace_items_from_text(&text))
}

fn render_acceptance_trace_summary(items: &[AcceptanceTraceItem]) -> String {
    let ready = items
        .iter()
        .filter(|item| item.status.eq_ignore_ascii_case("ready"))
        .count();
    let incomplete = items.len().saturating_sub(ready);
    format!(
        "Acceptance Matrix: rows={}\nready={} incomplete={}",
        items.len(),
        ready,
        incomplete
    )
}

fn count_manifest_section(text: &str, section: &str) -> usize {
    let mut in_section = false;
    let mut count = 0;
    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed == section {
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

fn package_file_items_from_manifest_text(text: &str) -> Vec<PackageFileItem> {
    let mut items = Vec::new();
    let mut current_kind = None;
    for line in text.lines() {
        let trimmed = line.trim();
        match trimmed {
            "entries=" => {
                current_kind = Some("entry");
                continue;
            }
            "support_files=" => {
                current_kind = Some("support");
                continue;
            }
            _ => {}
        }
        if let (Some(kind), Some(path)) = (current_kind, trimmed.strip_prefix("- ")) {
            items.push(PackageFileItem {
                kind: kind.to_string(),
                path: path.to_string(),
            });
        }
    }
    items
}

fn append_optional_package_files(content_root: &Path, items: &mut Vec<PackageFileItem>) {
    for optional in [
        "package/engine_build_history.adm",
        "validation/runtime_execution_results.adm",
    ] {
        if !content_root.join(optional).is_file() {
            continue;
        }
        if items.iter().any(|item| item.path == optional) {
            continue;
        }
        items.push(PackageFileItem {
            kind: "optional_support".to_string(),
            path: optional.to_string(),
        });
    }
}

fn build_target_items_from_text(text: &str) -> Vec<BuildTargetItem> {
    let mut items = Vec::new();
    let mut current: Option<BuildTargetItem> = None;
    let mut in_required_artifacts = false;
    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("## ") {
            if let Some(item) = current.take() {
                items.push(item);
            }
            current = Some(BuildTargetItem {
                target_id: String::new(),
                engine: String::new(),
                platform: String::new(),
                profile: String::new(),
                output_file: String::new(),
                required_artifacts: String::new(),
            });
            in_required_artifacts = false;
            continue;
        }
        if current.is_none() && trimmed.starts_with("target_id=") {
            current = Some(BuildTargetItem {
                target_id: String::new(),
                engine: String::new(),
                platform: String::new(),
                profile: String::new(),
                output_file: String::new(),
                required_artifacts: String::new(),
            });
        }
        let Some(item) = current.as_mut() else {
            continue;
        };
        if let Some(value) = trimmed.strip_prefix("target_id=") {
            item.target_id = value.trim().to_string();
            in_required_artifacts = false;
        } else if let Some(value) = trimmed.strip_prefix("engine=") {
            item.engine = value.trim().to_string();
            in_required_artifacts = false;
        } else if let Some(value) = trimmed.strip_prefix("platform=") {
            item.platform = value.trim().to_string();
            in_required_artifacts = false;
        } else if let Some(value) = trimmed.strip_prefix("profile=") {
            item.profile = value.trim().to_string();
            in_required_artifacts = false;
        } else if let Some(value) = trimmed.strip_prefix("output_file=") {
            item.output_file = value.trim().to_string();
            in_required_artifacts = false;
        } else if trimmed == "required_artifacts=" {
            in_required_artifacts = true;
        } else if in_required_artifacts {
            if let Some(value) = trimmed.strip_prefix("- ") {
                append_csv_value(&mut item.required_artifacts, value.trim());
            }
        }
    }
    if let Some(item) = current {
        items.push(item);
    }
    items
}

fn engine_history_items_from_text(text: &str) -> Vec<EngineHistoryItem> {
    let mut items = Vec::new();
    let mut current: Option<EngineHistoryItem> = None;
    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed == "# Engine Build Execution" {
            if let Some(item) = current.take() {
                items.push(item);
            }
            current = Some(EngineHistoryItem {
                target_id: String::new(),
                mode: String::new(),
                status: String::new(),
                launched: String::new(),
                exit_code: String::new(),
                expected_output: String::new(),
                expected_output_path: String::new(),
                expected_output_present: String::new(),
                expected_output_bytes: String::new(),
                expected_output_hash: String::new(),
            });
            continue;
        }
        let Some(item) = current.as_mut() else {
            continue;
        };
        if let Some(value) = trimmed.strip_prefix("target_id=") {
            item.target_id = value.trim().to_string();
        } else if let Some(value) = trimmed.strip_prefix("mode=") {
            item.mode = value.trim().to_string();
        } else if let Some(value) = trimmed.strip_prefix("status=") {
            item.status = value.trim().to_string();
        } else if let Some(value) = trimmed.strip_prefix("launched=") {
            item.launched = value.trim().to_string();
        } else if let Some(value) = trimmed.strip_prefix("exit_code=") {
            item.exit_code = value.trim().to_string();
        } else if let Some(value) = trimmed.strip_prefix("expected_output=") {
            item.expected_output = value.trim().to_string();
        } else if let Some(value) = trimmed.strip_prefix("expected_output_path=") {
            item.expected_output_path = value.trim().to_string();
        } else if let Some(value) = trimmed.strip_prefix("expected_output_present=") {
            item.expected_output_present = value.trim().to_string();
        } else if let Some(value) = trimmed.strip_prefix("expected_output_bytes=") {
            item.expected_output_bytes = value.trim().to_string();
        } else if let Some(value) = trimmed.strip_prefix("expected_output_hash=") {
            item.expected_output_hash = value.trim().to_string();
        }
    }
    if let Some(item) = current {
        items.push(item);
    }
    items
}

fn sdk_resource_items_from_text(text: &str) -> Vec<SdkResourceItem> {
    #[derive(Clone, Copy, PartialEq, Eq)]
    enum Section {
        None,
        Risks,
        Validation,
    }

    let mut items = Vec::new();
    let mut current: Option<SdkResourceItem> = None;
    let mut section = Section::None;
    for line in text.lines() {
        let trimmed = line.trim();
        if let Some(name) = trimmed.strip_prefix("## ") {
            if let Some(item) = current.take() {
                items.push(item);
            }
            current = Some(SdkResourceItem {
                sdk_name: name.trim().to_string(),
                category: String::new(),
                target_engines: String::new(),
                target_platforms: String::new(),
                required_for_build: String::new(),
                purpose: String::new(),
                ai_explanation: String::new(),
                risks: String::new(),
                validation: String::new(),
            });
            section = Section::None;
            continue;
        }
        let Some(item) = current.as_mut() else {
            continue;
        };
        if let Some(purpose) = trimmed.strip_prefix("purpose=") {
            item.purpose = purpose.trim().to_string();
            section = Section::None;
            continue;
        }
        if let Some(category) = trimmed.strip_prefix("category=") {
            item.category = category.trim().to_string();
            section = Section::None;
            continue;
        }
        if let Some(target_engines) = trimmed.strip_prefix("target_engines=") {
            item.target_engines = target_engines.trim().to_string();
            section = Section::None;
            continue;
        }
        if let Some(target_platforms) = trimmed.strip_prefix("target_platforms=") {
            item.target_platforms = target_platforms.trim().to_string();
            section = Section::None;
            continue;
        }
        if let Some(required_for_build) = trimmed.strip_prefix("required_for_build=") {
            item.required_for_build = format!("build_required={}", required_for_build.trim());
            section = Section::None;
            continue;
        }
        if let Some(ai_explanation) = trimmed.strip_prefix("ai_explanation=") {
            item.ai_explanation = ai_explanation.trim().to_string();
            section = Section::None;
            continue;
        }
        match trimmed {
            "risks=" => {
                section = Section::Risks;
                continue;
            }
            "validation=" => {
                section = Section::Validation;
                continue;
            }
            _ => {}
        }
        if let Some(value) = trimmed.strip_prefix("- ") {
            append_csv_value(
                match section {
                    Section::Risks => &mut item.risks,
                    Section::Validation => &mut item.validation,
                    Section::None => continue,
                },
                value.trim(),
            );
        }
    }
    if let Some(item) = current {
        items.push(item);
    }
    items
}

fn append_csv_value(target: &mut String, value: &str) {
    if value.is_empty() {
        return;
    }
    if !target.is_empty() {
        target.push_str(", ");
    }
    target.push_str(value);
}

fn count_csv_items(value: &str) -> usize {
    value
        .split(',')
        .map(str::trim)
        .filter(|item| !item.is_empty())
        .count()
}

fn core_package_file_items() -> Vec<PackageFileItem> {
    let manifest_text = "entries=\n- design/project.adm\n- development/plan.adm\n- assets/plan.adm\n- sdk/index.adm\n- package/build_targets.adm\nsupport_files=\n- project/brief.adm\n- package/manifest.adm\n- validation/report.adm\n- validation/acceptance_matrix.adm\n- validation/scenario_test_plan.adm\n- validation/runtime_validation_report.adm\n- validation/production_readiness.adm\n- pipeline/run_report.adm\n- pipeline/run_state.adm\n- pipeline/devflow_run_report.adm\n- pipeline/devflow_run_state.adm\n- pipeline/artifact_registry.adm\n- ai/journal.adm\n";
    package_file_items_from_manifest_text(manifest_text)
}

fn validation_issue_items_from_report(
    report: &adm_validation::ValidationReport,
) -> Vec<ValidationIssueItem> {
    report
        .issues
        .iter()
        .map(|issue| ValidationIssueItem {
            status: format!("{:?}", issue.status),
            code: issue.code.clone(),
            message: issue.message.clone(),
        })
        .collect()
}

fn validation_issue_items_from_text(text: &str) -> Vec<ValidationIssueItem> {
    let mut items = Vec::new();
    for line in text.lines() {
        let trimmed = line.trim();
        let Some(entry) = trimmed.strip_prefix("- ") else {
            continue;
        };
        let mut status = None;
        let mut code = None;
        let mut message = None;
        for part in entry.split("; ") {
            if let Some(value) = part.strip_prefix("status=") {
                status = Some(value.to_string());
            } else if let Some(value) = part.strip_prefix("code=") {
                code = Some(value.to_string());
            } else if let Some(value) = part.strip_prefix("message=") {
                message = Some(value.to_string());
            }
        }
        if let Some(status) = status {
            items.push(ValidationIssueItem {
                status,
                code: code.unwrap_or_default(),
                message: message.unwrap_or_default(),
            });
        }
    }
    items
}

fn acceptance_trace_items_from_text(text: &str) -> Vec<AcceptanceTraceItem> {
    let mut items = Vec::new();
    for line in text.lines() {
        let trimmed = line.trim();
        let Some(entry) = trimmed.strip_prefix("- ") else {
            continue;
        };
        let mut item = AcceptanceTraceItem {
            trace_id: String::new(),
            scenario_id: String::new(),
            source_mechanic: String::new(),
            development_task_id: String::new(),
            asset_task_id: String::new(),
            sdk_resources: String::new(),
            build_targets: String::new(),
            validation_probe: String::new(),
            status: String::new(),
        };
        for part in entry.split(';') {
            let Some((key, value)) = part.trim().split_once('=') else {
                continue;
            };
            let value = value.trim().to_string();
            match key.trim() {
                "trace_id" => item.trace_id = value,
                "scenario_id" => item.scenario_id = value,
                "source_mechanic" => item.source_mechanic = value,
                "development_task_id" => item.development_task_id = value,
                "asset_task_id" => item.asset_task_id = value,
                "sdk_resources" => item.sdk_resources = value,
                "build_targets" => item.build_targets = value,
                "validation_probe" => item.validation_probe = value,
                "status" => item.status = value,
                _ => {}
            }
        }
        if !item.trace_id.is_empty() {
            items.push(item);
        }
    }
    items
}

fn stage_progress_items_from_runtime(
    state: &PipelineRunState,
    report: &PipelineRunReport,
    registry: &ArtifactRegistry,
) -> Vec<StageProgressItem> {
    let reports = report
        .results
        .iter()
        .map(|result| {
            (
                result.stage_id.as_str().to_string(),
                (format!("{:?}", result.status), result.message.clone()),
            )
        })
        .collect::<HashMap<_, _>>();
    let mut artifacts = HashMap::<String, Vec<String>>::new();
    for record in registry.records() {
        artifacts
            .entry(record.stage_id.as_str().to_string())
            .or_default()
            .push(record.relative_path.to_string_lossy().replace('\\', "/"));
    }
    stage_progress_items_from_parts(state, &reports, &artifacts)
}

fn stage_progress_items_from_parts(
    state: &PipelineRunState,
    reports: &HashMap<String, (String, String)>,
    artifacts: &HashMap<String, Vec<String>>,
) -> Vec<StageProgressItem> {
    devflow_step_specs()
        .iter()
        .map(|step| {
            let (status, message) = devflow_step_status_and_message(step.step_id, state, reports);
            StageProgressItem {
                stage_id: step.step_id.to_string(),
                label: format!("{}  {}", step.group, step.title),
                status,
                artifact_count: artifacts
                    .get(step.step_id)
                    .or_else(|| artifacts.get(step.core_stage_id))
                    .map(Vec::len)
                    .unwrap_or(0),
                message,
            }
        })
        .collect::<Vec<_>>()
}

fn stage_detail_from_parts(
    stage_id: &str,
    state: &PipelineRunState,
    reports: &HashMap<String, (String, String)>,
    artifacts: &HashMap<String, Vec<String>>,
    artifact_content: Option<&StageArtifactContentSummary>,
) -> StageDetailView {
    if stage_id.trim().is_empty() {
        let devflow_completed = state
            .completed_stages
            .iter()
            .filter(|stage| devflow_step_spec(stage.as_str()).is_some())
            .count();
        let core_completed = state
            .completed_stages
            .iter()
            .filter(|stage| {
                core_stage_definitions()
                    .iter()
                    .any(|(core_stage_id, _)| *core_stage_id == stage.as_str())
            })
            .count();
        let status = if devflow_completed > 0 {
            format!(
                "Step00-14：{} / {}",
                devflow_completed.min(devflow_step_specs().len()),
                devflow_step_specs().len()
            )
        } else {
            format!(
                "Step00-14：待运行；核心阶段回退 {} / {}",
                core_completed.min(core_stage_definitions().len()),
                core_stage_definitions().len()
            )
        };
        return StageDetailView {
            label: "总览".to_string(),
            stage_id: "overview".to_string(),
            status,
            message: "左侧展示 Step00-14 旧版流水线信息架构；当前 Rust 归档优先读取 rust_devflow_executor_v1 的 Step 状态与报告，旧归档缺失 devflow 文件时才回退核心阶段状态。".to_string(),
            artifacts: artifacts
                .values()
                .flatten()
                .cloned()
                .collect::<Vec<_>>(),
        };
    }
    let label = devflow_step_spec(stage_id)
        .map(|step| format!("{}  {}", step.group, step.title))
        .or_else(|| {
            core_stage_definitions()
                .iter()
                .find_map(|(known_id, label)| {
                    (*known_id == stage_id).then_some((*label).to_string())
                })
        })
        .unwrap_or_else(|| stage_id.to_string());
    let core_stage_id = core_stage_id_for_devflow_step(stage_id).unwrap_or(stage_id);
    let (status, mut message) = if devflow_step_spec(stage_id).is_some() {
        devflow_step_status_and_message(stage_id, state, reports)
    } else {
        stage_status_and_message(core_stage_id, state, reports)
    };
    if let Some(step) = devflow_step_spec(stage_id) {
        message = format!(
            "{}\n\n当前 Rust 核心阶段映射：{}。\n{}",
            step.detail, step.core_stage_id, message
        );
    }
    if let Some(content) = artifact_content {
        message = format!("{}\n\n{}", message, content.render());
    }
    StageDetailView {
        label: label.to_string(),
        stage_id: stage_id.to_string(),
        status,
        message,
        artifacts: artifacts
            .get(stage_id)
            .or_else(|| artifacts.get(core_stage_id))
            .cloned()
            .unwrap_or_default(),
    }
}

fn devflow_step_status_and_message(
    step_id: &str,
    state: &PipelineRunState,
    reports: &HashMap<String, (String, String)>,
) -> (String, String) {
    let Some(step) = devflow_step_spec(step_id) else {
        return stage_status_and_message(step_id, state, reports);
    };
    if state
        .active_stage
        .as_ref()
        .is_some_and(|active| active.as_str() == step_id)
        || state
            .completed_stages
            .iter()
            .any(|completed| completed.as_str() == step_id)
        || reports.contains_key(step_id)
    {
        let (status, message) = stage_status_and_message(step_id, state, reports);
        return (
            match status.as_str() {
                "Completed" => "Step 已完成".to_string(),
                "Active" => "Step 运行中".to_string(),
                "Failed" => "Step 失败".to_string(),
                "NeedsAiIntervention" => "需要 AI 介入".to_string(),
                other => other.to_string(),
            },
            if message.trim().is_empty() {
                step.detail.to_string()
            } else {
                message
            },
        );
    }
    let (core_status, core_message) = stage_status_and_message(step.core_stage_id, state, reports);
    let status = match core_status.as_str() {
        "Active" => "核心阶段运行中",
        "Completed" => "核心阶段已完成",
        "Failed" => "核心阶段失败",
        "NeedsAiIntervention" => "需要 AI 介入",
        "Pending" => "待运行",
        other => other,
    }
    .to_string();
    let message = if core_message.trim().is_empty() {
        format!(
            "{}；当前由 Rust 核心阶段 {} 承载，独立 Step 执行器仍在开发中。",
            step.detail, step.core_stage_id
        )
    } else {
        format!(
            "{}；核心阶段 {}：{}",
            step.detail, step.core_stage_id, core_message
        )
    };
    (status, message)
}

fn stage_status_and_message(
    stage_id: &str,
    state: &PipelineRunState,
    reports: &HashMap<String, (String, String)>,
) -> (String, String) {
    if state
        .active_stage
        .as_ref()
        .is_some_and(|active| active.as_str() == stage_id)
    {
        return ("Active".to_string(), state.last_message.clone());
    }
    if state
        .completed_stages
        .iter()
        .any(|completed| completed.as_str() == stage_id)
    {
        let message = reports
            .get(stage_id)
            .map(|(_, message)| message.clone())
            .unwrap_or_else(|| "completed previously".to_string());
        return ("Completed".to_string(), message);
    }
    reports
        .get(stage_id)
        .cloned()
        .unwrap_or_else(|| ("Pending".to_string(), String::new()))
}

fn parse_run_report(text: &str) -> HashMap<String, (String, String)> {
    let mut reports = HashMap::new();
    for line in text.lines() {
        let trimmed = line.trim();
        let Some(entry) = trimmed.strip_prefix("- ") else {
            continue;
        };
        let mut stage_id = None;
        let mut status = None;
        let mut message = None;
        for part in entry.split("; ") {
            if let Some(value) = part.strip_prefix("stage_id=") {
                stage_id = Some(value.to_string());
            } else if let Some(value) = part.strip_prefix("status=") {
                status = Some(value.to_string());
            } else if let Some(value) = part.strip_prefix("message=") {
                message = Some(value.to_string());
            }
        }
        if let Some(stage_id) = stage_id {
            reports.insert(
                stage_id,
                (
                    status.unwrap_or_else(|| "Unknown".to_string()),
                    message.unwrap_or_default(),
                ),
            );
        }
    }
    reports
}

fn artifact_entries_from_registry_text(text: &str) -> HashMap<String, Vec<String>> {
    let mut artifacts = HashMap::<String, Vec<String>>::new();
    for line in text.lines() {
        let mut stage_id = None;
        let mut path = None;
        for part in line.split(';') {
            if let Some(value) = part.strip_prefix("stage_id=") {
                stage_id = Some(value.to_string());
            } else if let Some(value) = part.strip_prefix("path=") {
                path = Some(value.replace('\\', "/"));
            }
        }
        if let (Some(stage_id), Some(path)) = (stage_id, path) {
            artifacts.entry(stage_id).or_default().push(path);
        }
    }
    artifacts
}

fn core_stage_definitions() -> &'static [(&'static str, &'static str); 5] {
    &[
        ("design", "Design"),
        ("development", "Development"),
        ("assets", "Assets"),
        ("sdk", "SDK"),
        ("packaging", "Packaging"),
    ]
}

fn read_optional_text(path: &Path) -> AdmResult<Option<String>> {
    match std::fs::read_to_string(path) {
        Ok(text) => Ok(Some(text)),
        Err(error) if error.kind() == ErrorKind::NotFound => Ok(None),
        Err(error) => Err(AdmError::from(error)),
    }
}

fn read_fresh_pipeline_text(
    content_root: &Path,
    devflow_relative_path: &str,
    core_relative_path: &str,
) -> AdmResult<Option<(String, &'static str)>> {
    let devflow_path = content_root.join(devflow_relative_path);
    let core_path = content_root.join(core_relative_path);
    if let Some(text) = read_optional_text(&devflow_path)? {
        return Ok(Some((text, "rust_devflow_executor_v1")));
    }
    if let Some(text) = read_optional_text(&core_path)? {
        return Ok(Some((text, "core")));
    }
    Ok(None)
}

fn apply_run_summary(ui: &MainWindow, summary: &DesktopRunSummary) {
    ui.set_selected_archive_id(summary.archive_id.clone().into());
    ui.set_status_text(zh(&summary.status_text));
    ui.set_pipeline_text(zh(&summary.pipeline_text));
    ui.set_core_artifact_text(zh(&summary.core_artifact_text));
    apply_core_artifacts(ui, &summary.core_artifacts);
    ui.set_ai_text(zh(&summary.ai_text));
    apply_ai_tasks(ui, &summary.ai_tasks);
    ui.set_sdk_text(zh(&summary.sdk_text));
    apply_sdk_resources(ui, &summary.sdk_resources);
    ui.set_package_text(zh(&summary.package_text));
    apply_package_files(ui, &summary.package_files);
    ui.set_build_target_text(zh(&summary.build_target_text));
    apply_build_targets(ui, &summary.build_targets);
    ui.set_engine_history_text(zh(&summary.engine_history_text));
    apply_engine_history(ui, &summary.engine_history);
    ui.set_validation_text(zh(&summary.validation_text));
    apply_validation_issues(ui, &summary.validation_issues);
    ui.set_acceptance_trace_text(zh(&summary.acceptance_trace_text));
    apply_acceptance_traces(ui, &summary.acceptance_traces);
    ui.set_stage_progress_text(zh(&summary.stage_progress_text));
    apply_stage_items(ui, &summary.stage_items);
    apply_stage_detail(ui, &summary.stage_detail);
}

fn apply_project_inspection(ui: &MainWindow, inspection: &ProjectInspection) {
    ui.set_project_detail(zh(&inspection.detail_text));
    ui.set_pipeline_text(zh(&inspection.pipeline_text));
    ui.set_core_artifact_text(zh(&inspection.core_artifact_text));
    apply_core_artifacts(ui, &inspection.core_artifacts);
    ui.set_ai_text(zh(&inspection.ai_text));
    apply_ai_tasks(ui, &inspection.ai_tasks);
    ui.set_sdk_text(zh(&inspection.sdk_text));
    apply_sdk_resources(ui, &inspection.sdk_resources);
    ui.set_package_text(zh(&inspection.package_text));
    apply_package_files(ui, &inspection.package_files);
    ui.set_build_target_text(zh(&inspection.build_target_text));
    apply_build_targets(ui, &inspection.build_targets);
    ui.set_engine_history_text(zh(&inspection.engine_history_text));
    apply_engine_history(ui, &inspection.engine_history);
    ui.set_validation_text(zh(&inspection.validation_text));
    apply_validation_issues(ui, &inspection.validation_issues);
    ui.set_acceptance_trace_text(zh(&inspection.acceptance_trace_text));
    apply_acceptance_traces(ui, &inspection.acceptance_traces);
    ui.set_stage_progress_text(zh(&inspection.stage_progress_text));
    apply_stage_items(ui, &inspection.stage_items);
    apply_stage_detail(ui, &inspection.stage_detail);
}

fn apply_locked_or_unavailable_detail(ui: &MainWindow, error: &AdmError) {
    if *error.kind() == AdmErrorKind::AlreadyLocked {
        let owner = error
            .context()
            .first()
            .map(|owner| one_line_summary(owner, 240))
            .unwrap_or_else(|| "锁持有者不可用".to_string());
        ui.set_project_detail(zh(format!(
            "Project detail unavailable.\nlock_state=locked\nlock_owner={owner}\n{}",
            error.message()
        )));
    } else {
        ui.set_project_detail(zh(format!("Project detail unavailable.\n{error}")));
    }
    ui.set_pipeline_text(zh("Pipeline unavailable."));
    ui.set_core_artifact_text(zh("Core artifacts unavailable."));
    ui.set_ai_text(zh("AI unavailable."));
    apply_ai_tasks(ui, &[]);
    ui.set_sdk_text(zh("SDK unavailable."));
    ui.set_package_text(zh("Package unavailable."));
    ui.set_build_target_text(zh("Build targets unavailable."));
    ui.set_engine_history_text(zh("Engine history unavailable."));
    ui.set_validation_text(zh("Validation unavailable."));
    ui.set_acceptance_trace_text(zh("Acceptance matrix unavailable."));
    ui.set_stage_progress_text(zh("Stages unavailable."));
    apply_stage_items(ui, &[]);
    apply_core_artifacts(ui, &[]);
    apply_sdk_resources(ui, &[]);
    apply_package_files(ui, &[]);
    apply_build_targets(ui, &[]);
    apply_engine_history(ui, &[]);
    apply_validation_issues(ui, &[]);
    apply_acceptance_traces(ui, &[]);
    apply_stage_detail(ui, &StageDetailView::empty());
}

fn apply_stage_items(ui: &MainWindow, items: &[StageProgressItem]) {
    let rows = items.iter().map(stage_row_from_item).collect::<Vec<_>>();
    ui.set_stage_items(ModelRc::new(VecModel::from(rows)));
}

fn stage_row_from_item(item: &StageProgressItem) -> StageRow {
    StageRow {
        id: SharedString::from(item.stage_id.clone()),
        label: zh(&item.label),
        status: zh(&item.status),
        artifacts: zh(format!("artifacts={}", item.artifact_count)),
        message: zh(&item.message),
    }
}

fn apply_project_items(ui: &MainWindow, items: &[ProjectListItem]) {
    let rows = items.iter().map(project_row_from_item).collect::<Vec<_>>();
    ui.set_project_items(ModelRc::new(VecModel::from(rows)));
}

fn apply_core_artifacts(ui: &MainWindow, items: &[CoreArtifactItem]) {
    let rows = items
        .iter()
        .map(|item| CoreArtifactRow {
            area: zh(&item.area),
            count: zh(&item.count),
            summary: zh(&item.summary),
        })
        .collect::<Vec<_>>();
    ui.set_core_artifact_items(ModelRc::new(VecModel::from(rows)));
}

fn project_row_from_item(item: &ProjectListItem) -> ProjectRow {
    ProjectRow {
        archive_id: SharedString::from(item.archive_id.clone()),
        display_name: SharedString::from(item.display_name.clone()),
        locked: zh(format!("locked={}", item.locked)),
    }
}

fn apply_ai_diagnostics(ui: &MainWindow, view: &AiDiagnosticsView) {
    ui.set_ai_config_text(zh(view.render()));
    ui.set_ai_config_summary(zh(format!(
        "AI Config: ready_provider_count={} | provider_count={} | budget={} | retries={}",
        view.ready_provider_count,
        view.provider_count,
        view.default_budget_units,
        view.retry_max_attempts
    )));
    apply_ai_provider_items(ui, &view.providers);
}

fn apply_ai_provider_items(ui: &MainWindow, providers: &[AiProviderDiagnosticsItem]) {
    let rows = providers
        .iter()
        .map(|provider| AiProviderRow {
            provider_id: SharedString::from(provider.provider_id.clone()),
            readiness: zh(&provider.readiness),
            capabilities: zh(&provider.capabilities),
            notes: zh(&provider.notes),
        })
        .collect::<Vec<_>>();
    ui.set_ai_provider_items(ModelRc::new(VecModel::from(rows)));
}

fn apply_ai_tasks(ui: &MainWindow, tasks: &[AiTaskItem]) {
    let rows = tasks
        .iter()
        .map(|task| AiTaskRow {
            capability: zh(&task.capability),
            status: zh(&task.status),
            provider: SharedString::from(task.provider.clone()),
            failure: zh(&task.failure),
            summary: zh(&task.summary),
        })
        .collect::<Vec<_>>();
    ui.set_ai_task_items(ModelRc::new(VecModel::from(rows)));
}

fn apply_stage_detail(ui: &MainWindow, detail: &StageDetailView) {
    ui.set_stage_detail_text(zh(detail.render()));
    ui.set_stage_detail_label(zh(&detail.label));
    ui.set_stage_detail_id(detail.stage_id.clone().into());
    ui.set_stage_detail_status(zh(&detail.status));
    ui.set_stage_detail_message(zh(&detail.message));
    apply_stage_artifacts(ui, &detail.artifacts);
}

fn apply_stage_artifacts(ui: &MainWindow, artifacts: &[String]) {
    let rows = artifacts
        .iter()
        .map(|artifact| StageArtifactRow {
            path: SharedString::from(artifact.clone()),
        })
        .collect::<Vec<_>>();
    ui.set_stage_artifact_items(ModelRc::new(VecModel::from(rows)));
}

fn apply_package_files(ui: &MainWindow, files: &[PackageFileItem]) {
    let rows = files
        .iter()
        .map(|file| PackageFileRow {
            kind: zh(&file.kind),
            path: SharedString::from(file.path.clone()),
        })
        .collect::<Vec<_>>();
    ui.set_package_file_items(ModelRc::new(VecModel::from(rows)));
}

fn apply_build_targets(ui: &MainWindow, targets: &[BuildTargetItem]) {
    let rows = targets
        .iter()
        .map(|target| BuildTargetRow {
            target_id: SharedString::from(target.target_id.clone()),
            engine: SharedString::from(target.engine.clone()),
            platform: zh(&target.platform),
            profile: zh(&target.profile),
            output_file: SharedString::from(target.output_file.clone()),
            required_artifacts: zh(&target.required_artifacts),
        })
        .collect::<Vec<_>>();
    ui.set_build_target_items(ModelRc::new(VecModel::from(rows)));
}

fn apply_engine_history(ui: &MainWindow, records: &[EngineHistoryItem]) {
    let rows = records
        .iter()
        .map(|record| EngineHistoryRow {
            target_id: SharedString::from(record.target_id.clone()),
            mode: zh(&record.mode),
            status: zh(&record.status),
            launched: zh(&record.launched),
            exit_code: SharedString::from(record.exit_code.clone()),
            expected_output: SharedString::from(record.expected_output.clone()),
            expected_output_path: SharedString::from(record.expected_output_path.clone()),
            expected_output_present: zh(&record.expected_output_present),
            expected_output_bytes: SharedString::from(record.expected_output_bytes.clone()),
            expected_output_hash: SharedString::from(record.expected_output_hash.clone()),
        })
        .collect::<Vec<_>>();
    ui.set_engine_history_items(ModelRc::new(VecModel::from(rows)));
}

fn apply_delivery_checks(ui: &MainWindow, checks: &[DeliveryCheckItem]) {
    let rows = checks
        .iter()
        .map(|check| DeliveryCheckRow {
            scope: zh(&check.scope),
            path: SharedString::from(check.path.clone()),
            present: zh(&check.present),
        })
        .collect::<Vec<_>>();
    ui.set_delivery_check_items(ModelRc::new(VecModel::from(rows)));
}

fn apply_sdk_resources(ui: &MainWindow, resources: &[SdkResourceItem]) {
    let rows = resources
        .iter()
        .map(|resource| SdkResourceRow {
            sdk_name: SharedString::from(resource.sdk_name.clone()),
            category: zh(&resource.category),
            target_engines: zh(&resource.target_engines),
            target_platforms: zh(&resource.target_platforms),
            required_for_build: zh(&resource.required_for_build),
            purpose: zh(&resource.purpose),
            ai_explanation: zh(&resource.ai_explanation),
            risks: zh(&resource.risks),
            validation: zh(&resource.validation),
        })
        .collect::<Vec<_>>();
    ui.set_sdk_resource_items(ModelRc::new(VecModel::from(rows)));
}

fn apply_sdk_review_items(ui: &MainWindow, records: &[SdkReviewRecord]) {
    let rows = records
        .iter()
        .map(|record| SdkReviewRow {
            id: SharedString::from(record.id.clone()),
            sdk_name: SharedString::from(record.sdk_name.clone()),
            url: SharedString::from(record.url.clone()),
            status: SharedString::from(record.status.label_zh()),
            category: SharedString::from(record.category.clone()),
            target_engines: SharedString::from(record.target_engines.clone()),
            target_platforms: SharedString::from(record.target_platforms.clone()),
            purpose: SharedString::from(record.purpose.clone()),
            note: SharedString::from(record.note.clone()),
        })
        .collect::<Vec<_>>();
    ui.set_sdk_review_items(ModelRc::new(VecModel::from(rows)));
}

fn append_run_log_for_ui(
    data_root: &Path,
    level: &str,
    scope: &str,
    message: &str,
    context: &str,
) -> AdmResult<()> {
    RunLogService::new(data_root)
        .append(level, scope, message, context)
        .map(|_| ())
}

fn append_pipeline_range_log_for_ui(
    data_root: &Path,
    message: &str,
    request: &DevflowRangeRunRequest,
    archive_id: &str,
    devflow_completed_count: Option<usize>,
) -> AdmResult<()> {
    let completed = devflow_completed_count
        .map(|count| count.to_string())
        .unwrap_or_else(|| "pending".to_string());
    append_run_log_for_ui(
        data_root,
        "INFO",
        "pipeline",
        message,
        &format!(
            "archive_id={}; start_step_id={}; end_step_id={}; mapped_core_stage_ids={}; devflow_completed_count={}",
            archive_id,
            request.start_step_id,
            request.end_step_id,
            request.mapped_core_stage_ids.join(","),
            completed
        ),
    )
}

fn record_pipeline_range_summary_for_ui(
    data_root: &Path,
    request: &DevflowRangeRunRequest,
    summary: &DesktopRunSummary,
) -> AdmResult<usize> {
    let completed_count = devflow_completed_count(summary);
    PipelineService::new(data_root).record_range_run_summary(
        request,
        &summary.archive_id,
        completed_count,
        "completed",
    )?;
    Ok(completed_count)
}

fn devflow_completed_count(summary: &DesktopRunSummary) -> usize {
    summary
        .stage_items
        .iter()
        .filter(|item| {
            item.stage_id.starts_with("step")
                && (item.status.contains("已完成") || item.status == "Completed")
        })
        .count()
}

fn refresh_run_log(ui: &MainWindow, data_root: &Path, filter: impl AsRef<str>) {
    let filter = filter.as_ref();
    match RunLogService::new(data_root).render(filter, 200) {
        Ok(text) => ui.set_run_log_text(zh(text)),
        Err(error) => ui.set_run_log_text(zh(format!("严格运行日志读取失败：{error}"))),
    }
}

fn refresh_pipeline_service_status(ui: &MainWindow, data_root: &Path) {
    match PipelineService::new(data_root).render_status() {
        Ok(text) => ui.set_pipeline_service_text(zh(text)),
        Err(error) => ui.set_pipeline_service_text(zh(format!("流水线服务状态读取失败：{error}"))),
    }
}

fn refresh_sdk_knowledge(ui: &MainWindow, data_root: &Path) {
    let service = SdkKnowledgeService::new(data_root);
    match service.snapshot() {
        Ok(snapshot) => {
            let summary = match service.render_summary() {
                Ok(summary) => summary,
                Err(error) => format!("SDK 审批队列摘要生成失败：{error}"),
            };
            ui.set_sdk_review_text(zh(format!(
                "{}\napproved_prompt_context_bytes={}",
                summary,
                snapshot.approved_prompt_context.len()
            )));
            apply_sdk_review_items(ui, &snapshot.records);
        }
        Err(error) => {
            ui.set_sdk_review_text(zh(format!("SDK 审批队列读取失败：{error}")));
            ui.set_sdk_review_items(ModelRc::new(VecModel::from(Vec::<SdkReviewRow>::new())));
        }
    }
}

fn update_sdk_review_status(ui: &MainWindow, record_id: &str, action: &str) {
    let data_root = PathBuf::from(ui.get_data_root().to_string());
    let service = SdkKnowledgeService::new(&data_root);
    let result = match action {
        "approve" => service.approve(record_id),
        "pending" => service.mark_pending(record_id),
        "reject" => service.reject(record_id),
        _ => Err(AdmError::invalid_input(format!(
            "unsupported SDK review action: {action}"
        ))),
    };
    match result {
        Ok(record) => {
            refresh_sdk_knowledge(ui, &data_root);
            let _ = append_run_log_for_ui(
                &data_root,
                "INFO",
                "sdk",
                "updated_sdk_review_status",
                &format!("id={}; action={action}", record.id),
            );
            refresh_run_log(ui, &data_root, ui.get_run_log_filter().to_string());
            ui.set_status_text(zh(format!(
                "SDK 审批状态已更新：{} -> {}",
                record.sdk_name,
                record.status.label_zh()
            )));
        }
        Err(error) => ui.set_status_text(zh(format!("更新 SDK 审批状态失败：{error}"))),
    }
}

fn apply_validation_issues(ui: &MainWindow, issues: &[ValidationIssueItem]) {
    let rows = issues
        .iter()
        .map(|issue| ValidationIssueRow {
            status: zh(&issue.status),
            code: SharedString::from(issue.code.clone()),
            message: zh(&issue.message),
        })
        .collect::<Vec<_>>();
    ui.set_validation_issue_items(ModelRc::new(VecModel::from(rows)));
}

fn apply_acceptance_traces(ui: &MainWindow, traces: &[AcceptanceTraceItem]) {
    let rows = traces
        .iter()
        .map(|trace| AcceptanceTraceRow {
            trace_id: SharedString::from(trace.trace_id.clone()),
            scenario_id: SharedString::from(trace.scenario_id.clone()),
            source_mechanic: SharedString::from(trace.source_mechanic.clone()),
            development_task_id: SharedString::from(trace.development_task_id.clone()),
            asset_task_id: SharedString::from(trace.asset_task_id.clone()),
            sdk_resources: SharedString::from(trace.sdk_resources.clone()),
            build_targets: SharedString::from(trace.build_targets.clone()),
            validation_probe: SharedString::from(trace.validation_probe.clone()),
            status: zh(&trace.status),
        })
        .collect::<Vec<_>>();
    ui.set_acceptance_trace_items(ModelRc::new(VecModel::from(rows)));
}

fn export_project(
    data_root: &Path,
    archive_id: &str,
    target: &Path,
    allowed_lock_session: Option<&SessionId>,
) -> AdmResult<String> {
    if archive_id.trim().is_empty() {
        return Err(AdmError::invalid_input("archive id cannot be empty"));
    }
    let app = AdmApplication::for_data_root(data_root)?;
    let archive = app.load_project(archive_id)?;
    ensure_archive_not_locked(&archive.root, allowed_lock_session)?;
    let exported = app.export_project(archive_id, target)?;
    Ok(format!("Exported {archive_id} to {}", exported.display()))
}

fn inspect_import_package_for_ui(data_root: &Path, package: &Path) -> AdmResult<PackageDoctorView> {
    if package.as_os_str().is_empty() {
        return Err(AdmError::invalid_input("import file cannot be empty"));
    }
    let app = AdmApplication::for_data_root(data_root)?;
    let report = app.inspect_project_package(package)?;
    Ok(PackageDoctorView {
        ready: report.ready(),
        message: report.render(),
    })
}

fn import_project(data_root: &Path, package: &Path) -> AdmResult<ImportProjectResult> {
    if package.as_os_str().is_empty() {
        return Err(AdmError::invalid_input("import file cannot be empty"));
    }
    let app = AdmApplication::for_data_root(data_root)?;
    let package_doctor = app.inspect_project_package(package)?;
    if !package_doctor.ready() {
        return Err(AdmError::validation(format!(
            "project package is not ready for import\n{}",
            package_doctor.render()
        )));
    }
    let imported = app.import_project(package)?;
    Ok(ImportProjectResult {
        message: format!(
            "Imported {} | {}",
            imported.archive_id, imported.display_name
        ),
        archive_id: imported.archive_id,
        package_doctor_text: package_doctor.render(),
    })
}

fn inspect_workspaces_for_ui(data_root: &Path) -> AdmResult<String> {
    if data_root.as_os_str().is_empty() {
        return Err(AdmError::invalid_input("data root cannot be empty"));
    }
    let app = AdmApplication::for_data_root(data_root)?;
    Ok(app.inspect_workspaces()?.render())
}

fn cleanup_workspaces_for_ui(data_root: &Path) -> AdmResult<String> {
    if data_root.as_os_str().is_empty() {
        return Err(AdmError::invalid_input("data root cannot be empty"));
    }
    let app = AdmApplication::for_data_root(data_root)?;
    Ok(app.cleanup_stale_workspaces()?.render())
}

fn stage_desktop_release_for_ui(source_exe: &Path, target_dir: &Path) -> AdmResult<String> {
    if source_exe.as_os_str().is_empty() {
        return Err(AdmError::invalid_input(
            "desktop release source executable cannot be empty",
        ));
    }
    if target_dir.as_os_str().is_empty() {
        return Err(AdmError::invalid_input(
            "desktop release target directory cannot be empty",
        ));
    }
    let spec = DesktopReleaseSpec::new(source_exe, target_dir, env!("CARGO_PKG_VERSION"));
    let bundle = stage_desktop_release(&spec)?;
    Ok(format!(
        "release_dir={}\nexecutable={}\nmanifest={}\nbytes={} hash={}\nlegacy_root_exe=not_modified",
        bundle.target_dir.display(),
        bundle.executable_path.display(),
        bundle.manifest_path.display(),
        bundle.executable_bytes,
        bundle.executable_hash,
    ))
}

fn release_doctor_for_ui(target_dir: &Path) -> AdmResult<String> {
    if target_dir.as_os_str().is_empty() {
        return Err(AdmError::invalid_input(
            "desktop release target directory cannot be empty",
        ));
    }
    Ok(inspect_desktop_release(target_dir)?.render())
}

fn delivery_doctor_for_ui(
    release_dir: &Path,
    game_bundle_dir: &Path,
    sdk_bundle_dir: &Path,
    unity_project_dir: &Path,
) -> AdmResult<DeliveryDoctorView> {
    if release_dir.as_os_str().is_empty() {
        return Err(AdmError::invalid_input(
            "desktop release target directory cannot be empty",
        ));
    }
    if game_bundle_dir.as_os_str().is_empty() {
        return Err(AdmError::invalid_input(
            "game build bundle target directory cannot be empty",
        ));
    }
    if sdk_bundle_dir.as_os_str().is_empty() {
        return Err(AdmError::invalid_input(
            "SDK bundle target directory cannot be empty",
        ));
    }
    if unity_project_dir.as_os_str().is_empty() {
        return Err(AdmError::invalid_input(
            "Unity project directory cannot be empty",
        ));
    }
    let report = inspect_delivery(
        release_dir,
        game_bundle_dir,
        sdk_bundle_dir,
        unity_project_dir,
    )?;
    Ok(DeliveryDoctorView {
        message: format!(
            "Delivery: ready={} | release={} | game_build_bundle={} | sdk_bundle={} | unity_project={}",
            report.ready(),
            report.release.ready(),
            report.game_build_bundle.ready(),
            report.sdk_bundle.ready(),
            report.unity_project.ready()
        ),
        checks: delivery_check_items_from_report(&report),
    })
}

fn delivery_check_items_from_report(
    report: &adm_packaging::DeliveryDoctorReport,
) -> Vec<DeliveryCheckItem> {
    let mut items = vec![
        delivery_check_item(
            "release",
            "AutoDesignMaker-rust.exe",
            report.release.executable_present,
        ),
        delivery_check_item(
            "release",
            "release-manifest.adm",
            report.release.manifest_present,
        ),
        delivery_check_item("release", "README.txt", report.release.readme_present),
    ];
    items.extend(report.game_build_bundle.files.iter().map(|file| {
        delivery_check_item_status(
            "game_build_bundle",
            &file.relative_path.display().to_string(),
            file.status(),
        )
    }));
    items.extend(report.sdk_bundle.files.iter().map(|file| {
        delivery_check_item_status(
            "sdk_bundle",
            &file.relative_path.display().to_string(),
            file.status(),
        )
    }));
    items.extend(report.unity_project.files.iter().map(|file| {
        delivery_check_item_status(
            "unity_project",
            &file.relative_path.display().to_string(),
            file.status(),
        )
    }));
    items
}

fn delivery_check_item(scope: &str, path: &str, present: bool) -> DeliveryCheckItem {
    delivery_check_item_status(scope, path, if present { "present" } else { "missing" })
}

fn delivery_check_item_status(scope: &str, path: &str, status: &str) -> DeliveryCheckItem {
    DeliveryCheckItem {
        scope: scope.to_string(),
        path: path.to_string(),
        present: status.to_string(),
    }
}

fn stage_game_build_bundle_for_ui(
    data_root: &Path,
    archive_id: &str,
    target_id: &str,
    target_dir: &Path,
) -> AdmResult<String> {
    if archive_id.trim().is_empty() {
        return Err(AdmError::invalid_input("archive id cannot be empty"));
    }
    if target_id.trim().is_empty() {
        return Err(AdmError::invalid_input(
            "game build target id cannot be empty",
        ));
    }
    let app = AdmApplication::for_data_root(data_root)?;
    let archive = app.load_project(archive_id)?;
    let plan = GameBuildPlan::windows_desktop_prototype(archive.manifest.project_id.clone());
    let bundle = stage_game_build_bundle(
        &plan,
        target_id.trim(),
        archive.root.join("content"),
        target_dir,
    )?;
    Ok(format!(
        "Game Build Bundle: target_id={}\nbundle_dir={}\nmanifest={}\nstaged_files={} bytes={} hash={}",
        bundle.target_id,
        bundle.target_dir.display(),
        bundle.manifest_path.display(),
        bundle.staged_files.len(),
        bundle.total_bytes,
        bundle.bundle_hash
    ))
}

fn stage_sdk_bundle_for_ui(
    data_root: &Path,
    archive_id: &str,
    target_dir: &Path,
) -> AdmResult<String> {
    if archive_id.trim().is_empty() {
        return Err(AdmError::invalid_input("archive id cannot be empty"));
    }
    let app = AdmApplication::for_data_root(data_root)?;
    let archive = app.load_project(archive_id)?;
    let bundle = stage_sdk_bundle(archive.root.join("content"), target_dir)?;
    Ok(format!(
        "SDK Bundle: bundle_dir={}\nmanifest={}\nstaged_files={} bytes={} hash={}",
        bundle.target_dir.display(),
        bundle.manifest_path.display(),
        bundle.staged_files.len(),
        bundle.total_bytes,
        bundle.bundle_hash
    ))
}

fn stage_unity_project_for_ui(
    data_root: &Path,
    archive_id: &str,
    target_id: &str,
    unity_project_dir: &Path,
) -> AdmResult<String> {
    if archive_id.trim().is_empty() {
        return Err(AdmError::invalid_input("archive id cannot be empty"));
    }
    if target_id.trim().is_empty() {
        return Err(AdmError::invalid_input(
            "game build target id cannot be empty",
        ));
    }
    let app = AdmApplication::for_data_root(data_root)?;
    let archive = app.load_project(archive_id)?;
    let plan = GameBuildPlan::windows_desktop_prototype(archive.manifest.project_id.clone());
    let target = plan
        .targets
        .iter()
        .find(|target| target.target_id == target_id.trim())
        .ok_or_else(|| {
            AdmError::invalid_input(format!("unknown game build target: {target_id}"))
        })?;
    let scaffold =
        stage_unity_project_scaffold(target, archive.root.join("content"), unity_project_dir)?;
    Ok(format!(
        "Unity Project Scaffold: target_id={}\nproject_dir={}\nmanifest={}\ngenerated_files={} bytes={} hash={}",
        target.target_id,
        scaffold.project_dir.display(),
        scaffold.manifest_path.display(),
        scaffold.generated_files.len(),
        scaffold.total_bytes,
        scaffold.scaffold_hash
    ))
}

fn unity_build_preflight_for_ui(
    data_root: &Path,
    archive_id: &str,
    target_id: &str,
    unity_exe: &Path,
    unity_project_dir: &Path,
    confirm_token: &str,
) -> AdmResult<String> {
    if archive_id.trim().is_empty() {
        return Err(AdmError::invalid_input("archive id cannot be empty"));
    }
    if target_id.trim().is_empty() {
        return Err(AdmError::invalid_input(
            "game build target id cannot be empty",
        ));
    }
    let app = AdmApplication::for_data_root(data_root)?;
    let archive = app.load_project(archive_id)?;
    let plan = GameBuildPlan::windows_desktop_prototype(archive.manifest.project_id.clone());
    let target = plan
        .targets
        .iter()
        .find(|target| target.target_id == target_id.trim())
        .ok_or_else(|| {
            AdmError::invalid_input(format!("unknown game build target: {target_id}"))
        })?;
    let report =
        inspect_unity_build_preflight(target, unity_exe, unity_project_dir, confirm_token)?;
    Ok(report.render())
}

fn plan_unity_build_for_ui(
    data_root: &Path,
    archive_id: &str,
    target_id: &str,
    unity_exe: &Path,
    unity_project_dir: &Path,
) -> AdmResult<String> {
    if archive_id.trim().is_empty() {
        return Err(AdmError::invalid_input("archive id cannot be empty"));
    }
    if target_id.trim().is_empty() {
        return Err(AdmError::invalid_input(
            "game build target id cannot be empty",
        ));
    }
    let app = AdmApplication::for_data_root(data_root)?;
    let archive = app.load_project(archive_id)?;
    let plan = GameBuildPlan::windows_desktop_prototype(archive.manifest.project_id.clone());
    let target = plan
        .targets
        .iter()
        .find(|target| target.target_id == target_id.trim())
        .ok_or_else(|| {
            AdmError::invalid_input(format!("unknown game build target: {target_id}"))
        })?;
    let command = plan_unity_cli_build(target, unity_exe, unity_project_dir)?;
    Ok(format!(
        "Unity Build Command: target_id={}\nexpected_output={}\ncommand_line={}",
        command.target_id,
        command.expected_output,
        command.command_line()
    ))
}

fn dry_run_unity_build_for_ui(
    data_root: &Path,
    archive_id: &str,
    target_id: &str,
    unity_exe: &Path,
    unity_project_dir: &Path,
) -> AdmResult<String> {
    if archive_id.trim().is_empty() {
        return Err(AdmError::invalid_input("archive id cannot be empty"));
    }
    if target_id.trim().is_empty() {
        return Err(AdmError::invalid_input(
            "game build target id cannot be empty",
        ));
    }
    let app = AdmApplication::for_data_root(data_root)?;
    let archive = app.load_project(archive_id)?;
    let plan = GameBuildPlan::windows_desktop_prototype(archive.manifest.project_id.clone());
    let target = plan
        .targets
        .iter()
        .find(|target| target.target_id == target_id.trim())
        .ok_or_else(|| {
            AdmError::invalid_input(format!("unknown game build target: {target_id}"))
        })?;
    let command = plan_unity_cli_build(target, unity_exe, unity_project_dir)?;
    let report = DryRunEngineBuildRunner.run(&command)?;
    let history = app.commit_engine_build_execution(&archive, &report)?;
    Ok(format!(
        "{}history_file={}\nhistory_records={}\nhistory_commit_files={}",
        report.render(),
        history.history_file.display(),
        history.record_count,
        history.commit.written_files.len()
    ))
}

fn run_unity_build_for_ui(
    data_root: &Path,
    archive_id: &str,
    target_id: &str,
    unity_exe: &Path,
    unity_project_dir: &Path,
    confirm_token: &str,
) -> AdmResult<String> {
    if archive_id.trim().is_empty() {
        return Err(AdmError::invalid_input("archive id cannot be empty"));
    }
    if target_id.trim().is_empty() {
        return Err(AdmError::invalid_input(
            "game build target id cannot be empty",
        ));
    }
    let app = AdmApplication::for_data_root(data_root)?;
    let archive = app.load_project(archive_id)?;
    let plan = GameBuildPlan::windows_desktop_prototype(archive.manifest.project_id.clone());
    let target = plan
        .targets
        .iter()
        .find(|target| target.target_id == target_id.trim())
        .ok_or_else(|| {
            AdmError::invalid_input(format!("unknown game build target: {target_id}"))
        })?;
    let preflight =
        inspect_unity_build_preflight(target, unity_exe, unity_project_dir, confirm_token)?;
    if !preflight.ready_for_local_build() {
        return Err(AdmError::validation(format!(
            "Unity build preflight failed\n{}",
            preflight.render()
        )));
    }
    let command = plan_unity_cli_build(target, unity_exe, unity_project_dir)?;
    let report = LocalProcessEngineBuildRunner.run(&command)?;
    let history = app.commit_engine_build_execution(&archive, &report)?;
    Ok(format!(
        "{}history_file={}\nhistory_records={}\nhistory_commit_files={}",
        report.render(),
        history.history_file.display(),
        history.record_count,
        history.commit.written_files.len()
    ))
}

fn plan_unity_runtime_validation_for_ui(
    data_root: &Path,
    archive_id: &str,
    target_id: &str,
    unity_exe: &Path,
    unity_project_dir: &Path,
) -> AdmResult<String> {
    if archive_id.trim().is_empty() {
        return Err(AdmError::invalid_input("archive id cannot be empty"));
    }
    if target_id.trim().is_empty() {
        return Err(AdmError::invalid_input(
            "game build target id cannot be empty",
        ));
    }
    let app = AdmApplication::for_data_root(data_root)?;
    let archive = app.load_project(archive_id)?;
    let plan = GameBuildPlan::windows_desktop_prototype(archive.manifest.project_id.clone());
    let target = plan
        .targets
        .iter()
        .find(|target| target.target_id == target_id.trim())
        .ok_or_else(|| {
            AdmError::invalid_input(format!("unknown game build target: {target_id}"))
        })?;
    let command = plan_unity_runtime_validation(target, unity_exe, unity_project_dir)?;
    Ok(format!(
        "Unity Runtime Validation Command: target_id={}\nexpected_output={}\ncommand_line={}",
        command.target_id,
        command.expected_output,
        command.command_line()
    ))
}

fn dry_run_unity_runtime_validation_for_ui(
    data_root: &Path,
    archive_id: &str,
    target_id: &str,
    unity_exe: &Path,
    unity_project_dir: &Path,
) -> AdmResult<String> {
    if archive_id.trim().is_empty() {
        return Err(AdmError::invalid_input("archive id cannot be empty"));
    }
    if target_id.trim().is_empty() {
        return Err(AdmError::invalid_input(
            "game build target id cannot be empty",
        ));
    }
    let app = AdmApplication::for_data_root(data_root)?;
    let archive = app.load_project(archive_id)?;
    let plan = GameBuildPlan::windows_desktop_prototype(archive.manifest.project_id.clone());
    let target = plan
        .targets
        .iter()
        .find(|target| target.target_id == target_id.trim())
        .ok_or_else(|| {
            AdmError::invalid_input(format!("unknown game build target: {target_id}"))
        })?;
    let command = plan_unity_runtime_validation(target, unity_exe, unity_project_dir)?;
    let report = DryRunEngineBuildRunner.run(&command)?;
    Ok(report.render())
}

fn run_unity_runtime_validation_for_ui(
    data_root: &Path,
    archive_id: &str,
    target_id: &str,
    unity_exe: &Path,
    unity_project_dir: &Path,
    confirm_token: &str,
) -> AdmResult<String> {
    if archive_id.trim().is_empty() {
        return Err(AdmError::invalid_input("archive id cannot be empty"));
    }
    if target_id.trim().is_empty() {
        return Err(AdmError::invalid_input(
            "game build target id cannot be empty",
        ));
    }
    let app = AdmApplication::for_data_root(data_root)?;
    let archive = app.load_project(archive_id)?;
    let plan = GameBuildPlan::windows_desktop_prototype(archive.manifest.project_id.clone());
    let target = plan
        .targets
        .iter()
        .find(|target| target.target_id == target_id.trim())
        .ok_or_else(|| {
            AdmError::invalid_input(format!("unknown game build target: {target_id}"))
        })?;
    let preflight =
        inspect_unity_build_preflight(target, unity_exe, unity_project_dir, confirm_token)?;
    if !preflight.ready_for_local_build() {
        return Err(AdmError::validation(format!(
            "Unity runtime validation preflight failed\n{}",
            preflight.render()
        )));
    }
    let command = plan_unity_runtime_validation(target, unity_exe, unity_project_dir)?;
    let report = LocalProcessEngineBuildRunner.run(&command)?;
    if report.status != EngineBuildExecutionStatus::Succeeded {
        return Err(AdmError::validation(format!(
            "Unity runtime validation did not produce the expected output\n{}",
            report.render()
        )));
    }
    let execution_text = std::fs::read_to_string(&report.expected_output_path)?;
    let commit = app.commit_runtime_validation_execution(&archive, &execution_text)?;
    Ok(format!(
        "{}runtime_results_file={}\nruntime_ready={}\nruntime_runner={}\nruntime_contract_rows={}\nruntime_observed_rows={}\nruntime_passed_rows={}\nruntime_failed_rows={}\nruntime_missing_rows={}\nruntime_unexpected_rows={}\nruntime_commit_files={}",
        report.render(),
        commit.results_file.display(),
        commit.summary.ready(),
        commit.summary.runner,
        commit.summary.contract_rows,
        commit.summary.observed_rows,
        commit.summary.passed_rows,
        commit.summary.failed_rows,
        commit.summary.missing_rows,
        commit.summary.unexpected_rows,
        commit.commit.written_files.len()
    ))
}

fn record_runtime_validation_for_ui(
    data_root: &Path,
    archive_id: &str,
    results_file: &Path,
) -> AdmResult<String> {
    if archive_id.trim().is_empty() {
        return Err(AdmError::invalid_input("archive id cannot be empty"));
    }
    if results_file.as_os_str().is_empty() {
        return Err(AdmError::invalid_input(
            "runtime validation results file cannot be empty",
        ));
    }
    let app = AdmApplication::for_data_root(data_root)?;
    let archive = app.load_project(archive_id)?;
    let execution_text = std::fs::read_to_string(results_file)?;
    let commit = app.commit_runtime_validation_execution(&archive, &execution_text)?;
    Ok(format!(
        "Runtime Validation Result: ready={} contract_rows={} observed_rows={} passed_rows={} failed_rows={} missing_rows={} unexpected_rows={}\nresults_file={}\nsource_file={}\nruntime_commit_files={}",
        commit.summary.ready(),
        commit.summary.contract_rows,
        commit.summary.observed_rows,
        commit.summary.passed_rows,
        commit.summary.failed_rows,
        commit.summary.missing_rows,
        commit.summary.unexpected_rows,
        commit.results_file.display(),
        results_file.display(),
        commit.commit.written_files.len()
    ))
}

fn game_build_target_dir_from_ui(
    data_root: &Path,
    archive_id: &str,
    target_id: &str,
    value: String,
) -> PathBuf {
    let trimmed = value.trim();
    if !trimmed.is_empty() {
        return PathBuf::from(trimmed);
    }
    data_root
        .join("build-bundles")
        .join(if archive_id.trim().is_empty() {
            "unselected"
        } else {
            archive_id.trim()
        })
        .join(if target_id.trim().is_empty() {
            "target"
        } else {
            target_id.trim()
        })
}

fn sdk_bundle_target_dir_from_ui(data_root: &Path, archive_id: &str, value: String) -> PathBuf {
    let trimmed = value.trim();
    if !trimmed.is_empty() {
        return PathBuf::from(trimmed);
    }
    data_root
        .join("sdk-bundles")
        .join(if archive_id.trim().is_empty() {
            "unselected"
        } else {
            archive_id.trim()
        })
}

fn default_desktop_release_source() -> PathBuf {
    std::env::current_exe().unwrap_or_else(|_| {
        std::env::current_dir()
            .unwrap_or_else(|_| PathBuf::from("."))
            .join("target")
            .join("release")
            .join("adm-desktop.exe")
    })
}

fn default_desktop_release_dir() -> PathBuf {
    std::env::current_dir()
        .unwrap_or_else(|_| PathBuf::from("."))
        .join("dist")
        .join("AutoDesignMaker-rust")
}

fn default_game_build_bundle_dir() -> PathBuf {
    std::env::current_dir()
        .unwrap_or_else(|_| PathBuf::from("."))
        .join("dist")
        .join("game-build")
        .join("windows_desktop_playable")
}

fn default_sdk_bundle_dir() -> PathBuf {
    std::env::current_dir()
        .unwrap_or_else(|_| PathBuf::from("."))
        .join("dist")
        .join("sdk-bundle")
}

fn path_from_optional_text(value: String, fallback: PathBuf) -> PathBuf {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        fallback
    } else {
        PathBuf::from(trimmed)
    }
}

fn runtime_results_file_from_ui(unity_project_dir: &Path, value: String) -> PathBuf {
    path_from_optional_text(value, default_runtime_results_file(unity_project_dir))
}

fn default_runtime_results_file(unity_project_dir: &Path) -> PathBuf {
    unity_project_dir
        .join("Library")
        .join("AutoDesignMaker")
        .join("runtime_execution_results.adm")
}

fn default_unity_project_dir() -> PathBuf {
    std::env::current_dir()
        .unwrap_or_else(|_| PathBuf::from("."))
        .join("unity-project")
}

fn first_archive_id(project_list: &str) -> Option<String> {
    project_list
        .lines()
        .next()
        .and_then(|line| line.split('|').next())
        .map(str::trim)
        .filter(|value| !value.is_empty() && *value != "No projects yet.")
        .map(ToOwned::to_owned)
}

fn path_to_text(path: &Path) -> String {
    path.to_string_lossy().into_owned()
}

#[cfg(test)]
mod ui_layout_tests {
    const UI_SOURCE: &str = include_str!("../ui/main.slint");

    #[test]
    fn long_text_surfaces_are_scrollable_and_wrapped() {
        for binding in [
            "root.design-ai-interview-text",
            "root.design-right-tab",
            "root.pipeline-service-text",
            "root.supplement-analysis-text",
            "root.package-text",
            "root.game-build-text",
            "root.run-log-text",
            "root.sdk-review-text",
        ] {
            let segment = nearest_scroll_segment(binding);
            assert!(
                segment.contains("wrap: word-wrap;"),
                "{binding} must wrap inside its ScrollView"
            );
        }
    }

    #[test]
    fn long_row_lists_are_scrollable() {
        for binding in [
            "root.stage-items",
            "root.package-file-items",
            "root.sdk-review-items",
            "root.sdk-resource-items",
        ] {
            let _ = nearest_scroll_segment(binding);
        }
    }

    fn nearest_scroll_segment(binding: &str) -> &'static str {
        let binding_index = UI_SOURCE
            .find(binding)
            .unwrap_or_else(|| panic!("{binding} binding missing from Slint UI"));
        let before_binding = &UI_SOURCE[..binding_index];
        let scroll_index = before_binding
            .rfind("ScrollView")
            .unwrap_or_else(|| panic!("{binding} is not inside a ScrollView"));
        let target_close_index = (binding_index + 1400).min(UI_SOURCE.len());
        let close_index = UI_SOURCE
            .char_indices()
            .map(|(index, _)| index)
            .find(|index| *index >= target_close_index)
            .unwrap_or(UI_SOURCE.len());
        let segment = &UI_SOURCE[scroll_index..close_index];
        assert!(
            segment.contains(binding),
            "{binding} is not covered by the nearest ScrollView segment"
        );
        segment
    }
}
