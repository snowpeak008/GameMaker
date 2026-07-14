#![forbid(unsafe_code)]

use adm_new_ai::AiConfigService;
use adm_new_ai::adapters::CodexCliAdapter;
use adm_new_ai::image::{
    build_image_probe_request, check_png_metadata_bytes, codex_image_command,
    image_settings_from_config,
};
use adm_new_application::dev_tools::{
    check_environment_config, compile_all_config, compile_check_plan, evaluate_compile_output,
    generate_ui_state_artifacts, local_git_command_spec, run_error_logger_pipeline,
    run_perf_pipeline, run_test_generation_pipeline, scaffold_project, scaffold_step,
};
use adm_new_application::iteration::{
    IterationDeltaPlan, IterationPrepareRequest, build_delta_execution_plan,
    inherit_skipped_artifacts, parse_iteration_spec, prepare_iteration,
    summarize_iteration_resume_plan,
};
use adm_new_application::migration_tools::{
    export_design_concept_package, inspect_pipeline_reports,
    migrate_design_projects_to_execution_objects, migrate_execution_object_save_ids,
    migrate_structured_schema, run_legacy_migration, scan_hardcoded_paths,
};
use adm_new_application::validation_tools::{
    check_pipeline_plan_002, collect_design_semantic_quality, collect_pipeline_quality_metrics,
    lint_context_file, validate_agent_output, validate_config_tables,
    validate_contract_file_report, write_design_semantic_quality_outputs,
};
use adm_new_artifact::asset_tools::{
    generate_audio_placeholder, generate_sfx_placeholder, pack_sprite_atlas,
    run_localization_injector, slice_sprite_sheet,
};
use adm_new_contracts::package::PackageStatus;
use adm_new_contracts::patch::PatchStatus;
use adm_new_contracts::project::ProjectState;
use adm_new_contracts::sdk::SdkReviewStatus;
use adm_new_foundation::{AdmResult, GateReport};
use adm_new_governance::{
    design_sync_audit_report, final_handoff_v3_gate_report, find_repo_root, handoff_report,
    integration_test_migration_gate_report, is_standalone_repo_root, iteration_gate_report,
    package_gate_report, parity_gate_report, plan_gate_report, release_gate_report,
    render_design_sync_audit, standalone_boundary_gate_report, ui_ai_gate_report,
    ui_parity_v3_gate_report, ui_pipeline_gate_report, ui_settings_style_gate_report,
    ui_shell_gate_report, ui_utility_gate_report, ui_workbench_gate_report,
    unit_test_migration_gate_report, validation_gate_report,
};
use adm_new_packaging::{
    DEFAULT_DIST_EXE_NAME, DEFAULT_MIN_EXE_BYTES, PackagingService, dist_build_plan,
    verify_dist_bundle,
};
use adm_new_patch::{CodexPatchRunner, PatchAnalyzer, PatchExecutor, PatchStore};
use adm_new_sdk::{
    ExtractedSdkDocument, SdkKnowledgeBase, extract_readable_text, extract_sdk_spec_with_adapter,
};
use serde_json::json;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};
use std::sync::OnceLock;

const LEGACY_MEMORY_COMMAND_DEPRECATED: &str = "legacy_memory_command_deprecated";
static PROJECT_ROOT_OVERRIDE: OnceLock<PathBuf> = OnceLock::new();

fn main() -> ExitCode {
    let mut args: Vec<String> = env::args().collect();
    let explicit_project_root =
        match take_leading_project_root(&mut args).and_then(|root| match root {
            Some(root) => Ok(Some(root)),
            None => take_root_scoped_project_root(&mut args),
        }) {
            Ok(root) => root,
            Err(message) => {
                eprintln!("{message}");
                return ExitCode::from(2);
            }
        };
    if let Some(root) = explicit_project_root {
        if !is_standalone_repo_root(&root) {
            eprintln!("invalid_standalone_project_root:{}", root.display());
            return ExitCode::from(2);
        }
        let canonical = root.canonicalize().unwrap_or(root);
        let _ = PROJECT_ROOT_OVERRIDE.set(canonical);
    }
    let command = args.get(1).map(String::as_str).unwrap_or("doctor");
    match command {
        "doctor" | "standalone-boundary-gate" => {
            run_gate("standalone-boundary", standalone_boundary_gate_report)
        }
        "plan-gate" => run_gate("plan", plan_gate_report),
        "parity-gate" => run_gate("parity", |repo_root| {
            let mut report = parity_gate_report(repo_root)?;
            add_cargo_test_result(repo_root, &mut report);
            Ok(report)
        }),
        "ui-gate" => run_gate("ui", |repo_root| {
            let mut report = GateReport::new("NEWrust UI Parity Gate");
            let web_root = repo_root.join("web");
            report.add_row("web_root", web_root.display().to_string());
            add_process_result(
                &mut report,
                "npm_build",
                "npm.cmd",
                &["run", "build"],
                &web_root,
            );
            add_process_result(
                &mut report,
                "npm_e2e_all",
                "npm.cmd",
                &["run", "e2e"],
                &web_root,
            );
            add_process_result(
                &mut report,
                "npm_ui_gate",
                "npm.cmd",
                &["run", "ui-gate"],
                &web_root,
            );
            Ok(report)
        }),
        "package-gate" => run_gate("package", package_gate_report),
        "validation-gate" => run_gate("validation", validation_gate_report),
        "iteration-gate" => run_gate("iteration", iteration_gate_report),
        "ui-shell-gate" => run_gate("ui-shell", ui_shell_gate_report),
        "ui-workbench-gate" => run_gate("ui-workbench", ui_workbench_gate_report),
        "ui-ai-gate" => run_gate("ui-ai", ui_ai_gate_report),
        "ui-pipeline-gate" => run_gate("ui-pipeline", ui_pipeline_gate_report),
        "ui-utility-gate" => run_gate("ui-utility", ui_utility_gate_report),
        "ui-settings-style-gate" => run_gate("ui-settings-style", ui_settings_style_gate_report),
        "ui-parity-v3-gate" => run_gate("ui-parity-v3", ui_parity_v3_gate_report),
        "unit-test-migration-gate" => {
            run_gate("unit-test-migration", unit_test_migration_gate_report)
        }
        "integration-test-migration-gate" => run_gate(
            "integration-test-migration",
            integration_test_migration_gate_report,
        ),
        "release-gate" => run_gate("release", release_gate_report),
        "final-handoff-v3-gate" => run_gate("final-handoff-v3", final_handoff_v3_gate_report),
        "handoff-report" => run_gate("handoff", handoff_report),
        "design-sync-audit" => run_design_sync_audit_command(&args[2..]),
        "package" => run_package_command(&args[2..]),
        "dist" => run_dist_command(&args[2..]),
        "asset" => run_asset_command(&args[2..]),
        "image" => run_image_command(&args[2..]),
        "dev" => run_dev_command(&args[2..]),
        "project" => run_project_command(&args[2..]),
        "pipeline" => run_pipeline_command(&args[2..]),
        "unity" => run_unity_command(&args[2..]),
        "validate" => run_validate_command(&args[2..]),
        "migrate" => run_migrate_command(&args[2..]),
        "schema" => run_schema_command(&args[2..]),
        "governance" => run_governance_command(&args[2..]),
        "design" => run_design_command(&args[2..]),
        "iteration" | "iterate" => run_iteration_command(&args[2..]),
        "memory" | "ucos" => run_memory_command(&args[2..]),
        "patch" => run_patch_command(&args[2..]),
        "sdk" => run_sdk_command(&args[2..]),
        "help" | "--help" | "-h" => {
            print_help();
            ExitCode::SUCCESS
        }
        other => {
            eprintln!("unknown command: {other}");
            print_help();
            ExitCode::from(2)
        }
    }
}

fn run_patch_command(args: &[String]) -> ExitCode {
    if args
        .first()
        .map(|value| matches!(value.as_str(), "--help" | "-h" | "help"))
        .unwrap_or(true)
    {
        print_patch_help();
        return ExitCode::SUCCESS;
    }
    let Some(repo_root) = repo_root_from_cwd() else {
        eprintln!("failed to locate standalone project root");
        return ExitCode::from(1);
    };
    let parsed = match parse_patch_args(args, &repo_root) {
        Ok(parsed) => parsed,
        Err(error) => {
            eprintln!("{error}");
            print_patch_help();
            return ExitCode::from(2);
        }
    };
    let store = PatchStore::new(&parsed.store_root);
    let executor = PatchExecutor::new(&parsed.project_root, store.clone());
    match parsed.command.as_str() {
        "analyze" => {
            if parsed.rest.is_empty() {
                eprintln!("patch analyze requires a request");
                return ExitCode::from(2);
            }
            print_json_result(PatchAnalyzer::new(Some(store)).analyze(&parsed.rest.join(" "), true))
        }
        "list" => match store.list() {
            Ok(records) => {
                let records = if let Some(status) = parsed.status {
                    records
                        .into_iter()
                        .filter(|record| record.status == status)
                        .collect::<Vec<_>>()
                } else {
                    records
                };
                print_json_result(Ok(records))
            }
            Err(error) => {
                eprintln!("{error}");
                ExitCode::from(1)
            }
        },
        "show" => {
            let Some(patch_id) = parsed.rest.first() else {
                eprintln!("patch show requires patch_id");
                return ExitCode::from(2);
            };
            print_json_result(store.get(patch_id))
        }
        "validate" => {
            let Some(patch_id) = parsed.rest.first() else {
                eprintln!("patch validate requires patch_id");
                return ExitCode::from(2);
            };
            print_json_result(executor.validate(patch_id))
        }
        "apply" => {
            let Some(patch_id) = parsed.rest.first() else {
                eprintln!("patch apply requires patch_id");
                return ExitCode::from(2);
            };
            let runner = CodexPatchRunner::new(&parsed.project_root, CodexCliAdapter::default());
            print_json_result(executor.apply(patch_id, &runner))
        }
        "promote" => {
            if parsed.rest.len() < 2 {
                eprintln!("patch promote requires patch_id and iteration_spec");
                return ExitCode::from(2);
            }
            print_json_result(executor.promote(&parsed.rest[0], &parsed.rest[1]))
        }
        other => {
            eprintln!("unknown patch command: {other}");
            print_patch_help();
            ExitCode::from(2)
        }
    }
}

#[derive(Debug, Clone)]
struct ParsedPatchArgs {
    store_root: std::path::PathBuf,
    project_root: std::path::PathBuf,
    status: Option<PatchStatus>,
    command: String,
    rest: Vec<String>,
}

fn parse_patch_args(args: &[String], repo_root: &Path) -> AdmResult<ParsedPatchArgs> {
    let mut store_root = None;
    let mut project_root = repo_root.to_path_buf();
    let mut status = None;
    let mut command = String::new();
    let mut rest = Vec::new();
    let mut index = 0usize;
    while index < args.len() {
        match args[index].as_str() {
            "--root" => {
                index += 1;
                let Some(value) = args.get(index) else {
                    return Err(adm_new_foundation::AdmError::new("--root requires a path"));
                };
                store_root = Some(std::path::PathBuf::from(value));
            }
            "--project-root" => {
                index += 1;
                let Some(value) = args.get(index) else {
                    return Err(adm_new_foundation::AdmError::new(
                        "--project-root requires a path",
                    ));
                };
                project_root = std::path::PathBuf::from(value);
            }
            "--status" => {
                index += 1;
                let Some(value) = args.get(index) else {
                    return Err(adm_new_foundation::AdmError::new(
                        "--status requires a value",
                    ));
                };
                status = Some(parse_patch_status(value)?);
            }
            value if value.starts_with('-') => {
                return Err(adm_new_foundation::AdmError::new(format!(
                    "unknown patch option: {value}"
                )));
            }
            value => {
                command = value.to_string();
                rest.extend(args.iter().skip(index + 1).cloned());
                break;
            }
        }
        index += 1;
    }
    if command.is_empty() {
        return Err(adm_new_foundation::AdmError::new(
            "patch command is required",
        ));
    }
    let store_root = store_root.unwrap_or_else(|| {
        PatchStore::from_project_root(repo_root, "cli")
            .root()
            .to_path_buf()
    });
    Ok(ParsedPatchArgs {
        store_root,
        project_root,
        status,
        command,
        rest,
    })
}

fn parse_patch_status(value: &str) -> AdmResult<PatchStatus> {
    match value {
        "analyzed" => Ok(PatchStatus::Analyzed),
        "applied" => Ok(PatchStatus::Applied),
        "validated" => Ok(PatchStatus::Validated),
        "promoted" => Ok(PatchStatus::Promoted),
        "failed" => Ok(PatchStatus::Failed),
        other => Err(adm_new_foundation::AdmError::new(format!(
            "unknown patch status: {other}"
        ))),
    }
}

fn run_sdk_command(args: &[String]) -> ExitCode {
    if args
        .first()
        .map(|value| matches!(value.as_str(), "--help" | "-h" | "help"))
        .unwrap_or(true)
    {
        print_sdk_help();
        return ExitCode::SUCCESS;
    }
    let Some(repo_root) = repo_root_from_cwd() else {
        eprintln!("failed to locate standalone project root");
        return ExitCode::from(1);
    };
    let parsed = match parse_sdk_args(args, &repo_root) {
        Ok(parsed) => parsed,
        Err(error) => {
            eprintln!("{error}");
            print_sdk_help();
            return ExitCode::from(2);
        }
    };
    let kb = SdkKnowledgeBase::with_seed_root(&parsed.root, &parsed.seed_root);
    match parsed.command.as_str() {
        "init" => match kb.initialize() {
            Ok(()) => print_json_result(Ok(json!({
                "status": "ok",
                "root": kb.root().display().to_string()
            }))),
            Err(error) => {
                eprintln!("{error}");
                ExitCode::from(1)
            }
        },
        "list" => print_json_result(kb.read_index()),
        "show" => {
            let Some(sdk_id) = parsed.rest.first() else {
                eprintln!("sdk show requires sdk_id");
                return ExitCode::from(2);
            };
            match kb.read_spec(sdk_id) {
                Ok(Some(spec)) => print_json_result(Ok(spec)),
                Ok(None) => {
                    eprintln!("Unknown SDK spec: {sdk_id}");
                    ExitCode::from(1)
                }
                Err(error) => {
                    eprintln!("{error}");
                    ExitCode::from(1)
                }
            }
        }
        "add" => {
            let mut rest = parsed.rest.clone();
            let source_url = take_option(&mut rest, "--url").unwrap_or_default();
            let Some(name) = rest.first() else {
                eprintln!("sdk add requires name");
                return ExitCode::from(2);
            };
            print_json_result(kb.add_placeholder(name, &source_url))
        }
        "review" => {
            if parsed.rest.len() < 2 {
                eprintln!("sdk review requires sdk_id and status");
                return ExitCode::from(2);
            }
            let status = match parse_sdk_review_status(&parsed.rest[1]) {
                Ok(status) => status,
                Err(error) => {
                    eprintln!("{error}");
                    return ExitCode::from(2);
                }
            };
            print_json_result(kb.update_review_status(&parsed.rest[0], status))
        }
        "context" => match kb.approved_prompt_context() {
            Ok(context) => {
                print!("{context}");
                ExitCode::SUCCESS
            }
            Err(error) => {
                eprintln!("{error}");
                ExitCode::from(1)
            }
        },
        "sync" => {
            let Some(source) = parsed.rest.first() else {
                eprintln!("sdk sync requires a local HTML file path");
                return ExitCode::from(2);
            };
            let source_path = Path::new(source);
            if !source_path.exists()
                && (source.starts_with("http://") || source.starts_with("https://"))
            {
                eprintln!(
                    "sdk sync remote fetch is not part of deterministic gate; pass a local HTML file"
                );
                return ExitCode::from(1);
            }
            let html = match fs::read_to_string(source_path) {
                Ok(html) => html,
                Err(error) => {
                    eprintln!("failed to read SDK HTML source: {error}");
                    return ExitCode::from(1);
                }
            };
            let (title, text) = extract_readable_text(&html);
            let document = ExtractedSdkDocument {
                source_url: source.clone(),
                title,
                text,
            };
            match extract_sdk_spec_with_adapter(&document, CodexCliAdapter::default())
                .and_then(|spec| kb.write_spec(spec))
            {
                Ok(spec) => print_json_result(Ok(spec)),
                Err(error) => {
                    eprintln!("{error}");
                    ExitCode::from(1)
                }
            }
        }
        other => {
            eprintln!("unknown sdk command: {other}");
            print_sdk_help();
            ExitCode::from(2)
        }
    }
}

#[derive(Debug, Clone)]
struct ParsedSdkArgs {
    root: std::path::PathBuf,
    seed_root: std::path::PathBuf,
    command: String,
    rest: Vec<String>,
}

fn parse_sdk_args(args: &[String], repo_root: &Path) -> AdmResult<ParsedSdkArgs> {
    let mut root = None;
    let mut data_root = None;
    let mut command = String::new();
    let mut rest = Vec::new();
    let mut index = 0usize;
    while index < args.len() {
        match args[index].as_str() {
            "--root" => {
                index += 1;
                let Some(value) = args.get(index) else {
                    return Err(adm_new_foundation::AdmError::new("--root requires a path"));
                };
                root = Some(std::path::PathBuf::from(value));
            }
            "--data-root" => {
                index += 1;
                let Some(value) = args.get(index) else {
                    return Err(adm_new_foundation::AdmError::new(
                        "--data-root requires a path",
                    ));
                };
                data_root = Some(std::path::PathBuf::from(value));
            }
            value if value.starts_with('-') => {
                return Err(adm_new_foundation::AdmError::new(format!(
                    "unknown sdk option: {value}"
                )));
            }
            value => {
                command = value.to_string();
                rest.extend(args.iter().skip(index + 1).cloned());
                break;
            }
        }
        index += 1;
    }
    if command.is_empty() {
        return Err(adm_new_foundation::AdmError::new("sdk command is required"));
    }
    if root.is_some() && data_root.is_some() {
        return Err(adm_new_foundation::AdmError::new(
            "--root and --data-root are mutually exclusive",
        ));
    }
    let root = root.unwrap_or_else(|| {
        data_root
            .unwrap_or_else(default_runtime_data_root)
            .join("knowledge")
            .join("sdks")
    });
    Ok(ParsedSdkArgs {
        root,
        seed_root: repo_root.join("knowledge").join("sdks"),
        command,
        rest,
    })
}

fn default_runtime_data_root() -> PathBuf {
    if let Some(path) = env::var_os("ADM_NEWRUST_DATA_DIR").filter(|value| !value.is_empty()) {
        return PathBuf::from(path);
    }
    if cfg!(windows) {
        if let Some(path) = env::var_os("LOCALAPPDATA").filter(|value| !value.is_empty()) {
            return PathBuf::from(path).join("AutoDesignMaker-NEWrust");
        }
    } else {
        if let Some(path) = env::var_os("XDG_DATA_HOME").filter(|value| !value.is_empty()) {
            return PathBuf::from(path).join("autodesignmaker-newrust");
        }
        if let Some(path) = env::var_os("HOME").filter(|value| !value.is_empty()) {
            return PathBuf::from(path)
                .join(".local")
                .join("share")
                .join("autodesignmaker-newrust");
        }
    }
    env::temp_dir().join("AutoDesignMaker-NEWrust")
}

fn parse_sdk_review_status(value: &str) -> AdmResult<SdkReviewStatus> {
    match value {
        "draft" => Ok(SdkReviewStatus::Draft),
        "pending_review" => Ok(SdkReviewStatus::PendingReview),
        "approved" => Ok(SdkReviewStatus::Approved),
        "rejected" => Ok(SdkReviewStatus::Rejected),
        other => Err(adm_new_foundation::AdmError::new(format!(
            "unknown SDK review status: {other}"
        ))),
    }
}

fn take_option(values: &mut Vec<String>, flag: &str) -> Option<String> {
    let index = values.iter().position(|value| value == flag)?;
    values.remove(index);
    if index < values.len() {
        Some(values.remove(index))
    } else {
        None
    }
}

fn take_leading_project_root(args: &mut Vec<String>) -> Result<Option<PathBuf>, String> {
    let Some(value) = args.get(1).cloned() else {
        return Ok(None);
    };
    if value == "--project-root" {
        let Some(path) = args.get(2).cloned() else {
            return Err("--project-root requires a path".to_string());
        };
        args.drain(1..=2);
        return Ok(Some(PathBuf::from(path)));
    }
    if let Some(path) = value.strip_prefix("--project-root=") {
        if path.trim().is_empty() {
            return Err("--project-root requires a path".to_string());
        }
        args.remove(1);
        return Ok(Some(PathBuf::from(path)));
    }
    Ok(None)
}

fn take_root_scoped_project_root(args: &mut Vec<String>) -> Result<Option<PathBuf>, String> {
    let Some(command) = args.get(1) else {
        return Ok(None);
    };
    if !(command == "doctor"
        || command == "package"
        || command == "dist"
        || command == "handoff-report"
        || command.ends_with("-gate"))
    {
        return Ok(None);
    }
    let Some(index) = args.iter().enumerate().skip(2).find_map(|(index, value)| {
        (value == "--project-root" || value.starts_with("--project-root=")).then_some(index)
    }) else {
        return Ok(None);
    };
    let option = args[index].clone();
    if option == "--project-root" {
        let Some(path) = args.get(index + 1).cloned() else {
            return Err("--project-root requires a path".to_string());
        };
        args.drain(index..=index + 1);
        return Ok(Some(PathBuf::from(path)));
    }
    let path = option.trim_start_matches("--project-root=");
    if path.trim().is_empty() {
        return Err("--project-root requires a path".to_string());
    }
    let path = PathBuf::from(path);
    args.remove(index);
    Ok(Some(path))
}

fn run_package_command(args: &[String]) -> ExitCode {
    if args
        .first()
        .map(|value| matches!(value.as_str(), "--help" | "-h" | "help"))
        .unwrap_or(false)
    {
        print_package_help();
        return ExitCode::SUCCESS;
    }
    let Some(repo_root) = repo_root_from_cwd() else {
        eprintln!("failed to locate standalone project root");
        return ExitCode::from(1);
    };
    let parsed = match parse_package_args(args, &repo_root) {
        Ok(parsed) => parsed,
        Err(error) => {
            eprintln!("{error}");
            print_package_help();
            return ExitCode::from(2);
        }
    };
    match PackagingService::new().run_package_to_dir(&parsed.artifacts_dir, &parsed.outputs_dir) {
        Ok(result) => {
            let status = result.status.clone();
            println!(
                "{}",
                serde_json::to_string_pretty(&result).unwrap_or_default()
            );
            if status == PackageStatus::Success {
                ExitCode::SUCCESS
            } else {
                ExitCode::from(1)
            }
        }
        Err(error) => {
            eprintln!("{error}");
            ExitCode::from(1)
        }
    }
}

#[derive(Debug, Clone)]
struct ParsedPackageArgs {
    artifacts_dir: std::path::PathBuf,
    outputs_dir: std::path::PathBuf,
}

fn parse_package_args(args: &[String], repo_root: &Path) -> AdmResult<ParsedPackageArgs> {
    let mut artifacts_dir = repo_root.join("sandbox").join("outputs").join("artifacts");
    let mut outputs_dir = repo_root.join("sandbox").join("outputs");
    let mut index = 0usize;
    while index < args.len() {
        match args[index].as_str() {
            "run" => {}
            "--artifacts-dir" => {
                index += 1;
                let Some(value) = args.get(index) else {
                    return Err(adm_new_foundation::AdmError::new(
                        "--artifacts-dir requires a path",
                    ));
                };
                artifacts_dir = std::path::PathBuf::from(value);
            }
            "--outputs-dir" => {
                index += 1;
                let Some(value) = args.get(index) else {
                    return Err(adm_new_foundation::AdmError::new(
                        "--outputs-dir requires a path",
                    ));
                };
                outputs_dir = std::path::PathBuf::from(value);
            }
            value => {
                return Err(adm_new_foundation::AdmError::new(format!(
                    "unknown package option: {value}"
                )));
            }
        }
        index += 1;
    }
    Ok(ParsedPackageArgs {
        artifacts_dir,
        outputs_dir,
    })
}

fn run_dist_command(args: &[String]) -> ExitCode {
    if args
        .first()
        .map(|value| matches!(value.as_str(), "--help" | "-h" | "help"))
        .unwrap_or(true)
    {
        print_dist_help();
        return ExitCode::SUCCESS;
    }
    let Some(repo_root) = repo_root_from_cwd() else {
        eprintln!("failed to locate standalone project root");
        return ExitCode::from(1);
    };
    match args.first().map(String::as_str).unwrap_or("") {
        "build" => run_dist_build(&repo_root, &args[1..]),
        "verify-bundle" => run_dist_verify_bundle(&args[1..]),
        other => {
            eprintln!("unknown dist command: {other}");
            print_dist_help();
            ExitCode::from(2)
        }
    }
}

fn run_dist_build(repo_root: &Path, args: &[String]) -> ExitCode {
    let execute = args.iter().any(|arg| arg == "--execute");
    let unknown = args
        .iter()
        .find(|arg| arg.starts_with('-') && *arg != "--execute");
    if let Some(unknown) = unknown {
        eprintln!("unknown dist build option: {unknown}");
        return ExitCode::from(2);
    }
    let plan = dist_build_plan(repo_root);
    if !execute {
        return print_json_result(Ok(plan));
    }
    let Some((program, rest)) = plan.command.split_first() else {
        eprintln!("dist build plan has empty command");
        return ExitCode::from(1);
    };
    match Command::new(program)
        .args(rest)
        .current_dir(&plan.cwd)
        .status()
    {
        Ok(status) => {
            println!(
                "{}",
                serde_json::to_string_pretty(&json!({
                    "plan": plan,
                    "exit_code": status.code().unwrap_or(-1),
                    "success": status.success(),
                }))
                .unwrap_or_default()
            );
            if status.success() {
                ExitCode::SUCCESS
            } else {
                ExitCode::from(1)
            }
        }
        Err(error) => {
            eprintln!("failed to run dist build: {error}");
            ExitCode::from(1)
        }
    }
}

fn run_dist_verify_bundle(args: &[String]) -> ExitCode {
    let Some(bundle_dir) = args.first() else {
        eprintln!("dist verify-bundle requires bundle_dir");
        return ExitCode::from(2);
    };
    let mut rest = args.iter().skip(1).cloned().collect::<Vec<_>>();
    let exe_name = take_option(&mut rest, "--exe").unwrap_or_else(|| DEFAULT_DIST_EXE_NAME.into());
    let min_exe_bytes = match take_option(&mut rest, "--min-bytes") {
        Some(value) => match value.parse::<u64>() {
            Ok(value) => value,
            Err(_) => {
                eprintln!("--min-bytes requires an integer");
                return ExitCode::from(2);
            }
        },
        None => DEFAULT_MIN_EXE_BYTES,
    };
    let required_items = take_repeated_options(&mut rest, "--require");
    if let Some(unknown) = rest.iter().find(|value| value.starts_with('-')) {
        eprintln!("unknown dist verify-bundle option: {unknown}");
        return ExitCode::from(2);
    }
    let report = verify_dist_bundle(
        Path::new(bundle_dir),
        &exe_name,
        min_exe_bytes,
        &required_items,
    );
    let ok = report.ok;
    println!(
        "{}",
        serde_json::to_string_pretty(&report).unwrap_or_default()
    );
    if ok {
        ExitCode::SUCCESS
    } else {
        ExitCode::from(1)
    }
}

fn run_asset_command(args: &[String]) -> ExitCode {
    if args
        .first()
        .map(|value| matches!(value.as_str(), "--help" | "-h" | "help"))
        .unwrap_or(true)
    {
        print_asset_help();
        return ExitCode::SUCCESS;
    }
    match args.first().map(String::as_str).unwrap_or("") {
        "audio-placeholder" => {
            let mut rest = args.iter().skip(1).cloned().collect::<Vec<_>>();
            let output_dir =
                take_option(&mut rest, "--output-dir").unwrap_or_else(|| "ArtAssets/Audio".into());
            let filename = take_option(&mut rest, "--filename");
            if let Some(unknown) = rest.iter().find(|value| value.starts_with('-')) {
                eprintln!("unknown asset audio-placeholder option: {unknown}");
                return ExitCode::from(2);
            }
            print_json_result(generate_audio_placeholder(output_dir, filename.as_deref()))
        }
        "sfx-placeholder" => {
            let mut rest = args.iter().skip(1).cloned().collect::<Vec<_>>();
            let output_dir =
                take_option(&mut rest, "--output-dir").unwrap_or_else(|| "ArtAssets/Audio".into());
            let filename = take_option(&mut rest, "--filename");
            let duration = match take_option(&mut rest, "--duration") {
                Some(value) => match value.parse::<f32>() {
                    Ok(value) => value,
                    Err(_) => {
                        eprintln!("--duration requires a number");
                        return ExitCode::from(2);
                    }
                },
                None => 3.0,
            };
            if let Some(unknown) = rest.iter().find(|value| value.starts_with('-')) {
                eprintln!("unknown asset sfx-placeholder option: {unknown}");
                return ExitCode::from(2);
            }
            if rest.is_empty() {
                eprintln!("asset sfx-placeholder requires prompt");
                return ExitCode::from(2);
            }
            print_json_result(generate_sfx_placeholder(
                &rest.join(" "),
                output_dir,
                duration,
                filename.as_deref(),
            ))
        }
        "sprite-slice" => {
            let mut rest = args.iter().skip(1).cloned().collect::<Vec<_>>();
            let grid = take_option(&mut rest, "--grid").unwrap_or_else(|| "5x4".into());
            let cell_size =
                take_option(&mut rest, "--cell-size").unwrap_or_else(|| "128x128".into());
            let gap = match take_option(&mut rest, "--gap") {
                Some(value) => match value.parse::<u32>() {
                    Ok(value) => value,
                    Err(_) => {
                        eprintln!("--gap requires an integer");
                        return ExitCode::from(2);
                    }
                },
                None => 4,
            };
            if let Some(unknown) = rest.iter().find(|value| value.starts_with('-')) {
                eprintln!("unknown asset sprite-slice option: {unknown}");
                return ExitCode::from(2);
            }
            if rest.len() < 2 {
                eprintln!("asset sprite-slice requires sheet_path and output_dir");
                return ExitCode::from(2);
            }
            print_json_result(slice_sprite_sheet(
                &rest[0], &rest[1], &grid, &cell_size, gap,
            ))
        }
        "sprite-atlas" => {
            let mut rest = args.iter().skip(1).cloned().collect::<Vec<_>>();
            let cell_size = take_option(&mut rest, "--cell-size");
            let frames = take_repeated_options(&mut rest, "--frame")
                .into_iter()
                .map(std::path::PathBuf::from)
                .collect::<Vec<_>>();
            if let Some(unknown) = rest.iter().find(|value| value.starts_with('-')) {
                eprintln!("unknown asset sprite-atlas option: {unknown}");
                return ExitCode::from(2);
            }
            if rest.len() < 2 {
                eprintln!("asset sprite-atlas requires output_dir and atlas_name");
                return ExitCode::from(2);
            }
            if frames.is_empty() {
                eprintln!("asset sprite-atlas requires at least one --frame PNG");
                return ExitCode::from(2);
            }
            print_json_result(pack_sprite_atlas(
                &frames,
                &rest[0],
                &rest[1],
                cell_size.as_deref(),
            ))
        }
        "localization" => {
            let rest = args.iter().skip(1).cloned().collect::<Vec<_>>();
            if rest.len() < 2 {
                eprintln!("asset localization requires source_dir and output_dir");
                return ExitCode::from(2);
            }
            print_json_result(run_localization_injector(&rest[0], &rest[1]))
        }
        other => {
            eprintln!("unknown asset command: {other}");
            print_asset_help();
            ExitCode::from(2)
        }
    }
}

fn run_image_command(args: &[String]) -> ExitCode {
    if args
        .first()
        .map(|value| matches!(value.as_str(), "--help" | "-h" | "help"))
        .unwrap_or(true)
    {
        print_image_help();
        return ExitCode::SUCCESS;
    }
    match args.first().map(String::as_str).unwrap_or("") {
        "metadata" => run_image_metadata(&args[1..]),
        "probe-request" => run_image_probe_request(&args[1..]),
        "codex-command" => run_image_codex_command(&args[1..]),
        other => {
            eprintln!("unknown image command: {other}");
            print_image_help();
            ExitCode::from(2)
        }
    }
}

fn run_image_metadata(args: &[String]) -> ExitCode {
    let Some(path) = args.first() else {
        eprintln!("image metadata requires filepath");
        return ExitCode::from(2);
    };
    let mut rest = args.iter().skip(1).cloned().collect::<Vec<_>>();
    let expected_width = match take_option(&mut rest, "--width") {
        Some(value) => match value.parse::<u32>() {
            Ok(value) => value,
            Err(_) => {
                eprintln!("--width requires an integer");
                return ExitCode::from(2);
            }
        },
        None => 0,
    };
    let expected_height = match take_option(&mut rest, "--height") {
        Some(value) => match value.parse::<u32>() {
            Ok(value) => value,
            Err(_) => {
                eprintln!("--height requires an integer");
                return ExitCode::from(2);
            }
        },
        None => 0,
    };
    let expected_format = take_option(&mut rest, "--format").unwrap_or_else(|| "PNG".into());
    if let Some(unknown) = rest.iter().find(|value| value.starts_with('-')) {
        eprintln!("unknown image metadata option: {unknown}");
        return ExitCode::from(2);
    }
    let bytes = match fs::read(path) {
        Ok(bytes) => bytes,
        Err(error) => {
            eprintln!("failed to read image: {error}");
            return ExitCode::from(1);
        }
    };
    print_json_result(Ok(check_png_metadata_bytes(
        &bytes,
        expected_width,
        expected_height,
        &expected_format,
    )))
}

fn run_image_probe_request(args: &[String]) -> ExitCode {
    let Some(repo_root) = repo_root_from_cwd() else {
        eprintln!("failed to locate standalone project root");
        return ExitCode::from(1);
    };
    let mut rest = args.to_vec();
    let root = take_option(&mut rest, "--root")
        .map(std::path::PathBuf::from)
        .unwrap_or(repo_root);
    let provider = take_option(&mut rest, "--provider");
    let prompt = take_option(&mut rest, "--prompt")
        .unwrap_or_else(|| "A tiny underwater explorer icon, clean game asset, no text.".into());
    let size = take_option(&mut rest, "--size").unwrap_or_else(|| "1024x1024".into());
    let quality = take_option(&mut rest, "--quality").unwrap_or_else(|| "high".into());
    let format = take_option(&mut rest, "--format").unwrap_or_else(|| "png".into());
    if let Some(unknown) = rest.iter().find(|value| value.starts_with('-')) {
        eprintln!("unknown image probe-request option: {unknown}");
        return ExitCode::from(2);
    }
    let result = AiConfigService::new(&root)
        .and_then(|service| service.load_or_default())
        .and_then(|config| image_settings_from_config(&config, provider.as_deref()))
        .and_then(|settings| {
            build_image_probe_request(&settings, &prompt, &size, &quality, &format)
        });
    print_json_result(result)
}

fn run_image_codex_command(args: &[String]) -> ExitCode {
    let mut rest = args.to_vec();
    let project_root = take_option(&mut rest, "--project-root")
        .map(std::path::PathBuf::from)
        .or_else(repo_root_from_cwd)
        .unwrap_or_else(|| std::path::PathBuf::from("."));
    let codex_home = take_option(&mut rest, "--codex-home");
    let cli_path = take_option(&mut rest, "--cli");
    if let Some(unknown) = rest.iter().find(|value| value.starts_with('-')) {
        eprintln!("unknown image codex-command option: {unknown}");
        return ExitCode::from(2);
    }
    if rest.is_empty() {
        eprintln!("image codex-command requires prompt");
        return ExitCode::from(2);
    }
    print_json_result(Ok(codex_image_command(
        &project_root,
        &rest.join(" "),
        codex_home.as_deref(),
        cli_path.as_deref(),
    )))
}

fn run_dev_command(args: &[String]) -> ExitCode {
    if args
        .first()
        .map(|value| matches!(value.as_str(), "--help" | "-h" | "help"))
        .unwrap_or(true)
    {
        print_dev_help();
        return ExitCode::SUCCESS;
    }
    match args.first().map(String::as_str).unwrap_or("") {
        "config-compile" => {
            if args.len() < 4 {
                eprintln!("dev config-compile requires schema_path tables_dir output_dir");
                return ExitCode::from(2);
            }
            print_json_result(compile_all_config(&args[1], &args[2], &args[3]))
        }
        "git" => {
            let mut rest = args.iter().skip(1).cloned().collect::<Vec<_>>();
            let work_dir = take_option(&mut rest, "--work-dir")
                .map(std::path::PathBuf::from)
                .unwrap_or_else(|| env::current_dir().unwrap_or_else(|_| ".".into()));
            if rest.is_empty() {
                eprintln!("dev git requires a git command");
                return ExitCode::from(2);
            }
            print_json_result(local_git_command_spec(&rest, work_dir))
        }
        "generate-tests" => {
            if args.len() < 3 {
                eprintln!("dev generate-tests requires plans_dir output_dir");
                return ExitCode::from(2);
            }
            print_json_result(run_test_generation_pipeline(&args[1], &args[2]))
        }
        other => {
            eprintln!("unknown dev command: {other}");
            print_dev_help();
            ExitCode::from(2)
        }
    }
}

fn run_project_command(args: &[String]) -> ExitCode {
    if args
        .first()
        .map(|value| matches!(value.as_str(), "--help" | "-h" | "help"))
        .unwrap_or(true)
    {
        print_project_help();
        return ExitCode::SUCCESS;
    }
    match args.first().map(String::as_str).unwrap_or("") {
        "scaffold" => {
            let Some(root) = args.get(1) else {
                eprintln!("project scaffold requires root");
                return ExitCode::from(2);
            };
            print_json_result(scaffold_project(root))
        }
        other => {
            eprintln!("unknown project command: {other}");
            print_project_help();
            ExitCode::from(2)
        }
    }
}

fn run_pipeline_command(args: &[String]) -> ExitCode {
    if args
        .first()
        .map(|value| matches!(value.as_str(), "--help" | "-h" | "help"))
        .unwrap_or(true)
    {
        print_pipeline_help();
        return ExitCode::SUCCESS;
    }
    match args.first().map(String::as_str).unwrap_or("") {
        "scaffold-step" => {
            let mut rest = args.iter().skip(1).cloned().collect::<Vec<_>>();
            let force = take_bool_flag(&mut rest, "--force");
            if rest.len() < 3 {
                eprintln!("pipeline scaffold-step requires project_root step name");
                return ExitCode::from(2);
            }
            let step = match rest[1].parse::<u32>() {
                Ok(value) => value,
                Err(_) => {
                    eprintln!("step must be an integer");
                    return ExitCode::from(2);
                }
            };
            print_json_result(scaffold_step(&rest[0], step, &rest[2], force))
        }
        "inspect-reports" => {
            let mut rest = args.iter().skip(1).cloned().collect::<Vec<_>>();
            let artifacts_dir = take_option(&mut rest, "--artifacts-dir")
                .map(std::path::PathBuf::from)
                .or_else(|| {
                    repo_root_from_cwd()
                        .map(|root| root.join("drafts/design_flow/outputs/artifacts"))
                })
                .unwrap_or_else(|| std::path::PathBuf::from("outputs/artifacts"));
            let step = match take_option(&mut rest, "--step") {
                Some(value) => match value.parse::<u32>() {
                    Ok(value) => Some(value),
                    Err(_) => {
                        eprintln!("--step requires an integer");
                        return ExitCode::from(2);
                    }
                },
                None => None,
            };
            let max_step = match take_option(&mut rest, "--max-step") {
                Some(value) => match value.parse::<u32>() {
                    Ok(value) => Some(value),
                    Err(_) => {
                        eprintln!("--max-step requires an integer");
                        return ExitCode::from(2);
                    }
                },
                None => None,
            };
            if let Some(unknown) = rest.iter().find(|value| value.starts_with('-')) {
                eprintln!("unknown pipeline inspect-reports option: {unknown}");
                return ExitCode::from(2);
            }
            print_json_result(inspect_pipeline_reports(artifacts_dir, step, max_step))
        }
        other => {
            eprintln!("unknown pipeline command: {other}");
            print_pipeline_help();
            ExitCode::from(2)
        }
    }
}

fn run_unity_command(args: &[String]) -> ExitCode {
    if args
        .first()
        .map(|value| matches!(value.as_str(), "--help" | "-h" | "help"))
        .unwrap_or(true)
    {
        print_unity_help();
        return ExitCode::SUCCESS;
    }
    match args.first().map(String::as_str).unwrap_or("") {
        "error-logger" => {
            if args.len() < 3 {
                eprintln!("unity error-logger requires source_dir output_dir");
                return ExitCode::from(2);
            }
            print_json_result(run_error_logger_pipeline(&args[1], &args[2]))
        }
        "perf" => {
            if args.len() < 3 {
                eprintln!("unity perf requires source_dir output_dir");
                return ExitCode::from(2);
            }
            print_json_result(run_perf_pipeline(&args[1], &args[2]))
        }
        "ui-state" => {
            if args.len() < 3 {
                eprintln!("unity ui-state requires graph_path output_dir");
                return ExitCode::from(2);
            }
            print_json_result(generate_ui_state_artifacts(&args[1], &args[2]))
        }
        other => {
            eprintln!("unknown unity command: {other}");
            print_unity_help();
            ExitCode::from(2)
        }
    }
}

fn run_validate_command(args: &[String]) -> ExitCode {
    if args
        .first()
        .map(|value| matches!(value.as_str(), "--help" | "-h" | "help"))
        .unwrap_or(true)
    {
        print_validate_help();
        return ExitCode::SUCCESS;
    }
    match args.first().map(String::as_str).unwrap_or("") {
        "environment" => {
            let Some(config_path) = args.get(1) else {
                eprintln!("validate environment requires config_path");
                return ExitCode::from(2);
            };
            match check_environment_config(config_path) {
                Ok(report) => {
                    let ok = report.ok;
                    println!(
                        "{}",
                        serde_json::to_string_pretty(&report).unwrap_or_default()
                    );
                    if ok {
                        ExitCode::SUCCESS
                    } else {
                        ExitCode::from(1)
                    }
                }
                Err(error) => {
                    eprintln!("{error}");
                    ExitCode::from(1)
                }
            }
        }
        "compile-plan" => {
            let mut rest = args.iter().skip(1).cloned().collect::<Vec<_>>();
            let work_dir = take_option(&mut rest, "--work-dir")
                .map(std::path::PathBuf::from)
                .unwrap_or_else(|| env::current_dir().unwrap_or_else(|_| ".".into()));
            let command = take_option(&mut rest, "--command");
            if let Some(unknown) = rest.iter().find(|value| value.starts_with('-')) {
                eprintln!("unknown validate compile-plan option: {unknown}");
                return ExitCode::from(2);
            }
            print_json_result(Ok(compile_check_plan(&work_dir, command.as_deref())))
        }
        "compile-result" => {
            let mut rest = args.iter().skip(1).cloned().collect::<Vec<_>>();
            let exit_code = match take_option(&mut rest, "--exit-code") {
                Some(value) => match value.parse::<i32>() {
                    Ok(value) => value,
                    Err(_) => {
                        eprintln!("--exit-code requires an integer");
                        return ExitCode::from(2);
                    }
                },
                None => 0,
            };
            let stdout = take_option(&mut rest, "--stdout").unwrap_or_default();
            let stderr = take_option(&mut rest, "--stderr").unwrap_or_default();
            print_json_result(Ok(evaluate_compile_output(exit_code, &stdout, &stderr)))
        }
        "config" => {
            if args.len() < 3 {
                eprintln!("validate config requires schema_path tables_dir");
                return ExitCode::from(2);
            }
            match validate_config_tables(&args[1], &args[2]) {
                Ok(report) => {
                    let passed = report.passed();
                    println!(
                        "{}",
                        serde_json::to_string_pretty(&report).unwrap_or_default()
                    );
                    if passed {
                        ExitCode::SUCCESS
                    } else {
                        ExitCode::from(1)
                    }
                }
                Err(error) => {
                    eprintln!("{error}");
                    ExitCode::from(1)
                }
            }
        }
        "context" => {
            let Some(path) = args.get(1) else {
                eprintln!("validate context requires CONTEXT.md path");
                return ExitCode::from(2);
            };
            match lint_context_file(path) {
                Ok(report) => {
                    let valid = report.valid;
                    println!(
                        "{}",
                        serde_json::to_string_pretty(&report).unwrap_or_default()
                    );
                    if valid {
                        ExitCode::SUCCESS
                    } else {
                        ExitCode::from(1)
                    }
                }
                Err(error) => {
                    eprintln!("{error}");
                    ExitCode::from(1)
                }
            }
        }
        "contract" => {
            let mut rest = args.iter().skip(1).cloned().collect::<Vec<_>>();
            let report_path = take_option(&mut rest, "--report").map(std::path::PathBuf::from);
            if rest.len() < 2 {
                eprintln!("validate contract requires contract_path schema_path");
                return ExitCode::from(2);
            }
            match validate_contract_file_report(&rest[0], &rest[1], report_path) {
                Ok(report) => {
                    let valid = report.valid;
                    println!(
                        "{}",
                        serde_json::to_string_pretty(&report).unwrap_or_default()
                    );
                    if valid {
                        ExitCode::SUCCESS
                    } else {
                        ExitCode::from(1)
                    }
                }
                Err(error) => {
                    eprintln!("{error}");
                    ExitCode::from(1)
                }
            }
        }
        "output" => {
            let mut rest = args.iter().skip(1).cloned().collect::<Vec<_>>();
            let expected_format =
                take_option(&mut rest, "--format").unwrap_or_else(|| "markdown".to_string());
            let output_name =
                take_option(&mut rest, "--name").unwrap_or_else(|| "Agent output".to_string());
            let required = take_repeated_options(&mut rest, "--required");
            let text = take_option(&mut rest, "--text").unwrap_or_else(|| rest.join(" "));
            if text.trim().is_empty() {
                eprintln!("validate output requires --text or positional text");
                return ExitCode::from(2);
            }
            let report = validate_agent_output(&text, &expected_format, &required, &output_name);
            let valid = report.valid;
            println!(
                "{}",
                serde_json::to_string_pretty(&report).unwrap_or_default()
            );
            if valid {
                ExitCode::SUCCESS
            } else {
                ExitCode::from(1)
            }
        }
        "pipeline-quality" => {
            let mut rest = args.iter().skip(1).cloned().collect::<Vec<_>>();
            let artifacts_dir = take_option(&mut rest, "--artifacts-dir")
                .map(std::path::PathBuf::from)
                .unwrap_or_else(|| std::path::PathBuf::from("outputs/artifacts"));
            let check = take_option(&mut rest, "--check");
            if let Some(unknown) = rest.iter().find(|value| value.starts_with('-')) {
                eprintln!("unknown validate pipeline-quality option: {unknown}");
                return ExitCode::from(2);
            }
            let report = if check.as_deref() == Some("plan-002") {
                check_pipeline_plan_002(&artifacts_dir)
            } else {
                collect_pipeline_quality_metrics(&artifacts_dir)
            };
            let passed = report
                .get("checks")
                .and_then(|checks| checks.get("plan-002"))
                .and_then(|check| check.get("passed"))
                .and_then(serde_json::Value::as_bool)
                .unwrap_or(true);
            println!(
                "{}",
                serde_json::to_string_pretty(&report).unwrap_or_default()
            );
            if passed {
                ExitCode::SUCCESS
            } else {
                ExitCode::from(1)
            }
        }
        "design-semantic-quality" => {
            let mut rest = args.iter().skip(1).cloned().collect::<Vec<_>>();
            let artifacts_root = take_option(&mut rest, "--artifacts-root")
                .or_else(|| take_option(&mut rest, "--artifacts-dir"))
                .map(std::path::PathBuf::from)
                .unwrap_or_else(|| std::path::PathBuf::from("outputs/artifacts"));
            let output_json = take_option(&mut rest, "--output-json").map(std::path::PathBuf::from);
            let output_md = take_option(&mut rest, "--output-md").map(std::path::PathBuf::from);
            if let Some(unknown) = rest.iter().find(|value| value.starts_with('-')) {
                eprintln!("unknown validate design-semantic-quality option: {unknown}");
                return ExitCode::from(2);
            }
            match collect_design_semantic_quality(&artifacts_root).and_then(|report| {
                write_design_semantic_quality_outputs(&report, output_json, output_md)
                    .map(|()| report)
            }) {
                Ok(report) => {
                    let blocked = report
                        .get("blocking_issues")
                        .and_then(serde_json::Value::as_array)
                        .is_some_and(|items| !items.is_empty());
                    println!(
                        "{}",
                        serde_json::to_string_pretty(&report).unwrap_or_default()
                    );
                    if blocked {
                        ExitCode::from(1)
                    } else {
                        ExitCode::SUCCESS
                    }
                }
                Err(error) => {
                    eprintln!("{error}");
                    ExitCode::from(1)
                }
            }
        }
        other => {
            eprintln!("unknown validate command: {other}");
            print_validate_help();
            ExitCode::from(2)
        }
    }
}

fn run_migrate_command(args: &[String]) -> ExitCode {
    if args
        .first()
        .map(|value| matches!(value.as_str(), "--help" | "-h" | "help"))
        .unwrap_or(true)
    {
        print_migrate_help();
        return ExitCode::SUCCESS;
    }
    match args.first().map(String::as_str).unwrap_or("") {
        "legacy" => run_migrate_legacy_command(&args[1..]),
        "eo-save-id" => run_migrate_eo_save_id_command(&args[1..]),
        "design-projects" => run_migrate_design_projects_command(&args[1..]),
        other => {
            eprintln!("unknown migrate command: {other}");
            print_migrate_help();
            ExitCode::from(2)
        }
    }
}

fn run_migrate_legacy_command(args: &[String]) -> ExitCode {
    let mut rest = args.to_vec();
    let project_root = take_option(&mut rest, "--project-root")
        .map(std::path::PathBuf::from)
        .or_else(repo_root_from_cwd)
        .unwrap_or_else(|| env::current_dir().unwrap_or_else(|_| ".".into()));
    let apply = take_bool_flag(&mut rest, "--apply");
    let dry_run = take_bool_flag(&mut rest, "--dry-run");
    let source =
        take_option(&mut rest, "--source").or_else(|| (!rest.is_empty()).then(|| rest.remove(0)));
    let target =
        take_option(&mut rest, "--target").or_else(|| (!rest.is_empty()).then(|| rest.remove(0)));
    if !rest.is_empty() {
        eprintln!("unknown migrate legacy arguments: {}", rest.join(" "));
        return ExitCode::from(2);
    }
    let Some(source) = source else {
        eprintln!("migrate legacy requires --source");
        return ExitCode::from(2);
    };
    let Some(target) = target else {
        eprintln!("migrate legacy requires --target");
        return ExitCode::from(2);
    };
    print_json_result(run_legacy_migration(
        project_root,
        std::path::PathBuf::from(source),
        std::path::PathBuf::from(target),
        apply && !dry_run,
    ))
}

fn run_migrate_eo_save_id_command(args: &[String]) -> ExitCode {
    let mut rest = args.to_vec();
    let project_root = take_option(&mut rest, "--project-root")
        .map(std::path::PathBuf::from)
        .or_else(repo_root_from_cwd)
        .unwrap_or_else(|| env::current_dir().unwrap_or_else(|_| ".".into()));
    let save_root = take_option(&mut rest, "--save-root")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| project_root.join("save"));
    let report = take_option(&mut rest, "--report").map(std::path::PathBuf::from);
    let apply = take_bool_flag(&mut rest, "--apply");
    let dry_run = take_bool_flag(&mut rest, "--dry-run");
    if !rest.is_empty() {
        eprintln!("unknown migrate eo-save-id arguments: {}", rest.join(" "));
        return ExitCode::from(2);
    }
    print_json_result(migrate_execution_object_save_ids(
        save_root,
        apply && !dry_run,
        report,
    ))
}

fn run_migrate_design_projects_command(args: &[String]) -> ExitCode {
    let mut rest = args.to_vec();
    let project_root = take_option(&mut rest, "--project-root")
        .map(std::path::PathBuf::from)
        .or_else(repo_root_from_cwd)
        .unwrap_or_else(|| env::current_dir().unwrap_or_else(|_| ".".into()));
    let workspace_projects_dir =
        take_option(&mut rest, "--workspace-projects-dir").map(std::path::PathBuf::from);
    let store_path = take_option(&mut rest, "--store-path")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| {
            project_root.join("drafts/design_flow/outputs/execution_objects/execution_objects.json")
        });
    let backup = !take_bool_flag(&mut rest, "--no-backup");
    let delete_originals = take_bool_flag(&mut rest, "--delete-originals");
    let apply = take_bool_flag(&mut rest, "--apply");
    let dry_flag = take_bool_flag(&mut rest, "--dry-run");
    if !rest.is_empty() {
        eprintln!(
            "unknown migrate design-projects arguments: {}",
            rest.join(" ")
        );
        return ExitCode::from(2);
    }
    print_json_result(migrate_design_projects_to_execution_objects(
        project_root,
        workspace_projects_dir,
        store_path,
        backup,
        delete_originals,
        !apply || dry_flag,
    ))
}

fn run_schema_command(args: &[String]) -> ExitCode {
    if args
        .first()
        .map(|value| matches!(value.as_str(), "--help" | "-h" | "help"))
        .unwrap_or(true)
    {
        print_schema_help();
        return ExitCode::SUCCESS;
    }
    match args.first().map(String::as_str).unwrap_or("") {
        "migrate" => {
            let mut rest = args.iter().skip(1).cloned().collect::<Vec<_>>();
            let input = take_option(&mut rest, "--input")
                .or_else(|| (!rest.is_empty()).then(|| rest.remove(0)));
            let schema = take_option(&mut rest, "--schema")
                .or_else(|| (!rest.is_empty()).then(|| rest.remove(0)));
            let output = take_option(&mut rest, "--output")
                .or_else(|| (!rest.is_empty()).then(|| rest.remove(0)));
            if !rest.is_empty() {
                eprintln!("unknown schema migrate arguments: {}", rest.join(" "));
                return ExitCode::from(2);
            }
            let Some(input) = input else {
                eprintln!("schema migrate requires --input");
                return ExitCode::from(2);
            };
            let Some(schema) = schema else {
                eprintln!("schema migrate requires --schema");
                return ExitCode::from(2);
            };
            let Some(output) = output else {
                eprintln!("schema migrate requires --output");
                return ExitCode::from(2);
            };
            print_json_result(migrate_structured_schema(input, schema, output))
        }
        other => {
            eprintln!("unknown schema command: {other}");
            print_schema_help();
            ExitCode::from(2)
        }
    }
}

fn run_governance_command(args: &[String]) -> ExitCode {
    if args
        .first()
        .map(|value| matches!(value.as_str(), "--help" | "-h" | "help"))
        .unwrap_or(true)
    {
        print_governance_help();
        return ExitCode::SUCCESS;
    }
    match args.first().map(String::as_str).unwrap_or("") {
        "hardcoded-paths" => {
            let mut rest = args.iter().skip(1).cloned().collect::<Vec<_>>();
            let root = take_option(&mut rest, "--root")
                .map(std::path::PathBuf::from)
                .or_else(repo_root_from_cwd)
                .unwrap_or_else(|| env::current_dir().unwrap_or_else(|_| ".".into()));
            let allow_docs = take_bool_flag(&mut rest, "--allow-docs");
            if !rest.is_empty() {
                eprintln!(
                    "unknown governance hardcoded-paths arguments: {}",
                    rest.join(" ")
                );
                return ExitCode::from(2);
            }
            match scan_hardcoded_paths(root, allow_docs) {
                Ok(report) => {
                    let passed = report.status == "passed";
                    println!(
                        "{}",
                        serde_json::to_string_pretty(&report).unwrap_or_default()
                    );
                    if passed {
                        ExitCode::SUCCESS
                    } else {
                        ExitCode::from(1)
                    }
                }
                Err(error) => {
                    eprintln!("{error}");
                    ExitCode::from(1)
                }
            }
        }
        other => {
            eprintln!("unknown governance command: {other}");
            print_governance_help();
            ExitCode::from(2)
        }
    }
}

fn run_design_command(args: &[String]) -> ExitCode {
    if args
        .first()
        .map(|value| matches!(value.as_str(), "--help" | "-h" | "help"))
        .unwrap_or(true)
    {
        print_design_help();
        return ExitCode::SUCCESS;
    }
    match args.first().map(String::as_str).unwrap_or("") {
        "export-concept-package" => {
            let mut rest = args.iter().skip(1).cloned().collect::<Vec<_>>();
            let no_workspace_mirror = take_bool_flag(&mut rest, "--no-workspace-mirror");
            let workspace_mirror =
                take_option(&mut rest, "--workspace-mirror").map(std::path::PathBuf::from);
            let project_state = take_option(&mut rest, "--project-state")
                .or_else(|| (!rest.is_empty()).then(|| rest.remove(0)));
            let target_dir = take_option(&mut rest, "--target-dir")
                .or_else(|| (!rest.is_empty()).then(|| rest.remove(0)));
            if !rest.is_empty() {
                eprintln!(
                    "unknown design export-concept-package arguments: {}",
                    rest.join(" ")
                );
                return ExitCode::from(2);
            }
            let Some(project_state) = project_state else {
                eprintln!("design export-concept-package requires --project-state");
                return ExitCode::from(2);
            };
            let Some(target_dir) = target_dir else {
                eprintln!("design export-concept-package requires --target-dir");
                return ExitCode::from(2);
            };
            let mirror = if no_workspace_mirror {
                None
            } else {
                workspace_mirror
            };
            print_json_result(export_design_concept_package(
                std::path::PathBuf::from(project_state),
                std::path::PathBuf::from(target_dir),
                mirror,
            ))
        }
        other => {
            eprintln!("unknown design command: {other}");
            print_design_help();
            ExitCode::from(2)
        }
    }
}

fn run_iteration_command(args: &[String]) -> ExitCode {
    if args
        .first()
        .map(|value| matches!(value.as_str(), "--help" | "-h" | "help"))
        .unwrap_or(true)
    {
        print_iteration_help();
        return ExitCode::SUCCESS;
    }
    match args.first().map(String::as_str).unwrap_or("") {
        "parse" => {
            let mut rest = args.iter().skip(1).cloned().collect::<Vec<_>>();
            let spec = take_option(&mut rest, "--spec")
                .or_else(|| (!rest.is_empty()).then(|| rest.remove(0)));
            if !rest.is_empty() {
                eprintln!("unknown iteration parse arguments: {}", rest.join(" "));
                return ExitCode::from(2);
            }
            let Some(spec) = spec else {
                eprintln!("iteration parse requires --spec");
                return ExitCode::from(2);
            };
            match parse_iteration_spec(spec) {
                Ok(spec) => {
                    let valid = spec.valid();
                    match spec.to_value() {
                        Ok(value) => println!(
                            "{}",
                            serde_json::to_string_pretty(&value).unwrap_or_default()
                        ),
                        Err(error) => {
                            eprintln!("{error}");
                            return ExitCode::from(1);
                        }
                    }
                    if valid {
                        ExitCode::SUCCESS
                    } else {
                        ExitCode::from(1)
                    }
                }
                Err(error) => {
                    eprintln!("{error}");
                    ExitCode::from(1)
                }
            }
        }
        "plan" => {
            let mut rest = args.iter().skip(1).cloned().collect::<Vec<_>>();
            let output = take_option(&mut rest, "--output").map(std::path::PathBuf::from);
            let spec = take_option(&mut rest, "--spec")
                .or_else(|| (!rest.is_empty()).then(|| rest.remove(0)));
            if !rest.is_empty() {
                eprintln!("unknown iteration plan arguments: {}", rest.join(" "));
                return ExitCode::from(2);
            }
            let Some(spec) = spec else {
                eprintln!("iteration plan requires --spec");
                return ExitCode::from(2);
            };
            match parse_iteration_spec(&spec).map(|spec| build_delta_execution_plan(&spec)) {
                Ok(plan) => {
                    let text = serde_json::to_string_pretty(&plan).unwrap_or_default() + "\n";
                    if let Some(output) = output {
                        if let Some(parent) = output.parent()
                            && let Err(error) = fs::create_dir_all(parent)
                        {
                            eprintln!("{error}");
                            return ExitCode::from(1);
                        }
                        if let Err(error) = fs::write(output, &text) {
                            eprintln!("{error}");
                            return ExitCode::from(1);
                        }
                    }
                    print!("{text}");
                    if plan.status == "ready" {
                        ExitCode::SUCCESS
                    } else {
                        ExitCode::from(1)
                    }
                }
                Err(error) => {
                    eprintln!("{error}");
                    ExitCode::from(1)
                }
            }
        }
        "inherit" => {
            let mut rest = args.iter().skip(1).cloned().collect::<Vec<_>>();
            let parent_workspace =
                take_option(&mut rest, "--parent-workspace").map(std::path::PathBuf::from);
            let target_workspace =
                take_option(&mut rest, "--target-workspace").map(std::path::PathBuf::from);
            let parent_version =
                take_option(&mut rest, "--parent-version").unwrap_or_else(|| "current".to_string());
            let parent_save_id = take_option(&mut rest, "--parent-save-id").unwrap_or_default();
            let plan_path = take_option(&mut rest, "--plan").map(std::path::PathBuf::from);
            if !rest.is_empty() {
                eprintln!("unknown iteration inherit arguments: {}", rest.join(" "));
                return ExitCode::from(2);
            }
            let Some(parent_workspace) = parent_workspace else {
                eprintln!("iteration inherit requires --parent-workspace");
                return ExitCode::from(2);
            };
            let Some(target_workspace) = target_workspace else {
                eprintln!("iteration inherit requires --target-workspace");
                return ExitCode::from(2);
            };
            let Some(plan_path) = plan_path else {
                eprintln!("iteration inherit requires --plan");
                return ExitCode::from(2);
            };
            let plan = match read_iteration_delta_plan(&plan_path) {
                Ok(plan) => plan,
                Err(error) => {
                    eprintln!("{error}");
                    return ExitCode::from(1);
                }
            };
            print_json_result(inherit_skipped_artifacts(
                parent_workspace,
                target_workspace,
                &parent_version,
                &parent_save_id,
                &plan,
            ))
        }
        "prepare" => {
            let mut rest = args.iter().skip(1).cloned().collect::<Vec<_>>();
            let dry_run = take_bool_flag(&mut rest, "--dry-run");
            let project_root = take_option(&mut rest, "--project-root")
                .map(std::path::PathBuf::from)
                .or_else(repo_root_from_cwd)
                .unwrap_or_else(|| env::current_dir().unwrap_or_else(|_| ".".into()));
            let session_id = take_option(&mut rest, "--session-id")
                .unwrap_or_else(|| "cli_iteration".to_string());
            let state = match take_option(&mut rest, "--state") {
                Some(path) => match read_project_state_file(path) {
                    Ok(state) => state,
                    Err(error) => {
                        eprintln!("{error}");
                        return ExitCode::from(1);
                    }
                },
                None => ProjectState::empty(),
            };
            let spec = take_option(&mut rest, "--spec")
                .or_else(|| (!rest.is_empty()).then(|| rest.remove(0)));
            if !rest.is_empty() {
                eprintln!("unknown iteration prepare arguments: {}", rest.join(" "));
                return ExitCode::from(2);
            }
            let Some(spec) = spec else {
                eprintln!("iteration prepare requires --spec");
                return ExitCode::from(2);
            };
            print_json_result(prepare_iteration(IterationPrepareRequest {
                project_root,
                session_id,
                spec_path: std::path::PathBuf::from(spec),
                state,
                dry_run,
            }))
        }
        "resume" => {
            let mut rest = args.iter().skip(1).cloned().collect::<Vec<_>>();
            let plan = take_option(&mut rest, "--plan")
                .or_else(|| (!rest.is_empty()).then(|| rest.remove(0)));
            if !rest.is_empty() {
                eprintln!("unknown iteration resume arguments: {}", rest.join(" "));
                return ExitCode::from(2);
            }
            let Some(plan) = plan else {
                eprintln!("iteration resume requires --plan");
                return ExitCode::from(2);
            };
            print_json_result(summarize_iteration_resume_plan(plan))
        }
        other => {
            eprintln!("unknown iteration command: {other}");
            print_iteration_help();
            ExitCode::from(2)
        }
    }
}

fn read_iteration_delta_plan(path: &Path) -> AdmResult<IterationDeltaPlan> {
    let text = fs::read_to_string(path)?;
    serde_json::from_str(&text).map_err(|error| {
        adm_new_foundation::AdmError::new(format!("invalid iteration plan JSON: {error}"))
    })
}

fn read_project_state_file(path: impl AsRef<Path>) -> AdmResult<ProjectState> {
    let text = fs::read_to_string(path)?;
    serde_json::from_str(&text).map_err(|error| {
        adm_new_foundation::AdmError::new(format!("invalid project state JSON: {error}"))
    })
}

fn take_bool_flag(values: &mut Vec<String>, flag: &str) -> bool {
    if let Some(index) = values.iter().position(|value| value == flag) {
        values.remove(index);
        true
    } else {
        false
    }
}

fn take_repeated_options(values: &mut Vec<String>, flag: &str) -> Vec<String> {
    let mut results = Vec::new();
    while let Some(value) = take_option(values, flag) {
        results.push(value);
    }
    results
}

fn run_design_sync_audit_command(args: &[String]) -> ExitCode {
    let mut rest = args.to_vec();
    if take_bool_flag(&mut rest, "--help") || take_bool_flag(&mut rest, "-h") {
        println!("adm-new-cli design-sync-audit [--python-root DIR] [--json]");
        return ExitCode::SUCCESS;
    }
    let output_json = take_bool_flag(&mut rest, "--json");
    let Some(repo_root) = repo_root_from_cwd() else {
        eprintln!("failed to locate standalone project root");
        return ExitCode::from(1);
    };
    let python_root = match take_option(&mut rest, "--python-root")
        .map(PathBuf::from)
        .or_else(|| default_python_audit_root(&repo_root))
    {
        Some(root) => root,
        None => {
            eprintln!("failed to locate python audit root; pass --python-root DIR");
            return ExitCode::from(2);
        }
    };
    if !rest.is_empty() {
        eprintln!("unexpected design-sync-audit args: {}", rest.join(" "));
        return ExitCode::from(2);
    }
    match design_sync_audit_report(&repo_root, &python_root) {
        Ok(report) => {
            if output_json {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&report).unwrap_or_default()
                );
            } else {
                let rendered = render_design_sync_audit(&report);
                print!("{rendered}");
                if let Err(error) = write_gate_report(&repo_root, "design-sync-audit", &rendered) {
                    eprintln!("failed to write design sync audit report: {error}");
                    return ExitCode::from(1);
                }
            }
            design_sync_audit_exit_code(&report.status)
        }
        Err(error) => {
            eprintln!("design-sync-audit failed: {error}");
            ExitCode::from(1)
        }
    }
}

fn design_sync_audit_exit_code(status: &str) -> ExitCode {
    if status == "failed" {
        ExitCode::from(1)
    } else {
        ExitCode::SUCCESS
    }
}

fn default_python_audit_root(repo_root: &Path) -> Option<PathBuf> {
    let parent = repo_root.parent()?.to_path_buf();
    if parent.join("AI_README.md").is_file() || parent.join("AGENTS.md").is_file() {
        Some(parent)
    } else {
        None
    }
}

fn run_memory_command(_args: &[String]) -> ExitCode {
    eprintln!("{LEGACY_MEMORY_COMMAND_DEPRECATED}");
    ExitCode::from(2)
}

fn run_gate<F>(report_slug: &str, build_report: F) -> ExitCode
where
    F: FnOnce(&Path) -> AdmResult<GateReport>,
{
    let Some(repo_root) = repo_root_from_cwd() else {
        eprintln!("failed to locate standalone project root");
        return ExitCode::from(1);
    };
    match build_report(&repo_root) {
        Ok(report) => {
            let rendered = report.render();
            print!("{rendered}");
            if let Err(error) = write_gate_report(&repo_root, report_slug, &rendered) {
                eprintln!("failed to write gate report: {error}");
                return ExitCode::from(1);
            }
            if report.passed() {
                ExitCode::SUCCESS
            } else {
                ExitCode::from(1)
            }
        }
        Err(error) => {
            eprintln!("{report_slug} gate failed: {error}");
            ExitCode::from(1)
        }
    }
}

fn add_cargo_test_result(repo_root: &Path, report: &mut GateReport) {
    let output = Command::new("cargo")
        .arg("test")
        .arg("--workspace")
        .arg("--quiet")
        .current_dir(repo_root)
        .output();
    match output {
        Ok(output) => {
            report.add_row("cargo_test_workspace_status", output.status.to_string());
            report.add_row(
                "cargo_test_workspace_stdout_bytes",
                output.stdout.len().to_string(),
            );
            report.add_row(
                "cargo_test_workspace_stderr_bytes",
                output.stderr.len().to_string(),
            );
            if !output.status.success() {
                report.add_blocker("cargo_test_workspace_failed");
            }
        }
        Err(error) => {
            report.add_blocker(format!("cargo_test_workspace_unavailable:{error}"));
        }
    }
}

fn add_process_result(
    report: &mut GateReport,
    key: &str,
    program: &str,
    args: &[&str],
    cwd: &Path,
) {
    let output = Command::new(program).args(args).current_dir(cwd).output();
    match output {
        Ok(output) => {
            report.add_row(format!("{key}_status"), output.status.to_string());
            report.add_row(
                format!("{key}_stdout_bytes"),
                output.stdout.len().to_string(),
            );
            report.add_row(
                format!("{key}_stderr_bytes"),
                output.stderr.len().to_string(),
            );
            if !output.status.success() {
                report.add_blocker(format!("{key}_failed"));
            }
        }
        Err(error) => report.add_blocker(format!("{key}_unavailable:{error}")),
    }
}

fn write_gate_report(repo_root: &Path, report_slug: &str, rendered: &str) -> std::io::Result<()> {
    let report_dir = repo_root.join("gates");
    fs::create_dir_all(&report_dir)?;
    fs::write(report_dir.join(format!("{report_slug}-gate.adm")), rendered)
}

fn repo_root_from_cwd() -> Option<std::path::PathBuf> {
    if let Some(root) = PROJECT_ROOT_OVERRIDE.get() {
        return Some(root.clone());
    }
    env::current_dir().ok().and_then(|cwd| find_repo_root(&cwd))
}

fn print_json_result<T: serde::Serialize>(result: AdmResult<T>) -> ExitCode {
    match result {
        Ok(value) => {
            println!(
                "{}",
                serde_json::to_string_pretty(&value).unwrap_or_default()
            );
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("{error}");
            ExitCode::from(1)
        }
    }
}

fn print_help() {
    println!("usage: adm-new-cli [--project-root DIR] <command> [args]");
    println!("adm-new-cli commands:");
    println!("  doctor      verify the standalone source/resource/portable boundary");
    println!("  standalone-boundary-gate verify the relocatable independent-project contract");
    println!(
        "  design-sync-audit [--python-root DIR] [--json] compare Python/Rust design resources"
    );
    println!(
        "  parity-gate verify required Rust parity checks and run cargo test --workspace --quiet"
    );
    println!("  ui-gate     verify Web e2e flows and real browser screenshot evidence");
    println!("  package-gate verify packaging success and blocked manifest evidence");
    println!("  validation-gate verify A29 validator tool and CLI marker coverage");
    println!(
        "  iteration-gate verify A30 iteration parser, scheduler, inheritor, and CLI coverage"
    );
    println!("  ui-shell-gate verify A31 desktop shell, theme, startup, and Web shell coverage");
    println!("  release-gate verify current Rust/Web/resource/boundary/package release checks");
    println!("  package     run current project packaging readiness and write package outputs");
    println!("  dist        build or verify desktop distribution bundles");
    println!("  asset       run asset production helpers");
    println!("  image       inspect image API/Codex generation helpers");
    println!("  dev         run development codegen and safe git helpers");
    println!("  project     scaffold project-level development files");
    println!("  pipeline    scaffold pipeline development steps");
    println!("  unity       generate Unity helper code artifacts");
    println!("  validate    run environment and compile validation helpers");
    println!("  migrate     run legacy, save-id, and design-project migrations");
    println!("  schema      migrate structured schema payloads");
    println!("  governance  run repository governance inspections");
    println!("  design      export design handoff artifacts");
    println!("  iteration   parse, plan, inherit, prepare, and resume iteration delta runs");
    println!("  patch       analyze, apply, validate, promote quick patch records");
    println!("  sdk         manage SDK knowledge specs and approved prompt context");
}

fn print_patch_help() {
    println!("adm-new-cli patch commands:");
    println!("  patch [--root DIR] [--project-root DIR] analyze <request>");
    println!("  patch [--root DIR] [--status STATUS] list");
    println!("  patch [--root DIR] show <patch_id>");
    println!("  patch [--root DIR] [--project-root DIR] validate <patch_id>");
    println!("  patch [--root DIR] [--project-root DIR] apply <patch_id>");
    println!("  patch [--root DIR] promote <patch_id> <iteration_spec>");
}

fn print_sdk_help() {
    println!("adm-new-cli sdk commands:");
    println!("  sdk [--data-root DIR|--root OVERLAY_DIR] init");
    println!("  sdk [--data-root DIR|--root OVERLAY_DIR] list");
    println!("  sdk [--data-root DIR|--root OVERLAY_DIR] show <sdk_id>");
    println!("  sdk [--data-root DIR|--root OVERLAY_DIR] add <name> [--url URL]");
    println!(
        "  sdk [--data-root DIR|--root OVERLAY_DIR] review <sdk_id> <draft|pending_review|approved|rejected>"
    );
    println!("  sdk [--data-root DIR|--root OVERLAY_DIR] context");
    println!("  sdk [--data-root DIR|--root OVERLAY_DIR] sync <local_html_file>");
    println!("  seed: <project-root>/knowledge/sdks (read-only, lower priority)");
    println!("  overlay: <data-root>/knowledge/sdks (writable, higher priority)");
}

fn print_package_help() {
    println!("adm-new-cli package commands:");
    println!("  package [run] [--artifacts-dir DIR] [--outputs-dir DIR]");
}

fn print_dist_help() {
    println!("adm-new-cli dist commands:");
    println!("  dist build [--execute]");
    println!("  dist verify-bundle <bundle_dir> [--exe NAME] [--min-bytes N] [--require REL]...");
}

fn print_asset_help() {
    println!("adm-new-cli asset commands:");
    println!("  asset audio-placeholder [--output-dir DIR] [--filename NAME]");
    println!(
        "  asset sfx-placeholder <prompt> [--output-dir DIR] [--duration SECONDS] [--filename NAME]"
    );
    println!(
        "  asset sprite-slice <sheet_path> <output_dir> [--grid COLSxROWS] [--cell-size WxH] [--gap N]"
    );
    println!("  asset sprite-atlas <output_dir> <atlas_name> --frame PNG... [--cell-size WxH]");
    println!("  asset localization <source_dir> <output_dir>");
}

fn print_image_help() {
    println!("adm-new-cli image commands:");
    println!("  image metadata <filepath> [--width N] [--height N] [--format FMT]");
    println!(
        "  image probe-request [--root DIR] [--provider NAME] [--prompt TEXT] [--size S] [--quality Q] [--format F]"
    );
    println!("  image codex-command <prompt> [--project-root DIR] [--codex-home DIR] [--cli PATH]");
}

fn print_dev_help() {
    println!("adm-new-cli dev commands:");
    println!("  dev config-compile <schema_path> <tables_dir> <output_dir>");
    println!("  dev git [--work-dir DIR] git <init|add|commit|tag|status|log> ...");
    println!("  dev generate-tests <plans_dir> <output_dir>");
}

fn print_project_help() {
    println!("adm-new-cli project commands:");
    println!("  project scaffold <root>");
}

fn print_pipeline_help() {
    println!("adm-new-cli pipeline commands:");
    println!("  pipeline scaffold-step <project_root> <step> <name> [--force]");
    println!("  pipeline inspect-reports [--artifacts-dir DIR] [--step N|--max-step N]");
}

fn print_unity_help() {
    println!("adm-new-cli unity commands:");
    println!("  unity error-logger <source_dir> <output_dir>");
    println!("  unity perf <source_dir> <output_dir>");
    println!("  unity ui-state <graph_path> <output_dir>");
}

fn print_validate_help() {
    println!("adm-new-cli validate commands:");
    println!("  validate environment <config_path>");
    println!("  validate compile-plan [--work-dir DIR] [--command CMD]");
    println!("  validate compile-result [--exit-code N] [--stdout TEXT] [--stderr TEXT]");
    println!("  validate config <schema_path> <tables_dir>");
    println!("  validate context <CONTEXT.md>");
    println!("  validate contract <contract_path> <schema_path> [--report FILE]");
    println!("  validate output [--format markdown|json] [--required KEY]... --text TEXT");
    println!("  validate pipeline-quality [--artifacts-dir DIR] [--check plan-002]");
    println!(
        "  validate design-semantic-quality [--artifacts-root DIR] [--output-json FILE] [--output-md FILE]"
    );
}

fn print_migrate_help() {
    println!("adm-new-cli migrate commands:");
    println!("  migrate legacy --source SRC --target DST [--project-root DIR] [--apply|--dry-run]");
    println!("  migrate eo-save-id [--project-root DIR] [--save-root DIR] [--report MD] [--apply]");
    println!(
        "  migrate design-projects [--project-root DIR] [--workspace-projects-dir DIR] [--store-path JSON] [--apply] [--no-backup] [--delete-originals]"
    );
}

fn print_schema_help() {
    println!("adm-new-cli schema commands:");
    println!("  schema migrate --input FILE --schema FILE --output FILE");
}

fn print_governance_help() {
    println!("adm-new-cli governance commands:");
    println!("  governance hardcoded-paths [--root DIR] [--allow-docs]");
}

fn print_design_help() {
    println!("adm-new-cli design commands:");
    println!(
        "  design export-concept-package --project-state JSON --target-dir DIR [--workspace-mirror DIR] [--no-workspace-mirror]"
    );
}

fn print_iteration_help() {
    println!("adm-new-cli iteration commands:");
    println!("  iteration parse --spec FILE");
    println!("  iteration plan --spec FILE [--output FILE]");
    println!(
        "  iteration inherit --parent-workspace DIR --target-workspace DIR --plan FILE [--parent-version V] [--parent-save-id ID]"
    );
    println!(
        "  iteration prepare --project-root DIR --session-id ID --spec FILE [--state JSON] [--dry-run]"
    );
    println!("  iteration resume --plan FILE");
}

#[cfg(test)]
mod tests {
    use super::*;
    use adm_new_contracts::patch::{PatchRecord, PatchStatus, PatchTask};
    use adm_new_foundation::new_stable_id;
    use serde_json::Value;

    #[test]
    fn patch_cli_help_and_argument_validation() {
        assert_eq!(
            run_patch_command(&["--help".to_string()]),
            ExitCode::SUCCESS
        );
        assert_eq!(
            run_patch_command(&["analyze".to_string()]),
            ExitCode::from(2)
        );
        assert!(parse_patch_status("validated").is_ok());
        assert!(parse_patch_status("complete").is_err());
    }

    #[test]
    fn patch_cli_promote_marks_record() {
        let root = temp_root("patch_cli");
        let store = PatchStore::new(&root);
        let record = store
            .write(&PatchRecord {
                patch_id: "patch-1".to_string(),
                request: "change".to_string(),
                status: PatchStatus::Analyzed,
                created_at: String::new(),
                updated_at: String::new(),
                tasks: vec![PatchTask {
                    task_id: "PATCH-001".to_string(),
                    title: "Patch".to_string(),
                    description: String::new(),
                    affected_systems: Vec::new(),
                    expected_files: Vec::new(),
                    validation_route: Vec::new(),
                    requires_iteration: false,
                }],
                changed_files: Vec::new(),
                validation_summary: Value::Null,
                analysis_summary: String::new(),
                executor_result: Value::Null,
                promoted_iteration_spec: String::new(),
                errors: Vec::new(),
            })
            .unwrap();

        let status = run_patch_command(&[
            "--root".to_string(),
            root.to_string_lossy().to_string(),
            "promote".to_string(),
            record.patch_id.clone(),
            "iteration_specs/v2.0_change.md".to_string(),
        ]);
        let promoted = store.get(&record.patch_id).unwrap();

        assert_eq!(status, ExitCode::SUCCESS);
        assert_eq!(promoted.status, PatchStatus::Promoted);
        assert_eq!(
            promoted.promoted_iteration_spec,
            "iteration_specs/v2.0_change.md"
        );
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn sdk_cli_help_argument_validation_and_file_store() {
        let root = temp_root("sdk_cli");

        assert_eq!(run_sdk_command(&["--help".to_string()]), ExitCode::SUCCESS);
        assert_eq!(run_sdk_command(&["show".to_string()]), ExitCode::from(2));
        assert!(parse_sdk_review_status("approved").is_ok());
        assert!(parse_sdk_review_status("complete").is_err());

        assert_eq!(
            run_sdk_command(&[
                "--root".to_string(),
                root.to_string_lossy().to_string(),
                "init".to_string(),
            ]),
            ExitCode::SUCCESS
        );
        assert_eq!(
            run_sdk_command(&[
                "--root".to_string(),
                root.to_string_lossy().to_string(),
                "add".to_string(),
                "AdMob".to_string(),
                "--url".to_string(),
                "https://example.test".to_string(),
            ]),
            ExitCode::SUCCESS
        );
        assert_eq!(
            run_sdk_command(&[
                "--root".to_string(),
                root.to_string_lossy().to_string(),
                "review".to_string(),
                "admob".to_string(),
                "approved".to_string(),
            ]),
            ExitCode::SUCCESS
        );
        assert_eq!(
            run_sdk_command(&[
                "--root".to_string(),
                root.to_string_lossy().to_string(),
                "context".to_string(),
            ]),
            ExitCode::SUCCESS
        );

        let kb = SdkKnowledgeBase::new(&root);
        let index = kb.read_index().unwrap();
        assert_eq!(index.sdks[0].sdk_id, "admob");
        assert_eq!(index.sdks[0].review_status, SdkReviewStatus::Approved);
        assert!(kb.approved_prompt_context().unwrap().contains("AdMob"));
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn sdk_cli_uses_runtime_overlay_and_project_seed_roots() {
        let project_root = temp_root("sdk_project_seed");
        let data_root = temp_root("sdk_runtime_data");
        let parsed = parse_sdk_args(
            &[
                "--data-root".to_string(),
                data_root.to_string_lossy().to_string(),
                "list".to_string(),
            ],
            &project_root,
        )
        .unwrap();
        assert_eq!(parsed.root, data_root.join("knowledge/sdks"));
        assert_eq!(parsed.seed_root, project_root.join("knowledge/sdks"));
        assert_ne!(parsed.root, parsed.seed_root);
        let _ = fs::remove_dir_all(project_root);
        let _ = fs::remove_dir_all(data_root);
    }

    #[test]
    fn legacy_memory_commands_are_stably_deprecated() {
        assert_eq!(run_memory_command(&[]), ExitCode::from(2));
        assert_eq!(
            run_memory_command(&["inventory".to_string()]),
            ExitCode::from(2)
        );
        assert_eq!(
            run_memory_command(&["--help".to_string()]),
            ExitCode::from(2)
        );
    }

    #[test]
    fn design_sync_audit_help_and_default_parent_detection() {
        assert_eq!(
            run_design_sync_audit_command(&["--help".to_string()]),
            ExitCode::SUCCESS
        );
        let parent = temp_root("design_sync_parent");
        let rust_root = parent.join("NEWrust");
        fs::create_dir_all(&rust_root).unwrap();
        assert!(default_python_audit_root(&rust_root).is_none());
        fs::write(parent.join("AGENTS.md"), "# fixture\n").unwrap();
        assert_eq!(default_python_audit_root(&rust_root), Some(parent.clone()));
        assert_eq!(design_sync_audit_exit_code("passed"), ExitCode::SUCCESS);
        assert_eq!(
            design_sync_audit_exit_code("attention_required"),
            ExitCode::SUCCESS
        );
        assert_eq!(design_sync_audit_exit_code("failed"), ExitCode::from(1));
        let _ = fs::remove_dir_all(parent);
    }

    #[test]
    fn global_project_root_option_is_leading_and_name_independent() {
        let mut args = vec![
            "adm-new-cli".to_string(),
            "--project-root".to_string(),
            "C:/renamed standalone 项目".to_string(),
            "doctor".to_string(),
        ];
        let root = take_leading_project_root(&mut args).unwrap().unwrap();
        assert_eq!(root, PathBuf::from("C:/renamed standalone 项目"));
        assert_eq!(args, vec!["adm-new-cli", "doctor"]);

        let mut trailing = vec![
            "adm-new-cli".to_string(),
            "doctor".to_string(),
            "--project-root=C:/renamed standalone 项目".to_string(),
        ];
        let root = take_root_scoped_project_root(&mut trailing)
            .unwrap()
            .unwrap();
        assert_eq!(root, PathBuf::from("C:/renamed standalone 项目"));
        assert_eq!(trailing, vec!["adm-new-cli", "doctor"]);
    }

    #[test]
    fn gate_reports_are_written_under_internal_gates_directory() {
        let root = temp_root("gate_output_root");
        write_gate_report(&root, "doctor", "status=passed\n").unwrap();
        assert!(root.join("gates/doctor-gate.adm").is_file());
        assert_eq!(
            fs::read_to_string(root.join("gates/doctor-gate.adm")).unwrap(),
            "status=passed\n"
        );
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn package_cli_runs_packaging_service_and_reports_blocked_status() {
        let root = temp_root("package_cli");
        let artifacts = root.join("artifacts");
        let outputs = root.join("outputs");
        write_success_stage14(&artifacts);

        assert_eq!(
            run_package_command(&[
                "--artifacts-dir".to_string(),
                artifacts.to_string_lossy().to_string(),
                "--outputs-dir".to_string(),
                outputs.to_string_lossy().to_string(),
            ]),
            ExitCode::SUCCESS
        );
        assert!(
            outputs
                .join("package/current/package_manifest.json")
                .exists()
        );

        let blocked = temp_root("package_cli_blocked");
        assert_eq!(
            run_package_command(&[
                "--artifacts-dir".to_string(),
                blocked.join("missing").to_string_lossy().to_string(),
                "--outputs-dir".to_string(),
                blocked.join("outputs").to_string_lossy().to_string(),
            ]),
            ExitCode::from(1)
        );
        let _ = fs::remove_dir_all(root);
        let _ = fs::remove_dir_all(blocked);
    }

    #[test]
    fn dist_cli_build_plan_and_rejects_unverified_bundle() {
        assert_eq!(run_dist_command(&["build".to_string()]), ExitCode::SUCCESS);

        let root = temp_root("dist_cli");
        fs::write(root.join(DEFAULT_DIST_EXE_NAME), b"12345").unwrap();
        fs::create_dir_all(root.join("resources")).unwrap();
        fs::write(root.join("resources").join("app.json"), b"{}").unwrap();

        assert_eq!(
            run_dist_command(&[
                "verify-bundle".to_string(),
                root.to_string_lossy().to_string(),
                "--min-bytes".to_string(),
                "5".to_string(),
                "--require".to_string(),
                "resources/app.json".to_string(),
            ]),
            ExitCode::from(1)
        );
        assert_eq!(
            run_dist_command(&[
                "verify-bundle".to_string(),
                root.to_string_lossy().to_string(),
                "--min-bytes".to_string(),
                "5".to_string(),
                "--require".to_string(),
                "missing/file.txt".to_string(),
            ]),
            ExitCode::from(1)
        );
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn asset_cli_generates_placeholder_audio_and_localization_outputs() {
        let root = temp_root("asset_cli");
        let audio_dir = root.join("audio");
        assert_eq!(
            run_asset_command(&["--help".to_string()]),
            ExitCode::SUCCESS
        );
        assert_eq!(
            run_asset_command(&[
                "audio-placeholder".to_string(),
                "--output-dir".to_string(),
                audio_dir.to_string_lossy().to_string(),
                "--filename".to_string(),
                "placeholder.wav".to_string(),
            ]),
            ExitCode::SUCCESS
        );
        assert!(audio_dir.join("placeholder.wav").exists());
        assert_eq!(
            run_asset_command(&[
                "sfx-placeholder".to_string(),
                "jump".to_string(),
                "--output-dir".to_string(),
                audio_dir.to_string_lossy().to_string(),
                "--duration".to_string(),
                "0.1".to_string(),
            ]),
            ExitCode::SUCCESS
        );

        let source = root.join("scripts");
        fs::create_dir_all(&source).unwrap();
        fs::write(
            source.join("Title.cs"),
            "public class Title { string t = \"开始\"; }",
        )
        .unwrap();
        assert_eq!(
            run_asset_command(&[
                "localization".to_string(),
                source.to_string_lossy().to_string(),
                root.join("generated").to_string_lossy().to_string(),
            ]),
            ExitCode::SUCCESS
        );
        assert!(root.join("generated/LocalizationManager.cs").exists());
        assert!(
            fs::read_to_string(source.join("Title.cs"))
                .unwrap()
                .contains("Loc.Get")
        );
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn image_cli_reports_metadata_and_codex_command_specs() {
        let root = temp_root("image_cli");
        let png = root.join("tiny.png");
        fs::write(&png, minimal_png_header(32, 24)).unwrap();
        assert_eq!(
            run_image_command(&["--help".to_string()]),
            ExitCode::SUCCESS
        );
        assert_eq!(
            run_image_command(&[
                "metadata".to_string(),
                png.to_string_lossy().to_string(),
                "--width".to_string(),
                "32".to_string(),
                "--height".to_string(),
                "24".to_string(),
            ]),
            ExitCode::SUCCESS
        );
        assert_eq!(
            run_image_command(&[
                "codex-command".to_string(),
                "--project-root".to_string(),
                root.to_string_lossy().to_string(),
                "--codex-home".to_string(),
                root.join("codex").to_string_lossy().to_string(),
                "--cli".to_string(),
                "codex.cmd".to_string(),
                "style reference".to_string(),
            ]),
            ExitCode::SUCCESS
        );
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn dev_project_pipeline_unity_validate_cli_commands_cover_a27_tools() {
        let root = temp_root("a27_cli");
        let schema = root.join("schema.json");
        let tables = root.join("tables");
        let output = root.join("out");
        fs::create_dir_all(&tables).unwrap();
        fs::write(
            &schema,
            r#"{"tables":[{"name":"items","columns":[{"name":"id","type":"int"}]}]}"#,
        )
        .unwrap();
        fs::write(tables.join("items.csv"), "id\n1").unwrap();

        assert_eq!(run_dev_command(&["--help".to_string()]), ExitCode::SUCCESS);
        assert_eq!(
            run_dev_command(&[
                "config-compile".to_string(),
                schema.to_string_lossy().to_string(),
                tables.to_string_lossy().to_string(),
                output.to_string_lossy().to_string(),
            ]),
            ExitCode::SUCCESS
        );
        assert!(output.join("items.json").exists());
        assert_eq!(
            run_dev_command(&[
                "git".to_string(),
                "--work-dir".to_string(),
                root.to_string_lossy().to_string(),
                "git".to_string(),
                "status".to_string(),
            ]),
            ExitCode::SUCCESS
        );
        assert_eq!(
            run_dev_command(&["git".to_string(), "git".to_string(), "push".to_string()]),
            ExitCode::from(1)
        );

        assert_eq!(
            run_project_command(&[
                "scaffold".to_string(),
                root.join("project").to_string_lossy().to_string(),
            ]),
            ExitCode::SUCCESS
        );
        assert_eq!(
            run_pipeline_command(&[
                "scaffold-step".to_string(),
                root.join("project").to_string_lossy().to_string(),
                "16".to_string(),
                "new-stage".to_string(),
            ]),
            ExitCode::SUCCESS
        );

        let source = root.join("Assets/Scripts");
        fs::create_dir_all(&source).unwrap();
        fs::write(
            source.join("Mover.cs"),
            "public class Mover { void Update() { Tick(); } }",
        )
        .unwrap();
        assert_eq!(
            run_unity_command(&[
                "error-logger".to_string(),
                source.to_string_lossy().to_string(),
                root.join("generated").to_string_lossy().to_string(),
            ]),
            ExitCode::SUCCESS
        );
        let graph = root.join("ui_graph.json");
        fs::write(
            &graph,
            r#"{"registry":{"panels":["main"]},"graph":{"states":{"main":{"layer":"Screen"}}}}"#,
        )
        .unwrap();
        assert_eq!(
            run_unity_command(&[
                "ui-state".to_string(),
                graph.to_string_lossy().to_string(),
                root.join("ui").to_string_lossy().to_string(),
            ]),
            ExitCode::SUCCESS
        );
        assert_eq!(
            run_validate_command(&[
                "compile-plan".to_string(),
                "--work-dir".to_string(),
                root.to_string_lossy().to_string(),
            ]),
            ExitCode::SUCCESS
        );
        assert_eq!(
            run_validate_command(&[
                "compile-result".to_string(),
                "--exit-code".to_string(),
                "0".to_string(),
                "--stdout".to_string(),
                "Compiled".to_string(),
            ]),
            ExitCode::SUCCESS
        );
        let env_config = root.join("env.json");
        fs::write(&env_config, r#"{"tools":[],"python":["pytest"]}"#).unwrap();
        assert_eq!(
            run_validate_command(&[
                "environment".to_string(),
                env_config.to_string_lossy().to_string(),
            ]),
            ExitCode::SUCCESS
        );
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn migration_schema_governance_pipeline_design_cli_commands_cover_a28_scripts() {
        let root = temp_root("a28_cli");

        let legacy = root.join("legacy");
        fs::create_dir_all(legacy.join("Assets")).unwrap();
        fs::write(legacy.join("Assets/demo.txt"), "demo").unwrap();
        assert_eq!(
            run_migrate_command(&[
                "legacy".to_string(),
                "--project-root".to_string(),
                root.to_string_lossy().to_string(),
                "--source".to_string(),
                legacy.to_string_lossy().to_string(),
                "--target".to_string(),
                "workspace/imported".to_string(),
            ]),
            ExitCode::SUCCESS
        );
        assert!(
            root.join("workspace/outputs/migration_report.json")
                .exists()
        );
        assert!(!root.join("workspace/imported/Assets/demo.txt").exists());
        assert_eq!(
            run_migrate_command(&[
                "legacy".to_string(),
                "--project-root".to_string(),
                root.to_string_lossy().to_string(),
                "--source".to_string(),
                legacy.to_string_lossy().to_string(),
                "--target".to_string(),
                "workspace/imported".to_string(),
                "--apply".to_string(),
            ]),
            ExitCode::SUCCESS
        );
        assert!(root.join("workspace/imported/Assets/demo.txt").exists());

        let save_store = root.join("save/save_one/workspace/outputs/execution_objects");
        fs::create_dir_all(&save_store).unwrap();
        fs::write(
            save_store.join("execution_objects.json"),
            r#"{"objects":[]}"#,
        )
        .unwrap();
        assert_eq!(
            run_migrate_command(&[
                "eo-save-id".to_string(),
                "--save-root".to_string(),
                root.join("save").to_string_lossy().to_string(),
                "--report".to_string(),
                root.join("save_report.md").to_string_lossy().to_string(),
                "--apply".to_string(),
            ]),
            ExitCode::SUCCESS
        );
        let save_store_json =
            fs::read_to_string(save_store.join("execution_objects.json")).unwrap();
        assert!(save_store_json.contains("\"save_id\": \"save_one\""));

        let projects = root.join("projects");
        fs::create_dir_all(&projects).unwrap();
        fs::write(
            projects.join("demo.json"),
            r#"{"projectName":"Demo","nodes":{"core":{"decisionState":"completed","checklist":{"loop":true}}}}"#,
        )
        .unwrap();
        let store_path = root.join("eo/execution_objects.json");
        assert_eq!(
            run_migrate_command(&[
                "design-projects".to_string(),
                "--project-root".to_string(),
                root.to_string_lossy().to_string(),
                "--store-path".to_string(),
                store_path.to_string_lossy().to_string(),
                "--apply".to_string(),
            ]),
            ExitCode::SUCCESS
        );
        assert!(store_path.exists());

        let schema_input = root.join("schema_input.json");
        let schema_file = root.join("schema.json");
        let schema_output = root.join("schema_output.md");
        fs::write(
            &schema_input,
            r#"{"alignment_version":"2.0","unified_assets":[{"frames":["idle"]}]}"#,
        )
        .unwrap();
        fs::write(
            &schema_file,
            r#"{"contract_version":"2.1","schema_migration":{"migration_rules":[{"from":"2.0","action":"wrap_to_object","field":"frames","new_field":"animation","structure":{"frames":"$old_frames"}}]}}"#,
        )
        .unwrap();
        assert_eq!(
            run_schema_command(&[
                "migrate".to_string(),
                "--input".to_string(),
                schema_input.to_string_lossy().to_string(),
                "--schema".to_string(),
                schema_file.to_string_lossy().to_string(),
                "--output".to_string(),
                schema_output.to_string_lossy().to_string(),
            ]),
            ExitCode::SUCCESS
        );
        assert!(schema_output.exists());

        assert_eq!(
            run_governance_command(&[
                "hardcoded-paths".to_string(),
                "--root".to_string(),
                root.to_string_lossy().to_string(),
            ]),
            ExitCode::SUCCESS
        );

        let artifacts = root.join("artifacts/stage_00");
        fs::create_dir_all(&artifacts).unwrap();
        fs::write(
            artifacts.join("validation_report.json"),
            r#"{"status":"failed"}"#,
        )
        .unwrap();
        assert_eq!(
            run_pipeline_command(&[
                "inspect-reports".to_string(),
                "--artifacts-dir".to_string(),
                root.join("artifacts").to_string_lossy().to_string(),
            ]),
            ExitCode::SUCCESS
        );

        let state_path = root.join("state.json");
        fs::write(
            &state_path,
            r#"{"projectName":"Demo","nodes":{"core":{"decisionState":"completed","designNote":"Loop","checklist":{"loop":true}}}}"#,
        )
        .unwrap();
        assert_eq!(
            run_design_command(&[
                "export-concept-package".to_string(),
                "--project-state".to_string(),
                state_path.to_string_lossy().to_string(),
                "--target-dir".to_string(),
                root.join("packages").to_string_lossy().to_string(),
                "--workspace-mirror".to_string(),
                root.join("mirror").to_string_lossy().to_string(),
            ]),
            ExitCode::SUCCESS
        );
        assert!(
            root.join("packages/devflow_Design_v2/structured/handoff_manifest.json")
                .exists()
        );
        assert!(
            root.join("mirror/devflow_Concept_v2/attachments/concept.md")
                .exists()
        );
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn validation_cli_commands_cover_a29_validators() {
        let root = temp_root("a29_cli");

        let schema = root.join("config_schema.json");
        let tables = root.join("tables");
        fs::create_dir_all(&tables).unwrap();
        fs::write(
            &schema,
            r#"{"tables":[{"name":"items","columns":[{"name":"id","type":"int","required":true,"unique":true},{"name":"enabled","type":"bool"}]}]}"#,
        )
        .unwrap();
        fs::write(tables.join("items.csv"), "id,enabled\n1,true\n").unwrap();
        assert_eq!(
            run_validate_command(&[
                "config".to_string(),
                schema.to_string_lossy().to_string(),
                tables.to_string_lossy().to_string(),
            ]),
            ExitCode::SUCCESS
        );

        let context = root.join("CONTEXT.md");
        fs::write(
            &context,
            [
                "Core Pipeline And Save Boundaries",
                "Art Governance And Asset Contracts",
                "Execution Objects And GUI Gates",
                "Stage Sequence And Runtime Integration",
            ]
            .iter()
            .map(|section| format!("## {section}\n**Term**: definition\n_ADR_: adr-1\n"))
            .collect::<Vec<_>>()
            .join("\n"),
        )
        .unwrap();
        assert_eq!(
            run_validate_command(&["context".to_string(), context.to_string_lossy().to_string()]),
            ExitCode::SUCCESS
        );

        let contract = root.join("contract.json");
        let contract_schema = root.join("contract_schema.json");
        let contract_report = root.join("contract_report.json");
        fs::write(&contract, r#"{"name":"demo","items":[1]}"#).unwrap();
        fs::write(
            &contract_schema,
            r#"{"type":"object","required":["name","items"],"properties":{"name":{"type":"string"},"items":{"type":"array","items":{"type":"integer"}}}}"#,
        )
        .unwrap();
        assert_eq!(
            run_validate_command(&[
                "contract".to_string(),
                contract.to_string_lossy().to_string(),
                contract_schema.to_string_lossy().to_string(),
                "--report".to_string(),
                contract_report.to_string_lossy().to_string(),
            ]),
            ExitCode::SUCCESS
        );
        assert!(contract_report.exists());

        assert_eq!(
            run_validate_command(&[
                "output".to_string(),
                "--format".to_string(),
                "json".to_string(),
                "--required".to_string(),
                "ok".to_string(),
                "--text".to_string(),
                "```json\n{\"ok\":true}\n```".to_string(),
            ]),
            ExitCode::SUCCESS
        );
        assert_eq!(
            run_validate_command(&[
                "output".to_string(),
                "--text".to_string(),
                "sorry".to_string(),
            ]),
            ExitCode::from(1)
        );

        let artifacts = root.join("artifacts");
        fs::create_dir_all(artifacts.join("stage_02")).unwrap();
        fs::write(
            artifacts.join("stage_02/entity_coverage_report.json"),
            r#"{"entity_coverage_rate":0.4,"entity_count":4}"#,
        )
        .unwrap();
        assert_eq!(
            run_validate_command(&[
                "pipeline-quality".to_string(),
                "--artifacts-dir".to_string(),
                artifacts.to_string_lossy().to_string(),
                "--check".to_string(),
                "plan-002".to_string(),
            ]),
            ExitCode::SUCCESS
        );

        fs::create_dir_all(artifacts.join("stage_00")).unwrap();
        fs::create_dir_all(artifacts.join("stage_08")).unwrap();
        fs::write(
            artifacts.join("stage_00/concept_profile.json"),
            r#"{"project_name":"Moon Tower","project_signature":"sig-a"}"#,
        )
        .unwrap();
        fs::write(
            artifacts.join("stage_08/program_task_breakdown.json"),
            r#"{"project_signature":"sig-a","tasks":[{"task_id":"TASK-1","title":"Moon loop","project_semantic_refs":["moon_loop"]}]}"#,
        )
        .unwrap();
        assert_eq!(
            run_validate_command(&[
                "design-semantic-quality".to_string(),
                "--artifacts-root".to_string(),
                artifacts.to_string_lossy().to_string(),
                "--output-json".to_string(),
                root.join("semantic.json").to_string_lossy().to_string(),
                "--output-md".to_string(),
                root.join("semantic.md").to_string_lossy().to_string(),
            ]),
            ExitCode::SUCCESS
        );
        assert!(root.join("semantic.json").exists());
        assert!(root.join("semantic.md").exists());
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn iteration_cli_commands_cover_a30_iteration_flow() {
        let root = temp_root("a30_cli");
        let spec = root.join("v2.0_rewarded_ads.md");
        fs::write(
            &spec,
            r#"# 迭代规格：激励广告复活

版本：v2.0
基于：v1.0
类型：feature_addition
影响范围：narrow

## 新增功能

- **功能名称**：激励广告复活
  - 描述：玩家死亡后弹出看广告复活提示
  - 涉及系统：死亡系统、UI层

## 修改现有功能

（本次迭代无修改）

## 不变内容（明确排除）

- 核心玩法循环不变
- 美术资产不变
"#,
        )
        .unwrap();

        assert_eq!(
            run_iteration_command(&["parse".to_string(), spec.to_string_lossy().to_string()]),
            ExitCode::SUCCESS
        );
        let plan = root.join("plan.json");
        assert_eq!(
            run_iteration_command(&[
                "plan".to_string(),
                "--spec".to_string(),
                spec.to_string_lossy().to_string(),
                "--output".to_string(),
                plan.to_string_lossy().to_string(),
            ]),
            ExitCode::SUCCESS
        );
        assert!(plan.exists());

        let parent = root.join("parent");
        let target = root.join("target");
        fs::create_dir_all(parent.join("outputs/artifacts/stage_04")).unwrap();
        fs::write(
            parent.join("outputs/artifacts/stage_04/asset_spec_contract.json"),
            r#"{"ok":true}"#,
        )
        .unwrap();
        assert_eq!(
            run_iteration_command(&[
                "inherit".to_string(),
                "--parent-workspace".to_string(),
                parent.to_string_lossy().to_string(),
                "--target-workspace".to_string(),
                target.to_string_lossy().to_string(),
                "--parent-version".to_string(),
                "v1.0".to_string(),
                "--parent-save-id".to_string(),
                "save-1".to_string(),
                "--plan".to_string(),
                plan.to_string_lossy().to_string(),
            ]),
            ExitCode::SUCCESS
        );
        assert!(
            target
                .join("outputs/artifacts/stage_04/asset_spec_contract.json")
                .exists()
        );

        let save_service =
            adm_new_application::SaveApplicationService::with_pid(&root, "session_a", 100).unwrap();
        let initial = save_service.create_blank_save("Initial").unwrap();
        fs::create_dir_all(
            root.join("saves")
                .join(&initial.manifest.save_id)
                .join("workspace/outputs/artifacts/stage_04"),
        )
        .unwrap();
        assert_eq!(
            run_iteration_command(&[
                "prepare".to_string(),
                "--project-root".to_string(),
                root.to_string_lossy().to_string(),
                "--session-id".to_string(),
                "session_a".to_string(),
                "--spec".to_string(),
                spec.to_string_lossy().to_string(),
                "--dry-run".to_string(),
            ]),
            ExitCode::SUCCESS
        );
        let preview =
            root.join("drafts/session_a/outputs/artifacts/delta_execution_plan_preview.json");
        assert!(preview.exists());
        assert_eq!(
            run_iteration_command(&[
                "resume".to_string(),
                "--plan".to_string(),
                preview.to_string_lossy().to_string(),
            ]),
            ExitCode::SUCCESS
        );
        let _ = fs::remove_dir_all(root);
    }

    fn write_success_stage14(artifacts: &Path) {
        let stage14 = artifacts.join("stage_14");
        fs::create_dir_all(&stage14).unwrap();
        let checks = adm_new_contracts::package::REQUIRED_INTEGRATION_CHECKS
            .iter()
            .map(|id| ((*id).to_string(), Value::Bool(true)))
            .collect::<serde_json::Map<_, _>>();
        fs::write(
            stage14.join("integration.json"),
            serde_json::to_string_pretty(&json!({
                "status": "success",
                "checks": Value::Object(checks)
            }))
            .unwrap(),
        )
        .unwrap();
        fs::write(
            stage14.join("actual_project_file_audit.json"),
            serde_json::to_string_pretty(&json!({
                "development_path": "UnityProject",
                "actual_changed_files": ["Assets/DemoScene.unity"]
            }))
            .unwrap(),
        )
        .unwrap();
        fs::write(
            stage14.join("unity_validation_summary.json"),
            serde_json::to_string_pretty(&json!({
                "valid": true,
                "unity_editor_path": "Unity.exe",
                "validation_count": 3,
                "failed_validation_count": 0
            }))
            .unwrap(),
        )
        .unwrap();
    }

    fn temp_root(prefix: &str) -> std::path::PathBuf {
        let root = std::env::temp_dir().join(new_stable_id(prefix).unwrap());
        fs::create_dir_all(&root).unwrap();
        root
    }

    fn minimal_png_header(width: u32, height: u32) -> Vec<u8> {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(b"\x89PNG\r\n\x1a\n");
        bytes.extend_from_slice(&13_u32.to_be_bytes());
        bytes.extend_from_slice(b"IHDR");
        bytes.extend_from_slice(&width.to_be_bytes());
        bytes.extend_from_slice(&height.to_be_bytes());
        bytes.extend_from_slice(&[8, 6, 0, 0, 0]);
        bytes.extend_from_slice(&0_u32.to_be_bytes());
        bytes
    }
}
