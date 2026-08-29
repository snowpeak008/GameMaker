#![forbid(unsafe_code)]

use adm_ai::{AiCapability, AiProvider, AiTaskJournal, AiTaskRequest, MockAiProvider};
use adm_application::{
    AdmApplication, default_data_root, default_demo_brief, design_brief_from_parts,
};
use adm_archive::inspect_archive_package;
use adm_config::{
    AiProviderConfig, AiProviderReadiness, SecretRef, ai_provider_preset, ai_provider_presets,
    default_secret_ref_for_preset,
};
use adm_foundation::{AdmResult, ArchiveId, ContentHash, ProviderId, SessionId};
use adm_packaging::{
    DesktopReleaseSpec, DryRunEngineBuildRunner, EngineBuildExecutionStatus, EngineBuildRunner,
    ExternalAiProviderAcceptance, GameBuildPlan, LOCAL_ENGINE_BUILD_CONFIRMATION_TOKEN,
    LocalProcessEngineBuildRunner, discover_unity_editor, inspect_delivery,
    inspect_desktop_release, inspect_unity_build_preflight, plan_unity_cli_build,
    plan_unity_runtime_validation, run_external_acceptance, run_release_acceptance,
    stage_desktop_release, stage_game_build_bundle, stage_sdk_bundle, stage_unity_project_scaffold,
    validate_local_engine_build_confirmation,
};
use std::fs;
use std::path::{Path, PathBuf};

const UNITY_PLAYMODE_EVIDENCE_PATH: &str =
    "dist/unity-project/Assets/AutoDesignMaker/Generated/runtime_execution_results.adm";

fn main() {
    if let Err(error) = run() {
        eprintln!("{error}");
        std::process::exit(1);
    }
}

fn run() -> AdmResult<()> {
    let mut args = std::env::args().skip(1);
    match args.next().as_deref() {
        Some("--version") | Some("-V") => {
            println!("adm-cli {}", env!("CARGO_PKG_VERSION"));
        }
        Some("doctor") => {
            let session_id = SessionId::generate();
            println!("AutoDesignMaker Rust CLI doctor: ok");
            println!("session_id={session_id}");
        }
        Some("ai-doctor") => {
            let data_root = args
                .next()
                .map(PathBuf::from)
                .unwrap_or_else(|| default_data_root(&std::env::current_dir().unwrap()));
            let app = AdmApplication::for_data_root(&data_root)?;
            print!("{}", app.ai_diagnostics().render());
        }
        Some("ai-secret-set") => {
            let name = args.next().ok_or_else(|| {
                adm_foundation::AdmError::invalid_input("ai-secret-set requires name")
            })?;
            let secret = args.next().ok_or_else(|| {
                adm_foundation::AdmError::invalid_input("ai-secret-set requires secret")
            })?;
            let data_root = args
                .next()
                .map(PathBuf::from)
                .unwrap_or_else(|| default_data_root(&std::env::current_dir().unwrap()));
            let app = AdmApplication::for_data_root(&data_root)?;
            let secret_path = app.upsert_named_secret(name, secret)?;
            println!("saved_named_secrets={}", secret_path.display());
            print!("{}", app.ai_diagnostics().render());
        }
        Some("ai-journal") => {
            let archive_id = args.next().ok_or_else(|| {
                adm_foundation::AdmError::invalid_input("ai-journal requires archive_id")
            })?;
            let data_root = args
                .next()
                .map(PathBuf::from)
                .unwrap_or_else(|| default_data_root(&std::env::current_dir().unwrap()));
            let app = AdmApplication::for_data_root(&data_root)?;
            let archive = app.load_project(&archive_id)?;
            let journal_path = archive.root.join("content").join("ai").join("journal.adm");
            if !journal_path.exists() {
                println!("AI: no journal");
                return Ok(());
            }
            let journal = AiTaskJournal::load_from_path(journal_path)?;
            let summary = journal.summary();
            println!("records={}", summary.record_count);
            println!("accepted={}", summary.accepted_count);
            println!("failed={}", summary.failed_count);
            println!("rejected={}", summary.rejected_count);
            let failures = summary.failure_summary_line();
            if !failures.is_empty() {
                println!("failures={failures}");
            }
            if let Some(kind) = summary.last_failure_kind {
                println!("last_failure_kind={}", kind.as_str());
            }
            if let Some(error) = summary.last_error {
                println!("last_error={error}");
            }
        }
        Some("ai-provider-presets") => {
            println!("network_call=false");
            for preset in ai_provider_presets() {
                println!("{}", preset.render_line());
            }
        }
        Some("ai-provider-preset") => {
            let preset_id = args.next().ok_or_else(|| {
                adm_foundation::AdmError::invalid_input("ai-provider-preset requires preset_id")
            })?;
            let provider_id = args.next().ok_or_else(|| {
                adm_foundation::AdmError::invalid_input("ai-provider-preset requires provider_id")
            })?;
            let secret_ref = args.next().ok_or_else(|| {
                adm_foundation::AdmError::invalid_input(
                    "ai-provider-preset requires secret_ref|default|none",
                )
            })?;
            let data_root = args
                .next()
                .map(PathBuf::from)
                .unwrap_or_else(|| default_data_root(&std::env::current_dir().unwrap()));
            let preset = ai_provider_preset(&preset_id)?;
            let secret_ref = preset_secret_ref_from_cli(&preset, secret_ref)?;
            let provider = preset.to_provider_config(ProviderId::new(provider_id)?, secret_ref)?;
            let mut app = AdmApplication::for_data_root(&data_root)?;
            let config_path = app.upsert_ai_provider(provider)?;
            println!("network_call=false");
            println!("preset_id={}", preset.preset_id);
            println!("endpoint_hint={}", preset.endpoint_hint);
            println!("saved_config={}", config_path.display());
            print!("{}", app.ai_diagnostics().render());
        }
        Some("ai-provider-set") => {
            let provider_id = args.next().ok_or_else(|| {
                adm_foundation::AdmError::invalid_input("ai-provider-set requires provider_id")
            })?;
            let endpoint_hint = args.next().ok_or_else(|| {
                adm_foundation::AdmError::invalid_input("ai-provider-set requires endpoint_hint")
            })?;
            let secret_ref = args.next().ok_or_else(|| {
                adm_foundation::AdmError::invalid_input("ai-provider-set requires secret_ref")
            })?;
            let data_root = args
                .next()
                .map(PathBuf::from)
                .unwrap_or_else(|| default_data_root(&std::env::current_dir().unwrap()));
            let provider = ai_provider_config_from_cli(provider_id, endpoint_hint, secret_ref)?;
            let mut app = AdmApplication::for_data_root(&data_root)?;
            let config_path = app.upsert_ai_provider(provider)?;
            println!("saved_config={}", config_path.display());
            print!("{}", app.ai_diagnostics().render());
        }
        Some("ai-provider-disable") => {
            let provider_id = args.next().ok_or_else(|| {
                adm_foundation::AdmError::invalid_input("ai-provider-disable requires provider_id")
            })?;
            let data_root = args
                .next()
                .map(PathBuf::from)
                .unwrap_or_else(|| default_data_root(&std::env::current_dir().unwrap()));
            let mut app = AdmApplication::for_data_root(&data_root)?;
            let config_path = app.disable_ai_provider(ProviderId::new(provider_id)?)?;
            println!("saved_config={}", config_path.display());
            print!("{}", app.ai_diagnostics().render());
        }
        Some("ai-provider-check") => {
            let provider_id = args.next().ok_or_else(|| {
                adm_foundation::AdmError::invalid_input("ai-provider-check requires provider_id")
            })?;
            let model = args.next().ok_or_else(|| {
                adm_foundation::AdmError::invalid_input("ai-provider-check requires model")
            })?;
            let data_root = args
                .next()
                .map(PathBuf::from)
                .unwrap_or_else(|| default_data_root(&std::env::current_dir().unwrap()));
            let app = AdmApplication::for_data_root(&data_root)?;
            let provider_id = ProviderId::new(provider_id)?;
            let provider =
                app.chat_completions_provider_from_config(&provider_id, model.clone())?;
            println!("provider_id={}", provider.provider_id());
            println!("model={model}");
            println!("network_call=false");
            for capability in cli_capabilities() {
                println!(
                    "supports.{}={}",
                    capability.as_str(),
                    provider.supports(&capability)
                );
            }
        }
        Some("ai-provider-invoke") => {
            let provider_id = args.next().ok_or_else(|| {
                adm_foundation::AdmError::invalid_input("ai-provider-invoke requires provider_id")
            })?;
            let model = args.next().ok_or_else(|| {
                adm_foundation::AdmError::invalid_input("ai-provider-invoke requires model")
            })?;
            let prompt = args.next().ok_or_else(|| {
                adm_foundation::AdmError::invalid_input("ai-provider-invoke requires prompt")
            })?;
            let data_root = args
                .next()
                .map(PathBuf::from)
                .unwrap_or_else(|| default_data_root(&std::env::current_dir().unwrap()));
            let app = AdmApplication::for_data_root(&data_root)?;
            let provider_id = ProviderId::new(provider_id)?;
            let provider =
                app.chat_completions_provider_from_config(&provider_id, model.clone())?;
            let request = AiTaskRequest::new(
                AiCapability::TextGeneration,
                prompt,
                "manual CLI provider invocation",
            )?;
            let result = provider.run(&request)?;
            println!("provider_id={}", result.provider_id);
            println!("model={model}");
            println!("network_call=true");
            println!("output_state={:?}", result.output_state);
            println!("raw_output={}", result.raw_output);
            if !result.validation_notes.is_empty() {
                println!("validation_notes={}", result.validation_notes.join("; "));
            }
        }
        Some("ai-acceptance") => {
            let raw_args = args.collect::<Vec<_>>();
            if raw_args
                .iter()
                .any(|arg| arg.as_str() == "--help" || arg.as_str() == "-h")
            {
                println!(
                    "ai-acceptance [--invoke] [--require-ready] [--require-invoke] <provider_id> <model> [report_path] [data_root]"
                );
                println!(
                    "Writes a redacted AI provider acceptance report. --invoke performs the network call; --require-ready exits non-zero unless the report is ready."
                );
                return Ok(());
            }

            let mut invoke = false;
            let mut require_ready = false;
            let mut positional = Vec::new();
            for arg in raw_args {
                match arg.as_str() {
                    "--invoke" => invoke = true,
                    "--require-ready" => require_ready = true,
                    "--require-invoke" => {
                        invoke = true;
                        require_ready = true;
                    }
                    flag if flag.starts_with('-') => {
                        return Err(adm_foundation::AdmError::invalid_input(format!(
                            "unknown ai-acceptance flag: {flag}"
                        )));
                    }
                    _ => positional.push(arg),
                }
            }
            if positional.len() < 2 || positional.len() > 4 {
                return Err(adm_foundation::AdmError::invalid_input(
                    "ai-acceptance requires provider_id and model, with optional report_path and data_root",
                ));
            }
            let provider_id = positional[0].clone();
            let model = positional[1].clone();
            let report_path = positional
                .get(2)
                .map(PathBuf::from)
                .unwrap_or_else(default_ai_acceptance_report_path);
            let data_root = positional
                .get(3)
                .map(PathBuf::from)
                .unwrap_or_else(|| default_data_root(&std::env::current_dir().unwrap()));
            let report = run_ai_acceptance(
                &provider_id,
                &model,
                report_path,
                data_root,
                invoke,
                require_ready,
            )?;
            print!("{}", report.render());
            if require_ready && !report.ready() {
                return Err(adm_foundation::AdmError::validation(format!(
                    "AI acceptance is not ready; report={}",
                    report.report_path.display()
                )));
            }
        }
        Some("demo-core") => {
            let title = args.next().unwrap_or_else(|| "Demo Game".to_string());
            let data_root = args
                .next()
                .map(PathBuf::from)
                .unwrap_or_else(|| default_data_root(&std::env::current_dir().unwrap()));
            let app = AdmApplication::for_data_root(&data_root)?;
            let created = app.create_project(&title)?;
            let provider =
                MockAiProvider::new(ProviderId::new("mock")?, vec![AiCapability::TextGeneration]);
            let report =
                app.run_core_pipeline(&created.archive, default_demo_brief(&title)?, &provider)?;
            println!("created_archive={}", created.archive.manifest.archive_id);
            println!("archive_root={}", created.archive.root.display());
            println!("pipeline_status={:?}", report.pipeline_report.status());
            println!("validation_status={:?}", report.validation.status);
            println!("written_files={}", report.commit.written_files.len());
        }
        Some("run-core") => {
            let title = args.next().ok_or_else(|| {
                adm_foundation::AdmError::invalid_input("run-core requires title")
            })?;
            let genre = args.next().ok_or_else(|| {
                adm_foundation::AdmError::invalid_input("run-core requires genre")
            })?;
            let player_promise = args.next().ok_or_else(|| {
                adm_foundation::AdmError::invalid_input("run-core requires player_promise")
            })?;
            let core_loop_steps = args.next().ok_or_else(|| {
                adm_foundation::AdmError::invalid_input("run-core requires core_loop_steps")
            })?;
            let data_root = args
                .next()
                .map(PathBuf::from)
                .unwrap_or_else(|| default_data_root(&std::env::current_dir().unwrap()));
            let app = AdmApplication::for_data_root(&data_root)?;
            let created = app.create_project(&title)?;
            let provider =
                MockAiProvider::new(ProviderId::new("mock")?, vec![AiCapability::TextGeneration]);
            let brief = design_brief_from_parts(&title, &genre, &player_promise, &core_loop_steps)?;
            let report = app.run_core_pipeline(&created.archive, brief, &provider)?;
            println!("created_archive={}", created.archive.manifest.archive_id);
            println!("archive_root={}", created.archive.root.display());
            println!("pipeline_status={:?}", report.pipeline_report.status());
            println!("validation_status={:?}", report.validation.status);
            println!("written_files={}", report.commit.written_files.len());
        }
        Some("rerun-stage") => {
            let archive_id = args.next().ok_or_else(|| {
                adm_foundation::AdmError::invalid_input("rerun-stage requires archive_id")
            })?;
            let stage_id = args.next().ok_or_else(|| {
                adm_foundation::AdmError::invalid_input("rerun-stage requires stage_id")
            })?;
            let data_root = args
                .next()
                .map(PathBuf::from)
                .unwrap_or_else(|| default_data_root(&std::env::current_dir().unwrap()));
            let app = AdmApplication::for_data_root(&data_root)?;
            let archive = app.load_project(&archive_id)?;
            let provider =
                MockAiProvider::new(ProviderId::new("mock")?, vec![AiCapability::TextGeneration]);
            let brief = app.load_project_brief(&archive)?;
            let report = app.rerun_core_pipeline_stage(&archive, brief, &provider, &stage_id)?;
            println!("archive_id={}", archive.manifest.archive_id);
            println!("rerun_stage={stage_id}");
            println!("pipeline_status={:?}", report.pipeline_report.status());
            println!("validation_status={:?}", report.validation.status);
            println!("rerun_results={}", report.pipeline_report.results.len());
            println!("written_files={}", report.commit.written_files.len());
        }
        Some("resume-failed") => {
            let archive_id = args.next().ok_or_else(|| {
                adm_foundation::AdmError::invalid_input("resume-failed requires archive_id")
            })?;
            let data_root = args
                .next()
                .map(PathBuf::from)
                .unwrap_or_else(|| default_data_root(&std::env::current_dir().unwrap()));
            let app = AdmApplication::for_data_root(&data_root)?;
            let archive = app.load_project(&archive_id)?;
            let provider =
                MockAiProvider::new(ProviderId::new("mock")?, vec![AiCapability::TextGeneration]);
            let brief = app.load_project_brief(&archive)?;
            let report = app.resume_failed_core_pipeline(&archive, brief, &provider)?;
            println!("archive_id={}", archive.manifest.archive_id);
            println!("pipeline_status={:?}", report.pipeline_report.status());
            println!("validation_status={:?}", report.validation.status);
            println!("rerun_results={}", report.pipeline_report.results.len());
            println!("written_files={}", report.commit.written_files.len());
        }
        Some("list") => {
            let data_root = args
                .next()
                .map(PathBuf::from)
                .unwrap_or_else(|| default_data_root(&std::env::current_dir().unwrap()));
            let app = AdmApplication::for_data_root(&data_root)?;
            for project in app.list_projects()? {
                println!(
                    "{}\t{}\t{}",
                    project.archive_id,
                    project.display_name,
                    project.root.display()
                );
            }
        }
        Some("workspace-doctor") => {
            let data_root = args
                .next()
                .map(PathBuf::from)
                .unwrap_or_else(|| default_data_root(&std::env::current_dir().unwrap()));
            let app = AdmApplication::for_data_root(&data_root)?;
            print!("{}", app.inspect_workspaces()?.render());
        }
        Some("workspace-cleanup") => {
            let data_root = args
                .next()
                .map(PathBuf::from)
                .unwrap_or_else(|| default_data_root(&std::env::current_dir().unwrap()));
            let app = AdmApplication::for_data_root(&data_root)?;
            print!("{}", app.cleanup_stale_workspaces()?.render());
        }
        Some("export") => {
            let archive_id = args.next().ok_or_else(|| {
                adm_foundation::AdmError::invalid_input("export requires archive_id")
            })?;
            let target = args.next().map(PathBuf::from).ok_or_else(|| {
                adm_foundation::AdmError::invalid_input("export requires target_file")
            })?;
            let data_root = args
                .next()
                .map(PathBuf::from)
                .unwrap_or_else(|| default_data_root(&std::env::current_dir().unwrap()));
            let app = AdmApplication::for_data_root(&data_root)?;
            let exported = app.export_project(&archive_id, target)?;
            println!("exported={}", exported.display());
        }
        Some("import") => {
            let package = args.next().map(PathBuf::from).ok_or_else(|| {
                adm_foundation::AdmError::invalid_input("import requires package_file")
            })?;
            let data_root = args
                .next()
                .map(PathBuf::from)
                .unwrap_or_else(|| default_data_root(&std::env::current_dir().unwrap()));
            let app = AdmApplication::for_data_root(&data_root)?;
            let imported = app.import_project(package)?;
            println!("imported_archive={}", imported.archive_id);
            println!("display_name={}", imported.display_name);
            println!("archive_root={}", imported.root.display());
        }
        Some("package-doctor") => {
            let package = args.next().map(PathBuf::from).ok_or_else(|| {
                adm_foundation::AdmError::invalid_input("package-doctor requires package_file")
            })?;
            let report = inspect_archive_package(package)?;
            print!("{}", report.render());
        }
        Some("runtime-validation-record") => {
            let archive_id = args.next().ok_or_else(|| {
                adm_foundation::AdmError::invalid_input(
                    "runtime-validation-record requires archive_id",
                )
            })?;
            let results_file = args.next().map(PathBuf::from).ok_or_else(|| {
                adm_foundation::AdmError::invalid_input(
                    "runtime-validation-record requires results_file",
                )
            })?;
            let data_root = args
                .next()
                .map(PathBuf::from)
                .unwrap_or_else(|| default_data_root(&std::env::current_dir().unwrap()));
            let execution_text = fs::read_to_string(&results_file)?;
            let app = AdmApplication::for_data_root(&data_root)?;
            let archive = app.load_project(&archive_id)?;
            let commit = app.commit_runtime_validation_execution(&archive, &execution_text)?;
            println!("archive_id={}", archive.manifest.archive_id);
            println!("results_file={}", commit.results_file.display());
            println!("ready={}", commit.summary.ready());
            println!("runner={}", commit.summary.runner);
            println!("target_id={}", commit.summary.target_id);
            println!("contract_rows={}", commit.summary.contract_rows);
            println!("observed_rows={}", commit.summary.observed_rows);
            println!("passed_rows={}", commit.summary.passed_rows);
            println!("failed_rows={}", commit.summary.failed_rows);
            println!("missing_rows={}", commit.summary.missing_rows);
            println!("unexpected_rows={}", commit.summary.unexpected_rows);
            println!("written_files={}", commit.commit.written_files.len());
        }
        Some("stage-desktop-release") => {
            let desktop_exe = args.next().map(PathBuf::from).ok_or_else(|| {
                adm_foundation::AdmError::invalid_input(
                    "stage-desktop-release requires desktop_exe",
                )
            })?;
            let target_dir = args
                .next()
                .map(PathBuf::from)
                .unwrap_or_else(default_desktop_release_dir);
            let spec = DesktopReleaseSpec::new(desktop_exe, target_dir, env!("CARGO_PKG_VERSION"));
            let bundle = stage_desktop_release(&spec)?;
            println!("release_dir={}", bundle.target_dir.display());
            println!("executable={}", bundle.executable_path.display());
            println!("manifest={}", bundle.manifest_path.display());
            println!("readme={}", bundle.readme_path.display());
            println!("bytes={}", bundle.executable_bytes);
            println!("hash={}", bundle.executable_hash);
            println!("legacy_root_exe=not_modified");
        }
        Some("release-doctor") => {
            let target_dir = args
                .next()
                .map(PathBuf::from)
                .unwrap_or_else(default_desktop_release_dir);
            let report = inspect_desktop_release(target_dir)?;
            print!("{}", report.render());
        }
        Some("delivery-doctor") => {
            let release_dir = args
                .next()
                .map(PathBuf::from)
                .unwrap_or_else(default_desktop_release_dir);
            let game_bundle_dir = args
                .next()
                .map(PathBuf::from)
                .unwrap_or_else(default_game_build_bundle_dir);
            let sdk_bundle_dir = args
                .next()
                .map(PathBuf::from)
                .unwrap_or_else(default_sdk_bundle_dir);
            let unity_project_dir = args
                .next()
                .map(PathBuf::from)
                .unwrap_or_else(default_unity_project_dir);
            let report = inspect_delivery(
                release_dir,
                game_bundle_dir,
                sdk_bundle_dir,
                unity_project_dir,
            )?;
            print!("{}", report.render());
        }
        Some("release-acceptance") => {
            let first_arg = args.next();
            if first_arg
                .as_deref()
                .is_some_and(|arg| arg == "--help" || arg == "-h")
            {
                println!(
                    "release-acceptance [release_dir] [game_bundle_dir] [sdk_bundle_dir] [unity_project_dir]"
                );
                println!(
                    "Runs delivery doctor, executes staged AutoDesignMaker-rust.exe --smoke when delivery is ready, and writes release-acceptance.adm."
                );
                return Ok(());
            }
            let release_dir = first_arg
                .map(PathBuf::from)
                .unwrap_or_else(default_desktop_release_dir);
            let game_bundle_dir = args
                .next()
                .map(PathBuf::from)
                .unwrap_or_else(default_game_build_bundle_dir);
            let sdk_bundle_dir = args
                .next()
                .map(PathBuf::from)
                .unwrap_or_else(default_sdk_bundle_dir);
            let unity_project_dir = args
                .next()
                .map(PathBuf::from)
                .unwrap_or_else(default_unity_project_dir);
            let report = run_release_acceptance(
                release_dir,
                game_bundle_dir,
                sdk_bundle_dir,
                unity_project_dir,
            )?;
            print!("{}", report.render());
            if !report.accepted() {
                return Err(adm_foundation::AdmError::validation(format!(
                    "release acceptance failed; report={}",
                    report.report_path.display()
                )));
            }
        }
        Some("external-acceptance") => {
            let raw_args = args.collect::<Vec<_>>();
            if raw_args
                .iter()
                .any(|arg| arg.as_str() == "--help" || arg.as_str() == "-h")
            {
                println!(
                    "external-acceptance [--require-ready] [--require-ai-invoke] [--unity-exe <path>] [release_dir] [report_path] [data_root]"
                );
                println!(
                    "Reads release-acceptance.adm, discovers Unity, verifies imported unity_playmode runtime evidence, checks non-mock AI providers, and writes external-acceptance.adm."
                );
                return Ok(());
            }

            let parsed = parse_external_acceptance_cli_args(raw_args)?;
            let app = AdmApplication::for_data_root(&parsed.data_root)?;
            let diagnostics = app.ai_diagnostics();
            let report = run_external_acceptance(
                &parsed.release_dir,
                parsed.report_path,
                &parsed.data_root,
                discover_unity_editor(parsed.unity_exe),
                external_ai_provider_acceptance(&diagnostics),
                parsed.require_ready,
                parsed.require_ai_invoke,
            )?;
            print!("{}", report.render());
            if parsed.require_ready && !report.ready() {
                let blockers = report.blockers().join(",");
                return Err(adm_foundation::AdmError::validation(format!(
                    "external acceptance is not ready; blockers={blockers}; report={}",
                    report.report_path.display()
                )));
            }
        }
        Some("handoff-status") => {
            let raw_args = args.collect::<Vec<_>>();
            if raw_args
                .iter()
                .any(|arg| arg.as_str() == "--help" || arg.as_str() == "-h")
            {
                println!("handoff-status [--require-ready] [release_dir] [report_path]");
                println!(
                    "Reads release, external, AI, source, and handoff bundle reports, then writes handoff-status.adm."
                );
                return Ok(());
            }

            let mut require_ready = false;
            let mut positional = Vec::new();
            for arg in raw_args {
                if arg == "--require-ready" {
                    require_ready = true;
                } else if arg.starts_with('-') {
                    return Err(adm_foundation::AdmError::invalid_input(format!(
                        "unknown handoff-status flag: {arg}"
                    )));
                } else {
                    positional.push(arg);
                }
            }
            if positional.len() > 2 {
                return Err(adm_foundation::AdmError::invalid_input(
                    "handoff-status accepts at most release_dir and report_path",
                ));
            }

            let release_dir = positional
                .first()
                .map(PathBuf::from)
                .unwrap_or_else(default_desktop_release_dir);
            let report_path = positional
                .get(1)
                .map(PathBuf::from)
                .unwrap_or_else(|| default_handoff_status_report_path(&release_dir));
            let report = run_handoff_status(release_dir, report_path, require_ready)?;
            print!("{}", report.render());
            if require_ready && !report.ready() {
                return Err(adm_foundation::AdmError::validation(format!(
                    "handoff status is not ready; report={}",
                    report.report_path.display()
                )));
            }
        }
        Some("stage-source-bundle") => {
            let raw_args = args.collect::<Vec<_>>();
            if raw_args
                .iter()
                .any(|arg| arg.as_str() == "--help" || arg.as_str() == "-h")
            {
                println!("stage-source-bundle [source_root] [bundle_dir] [report_path]");
                println!("Copies the Rust source tree for handoff and writes source-manifest.adm.");
                return Ok(());
            }
            if raw_args.len() > 3 {
                return Err(adm_foundation::AdmError::invalid_input(
                    "stage-source-bundle accepts at most source_root, bundle_dir, and report_path",
                ));
            }
            let source_root = raw_args
                .first()
                .map(PathBuf::from)
                .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")));
            let bundle_dir = raw_args
                .get(1)
                .map(PathBuf::from)
                .unwrap_or_else(default_source_bundle_dir);
            let report_path = raw_args
                .get(2)
                .map(PathBuf::from)
                .unwrap_or_else(default_source_manifest_report_path);
            let report = stage_source_bundle(source_root, bundle_dir, report_path)?;
            print!("{}", report.render());
        }
        Some("stage-handoff-bundle") => {
            let raw_args = args.collect::<Vec<_>>();
            if raw_args
                .iter()
                .any(|arg| arg.as_str() == "--help" || arg.as_str() == "-h")
            {
                println!("stage-handoff-bundle [dist_root] [bundle_dir] [report_path]");
                println!(
                    "Copies release, source, SDK, game, and Unity delivery outputs into a single handoff bundle."
                );
                return Ok(());
            }
            if raw_args.len() > 3 {
                return Err(adm_foundation::AdmError::invalid_input(
                    "stage-handoff-bundle accepts at most dist_root, bundle_dir, and report_path",
                ));
            }
            let dist_root = raw_args
                .first()
                .map(PathBuf::from)
                .unwrap_or_else(default_dist_root);
            let bundle_dir = raw_args
                .get(1)
                .map(PathBuf::from)
                .unwrap_or_else(default_handoff_bundle_dir);
            let report_path = raw_args
                .get(2)
                .map(PathBuf::from)
                .unwrap_or_else(default_handoff_bundle_manifest_report_path);
            let report = stage_handoff_bundle(dist_root, bundle_dir, report_path)?;
            print!("{}", report.render());
        }
        Some("sync-handoff-evidence") => {
            let raw_args = args.collect::<Vec<_>>();
            if raw_args
                .iter()
                .any(|arg| arg.as_str() == "--help" || arg.as_str() == "-h")
            {
                println!("sync-handoff-evidence [release_dir] [bundle_dir] [report_path]");
                println!(
                    "Copies current final gate reports into the handoff bundle evidence directory."
                );
                return Ok(());
            }
            if raw_args.len() > 3 {
                return Err(adm_foundation::AdmError::invalid_input(
                    "sync-handoff-evidence accepts at most release_dir, bundle_dir, and report_path",
                ));
            }
            let release_dir = raw_args
                .first()
                .map(PathBuf::from)
                .unwrap_or_else(default_desktop_release_dir);
            let bundle_dir = raw_args
                .get(1)
                .map(PathBuf::from)
                .unwrap_or_else(default_handoff_bundle_dir);
            let report_path = raw_args
                .get(2)
                .map(PathBuf::from)
                .unwrap_or_else(default_handoff_evidence_manifest_report_path);
            let report = sync_handoff_evidence(release_dir, bundle_dir, report_path)?;
            print!("{}", report.render());
        }
        Some("write-source-handoff-policy") => {
            let raw_args = args.collect::<Vec<_>>();
            if raw_args
                .iter()
                .any(|arg| arg.as_str() == "--help" || arg.as_str() == "-h")
            {
                println!("write-source-handoff-policy [release_dir] [bundle_dir] [report_path]");
                println!("Writes the source handoff policy for the generated source bundle.");
                return Ok(());
            }
            if raw_args.len() > 3 {
                return Err(adm_foundation::AdmError::invalid_input(
                    "write-source-handoff-policy accepts at most release_dir, bundle_dir, and report_path",
                ));
            }
            let release_dir = raw_args
                .first()
                .map(PathBuf::from)
                .unwrap_or_else(default_desktop_release_dir);
            let bundle_dir = raw_args
                .get(1)
                .map(PathBuf::from)
                .unwrap_or_else(default_handoff_bundle_dir);
            let report_path = raw_args
                .get(2)
                .map(PathBuf::from)
                .unwrap_or_else(default_source_handoff_policy_report_path);
            let report = write_source_handoff_policy(release_dir, bundle_dir, report_path)?;
            print!("{}", report.render());
        }
        Some("write-handoff-instructions") => {
            let raw_args = args.collect::<Vec<_>>();
            if raw_args
                .iter()
                .any(|arg| arg.as_str() == "--help" || arg.as_str() == "-h")
            {
                println!("write-handoff-instructions [release_dir] [report_path]");
                println!(
                    "Writes machine-readable next steps for unresolved external handoff blockers."
                );
                return Ok(());
            }
            if raw_args.len() > 2 {
                return Err(adm_foundation::AdmError::invalid_input(
                    "write-handoff-instructions accepts at most release_dir and report_path",
                ));
            }
            let release_dir = raw_args
                .first()
                .map(PathBuf::from)
                .unwrap_or_else(default_desktop_release_dir);
            let report_path = raw_args
                .get(1)
                .map(PathBuf::from)
                .unwrap_or_else(default_handoff_instructions_report_path);
            let report = write_handoff_instructions(release_dir, report_path)?;
            print!("{}", report.render());
        }
        Some("finalize-handoff-package") => {
            let raw_args = args.collect::<Vec<_>>();
            if raw_args
                .iter()
                .any(|arg| arg.as_str() == "--help" || arg.as_str() == "-h")
            {
                println!("finalize-handoff-package [bundle_dir] [report_path]");
                println!(
                    "Writes a final manifest and package hash for the complete handoff bundle."
                );
                return Ok(());
            }
            if raw_args.len() > 2 {
                return Err(adm_foundation::AdmError::invalid_input(
                    "finalize-handoff-package accepts at most bundle_dir and report_path",
                ));
            }
            let bundle_dir = raw_args
                .first()
                .map(PathBuf::from)
                .unwrap_or_else(default_handoff_bundle_dir);
            let report_path = raw_args
                .get(1)
                .map(PathBuf::from)
                .unwrap_or_else(default_final_handoff_manifest_report_path);
            let report = finalize_handoff_package(bundle_dir, report_path)?;
            print!("{}", report.render());
        }
        Some("stage-game-build-bundle") => {
            let archive_id = args.next().ok_or_else(|| {
                adm_foundation::AdmError::invalid_input(
                    "stage-game-build-bundle requires archive_id",
                )
            })?;
            let target_id = args.next().ok_or_else(|| {
                adm_foundation::AdmError::invalid_input(
                    "stage-game-build-bundle requires target_id",
                )
            })?;
            let target_dir = args.next().map(PathBuf::from).ok_or_else(|| {
                adm_foundation::AdmError::invalid_input(
                    "stage-game-build-bundle requires target_dir",
                )
            })?;
            let data_root = args
                .next()
                .map(PathBuf::from)
                .unwrap_or_else(|| default_data_root(&std::env::current_dir().unwrap()));
            let app = AdmApplication::for_data_root(&data_root)?;
            let archive = app.load_project(&archive_id)?;
            let plan =
                GameBuildPlan::windows_desktop_prototype(archive.manifest.project_id.clone());
            let bundle = stage_game_build_bundle(
                &plan,
                &target_id,
                archive.root.join("content"),
                target_dir,
            )?;
            println!("target_id={}", bundle.target_id);
            println!("bundle_dir={}", bundle.target_dir.display());
            println!("manifest={}", bundle.manifest_path.display());
            println!("staged_files={}", bundle.staged_files.len());
            println!("bytes={}", bundle.total_bytes);
            println!("hash={}", bundle.bundle_hash);
        }
        Some("stage-sdk-bundle") => {
            let archive_id = args.next().ok_or_else(|| {
                adm_foundation::AdmError::invalid_input("stage-sdk-bundle requires archive_id")
            })?;
            let target_dir = args.next().map(PathBuf::from).ok_or_else(|| {
                adm_foundation::AdmError::invalid_input("stage-sdk-bundle requires target_dir")
            })?;
            let data_root = args
                .next()
                .map(PathBuf::from)
                .unwrap_or_else(|| default_data_root(&std::env::current_dir().unwrap()));
            let app = AdmApplication::for_data_root(&data_root)?;
            let archive = app.load_project(&archive_id)?;
            let bundle = stage_sdk_bundle(archive.root.join("content"), target_dir)?;
            println!("bundle_dir={}", bundle.target_dir.display());
            println!("manifest={}", bundle.manifest_path.display());
            println!("staged_files={}", bundle.staged_files.len());
            println!("bytes={}", bundle.total_bytes);
            println!("hash={}", bundle.bundle_hash);
        }
        Some("stage-unity-project") => {
            let archive_id = args.next().ok_or_else(|| {
                adm_foundation::AdmError::invalid_input("stage-unity-project requires archive_id")
            })?;
            let target_id = args.next().ok_or_else(|| {
                adm_foundation::AdmError::invalid_input("stage-unity-project requires target_id")
            })?;
            let unity_project_dir = args.next().map(PathBuf::from).ok_or_else(|| {
                adm_foundation::AdmError::invalid_input(
                    "stage-unity-project requires unity_project_dir",
                )
            })?;
            let data_root = args
                .next()
                .map(PathBuf::from)
                .unwrap_or_else(|| default_data_root(&std::env::current_dir().unwrap()));
            let app = AdmApplication::for_data_root(&data_root)?;
            let archive = app.load_project(&archive_id)?;
            let plan =
                GameBuildPlan::windows_desktop_prototype(archive.manifest.project_id.clone());
            let target = plan
                .targets
                .iter()
                .find(|target| target.target_id == target_id)
                .ok_or_else(|| {
                    adm_foundation::AdmError::invalid_input(format!(
                        "unknown game build target: {target_id}"
                    ))
                })?;
            let scaffold = stage_unity_project_scaffold(
                target,
                archive.root.join("content"),
                unity_project_dir,
            )?;
            println!("target_id={}", target.target_id);
            println!("project_dir={}", scaffold.project_dir.display());
            println!("manifest={}", scaffold.manifest_path.display());
            println!("generated_files={}", scaffold.generated_files.len());
            println!("bytes={}", scaffold.total_bytes);
            println!("hash={}", scaffold.scaffold_hash);
        }
        Some("unity-doctor") => {
            let explicit = args.next().map(PathBuf::from);
            let report = discover_unity_editor(explicit);
            print!("{}", report.render());
        }
        Some("unity-build-preflight") => {
            let archive_id = args.next().ok_or_else(|| {
                adm_foundation::AdmError::invalid_input("unity-build-preflight requires archive_id")
            })?;
            let target_id = args.next().ok_or_else(|| {
                adm_foundation::AdmError::invalid_input("unity-build-preflight requires target_id")
            })?;
            let unity_exe = args.next().map(PathBuf::from).ok_or_else(|| {
                adm_foundation::AdmError::invalid_input("unity-build-preflight requires unity_exe")
            })?;
            let unity_project_dir = args.next().map(PathBuf::from).ok_or_else(|| {
                adm_foundation::AdmError::invalid_input(
                    "unity-build-preflight requires unity_project_dir",
                )
            })?;
            let confirm_token = args.next().ok_or_else(|| {
                adm_foundation::AdmError::invalid_input(
                    "unity-build-preflight requires confirmation token or none",
                )
            })?;
            let data_root = args
                .next()
                .map(PathBuf::from)
                .unwrap_or_else(|| default_data_root(&std::env::current_dir().unwrap()));
            let app = AdmApplication::for_data_root(&data_root)?;
            let archive = app.load_project(&archive_id)?;
            let plan =
                GameBuildPlan::windows_desktop_prototype(archive.manifest.project_id.clone());
            let target = plan
                .targets
                .iter()
                .find(|target| target.target_id == target_id)
                .ok_or_else(|| {
                    adm_foundation::AdmError::invalid_input(format!(
                        "unknown game build target: {target_id}"
                    ))
                })?;
            let report = inspect_unity_build_preflight(
                target,
                unity_exe,
                unity_project_dir,
                &confirm_token,
            )?;
            print!("{}", report.render());
        }
        Some("plan-unity-build") => {
            let archive_id = args.next().ok_or_else(|| {
                adm_foundation::AdmError::invalid_input("plan-unity-build requires archive_id")
            })?;
            let target_id = args.next().ok_or_else(|| {
                adm_foundation::AdmError::invalid_input("plan-unity-build requires target_id")
            })?;
            let unity_exe = args.next().map(PathBuf::from).ok_or_else(|| {
                adm_foundation::AdmError::invalid_input("plan-unity-build requires unity_exe")
            })?;
            let unity_project_dir = args.next().map(PathBuf::from).ok_or_else(|| {
                adm_foundation::AdmError::invalid_input(
                    "plan-unity-build requires unity_project_dir",
                )
            })?;
            let data_root = args
                .next()
                .map(PathBuf::from)
                .unwrap_or_else(|| default_data_root(&std::env::current_dir().unwrap()));
            let app = AdmApplication::for_data_root(&data_root)?;
            let archive = app.load_project(&archive_id)?;
            let plan =
                GameBuildPlan::windows_desktop_prototype(archive.manifest.project_id.clone());
            let target = plan
                .targets
                .iter()
                .find(|target| target.target_id == target_id)
                .ok_or_else(|| {
                    adm_foundation::AdmError::invalid_input(format!(
                        "unknown game build target: {target_id}"
                    ))
                })?;
            let command = plan_unity_cli_build(target, unity_exe, unity_project_dir)?;
            print!("{}", command.render());
            println!("command_line={}", command.command_line());
        }
        Some("dry-run-unity-build") => {
            let archive_id = args.next().ok_or_else(|| {
                adm_foundation::AdmError::invalid_input("dry-run-unity-build requires archive_id")
            })?;
            let target_id = args.next().ok_or_else(|| {
                adm_foundation::AdmError::invalid_input("dry-run-unity-build requires target_id")
            })?;
            let unity_exe = args.next().map(PathBuf::from).ok_or_else(|| {
                adm_foundation::AdmError::invalid_input("dry-run-unity-build requires unity_exe")
            })?;
            let unity_project_dir = args.next().map(PathBuf::from).ok_or_else(|| {
                adm_foundation::AdmError::invalid_input(
                    "dry-run-unity-build requires unity_project_dir",
                )
            })?;
            let data_root = args
                .next()
                .map(PathBuf::from)
                .unwrap_or_else(|| default_data_root(&std::env::current_dir().unwrap()));
            let app = AdmApplication::for_data_root(&data_root)?;
            let archive = app.load_project(&archive_id)?;
            let plan =
                GameBuildPlan::windows_desktop_prototype(archive.manifest.project_id.clone());
            let target = plan
                .targets
                .iter()
                .find(|target| target.target_id == target_id)
                .ok_or_else(|| {
                    adm_foundation::AdmError::invalid_input(format!(
                        "unknown game build target: {target_id}"
                    ))
                })?;
            let command = plan_unity_cli_build(target, unity_exe, unity_project_dir)?;
            let report = DryRunEngineBuildRunner.run(&command)?;
            print!("{}", report.render());
            let history = app.commit_engine_build_execution(&archive, &report)?;
            println!("history_file={}", history.history_file.display());
            println!("history_records={}", history.record_count);
            println!(
                "history_commit_files={}",
                history.commit.written_files.len()
            );
        }
        Some("run-unity-build") => {
            let archive_id = args.next().ok_or_else(|| {
                adm_foundation::AdmError::invalid_input("run-unity-build requires archive_id")
            })?;
            let target_id = args.next().ok_or_else(|| {
                adm_foundation::AdmError::invalid_input("run-unity-build requires target_id")
            })?;
            let unity_exe = args.next().map(PathBuf::from).ok_or_else(|| {
                adm_foundation::AdmError::invalid_input("run-unity-build requires unity_exe")
            })?;
            let unity_project_dir = args.next().map(PathBuf::from).ok_or_else(|| {
                adm_foundation::AdmError::invalid_input(
                    "run-unity-build requires unity_project_dir",
                )
            })?;
            let confirm_token = args.next().ok_or_else(|| {
                adm_foundation::AdmError::invalid_input(format!(
                    "run-unity-build requires confirmation token {LOCAL_ENGINE_BUILD_CONFIRMATION_TOKEN}"
                ))
            })?;
            validate_local_engine_build_confirmation(&confirm_token)?;
            let data_root = args
                .next()
                .map(PathBuf::from)
                .unwrap_or_else(|| default_data_root(&std::env::current_dir().unwrap()));
            let app = AdmApplication::for_data_root(&data_root)?;
            let archive = app.load_project(&archive_id)?;
            let plan =
                GameBuildPlan::windows_desktop_prototype(archive.manifest.project_id.clone());
            let target = plan
                .targets
                .iter()
                .find(|target| target.target_id == target_id)
                .ok_or_else(|| {
                    adm_foundation::AdmError::invalid_input(format!(
                        "unknown game build target: {target_id}"
                    ))
                })?;
            let preflight = inspect_unity_build_preflight(
                target,
                &unity_exe,
                &unity_project_dir,
                &confirm_token,
            )?;
            if !preflight.ready_for_local_build() {
                return Err(adm_foundation::AdmError::validation(format!(
                    "Unity build preflight failed\n{}",
                    preflight.render()
                )));
            }
            let command = plan_unity_cli_build(target, unity_exe, unity_project_dir)?;
            let report = LocalProcessEngineBuildRunner.run(&command)?;
            print!("{}", report.render());
            let history = app.commit_engine_build_execution(&archive, &report)?;
            println!("history_file={}", history.history_file.display());
            println!("history_records={}", history.record_count);
            println!(
                "history_commit_files={}",
                history.commit.written_files.len()
            );
        }
        Some("plan-unity-runtime-validation") => {
            let archive_id = args.next().ok_or_else(|| {
                adm_foundation::AdmError::invalid_input(
                    "plan-unity-runtime-validation requires archive_id",
                )
            })?;
            let target_id = args.next().ok_or_else(|| {
                adm_foundation::AdmError::invalid_input(
                    "plan-unity-runtime-validation requires target_id",
                )
            })?;
            let unity_exe = args.next().map(PathBuf::from).ok_or_else(|| {
                adm_foundation::AdmError::invalid_input(
                    "plan-unity-runtime-validation requires unity_exe",
                )
            })?;
            let unity_project_dir = args.next().map(PathBuf::from).ok_or_else(|| {
                adm_foundation::AdmError::invalid_input(
                    "plan-unity-runtime-validation requires unity_project_dir",
                )
            })?;
            let data_root = args
                .next()
                .map(PathBuf::from)
                .unwrap_or_else(|| default_data_root(&std::env::current_dir().unwrap()));
            let app = AdmApplication::for_data_root(&data_root)?;
            let archive = app.load_project(&archive_id)?;
            let plan =
                GameBuildPlan::windows_desktop_prototype(archive.manifest.project_id.clone());
            let target = plan
                .targets
                .iter()
                .find(|target| target.target_id == target_id)
                .ok_or_else(|| {
                    adm_foundation::AdmError::invalid_input(format!(
                        "unknown game build target: {target_id}"
                    ))
                })?;
            let command = plan_unity_runtime_validation(target, unity_exe, unity_project_dir)?;
            print!("{}", command.render());
            println!("command_line={}", command.command_line());
        }
        Some("dry-run-unity-runtime-validation") => {
            let archive_id = args.next().ok_or_else(|| {
                adm_foundation::AdmError::invalid_input(
                    "dry-run-unity-runtime-validation requires archive_id",
                )
            })?;
            let target_id = args.next().ok_or_else(|| {
                adm_foundation::AdmError::invalid_input(
                    "dry-run-unity-runtime-validation requires target_id",
                )
            })?;
            let unity_exe = args.next().map(PathBuf::from).ok_or_else(|| {
                adm_foundation::AdmError::invalid_input(
                    "dry-run-unity-runtime-validation requires unity_exe",
                )
            })?;
            let unity_project_dir = args.next().map(PathBuf::from).ok_or_else(|| {
                adm_foundation::AdmError::invalid_input(
                    "dry-run-unity-runtime-validation requires unity_project_dir",
                )
            })?;
            let data_root = args
                .next()
                .map(PathBuf::from)
                .unwrap_or_else(|| default_data_root(&std::env::current_dir().unwrap()));
            let app = AdmApplication::for_data_root(&data_root)?;
            let archive = app.load_project(&archive_id)?;
            let plan =
                GameBuildPlan::windows_desktop_prototype(archive.manifest.project_id.clone());
            let target = plan
                .targets
                .iter()
                .find(|target| target.target_id == target_id)
                .ok_or_else(|| {
                    adm_foundation::AdmError::invalid_input(format!(
                        "unknown game build target: {target_id}"
                    ))
                })?;
            let command = plan_unity_runtime_validation(target, unity_exe, unity_project_dir)?;
            let report = DryRunEngineBuildRunner.run(&command)?;
            print!("{}", report.render());
        }
        Some("run-unity-runtime-validation") => {
            let archive_id = args.next().ok_or_else(|| {
                adm_foundation::AdmError::invalid_input(
                    "run-unity-runtime-validation requires archive_id",
                )
            })?;
            let target_id = args.next().ok_or_else(|| {
                adm_foundation::AdmError::invalid_input(
                    "run-unity-runtime-validation requires target_id",
                )
            })?;
            let unity_exe = args.next().map(PathBuf::from).ok_or_else(|| {
                adm_foundation::AdmError::invalid_input(
                    "run-unity-runtime-validation requires unity_exe",
                )
            })?;
            let unity_project_dir = args.next().map(PathBuf::from).ok_or_else(|| {
                adm_foundation::AdmError::invalid_input(
                    "run-unity-runtime-validation requires unity_project_dir",
                )
            })?;
            let confirm_token = args.next().ok_or_else(|| {
                adm_foundation::AdmError::invalid_input(format!(
                    "run-unity-runtime-validation requires confirmation token {LOCAL_ENGINE_BUILD_CONFIRMATION_TOKEN}"
                ))
            })?;
            validate_local_engine_build_confirmation(&confirm_token)?;
            let data_root = args
                .next()
                .map(PathBuf::from)
                .unwrap_or_else(|| default_data_root(&std::env::current_dir().unwrap()));
            let app = AdmApplication::for_data_root(&data_root)?;
            let archive = app.load_project(&archive_id)?;
            let plan =
                GameBuildPlan::windows_desktop_prototype(archive.manifest.project_id.clone());
            let target = plan
                .targets
                .iter()
                .find(|target| target.target_id == target_id)
                .ok_or_else(|| {
                    adm_foundation::AdmError::invalid_input(format!(
                        "unknown game build target: {target_id}"
                    ))
                })?;
            let preflight = inspect_unity_build_preflight(
                target,
                &unity_exe,
                &unity_project_dir,
                &confirm_token,
            )?;
            if !preflight.ready_for_local_build() {
                return Err(adm_foundation::AdmError::validation(format!(
                    "Unity runtime validation preflight failed\n{}",
                    preflight.render()
                )));
            }
            let command = plan_unity_runtime_validation(target, unity_exe, unity_project_dir)?;
            let report = LocalProcessEngineBuildRunner.run(&command)?;
            print!("{}", report.render());
            if report.status != EngineBuildExecutionStatus::Succeeded {
                return Err(adm_foundation::AdmError::validation(
                    "Unity runtime validation did not produce the expected output",
                ));
            }
            let execution_text = fs::read_to_string(&report.expected_output_path)?;
            let commit = app.commit_runtime_validation_execution(&archive, &execution_text)?;
            println!("runtime_results_file={}", commit.results_file.display());
            println!("runtime_ready={}", commit.summary.ready());
            println!("runtime_runner={}", commit.summary.runner);
            println!("runtime_contract_rows={}", commit.summary.contract_rows);
            println!("runtime_observed_rows={}", commit.summary.observed_rows);
            println!("runtime_passed_rows={}", commit.summary.passed_rows);
            println!("runtime_failed_rows={}", commit.summary.failed_rows);
            println!("runtime_missing_rows={}", commit.summary.missing_rows);
            println!("runtime_unexpected_rows={}", commit.summary.unexpected_rows);
            println!("runtime_commit_files={}", commit.commit.written_files.len());
        }
        _ => {
            println!("AutoDesignMaker Rust CLI");
            println!("commands:");
            println!("  --version");
            println!("  doctor");
            println!("  ai-doctor [data_root]");
            println!("  ai-secret-set <name> <secret> [data_root]");
            println!("  ai-journal <archive_id> [data_root]");
            println!("  ai-provider-presets");
            println!(
                "  ai-provider-preset <preset_id> <provider_id> <secret_ref|default|none> [data_root]"
            );
            println!(
                "  ai-provider-set <provider_id> <endpoint_hint|none> <secret_ref|none> [data_root]"
            );
            println!("  ai-provider-disable <provider_id> [data_root]");
            println!("  ai-provider-check <provider_id> <model> [data_root]");
            println!("  ai-provider-invoke <provider_id> <model> <prompt> [data_root]");
            println!(
                "  ai-acceptance [--invoke] [--require-ready] [--require-invoke] <provider_id> <model> [report_path] [data_root]"
            );
            println!("  demo-core [title] [data_root]");
            println!("  run-core <title> <genre> <player_promise> <core_loop_steps> [data_root]");
            println!("  rerun-stage <archive_id> <stage_id> [data_root]");
            println!("  resume-failed <archive_id> [data_root]");
            println!("  list [data_root]");
            println!("  export <archive_id> <target_file> [data_root]");
            println!("  import <package_file> [data_root]");
            println!("  runtime-validation-record <archive_id> <results_file> [data_root]");
            println!("  stage-desktop-release <desktop_exe> [target_dir]");
            println!("  release-doctor [release_dir]");
            println!(
                "  delivery-doctor [release_dir] [game_bundle_dir] [sdk_bundle_dir] [unity_project_dir]"
            );
            println!(
                "  release-acceptance [release_dir] [game_bundle_dir] [sdk_bundle_dir] [unity_project_dir]"
            );
            println!(
                "  external-acceptance [--require-ready] [--require-ai-invoke] [--unity-exe <path>] [release_dir] [report_path] [data_root]"
            );
            println!("  handoff-status [--require-ready] [release_dir] [report_path]");
            println!("  stage-source-bundle [source_root] [bundle_dir] [report_path]");
            println!("  stage-handoff-bundle [dist_root] [bundle_dir] [report_path]");
            println!("  sync-handoff-evidence [release_dir] [bundle_dir] [report_path]");
            println!("  write-source-handoff-policy [release_dir] [bundle_dir] [report_path]");
            println!("  write-handoff-instructions [release_dir] [report_path]");
            println!("  finalize-handoff-package [bundle_dir] [report_path]");
            println!("  stage-game-build-bundle <archive_id> <target_id> <target_dir> [data_root]");
            println!("  stage-sdk-bundle <archive_id> <target_dir> [data_root]");
            println!(
                "  stage-unity-project <archive_id> <target_id> <unity_project_dir> [data_root]"
            );
            println!("  unity-doctor [unity_exe]");
            println!(
                "  unity-build-preflight <archive_id> <target_id> <unity_exe> <unity_project_dir> <confirm_token|none> [data_root]"
            );
            println!(
                "  plan-unity-build <archive_id> <target_id> <unity_exe> <unity_project_dir> [data_root]"
            );
            println!(
                "  dry-run-unity-build <archive_id> <target_id> <unity_exe> <unity_project_dir> [data_root]"
            );
            println!(
                "  run-unity-build <archive_id> <target_id> <unity_exe> <unity_project_dir> {LOCAL_ENGINE_BUILD_CONFIRMATION_TOKEN} [data_root]"
            );
            println!(
                "  plan-unity-runtime-validation <archive_id> <target_id> <unity_exe> <unity_project_dir> [data_root]"
            );
            println!(
                "  dry-run-unity-runtime-validation <archive_id> <target_id> <unity_exe> <unity_project_dir> [data_root]"
            );
            println!(
                "  run-unity-runtime-validation <archive_id> <target_id> <unity_exe> <unity_project_dir> {LOCAL_ENGINE_BUILD_CONFIRMATION_TOKEN} [data_root]"
            );
        }
    }
    Ok(())
}

fn ai_provider_config_from_cli(
    provider_id: String,
    endpoint_hint: String,
    secret_ref: String,
) -> AdmResult<AiProviderConfig> {
    let provider_id = ProviderId::new(provider_id)?;
    let endpoint_hint = optional_cli_value(endpoint_hint);
    let secret_ref = optional_cli_value(secret_ref)
        .map(SecretRef::new)
        .transpose()?;
    let requires_secret = secret_ref.is_some();
    let provider = AiProviderConfig {
        display_name: Some(provider_id.as_str().to_string()),
        provider_id,
        enabled: true,
        endpoint_hint,
        secret_ref,
        requires_secret,
        capabilities: vec![AiCapability::TextGeneration],
    };
    provider.validate()?;
    Ok(provider)
}

fn preset_secret_ref_from_cli(
    preset: &adm_config::AiProviderPreset,
    value: String,
) -> AdmResult<Option<SecretRef>> {
    let trimmed = value.trim();
    if trimmed.eq_ignore_ascii_case("default") {
        default_secret_ref_for_preset(preset)
    } else if trimmed.eq_ignore_ascii_case("none") || trimmed.is_empty() {
        Ok(None)
    } else {
        SecretRef::new(trimmed).map(Some)
    }
}

fn optional_cli_value(value: String) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() || trimmed.eq_ignore_ascii_case("none") {
        None
    } else {
        Some(trimmed.to_string())
    }
}

fn cli_capabilities() -> Vec<AiCapability> {
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

fn external_ai_provider_acceptance(
    diagnostics: &adm_application::AiDiagnosticsReport,
) -> ExternalAiProviderAcceptance {
    let real_provider_ids = diagnostics
        .providers
        .iter()
        .filter(|provider| {
            provider.readiness == AiProviderReadiness::Ready
                && !provider.provider_id.eq_ignore_ascii_case("mock")
        })
        .map(|provider| provider.provider_id.clone())
        .collect();
    ExternalAiProviderAcceptance::new(
        diagnostics.ready_provider_count(),
        real_provider_ids,
        diagnostics.render(),
    )
}

#[derive(Debug, Clone)]
struct AiAcceptanceReport {
    report_path: PathBuf,
    data_root: PathBuf,
    provider_id: String,
    model: String,
    provider_is_mock: bool,
    diagnostic_readiness: String,
    configured_ready: bool,
    supports_text_generation: bool,
    network_call: bool,
    invoke_attempted: bool,
    invoke_succeeded: bool,
    require_ready: bool,
    output_state: String,
    raw_output_bytes: usize,
    validation_notes_count: usize,
    error: Option<String>,
    diagnostics_document: String,
}

impl AiAcceptanceReport {
    fn ready(&self) -> bool {
        self.configured_ready
            && !self.provider_is_mock
            && self.supports_text_generation
            && (!self.invoke_attempted || self.invoke_succeeded)
            && self.error.is_none()
    }

    fn render(&self) -> String {
        let mut document = String::from("# AI Provider Acceptance\n");
        document.push_str(&format!("ready={}\n", self.ready()));
        document.push_str(&format!("report_path={}\n", self.report_path.display()));
        document.push_str(&format!("data_root={}\n", self.data_root.display()));
        document.push_str(&format!("provider_id={}\n", self.provider_id));
        document.push_str(&format!("model={}\n", single_line_cli_value(&self.model)));
        document.push_str(&format!("provider_is_mock={}\n", self.provider_is_mock));
        document.push_str(&format!(
            "diagnostic_readiness={}\n",
            self.diagnostic_readiness
        ));
        document.push_str(&format!("configured_ready={}\n", self.configured_ready));
        document.push_str(&format!(
            "supports.text_generation={}\n",
            self.supports_text_generation
        ));
        document.push_str(&format!("network_call={}\n", self.network_call));
        document.push_str(&format!("invoke_attempted={}\n", self.invoke_attempted));
        document.push_str(&format!("invoke_succeeded={}\n", self.invoke_succeeded));
        document.push_str(&format!("require_ready={}\n", self.require_ready));
        document.push_str(&format!("output_state={}\n", self.output_state));
        document.push_str(&format!("raw_output_bytes={}\n", self.raw_output_bytes));
        document.push_str(&format!(
            "validation_notes_count={}\n",
            self.validation_notes_count
        ));
        document.push_str(&format!(
            "error={}\n",
            self.error
                .as_deref()
                .map(single_line_cli_value)
                .unwrap_or_else(|| "none".to_string())
        ));
        document.push('\n');
        document.push_str("## AI Diagnostics\n");
        document.push_str(self.diagnostics_document.trim_end());
        document.push('\n');
        document
    }
}

fn run_ai_acceptance(
    provider_id: &str,
    model: &str,
    report_path: PathBuf,
    data_root: PathBuf,
    invoke: bool,
    require_ready: bool,
) -> AdmResult<AiAcceptanceReport> {
    let app = AdmApplication::for_data_root(&data_root)?;
    let diagnostics = app.ai_diagnostics();
    let diagnostic = diagnostics
        .providers
        .iter()
        .find(|provider| provider.provider_id == provider_id);
    let diagnostic_readiness = diagnostic
        .map(|provider| format!("{:?}", provider.readiness))
        .unwrap_or_else(|| "MissingProvider".to_string());
    let configured_ready = diagnostic
        .map(|provider| provider.readiness == AiProviderReadiness::Ready)
        .unwrap_or(false);
    let provider_is_mock = provider_id.eq_ignore_ascii_case("mock");

    let provider_id_value = ProviderId::new(provider_id.to_string())?;
    let mut supports_text_generation = false;
    let mut invoke_succeeded = false;
    let mut output_state = "not_attempted".to_string();
    let mut raw_output_bytes = 0usize;
    let mut validation_notes_count = 0usize;
    let mut error = None;

    match app.chat_completions_provider_from_config(&provider_id_value, model.to_string()) {
        Ok(provider) => {
            supports_text_generation = provider.supports(&AiCapability::TextGeneration);
            if invoke {
                let request = AiTaskRequest::new(
                    AiCapability::TextGeneration,
                    "Reply with a concise AutoDesignMaker provider acceptance confirmation.",
                    "AI provider acceptance gate",
                )?;
                match provider.run(&request) {
                    Ok(result) => {
                        output_state = result.output_state.as_str().to_string();
                        raw_output_bytes = result.raw_output.len();
                        validation_notes_count = result.validation_notes.len();
                        invoke_succeeded = !result.raw_output.trim().is_empty();
                    }
                    Err(run_error) => {
                        error = Some(run_error.to_string());
                    }
                }
            }
        }
        Err(config_error) => {
            error = Some(config_error.to_string());
        }
    }

    let report = AiAcceptanceReport {
        report_path,
        data_root,
        provider_id: provider_id.to_string(),
        model: model.to_string(),
        provider_is_mock,
        diagnostic_readiness,
        configured_ready,
        supports_text_generation,
        network_call: invoke,
        invoke_attempted: invoke,
        invoke_succeeded,
        require_ready,
        output_state,
        raw_output_bytes,
        validation_notes_count,
        error,
        diagnostics_document: diagnostics.render(),
    };
    write_ai_acceptance_report(&report)?;
    Ok(report)
}

fn write_ai_acceptance_report(report: &AiAcceptanceReport) -> AdmResult<()> {
    if let Some(parent) = report
        .report_path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        fs::create_dir_all(parent)?;
    }
    fs::write(&report.report_path, report.render())?;
    Ok(())
}

#[derive(Debug, Clone)]
struct AcceptanceSnapshot {
    path: PathBuf,
    present: bool,
    text: String,
}

impl AcceptanceSnapshot {
    fn load(path: PathBuf) -> AdmResult<Self> {
        if path.exists() {
            let text = fs::read_to_string(&path)?;
            Ok(Self {
                path,
                present: true,
                text,
            })
        } else {
            Ok(Self {
                path,
                present: false,
                text: String::new(),
            })
        }
    }

    fn value(&self, key: &str) -> Option<String> {
        report_value(&self.text, key)
    }

    fn bool_value(&self, key: &str) -> bool {
        self.value(key)
            .is_some_and(|value| value.eq_ignore_ascii_case("true"))
    }
}

#[derive(Debug, Clone)]
struct HandoffStatusReport {
    report_path: PathBuf,
    release_dir: PathBuf,
    release_acceptance: AcceptanceSnapshot,
    external_acceptance: AcceptanceSnapshot,
    ai_acceptance: AcceptanceSnapshot,
    source_manifest: AcceptanceSnapshot,
    handoff_bundle_manifest: AcceptanceSnapshot,
    require_ready: bool,
    release_accepted: bool,
    release_delivery_ready: bool,
    release_ready: bool,
    release_smoke_ready: bool,
    release_hash: String,
    external_ready: bool,
    external_acceptance_data_root: String,
    unity_ready: bool,
    unity_runtime_report: String,
    unity_runtime_present: bool,
    unity_runtime_ready: bool,
    unity_runtime_runner: String,
    real_ai_provider_ready: bool,
    ai_ready: bool,
    ai_acceptance_data_root: String,
    ai_acceptance_provider_matches_real_provider: bool,
    ai_provider_id: String,
    ai_configured_ready: bool,
    ai_invoke_attempted: bool,
    ai_invoke_succeeded: bool,
    external_acceptance_require_ai_invoke: bool,
    source_ready: bool,
    source_handoff_mode: String,
    source_file_count: String,
    source_bundle_hash: String,
    handoff_bundle_ready: bool,
    handoff_bundle_dir: String,
    handoff_bundle_file_count: String,
    handoff_bundle_hash: String,
    blockers: Vec<String>,
}

impl HandoffStatusReport {
    fn local_release_ready(&self) -> bool {
        self.release_acceptance.present
            && self.release_accepted
            && self.release_delivery_ready
            && self.release_ready
            && self.release_smoke_ready
    }

    fn ready(&self) -> bool {
        self.local_release_ready()
            && self.external_ready
            && !self.ai_acceptance_data_root_mismatch()
            && self.unity_ready
            && self.unity_runtime_present
            && self.unity_runtime_ready
            && self.unity_runtime_runner == "unity_playmode"
            && self.real_ai_provider_ready
            && self.ai_ready
            && !self.ai_acceptance_provider_match_blocking()
            && !self.ai_invoke_requirement_blocking()
            && self.source_ready
            && self.handoff_bundle_ready
    }

    fn render(&self) -> String {
        let mut document = String::from("# Handoff Status\n");
        document.push_str(&format!("ready={}\n", self.ready()));
        document.push_str(&format!("report_path={}\n", self.report_path.display()));
        document.push_str(&format!("release_dir={}\n", self.release_dir.display()));
        document.push_str(&format!(
            "release_acceptance_report={}\n",
            self.release_acceptance.path.display()
        ));
        document.push_str(&format!(
            "release_acceptance_present={}\n",
            self.release_acceptance.present
        ));
        document.push_str(&format!(
            "local_release_ready={}\n",
            self.local_release_ready()
        ));
        document.push_str(&format!("release_accepted={}\n", self.release_accepted));
        document.push_str(&format!(
            "release_delivery_ready={}\n",
            self.release_delivery_ready
        ));
        document.push_str(&format!("release_ready={}\n", self.release_ready));
        document.push_str(&format!(
            "release_smoke_ready={}\n",
            self.release_smoke_ready
        ));
        document.push_str(&format!("release_hash={}\n", self.release_hash));
        document.push_str(&format!(
            "external_acceptance_report={}\n",
            self.external_acceptance.path.display()
        ));
        document.push_str(&format!(
            "external_acceptance_present={}\n",
            self.external_acceptance.present
        ));
        document.push_str(&format!(
            "external_acceptance_ready={}\n",
            self.external_ready
        ));
        document.push_str(&format!(
            "external_acceptance_data_root={}\n",
            self.external_acceptance_data_root
        ));
        document.push_str(&format!("unity_ready={}\n", self.unity_ready));
        document.push_str(&format!(
            "unity_runtime_report={}\n",
            self.unity_runtime_report
        ));
        document.push_str(&format!(
            "unity_runtime_present={}\n",
            self.unity_runtime_present
        ));
        document.push_str(&format!(
            "unity_runtime_ready={}\n",
            self.unity_runtime_ready
        ));
        document.push_str(&format!(
            "unity_runtime_runner={}\n",
            self.unity_runtime_runner
        ));
        document.push_str(&format!(
            "real_ai_provider_ready={}\n",
            self.real_ai_provider_ready
        ));
        document.push_str(&format!(
            "ai_acceptance_report={}\n",
            self.ai_acceptance.path.display()
        ));
        document.push_str(&format!(
            "ai_acceptance_present={}\n",
            self.ai_acceptance.present
        ));
        document.push_str(&format!("ai_acceptance_ready={}\n", self.ai_ready));
        document.push_str(&format!(
            "ai_acceptance_data_root={}\n",
            self.ai_acceptance_data_root
        ));
        document.push_str(&format!(
            "ai_acceptance_data_root_matches_external={}\n",
            !self.ai_acceptance_data_root_mismatch()
        ));
        document.push_str(&format!(
            "ai_acceptance_provider_matches_real_provider={}\n",
            self.ai_acceptance_provider_matches_real_provider
        ));
        document.push_str(&format!("ai_provider_id={}\n", self.ai_provider_id));
        document.push_str(&format!(
            "ai_configured_ready={}\n",
            self.ai_configured_ready
        ));
        document.push_str(&format!(
            "ai_invoke_attempted={}\n",
            self.ai_invoke_attempted
        ));
        document.push_str(&format!(
            "ai_invoke_succeeded={}\n",
            self.ai_invoke_succeeded
        ));
        document.push_str(&format!(
            "external_acceptance_require_ai_invoke={}\n",
            self.external_acceptance_require_ai_invoke
        ));
        document.push_str(&format!(
            "source_manifest_report={}\n",
            self.source_manifest.path.display()
        ));
        document.push_str(&format!(
            "source_manifest_present={}\n",
            self.source_manifest.present
        ));
        document.push_str(&format!("source_ready={}\n", self.source_ready));
        document.push_str(&format!(
            "source_handoff_mode={}\n",
            self.source_handoff_mode
        ));
        document.push_str(&format!("source_file_count={}\n", self.source_file_count));
        document.push_str(&format!("source_bundle_hash={}\n", self.source_bundle_hash));
        document.push_str(&format!(
            "handoff_bundle_manifest_report={}\n",
            self.handoff_bundle_manifest.path.display()
        ));
        document.push_str(&format!(
            "handoff_bundle_manifest_present={}\n",
            self.handoff_bundle_manifest.present
        ));
        document.push_str(&format!(
            "handoff_bundle_ready={}\n",
            self.handoff_bundle_ready
        ));
        document.push_str(&format!("handoff_bundle_dir={}\n", self.handoff_bundle_dir));
        document.push_str(&format!(
            "handoff_bundle_file_count={}\n",
            self.handoff_bundle_file_count
        ));
        document.push_str(&format!(
            "handoff_bundle_hash={}\n",
            self.handoff_bundle_hash
        ));
        document.push_str(&format!("require_ready={}\n", self.require_ready));
        document.push_str(&format!("blocker_count={}\n", self.blockers.len()));
        for blocker in &self.blockers {
            document.push_str(&format!("blocker={}\n", blocker));
        }
        document
    }

    fn ai_acceptance_data_root_mismatch(&self) -> bool {
        self.external_acceptance_data_root != "none"
            && self.ai_acceptance_data_root != "none"
            && self.external_acceptance_data_root != self.ai_acceptance_data_root
    }

    fn ai_acceptance_provider_match_blocking(&self) -> bool {
        self.real_ai_provider_ready
            && self.ai_ready
            && self.ai_configured_ready
            && !self.ai_acceptance_provider_matches_real_provider
    }

    fn ai_invoke_requirement_blocking(&self) -> bool {
        self.external_acceptance_require_ai_invoke
            && (!self.ai_invoke_attempted || !self.ai_invoke_succeeded)
    }
}

fn run_handoff_status(
    release_dir: PathBuf,
    report_path: PathBuf,
    require_ready: bool,
) -> AdmResult<HandoffStatusReport> {
    let release_acceptance = AcceptanceSnapshot::load(release_dir.join("release-acceptance.adm"))?;
    let external_acceptance =
        AcceptanceSnapshot::load(release_dir.join("external-acceptance.adm"))?;
    let ai_acceptance = AcceptanceSnapshot::load(release_dir.join("ai-acceptance.adm"))?;
    let source_manifest = AcceptanceSnapshot::load(release_dir.join("source-manifest.adm"))?;
    let handoff_bundle_manifest =
        AcceptanceSnapshot::load(release_dir.join("handoff-bundle-manifest.adm"))?;

    let release_accepted = release_acceptance.bool_value("accepted");
    let release_delivery_ready = release_acceptance.bool_value("delivery_ready");
    let release_ready = release_acceptance.bool_value("release_ready");
    let release_smoke_ready = release_acceptance.bool_value("smoke_ready");
    let release_hash = release_acceptance
        .value("release_hash")
        .unwrap_or_else(|| "none".to_string());
    let external_ready = external_acceptance.bool_value("ready");
    let external_acceptance_data_root = external_acceptance
        .value("data_root")
        .unwrap_or_else(|| "none".to_string());
    let unity_ready = external_acceptance.bool_value("unity_ready");
    let unity_runtime_report = external_acceptance
        .value("unity_runtime_report")
        .unwrap_or_else(|| "none".to_string());
    let unity_runtime_present = external_acceptance.bool_value("unity_runtime_present");
    let unity_runtime_ready = external_acceptance.bool_value("unity_runtime_ready");
    let unity_runtime_runner = external_acceptance
        .value("unity_runtime_runner")
        .unwrap_or_else(|| "none".to_string());
    let real_ai_provider_ready = external_acceptance.bool_value("real_ai_provider_ready");
    let ai_ready = ai_acceptance.bool_value("ready");
    let ai_acceptance_data_root = ai_acceptance
        .value("data_root")
        .unwrap_or_else(|| "none".to_string());
    let ai_acceptance_provider_matches_real_provider = external_acceptance
        .value("ai_acceptance_provider_matches_real_provider")
        .map(|value| value.eq_ignore_ascii_case("true"))
        .unwrap_or(true);
    let ai_provider_id = ai_acceptance
        .value("provider_id")
        .unwrap_or_else(|| "none".to_string());
    let ai_configured_ready = ai_acceptance.bool_value("configured_ready");
    let ai_invoke_attempted = ai_acceptance.bool_value("invoke_attempted");
    let ai_invoke_succeeded = ai_acceptance.bool_value("invoke_succeeded");
    let external_acceptance_require_ai_invoke = external_acceptance.bool_value("require_ai_invoke");
    let source_ready = source_manifest.bool_value("ready");
    let source_handoff_mode = source_manifest
        .value("source_handoff_mode")
        .unwrap_or_else(|| "none".to_string());
    let source_file_count = source_manifest
        .value("file_count")
        .unwrap_or_else(|| "0".to_string());
    let source_bundle_hash = source_manifest
        .value("bundle_hash")
        .unwrap_or_else(|| "none".to_string());
    let handoff_bundle_ready = handoff_bundle_manifest.bool_value("ready");
    let handoff_bundle_dir = handoff_bundle_manifest
        .value("bundle_dir")
        .unwrap_or_else(|| "none".to_string());
    let handoff_bundle_file_count = handoff_bundle_manifest
        .value("file_count")
        .unwrap_or_else(|| "0".to_string());
    let handoff_bundle_hash = handoff_bundle_manifest
        .value("bundle_hash")
        .unwrap_or_else(|| "none".to_string());

    let mut report = HandoffStatusReport {
        report_path,
        release_dir,
        release_acceptance,
        external_acceptance,
        ai_acceptance,
        source_manifest,
        handoff_bundle_manifest,
        require_ready,
        release_accepted,
        release_delivery_ready,
        release_ready,
        release_smoke_ready,
        release_hash,
        external_ready,
        external_acceptance_data_root,
        unity_ready,
        unity_runtime_report,
        unity_runtime_present,
        unity_runtime_ready,
        unity_runtime_runner,
        real_ai_provider_ready,
        ai_ready,
        ai_acceptance_data_root,
        ai_acceptance_provider_matches_real_provider,
        ai_provider_id,
        ai_configured_ready,
        ai_invoke_attempted,
        ai_invoke_succeeded,
        external_acceptance_require_ai_invoke,
        source_ready,
        source_handoff_mode,
        source_file_count,
        source_bundle_hash,
        handoff_bundle_ready,
        handoff_bundle_dir,
        handoff_bundle_file_count,
        handoff_bundle_hash,
        blockers: Vec::new(),
    };
    report.blockers = handoff_blockers(&report);
    write_handoff_status_report(&report)?;
    Ok(report)
}

fn handoff_blockers(report: &HandoffStatusReport) -> Vec<String> {
    let mut blockers = Vec::new();
    if !report.release_acceptance.present {
        blockers.push("release_acceptance_report_missing".to_string());
    } else {
        if !report.release_accepted {
            blockers.push("release_not_accepted".to_string());
        }
        if !report.release_delivery_ready {
            blockers.push("release_delivery_not_ready".to_string());
        }
        if !report.release_ready {
            blockers.push("desktop_release_not_ready".to_string());
        }
        if !report.release_smoke_ready {
            blockers.push("release_smoke_not_ready".to_string());
        }
    }

    if !report.external_acceptance.present {
        blockers.push("external_acceptance_report_missing".to_string());
    } else {
        if !report.external_ready {
            blockers.push("external_acceptance_not_ready".to_string());
        }
        if !report.unity_ready {
            blockers.push("unity_not_ready".to_string());
        }
        if !report.unity_runtime_present {
            blockers.push("unity_runtime_report_missing".to_string());
        } else {
            if !report.unity_runtime_ready {
                blockers.push("unity_runtime_not_ready".to_string());
            }
            if report.unity_runtime_runner != "unity_playmode" {
                blockers.push("unity_runtime_runner_not_unity_playmode".to_string());
            }
        }
        if !report.real_ai_provider_ready {
            blockers.push("real_ai_provider_not_ready".to_string());
        }
    }

    if !report.ai_acceptance.present {
        blockers.push("ai_acceptance_report_missing".to_string());
    } else {
        if !report.ai_ready {
            blockers.push("ai_provider_acceptance_not_ready".to_string());
        }
        if !report.ai_configured_ready {
            blockers.push("ai_provider_not_configured".to_string());
        }
        if report.external_acceptance_require_ai_invoke {
            if !report.ai_invoke_attempted {
                blockers.push("ai_acceptance_invoke_not_attempted".to_string());
            } else if !report.ai_invoke_succeeded {
                blockers.push("ai_acceptance_invoke_not_succeeded".to_string());
            }
        } else if report.ai_invoke_attempted && !report.ai_invoke_succeeded {
            blockers.push("ai_provider_invoke_failed".to_string());
        }
    }
    if report.ai_acceptance_data_root_mismatch() {
        blockers.push("ai_acceptance_data_root_mismatch".to_string());
    }
    if report.ai_acceptance_provider_match_blocking() {
        blockers.push("ai_acceptance_provider_not_real_provider".to_string());
    }
    if !report.source_manifest.present {
        blockers.push("source_manifest_missing".to_string());
    } else if !report.source_ready {
        blockers.push("source_bundle_not_ready".to_string());
    }
    if !report.handoff_bundle_manifest.present {
        blockers.push("handoff_bundle_manifest_missing".to_string());
    } else if !report.handoff_bundle_ready {
        blockers.push("handoff_bundle_not_ready".to_string());
    }
    blockers
}

fn write_handoff_status_report(report: &HandoffStatusReport) -> AdmResult<()> {
    if let Some(parent) = report
        .report_path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        fs::create_dir_all(parent)?;
    }
    fs::write(&report.report_path, report.render())?;
    Ok(())
}

#[derive(Debug, Clone)]
struct SourceHandoffPolicyReport {
    release_dir: PathBuf,
    bundle_dir: PathBuf,
    report_path: PathBuf,
    source_manifest: AcceptanceSnapshot,
    handoff_bundle_manifest: AcceptanceSnapshot,
    source_ready: bool,
    source_handoff_mode: String,
    source_file_count: String,
    source_bundle_hash: String,
    source_bundle_dir_present: bool,
    source_bundle_copied: bool,
    source_bundle_hash_matches: bool,
}

impl SourceHandoffPolicyReport {
    fn ready(&self) -> bool {
        self.source_manifest.present
            && self.handoff_bundle_manifest.present
            && self.source_ready
            && self.source_handoff_mode == "bundled"
            && self.handoff_bundle_manifest.bool_value("ready")
            && self.source_bundle_dir_present
            && self.source_bundle_copied
            && self.source_bundle_hash_matches
    }

    fn render(&self) -> String {
        let mut document = String::from("# Source Handoff Policy\n");
        document.push_str(&format!("ready={}\n", self.ready()));
        document.push_str(&format!("report_path={}\n", self.report_path.display()));
        document.push_str(&format!("release_dir={}\n", self.release_dir.display()));
        document.push_str(&format!("bundle_dir={}\n", self.bundle_dir.display()));
        document.push_str(&format!(
            "source_manifest_report={}\n",
            self.source_manifest.path.display()
        ));
        document.push_str(&format!(
            "source_manifest_present={}\n",
            self.source_manifest.present
        ));
        document.push_str(&format!(
            "handoff_bundle_manifest_report={}\n",
            self.handoff_bundle_manifest.path.display()
        ));
        document.push_str(&format!(
            "handoff_bundle_manifest_present={}\n",
            self.handoff_bundle_manifest.present
        ));
        document.push_str(&format!("source_ready={}\n", self.source_ready));
        document.push_str(&format!(
            "source_handoff_mode={}\n",
            self.source_handoff_mode
        ));
        document.push_str(&format!("source_file_count={}\n", self.source_file_count));
        document.push_str(&format!("source_bundle_hash={}\n", self.source_bundle_hash));
        document
            .push_str("source_handoff_policy=bundled-source-bundle-is-current-delivery-evidence\n");
        document.push_str("parent_repo_commit_required_for_package_ready=false\n");
        document.push_str("parent_repo_commit_note=outside-generated-handoff-bundle-readiness\n");
        document.push_str(&format!(
            "source_bundle_dir_present={}\n",
            self.source_bundle_dir_present
        ));
        document.push_str(&format!(
            "source_bundle_copied={}\n",
            self.source_bundle_copied
        ));
        document.push_str(&format!(
            "source_bundle_hash_matches={}\n",
            self.source_bundle_hash_matches
        ));
        document.push_str("evidence=dist/source-bundle\n");
        document.push_str("evidence=dist/AutoDesignMaker-rust/source-manifest.adm\n");
        document
    }
}

fn write_source_handoff_policy(
    release_dir: PathBuf,
    bundle_dir: PathBuf,
    report_path: PathBuf,
) -> AdmResult<SourceHandoffPolicyReport> {
    let release_dir = fs::canonicalize(release_dir)?;
    let bundle_dir = fs::canonicalize(bundle_dir)?;
    let source_manifest = AcceptanceSnapshot::load(release_dir.join("source-manifest.adm"))?;
    let handoff_bundle_manifest =
        AcceptanceSnapshot::load(release_dir.join("handoff-bundle-manifest.adm"))?;

    let source_handoff_mode = source_manifest
        .value("source_handoff_mode")
        .unwrap_or_else(|| "none".to_string());
    let source_file_count = source_manifest
        .value("file_count")
        .unwrap_or_else(|| "0".to_string());
    let source_bundle_hash = source_manifest
        .value("bundle_hash")
        .unwrap_or_else(|| "none".to_string());
    let source_bundle_entry = handoff_bundle_manifest
        .text
        .lines()
        .find(|line| line.starts_with("required_dir=source-bundle;"))
        .unwrap_or("");
    let source_bundle_copied =
        source_bundle_entry.contains("present=true") && source_bundle_entry.contains("copied=true");
    let source_bundle_hash_matches = source_bundle_hash != "none"
        && source_bundle_entry.contains(&format!("hash={source_bundle_hash}"));

    let report = SourceHandoffPolicyReport {
        release_dir,
        source_bundle_dir_present: bundle_dir.join("source-bundle").is_dir(),
        bundle_dir,
        report_path,
        source_ready: source_manifest.bool_value("ready"),
        source_handoff_mode,
        source_file_count,
        source_bundle_hash,
        source_bundle_copied,
        source_bundle_hash_matches,
        source_manifest,
        handoff_bundle_manifest,
    };
    write_source_handoff_policy_report(&report)?;
    Ok(report)
}

fn write_source_handoff_policy_report(report: &SourceHandoffPolicyReport) -> AdmResult<()> {
    if let Some(parent) = report
        .report_path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        fs::create_dir_all(parent)?;
    }
    fs::write(&report.report_path, report.render())?;
    Ok(())
}

#[derive(Debug, Clone)]
struct HandoffInstruction {
    id: &'static str,
    required: bool,
    status: &'static str,
    estimate: &'static str,
    command: &'static str,
    evidence: &'static str,
    note: &'static str,
}

#[derive(Debug, Clone)]
struct HandoffBlockerResolution {
    blocker: String,
    action: &'static str,
    command: String,
    evidence: &'static str,
    done_when: &'static str,
}

#[derive(Debug, Clone)]
struct HandoffExternalDependency {
    id: &'static str,
    status: &'static str,
    requirement: String,
    command: String,
    evidence: &'static str,
    unlocks: &'static str,
}

#[derive(Debug, Clone)]
struct HandoffOperatorInput {
    id: &'static str,
    status: &'static str,
    placeholder: &'static str,
    requirement: String,
    check_command: String,
    set_command: String,
    required_for: &'static str,
    note: &'static str,
}

#[derive(Debug, Clone)]
struct HandoffInstructionsReport {
    release_dir: PathBuf,
    report_path: PathBuf,
    handoff_status: AcceptanceSnapshot,
    external_acceptance: AcceptanceSnapshot,
    ai_acceptance: AcceptanceSnapshot,
    source_policy: AcceptanceSnapshot,
    final_handoff_manifest: AcceptanceSnapshot,
    handoff_ready: bool,
    external_acceptance_ready: bool,
    unity_ready: bool,
    unity_runtime_ready: bool,
    unity_runtime_runner: String,
    real_ai_provider_ready: bool,
    ai_acceptance_ready: bool,
    ai_configured_ready: bool,
    ai_invoke_attempted: bool,
    ai_invoke_succeeded: bool,
    external_acceptance_require_ai_invoke: bool,
    source_policy_ready: bool,
    final_package_present: bool,
    final_package_ready: bool,
    final_delivery_ready: bool,
    final_handoff_ready: bool,
    blockers: Vec<String>,
    instructions: Vec<HandoffInstruction>,
}

impl HandoffInstructionsReport {
    fn ready(&self) -> bool {
        self.handoff_status.present
            && self.external_acceptance.present
            && self.ai_acceptance.present
            && self.source_policy.present
            && self.source_policy_ready
            && !self.instructions.is_empty()
    }

    fn render(&self) -> String {
        let ai_provider_id = self
            .ai_acceptance
            .value("provider_id")
            .unwrap_or_else(|| "<provider_id>".to_string());
        let ai_provider_model = self
            .ai_acceptance
            .value("model")
            .unwrap_or_else(|| "<model>".to_string());
        let acceptance_data_root = self
            .external_acceptance
            .value("data_root")
            .or_else(|| self.ai_acceptance.value("data_root"))
            .unwrap_or_else(|| "<data_root>".to_string());
        let suggested_ai_preset = suggested_ai_provider_preset(&ai_provider_id);
        let suggested_ai_secret_ref = suggested_ai_secret_ref(suggested_ai_preset);
        let suggested_ai_secret_env_var = suggested_ai_secret_env_var(suggested_ai_preset);
        let suggested_ai_secret_requirement =
            suggested_ai_secret_requirement(suggested_ai_secret_env_var);
        let suggested_ai_secret_check_command =
            suggested_ai_secret_check_command(suggested_ai_secret_env_var);
        let suggested_ai_secret_session_set_command =
            suggested_ai_secret_session_set_command(suggested_ai_secret_env_var);
        let suggested_unity_exe_check_command = suggested_unity_exe_check_command();
        let (suggested_unity_archive_id, suggested_unity_archive_source) =
            match latest_archive_id_from_data_root(&acceptance_data_root) {
                Some(archive_id) => (archive_id, "data_root_latest_archive"),
                None => ("<archive_id>".to_string(), "placeholder_no_archive_found"),
            };
        let unity_candidate_details = unity_candidate_values(&self.external_acceptance.text);
        let mut ai_provider_details = ai_provider_diagnostic_values(&self.ai_acceptance.text);
        for provider in ai_provider_diagnostic_values(&self.external_acceptance.text) {
            if !ai_provider_details.contains(&provider) {
                ai_provider_details.push(provider);
            }
        }
        let suggested_ai_acceptance_command = format!(
            "powershell -ExecutionPolicy Bypass -File .\\scripts\\ai_acceptance_gate.ps1 -ProviderId {} -Model {} -Preset {} -SecretRef {} -DataRoot {} -RequireReady",
            powershell_arg(&ai_provider_id),
            powershell_arg(&ai_provider_model),
            powershell_arg(suggested_ai_preset),
            powershell_arg(suggested_ai_secret_ref),
            powershell_arg(&acceptance_data_root)
        );
        let suggested_ai_acceptance_invoke_command = format!(
            "powershell -ExecutionPolicy Bypass -File .\\scripts\\ai_acceptance_gate.ps1 -ProviderId {} -Model {} -Preset {} -SecretRef {} -DataRoot {} -Invoke -RequireReady -RequireInvoke",
            powershell_arg(&ai_provider_id),
            powershell_arg(&ai_provider_model),
            powershell_arg(suggested_ai_preset),
            powershell_arg(suggested_ai_secret_ref),
            powershell_arg(&acceptance_data_root)
        );
        let suggested_unity_acceptance_command = format!(
            "powershell -ExecutionPolicy Bypass -File .\\scripts\\unity_acceptance_gate.ps1 -ArchiveId {} -UnityExe {} -DataRoot {}",
            powershell_arg(&suggested_unity_archive_id),
            powershell_arg("<path-to-Unity.exe>"),
            powershell_arg(&acceptance_data_root)
        );
        let suggested_external_acceptance_command = format!(
            "powershell -ExecutionPolicy Bypass -File .\\scripts\\external_acceptance_doctor.ps1 -UnityExe {} -DataRoot {} -RequireReady",
            powershell_arg("<path-to-Unity.exe>"),
            powershell_arg(&acceptance_data_root)
        );
        let suggested_strict_release_gate_command = format!(
            "powershell -ExecutionPolicy Bypass -File .\\scripts\\release_gate.ps1 -RequireExternalAcceptance -UnityExe {} -DataRoot {}",
            powershell_arg("<path-to-Unity.exe>"),
            powershell_arg(&acceptance_data_root)
        );
        let suggested_strict_release_gate_ai_invoke_command = format!(
            "powershell -ExecutionPolicy Bypass -File .\\scripts\\release_gate.ps1 -RequireExternalAcceptance -RequireAiInvoke -UnityExe {} -DataRoot {}",
            powershell_arg("<path-to-Unity.exe>"),
            powershell_arg(&acceptance_data_root)
        );
        let suggested_operator_preflight_command = format!(
            "powershell -ExecutionPolicy Bypass -File .\\scripts\\handoff_operator_preflight.ps1 -DataRoot {}",
            powershell_arg(&acceptance_data_root)
        );
        let suggested_operator_preflight_require_ready_command = format!(
            "powershell -ExecutionPolicy Bypass -File .\\scripts\\handoff_operator_preflight.ps1 -UnityExe {} -DataRoot {} -RequireReady",
            powershell_arg("<path-to-Unity.exe>"),
            powershell_arg(&acceptance_data_root)
        );
        let suggested_operator_preflight_bundle_root_command = format!(
            "powershell -ExecutionPolicy Bypass -File .\\source-bundle\\scripts\\handoff_operator_preflight.ps1 -InstructionsPath ..\\evidence\\handoff-instructions.adm -DataRoot {}",
            powershell_arg(&acceptance_data_root)
        );
        let suggested_operator_preflight_bundle_root_require_ready_command = format!(
            "powershell -ExecutionPolicy Bypass -File .\\source-bundle\\scripts\\handoff_operator_preflight.ps1 -InstructionsPath ..\\evidence\\handoff-instructions.adm -UnityExe {} -DataRoot {} -RequireReady",
            powershell_arg("<path-to-Unity.exe>"),
            powershell_arg(&acceptance_data_root)
        );
        let suggested_handoff_rehydration_command =
            "powershell -ExecutionPolicy Bypass -File .\\source-bundle\\scripts\\rehydrate_handoff_workspace.ps1 -DestinationPath '<path-to-rehydrated-rust-workspace>'"
                .to_string();
        let rehydrated_release_smoke_report = "dist/AutoDesignMaker-rust/release-acceptance.adm";
        let rehydrated_release_smoke_command =
            ".\\dist\\AutoDesignMaker-rust\\AutoDesignMaker-rust.exe --smoke";
        let rehydrated_release_smoke_working_dir = "rehydrated-rust-workspace-root";
        let suggested_final_acceptance_command = format!(
            "powershell -ExecutionPolicy Bypass -File .\\scripts\\final_handoff_acceptance.ps1 -UnityExe {} -DataRoot {}",
            powershell_arg("<path-to-Unity.exe>"),
            powershell_arg(&acceptance_data_root)
        );
        let suggested_final_acceptance_ai_invoke_command = format!(
            "powershell -ExecutionPolicy Bypass -File .\\scripts\\final_handoff_acceptance.ps1 -UnityExe {} -DataRoot {} -RequireAiInvoke",
            powershell_arg("<path-to-Unity.exe>"),
            powershell_arg(&acceptance_data_root)
        );
        let final_acceptance_report = "dist/AutoDesignMaker-rust/final-acceptance-run.adm";
        let blocker_resolutions = self
            .blockers
            .iter()
            .map(|blocker| {
                handoff_blocker_resolution(
                    blocker,
                    &suggested_ai_acceptance_command,
                    &suggested_ai_acceptance_invoke_command,
                    &suggested_unity_acceptance_command,
                    &suggested_external_acceptance_command,
                    &suggested_strict_release_gate_command,
                )
            })
            .collect::<Vec<_>>();
        let external_dependencies = self.handoff_external_dependencies(
            &suggested_ai_secret_requirement,
            &suggested_ai_acceptance_command,
            &suggested_ai_acceptance_invoke_command,
            &suggested_unity_acceptance_command,
        );
        let operator_inputs = self.handoff_operator_inputs(
            &suggested_ai_secret_requirement,
            &suggested_ai_secret_check_command,
            &suggested_ai_secret_session_set_command,
            &suggested_unity_exe_check_command,
        );

        let mut document = String::from("# Handoff Instructions\n");
        document.push_str(&format!("ready={}\n", self.ready()));
        document.push_str(&format!("report_path={}\n", self.report_path.display()));
        document.push_str(&format!("release_dir={}\n", self.release_dir.display()));
        document.push_str(&format!(
            "handoff_status_report={}\n",
            self.handoff_status.path.display()
        ));
        document.push_str(&format!(
            "handoff_status_present={}\n",
            self.handoff_status.present
        ));
        document.push_str(&format!(
            "external_acceptance_report={}\n",
            self.external_acceptance.path.display()
        ));
        document.push_str(&format!(
            "external_acceptance_present={}\n",
            self.external_acceptance.present
        ));
        document.push_str(&format!(
            "ai_acceptance_report={}\n",
            self.ai_acceptance.path.display()
        ));
        document.push_str(&format!(
            "ai_acceptance_present={}\n",
            self.ai_acceptance.present
        ));
        document.push_str(&format!(
            "source_policy_report={}\n",
            self.source_policy.path.display()
        ));
        document.push_str(&format!(
            "source_policy_present={}\n",
            self.source_policy.present
        ));
        document.push_str(&format!(
            "final_handoff_manifest_report={}\n",
            self.final_handoff_manifest.path.display()
        ));
        document.push_str(&format!(
            "final_handoff_manifest_present={}\n",
            self.final_package_present
        ));
        document.push_str(&format!("handoff_ready={}\n", self.handoff_ready));
        document.push_str(&format!(
            "external_acceptance_ready={}\n",
            self.external_acceptance_ready
        ));
        document.push_str(&format!("unity_ready={}\n", self.unity_ready));
        document.push_str(&format!(
            "unity_runtime_ready={}\n",
            self.unity_runtime_ready
        ));
        document.push_str(&format!(
            "unity_runtime_runner={}\n",
            self.unity_runtime_runner
        ));
        document.push_str(&format!(
            "real_ai_provider_ready={}\n",
            self.real_ai_provider_ready
        ));
        document.push_str(&format!(
            "ai_acceptance_ready={}\n",
            self.ai_acceptance_ready
        ));
        document.push_str(&format!(
            "ai_configured_ready={}\n",
            self.ai_configured_ready
        ));
        document.push_str(&format!(
            "ai_invoke_attempted={}\n",
            self.ai_invoke_attempted
        ));
        document.push_str(&format!(
            "ai_invoke_succeeded={}\n",
            self.ai_invoke_succeeded
        ));
        document.push_str(&format!(
            "external_acceptance_require_ai_invoke={}\n",
            self.external_acceptance_require_ai_invoke
        ));
        for (output_key, source, source_key) in [
            (
                "external_acceptance_data_root",
                &self.external_acceptance,
                "data_root",
            ),
            ("ai_acceptance_data_root", &self.ai_acceptance, "data_root"),
            ("ai_provider_id", &self.ai_acceptance, "provider_id"),
            ("ai_provider_model", &self.ai_acceptance, "model"),
            (
                "ai_diagnostic_readiness",
                &self.ai_acceptance,
                "diagnostic_readiness",
            ),
            (
                "unity_selected",
                &self.external_acceptance,
                "unity_selected",
            ),
            (
                "unity_candidates",
                &self.external_acceptance,
                "unity_candidates",
            ),
            (
                "real_ai_provider_count",
                &self.external_acceptance,
                "real_ai_provider_count",
            ),
            (
                "ready_provider_count",
                &self.external_acceptance,
                "ready_provider_count",
            ),
        ] {
            if let Some(value) = source.value(source_key) {
                document.push_str(&format!("{output_key}={value}\n"));
            }
        }
        document.push_str(&format!(
            "unity_candidate_detail_count={}\n",
            unity_candidate_details.len()
        ));
        for candidate in &unity_candidate_details {
            document.push_str(&format!("unity_candidate={candidate}\n"));
        }
        document.push_str(&format!(
            "ai_provider_detail_count={}\n",
            ai_provider_details.len()
        ));
        for provider in &ai_provider_details {
            document.push_str(&format!("ai_provider={provider}\n"));
        }
        document.push_str(&format!(
            "source_policy_ready={}\n",
            self.source_policy_ready
        ));
        document.push_str(&format!(
            "final_package_ready={}\n",
            self.final_package_ready
        ));
        document.push_str(&format!(
            "final_delivery_ready={}\n",
            self.final_delivery_ready
        ));
        document.push_str(&format!(
            "final_handoff_ready={}\n",
            self.final_handoff_ready
        ));
        document.push_str(&format!(
            "strict_gate_command={suggested_strict_release_gate_command}\n"
        ));
        document.push_str(&format!(
            "strict_gate_ai_invoke_command={suggested_strict_release_gate_ai_invoke_command}\n"
        ));
        document.push_str("strict_gate_requires_final_delivery=true\n");
        document.push_str(
            "strict_gate_final_manifest_requires=package_ready,handoff_ready,delivery_ready\n",
        );
        document.push_str(&format!(
            "suggested_ai_provider_preset={suggested_ai_preset}\n"
        ));
        document.push_str(&format!(
            "suggested_ai_secret_ref={suggested_ai_secret_ref}\n"
        ));
        document.push_str(&format!(
            "suggested_ai_secret_env_var={suggested_ai_secret_env_var}\n"
        ));
        document.push_str(&format!(
            "suggested_ai_secret_requirement={suggested_ai_secret_requirement}\n"
        ));
        document.push_str(&format!(
            "suggested_ai_secret_check_command={suggested_ai_secret_check_command}\n"
        ));
        document.push_str(&format!(
            "suggested_ai_secret_session_set_command={suggested_ai_secret_session_set_command}\n"
        ));
        document.push_str(&format!(
            "suggested_unity_archive_id={suggested_unity_archive_id}\n"
        ));
        document.push_str(&format!(
            "suggested_unity_archive_source={suggested_unity_archive_source}\n"
        ));
        document.push_str(&format!(
            "suggested_ai_acceptance_command={suggested_ai_acceptance_command}\n"
        ));
        document.push_str(&format!(
            "suggested_ai_acceptance_invoke_command={suggested_ai_acceptance_invoke_command}\n"
        ));
        document.push_str(&format!(
            "suggested_unity_acceptance_command={suggested_unity_acceptance_command}\n"
        ));
        document.push_str(&format!(
            "suggested_external_acceptance_command={suggested_external_acceptance_command}\n"
        ));
        document.push_str(&format!(
            "suggested_strict_release_gate_command={suggested_strict_release_gate_command}\n"
        ));
        document.push_str(&format!(
            "suggested_strict_release_gate_ai_invoke_command={suggested_strict_release_gate_ai_invoke_command}\n"
        ));
        document.push_str(&format!(
            "suggested_operator_preflight_command={suggested_operator_preflight_command}\n"
        ));
        document.push_str(&format!(
            "suggested_operator_preflight_require_ready_command={suggested_operator_preflight_require_ready_command}\n"
        ));
        document.push_str(
            "operator_preflight_working_dir=rust-workspace-root-with-scripts-directory\n",
        );
        document.push_str("operator_preflight_bundle_root_supported=true\n");
        document.push_str(
            "operator_preflight_bundle_root_script=source-bundle/scripts/handoff_operator_preflight.ps1\n",
        );
        document.push_str(
            "operator_preflight_bundle_root_instructions_path=..\\evidence\\handoff-instructions.adm\n",
        );
        document.push_str(&format!(
            "suggested_operator_preflight_bundle_root_command={suggested_operator_preflight_bundle_root_command}\n"
        ));
        document.push_str(&format!(
            "suggested_operator_preflight_bundle_root_require_ready_command={suggested_operator_preflight_bundle_root_require_ready_command}\n"
        ));
        document.push_str("handoff_rehydration_bundle_root_supported=true\n");
        document.push_str(
            "handoff_rehydration_script=source-bundle/scripts/rehydrate_handoff_workspace.ps1\n",
        );
        document.push_str(
            "handoff_rehydration_destination_placeholder=<path-to-rehydrated-rust-workspace>\n",
        );
        document.push_str("handoff_rehydration_manifest=dist/handoff-rehydration-manifest.adm\n");
        document.push_str(&format!(
            "suggested_handoff_rehydration_command={suggested_handoff_rehydration_command}\n"
        ));
        document.push_str(&format!(
            "rehydrated_release_smoke_report={rehydrated_release_smoke_report}\n"
        ));
        document.push_str(&format!(
            "rehydrated_release_smoke_command={rehydrated_release_smoke_command}\n"
        ));
        document.push_str(&format!(
            "rehydrated_release_smoke_working_dir={rehydrated_release_smoke_working_dir}\n"
        ));
        document.push_str(
            "final_acceptance_working_dir=rust-workspace-root-after-rehydration-or-original\n",
        );
        document.push_str("final_acceptance_script=scripts/final_handoff_acceptance.ps1\n");
        document.push_str(
            "final_acceptance_sequence=operator-preflight,ai-acceptance,unity-acceptance,external-acceptance,strict-release-gate\n",
        );
        document.push_str("final_acceptance_requires=ai_secret,unity_exe,data_root\n");
        document.push_str(&format!(
            "final_acceptance_report={final_acceptance_report}\n"
        ));
        document
            .push_str("final_acceptance_package_refresh=after-successful-default-report-write\n");
        document.push_str(&format!(
            "suggested_final_acceptance_command={suggested_final_acceptance_command}\n"
        ));
        document.push_str(&format!(
            "suggested_final_acceptance_ai_invoke_command={suggested_final_acceptance_ai_invoke_command}\n"
        ));
        document.push_str(&format!(
            "external_dependency_count={}\n",
            external_dependencies.len()
        ));
        for dependency in &external_dependencies {
            document.push_str(&format!(
                "external_dependency={}; status={}; requirement={}; command={}; evidence={}; unlocks={}\n",
                dependency.id,
                dependency.status,
                dependency.requirement,
                dependency.command,
                dependency.evidence,
                dependency.unlocks
            ));
        }
        document.push_str(&format!("operator_input_count={}\n", operator_inputs.len()));
        for input in &operator_inputs {
            document.push_str(&format!(
                "operator_input={}; status={}; placeholder={}; requirement={}; check_command={}; set_command={}; required_for={}; note={}\n",
                input.id,
                input.status,
                input.placeholder,
                input.requirement,
                input.check_command,
                input.set_command,
                input.required_for,
                input.note
            ));
        }
        document.push_str(&format!("blocker_count={}\n", self.blockers.len()));
        for blocker in &self.blockers {
            document.push_str(&format!("blocker={blocker}\n"));
        }
        document.push_str(&format!(
            "blocker_resolution_count={}\n",
            blocker_resolutions.len()
        ));
        for resolution in &blocker_resolutions {
            document.push_str(&format!(
                "blocker_resolution={}; action={}; command={}; evidence={}; done_when={}\n",
                resolution.blocker,
                resolution.action,
                resolution.command,
                resolution.evidence,
                resolution.done_when
            ));
        }
        let required_instruction_count = self
            .instructions
            .iter()
            .filter(|instruction| instruction.required)
            .count();
        let required_blocked_instruction_count = self
            .instructions
            .iter()
            .filter(|instruction| instruction.required && instruction.status == "blocked")
            .count();
        let required_waiting_instruction_count = self
            .instructions
            .iter()
            .filter(|instruction| instruction.required && instruction.status.starts_with("waiting"))
            .count();
        let optional_instruction_count = self
            .instructions
            .iter()
            .filter(|instruction| !instruction.required)
            .count();
        let manual_decision_instruction_count = self
            .instructions
            .iter()
            .filter(|instruction| instruction.status == "manual-decision")
            .count();
        let next_required = self
            .instructions
            .iter()
            .find(|instruction| instruction.required && instruction.status != "ready");
        let next_required_instruction = next_required
            .map(|instruction| instruction.id)
            .unwrap_or("none");
        let next_required_instruction_status = next_required
            .map(|instruction| instruction.status)
            .unwrap_or("none");
        let next_required_instruction_estimate = next_required
            .map(|instruction| instruction.estimate)
            .unwrap_or("none");
        let next_required_instruction_command = next_required
            .map(|instruction| {
                handoff_instruction_command(
                    instruction,
                    &suggested_ai_acceptance_command,
                    &suggested_ai_acceptance_invoke_command,
                    &suggested_unity_acceptance_command,
                    &suggested_external_acceptance_command,
                    &suggested_strict_release_gate_command,
                )
            })
            .unwrap_or("none");
        let next_required_instruction_evidence = next_required
            .map(|instruction| instruction.evidence)
            .unwrap_or("none");
        let next_required_instruction_done_when = next_required
            .map(handoff_instruction_done_when)
            .unwrap_or("none");
        let next_required_instruction_note = next_required
            .map(|instruction| instruction.note)
            .unwrap_or("none");
        document.push_str(&format!(
            "required_instruction_count={required_instruction_count}\n"
        ));
        document.push_str(&format!(
            "required_blocked_instruction_count={required_blocked_instruction_count}\n"
        ));
        document.push_str(&format!(
            "required_waiting_instruction_count={required_waiting_instruction_count}\n"
        ));
        document.push_str(&format!(
            "optional_instruction_count={optional_instruction_count}\n"
        ));
        document.push_str(&format!(
            "manual_decision_instruction_count={manual_decision_instruction_count}\n"
        ));
        document.push_str(&format!(
            "next_required_instruction={next_required_instruction}\n"
        ));
        document.push_str(&format!(
            "next_required_instruction_status={next_required_instruction_status}\n"
        ));
        document.push_str(&format!(
            "next_required_instruction_estimate={next_required_instruction_estimate}\n"
        ));
        document.push_str(&format!(
            "next_required_instruction_command={next_required_instruction_command}\n"
        ));
        document.push_str(&format!(
            "next_required_instruction_evidence={next_required_instruction_evidence}\n"
        ));
        document.push_str(&format!(
            "next_required_instruction_done_when={next_required_instruction_done_when}\n"
        ));
        document.push_str(&format!(
            "next_required_instruction_note={next_required_instruction_note}\n"
        ));
        let remaining_required_execution_steps = self
            .instructions
            .iter()
            .filter(|instruction| instruction.required && instruction.status != "ready")
            .collect::<Vec<_>>();
        document.push_str(&format!(
            "remaining_required_execution_step_count={}\n",
            remaining_required_execution_steps.len()
        ));
        for (index, instruction) in remaining_required_execution_steps.iter().enumerate() {
            let instruction_command = handoff_instruction_command(
                instruction,
                &suggested_ai_acceptance_command,
                &suggested_ai_acceptance_invoke_command,
                &suggested_unity_acceptance_command,
                &suggested_external_acceptance_command,
                &suggested_strict_release_gate_command,
            );
            document.push_str(&format!(
                "remaining_required_execution_step={}; instruction={}; status={}; estimate={}; command={}; evidence={}; done_when={}; note={}\n",
                index + 1,
                instruction.id,
                instruction.status,
                instruction.estimate,
                instruction_command,
                instruction.evidence,
                handoff_instruction_done_when(instruction),
                instruction.note
            ));
        }
        document.push_str(&format!("instruction_count={}\n", self.instructions.len()));
        for instruction in &self.instructions {
            let instruction_command = handoff_instruction_command(
                instruction,
                &suggested_ai_acceptance_command,
                &suggested_ai_acceptance_invoke_command,
                &suggested_unity_acceptance_command,
                &suggested_external_acceptance_command,
                &suggested_strict_release_gate_command,
            );
            document.push_str(&format!(
                "instruction={}; required={}; status={}; estimate={}; command={}; evidence={}; note={}\n",
                instruction.id,
                instruction.required,
                instruction.status,
                instruction.estimate,
                instruction_command,
                instruction.evidence,
                instruction.note
            ));
        }
        document
    }

    fn handoff_external_dependencies(
        &self,
        suggested_ai_secret_requirement: &str,
        suggested_ai_acceptance_command: &str,
        suggested_ai_acceptance_invoke_command: &str,
        suggested_unity_acceptance_command: &str,
    ) -> Vec<HandoffExternalDependency> {
        let mut dependencies = Vec::new();
        if !self.ai_configured_ready || !self.real_ai_provider_ready || !self.ai_acceptance_ready {
            let status = if !self.ai_configured_ready {
                "missing_secret_or_provider_config"
            } else if !self.ai_acceptance_ready {
                "acceptance_not_ready"
            } else {
                "not_reported_as_ready_real_provider"
            };
            dependencies.push(HandoffExternalDependency {
                id: "real_ai_provider",
                status,
                requirement: suggested_ai_secret_requirement.to_string(),
                command: suggested_ai_acceptance_command.to_string(),
                evidence: "dist/AutoDesignMaker-rust/ai-acceptance.adm",
                unlocks: "configure-real-ai-provider",
            });
        }
        if self.external_acceptance_require_ai_invoke
            && (!self.ai_invoke_attempted || !self.ai_invoke_succeeded)
        {
            let status = if !self.ai_invoke_attempted {
                "invoke_not_attempted"
            } else {
                "invoke_not_succeeded"
            };
            dependencies.push(HandoffExternalDependency {
                id: "real_ai_provider_invoke",
                status,
                requirement: "real-provider-network-invoke".to_string(),
                command: suggested_ai_acceptance_invoke_command.to_string(),
                evidence: "dist/AutoDesignMaker-rust/ai-acceptance.adm",
                unlocks: "run-ai-provider-invoke-acceptance",
            });
        }
        if !self.unity_ready
            || !self.unity_runtime_ready
            || self.unity_runtime_runner != "unity_playmode"
        {
            let status = if !self.unity_ready {
                "unity_not_ready"
            } else if !self.unity_runtime_ready {
                "runtime_not_ready"
            } else {
                "runner_not_unity_playmode"
            };
            dependencies.push(HandoffExternalDependency {
                id: "unity_playmode",
                status,
                requirement: "compatible-unity-editor-path".to_string(),
                command: suggested_unity_acceptance_command.to_string(),
                evidence: UNITY_PLAYMODE_EVIDENCE_PATH,
                unlocks: "run-unity-acceptance",
            });
        }
        dependencies
    }

    fn handoff_operator_inputs(
        &self,
        suggested_ai_secret_requirement: &str,
        suggested_ai_secret_check_command: &str,
        suggested_ai_secret_session_set_command: &str,
        suggested_unity_exe_check_command: &str,
    ) -> Vec<HandoffOperatorInput> {
        let mut inputs = Vec::new();
        if !self.ai_configured_ready || !self.real_ai_provider_ready || !self.ai_acceptance_ready {
            let status = if !self.ai_configured_ready {
                "missing_secret_or_provider_config"
            } else if !self.ai_acceptance_ready {
                "acceptance_not_ready"
            } else {
                "not_reported_as_ready_real_provider"
            };
            inputs.push(HandoffOperatorInput {
                id: "ai_secret",
                status,
                placeholder: "<secret>",
                requirement: suggested_ai_secret_requirement.to_string(),
                check_command: suggested_ai_secret_check_command.to_string(),
                set_command: suggested_ai_secret_session_set_command.to_string(),
                required_for: "configure-real-ai-provider",
                note: "provide-redacted-secret-in-receiving-shell-before-running-ai-acceptance",
            });
        }
        if !self.unity_ready
            || !self.unity_runtime_ready
            || self.unity_runtime_runner != "unity_playmode"
        {
            let status = if !self.unity_ready {
                "missing_or_not_ready"
            } else if !self.unity_runtime_ready {
                "runtime_not_ready"
            } else {
                "runner_not_unity_playmode"
            };
            inputs.push(HandoffOperatorInput {
                id: "unity_exe",
                status,
                placeholder: "<path-to-Unity.exe>",
                requirement: "compatible-unity-editor-path".to_string(),
                check_command: suggested_unity_exe_check_command.to_string(),
                set_command: "replace-placeholder-in-unity-commands".to_string(),
                required_for:
                    "run-unity-acceptance,rerun-external-acceptance,run-strict-release-gate",
                note: "use-compatible-unity-editor-that-can-run-playmode-validation",
            });
        }
        inputs
    }
}

fn handoff_instruction_command<'a>(
    instruction: &'a HandoffInstruction,
    suggested_ai_acceptance_command: &'a str,
    suggested_ai_acceptance_invoke_command: &'a str,
    suggested_unity_acceptance_command: &'a str,
    suggested_external_acceptance_command: &'a str,
    suggested_strict_release_gate_command: &'a str,
) -> &'a str {
    match instruction.id {
        "configure-real-ai-provider" => suggested_ai_acceptance_command,
        "run-ai-provider-invoke-acceptance" => suggested_ai_acceptance_invoke_command,
        "run-unity-acceptance" => suggested_unity_acceptance_command,
        "rerun-external-acceptance" => suggested_external_acceptance_command,
        "run-strict-release-gate" => suggested_strict_release_gate_command,
        _ => instruction.command,
    }
}

fn handoff_instruction_done_when(instruction: &HandoffInstruction) -> &'static str {
    match instruction.id {
        "configure-real-ai-provider" => "ready=true-and-configured_ready=true",
        "run-ai-provider-invoke-acceptance" => "invoke_attempted=true-and-invoke_succeeded=true",
        "run-unity-acceptance" => {
            "runtime_execution_results.adm-has-ready=true-and-runner=unity_playmode"
        }
        "rerun-external-acceptance" => "ready=true",
        "run-strict-release-gate" => {
            "handoff-status-ready=true-and-final-manifest-package-handoff-delivery-ready"
        }
        "confirm-final-delivery-package" => "final-handoff-manifest-delivery_ready=true",
        "decide-source-handoff-policy" => "source-handoff-policy-ready=true",
        "explain-package-vs-delivery-readiness" => "informational-only",
        _ => "see-instruction-evidence",
    }
}

fn handoff_blocker_resolution(
    blocker: &str,
    suggested_ai_acceptance_command: &str,
    suggested_ai_acceptance_invoke_command: &str,
    suggested_unity_acceptance_command: &str,
    suggested_external_acceptance_command: &str,
    suggested_strict_release_gate_command: &str,
) -> HandoffBlockerResolution {
    let (action, command, evidence, done_when) = match blocker {
        "release_acceptance_report_missing"
        | "release_not_accepted"
        | "release_delivery_not_ready"
        | "desktop_release_not_ready"
        | "release_smoke_not_ready" => (
            "rerun-local-release-gate",
            "powershell -ExecutionPolicy Bypass -File .\\scripts\\release_gate.ps1 -SkipExternalAcceptance",
            "dist/AutoDesignMaker-rust/release-acceptance.adm",
            "accepted=true-and-smoke_ready=true",
        ),
        "external_acceptance_report_missing" | "external_acceptance_not_ready" => (
            "rerun-external-acceptance-after-ai-and-unity-are-ready",
            suggested_external_acceptance_command,
            "dist/AutoDesignMaker-rust/external-acceptance.adm",
            "ready=true",
        ),
        "unity_not_ready" | "unity_runtime_report_missing" | "unity_runtime_not_ready" => (
            "run-unity-acceptance-with-real-editor",
            suggested_unity_acceptance_command,
            UNITY_PLAYMODE_EVIDENCE_PATH,
            "runtime_execution_results.adm-has-ready=true-and-runner=unity_playmode",
        ),
        "unity_runtime_runner_not_unity_playmode" => (
            "rerun-unity-playmode-validation",
            suggested_unity_acceptance_command,
            UNITY_PLAYMODE_EVIDENCE_PATH,
            "runner=unity_playmode",
        ),
        "real_ai_provider_not_ready"
        | "ai_acceptance_report_missing"
        | "ai_provider_acceptance_not_ready"
        | "ai_provider_not_configured"
        | "ai_acceptance_provider_not_real_provider" => (
            "configure-and-run-real-ai-provider-acceptance",
            suggested_ai_acceptance_command,
            "dist/AutoDesignMaker-rust/ai-acceptance.adm",
            "ready=true-and-configured_ready=true",
        ),
        "ai_acceptance_invoke_not_attempted"
        | "ai_acceptance_invoke_not_succeeded"
        | "ai_provider_invoke_failed" => (
            "rerun-ai-provider-invoke-acceptance",
            suggested_ai_acceptance_invoke_command,
            "dist/AutoDesignMaker-rust/ai-acceptance.adm",
            "invoke_attempted=true-and-invoke_succeeded=true",
        ),
        "ai_acceptance_data_root_mismatch" => (
            "rerun-ai-and-external-acceptance-with-same-data-root",
            suggested_external_acceptance_command,
            "dist/AutoDesignMaker-rust/handoff-status.adm",
            "ai_acceptance_data_root_matches_external=true",
        ),
        "source_manifest_missing" | "source_bundle_not_ready" => (
            "regenerate-source-bundle",
            "cargo run -q -p adm-cli -- stage-source-bundle",
            "dist/AutoDesignMaker-rust/source-manifest.adm",
            "ready=true",
        ),
        "handoff_bundle_manifest_missing" | "handoff_bundle_not_ready" => (
            "regenerate-handoff-bundle",
            "cargo run -q -p adm-cli -- stage-handoff-bundle",
            "dist/AutoDesignMaker-rust/handoff-bundle-manifest.adm",
            "ready=true",
        ),
        _ => (
            "rerun-strict-release-gate-and-inspect-evidence",
            suggested_strict_release_gate_command,
            "dist/AutoDesignMaker-rust/handoff-status.adm",
            "blocker-absent-from-handoff-status",
        ),
    };

    HandoffBlockerResolution {
        blocker: blocker.to_string(),
        action,
        command: command.to_string(),
        evidence,
        done_when,
    }
}

fn suggested_ai_provider_preset(provider_id: &str) -> &'static str {
    let normalized = provider_id.to_ascii_lowercase();
    if normalized.contains("openrouter") {
        "openrouter"
    } else if normalized.contains("deepseek") {
        "deepseek"
    } else if normalized.contains("local") {
        "local_openai"
    } else {
        "openai"
    }
}

fn suggested_ai_secret_ref(preset_id: &str) -> &'static str {
    if preset_id == "local_openai" {
        "none"
    } else {
        "default"
    }
}

fn suggested_ai_secret_env_var(preset_id: &str) -> &'static str {
    match preset_id {
        "openrouter" => "OPENROUTER_API_KEY",
        "deepseek" => "DEEPSEEK_API_KEY",
        "local_openai" => "none",
        _ => "OPENAI_API_KEY",
    }
}

fn suggested_ai_secret_requirement(env_var: &str) -> String {
    if env_var == "none" {
        "none".to_string()
    } else {
        format!("env:{env_var}")
    }
}

fn suggested_ai_secret_check_command(env_var: &str) -> String {
    if env_var == "none" {
        "none".to_string()
    } else {
        format!("powershell -NoProfile -Command \"[bool]`$env:{env_var}\"")
    }
}

fn suggested_ai_secret_session_set_command(env_var: &str) -> String {
    if env_var == "none" {
        "none".to_string()
    } else {
        format!("$env:{env_var}='<secret>'")
    }
}

fn suggested_unity_exe_check_command() -> String {
    "powershell -NoProfile -Command \"Test-Path -LiteralPath '<path-to-Unity.exe>'\"".to_string()
}

fn latest_archive_id_from_data_root(data_root: &str) -> Option<String> {
    let trimmed = data_root.trim();
    if trimmed.is_empty() || (trimmed.starts_with('<') && trimmed.ends_with('>')) {
        return None;
    }

    let archives_dir = Path::new(trimmed).join("archives");
    let entries = fs::read_dir(archives_dir).ok()?;
    let mut archive_ids = Vec::new();
    for entry in entries.flatten() {
        let Ok(file_type) = entry.file_type() else {
            continue;
        };
        if !file_type.is_dir() {
            continue;
        }

        let archive_id = entry.file_name().to_string_lossy().to_string();
        if ArchiveId::new(archive_id.clone()).is_err() {
            continue;
        }

        let manifest_path = entry.path().join("manifest.adm");
        if !manifest_path.is_file() {
            continue;
        }
        let Ok(manifest_text) = fs::read_to_string(&manifest_path) else {
            continue;
        };
        let expected_line = format!("archive_id={archive_id}");
        if !manifest_text
            .lines()
            .any(|line| line.trim() == expected_line)
        {
            continue;
        }

        archive_ids.push(archive_id);
    }
    archive_ids.sort();
    archive_ids.pop()
}

fn powershell_arg(value: &str) -> String {
    let trimmed = value.trim();
    if (trimmed.starts_with('<') && trimmed.ends_with('>'))
        || trimmed.chars().any(char::is_whitespace)
    {
        format!("'{}'", trimmed.replace('\'', "''"))
    } else {
        trimmed.to_string()
    }
}

fn write_handoff_instructions(
    release_dir: PathBuf,
    report_path: PathBuf,
) -> AdmResult<HandoffInstructionsReport> {
    let release_dir = fs::canonicalize(release_dir)?;
    let handoff_status = AcceptanceSnapshot::load(release_dir.join("handoff-status.adm"))?;
    let external_acceptance =
        AcceptanceSnapshot::load(release_dir.join("external-acceptance.adm"))?;
    let ai_acceptance = AcceptanceSnapshot::load(release_dir.join("ai-acceptance.adm"))?;
    let source_policy = AcceptanceSnapshot::load(release_dir.join("source-handoff-policy.adm"))?;
    let final_handoff_manifest =
        AcceptanceSnapshot::load(release_dir.join("final-handoff-manifest.adm"))?;

    let handoff_ready = handoff_status.bool_value("ready");
    let external_acceptance_ready = external_acceptance.bool_value("ready");
    let unity_ready = external_acceptance.bool_value("unity_ready");
    let unity_runtime_ready = external_acceptance.bool_value("unity_runtime_ready");
    let unity_runtime_runner = external_acceptance
        .value("unity_runtime_runner")
        .unwrap_or_else(|| "none".to_string());
    let real_ai_provider_ready = external_acceptance.bool_value("real_ai_provider_ready");
    let ai_acceptance_ready = ai_acceptance.bool_value("ready");
    let ai_configured_ready = ai_acceptance.bool_value("configured_ready");
    let ai_invoke_attempted = ai_acceptance.bool_value("invoke_attempted");
    let ai_invoke_succeeded = ai_acceptance.bool_value("invoke_succeeded");
    let external_acceptance_require_ai_invoke = external_acceptance.bool_value("require_ai_invoke");
    let source_policy_ready = source_policy.bool_value("ready");
    let final_package_present = final_handoff_manifest.present;
    let final_package_ready = final_handoff_manifest.bool_value("package_ready");
    let final_delivery_ready = final_handoff_manifest.bool_value("delivery_ready");
    let final_handoff_ready = final_handoff_manifest.bool_value("handoff_ready");
    let blockers = report_values(&handoff_status.text, "blocker");

    let mut instructions = vec![
        HandoffInstruction {
            id: "configure-real-ai-provider",
            required: !real_ai_provider_ready || !ai_configured_ready || !ai_acceptance_ready,
            status: if real_ai_provider_ready && ai_configured_ready && ai_acceptance_ready {
                "ready"
            } else {
                "blocked"
            },
            estimate: "0.5-1h-if-credentials-are-ready",
            command: "powershell -ExecutionPolicy Bypass -File .\\scripts\\ai_acceptance_gate.ps1 -ProviderId '<provider_id>' -Model '<model>' -Preset '<preset_id>' -SecretRef default -DataRoot '<data_root>' -RequireReady",
            evidence: "dist/AutoDesignMaker-rust/ai-acceptance.adm",
            note: "configures-non-mock-provider-then-writes-redacted-acceptance-report",
        },
        HandoffInstruction {
            id: "run-ai-provider-invoke-acceptance",
            required: external_acceptance_require_ai_invoke && !ai_invoke_succeeded,
            status: if ai_invoke_attempted && ai_invoke_succeeded {
                "ready"
            } else if external_acceptance_require_ai_invoke {
                "blocked"
            } else {
                "manual-decision"
            },
            estimate: "0.25-1h-if-credentials-are-ready",
            command: "powershell -ExecutionPolicy Bypass -File .\\scripts\\ai_acceptance_gate.ps1 -ProviderId '<provider_id>' -Model '<model>' -Preset '<preset_id>' -SecretRef default -DataRoot '<data_root>' -Invoke -RequireReady -RequireInvoke",
            evidence: "dist/AutoDesignMaker-rust/ai-acceptance.adm",
            note: "performs-redacted-real-network-call-before-strict-ai-invoke-gate",
        },
        HandoffInstruction {
            id: "run-unity-acceptance",
            required: !unity_ready
                || !unity_runtime_ready
                || unity_runtime_runner != "unity_playmode",
            status: if unity_ready
                && unity_runtime_ready
                && unity_runtime_runner == "unity_playmode"
            {
                "ready"
            } else {
                "blocked"
            },
            estimate: "1-3h-if-unity-is-installed",
            command: "powershell -ExecutionPolicy Bypass -File .\\scripts\\unity_acceptance_gate.ps1 -ArchiveId '<archive_id>' -UnityExe '<path-to-Unity.exe>' -DataRoot '<data_root>'",
            evidence: UNITY_PLAYMODE_EVIDENCE_PATH,
            note: "requires-compatible-unity-editor-and-unity_playmode-runtime-results",
        },
        HandoffInstruction {
            id: "rerun-external-acceptance",
            required: !external_acceptance_ready,
            status: if external_acceptance_ready {
                "ready"
            } else {
                "blocked"
            },
            estimate: "0.5h-after-ai-and-unity-are-ready",
            command: "powershell -ExecutionPolicy Bypass -File .\\scripts\\external_acceptance_doctor.ps1 -UnityExe '<path-to-Unity.exe>' -DataRoot '<data_root>' -RequireReady",
            evidence: "dist/AutoDesignMaker-rust/external-acceptance.adm",
            note: "requires-unity-ready-and-real-ai-provider-ready",
        },
        HandoffInstruction {
            id: "run-strict-release-gate",
            required: !handoff_ready,
            status: if handoff_ready { "ready" } else { "blocked" },
            estimate: "0.5-1h-after-external-acceptance-is-ready",
            command: "powershell -ExecutionPolicy Bypass -File .\\scripts\\release_gate.ps1 -RequireExternalAcceptance -UnityExe '<path-to-Unity.exe>' -DataRoot '<data_root>'",
            evidence: "dist/AutoDesignMaker-rust/handoff-status.adm",
            note: "final-gate-must-report-handoff-ready-true-and-final-manifest-package-handoff-delivery-ready",
        },
        HandoffInstruction {
            id: "confirm-final-delivery-package",
            required: !final_delivery_ready,
            status: if final_delivery_ready {
                "ready"
            } else if handoff_ready {
                "blocked"
            } else {
                "waiting-for-strict-gate"
            },
            estimate: "0.1h-after-strict-release-gate",
            command: "cargo run -q -p adm-cli -- finalize-handoff-package",
            evidence: "dist/AutoDesignMaker-rust/final-handoff-manifest.adm",
            note: "requires-final-handoff-manifest-delivery_ready-true-before-full-completion",
        },
        HandoffInstruction {
            id: "decide-source-handoff-policy",
            required: !source_policy_ready,
            status: if source_policy_ready {
                "ready"
            } else {
                "manual-decision"
            },
            estimate: "0.5-2h",
            command: "cargo run -q -p adm-cli -- write-source-handoff-policy",
            evidence: "dist/AutoDesignMaker-rust/source-handoff-policy.adm",
            note: "current-package-uses-bundled-source-as-delivery-evidence",
        },
    ];

    if final_package_ready && !final_delivery_ready {
        instructions.push(HandoffInstruction {
            id: "explain-package-vs-delivery-readiness",
            required: false,
            status: "informational",
            estimate: "0h",
            command: "Get-Content .\\dist\\AutoDesignMaker-rust\\final-handoff-manifest.adm",
            evidence: "dist/AutoDesignMaker-rust/final-handoff-manifest.adm",
            note: "package_ready-means-assembled; delivery_ready-requires-handoff_ready-true",
        });
    }

    let report = HandoffInstructionsReport {
        release_dir,
        report_path,
        handoff_status,
        external_acceptance,
        ai_acceptance,
        source_policy,
        final_handoff_manifest,
        handoff_ready,
        external_acceptance_ready,
        unity_ready,
        unity_runtime_ready,
        unity_runtime_runner,
        real_ai_provider_ready,
        ai_acceptance_ready,
        ai_configured_ready,
        ai_invoke_attempted,
        ai_invoke_succeeded,
        external_acceptance_require_ai_invoke,
        source_policy_ready,
        final_package_present,
        final_package_ready,
        final_delivery_ready,
        final_handoff_ready,
        blockers,
        instructions,
    };
    write_handoff_instructions_report(&report)?;
    Ok(report)
}

fn write_handoff_instructions_report(report: &HandoffInstructionsReport) -> AdmResult<()> {
    if let Some(parent) = report
        .report_path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        fs::create_dir_all(parent)?;
    }
    fs::write(&report.report_path, report.render())?;
    Ok(())
}

fn report_value(document: &str, key: &str) -> Option<String> {
    let prefix = format!("{key}=");
    document.lines().find_map(|line| {
        line.strip_prefix(&prefix)
            .map(single_line_cli_value)
            .filter(|value| !value.is_empty())
    })
}

fn report_values(document: &str, key: &str) -> Vec<String> {
    let prefix = format!("{key}=");
    document
        .lines()
        .filter_map(|line| {
            line.strip_prefix(&prefix)
                .map(single_line_cli_value)
                .filter(|value| !value.is_empty())
        })
        .collect()
}

fn unity_candidate_values(document: &str) -> Vec<String> {
    document
        .lines()
        .filter_map(|line| {
            line.trim_start()
                .strip_prefix("- source=")
                .map(|value| format!("source={}", single_line_cli_value(value)))
                .filter(|value| !value.is_empty())
        })
        .collect()
}

fn ai_provider_diagnostic_values(document: &str) -> Vec<String> {
    document
        .lines()
        .filter_map(|line| {
            let trimmed = line.trim();
            let parts = trimmed.split('\t').collect::<Vec<_>>();
            if parts.len() < 3 {
                return None;
            }

            let provider_id = single_line_cli_value(parts[0]);
            let readiness = single_line_cli_value(parts[1]);
            let capabilities = parts[2]
                .strip_prefix("capabilities=")
                .map(single_line_cli_value)
                .unwrap_or_else(|| single_line_cli_value(parts[2]));
            let note = if parts.len() > 3 {
                single_line_cli_value(&parts[3..].join(" "))
            } else {
                "none".to_string()
            };

            Some(format!(
                "provider_id={provider_id}; readiness={readiness}; capabilities={capabilities}; note={note}"
            ))
        })
        .collect()
}

#[derive(Debug, Clone)]
struct SourceBundleFile {
    relative_path: String,
    bytes: u64,
    hash: ContentHash,
}

#[derive(Debug, Clone)]
struct SourceBundleReport {
    source_root: PathBuf,
    bundle_dir: PathBuf,
    report_path: PathBuf,
    stale_cleanup: String,
    files: Vec<SourceBundleFile>,
    total_bytes: u64,
    bundle_hash: ContentHash,
}

impl SourceBundleReport {
    fn ready(&self) -> bool {
        !self.files.is_empty() && self.bundle_dir.exists()
    }

    fn render(&self) -> String {
        let mut document = String::from("# Rust Source Bundle\n");
        document.push_str(&format!("ready={}\n", self.ready()));
        document.push_str(&format!("source_root={}\n", self.source_root.display()));
        document.push_str(&format!("bundle_dir={}\n", self.bundle_dir.display()));
        document.push_str(&format!("report_path={}\n", self.report_path.display()));
        document.push_str("source_handoff_mode=bundled\n");
        document.push_str(&format!("file_count={}\n", self.files.len()));
        document.push_str(&format!("total_bytes={}\n", self.total_bytes));
        document.push_str(&format!("bundle_hash={}\n", self.bundle_hash));
        for excluded in source_bundle_excluded_dirs() {
            document.push_str(&format!("excluded_dir={excluded}\n"));
        }
        document.push_str(&format!("stale_cleanup={}\n", self.stale_cleanup));
        for file in &self.files {
            document.push_str(&format!(
                "- path={}; bytes={}; hash={}\n",
                file.relative_path, file.bytes, file.hash
            ));
        }
        document
    }
}

fn stage_source_bundle(
    source_root: PathBuf,
    bundle_dir: PathBuf,
    report_path: PathBuf,
) -> AdmResult<SourceBundleReport> {
    let source_root = fs::canonicalize(source_root)?;
    let mut source_files = Vec::new();
    collect_source_files(&source_root, &source_root, &mut source_files)?;
    source_files.sort();

    let (bundle_dir, stale_cleanup) = prepare_source_bundle_dir(&source_root, &bundle_dir)?;
    let mut records = Vec::new();
    let mut total_bytes = 0u64;
    let mut aggregate = Vec::new();
    for relative_path in source_files {
        let source_file = source_root.join(&relative_path);
        let bytes = fs::read(&source_file)?;
        let hash = ContentHash::from_bytes(&bytes);
        let normalized = normalize_relative_path(&relative_path);
        let target_file = bundle_dir.join(&relative_path);
        if let Some(parent) = target_file.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(&target_file, &bytes)?;
        total_bytes += bytes.len() as u64;
        aggregate.extend_from_slice(normalized.as_bytes());
        aggregate.extend_from_slice(bytes.len().to_string().as_bytes());
        aggregate.extend_from_slice(hash.as_str().as_bytes());
        records.push(SourceBundleFile {
            relative_path: normalized,
            bytes: bytes.len() as u64,
            hash,
        });
    }
    let bundle_hash = ContentHash::from_bytes(&aggregate);
    let report = SourceBundleReport {
        source_root,
        bundle_dir,
        report_path,
        stale_cleanup,
        files: records,
        total_bytes,
        bundle_hash,
    };
    write_source_bundle_report(&report)?;
    Ok(report)
}

fn prepare_source_bundle_dir(
    source_root: &Path,
    bundle_dir: &Path,
) -> AdmResult<(PathBuf, String)> {
    let bundle_dir_abs = absolute_output_path(bundle_dir)?;
    if source_root == bundle_dir_abs || source_root.starts_with(&bundle_dir_abs) {
        return Err(adm_foundation::AdmError::invalid_input(format!(
            "source bundle dir must not be the source root or a parent of the source root: {}",
            bundle_dir_abs.display()
        )));
    }

    let stale_cleanup = if bundle_dir_abs.exists() {
        fs::remove_dir_all(&bundle_dir_abs)?;
        "removed_existing_bundle_dir"
    } else {
        "created_clean_bundle_dir"
    };
    fs::create_dir_all(&bundle_dir_abs)?;
    Ok((bundle_dir_abs, stale_cleanup.to_string()))
}

fn absolute_output_path(path: &Path) -> AdmResult<PathBuf> {
    if path.exists() {
        return Ok(fs::canonicalize(path)?);
    }
    let parent = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    fs::create_dir_all(parent)?;
    let parent_abs = fs::canonicalize(parent)?;
    let name = path.file_name().ok_or_else(|| {
        adm_foundation::AdmError::invalid_input(format!(
            "output path must include a final directory name: {}",
            path.display()
        ))
    })?;
    Ok(parent_abs.join(name))
}

fn collect_source_files(root: &Path, current: &Path, files: &mut Vec<PathBuf>) -> AdmResult<()> {
    for entry in fs::read_dir(current)? {
        let entry = entry?;
        let path = entry.path();
        let file_type = entry.file_type()?;
        if file_type.is_dir() {
            let name = entry.file_name();
            let name = name.to_string_lossy();
            if source_bundle_excluded_dirs()
                .iter()
                .any(|excluded| name.eq_ignore_ascii_case(excluded))
            {
                continue;
            }
            collect_source_files(root, &path, files)?;
        } else if file_type.is_file() {
            let relative = path.strip_prefix(root).map_err(|error| {
                adm_foundation::AdmError::invalid_input(format!(
                    "source file is outside source root: {error}"
                ))
            })?;
            files.push(relative.to_path_buf());
        }
    }
    Ok(())
}

fn write_source_bundle_report(report: &SourceBundleReport) -> AdmResult<()> {
    if let Some(parent) = report
        .report_path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        fs::create_dir_all(parent)?;
    }
    fs::write(&report.report_path, report.render())?;
    Ok(())
}

#[derive(Debug, Clone, Copy)]
struct HandoffBundleSource {
    name: &'static str,
    required: bool,
}

#[derive(Debug, Clone)]
struct HandoffBundleEntry {
    name: String,
    required: bool,
    present: bool,
    copied: bool,
    file_count: usize,
    total_bytes: u64,
    hash: Option<ContentHash>,
}

#[derive(Debug, Clone)]
struct HandoffBundleReport {
    dist_root: PathBuf,
    bundle_dir: PathBuf,
    report_path: PathBuf,
    manifest_bundle_path: PathBuf,
    stale_cleanup: String,
    excluded_files: Vec<String>,
    entries: Vec<HandoffBundleEntry>,
    file_count: usize,
    total_bytes: u64,
    bundle_hash: ContentHash,
}

impl HandoffBundleReport {
    fn ready(&self) -> bool {
        self.bundle_dir.exists()
            && self.file_count > 0
            && self
                .entries
                .iter()
                .filter(|entry| entry.required)
                .all(|entry| entry.present && entry.copied && entry.file_count > 0)
    }

    fn render(&self) -> String {
        let mut document = String::from("# Handoff Bundle\n");
        document.push_str(&format!("ready={}\n", self.ready()));
        document.push_str(&format!("dist_root={}\n", self.dist_root.display()));
        document.push_str(&format!("bundle_dir={}\n", self.bundle_dir.display()));
        document.push_str(&format!("report_path={}\n", self.report_path.display()));
        document.push_str(&format!(
            "manifest_bundle_path={}\n",
            self.manifest_bundle_path.display()
        ));
        document.push_str(&format!("file_count={}\n", self.file_count));
        document.push_str(&format!("total_bytes={}\n", self.total_bytes));
        document.push_str(&format!("bundle_hash={}\n", self.bundle_hash));
        document.push_str(&format!("stale_cleanup={}\n", self.stale_cleanup));
        for excluded_file in &self.excluded_files {
            document.push_str(&format!("excluded_file={excluded_file}\n"));
        }
        for entry in &self.entries {
            let key = if entry.required {
                "required_dir"
            } else {
                "optional_dir"
            };
            let hash = entry
                .hash
                .as_ref()
                .map(ToString::to_string)
                .unwrap_or_else(|| "none".to_string());
            document.push_str(&format!(
                "{key}={}; present={}; copied={}; files={}; bytes={}; hash={}\n",
                entry.name, entry.present, entry.copied, entry.file_count, entry.total_bytes, hash
            ));
        }
        document
    }
}

fn stage_handoff_bundle(
    dist_root: PathBuf,
    bundle_dir: PathBuf,
    report_path: PathBuf,
) -> AdmResult<HandoffBundleReport> {
    let dist_root = fs::canonicalize(dist_root)?;
    let source_dirs = handoff_bundle_sources()
        .iter()
        .map(|source| dist_root.join(source.name))
        .collect::<Vec<_>>();
    let (bundle_dir, stale_cleanup) =
        prepare_handoff_bundle_dir(&dist_root, &bundle_dir, &source_dirs)?;

    let mut entries = Vec::new();
    let mut file_count = 0usize;
    let mut total_bytes = 0u64;
    let mut bundle_aggregate = Vec::new();
    let mut excluded_files = Vec::new();

    for source in handoff_bundle_sources() {
        let source_dir = dist_root.join(source.name);
        let target_dir = bundle_dir.join(source.name);
        let mut entry = HandoffBundleEntry {
            name: source.name.to_string(),
            required: source.required,
            present: source_dir.exists(),
            copied: false,
            file_count: 0,
            total_bytes: 0,
            hash: None,
        };

        bundle_aggregate.extend_from_slice(source.name.as_bytes());
        bundle_aggregate.extend_from_slice(source.required.to_string().as_bytes());

        if !entry.present {
            bundle_aggregate.extend_from_slice(b"missing");
            entries.push(entry);
            continue;
        }
        if !source_dir.is_dir() {
            return Err(adm_foundation::AdmError::invalid_input(format!(
                "handoff bundle source must be a directory: {}",
                source_dir.display()
            )));
        }

        let mut files = Vec::new();
        collect_handoff_bundle_files(&source_dir, &source_dir, &mut files)?;
        let excluded_for_source = handoff_bundle_excluded_files(source.name);
        files.retain(|relative_path| {
            let normalized = normalize_relative_path(relative_path);
            let excluded = excluded_for_source
                .iter()
                .any(|candidate| normalized.eq_ignore_ascii_case(candidate));
            if excluded {
                excluded_files.push(format!("{}/{}", source.name, normalized));
            }
            !excluded
        });
        files.sort();

        let mut entry_aggregate = Vec::new();
        for relative_path in files {
            let source_file = source_dir.join(&relative_path);
            let normalized = normalize_relative_path(&relative_path);
            let mut bytes = fs::read(&source_file)?;
            if source.name == "AutoDesignMaker-rust"
                && normalized.eq_ignore_ascii_case("final-acceptance-run.adm")
            {
                let text = String::from_utf8_lossy(&bytes);
                bytes = portable_final_acceptance_run_for_bundle(&text).into_bytes();
            } else if source.name == "AutoDesignMaker-rust"
                && normalized.eq_ignore_ascii_case("release-acceptance.adm")
            {
                let text = String::from_utf8_lossy(&bytes);
                bytes = portable_release_acceptance_for_bundle(&text).into_bytes();
            }
            let hash = ContentHash::from_bytes(&bytes);
            let target_file = target_dir.join(&relative_path);
            if let Some(parent) = target_file.parent() {
                fs::create_dir_all(parent)?;
            }
            fs::write(&target_file, &bytes)?;

            let byte_count = bytes.len() as u64;
            entry.file_count += 1;
            entry.total_bytes += byte_count;
            file_count += 1;
            total_bytes += byte_count;
            entry_aggregate.extend_from_slice(normalized.as_bytes());
            entry_aggregate.extend_from_slice(byte_count.to_string().as_bytes());
            entry_aggregate.extend_from_slice(hash.as_str().as_bytes());
        }

        entry.copied = true;
        entry.hash = Some(ContentHash::from_bytes(&entry_aggregate));
        bundle_aggregate.extend_from_slice(entry.file_count.to_string().as_bytes());
        bundle_aggregate.extend_from_slice(entry.total_bytes.to_string().as_bytes());
        if let Some(hash) = &entry.hash {
            bundle_aggregate.extend_from_slice(hash.as_str().as_bytes());
        }
        entries.push(entry);
    }

    let bundle_hash = ContentHash::from_bytes(&bundle_aggregate);
    let report = HandoffBundleReport {
        dist_root,
        manifest_bundle_path: bundle_dir.join("handoff-bundle-manifest.adm"),
        bundle_dir,
        report_path,
        stale_cleanup,
        excluded_files,
        entries,
        file_count,
        total_bytes,
        bundle_hash,
    };
    write_handoff_bundle_report(&report)?;
    Ok(report)
}

fn prepare_handoff_bundle_dir(
    dist_root: &Path,
    bundle_dir: &Path,
    source_dirs: &[PathBuf],
) -> AdmResult<(PathBuf, String)> {
    let bundle_dir_abs = absolute_output_path(bundle_dir)?;
    if dist_root == bundle_dir_abs || dist_root.starts_with(&bundle_dir_abs) {
        return Err(adm_foundation::AdmError::invalid_input(format!(
            "handoff bundle dir must not be the dist root or a parent of the dist root: {}",
            bundle_dir_abs.display()
        )));
    }

    for source_dir in source_dirs {
        if !source_dir.exists() {
            continue;
        }
        let source_dir_abs = fs::canonicalize(source_dir)?;
        if source_dir_abs == bundle_dir_abs
            || source_dir_abs.starts_with(&bundle_dir_abs)
            || bundle_dir_abs.starts_with(&source_dir_abs)
        {
            return Err(adm_foundation::AdmError::invalid_input(format!(
                "handoff bundle dir must not overlap a source delivery dir: {}",
                bundle_dir_abs.display()
            )));
        }
    }

    let stale_cleanup = if bundle_dir_abs.exists() {
        fs::remove_dir_all(&bundle_dir_abs)?;
        "removed_existing_bundle_dir"
    } else {
        "created_clean_bundle_dir"
    };
    fs::create_dir_all(&bundle_dir_abs)?;
    Ok((bundle_dir_abs, stale_cleanup.to_string()))
}

fn collect_handoff_bundle_files(
    root: &Path,
    current: &Path,
    files: &mut Vec<PathBuf>,
) -> AdmResult<()> {
    for entry in fs::read_dir(current)? {
        let entry = entry?;
        let path = entry.path();
        let file_type = entry.file_type()?;
        if file_type.is_dir() {
            collect_handoff_bundle_files(root, &path, files)?;
        } else if file_type.is_file() {
            let relative = path.strip_prefix(root).map_err(|error| {
                adm_foundation::AdmError::invalid_input(format!(
                    "handoff bundle file is outside source dir: {error}"
                ))
            })?;
            files.push(relative.to_path_buf());
        }
    }
    Ok(())
}

fn write_handoff_bundle_report(report: &HandoffBundleReport) -> AdmResult<()> {
    if let Some(parent) = report
        .report_path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        fs::create_dir_all(parent)?;
    }
    fs::write(&report.report_path, report.render())?;
    fs::write(&report.manifest_bundle_path, report.render())?;
    Ok(())
}

#[derive(Debug, Clone, Copy)]
struct HandoffEvidenceSource {
    name: &'static str,
    required: bool,
}

#[derive(Debug, Clone)]
struct HandoffEvidenceFile {
    name: String,
    required: bool,
    present: bool,
    copied: bool,
    bytes: u64,
    hash: Option<ContentHash>,
}

#[derive(Debug, Clone)]
struct HandoffEvidenceReport {
    release_dir: PathBuf,
    bundle_dir: PathBuf,
    evidence_dir: PathBuf,
    report_path: PathBuf,
    manifest_bundle_path: PathBuf,
    stale_cleanup: String,
    handoff_ready: bool,
    external_acceptance_ready: bool,
    ai_acceptance_ready: bool,
    files: Vec<HandoffEvidenceFile>,
    total_bytes: u64,
    evidence_hash: ContentHash,
}

impl HandoffEvidenceReport {
    fn ready(&self) -> bool {
        self.evidence_dir.exists()
            && self
                .files
                .iter()
                .filter(|file| file.required)
                .all(|file| file.present && file.copied)
    }

    fn render(&self) -> String {
        let mut document = String::from("# Handoff Evidence\n");
        document.push_str(&format!("ready={}\n", self.ready()));
        document.push_str(&format!("release_dir={}\n", self.release_dir.display()));
        document.push_str(&format!("bundle_dir={}\n", self.bundle_dir.display()));
        document.push_str(&format!("evidence_dir={}\n", self.evidence_dir.display()));
        document.push_str(&format!("report_path={}\n", self.report_path.display()));
        document.push_str(&format!(
            "manifest_bundle_path={}\n",
            self.manifest_bundle_path.display()
        ));
        document.push_str(&format!("file_count={}\n", self.files.len()));
        document.push_str(&format!("total_bytes={}\n", self.total_bytes));
        document.push_str(&format!("evidence_hash={}\n", self.evidence_hash));
        document.push_str(&format!("stale_cleanup={}\n", self.stale_cleanup));
        document.push_str(&format!("handoff_ready={}\n", self.handoff_ready));
        document.push_str(&format!(
            "external_acceptance_ready={}\n",
            self.external_acceptance_ready
        ));
        document.push_str(&format!(
            "ai_acceptance_ready={}\n",
            self.ai_acceptance_ready
        ));
        for file in &self.files {
            let hash = file
                .hash
                .as_ref()
                .map(ToString::to_string)
                .unwrap_or_else(|| "none".to_string());
            document.push_str(&format!(
                "evidence_file={}; required={}; present={}; copied={}; bytes={}; hash={}\n",
                file.name, file.required, file.present, file.copied, file.bytes, hash
            ));
        }
        document
    }
}

fn sync_handoff_evidence(
    release_dir: PathBuf,
    bundle_dir: PathBuf,
    report_path: PathBuf,
) -> AdmResult<HandoffEvidenceReport> {
    let release_dir = fs::canonicalize(release_dir)?;
    let bundle_dir = fs::canonicalize(bundle_dir)?;
    let (evidence_dir, stale_cleanup) = prepare_handoff_evidence_dir(&bundle_dir)?;

    let handoff_status = AcceptanceSnapshot::load(release_dir.join("handoff-status.adm"))?;
    let external_acceptance =
        AcceptanceSnapshot::load(release_dir.join("external-acceptance.adm"))?;
    let ai_acceptance = AcceptanceSnapshot::load(release_dir.join("ai-acceptance.adm"))?;
    let mut files = Vec::new();
    let mut total_bytes = 0u64;
    let mut aggregate = Vec::new();

    for source in handoff_evidence_sources() {
        let source_file = release_dir.join(source.name);
        let target_file = evidence_dir.join(source.name);
        let present = source_file.exists() && source_file.is_file();
        let mut file = HandoffEvidenceFile {
            name: source.name.to_string(),
            required: source.required,
            present,
            copied: false,
            bytes: 0,
            hash: None,
        };

        aggregate.extend_from_slice(source.name.as_bytes());
        aggregate.extend_from_slice(source.required.to_string().as_bytes());
        if present {
            let mut bytes = fs::read(&source_file)?;
            if source.name == "release-acceptance.adm" {
                let text = String::from_utf8_lossy(&bytes);
                bytes = portable_release_acceptance_for_bundle(&text).into_bytes();
            } else if source.name == "handoff-instructions.adm" {
                let text = String::from_utf8_lossy(&bytes);
                bytes = portable_handoff_instructions_for_bundle(&text).into_bytes();
            }
            let hash = ContentHash::from_bytes(&bytes);
            fs::write(&target_file, &bytes)?;
            file.copied = true;
            file.bytes = bytes.len() as u64;
            file.hash = Some(hash);
            total_bytes += file.bytes;
            aggregate.extend_from_slice(file.bytes.to_string().as_bytes());
            if let Some(hash) = &file.hash {
                aggregate.extend_from_slice(hash.as_str().as_bytes());
            }
        } else {
            aggregate.extend_from_slice(b"missing");
        }
        files.push(file);
    }

    let evidence_hash = ContentHash::from_bytes(&aggregate);
    let report = HandoffEvidenceReport {
        release_dir,
        manifest_bundle_path: evidence_dir.join("handoff-evidence-manifest.adm"),
        bundle_dir,
        evidence_dir,
        report_path,
        stale_cleanup,
        handoff_ready: handoff_status.bool_value("ready"),
        external_acceptance_ready: external_acceptance.bool_value("ready"),
        ai_acceptance_ready: ai_acceptance.bool_value("ready"),
        files,
        total_bytes,
        evidence_hash,
    };
    write_handoff_evidence_report(&report)?;
    Ok(report)
}

fn prepare_handoff_evidence_dir(bundle_dir: &Path) -> AdmResult<(PathBuf, String)> {
    let evidence_dir = bundle_dir.join("evidence");
    if evidence_dir == bundle_dir || !evidence_dir.starts_with(bundle_dir) {
        return Err(adm_foundation::AdmError::invalid_input(format!(
            "handoff evidence dir must be inside the handoff bundle: {}",
            evidence_dir.display()
        )));
    }
    let stale_cleanup = if evidence_dir.exists() {
        fs::remove_dir_all(&evidence_dir)?;
        "removed_existing_evidence_dir"
    } else {
        "created_clean_evidence_dir"
    };
    fs::create_dir_all(&evidence_dir)?;
    Ok((evidence_dir, stale_cleanup.to_string()))
}

fn write_handoff_evidence_report(report: &HandoffEvidenceReport) -> AdmResult<()> {
    if let Some(parent) = report
        .report_path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        fs::create_dir_all(parent)?;
    }
    fs::write(&report.report_path, report.render())?;
    fs::write(&report.manifest_bundle_path, report.render())?;
    Ok(())
}

fn handoff_evidence_sources() -> [HandoffEvidenceSource; 8] {
    [
        HandoffEvidenceSource {
            name: "release-acceptance.adm",
            required: true,
        },
        HandoffEvidenceSource {
            name: "source-manifest.adm",
            required: true,
        },
        HandoffEvidenceSource {
            name: "handoff-bundle-manifest.adm",
            required: true,
        },
        HandoffEvidenceSource {
            name: "external-acceptance.adm",
            required: true,
        },
        HandoffEvidenceSource {
            name: "ai-acceptance.adm",
            required: true,
        },
        HandoffEvidenceSource {
            name: "handoff-status.adm",
            required: true,
        },
        HandoffEvidenceSource {
            name: "source-handoff-policy.adm",
            required: true,
        },
        HandoffEvidenceSource {
            name: "handoff-instructions.adm",
            required: true,
        },
    ]
}

#[derive(Debug, Clone)]
struct FinalHandoffPackageFile {
    relative_path: String,
    bytes: u64,
    hash: ContentHash,
}

#[derive(Debug, Clone)]
struct FinalHandoffPackageReport {
    bundle_dir: PathBuf,
    report_path: PathBuf,
    manifest_bundle_path: PathBuf,
    bundle_manifest: AcceptanceSnapshot,
    evidence_manifest: AcceptanceSnapshot,
    required_dirs: Vec<(String, bool)>,
    required_files: Vec<(String, bool)>,
    files: Vec<FinalHandoffPackageFile>,
    total_bytes: u64,
    package_hash: ContentHash,
}

impl FinalHandoffPackageReport {
    fn package_ready(&self) -> bool {
        self.bundle_dir.exists()
            && self.bundle_manifest.present
            && self.bundle_manifest.bool_value("ready")
            && self.evidence_manifest.present
            && self.evidence_manifest.bool_value("ready")
            && self.required_dirs.iter().all(|(_, present)| *present)
            && self.required_files.iter().all(|(_, present)| *present)
            && !self.files.is_empty()
    }

    fn handoff_ready(&self) -> bool {
        self.evidence_manifest.bool_value("handoff_ready")
    }

    fn external_acceptance_ready(&self) -> bool {
        self.evidence_manifest
            .bool_value("external_acceptance_ready")
    }

    fn ai_acceptance_ready(&self) -> bool {
        self.evidence_manifest.bool_value("ai_acceptance_ready")
    }

    fn delivery_ready(&self) -> bool {
        self.package_ready() && self.handoff_ready()
    }

    fn ready(&self) -> bool {
        self.package_ready()
    }

    fn render(&self) -> String {
        let mut document = String::from("# Final Handoff Package\n");
        document.push_str(&format!("ready={}\n", self.ready()));
        document.push_str(&format!("package_ready={}\n", self.package_ready()));
        document.push_str(&format!("delivery_ready={}\n", self.delivery_ready()));
        document.push_str(&format!("bundle_dir={}\n", self.bundle_dir.display()));
        document.push_str(&format!("report_path={}\n", self.report_path.display()));
        document.push_str(&format!(
            "manifest_bundle_path={}\n",
            self.manifest_bundle_path.display()
        ));
        document.push_str(&format!(
            "handoff_bundle_manifest_present={}\n",
            self.bundle_manifest.present
        ));
        document.push_str(&format!(
            "handoff_bundle_ready={}\n",
            self.bundle_manifest.bool_value("ready")
        ));
        document.push_str(&format!(
            "handoff_evidence_manifest_present={}\n",
            self.evidence_manifest.present
        ));
        document.push_str(&format!(
            "handoff_evidence_ready={}\n",
            self.evidence_manifest.bool_value("ready")
        ));
        document.push_str(&format!("handoff_ready={}\n", self.handoff_ready()));
        document.push_str(&format!(
            "external_acceptance_ready={}\n",
            self.external_acceptance_ready()
        ));
        document.push_str(&format!(
            "ai_acceptance_ready={}\n",
            self.ai_acceptance_ready()
        ));
        document.push_str(&format!("file_count={}\n", self.files.len()));
        document.push_str(&format!("total_bytes={}\n", self.total_bytes));
        document.push_str(&format!("package_hash={}\n", self.package_hash));
        document.push_str("excluded_file=final-handoff-manifest.adm\n");
        for (dir, present) in &self.required_dirs {
            document.push_str(&format!("required_dir={dir}; present={present}\n"));
        }
        for (file, present) in &self.required_files {
            document.push_str(&format!("required_file={file}; present={present}\n"));
        }
        for file in &self.files {
            document.push_str(&format!(
                "- path={}; bytes={}; hash={}\n",
                file.relative_path, file.bytes, file.hash
            ));
        }
        document
    }
}

fn finalize_handoff_package(
    bundle_dir: PathBuf,
    report_path: PathBuf,
) -> AdmResult<FinalHandoffPackageReport> {
    let bundle_dir = fs::canonicalize(bundle_dir)?;
    let manifest_bundle_path = bundle_dir.join("final-handoff-manifest.adm");
    let bundle_manifest = AcceptanceSnapshot::load(bundle_dir.join("handoff-bundle-manifest.adm"))?;
    let evidence_manifest = AcceptanceSnapshot::load(
        bundle_dir
            .join("evidence")
            .join("handoff-evidence-manifest.adm"),
    )?;
    let handoff_instructions =
        AcceptanceSnapshot::load(bundle_dir.join("evidence").join("handoff-instructions.adm"))?;
    write_handoff_bundle_readme(
        &bundle_dir,
        &bundle_manifest,
        &evidence_manifest,
        &handoff_instructions,
    )?;

    let required_dirs = final_handoff_required_dirs()
        .iter()
        .map(|dir| (dir.to_string(), bundle_dir.join(dir).is_dir()))
        .collect::<Vec<_>>();
    let required_files = final_handoff_required_files()
        .iter()
        .map(|file| (file.to_string(), bundle_dir.join(file).is_file()))
        .collect::<Vec<_>>();

    let mut relative_files = Vec::new();
    collect_handoff_bundle_files(&bundle_dir, &bundle_dir, &mut relative_files)?;
    relative_files.retain(|relative_path| {
        !normalize_relative_path(relative_path).eq_ignore_ascii_case("final-handoff-manifest.adm")
    });
    relative_files.sort();

    let mut files = Vec::new();
    let mut total_bytes = 0u64;
    let mut aggregate = Vec::new();
    for relative_path in relative_files {
        let source_file = bundle_dir.join(&relative_path);
        let bytes = fs::read(&source_file)?;
        let hash = ContentHash::from_bytes(&bytes);
        let normalized = normalize_relative_path(&relative_path);
        let byte_count = bytes.len() as u64;
        total_bytes += byte_count;
        aggregate.extend_from_slice(normalized.as_bytes());
        aggregate.extend_from_slice(byte_count.to_string().as_bytes());
        aggregate.extend_from_slice(hash.as_str().as_bytes());
        files.push(FinalHandoffPackageFile {
            relative_path: normalized,
            bytes: byte_count,
            hash,
        });
    }

    let report = FinalHandoffPackageReport {
        bundle_dir,
        report_path,
        manifest_bundle_path,
        bundle_manifest,
        evidence_manifest,
        required_dirs,
        required_files,
        files,
        total_bytes,
        package_hash: ContentHash::from_bytes(&aggregate),
    };
    write_final_handoff_package_report(&report)?;
    Ok(report)
}

fn write_final_handoff_package_report(report: &FinalHandoffPackageReport) -> AdmResult<()> {
    if let Some(parent) = report
        .report_path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        fs::create_dir_all(parent)?;
    }
    fs::write(&report.report_path, report.render())?;
    fs::write(&report.manifest_bundle_path, report.render())?;
    Ok(())
}

fn final_handoff_required_dirs() -> [&'static str; 3] {
    ["AutoDesignMaker-rust", "source-bundle", "evidence"]
}

fn final_handoff_required_files() -> [&'static str; 1] {
    ["HANDOFF_README.txt"]
}

fn write_handoff_bundle_readme(
    bundle_dir: &Path,
    bundle_manifest: &AcceptanceSnapshot,
    evidence_manifest: &AcceptanceSnapshot,
    handoff_instructions: &AcceptanceSnapshot,
) -> AdmResult<()> {
    fs::write(
        bundle_dir.join("HANDOFF_README.txt"),
        render_handoff_bundle_readme(bundle_manifest, evidence_manifest, handoff_instructions),
    )?;
    Ok(())
}

fn portable_handoff_command_data_root(value: String, data_roots: &[String]) -> String {
    let mut portable = value;
    for data_root in data_roots {
        let trimmed = data_root.trim();
        if trimmed.is_empty()
            || trimmed == "none"
            || trimmed == "unknown"
            || (trimmed.starts_with('<') && trimmed.ends_with('>'))
        {
            continue;
        }

        portable = portable.replace(
            &format!("-DataRoot {}", powershell_arg(trimmed)),
            "-DataRoot '<data_root>'",
        );
        portable = portable.replace(&format!("-DataRoot {}", trimmed), "-DataRoot '<data_root>'");
    }
    portable
}

fn portable_handoff_command_script_paths(value: String, rust_roots: &[String]) -> String {
    let mut portable = value;
    for rust_root in rust_roots {
        let trimmed = rust_root.trim();
        if trimmed.is_empty()
            || trimmed == "none"
            || trimmed == "unknown"
            || (trimmed.starts_with('<') && trimmed.ends_with('>'))
        {
            continue;
        }

        let root = trimmed.trim_end_matches(|ch| ch == '\\' || ch == '/');
        let script_dirs = [format!("{root}\\scripts\\"), format!("{root}/scripts/")];
        for script_dir in script_dirs {
            portable = portable.replace(&format!("-File {}", script_dir), "-File .\\scripts\\");
            portable = portable.replace(
                &format!("-File '{}", script_dir.replace('\'', "''")),
                "-File '.\\scripts\\",
            );
            portable = portable.replace(&format!("-File \"{script_dir}"), "-File \".\\scripts\\");
        }
    }
    portable
}

fn portable_handoff_command_argument_values(
    value: String,
    argument_name: &str,
    originals: &[String],
    replacement: &str,
) -> String {
    let mut portable = value;
    for original in originals {
        let trimmed = original.trim();
        if trimmed.is_empty()
            || trimmed == "none"
            || trimmed == "unknown"
            || (trimmed.starts_with('<') && trimmed.ends_with('>'))
        {
            continue;
        }

        let argument = format!("-{argument_name}");
        portable = portable.replace(
            &format!("{argument} {}", powershell_arg(trimmed)),
            &format!("{argument} {replacement}"),
        );
        portable = portable.replace(
            &format!("{argument} '{}'", trimmed.replace('\'', "''")),
            &format!("{argument} {replacement}"),
        );
        portable = portable.replace(
            &format!("{argument} \"{trimmed}\""),
            &format!("{argument} {replacement}"),
        );
        portable = portable.replace(
            &format!("{argument} {trimmed}"),
            &format!("{argument} {replacement}"),
        );
    }
    portable
}

fn portable_report_path_file_name(value: &str) -> Option<String> {
    let trimmed = value.trim().trim_matches('"').trim_matches('\'');
    trimmed
        .rsplit(|ch| ch == '\\' || ch == '/')
        .find(|part| !part.trim().is_empty())
        .map(|part| part.to_string())
}

fn portable_release_acceptance_for_bundle(text: &str) -> String {
    let smoke_executable =
        report_value(text, "smoke_executable").unwrap_or_else(|| "unknown".to_string());
    let executable_name = portable_report_path_file_name(&smoke_executable)
        .unwrap_or_else(|| "AutoDesignMaker-rust.exe".to_string());
    let portable_executable = format!(".\\AutoDesignMaker-rust\\{executable_name}");

    let mut document = text
        .lines()
        .map(|line| {
            if line.starts_with("smoke_executable=") {
                return format!("smoke_executable={portable_executable}");
            }

            if let Some(command) = line.strip_prefix("smoke_command=") {
                let command = command.trim();
                if smoke_executable != "unknown" && command.starts_with(&smoke_executable) {
                    let suffix = command[smoke_executable.len()..].trim_start();
                    if suffix.is_empty() {
                        return format!("smoke_command={portable_executable}");
                    }
                    return format!("smoke_command={portable_executable} {suffix}");
                }
                return format!("smoke_command={portable_executable} --smoke");
            }

            line.to_string()
        })
        .collect::<Vec<_>>()
        .join("\n");
    document.push('\n');

    if report_value(&document, "handoff_bundle_smoke_command_mode").is_none() {
        document.push_str("handoff_bundle_smoke_command_mode=portable-package-root-relative\n");
        document.push_str("handoff_bundle_smoke_command_working_dir=handoff-bundle-root\n");
        document.push_str("handoff_bundle_smoke_executable_placeholder=.\\AutoDesignMaker-rust\\AutoDesignMaker-rust.exe\n");
    }

    document
}

fn portable_final_acceptance_run_for_bundle(text: &str) -> String {
    let data_root = report_value(text, "data_root").unwrap_or_else(|| "unknown".to_string());
    let rust_root = report_value(text, "rust_root").unwrap_or_else(|| "unknown".to_string());
    let instructions_path =
        report_value(text, "instructions_path").unwrap_or_else(|| "unknown".to_string());
    let command_data_roots = [data_root];
    let command_rust_roots = [rust_root];
    let command_instructions_paths = [instructions_path];
    let mut document = text
        .lines()
        .map(|line| {
            let line = portable_handoff_command_data_root(line.to_string(), &command_data_roots);
            let line = portable_handoff_command_script_paths(line, &command_rust_roots);
            portable_handoff_command_argument_values(
                line,
                "InstructionsPath",
                &command_instructions_paths,
                ".\\dist\\handoff-bundle\\evidence\\handoff-instructions.adm",
            )
        })
        .collect::<Vec<_>>()
        .join("\n");
    document.push('\n');

    if report_value(&document, "handoff_bundle_command_data_root_mode").is_none() {
        document.push_str("handoff_bundle_command_data_root_mode=portable-placeholder\n");
        document.push_str("handoff_bundle_command_data_root_placeholder=<data_root>\n");
    }
    if report_value(&document, "handoff_bundle_command_script_path_mode").is_none() {
        document.push_str("handoff_bundle_command_script_path_mode=portable-workspace-relative\n");
        document.push_str("handoff_bundle_command_script_path_placeholder=.\\scripts\n");
    }
    if report_value(&document, "handoff_bundle_command_instructions_path_mode").is_none() {
        document.push_str(
            "handoff_bundle_command_instructions_path_mode=portable-workspace-relative\n",
        );
        document.push_str(
            "handoff_bundle_command_instructions_path_placeholder=.\\dist\\handoff-bundle\\evidence\\handoff-instructions.adm\n",
        );
    }

    document
}

fn portable_handoff_instructions_for_bundle(text: &str) -> String {
    let command_data_roots = [
        report_value(text, "external_acceptance_data_root")
            .unwrap_or_else(|| "unknown".to_string()),
        report_value(text, "ai_acceptance_data_root").unwrap_or_else(|| "unknown".to_string()),
    ];
    let mut document = text
        .lines()
        .map(|line| portable_handoff_command_data_root(line.to_string(), &command_data_roots))
        .collect::<Vec<_>>()
        .join("\n");
    document.push('\n');

    if report_value(&document, "handoff_bundle_command_data_root_mode").is_none() {
        document.push_str("handoff_bundle_command_data_root_mode=portable-placeholder\n");
        document.push_str("handoff_bundle_command_data_root_placeholder=<data_root>\n");
    }

    document
}

fn render_handoff_bundle_readme(
    bundle_manifest: &AcceptanceSnapshot,
    evidence_manifest: &AcceptanceSnapshot,
    handoff_instructions: &AcceptanceSnapshot,
) -> String {
    let external_acceptance_data_root = handoff_instructions
        .value("external_acceptance_data_root")
        .unwrap_or_else(|| "unknown".to_string());
    let ai_acceptance_data_root = handoff_instructions
        .value("ai_acceptance_data_root")
        .unwrap_or_else(|| "unknown".to_string());
    let command_data_roots = [
        external_acceptance_data_root.clone(),
        ai_acceptance_data_root.clone(),
    ];
    let strict_gate_command = handoff_instructions
        .value("strict_gate_command")
        .unwrap_or_else(|| {
            "powershell -ExecutionPolicy Bypass -File .\\scripts\\release_gate.ps1 -RequireExternalAcceptance -UnityExe '<path-to-Unity.exe>' -DataRoot '<data_root>'".to_string()
        });
    let strict_gate_command =
        portable_handoff_command_data_root(strict_gate_command, &command_data_roots);
    let strict_gate_ai_invoke_command = handoff_instructions
        .value("strict_gate_ai_invoke_command")
        .unwrap_or_else(|| {
            "powershell -ExecutionPolicy Bypass -File .\\scripts\\release_gate.ps1 -RequireExternalAcceptance -RequireAiInvoke -UnityExe '<path-to-Unity.exe>' -DataRoot '<data_root>'".to_string()
        });
    let strict_gate_ai_invoke_command =
        portable_handoff_command_data_root(strict_gate_ai_invoke_command, &command_data_roots);
    let strict_gate_requires_final_delivery = handoff_instructions
        .value("strict_gate_requires_final_delivery")
        .unwrap_or_else(|| "true".to_string());
    let blocker_count = handoff_instructions
        .value("blocker_count")
        .unwrap_or_else(|| "unknown".to_string());
    let blockers = report_values(&handoff_instructions.text, "blocker");
    let blocker_lines = blockers
        .iter()
        .map(|blocker| format!("blocker={blocker}\n"))
        .collect::<String>();
    let blocker_resolution_count = handoff_instructions
        .value("blocker_resolution_count")
        .unwrap_or_else(|| "unknown".to_string());
    let blocker_resolutions = report_values(&handoff_instructions.text, "blocker_resolution");
    let blocker_resolution_lines = blocker_resolutions
        .iter()
        .map(|resolution| {
            format!(
                "blocker_resolution={}\n",
                portable_handoff_command_data_root(resolution.to_string(), &command_data_roots)
            )
        })
        .collect::<String>();
    let required_instruction_count = handoff_instructions
        .value("required_instruction_count")
        .unwrap_or_else(|| "unknown".to_string());
    let required_blocked_instruction_count = handoff_instructions
        .value("required_blocked_instruction_count")
        .unwrap_or_else(|| "unknown".to_string());
    let required_waiting_instruction_count = handoff_instructions
        .value("required_waiting_instruction_count")
        .unwrap_or_else(|| "unknown".to_string());
    let optional_instruction_count = handoff_instructions
        .value("optional_instruction_count")
        .unwrap_or_else(|| "unknown".to_string());
    let manual_decision_instruction_count = handoff_instructions
        .value("manual_decision_instruction_count")
        .unwrap_or_else(|| "unknown".to_string());
    let next_required_instruction = handoff_instructions
        .value("next_required_instruction")
        .unwrap_or_else(|| "unknown".to_string());
    let next_required_instruction_status = handoff_instructions
        .value("next_required_instruction_status")
        .unwrap_or_else(|| "unknown".to_string());
    let next_required_instruction_estimate = handoff_instructions
        .value("next_required_instruction_estimate")
        .unwrap_or_else(|| "unknown".to_string());
    let next_required_instruction_command = handoff_instructions
        .value("next_required_instruction_command")
        .unwrap_or_else(|| "unknown".to_string());
    let next_required_instruction_command =
        portable_handoff_command_data_root(next_required_instruction_command, &command_data_roots);
    let next_required_instruction_evidence = handoff_instructions
        .value("next_required_instruction_evidence")
        .unwrap_or_else(|| "unknown".to_string());
    let next_required_instruction_done_when = handoff_instructions
        .value("next_required_instruction_done_when")
        .unwrap_or_else(|| "unknown".to_string());
    let next_required_instruction_note = handoff_instructions
        .value("next_required_instruction_note")
        .unwrap_or_else(|| "unknown".to_string());
    let remaining_required_execution_step_count = handoff_instructions
        .value("remaining_required_execution_step_count")
        .unwrap_or_else(|| "unknown".to_string());
    let remaining_required_execution_steps = report_values(
        &handoff_instructions.text,
        "remaining_required_execution_step",
    );
    let remaining_required_execution_step_lines = remaining_required_execution_steps
        .iter()
        .map(|step| {
            format!(
                "remaining_required_execution_step={}\n",
                portable_handoff_command_data_root(step.to_string(), &command_data_roots)
            )
        })
        .collect::<String>();
    let instruction_count = handoff_instructions
        .value("instruction_count")
        .unwrap_or_else(|| "unknown".to_string());
    let instructions = report_values(&handoff_instructions.text, "instruction");
    let instruction_lines = instructions
        .iter()
        .map(|instruction| {
            format!(
                "instruction={}\n",
                portable_handoff_command_data_root(instruction.to_string(), &command_data_roots)
            )
        })
        .collect::<String>();
    let external_dependency_count = handoff_instructions
        .value("external_dependency_count")
        .unwrap_or_else(|| "unknown".to_string());
    let external_dependencies = report_values(&handoff_instructions.text, "external_dependency");
    let external_dependency_lines = external_dependencies
        .iter()
        .map(|dependency| {
            format!(
                "external_dependency={}\n",
                portable_handoff_command_data_root(dependency.to_string(), &command_data_roots)
            )
        })
        .collect::<String>();
    let operator_input_count = handoff_instructions
        .value("operator_input_count")
        .unwrap_or_else(|| "unknown".to_string());
    let operator_inputs = report_values(&handoff_instructions.text, "operator_input");
    let operator_input_lines = operator_inputs
        .iter()
        .map(|input| format!("operator_input={input}\n"))
        .collect::<String>();
    let final_package_ready = handoff_instructions
        .value("final_package_ready")
        .unwrap_or_else(|| "unknown".to_string());
    let final_delivery_ready = handoff_instructions
        .value("final_delivery_ready")
        .unwrap_or_else(|| "unknown".to_string());
    let final_handoff_ready = handoff_instructions
        .value("final_handoff_ready")
        .unwrap_or_else(|| "unknown".to_string());
    let unity_runtime_runner = handoff_instructions
        .value("unity_runtime_runner")
        .unwrap_or_else(|| "unknown".to_string());
    let unity_selected = handoff_instructions
        .value("unity_selected")
        .unwrap_or_else(|| "unknown".to_string());
    let unity_candidates = handoff_instructions
        .value("unity_candidates")
        .unwrap_or_else(|| "unknown".to_string());
    let unity_candidate_detail_count = handoff_instructions
        .value("unity_candidate_detail_count")
        .unwrap_or_else(|| "unknown".to_string());
    let unity_candidate_details = report_values(&handoff_instructions.text, "unity_candidate");
    let unity_candidate_detail_lines = unity_candidate_details
        .iter()
        .map(|candidate| format!("unity_candidate={candidate}\n"))
        .collect::<String>();
    let ai_provider_id = handoff_instructions
        .value("ai_provider_id")
        .unwrap_or_else(|| "unknown".to_string());
    let ai_provider_model = handoff_instructions
        .value("ai_provider_model")
        .unwrap_or_else(|| "unknown".to_string());
    let ai_diagnostic_readiness = handoff_instructions
        .value("ai_diagnostic_readiness")
        .unwrap_or_else(|| "unknown".to_string());
    let ai_configured_ready = handoff_instructions
        .value("ai_configured_ready")
        .unwrap_or_else(|| "unknown".to_string());
    let real_ai_provider_ready = handoff_instructions
        .value("real_ai_provider_ready")
        .unwrap_or_else(|| "unknown".to_string());
    let real_ai_provider_count = handoff_instructions
        .value("real_ai_provider_count")
        .unwrap_or_else(|| "unknown".to_string());
    let ready_provider_count = handoff_instructions
        .value("ready_provider_count")
        .unwrap_or_else(|| "unknown".to_string());
    let ai_provider_detail_count = handoff_instructions
        .value("ai_provider_detail_count")
        .unwrap_or_else(|| "unknown".to_string());
    let ai_provider_details = report_values(&handoff_instructions.text, "ai_provider");
    let ai_provider_detail_lines = ai_provider_details
        .iter()
        .map(|provider| format!("ai_provider={provider}\n"))
        .collect::<String>();
    let suggested_ai_provider_preset = handoff_instructions
        .value("suggested_ai_provider_preset")
        .unwrap_or_else(|| "openai".to_string());
    let suggested_ai_secret_ref = handoff_instructions
        .value("suggested_ai_secret_ref")
        .unwrap_or_else(|| "default".to_string());
    let suggested_ai_secret_env_var = handoff_instructions
        .value("suggested_ai_secret_env_var")
        .unwrap_or_else(|| "OPENAI_API_KEY".to_string());
    let suggested_ai_secret_requirement = handoff_instructions
        .value("suggested_ai_secret_requirement")
        .unwrap_or_else(|| "env:OPENAI_API_KEY".to_string());
    let suggested_ai_secret_check_command = handoff_instructions
        .value("suggested_ai_secret_check_command")
        .unwrap_or_else(|| {
            "powershell -NoProfile -Command \"[bool]`$env:OPENAI_API_KEY\"".to_string()
        });
    let suggested_ai_secret_session_set_command = handoff_instructions
        .value("suggested_ai_secret_session_set_command")
        .unwrap_or_else(|| "$env:OPENAI_API_KEY='<secret>'".to_string());
    let suggested_unity_archive_id = handoff_instructions
        .value("suggested_unity_archive_id")
        .unwrap_or_else(|| "<archive_id>".to_string());
    let suggested_unity_archive_source = handoff_instructions
        .value("suggested_unity_archive_source")
        .unwrap_or_else(|| "unknown".to_string());
    let suggested_ai_acceptance_command = handoff_instructions
        .value("suggested_ai_acceptance_command")
        .unwrap_or_else(|| {
            "powershell -ExecutionPolicy Bypass -File .\\scripts\\ai_acceptance_gate.ps1 -ProviderId '<provider_id>' -Model '<model>' -Preset '<preset_id>' -SecretRef default -DataRoot '<data_root>' -RequireReady".to_string()
        });
    let suggested_ai_acceptance_command =
        portable_handoff_command_data_root(suggested_ai_acceptance_command, &command_data_roots);
    let suggested_ai_acceptance_invoke_command = handoff_instructions
        .value("suggested_ai_acceptance_invoke_command")
        .unwrap_or_else(|| {
            "powershell -ExecutionPolicy Bypass -File .\\scripts\\ai_acceptance_gate.ps1 -ProviderId '<provider_id>' -Model '<model>' -Preset '<preset_id>' -SecretRef default -DataRoot '<data_root>' -Invoke -RequireReady -RequireInvoke".to_string()
        });
    let suggested_ai_acceptance_invoke_command = portable_handoff_command_data_root(
        suggested_ai_acceptance_invoke_command,
        &command_data_roots,
    );
    let suggested_unity_acceptance_command = handoff_instructions
        .value("suggested_unity_acceptance_command")
        .unwrap_or_else(|| {
            "powershell -ExecutionPolicy Bypass -File .\\scripts\\unity_acceptance_gate.ps1 -ArchiveId '<archive_id>' -UnityExe '<path-to-Unity.exe>' -DataRoot '<data_root>'".to_string()
        });
    let suggested_unity_acceptance_command =
        portable_handoff_command_data_root(suggested_unity_acceptance_command, &command_data_roots);
    let suggested_external_acceptance_command = handoff_instructions
        .value("suggested_external_acceptance_command")
        .unwrap_or_else(|| {
            "powershell -ExecutionPolicy Bypass -File .\\scripts\\external_acceptance_doctor.ps1 -UnityExe '<path-to-Unity.exe>' -DataRoot '<data_root>' -RequireReady".to_string()
        });
    let suggested_external_acceptance_command = portable_handoff_command_data_root(
        suggested_external_acceptance_command,
        &command_data_roots,
    );
    let suggested_strict_release_gate_command = handoff_instructions
        .value("suggested_strict_release_gate_command")
        .unwrap_or_else(|| {
            "powershell -ExecutionPolicy Bypass -File .\\scripts\\release_gate.ps1 -RequireExternalAcceptance -UnityExe '<path-to-Unity.exe>' -DataRoot '<data_root>'".to_string()
        });
    let suggested_strict_release_gate_command = portable_handoff_command_data_root(
        suggested_strict_release_gate_command,
        &command_data_roots,
    );
    let suggested_strict_release_gate_ai_invoke_command = handoff_instructions
        .value("suggested_strict_release_gate_ai_invoke_command")
        .unwrap_or_else(|| {
            "powershell -ExecutionPolicy Bypass -File .\\scripts\\release_gate.ps1 -RequireExternalAcceptance -RequireAiInvoke -UnityExe '<path-to-Unity.exe>' -DataRoot '<data_root>'".to_string()
        });
    let suggested_strict_release_gate_ai_invoke_command = portable_handoff_command_data_root(
        suggested_strict_release_gate_ai_invoke_command,
        &command_data_roots,
    );
    let suggested_operator_preflight_command = handoff_instructions
        .value("suggested_operator_preflight_command")
        .unwrap_or_else(|| {
            "powershell -ExecutionPolicy Bypass -File .\\scripts\\handoff_operator_preflight.ps1 -DataRoot '<data_root>'".to_string()
        });
    let suggested_operator_preflight_command = portable_handoff_command_data_root(
        suggested_operator_preflight_command,
        &command_data_roots,
    );
    let suggested_operator_preflight_require_ready_command = handoff_instructions
        .value("suggested_operator_preflight_require_ready_command")
        .unwrap_or_else(|| {
            "powershell -ExecutionPolicy Bypass -File .\\scripts\\handoff_operator_preflight.ps1 -UnityExe '<path-to-Unity.exe>' -DataRoot '<data_root>' -RequireReady".to_string()
        });
    let suggested_operator_preflight_require_ready_command = portable_handoff_command_data_root(
        suggested_operator_preflight_require_ready_command,
        &command_data_roots,
    );
    let operator_preflight_working_dir = handoff_instructions
        .value("operator_preflight_working_dir")
        .unwrap_or_else(|| "rust-workspace-root-with-scripts-directory".to_string());
    let operator_preflight_bundle_root_supported = handoff_instructions
        .value("operator_preflight_bundle_root_supported")
        .unwrap_or_else(|| "true".to_string());
    let operator_preflight_bundle_root_script = handoff_instructions
        .value("operator_preflight_bundle_root_script")
        .unwrap_or_else(|| "source-bundle/scripts/handoff_operator_preflight.ps1".to_string());
    let operator_preflight_bundle_root_instructions_path = handoff_instructions
        .value("operator_preflight_bundle_root_instructions_path")
        .unwrap_or_else(|| "..\\evidence\\handoff-instructions.adm".to_string());
    let suggested_operator_preflight_bundle_root_command = handoff_instructions
        .value("suggested_operator_preflight_bundle_root_command")
        .unwrap_or_else(|| {
            "powershell -ExecutionPolicy Bypass -File .\\source-bundle\\scripts\\handoff_operator_preflight.ps1 -InstructionsPath ..\\evidence\\handoff-instructions.adm -DataRoot '<data_root>'".to_string()
        });
    let suggested_operator_preflight_bundle_root_command = portable_handoff_command_data_root(
        suggested_operator_preflight_bundle_root_command,
        &command_data_roots,
    );
    let suggested_operator_preflight_bundle_root_require_ready_command = handoff_instructions
        .value("suggested_operator_preflight_bundle_root_require_ready_command")
        .unwrap_or_else(|| {
            "powershell -ExecutionPolicy Bypass -File .\\source-bundle\\scripts\\handoff_operator_preflight.ps1 -InstructionsPath ..\\evidence\\handoff-instructions.adm -UnityExe '<path-to-Unity.exe>' -DataRoot '<data_root>' -RequireReady".to_string()
        });
    let suggested_operator_preflight_bundle_root_require_ready_command =
        portable_handoff_command_data_root(
            suggested_operator_preflight_bundle_root_require_ready_command,
            &command_data_roots,
        );
    let handoff_rehydration_bundle_root_supported = handoff_instructions
        .value("handoff_rehydration_bundle_root_supported")
        .unwrap_or_else(|| "true".to_string());
    let handoff_rehydration_script = handoff_instructions
        .value("handoff_rehydration_script")
        .unwrap_or_else(|| "source-bundle/scripts/rehydrate_handoff_workspace.ps1".to_string());
    let handoff_rehydration_destination_placeholder = handoff_instructions
        .value("handoff_rehydration_destination_placeholder")
        .unwrap_or_else(|| "<path-to-rehydrated-rust-workspace>".to_string());
    let handoff_rehydration_manifest = handoff_instructions
        .value("handoff_rehydration_manifest")
        .unwrap_or_else(|| "dist/handoff-rehydration-manifest.adm".to_string());
    let suggested_handoff_rehydration_command = handoff_instructions
        .value("suggested_handoff_rehydration_command")
        .unwrap_or_else(|| {
            "powershell -ExecutionPolicy Bypass -File .\\source-bundle\\scripts\\rehydrate_handoff_workspace.ps1 -DestinationPath '<path-to-rehydrated-rust-workspace>'".to_string()
        });
    let rehydrated_release_smoke_report = handoff_instructions
        .value("rehydrated_release_smoke_report")
        .unwrap_or_else(|| "dist/AutoDesignMaker-rust/release-acceptance.adm".to_string());
    let rehydrated_release_smoke_command = handoff_instructions
        .value("rehydrated_release_smoke_command")
        .unwrap_or_else(|| {
            ".\\dist\\AutoDesignMaker-rust\\AutoDesignMaker-rust.exe --smoke".to_string()
        });
    let rehydrated_release_smoke_working_dir = handoff_instructions
        .value("rehydrated_release_smoke_working_dir")
        .unwrap_or_else(|| "rehydrated-rust-workspace-root".to_string());
    let final_acceptance_working_dir = handoff_instructions
        .value("final_acceptance_working_dir")
        .unwrap_or_else(|| "rust-workspace-root-after-rehydration-or-original".to_string());
    let final_acceptance_script = handoff_instructions
        .value("final_acceptance_script")
        .unwrap_or_else(|| "scripts/final_handoff_acceptance.ps1".to_string());
    let final_acceptance_sequence = handoff_instructions
        .value("final_acceptance_sequence")
        .unwrap_or_else(|| {
            "operator-preflight,ai-acceptance,unity-acceptance,external-acceptance,strict-release-gate"
                .to_string()
        });
    let final_acceptance_requires = handoff_instructions
        .value("final_acceptance_requires")
        .unwrap_or_else(|| "ai_secret,unity_exe,data_root".to_string());
    let final_acceptance_report = handoff_instructions
        .value("final_acceptance_report")
        .unwrap_or_else(|| "dist/AutoDesignMaker-rust/final-acceptance-run.adm".to_string());
    let final_acceptance_package_refresh = handoff_instructions
        .value("final_acceptance_package_refresh")
        .unwrap_or_else(|| "after-successful-default-report-write".to_string());
    let suggested_final_acceptance_command = handoff_instructions
        .value("suggested_final_acceptance_command")
        .unwrap_or_else(|| {
            "powershell -ExecutionPolicy Bypass -File .\\scripts\\final_handoff_acceptance.ps1 -UnityExe '<path-to-Unity.exe>' -DataRoot '<data_root>'".to_string()
        });
    let suggested_final_acceptance_command =
        portable_handoff_command_data_root(suggested_final_acceptance_command, &command_data_roots);
    let suggested_final_acceptance_ai_invoke_command = handoff_instructions
        .value("suggested_final_acceptance_ai_invoke_command")
        .unwrap_or_else(|| {
            "powershell -ExecutionPolicy Bypass -File .\\scripts\\final_handoff_acceptance.ps1 -UnityExe '<path-to-Unity.exe>' -DataRoot '<data_root>' -RequireAiInvoke".to_string()
        });
    let suggested_final_acceptance_ai_invoke_command = portable_handoff_command_data_root(
        suggested_final_acceptance_ai_invoke_command,
        &command_data_roots,
    );
    let bundle_hash = bundle_manifest
        .value("bundle_hash")
        .unwrap_or_else(|| "none".to_string());
    let evidence_hash = evidence_manifest
        .value("evidence_hash")
        .unwrap_or_else(|| "none".to_string());

    format!(
        concat!(
            "AutoDesignMaker Rust Handoff Bundle\n",
            "\n",
            "entrypoint=AutoDesignMaker-rust/AutoDesignMaker-rust.exe\n",
            "source_entrypoint=source-bundle/README.md\n",
            "evidence_entrypoint=evidence/handoff-instructions.adm\n",
            "final_manifest=final-handoff-manifest.adm\n",
            "package_manifest=final-handoff-manifest.adm\n",
            "handoff_bundle_root_mode=package-inspection-and-evidence-entrypoint\n",
            "source_bundle_mode=source-audit-snapshot\n",
            "source_bundle_scripts_path=source-bundle/scripts\n",
            "handoff_bundle_dist_layout=top-level-delivery-artifacts\n",
            "handoff_bundle_contains_data_root=false\n",
            "strict_gate_original_data_root={}\n",
            "handoff_bundle_command_data_root_mode=portable-placeholder\n",
            "handoff_bundle_command_data_root_placeholder=<data_root>\n",
            "strict_gate_requires_matching_data_root=true\n",
            "strict_gate_requires_rehydrated_workspace_when_not_original=true\n",
            "strict_gate_rehydration_source_dir=source-bundle\n",
            "strict_gate_rehydration_dist_dirs=AutoDesignMaker-rust,game-build,sdk-bundle,unity-project\n",
            "handoff_rehydration_bundle_root_supported={}\n",
            "handoff_rehydration_script={}\n",
            "handoff_rehydration_destination_placeholder={}\n",
            "handoff_rehydration_manifest={}\n",
            "suggested_handoff_rehydration_command={}\n",
            "rehydrated_release_smoke_report={}\n",
            "rehydrated_release_smoke_command={}\n",
            "rehydrated_release_smoke_working_dir={}\n",
            "final_acceptance_working_dir={}\n",
            "final_acceptance_script={}\n",
            "final_acceptance_sequence={}\n",
            "final_acceptance_requires={}\n",
            "final_acceptance_report={}\n",
            "final_acceptance_package_refresh={}\n",
            "suggested_final_acceptance_command={}\n",
            "suggested_final_acceptance_ai_invoke_command={}\n",
            "strict_gate_rehydration_note=If running outside the original Rust workspace, copy source-bundle as the Rust workspace root, copy the listed bundle artifact directories into that workspace's dist directory, and provide the same DataRoot or an imported equivalent before rerunning acceptance gates.\n",
            "\n",
            "handoff_bundle_ready={}\n",
            "handoff_bundle_hash={}\n",
            "handoff_evidence_ready={}\n",
            "handoff_evidence_hash={}\n",
            "handoff_ready={}\n",
            "external_acceptance_ready={}\n",
            "ai_acceptance_ready={}\n",
            "final_package_ready={}\n",
            "final_delivery_ready={}\n",
            "final_handoff_ready={}\n",
            "external_dependency_count={}\n",
            "{}",
            "operator_input_count={}\n",
            "{}",
            "blocker_count={}\n",
            "{}",
            "blocker_resolution_count={}\n",
            "{}",
            "required_instruction_count={}\n",
            "required_blocked_instruction_count={}\n",
            "required_waiting_instruction_count={}\n",
            "optional_instruction_count={}\n",
            "manual_decision_instruction_count={}\n",
            "next_required_instruction={}\n",
            "next_required_instruction_status={}\n",
            "next_required_instruction_estimate={}\n",
            "next_required_instruction_command={}\n",
            "next_required_instruction_evidence={}\n",
            "next_required_instruction_done_when={}\n",
            "next_required_instruction_note={}\n",
            "remaining_required_execution_step_count={}\n",
            "{}",
            "instruction_count={}\n",
            "{}",
            "external_acceptance_data_root={}\n",
            "ai_acceptance_data_root={}\n",
            "unity_runtime_runner={}\n",
            "unity_selected={}\n",
            "unity_candidates={}\n",
            "unity_candidate_detail_count={}\n",
            "{}",
            "ai_provider_id={}\n",
            "ai_provider_model={}\n",
            "ai_diagnostic_readiness={}\n",
            "ai_configured_ready={}\n",
            "real_ai_provider_ready={}\n",
            "real_ai_provider_count={}\n",
            "ready_provider_count={}\n",
            "ai_provider_detail_count={}\n",
            "{}",
            "suggested_ai_provider_preset={}\n",
            "suggested_ai_secret_ref={}\n",
            "suggested_ai_secret_env_var={}\n",
            "suggested_ai_secret_requirement={}\n",
            "suggested_ai_secret_check_command={}\n",
            "suggested_ai_secret_session_set_command={}\n",
            "suggested_unity_archive_id={}\n",
            "suggested_unity_archive_source={}\n",
            "suggested_ai_acceptance_command={}\n",
            "suggested_ai_acceptance_invoke_command={}\n",
            "suggested_unity_acceptance_command={}\n",
            "suggested_external_acceptance_command={}\n",
            "suggested_strict_release_gate_command={}\n",
            "suggested_strict_release_gate_ai_invoke_command={}\n",
            "suggested_operator_preflight_command={}\n",
            "suggested_operator_preflight_require_ready_command={}\n",
            "operator_preflight_working_dir={}\n",
            "operator_preflight_bundle_root_supported={}\n",
            "operator_preflight_bundle_root_script={}\n",
            "operator_preflight_bundle_root_instructions_path={}\n",
            "suggested_operator_preflight_bundle_root_command={}\n",
            "suggested_operator_preflight_bundle_root_require_ready_command={}\n",
            "\n",
            "strict_gate_working_dir=rust-workspace-root-with-scripts-directory\n",
            "strict_gate_bundle_root_runnable=false\n",
            "strict_gate_context_note=Use a Rust workspace root with generated delivery artifacts, matching DataRoot, Unity editor, and real AI credentials; do not run the strict gate from the handoff bundle root.\n",
            "strict_gate_command={}\n",
            "strict_gate_ai_invoke_command={}\n",
            "strict_gate_requires_final_delivery={}\n",
            "strict_gate_final_manifest_requires=package_ready,handoff_ready,delivery_ready\n",
            "\n",
            "next_steps=Read evidence/handoff-instructions.adm, resolve required blocked instructions in a Rust workspace root, then rerun the strict release gate from that workspace root.\n",
            "delivery_note=package_ready means the bundle is assembled; delivery_ready requires external Unity PlayMode acceptance and real non-mock AI provider acceptance.\n"
        ),
        external_acceptance_data_root,
        handoff_rehydration_bundle_root_supported,
        handoff_rehydration_script,
        handoff_rehydration_destination_placeholder,
        handoff_rehydration_manifest,
        suggested_handoff_rehydration_command,
        rehydrated_release_smoke_report,
        rehydrated_release_smoke_command,
        rehydrated_release_smoke_working_dir,
        final_acceptance_working_dir,
        final_acceptance_script,
        final_acceptance_sequence,
        final_acceptance_requires,
        final_acceptance_report,
        final_acceptance_package_refresh,
        suggested_final_acceptance_command,
        suggested_final_acceptance_ai_invoke_command,
        bundle_manifest.bool_value("ready"),
        bundle_hash,
        evidence_manifest.bool_value("ready"),
        evidence_hash,
        evidence_manifest.bool_value("handoff_ready"),
        evidence_manifest.bool_value("external_acceptance_ready"),
        evidence_manifest.bool_value("ai_acceptance_ready"),
        final_package_ready,
        final_delivery_ready,
        final_handoff_ready,
        external_dependency_count,
        external_dependency_lines,
        operator_input_count,
        operator_input_lines,
        blocker_count,
        blocker_lines,
        blocker_resolution_count,
        blocker_resolution_lines,
        required_instruction_count,
        required_blocked_instruction_count,
        required_waiting_instruction_count,
        optional_instruction_count,
        manual_decision_instruction_count,
        next_required_instruction,
        next_required_instruction_status,
        next_required_instruction_estimate,
        next_required_instruction_command,
        next_required_instruction_evidence,
        next_required_instruction_done_when,
        next_required_instruction_note,
        remaining_required_execution_step_count,
        remaining_required_execution_step_lines,
        instruction_count,
        instruction_lines,
        external_acceptance_data_root,
        ai_acceptance_data_root,
        unity_runtime_runner,
        unity_selected,
        unity_candidates,
        unity_candidate_detail_count,
        unity_candidate_detail_lines,
        ai_provider_id,
        ai_provider_model,
        ai_diagnostic_readiness,
        ai_configured_ready,
        real_ai_provider_ready,
        real_ai_provider_count,
        ready_provider_count,
        ai_provider_detail_count,
        ai_provider_detail_lines,
        suggested_ai_provider_preset,
        suggested_ai_secret_ref,
        suggested_ai_secret_env_var,
        suggested_ai_secret_requirement,
        suggested_ai_secret_check_command,
        suggested_ai_secret_session_set_command,
        suggested_unity_archive_id,
        suggested_unity_archive_source,
        suggested_ai_acceptance_command,
        suggested_ai_acceptance_invoke_command,
        suggested_unity_acceptance_command,
        suggested_external_acceptance_command,
        suggested_strict_release_gate_command,
        suggested_strict_release_gate_ai_invoke_command,
        suggested_operator_preflight_command,
        suggested_operator_preflight_require_ready_command,
        operator_preflight_working_dir,
        operator_preflight_bundle_root_supported,
        operator_preflight_bundle_root_script,
        operator_preflight_bundle_root_instructions_path,
        suggested_operator_preflight_bundle_root_command,
        suggested_operator_preflight_bundle_root_require_ready_command,
        strict_gate_command,
        strict_gate_ai_invoke_command,
        strict_gate_requires_final_delivery
    )
}

fn handoff_bundle_sources() -> [HandoffBundleSource; 5] {
    [
        HandoffBundleSource {
            name: "AutoDesignMaker-rust",
            required: true,
        },
        HandoffBundleSource {
            name: "source-bundle",
            required: true,
        },
        HandoffBundleSource {
            name: "game-build",
            required: false,
        },
        HandoffBundleSource {
            name: "sdk-bundle",
            required: false,
        },
        HandoffBundleSource {
            name: "unity-project",
            required: false,
        },
    ]
}

fn handoff_bundle_excluded_files(source_name: &str) -> &'static [&'static str] {
    match source_name {
        "AutoDesignMaker-rust" => &[
            "external-acceptance.adm",
            "handoff-status.adm",
            "handoff-bundle-manifest.adm",
            "handoff-evidence-manifest.adm",
            "handoff-instructions.adm",
            "source-handoff-policy.adm",
            "final-handoff-manifest.adm",
        ],
        _ => &[],
    }
}

fn source_bundle_excluded_dirs() -> [&'static str; 4] {
    ["target", "dist", ".adm_rust_data", ".git"]
}

fn normalize_relative_path(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

fn single_line_cli_value(value: &str) -> String {
    value
        .replace('\r', " ")
        .replace('\n', " ")
        .trim()
        .to_string()
}

#[derive(Debug, Clone)]
struct ExternalAcceptanceCliArgs {
    require_ready: bool,
    require_ai_invoke: bool,
    unity_exe: Option<PathBuf>,
    release_dir: PathBuf,
    report_path: Option<PathBuf>,
    data_root: PathBuf,
}

fn parse_external_acceptance_cli_args(
    raw_args: Vec<String>,
) -> AdmResult<ExternalAcceptanceCliArgs> {
    let mut require_ready = false;
    let mut require_ai_invoke = false;
    let mut unity_exe = None;
    let mut positional = Vec::new();
    let mut index = 0;
    while index < raw_args.len() {
        let arg = &raw_args[index];
        if arg == "--require-ready" {
            require_ready = true;
        } else if arg == "--require-ai-invoke" {
            require_ai_invoke = true;
        } else if arg == "--unity-exe" {
            index += 1;
            let value = raw_args.get(index).ok_or_else(|| {
                adm_foundation::AdmError::invalid_input(
                    "external-acceptance --unity-exe requires a path",
                )
            })?;
            if value.trim().is_empty() {
                return Err(adm_foundation::AdmError::invalid_input(
                    "external-acceptance --unity-exe cannot be empty",
                ));
            }
            unity_exe = Some(PathBuf::from(value));
        } else if let Some(value) = arg.strip_prefix("--unity-exe=") {
            if value.trim().is_empty() {
                return Err(adm_foundation::AdmError::invalid_input(
                    "external-acceptance --unity-exe cannot be empty",
                ));
            }
            unity_exe = Some(PathBuf::from(value));
        } else if arg.starts_with('-') {
            return Err(adm_foundation::AdmError::invalid_input(format!(
                "unknown external-acceptance flag: {arg}"
            )));
        } else {
            positional.push(arg.clone());
        }
        index += 1;
    }
    if positional.len() > 3 {
        return Err(adm_foundation::AdmError::invalid_input(
            "external-acceptance accepts at most release_dir, report_path, and data_root",
        ));
    }

    Ok(ExternalAcceptanceCliArgs {
        require_ready,
        require_ai_invoke,
        unity_exe,
        release_dir: positional
            .first()
            .map(PathBuf::from)
            .unwrap_or_else(default_desktop_release_dir),
        report_path: positional.get(1).map(PathBuf::from),
        data_root: positional
            .get(2)
            .map(PathBuf::from)
            .unwrap_or_else(|| default_data_root(&std::env::current_dir().unwrap())),
    })
}

fn default_desktop_release_dir() -> PathBuf {
    std::env::current_dir()
        .unwrap_or_else(|_| PathBuf::from("."))
        .join("dist")
        .join("AutoDesignMaker-rust")
}

fn default_ai_acceptance_report_path() -> PathBuf {
    default_desktop_release_dir().join("ai-acceptance.adm")
}

fn default_handoff_status_report_path(release_dir: &Path) -> PathBuf {
    release_dir.join("handoff-status.adm")
}

fn default_source_bundle_dir() -> PathBuf {
    std::env::current_dir()
        .unwrap_or_else(|_| PathBuf::from("."))
        .join("dist")
        .join("source-bundle")
}

fn default_source_manifest_report_path() -> PathBuf {
    default_desktop_release_dir().join("source-manifest.adm")
}

fn default_dist_root() -> PathBuf {
    std::env::current_dir()
        .unwrap_or_else(|_| PathBuf::from("."))
        .join("dist")
}

fn default_handoff_bundle_dir() -> PathBuf {
    default_dist_root().join("handoff-bundle")
}

fn default_handoff_bundle_manifest_report_path() -> PathBuf {
    default_desktop_release_dir().join("handoff-bundle-manifest.adm")
}

fn default_handoff_evidence_manifest_report_path() -> PathBuf {
    default_desktop_release_dir().join("handoff-evidence-manifest.adm")
}

fn default_handoff_instructions_report_path() -> PathBuf {
    default_desktop_release_dir().join("handoff-instructions.adm")
}

fn default_source_handoff_policy_report_path() -> PathBuf {
    default_desktop_release_dir().join("source-handoff-policy.adm")
}

fn default_final_handoff_manifest_report_path() -> PathBuf {
    default_desktop_release_dir().join("final-handoff-manifest.adm")
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

fn default_unity_project_dir() -> PathBuf {
    std::env::current_dir()
        .unwrap_or_else(|_| PathBuf::from("."))
        .join("dist")
        .join("unity-project")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn unique_test_dir(name: &str) -> PathBuf {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!("adm-cli-{name}-{}-{nanos}", std::process::id()))
    }

    fn quote_command_placeholder_args(text: &str) -> String {
        text.replace(
            "-UnityExe <path-to-Unity.exe>",
            "-UnityExe '<path-to-Unity.exe>'",
        )
        .replace("-DataRoot <data_root>", "-DataRoot '<data_root>'")
        .replace(
            "-DestinationPath <path-to-rehydrated-rust-workspace>",
            "-DestinationPath '<path-to-rehydrated-rust-workspace>'",
        )
        .replace("-ProviderId <provider_id>", "-ProviderId '<provider_id>'")
        .replace("-Model <model>", "-Model '<model>'")
        .replace("-Preset <preset_id>", "-Preset '<preset_id>'")
        .replace("-ArchiveId <archive_id>", "-ArchiveId '<archive_id>'")
    }

    #[test]
    fn external_acceptance_cli_args_accept_explicit_unity_exe() {
        let parsed = parse_external_acceptance_cli_args(vec![
            "--unity-exe".to_string(),
            "C:/Unity/Editor/Unity.exe".to_string(),
            "--require-ready".to_string(),
            "--require-ai-invoke".to_string(),
            "dist/AutoDesignMaker-rust".to_string(),
            "dist/AutoDesignMaker-rust/custom-external.adm".to_string(),
            ".adm_rust_data".to_string(),
        ])
        .unwrap();

        assert!(parsed.require_ready);
        assert!(parsed.require_ai_invoke);
        assert_eq!(
            parsed.unity_exe,
            Some(PathBuf::from("C:/Unity/Editor/Unity.exe"))
        );
        assert_eq!(
            parsed.release_dir,
            PathBuf::from("dist/AutoDesignMaker-rust")
        );
        assert_eq!(
            parsed.report_path,
            Some(PathBuf::from(
                "dist/AutoDesignMaker-rust/custom-external.adm"
            ))
        );
        assert_eq!(parsed.data_root, PathBuf::from(".adm_rust_data"));
    }

    #[test]
    fn external_acceptance_cli_args_accept_equals_unity_exe() {
        let parsed = parse_external_acceptance_cli_args(vec![
            "--unity-exe=C:/Unity/Editor/Unity.exe".to_string(),
        ])
        .unwrap();

        assert_eq!(
            parsed.unity_exe,
            Some(PathBuf::from("C:/Unity/Editor/Unity.exe"))
        );
    }

    #[test]
    fn handoff_status_reports_ready_when_all_acceptance_reports_are_ready() {
        let dir = unique_test_dir("handoff-ready");
        fs::create_dir_all(&dir).unwrap();
        fs::write(
            dir.join("release-acceptance.adm"),
            "# Release Acceptance Report\naccepted=true\ndelivery_ready=true\nrelease_ready=true\nsmoke_ready=true\nrelease_hash=fnv64:ready\n",
        )
        .unwrap();
        fs::write(
            dir.join("external-acceptance.adm"),
            "# External Acceptance Doctor\nready=true\nunity_ready=true\nunity_runtime_present=true\nunity_runtime_ready=true\nunity_runtime_runner=unity_playmode\nreal_ai_provider_ready=true\n",
        )
        .unwrap();
        fs::write(
            dir.join("ai-acceptance.adm"),
            "# AI Provider Acceptance\nready=true\nprovider_id=openai_main\nconfigured_ready=true\ninvoke_attempted=true\ninvoke_succeeded=true\n",
        )
        .unwrap();
        fs::write(
            dir.join("source-manifest.adm"),
            "# Rust Source Bundle\nready=true\nsource_handoff_mode=bundled\nfile_count=3\nbundle_hash=fnv64:source\n",
        )
        .unwrap();
        fs::write(
            dir.join("handoff-bundle-manifest.adm"),
            "# Handoff Bundle\nready=true\nbundle_dir=dist/handoff-bundle\nfile_count=7\nbundle_hash=fnv64:bundle\n",
        )
        .unwrap();

        let report_path = dir.join("handoff-status.adm");
        let report = run_handoff_status(dir.clone(), report_path.clone(), true).unwrap();
        let rendered = fs::read_to_string(report_path).unwrap();
        assert!(report.ready());
        assert!(rendered.contains("ready=true"));
        assert!(rendered.contains("local_release_ready=true"));
        assert!(rendered.contains("external_acceptance_ready=true"));
        assert!(rendered.contains("external_acceptance_data_root=none"));
        assert!(rendered.contains("unity_runtime_present=true"));
        assert!(rendered.contains("unity_runtime_ready=true"));
        assert!(rendered.contains("unity_runtime_runner=unity_playmode"));
        assert!(rendered.contains("ai_acceptance_ready=true"));
        assert!(rendered.contains("ai_acceptance_data_root=none"));
        assert!(rendered.contains("ai_acceptance_data_root_matches_external=true"));
        assert!(rendered.contains("ai_acceptance_provider_matches_real_provider=true"));
        assert!(rendered.contains("external_acceptance_require_ai_invoke=false"));
        assert!(rendered.contains("source_ready=true"));
        assert!(rendered.contains("source_handoff_mode=bundled"));
        assert!(rendered.contains("handoff_bundle_ready=true"));
        assert!(rendered.contains("handoff_bundle_hash=fnv64:bundle"));
        assert!(rendered.contains("blocker_count=0"));
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn handoff_status_blocks_when_external_acceptance_requires_ai_invoke() {
        let dir = unique_test_dir("handoff-ai-invoke-required");
        fs::create_dir_all(&dir).unwrap();
        fs::write(
            dir.join("release-acceptance.adm"),
            "# Release Acceptance Report\naccepted=true\ndelivery_ready=true\nrelease_ready=true\nsmoke_ready=true\nrelease_hash=fnv64:local\n",
        )
        .unwrap();
        fs::write(
            dir.join("external-acceptance.adm"),
            "# External Acceptance Doctor\nready=true\nunity_ready=true\nunity_runtime_present=true\nunity_runtime_ready=true\nunity_runtime_runner=unity_playmode\nreal_ai_provider_ready=true\nrequire_ai_invoke=true\n",
        )
        .unwrap();
        fs::write(
            dir.join("ai-acceptance.adm"),
            "# AI Provider Acceptance\nready=true\nprovider_id=openai_main\nconfigured_ready=true\ninvoke_attempted=false\ninvoke_succeeded=false\n",
        )
        .unwrap();
        fs::write(
            dir.join("source-manifest.adm"),
            "# Rust Source Bundle\nready=true\nsource_handoff_mode=bundled\nfile_count=3\nbundle_hash=fnv64:source\n",
        )
        .unwrap();
        fs::write(
            dir.join("handoff-bundle-manifest.adm"),
            "# Handoff Bundle\nready=true\nbundle_dir=dist/handoff-bundle\nfile_count=7\nbundle_hash=fnv64:bundle\n",
        )
        .unwrap();

        let report_path = dir.join("handoff-status.adm");
        let report = run_handoff_status(dir.clone(), report_path.clone(), true).unwrap();
        let rendered = fs::read_to_string(report_path).unwrap();
        assert!(!report.ready());
        assert!(rendered.contains("external_acceptance_require_ai_invoke=true"));
        assert!(rendered.contains("ai_invoke_attempted=false"));
        assert!(rendered.contains("ai_invoke_succeeded=false"));
        assert!(rendered.contains("blocker=ai_acceptance_invoke_not_attempted"));
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn handoff_status_lists_remaining_external_and_ai_blockers() {
        let dir = unique_test_dir("handoff-blocked");
        fs::create_dir_all(&dir).unwrap();
        fs::write(
            dir.join("release-acceptance.adm"),
            "# Release Acceptance Report\naccepted=true\ndelivery_ready=true\nrelease_ready=true\nsmoke_ready=true\nrelease_hash=fnv64:local\n",
        )
        .unwrap();
        fs::write(
            dir.join("external-acceptance.adm"),
            "# External Acceptance Doctor\nready=false\nunity_ready=false\nunity_runtime_present=false\nunity_runtime_ready=false\nunity_runtime_runner=none\nreal_ai_provider_ready=false\n",
        )
        .unwrap();
        fs::write(
            dir.join("ai-acceptance.adm"),
            "# AI Provider Acceptance\nready=false\nprovider_id=openai_main\nconfigured_ready=false\ninvoke_attempted=false\ninvoke_succeeded=false\n",
        )
        .unwrap();
        fs::write(
            dir.join("source-manifest.adm"),
            "# Rust Source Bundle\nready=true\nsource_handoff_mode=bundled\nfile_count=3\nbundle_hash=fnv64:source\n",
        )
        .unwrap();
        fs::write(
            dir.join("handoff-bundle-manifest.adm"),
            "# Handoff Bundle\nready=true\nbundle_dir=dist/handoff-bundle\nfile_count=7\nbundle_hash=fnv64:bundle\n",
        )
        .unwrap();

        let report_path = dir.join("handoff-status.adm");
        let report = run_handoff_status(dir.clone(), report_path.clone(), false).unwrap();
        let rendered = fs::read_to_string(report_path).unwrap();
        assert!(report.local_release_ready());
        assert!(!report.ready());
        assert!(rendered.contains("blocker=external_acceptance_not_ready"));
        assert!(rendered.contains("blocker=unity_not_ready"));
        assert!(rendered.contains("blocker=unity_runtime_report_missing"));
        assert!(rendered.contains("blocker=real_ai_provider_not_ready"));
        assert!(rendered.contains("blocker=ai_provider_acceptance_not_ready"));
        assert!(rendered.contains("blocker=ai_provider_not_configured"));
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn handoff_status_blocks_mismatched_ai_acceptance_data_root() {
        let dir = unique_test_dir("handoff-data-root-mismatch");
        fs::create_dir_all(&dir).unwrap();
        fs::write(
            dir.join("release-acceptance.adm"),
            "# Release Acceptance Report\naccepted=true\ndelivery_ready=true\nrelease_ready=true\nsmoke_ready=true\nrelease_hash=fnv64:local\n",
        )
        .unwrap();
        fs::write(
            dir.join("external-acceptance.adm"),
            "# External Acceptance Doctor\nready=true\ndata_root=data-root-a\nunity_ready=true\nunity_runtime_present=true\nunity_runtime_ready=true\nunity_runtime_runner=unity_playmode\nreal_ai_provider_ready=true\n",
        )
        .unwrap();
        fs::write(
            dir.join("ai-acceptance.adm"),
            "# AI Provider Acceptance\nready=true\ndata_root=data-root-b\nprovider_id=openai_main\nconfigured_ready=true\ninvoke_attempted=true\ninvoke_succeeded=true\n",
        )
        .unwrap();
        fs::write(
            dir.join("source-manifest.adm"),
            "# Rust Source Bundle\nready=true\nsource_handoff_mode=bundled\nfile_count=3\nbundle_hash=fnv64:source\n",
        )
        .unwrap();
        fs::write(
            dir.join("handoff-bundle-manifest.adm"),
            "# Handoff Bundle\nready=true\nbundle_dir=dist/handoff-bundle\nfile_count=7\nbundle_hash=fnv64:bundle\n",
        )
        .unwrap();

        let report_path = dir.join("handoff-status.adm");
        let report = run_handoff_status(dir.clone(), report_path.clone(), true).unwrap();
        let rendered = fs::read_to_string(report_path).unwrap();
        assert!(!report.ready());
        assert!(rendered.contains("external_acceptance_data_root=data-root-a"));
        assert!(rendered.contains("ai_acceptance_data_root=data-root-b"));
        assert!(rendered.contains("ai_acceptance_data_root_matches_external=false"));
        assert!(rendered.contains("blocker=ai_acceptance_data_root_mismatch"));
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn handoff_status_blocks_ai_acceptance_provider_that_is_not_real_ready_provider() {
        let dir = unique_test_dir("handoff-ai-provider-mismatch");
        fs::create_dir_all(&dir).unwrap();
        fs::write(
            dir.join("release-acceptance.adm"),
            "# Release Acceptance Report\naccepted=true\ndelivery_ready=true\nrelease_ready=true\nsmoke_ready=true\nrelease_hash=fnv64:local\n",
        )
        .unwrap();
        fs::write(
            dir.join("external-acceptance.adm"),
            "# External Acceptance Doctor\nready=false\nunity_ready=true\nunity_runtime_present=true\nunity_runtime_ready=true\nunity_runtime_runner=unity_playmode\nreal_ai_provider_ready=true\nai_acceptance_provider_matches_real_provider=false\n",
        )
        .unwrap();
        fs::write(
            dir.join("ai-acceptance.adm"),
            "# AI Provider Acceptance\nready=true\nprovider_id=openai_main\nconfigured_ready=true\ninvoke_attempted=true\ninvoke_succeeded=true\n",
        )
        .unwrap();
        fs::write(
            dir.join("source-manifest.adm"),
            "# Rust Source Bundle\nready=true\nsource_handoff_mode=bundled\nfile_count=3\nbundle_hash=fnv64:source\n",
        )
        .unwrap();
        fs::write(
            dir.join("handoff-bundle-manifest.adm"),
            "# Handoff Bundle\nready=true\nbundle_dir=dist/handoff-bundle\nfile_count=7\nbundle_hash=fnv64:bundle\n",
        )
        .unwrap();

        let report_path = dir.join("handoff-status.adm");
        let report = run_handoff_status(dir.clone(), report_path.clone(), true).unwrap();
        let rendered = fs::read_to_string(report_path).unwrap();
        assert!(!report.ready());
        assert!(rendered.contains("ai_acceptance_provider_matches_real_provider=false"));
        assert!(rendered.contains("blocker=ai_acceptance_provider_not_real_provider"));
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn stage_handoff_bundle_copies_required_and_optional_dirs_and_cleans_stale_files() {
        let dir = unique_test_dir("handoff-bundle");
        let dist_root = dir.join("dist");
        let release_dir = dist_root.join("AutoDesignMaker-rust");
        let source_bundle_dir = dist_root.join("source-bundle");
        let game_build_dir = dist_root.join("game-build");
        let bundle_dir = dist_root.join("handoff-bundle");
        let report_path = release_dir.join("handoff-bundle-manifest.adm");
        fs::create_dir_all(&release_dir).unwrap();
        fs::create_dir_all(source_bundle_dir.join("apps")).unwrap();
        fs::create_dir_all(&game_build_dir).unwrap();
        fs::create_dir_all(bundle_dir.join("stale")).unwrap();
        fs::write(release_dir.join("AutoDesignMaker-rust.exe"), "exe").unwrap();
        fs::write(
            release_dir.join("release-acceptance.adm"),
            concat!(
                "accepted=true\n",
                "smoke_executable=C:\\work\\adm\\RUST\\dist\\AutoDesignMaker-rust\\AutoDesignMaker-rust.exe\n",
                "smoke_command=C:\\work\\adm\\RUST\\dist\\AutoDesignMaker-rust\\AutoDesignMaker-rust.exe --smoke\n",
            ),
        )
        .unwrap();
        fs::write(
            release_dir.join("final-acceptance-run.adm"),
            concat!(
                "dry_run=true\n",
                "rust_root=C:\\work\\adm\\RUST\n",
                "instructions_path=C:\\work\\adm\\RUST\\dist\\AutoDesignMaker-rust\\handoff-instructions.adm\n",
                "data_root=.adm_rust_data\n",
                "step=2; name=AI acceptance; command=powershell -NoProfile -ExecutionPolicy Bypass -File C:\\work\\adm\\RUST\\scripts\\ai_acceptance_gate.ps1 -InstructionsPath C:\\work\\adm\\RUST\\dist\\AutoDesignMaker-rust\\handoff-instructions.adm -DataRoot .adm_rust_data -RequireReady\n",
            ),
        )
        .unwrap();
        fs::write(release_dir.join("external-acceptance.adm"), "ready=stale\n").unwrap();
        fs::write(release_dir.join("handoff-status.adm"), "ready=stale\n").unwrap();
        fs::write(
            release_dir.join("handoff-bundle-manifest.adm"),
            "ready=stale\n",
        )
        .unwrap();
        fs::write(
            release_dir.join("handoff-evidence-manifest.adm"),
            "ready=stale\n",
        )
        .unwrap();
        fs::write(
            release_dir.join("handoff-instructions.adm"),
            "ready=stale\n",
        )
        .unwrap();
        fs::write(
            release_dir.join("source-handoff-policy.adm"),
            "ready=stale\n",
        )
        .unwrap();
        fs::write(
            release_dir.join("final-handoff-manifest.adm"),
            "ready=stale\n",
        )
        .unwrap();
        fs::write(source_bundle_dir.join("Cargo.toml"), "[workspace]\n").unwrap();
        fs::write(
            source_bundle_dir.join("apps").join("main.rs"),
            "fn main() {}\n",
        )
        .unwrap();
        fs::write(game_build_dir.join("prototype.txt"), "game").unwrap();
        fs::write(bundle_dir.join("stale").join("old.txt"), "old").unwrap();

        let report =
            stage_handoff_bundle(dist_root.clone(), bundle_dir.clone(), report_path.clone())
                .unwrap();
        let rendered = fs::read_to_string(&report_path).unwrap();
        assert!(report.ready());
        assert_eq!(report.file_count, 6);
        assert!(
            bundle_dir
                .join("AutoDesignMaker-rust")
                .join("AutoDesignMaker-rust.exe")
                .exists()
        );
        let bundled_release_acceptance = fs::read_to_string(
            bundle_dir
                .join("AutoDesignMaker-rust")
                .join("release-acceptance.adm"),
        )
        .unwrap();
        assert!(
            bundled_release_acceptance
                .contains("smoke_executable=.\\AutoDesignMaker-rust\\AutoDesignMaker-rust.exe")
        );
        assert!(
            bundled_release_acceptance.contains(
                "smoke_command=.\\AutoDesignMaker-rust\\AutoDesignMaker-rust.exe --smoke"
            )
        );
        assert!(
            bundled_release_acceptance
                .contains("handoff_bundle_smoke_command_mode=portable-package-root-relative")
        );
        assert!(
            bundled_release_acceptance
                .contains("handoff_bundle_smoke_command_working_dir=handoff-bundle-root")
        );
        assert!(bundled_release_acceptance.contains(
            "handoff_bundle_smoke_executable_placeholder=.\\AutoDesignMaker-rust\\AutoDesignMaker-rust.exe"
        ));
        assert!(
            !bundled_release_acceptance.contains(
                "C:\\work\\adm\\RUST\\dist\\AutoDesignMaker-rust\\AutoDesignMaker-rust.exe"
            )
        );
        let bundled_final_acceptance = fs::read_to_string(
            bundle_dir
                .join("AutoDesignMaker-rust")
                .join("final-acceptance-run.adm"),
        )
        .unwrap();
        assert!(bundled_final_acceptance.contains("data_root=.adm_rust_data"));
        assert!(bundled_final_acceptance.contains("rust_root=C:\\work\\adm\\RUST"));
        assert!(bundled_final_acceptance.contains(
            "instructions_path=C:\\work\\adm\\RUST\\dist\\AutoDesignMaker-rust\\handoff-instructions.adm"
        ));
        assert!(bundled_final_acceptance.contains(
            "command=powershell -NoProfile -ExecutionPolicy Bypass -File .\\scripts\\ai_acceptance_gate.ps1 -InstructionsPath .\\dist\\handoff-bundle\\evidence\\handoff-instructions.adm -DataRoot '<data_root>' -RequireReady"
        ));
        assert!(
            bundled_final_acceptance
                .contains("handoff_bundle_command_data_root_mode=portable-placeholder")
        );
        assert!(
            bundled_final_acceptance
                .contains("handoff_bundle_command_data_root_placeholder=<data_root>")
        );
        assert!(
            bundled_final_acceptance
                .contains("handoff_bundle_command_script_path_mode=portable-workspace-relative")
        );
        assert!(
            bundled_final_acceptance
                .contains("handoff_bundle_command_script_path_placeholder=.\\scripts")
        );
        assert!(
            bundled_final_acceptance.contains(
                "handoff_bundle_command_instructions_path_mode=portable-workspace-relative"
            )
        );
        assert!(bundled_final_acceptance.contains(
            "handoff_bundle_command_instructions_path_placeholder=.\\dist\\handoff-bundle\\evidence\\handoff-instructions.adm"
        ));
        assert!(!bundled_final_acceptance.contains("-DataRoot .adm_rust_data"));
        assert!(!bundled_final_acceptance.contains("-File C:\\work\\adm\\RUST\\scripts"));
        assert!(!bundled_final_acceptance.contains("-InstructionsPath C:\\work\\adm\\RUST\\dist"));
        assert!(bundle_dir.join("source-bundle").join("Cargo.toml").exists());
        assert!(bundle_dir.join("game-build").join("prototype.txt").exists());
        assert!(
            !bundle_dir
                .join("AutoDesignMaker-rust")
                .join("external-acceptance.adm")
                .exists()
        );
        assert!(
            !bundle_dir
                .join("AutoDesignMaker-rust")
                .join("handoff-status.adm")
                .exists()
        );
        assert!(
            !bundle_dir
                .join("AutoDesignMaker-rust")
                .join("handoff-bundle-manifest.adm")
                .exists()
        );
        assert!(
            !bundle_dir
                .join("AutoDesignMaker-rust")
                .join("handoff-evidence-manifest.adm")
                .exists()
        );
        assert!(
            !bundle_dir
                .join("AutoDesignMaker-rust")
                .join("handoff-instructions.adm")
                .exists()
        );
        assert!(
            !bundle_dir
                .join("AutoDesignMaker-rust")
                .join("source-handoff-policy.adm")
                .exists()
        );
        assert!(
            !bundle_dir
                .join("AutoDesignMaker-rust")
                .join("final-handoff-manifest.adm")
                .exists()
        );
        assert!(!bundle_dir.join("stale").join("old.txt").exists());
        assert!(bundle_dir.join("handoff-bundle-manifest.adm").exists());
        assert!(rendered.contains("ready=true"));
        assert!(rendered.contains("file_count=6"));
        assert!(rendered.contains("bundle_hash=fnv64:"));
        assert!(rendered.contains("stale_cleanup=removed_existing_bundle_dir"));
        assert!(rendered.contains("excluded_file=AutoDesignMaker-rust/external-acceptance.adm"));
        assert!(rendered.contains("excluded_file=AutoDesignMaker-rust/handoff-status.adm"));
        assert!(
            rendered.contains("excluded_file=AutoDesignMaker-rust/handoff-bundle-manifest.adm")
        );
        assert!(
            rendered.contains("excluded_file=AutoDesignMaker-rust/handoff-evidence-manifest.adm")
        );
        assert!(rendered.contains("excluded_file=AutoDesignMaker-rust/handoff-instructions.adm"));
        assert!(rendered.contains("excluded_file=AutoDesignMaker-rust/source-handoff-policy.adm"));
        assert!(rendered.contains("excluded_file=AutoDesignMaker-rust/final-handoff-manifest.adm"));
        assert!(
            rendered
                .contains("required_dir=AutoDesignMaker-rust; present=true; copied=true; files=3;")
        );
        assert!(
            rendered.contains("required_dir=source-bundle; present=true; copied=true; files=2;")
        );
        assert!(rendered.contains("optional_dir=game-build; present=true; copied=true; files=1;"));
        assert!(
            rendered.contains("optional_dir=sdk-bundle; present=false; copied=false; files=0;")
        );
        assert!(
            rendered.contains("optional_dir=unity-project; present=false; copied=false; files=0;")
        );
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn stage_handoff_bundle_rejects_bundle_dir_that_contains_dist_root() {
        let dir = unique_test_dir("handoff-bundle-guard");
        let dist_root = dir.join("dist");
        fs::create_dir_all(dist_root.join("AutoDesignMaker-rust")).unwrap();
        fs::create_dir_all(dist_root.join("source-bundle")).unwrap();
        fs::write(
            dist_root.join("AutoDesignMaker-rust").join("app.exe"),
            "exe",
        )
        .unwrap();
        fs::write(
            dist_root.join("source-bundle").join("Cargo.toml"),
            "[workspace]\n",
        )
        .unwrap();
        let result = stage_handoff_bundle(
            dist_root,
            dir.clone(),
            dir.join("release").join("handoff-bundle-manifest.adm"),
        );
        assert!(result.is_err());
        let error = result.unwrap_err().to_string();
        assert!(
            error.contains("handoff bundle dir must not be the dist root or a parent"),
            "{error}"
        );
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn write_source_handoff_policy_reports_bundled_source_ready() {
        let dir = unique_test_dir("source-handoff-policy");
        let release_dir = dir.join("release");
        let bundle_dir = dir.join("bundle");
        let report_path = release_dir.join("source-handoff-policy.adm");
        fs::create_dir_all(&release_dir).unwrap();
        fs::create_dir_all(bundle_dir.join("source-bundle")).unwrap();
        fs::write(
            release_dir.join("source-manifest.adm"),
            "ready=true\nsource_handoff_mode=bundled\nfile_count=55\nbundle_hash=fnv64:source\n",
        )
        .unwrap();
        fs::write(
            release_dir.join("handoff-bundle-manifest.adm"),
            "ready=true\nrequired_dir=source-bundle; present=true; copied=true; files=55; bytes=123; hash=fnv64:source\n",
        )
        .unwrap();

        let report =
            write_source_handoff_policy(release_dir, bundle_dir, report_path.clone()).unwrap();
        let rendered = fs::read_to_string(&report_path).unwrap();
        assert!(report.ready());
        assert!(rendered.contains("ready=true"));
        assert!(rendered.contains("source_handoff_mode=bundled"));
        assert!(rendered.contains("source_bundle_hash=fnv64:source"));
        assert!(
            rendered.contains(
                "source_handoff_policy=bundled-source-bundle-is-current-delivery-evidence"
            )
        );
        assert!(rendered.contains("parent_repo_commit_required_for_package_ready=false"));
        assert!(rendered.contains("source_bundle_dir_present=true"));
        assert!(rendered.contains("source_bundle_copied=true"));
        assert!(rendered.contains("source_bundle_hash_matches=true"));
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn write_handoff_instructions_lists_external_blockers_and_next_steps() {
        let dir = unique_test_dir("handoff-instructions");
        let release_dir = dir.join("release");
        let data_root = dir.join("data-root");
        let data_root_text = data_root.display().to_string();
        let old_archive_id = "archive_1000_1_1";
        let latest_archive_id = "archive_2000_1_1";
        let report_path = release_dir.join("handoff-instructions.adm");
        fs::create_dir_all(&release_dir).unwrap();
        fs::create_dir_all(data_root.join("archives").join(old_archive_id)).unwrap();
        fs::create_dir_all(data_root.join("archives").join(latest_archive_id)).unwrap();
        fs::write(
            data_root
                .join("archives")
                .join(old_archive_id)
                .join("manifest.adm"),
            format!("archive_id={old_archive_id}\n"),
        )
        .unwrap();
        fs::write(
            data_root
                .join("archives")
                .join(latest_archive_id)
                .join("manifest.adm"),
            format!("archive_id={latest_archive_id}\n"),
        )
        .unwrap();
        fs::write(
            release_dir.join("handoff-status.adm"),
            concat!(
                "ready=false\n",
                "blocker_count=6\n",
                "blocker=external_acceptance_not_ready\n",
                "blocker=unity_not_ready\n",
                "blocker=unity_runtime_report_missing\n",
                "blocker=real_ai_provider_not_ready\n",
                "blocker=ai_provider_acceptance_not_ready\n",
                "blocker=ai_provider_not_configured\n",
            ),
        )
        .unwrap();
        fs::write(
            release_dir.join("external-acceptance.adm"),
            format!("ready=false\ndata_root={data_root_text}\nunity_ready=false\nunity_runtime_present=false\nunity_runtime_ready=false\nunity_runtime_runner=none\nunity_candidates=2\nreal_ai_provider_ready=false\nrequire_ai_invoke=true\n\n## Unity Doctor Output\n# Unity Editor Discovery\nselected=none\ncandidates=2\n- source=env; path=C:/Unity/Editor/Unity.exe; present=false; looks_like_unity_editor=true; ready=false\n- source=default; path=C:/Program Files/Unity/Editor/Unity.exe; present=false; looks_like_unity_editor=true; ready=false\n"),
        )
        .unwrap();
        fs::write(
            release_dir.join("ai-acceptance.adm"),
            format!("ready=false\ndata_root={data_root_text}\nprovider_id=openai_main\nmodel=gpt-4.1\nconfigured_ready=false\ninvoke_attempted=false\ninvoke_succeeded=false\n\n## AI Diagnostics\n# AI Diagnostics\nready_provider_count=1\nmock\tReady\tcapabilities=text_generation\tprovider does not require a secret\n"),
        )
        .unwrap();
        fs::write(
            release_dir.join("source-handoff-policy.adm"),
            "ready=true\n",
        )
        .unwrap();
        fs::write(
            release_dir.join("final-handoff-manifest.adm"),
            "ready=true\npackage_ready=true\ndelivery_ready=false\nhandoff_ready=false\n",
        )
        .unwrap();

        let report = write_handoff_instructions(release_dir, report_path.clone()).unwrap();
        let rendered = fs::read_to_string(&report_path).unwrap();
        assert!(report.ready());
        assert!(!report.handoff_ready);
        assert!(report.source_policy_ready);
        assert!(report.final_package_present);
        assert!(report.final_package_ready);
        assert!(!report.final_delivery_ready);
        assert_eq!(report.blockers.len(), 6);
        assert_eq!(report.instructions.len(), 8);
        assert!(rendered.contains("ready=true"));
        assert!(rendered.contains("required_instruction_count=6"));
        assert!(rendered.contains("required_blocked_instruction_count=5"));
        assert!(rendered.contains("required_waiting_instruction_count=1"));
        assert!(rendered.contains("optional_instruction_count=2"));
        assert!(rendered.contains("manual_decision_instruction_count=0"));
        assert!(rendered.contains("next_required_instruction=configure-real-ai-provider"));
        assert!(rendered.contains("next_required_instruction_status=blocked"));
        assert!(
            rendered.contains("next_required_instruction_estimate=0.5-1h-if-credentials-are-ready")
        );
        assert!(rendered.contains(&format!(
            "next_required_instruction_command=powershell -ExecutionPolicy Bypass -File .\\scripts\\ai_acceptance_gate.ps1 -ProviderId openai_main -Model gpt-4.1 -Preset openai -SecretRef default -DataRoot {data_root_text} -RequireReady"
        )));
        assert!(rendered.contains(
            "next_required_instruction_evidence=dist/AutoDesignMaker-rust/ai-acceptance.adm"
        ));
        assert!(
            rendered.contains(
                "next_required_instruction_done_when=ready=true-and-configured_ready=true"
            )
        );
        assert!(rendered.contains(
            "next_required_instruction_note=configures-non-mock-provider-then-writes-redacted-acceptance-report"
        ));
        assert!(rendered.contains("remaining_required_execution_step_count=6"));
        assert!(rendered.contains(&format!(
            "remaining_required_execution_step=1; instruction=configure-real-ai-provider; status=blocked; estimate=0.5-1h-if-credentials-are-ready; command=powershell -ExecutionPolicy Bypass -File .\\scripts\\ai_acceptance_gate.ps1 -ProviderId openai_main -Model gpt-4.1 -Preset openai -SecretRef default -DataRoot {data_root_text} -RequireReady; evidence=dist/AutoDesignMaker-rust/ai-acceptance.adm; done_when=ready=true-and-configured_ready=true; note=configures-non-mock-provider-then-writes-redacted-acceptance-report"
        )));
        assert!(rendered.contains(
            "remaining_required_execution_step=6; instruction=confirm-final-delivery-package; status=waiting-for-strict-gate; estimate=0.1h-after-strict-release-gate; command=cargo run -q -p adm-cli -- finalize-handoff-package; evidence=dist/AutoDesignMaker-rust/final-handoff-manifest.adm; done_when=final-handoff-manifest-delivery_ready=true; note=requires-final-handoff-manifest-delivery_ready-true-before-full-completion"
        ));
        assert!(rendered.contains("source_policy_ready=true"));
        assert!(rendered.contains("final_handoff_manifest_present=true"));
        assert!(rendered.contains("final_package_ready=true"));
        assert!(rendered.contains("final_delivery_ready=false"));
        assert!(rendered.contains("final_handoff_ready=false"));
        assert!(rendered.contains("ai_invoke_attempted=false"));
        assert!(rendered.contains("ai_invoke_succeeded=false"));
        assert!(rendered.contains("external_acceptance_require_ai_invoke=true"));
        assert!(rendered.contains("unity_candidate_detail_count=2"));
        assert!(rendered.contains(
            "unity_candidate=source=env; path=C:/Unity/Editor/Unity.exe; present=false; looks_like_unity_editor=true; ready=false"
        ));
        assert!(rendered.contains(
            "unity_candidate=source=default; path=C:/Program Files/Unity/Editor/Unity.exe; present=false; looks_like_unity_editor=true; ready=false"
        ));
        assert!(rendered.contains("ai_provider_detail_count=1"));
        assert!(rendered.contains(
            "ai_provider=provider_id=mock; readiness=Ready; capabilities=text_generation; note=provider does not require a secret"
        ));
        assert!(rendered.contains("blocker=unity_not_ready"));
        assert!(rendered.contains("blocker=unity_runtime_report_missing"));
        assert!(rendered.contains("blocker_resolution_count=6"));
        assert!(rendered.contains(&format!(
            "blocker_resolution=unity_runtime_report_missing; action=run-unity-acceptance-with-real-editor; command=powershell -ExecutionPolicy Bypass -File .\\scripts\\unity_acceptance_gate.ps1 -ArchiveId {latest_archive_id} -UnityExe '<path-to-Unity.exe>' -DataRoot {data_root_text}; evidence=dist/unity-project/Assets/AutoDesignMaker/Generated/runtime_execution_results.adm; done_when=runtime_execution_results.adm-has-ready=true-and-runner=unity_playmode"
        )));
        assert!(rendered.contains(&format!(
            "blocker_resolution=ai_provider_not_configured; action=configure-and-run-real-ai-provider-acceptance; command=powershell -ExecutionPolicy Bypass -File .\\scripts\\ai_acceptance_gate.ps1 -ProviderId openai_main -Model gpt-4.1 -Preset openai -SecretRef default -DataRoot {data_root_text} -RequireReady; evidence=dist/AutoDesignMaker-rust/ai-acceptance.adm; done_when=ready=true-and-configured_ready=true"
        )));
        assert!(rendered.contains(&format!(
            "blocker_resolution=external_acceptance_not_ready; action=rerun-external-acceptance-after-ai-and-unity-are-ready; command=powershell -ExecutionPolicy Bypass -File .\\scripts\\external_acceptance_doctor.ps1 -UnityExe '<path-to-Unity.exe>' -DataRoot {data_root_text} -RequireReady; evidence=dist/AutoDesignMaker-rust/external-acceptance.adm; done_when=ready=true"
        )));
        assert!(rendered.contains(&format!(
            "instruction=configure-real-ai-provider; required=true; status=blocked; estimate=0.5-1h-if-credentials-are-ready; command=powershell -ExecutionPolicy Bypass -File .\\scripts\\ai_acceptance_gate.ps1 -ProviderId openai_main -Model gpt-4.1 -Preset openai -SecretRef default -DataRoot {data_root_text} -RequireReady;"
        )));
        assert!(rendered.contains(&format!(
            "instruction=run-ai-provider-invoke-acceptance; required=true; status=blocked; estimate=0.25-1h-if-credentials-are-ready; command=powershell -ExecutionPolicy Bypass -File .\\scripts\\ai_acceptance_gate.ps1 -ProviderId openai_main -Model gpt-4.1 -Preset openai -SecretRef default -DataRoot {data_root_text} -Invoke -RequireReady -RequireInvoke;"
        )));
        assert!(rendered.contains(&format!(
            "instruction=run-unity-acceptance; required=true; status=blocked; estimate=1-3h-if-unity-is-installed; command=powershell -ExecutionPolicy Bypass -File .\\scripts\\unity_acceptance_gate.ps1 -ArchiveId {latest_archive_id} -UnityExe '<path-to-Unity.exe>' -DataRoot {data_root_text};"
        )));
        assert!(
            rendered.contains(
                "instruction=decide-source-handoff-policy; required=false; status=ready;"
            )
        );
        assert!(rendered.contains(&format!(
            "strict_gate_command=powershell -ExecutionPolicy Bypass -File .\\scripts\\release_gate.ps1 -RequireExternalAcceptance -UnityExe '<path-to-Unity.exe>' -DataRoot {data_root_text}"
        )));
        assert!(rendered.contains(&format!(
            "strict_gate_ai_invoke_command=powershell -ExecutionPolicy Bypass -File .\\scripts\\release_gate.ps1 -RequireExternalAcceptance -RequireAiInvoke -UnityExe '<path-to-Unity.exe>' -DataRoot {data_root_text}"
        )));
        assert!(rendered.contains("strict_gate_requires_final_delivery=true"));
        assert!(rendered.contains(
            "strict_gate_final_manifest_requires=package_ready,handoff_ready,delivery_ready"
        ));
        assert!(rendered.contains("suggested_ai_provider_preset=openai"));
        assert!(rendered.contains("suggested_ai_secret_ref=default"));
        assert!(rendered.contains("suggested_ai_secret_env_var=OPENAI_API_KEY"));
        assert!(rendered.contains("suggested_ai_secret_requirement=env:OPENAI_API_KEY"));
        assert!(rendered.contains(
            "suggested_ai_secret_check_command=powershell -NoProfile -Command \"[bool]`$env:OPENAI_API_KEY\""
        ));
        assert!(
            rendered
                .contains("suggested_ai_secret_session_set_command=$env:OPENAI_API_KEY='<secret>'")
        );
        assert!(rendered.contains(&format!("suggested_unity_archive_id={latest_archive_id}")));
        assert!(rendered.contains("suggested_unity_archive_source=data_root_latest_archive"));
        assert!(rendered.contains(&format!(
            "suggested_ai_acceptance_command=powershell -ExecutionPolicy Bypass -File .\\scripts\\ai_acceptance_gate.ps1 -ProviderId openai_main -Model gpt-4.1 -Preset openai -SecretRef default -DataRoot {data_root_text} -RequireReady"
        )));
        assert!(rendered.contains(&format!(
            "suggested_ai_acceptance_invoke_command=powershell -ExecutionPolicy Bypass -File .\\scripts\\ai_acceptance_gate.ps1 -ProviderId openai_main -Model gpt-4.1 -Preset openai -SecretRef default -DataRoot {data_root_text} -Invoke -RequireReady -RequireInvoke"
        )));
        assert!(rendered.contains(&format!(
            "suggested_unity_acceptance_command=powershell -ExecutionPolicy Bypass -File .\\scripts\\unity_acceptance_gate.ps1 -ArchiveId {latest_archive_id} -UnityExe '<path-to-Unity.exe>' -DataRoot {data_root_text}"
        )));
        assert!(rendered.contains(&format!(
            "suggested_external_acceptance_command=powershell -ExecutionPolicy Bypass -File .\\scripts\\external_acceptance_doctor.ps1 -UnityExe '<path-to-Unity.exe>' -DataRoot {data_root_text} -RequireReady"
        )));
        assert!(rendered.contains(&format!(
            "suggested_strict_release_gate_command=powershell -ExecutionPolicy Bypass -File .\\scripts\\release_gate.ps1 -RequireExternalAcceptance -UnityExe '<path-to-Unity.exe>' -DataRoot {data_root_text}"
        )));
        assert!(rendered.contains(&format!(
            "suggested_strict_release_gate_ai_invoke_command=powershell -ExecutionPolicy Bypass -File .\\scripts\\release_gate.ps1 -RequireExternalAcceptance -RequireAiInvoke -UnityExe '<path-to-Unity.exe>' -DataRoot {data_root_text}"
        )));
        assert!(rendered.contains(&format!(
            "suggested_operator_preflight_command=powershell -ExecutionPolicy Bypass -File .\\scripts\\handoff_operator_preflight.ps1 -DataRoot {data_root_text}"
        )));
        assert!(rendered.contains(&format!(
            "suggested_operator_preflight_require_ready_command=powershell -ExecutionPolicy Bypass -File .\\scripts\\handoff_operator_preflight.ps1 -UnityExe '<path-to-Unity.exe>' -DataRoot {data_root_text} -RequireReady"
        )));
        assert!(
            rendered.contains(
                "operator_preflight_working_dir=rust-workspace-root-with-scripts-directory"
            )
        );
        assert!(rendered.contains("operator_preflight_bundle_root_supported=true"));
        assert!(rendered.contains(
            "operator_preflight_bundle_root_script=source-bundle/scripts/handoff_operator_preflight.ps1"
        ));
        assert!(rendered.contains(
            "operator_preflight_bundle_root_instructions_path=..\\evidence\\handoff-instructions.adm"
        ));
        assert!(rendered.contains(&format!(
            "suggested_operator_preflight_bundle_root_command=powershell -ExecutionPolicy Bypass -File .\\source-bundle\\scripts\\handoff_operator_preflight.ps1 -InstructionsPath ..\\evidence\\handoff-instructions.adm -DataRoot {data_root_text}"
        )));
        assert!(rendered.contains(&format!(
            "suggested_operator_preflight_bundle_root_require_ready_command=powershell -ExecutionPolicy Bypass -File .\\source-bundle\\scripts\\handoff_operator_preflight.ps1 -InstructionsPath ..\\evidence\\handoff-instructions.adm -UnityExe '<path-to-Unity.exe>' -DataRoot {data_root_text} -RequireReady"
        )));
        assert!(rendered.contains("handoff_rehydration_bundle_root_supported=true"));
        assert!(rendered.contains(
            "handoff_rehydration_script=source-bundle/scripts/rehydrate_handoff_workspace.ps1"
        ));
        assert!(rendered.contains(
            "handoff_rehydration_destination_placeholder=<path-to-rehydrated-rust-workspace>"
        ));
        assert!(
            rendered.contains("handoff_rehydration_manifest=dist/handoff-rehydration-manifest.adm")
        );
        assert!(rendered.contains(
            "suggested_handoff_rehydration_command=powershell -ExecutionPolicy Bypass -File .\\source-bundle\\scripts\\rehydrate_handoff_workspace.ps1 -DestinationPath '<path-to-rehydrated-rust-workspace>'"
        ));
        assert!(rendered.contains(
            "rehydrated_release_smoke_report=dist/AutoDesignMaker-rust/release-acceptance.adm"
        ));
        assert!(rendered.contains(
            "rehydrated_release_smoke_command=.\\dist\\AutoDesignMaker-rust\\AutoDesignMaker-rust.exe --smoke"
        ));
        assert!(
            rendered
                .contains("rehydrated_release_smoke_working_dir=rehydrated-rust-workspace-root")
        );
        assert!(rendered.contains(
            "final_acceptance_working_dir=rust-workspace-root-after-rehydration-or-original"
        ));
        assert!(rendered.contains("final_acceptance_script=scripts/final_handoff_acceptance.ps1"));
        assert!(rendered.contains(
            "final_acceptance_sequence=operator-preflight,ai-acceptance,unity-acceptance,external-acceptance,strict-release-gate"
        ));
        assert!(rendered.contains("final_acceptance_requires=ai_secret,unity_exe,data_root"));
        assert!(rendered.contains(
            "final_acceptance_report=dist/AutoDesignMaker-rust/final-acceptance-run.adm"
        ));
        assert!(
            rendered
                .contains("final_acceptance_package_refresh=after-successful-default-report-write")
        );
        assert!(rendered.contains(&format!(
            "suggested_final_acceptance_command=powershell -ExecutionPolicy Bypass -File .\\scripts\\final_handoff_acceptance.ps1 -UnityExe '<path-to-Unity.exe>' -DataRoot {data_root_text}"
        )));
        assert!(rendered.contains(&format!(
            "suggested_final_acceptance_ai_invoke_command=powershell -ExecutionPolicy Bypass -File .\\scripts\\final_handoff_acceptance.ps1 -UnityExe '<path-to-Unity.exe>' -DataRoot {data_root_text} -RequireAiInvoke"
        )));
        assert!(rendered.contains("external_dependency_count=3"));
        assert!(rendered.contains(&format!(
            "external_dependency=real_ai_provider; status=missing_secret_or_provider_config; requirement=env:OPENAI_API_KEY; command=powershell -ExecutionPolicy Bypass -File .\\scripts\\ai_acceptance_gate.ps1 -ProviderId openai_main -Model gpt-4.1 -Preset openai -SecretRef default -DataRoot {data_root_text} -RequireReady; evidence=dist/AutoDesignMaker-rust/ai-acceptance.adm; unlocks=configure-real-ai-provider"
        )));
        assert!(rendered.contains(&format!(
            "external_dependency=real_ai_provider_invoke; status=invoke_not_attempted; requirement=real-provider-network-invoke; command=powershell -ExecutionPolicy Bypass -File .\\scripts\\ai_acceptance_gate.ps1 -ProviderId openai_main -Model gpt-4.1 -Preset openai -SecretRef default -DataRoot {data_root_text} -Invoke -RequireReady -RequireInvoke; evidence=dist/AutoDesignMaker-rust/ai-acceptance.adm; unlocks=run-ai-provider-invoke-acceptance"
        )));
        assert!(rendered.contains(&format!(
            "external_dependency=unity_playmode; status=unity_not_ready; requirement=compatible-unity-editor-path; command=powershell -ExecutionPolicy Bypass -File .\\scripts\\unity_acceptance_gate.ps1 -ArchiveId {latest_archive_id} -UnityExe '<path-to-Unity.exe>' -DataRoot {data_root_text}; evidence=dist/unity-project/Assets/AutoDesignMaker/Generated/runtime_execution_results.adm; unlocks=run-unity-acceptance"
        )));
        assert!(rendered.contains("operator_input_count=2"));
        assert!(rendered.contains(
            "operator_input=ai_secret; status=missing_secret_or_provider_config; placeholder=<secret>; requirement=env:OPENAI_API_KEY; check_command=powershell -NoProfile -Command \"[bool]`$env:OPENAI_API_KEY\"; set_command=$env:OPENAI_API_KEY='<secret>'; required_for=configure-real-ai-provider; note=provide-redacted-secret-in-receiving-shell-before-running-ai-acceptance"
        ));
        assert!(rendered.contains(
            "operator_input=unity_exe; status=missing_or_not_ready; placeholder=<path-to-Unity.exe>; requirement=compatible-unity-editor-path; check_command=powershell -NoProfile -Command \"Test-Path -LiteralPath '<path-to-Unity.exe>'\"; set_command=replace-placeholder-in-unity-commands; required_for=run-unity-acceptance,rerun-external-acceptance,run-strict-release-gate; note=use-compatible-unity-editor-that-can-run-playmode-validation"
        ));
        assert!(rendered.contains(&format!(
            "instruction=rerun-external-acceptance; required=true; status=blocked; estimate=0.5h-after-ai-and-unity-are-ready; command=powershell -ExecutionPolicy Bypass -File .\\scripts\\external_acceptance_doctor.ps1 -UnityExe '<path-to-Unity.exe>' -DataRoot {data_root_text} -RequireReady;"
        )));
        assert!(rendered.contains(&format!(
            "instruction=run-strict-release-gate; required=true; status=blocked; estimate=0.5-1h-after-external-acceptance-is-ready; command=powershell -ExecutionPolicy Bypass -File .\\scripts\\release_gate.ps1 -RequireExternalAcceptance -UnityExe '<path-to-Unity.exe>' -DataRoot {data_root_text};"
        )));
        assert!(rendered.contains(
            "note=final-gate-must-report-handoff-ready-true-and-final-manifest-package-handoff-delivery-ready"
        ));
        assert!(rendered.contains(
            "instruction=confirm-final-delivery-package; required=true; status=waiting-for-strict-gate;"
        ));
        assert!(rendered.contains(
            "note=requires-final-handoff-manifest-delivery_ready-true-before-full-completion"
        ));
        assert!(rendered.contains(
            "instruction=explain-package-vs-delivery-readiness; required=false; status=informational;"
        ));
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn write_handoff_instructions_allows_missing_final_manifest_before_finalize() {
        let dir = unique_test_dir("handoff-instructions-no-final");
        let release_dir = dir.join("release");
        let report_path = release_dir.join("handoff-instructions.adm");
        fs::create_dir_all(&release_dir).unwrap();
        fs::write(release_dir.join("handoff-status.adm"), "ready=true\n").unwrap();
        fs::write(
            release_dir.join("external-acceptance.adm"),
            "ready=true\nunity_ready=true\nunity_runtime_ready=true\nunity_runtime_runner=unity_playmode\nreal_ai_provider_ready=true\nrequire_ai_invoke=false\n",
        )
        .unwrap();
        fs::write(
            release_dir.join("ai-acceptance.adm"),
            "ready=true\nconfigured_ready=true\ninvoke_attempted=false\ninvoke_succeeded=false\n",
        )
        .unwrap();
        fs::write(
            release_dir.join("source-handoff-policy.adm"),
            "ready=true\n",
        )
        .unwrap();

        let report = write_handoff_instructions(release_dir, report_path.clone()).unwrap();
        let rendered = fs::read_to_string(&report_path).unwrap();
        assert!(report.ready());
        assert!(!report.final_package_present);
        assert!(!report.final_delivery_ready);
        assert!(rendered.contains("final_handoff_manifest_present=false"));
        assert!(rendered.contains("remaining_required_execution_step_count=1"));
        assert!(rendered.contains(
            "remaining_required_execution_step=1; instruction=confirm-final-delivery-package; status=blocked;"
        ));
        assert!(rendered.contains(
            "instruction=confirm-final-delivery-package; required=true; status=blocked;"
        ));
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn sync_handoff_evidence_copies_current_reports_and_cleans_stale_files() {
        let dir = unique_test_dir("handoff-evidence");
        let release_dir = dir.join("release");
        let bundle_dir = dir.join("bundle");
        let evidence_dir = bundle_dir.join("evidence");
        let report_path = release_dir.join("handoff-evidence-manifest.adm");
        fs::create_dir_all(&release_dir).unwrap();
        fs::create_dir_all(&evidence_dir).unwrap();
        fs::write(
            release_dir.join("release-acceptance.adm"),
            concat!(
                "accepted=true\n",
                "release_hash=fnv64:release\n",
                "smoke_executable=C:\\work\\adm\\RUST\\dist\\AutoDesignMaker-rust\\AutoDesignMaker-rust.exe\n",
                "smoke_command=C:\\work\\adm\\RUST\\dist\\AutoDesignMaker-rust\\AutoDesignMaker-rust.exe --smoke\n",
            ),
        )
        .unwrap();
        fs::write(
            release_dir.join("source-manifest.adm"),
            "ready=true\nbundle_hash=fnv64:source\n",
        )
        .unwrap();
        fs::write(
            release_dir.join("handoff-bundle-manifest.adm"),
            "ready=true\nbundle_hash=fnv64:bundle\n",
        )
        .unwrap();
        fs::write(
            release_dir.join("external-acceptance.adm"),
            "ready=false\nunity_ready=false\n",
        )
        .unwrap();
        fs::write(
            release_dir.join("ai-acceptance.adm"),
            "ready=false\nconfigured_ready=false\n",
        )
        .unwrap();
        fs::write(
            release_dir.join("handoff-status.adm"),
            "ready=false\nhandoff_bundle_ready=true\n",
        )
        .unwrap();
        fs::write(
            release_dir.join("source-handoff-policy.adm"),
            "ready=true\nsource_handoff_policy=bundled-source-bundle-is-current-delivery-evidence\n",
        )
        .unwrap();
        fs::write(
            release_dir.join("handoff-instructions.adm"),
            "ready=true\nexternal_acceptance_data_root=.adm_rust_data\nai_acceptance_data_root=.adm_rust_data\ninstruction_count=5\nsuggested_ai_acceptance_command=powershell -ExecutionPolicy Bypass -File .\\scripts\\ai_acceptance_gate.ps1 -ProviderId openai_main -Model gpt-4.1 -Preset openai -SecretRef default -DataRoot .adm_rust_data -RequireReady\nremaining_required_execution_step=1; instruction=configure-real-ai-provider; command=powershell -ExecutionPolicy Bypass -File .\\scripts\\ai_acceptance_gate.ps1 -ProviderId openai_main -Model gpt-4.1 -Preset openai -SecretRef default -DataRoot .adm_rust_data -RequireReady\n",
        )
        .unwrap();
        fs::write(evidence_dir.join("stale.adm"), "old").unwrap();

        let report =
            sync_handoff_evidence(release_dir.clone(), bundle_dir.clone(), report_path.clone())
                .unwrap();
        let rendered = fs::read_to_string(&report_path).unwrap();
        assert!(report.ready());
        assert!(!report.handoff_ready);
        assert!(!report.external_acceptance_ready);
        assert!(!report.ai_acceptance_ready);
        assert_eq!(report.files.len(), 8);
        assert_eq!(report.files.iter().filter(|file| file.copied).count(), 8);
        assert!(!evidence_dir.join("stale.adm").exists());
        assert!(evidence_dir.join("release-acceptance.adm").exists());
        assert!(evidence_dir.join("source-manifest.adm").exists());
        assert!(evidence_dir.join("handoff-bundle-manifest.adm").exists());
        assert!(evidence_dir.join("external-acceptance.adm").exists());
        assert!(evidence_dir.join("ai-acceptance.adm").exists());
        assert!(evidence_dir.join("handoff-status.adm").exists());
        assert!(evidence_dir.join("source-handoff-policy.adm").exists());
        assert!(evidence_dir.join("handoff-instructions.adm").exists());
        assert!(evidence_dir.join("handoff-evidence-manifest.adm").exists());
        let bundled_release_acceptance =
            fs::read_to_string(evidence_dir.join("release-acceptance.adm")).unwrap();
        assert!(
            bundled_release_acceptance
                .contains("smoke_executable=.\\AutoDesignMaker-rust\\AutoDesignMaker-rust.exe")
        );
        assert!(
            bundled_release_acceptance.contains(
                "smoke_command=.\\AutoDesignMaker-rust\\AutoDesignMaker-rust.exe --smoke"
            )
        );
        assert!(
            bundled_release_acceptance
                .contains("handoff_bundle_smoke_command_mode=portable-package-root-relative")
        );
        assert!(
            bundled_release_acceptance
                .contains("handoff_bundle_smoke_command_working_dir=handoff-bundle-root")
        );
        assert!(
            !bundled_release_acceptance.contains(
                "C:\\work\\adm\\RUST\\dist\\AutoDesignMaker-rust\\AutoDesignMaker-rust.exe"
            )
        );
        let bundled_instructions =
            fs::read_to_string(evidence_dir.join("handoff-instructions.adm")).unwrap();
        assert!(bundled_instructions.contains("external_acceptance_data_root=.adm_rust_data"));
        assert!(bundled_instructions.contains("ai_acceptance_data_root=.adm_rust_data"));
        assert!(
            bundled_instructions
                .contains("handoff_bundle_command_data_root_mode=portable-placeholder")
        );
        assert!(
            bundled_instructions
                .contains("handoff_bundle_command_data_root_placeholder=<data_root>")
        );
        assert!(bundled_instructions.contains(
            "suggested_ai_acceptance_command=powershell -ExecutionPolicy Bypass -File .\\scripts\\ai_acceptance_gate.ps1 -ProviderId openai_main -Model gpt-4.1 -Preset openai -SecretRef default -DataRoot '<data_root>' -RequireReady"
        ));
        assert!(bundled_instructions.contains(
            "remaining_required_execution_step=1; instruction=configure-real-ai-provider; command=powershell -ExecutionPolicy Bypass -File .\\scripts\\ai_acceptance_gate.ps1 -ProviderId openai_main -Model gpt-4.1 -Preset openai -SecretRef default -DataRoot '<data_root>' -RequireReady"
        ));
        assert!(!bundled_instructions.contains("-DataRoot .adm_rust_data"));
        assert!(rendered.contains("ready=true"));
        assert!(rendered.contains("handoff_ready=false"));
        assert!(rendered.contains("external_acceptance_ready=false"));
        assert!(rendered.contains("ai_acceptance_ready=false"));
        assert!(rendered.contains("stale_cleanup=removed_existing_evidence_dir"));
        assert!(rendered.contains("evidence_hash=fnv64:"));
        assert!(rendered.contains(
            "evidence_file=handoff-status.adm; required=true; present=true; copied=true;"
        ));
        assert!(rendered.contains(
            "evidence_file=source-handoff-policy.adm; required=true; present=true; copied=true;"
        ));
        assert!(rendered.contains(
            "evidence_file=handoff-instructions.adm; required=true; present=true; copied=true;"
        ));
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn finalize_handoff_package_hashes_complete_bundle_and_excludes_stale_self_manifest() {
        let dir = unique_test_dir("final-handoff");
        let bundle_dir = dir.join("bundle");
        let report_path = dir.join("release").join("final-handoff-manifest.adm");
        fs::create_dir_all(bundle_dir.join("AutoDesignMaker-rust")).unwrap();
        fs::create_dir_all(bundle_dir.join("source-bundle")).unwrap();
        fs::create_dir_all(bundle_dir.join("evidence")).unwrap();
        fs::write(
            bundle_dir.join("AutoDesignMaker-rust").join("app.exe"),
            "exe",
        )
        .unwrap();
        fs::write(
            bundle_dir.join("source-bundle").join("Cargo.toml"),
            "[workspace]\n",
        )
        .unwrap();
        fs::write(
            bundle_dir.join("handoff-bundle-manifest.adm"),
            "# Handoff Bundle\nready=true\nbundle_hash=fnv64:bundle\n",
        )
        .unwrap();
        fs::write(
            bundle_dir
                .join("evidence")
                .join("handoff-evidence-manifest.adm"),
            "# Handoff Evidence\nready=true\nhandoff_ready=false\nexternal_acceptance_ready=false\nai_acceptance_ready=false\nevidence_hash=fnv64:evidence\n",
        )
        .unwrap();
        fs::write(
            bundle_dir.join("evidence").join("handoff-instructions.adm"),
            quote_command_placeholder_args("ready=true\nfinal_package_ready=true\nfinal_delivery_ready=false\nfinal_handoff_ready=false\nunity_runtime_runner=cli_smoke_runner\nreal_ai_provider_ready=false\nai_configured_ready=false\nexternal_acceptance_data_root=.adm_rust_data\nai_acceptance_data_root=.adm_rust_data\nai_provider_id=openai_main\nai_provider_model=gpt-4.1\nai_diagnostic_readiness=MissingProvider\nunity_selected=none\nunity_candidates=2\nunity_candidate_detail_count=2\nunity_candidate=source=env; path=C:/Unity/Editor/Unity.exe; present=false; looks_like_unity_editor=true; ready=false\nunity_candidate=source=default; path=C:/Program Files/Unity/Editor/Unity.exe; present=false; looks_like_unity_editor=true; ready=false\nreal_ai_provider_count=0\nready_provider_count=1\nai_provider_detail_count=1\nai_provider=provider_id=mock; readiness=Ready; capabilities=text_generation; note=provider does not require a secret\nsuggested_ai_provider_preset=openai\nsuggested_ai_secret_ref=default\nsuggested_ai_secret_env_var=OPENAI_API_KEY\nsuggested_ai_secret_requirement=env:OPENAI_API_KEY\nsuggested_ai_secret_check_command=powershell -NoProfile -Command \"[bool]`$env:OPENAI_API_KEY\"\nsuggested_ai_secret_session_set_command=$env:OPENAI_API_KEY='<secret>'\nsuggested_unity_archive_id=archive_999\nsuggested_unity_archive_source=data_root_latest_archive\nsuggested_ai_acceptance_command=powershell -ExecutionPolicy Bypass -File .\\scripts\\ai_acceptance_gate.ps1 -ProviderId openai_main -Model gpt-4.1 -Preset openai -SecretRef default -DataRoot .adm_rust_data -RequireReady\nsuggested_ai_acceptance_invoke_command=powershell -ExecutionPolicy Bypass -File .\\scripts\\ai_acceptance_gate.ps1 -ProviderId openai_main -Model gpt-4.1 -Preset openai -SecretRef default -DataRoot .adm_rust_data -Invoke -RequireReady -RequireInvoke\nsuggested_unity_acceptance_command=powershell -ExecutionPolicy Bypass -File .\\scripts\\unity_acceptance_gate.ps1 -ArchiveId archive_999 -UnityExe <path-to-Unity.exe> -DataRoot .adm_rust_data\nsuggested_external_acceptance_command=powershell -ExecutionPolicy Bypass -File .\\scripts\\external_acceptance_doctor.ps1 -UnityExe <path-to-Unity.exe> -DataRoot .adm_rust_data -RequireReady\nsuggested_strict_release_gate_command=powershell -ExecutionPolicy Bypass -File .\\scripts\\release_gate.ps1 -RequireExternalAcceptance -UnityExe <path-to-Unity.exe> -DataRoot .adm_rust_data\nsuggested_strict_release_gate_ai_invoke_command=powershell -ExecutionPolicy Bypass -File .\\scripts\\release_gate.ps1 -RequireExternalAcceptance -RequireAiInvoke -UnityExe <path-to-Unity.exe> -DataRoot .adm_rust_data\nsuggested_operator_preflight_command=powershell -ExecutionPolicy Bypass -File .\\scripts\\handoff_operator_preflight.ps1 -DataRoot .adm_rust_data\nsuggested_operator_preflight_require_ready_command=powershell -ExecutionPolicy Bypass -File .\\scripts\\handoff_operator_preflight.ps1 -UnityExe <path-to-Unity.exe> -DataRoot .adm_rust_data -RequireReady\noperator_input_count=2\noperator_input=ai_secret; status=missing_secret_or_provider_config; placeholder=<secret>; requirement=env:OPENAI_API_KEY; check_command=powershell -NoProfile -Command \"[bool]`$env:OPENAI_API_KEY\"; set_command=$env:OPENAI_API_KEY='<secret>'; required_for=configure-real-ai-provider; note=provide-redacted-secret-in-receiving-shell-before-running-ai-acceptance\noperator_input=unity_exe; status=missing_or_not_ready; placeholder=<path-to-Unity.exe>; requirement=compatible-unity-editor-path; check_command=powershell -NoProfile -Command \"Test-Path -LiteralPath '<path-to-Unity.exe>'\"; set_command=replace-placeholder-in-unity-commands; required_for=run-unity-acceptance,rerun-external-acceptance,run-strict-release-gate; note=use-compatible-unity-editor-that-can-run-playmode-validation\nblocker_count=2\nblocker=unity_not_ready\nblocker=ai_provider_not_configured\nblocker_resolution_count=2\nblocker_resolution=unity_not_ready; action=run-unity-acceptance-with-real-editor; command=powershell -ExecutionPolicy Bypass -File .\\scripts\\unity_acceptance_gate.ps1 -ArchiveId archive_999 -UnityExe <path-to-Unity.exe> -DataRoot .adm_rust_data; evidence=dist/unity-project/Assets/AutoDesignMaker/Generated/runtime_execution_results.adm; done_when=runtime_execution_results.adm-has-ready=true-and-runner=unity_playmode\nblocker_resolution=ai_provider_not_configured; action=configure-and-run-real-ai-provider-acceptance; command=powershell -ExecutionPolicy Bypass -File .\\scripts\\ai_acceptance_gate.ps1 -ProviderId openai_main -Model gpt-4.1 -Preset openai -SecretRef default -DataRoot .adm_rust_data -RequireReady; evidence=dist/AutoDesignMaker-rust/ai-acceptance.adm; done_when=ready=true-and-configured_ready=true\nrequired_instruction_count=5\nrequired_blocked_instruction_count=4\nrequired_waiting_instruction_count=1\noptional_instruction_count=3\nmanual_decision_instruction_count=1\nnext_required_instruction=configure-real-ai-provider\nnext_required_instruction_status=blocked\nnext_required_instruction_estimate=0.5-1h-if-credentials-are-ready\nnext_required_instruction_command=powershell -ExecutionPolicy Bypass -File .\\scripts\\ai_acceptance_gate.ps1 -ProviderId openai_main -Model gpt-4.1 -Preset openai -SecretRef default -DataRoot .adm_rust_data -RequireReady\nnext_required_instruction_evidence=dist/AutoDesignMaker-rust/ai-acceptance.adm\nnext_required_instruction_done_when=ready=true-and-configured_ready=true\nnext_required_instruction_note=configures-non-mock-provider-then-writes-redacted-acceptance-report\ninstruction_count=8\ninstruction=configure-real-ai-provider; required=true; status=blocked; estimate=0.5-1h-if-credentials-are-ready; command=powershell -ExecutionPolicy Bypass -File .\\scripts\\ai_acceptance_gate.ps1 -ProviderId openai_main -Model gpt-4.1 -Preset openai -SecretRef default -DataRoot .adm_rust_data -RequireReady; evidence=dist/AutoDesignMaker-rust/ai-acceptance.adm; note=configures-non-mock-provider-then-writes-redacted-acceptance-report\ninstruction=run-unity-acceptance; required=true; status=blocked; estimate=1-3h-if-unity-is-installed; command=powershell -ExecutionPolicy Bypass -File .\\scripts\\unity_acceptance_gate.ps1 -ArchiveId archive_999 -UnityExe <path-to-Unity.exe> -DataRoot .adm_rust_data; evidence=dist/unity-project/Assets/AutoDesignMaker/Generated/runtime_execution_results.adm; note=requires-compatible-unity-editor-and-unity_playmode-runtime-results\nstrict_gate_command=powershell -ExecutionPolicy Bypass -File .\\scripts\\release_gate.ps1 -RequireExternalAcceptance -UnityExe <path-to-Unity.exe> -DataRoot .adm_rust_data\nstrict_gate_ai_invoke_command=powershell -ExecutionPolicy Bypass -File .\\scripts\\release_gate.ps1 -RequireExternalAcceptance -RequireAiInvoke -UnityExe <path-to-Unity.exe> -DataRoot .adm_rust_data\nstrict_gate_requires_final_delivery=true\n"),
        )
        .unwrap();
        let handoff_instructions_path =
            bundle_dir.join("evidence").join("handoff-instructions.adm");
        let mut handoff_instructions_text = fs::read_to_string(&handoff_instructions_path).unwrap();
        handoff_instructions_text.push_str(&quote_command_placeholder_args(
            "remaining_required_execution_step_count=5\n\
remaining_required_execution_step=1; instruction=configure-real-ai-provider; status=blocked; estimate=0.5-1h-if-credentials-are-ready; command=powershell -ExecutionPolicy Bypass -File .\\scripts\\ai_acceptance_gate.ps1 -ProviderId openai_main -Model gpt-4.1 -Preset openai -SecretRef default -DataRoot .adm_rust_data -RequireReady; evidence=dist/AutoDesignMaker-rust/ai-acceptance.adm; done_when=ready=true-and-configured_ready=true; note=configures-non-mock-provider-then-writes-redacted-acceptance-report\n\
remaining_required_execution_step=2; instruction=run-unity-acceptance; status=blocked; estimate=1-3h-if-unity-is-installed; command=powershell -ExecutionPolicy Bypass -File .\\scripts\\unity_acceptance_gate.ps1 -ArchiveId archive_999 -UnityExe <path-to-Unity.exe> -DataRoot .adm_rust_data; evidence=dist/unity-project/Assets/AutoDesignMaker/Generated/runtime_execution_results.adm; done_when=runtime_execution_results.adm-has-ready=true-and-runner=unity_playmode; note=requires-compatible-unity-editor-and-unity_playmode-runtime-results\n\
remaining_required_execution_step=3; instruction=rerun-external-acceptance; status=blocked; estimate=0.5h-after-ai-and-unity-are-ready; command=powershell -ExecutionPolicy Bypass -File .\\scripts\\external_acceptance_doctor.ps1 -UnityExe <path-to-Unity.exe> -DataRoot .adm_rust_data -RequireReady; evidence=dist/AutoDesignMaker-rust/external-acceptance.adm; done_when=ready=true; note=requires-unity-ready-and-real-ai-provider-ready\n\
remaining_required_execution_step=4; instruction=run-strict-release-gate; status=blocked; estimate=0.5-1h-after-external-acceptance-is-ready; command=powershell -ExecutionPolicy Bypass -File .\\scripts\\release_gate.ps1 -RequireExternalAcceptance -UnityExe <path-to-Unity.exe> -DataRoot .adm_rust_data; evidence=dist/AutoDesignMaker-rust/handoff-status.adm; done_when=handoff-status-ready=true-and-final-manifest-package-handoff-delivery-ready; note=final-gate-must-report-handoff-ready-true-and-final-manifest-package-handoff-delivery-ready\n\
remaining_required_execution_step=5; instruction=confirm-final-delivery-package; status=waiting-for-strict-gate; estimate=0.1h-after-strict-release-gate; command=cargo run -q -p adm-cli -- finalize-handoff-package; evidence=dist/AutoDesignMaker-rust/final-handoff-manifest.adm; done_when=final-handoff-manifest-delivery_ready=true; note=requires-final-handoff-manifest-delivery_ready-true-before-full-completion\n",
        ));
        fs::write(&handoff_instructions_path, handoff_instructions_text).unwrap();
        fs::write(
            bundle_dir.join("final-handoff-manifest.adm"),
            "stale-final-manifest",
        )
        .unwrap();

        let report = finalize_handoff_package(bundle_dir.clone(), report_path.clone()).unwrap();
        let rendered = fs::read_to_string(&report_path).unwrap();
        let handoff_readme = fs::read_to_string(bundle_dir.join("HANDOFF_README.txt")).unwrap();
        assert!(report.ready());
        assert_eq!(report.files.len(), 6);
        assert!(
            report
                .files
                .iter()
                .all(|file| file.relative_path != "final-handoff-manifest.adm")
        );
        assert!(bundle_dir.join("final-handoff-manifest.adm").exists());
        assert!(bundle_dir.join("HANDOFF_README.txt").exists());
        assert!(rendered.contains("ready=true"));
        assert!(rendered.contains("package_ready=true"));
        assert!(rendered.contains("delivery_ready=false"));
        assert!(rendered.contains("handoff_bundle_ready=true"));
        assert!(rendered.contains("handoff_evidence_ready=true"));
        assert!(rendered.contains("handoff_ready=false"));
        assert!(rendered.contains("external_acceptance_ready=false"));
        assert!(rendered.contains("ai_acceptance_ready=false"));
        assert!(rendered.contains("required_dir=AutoDesignMaker-rust; present=true"));
        assert!(rendered.contains("required_dir=source-bundle; present=true"));
        assert!(rendered.contains("required_dir=evidence; present=true"));
        assert!(rendered.contains("required_file=HANDOFF_README.txt; present=true"));
        assert!(rendered.contains("excluded_file=final-handoff-manifest.adm"));
        assert!(rendered.contains("package_hash=fnv64:"));
        assert!(rendered.contains("- path=HANDOFF_README.txt;"));
        assert!(rendered.contains("- path=evidence/handoff-evidence-manifest.adm;"));
        assert!(
            handoff_readme.contains("entrypoint=AutoDesignMaker-rust/AutoDesignMaker-rust.exe")
        );
        assert!(handoff_readme.contains("evidence_entrypoint=evidence/handoff-instructions.adm"));
        assert!(
            handoff_readme
                .contains("handoff_bundle_root_mode=package-inspection-and-evidence-entrypoint")
        );
        assert!(handoff_readme.contains("source_bundle_scripts_path=source-bundle/scripts"));
        assert!(handoff_readme.contains("handoff_bundle_dist_layout=top-level-delivery-artifacts"));
        assert!(handoff_readme.contains("handoff_bundle_contains_data_root=false"));
        assert!(handoff_readme.contains("strict_gate_original_data_root=.adm_rust_data"));
        assert!(
            handoff_readme.contains("handoff_bundle_command_data_root_mode=portable-placeholder")
        );
        assert!(
            handoff_readme.contains("handoff_bundle_command_data_root_placeholder=<data_root>")
        );
        assert!(handoff_readme.contains("strict_gate_requires_matching_data_root=true"));
        assert!(
            handoff_readme
                .contains("strict_gate_requires_rehydrated_workspace_when_not_original=true")
        );
        assert!(handoff_readme.contains("strict_gate_rehydration_source_dir=source-bundle"));
        assert!(handoff_readme.contains(
            "strict_gate_rehydration_dist_dirs=AutoDesignMaker-rust,game-build,sdk-bundle,unity-project"
        ));
        assert!(handoff_readme.contains(
            "copy source-bundle as the Rust workspace root, copy the listed bundle artifact directories into that workspace's dist directory"
        ));
        assert!(
            handoff_readme
                .contains("strict_gate_working_dir=rust-workspace-root-with-scripts-directory")
        );
        assert!(handoff_readme.contains("strict_gate_bundle_root_runnable=false"));
        assert!(handoff_readme.contains("do not run the strict gate from the handoff bundle root"));
        assert!(handoff_readme.contains("final_delivery_ready=false"));
        assert!(handoff_readme.contains("blocker_count=2"));
        assert!(handoff_readme.contains("blocker=unity_not_ready"));
        assert!(handoff_readme.contains("blocker=ai_provider_not_configured"));
        assert!(handoff_readme.contains("blocker_resolution_count=2"));
        assert!(handoff_readme.contains(
            "blocker_resolution=unity_not_ready; action=run-unity-acceptance-with-real-editor; command=powershell -ExecutionPolicy Bypass -File .\\scripts\\unity_acceptance_gate.ps1 -ArchiveId archive_999 -UnityExe '<path-to-Unity.exe>' -DataRoot '<data_root>'; evidence=dist/unity-project/Assets/AutoDesignMaker/Generated/runtime_execution_results.adm; done_when=runtime_execution_results.adm-has-ready=true-and-runner=unity_playmode"
        ));
        assert!(handoff_readme.contains(
            "blocker_resolution=ai_provider_not_configured; action=configure-and-run-real-ai-provider-acceptance; command=powershell -ExecutionPolicy Bypass -File .\\scripts\\ai_acceptance_gate.ps1 -ProviderId openai_main -Model gpt-4.1 -Preset openai -SecretRef default -DataRoot '<data_root>' -RequireReady; evidence=dist/AutoDesignMaker-rust/ai-acceptance.adm; done_when=ready=true-and-configured_ready=true"
        ));
        assert!(handoff_readme.contains("instruction_count=8"));
        assert!(handoff_readme.contains("required_instruction_count=5"));
        assert!(handoff_readme.contains("required_blocked_instruction_count=4"));
        assert!(handoff_readme.contains("required_waiting_instruction_count=1"));
        assert!(handoff_readme.contains("optional_instruction_count=3"));
        assert!(handoff_readme.contains("manual_decision_instruction_count=1"));
        assert!(handoff_readme.contains("next_required_instruction=configure-real-ai-provider"));
        assert!(handoff_readme.contains("next_required_instruction_status=blocked"));
        assert!(
            handoff_readme
                .contains("next_required_instruction_estimate=0.5-1h-if-credentials-are-ready")
        );
        assert!(handoff_readme.contains(
            "next_required_instruction_command=powershell -ExecutionPolicy Bypass -File .\\scripts\\ai_acceptance_gate.ps1 -ProviderId openai_main -Model gpt-4.1 -Preset openai -SecretRef default -DataRoot '<data_root>' -RequireReady"
        ));
        assert!(handoff_readme.contains(
            "next_required_instruction_evidence=dist/AutoDesignMaker-rust/ai-acceptance.adm"
        ));
        assert!(
            handoff_readme.contains(
                "next_required_instruction_done_when=ready=true-and-configured_ready=true"
            )
        );
        assert!(handoff_readme.contains(
            "next_required_instruction_note=configures-non-mock-provider-then-writes-redacted-acceptance-report"
        ));
        assert!(handoff_readme.contains("remaining_required_execution_step_count=5"));
        assert!(handoff_readme.contains(
            "remaining_required_execution_step=1; instruction=configure-real-ai-provider; status=blocked; estimate=0.5-1h-if-credentials-are-ready; command=powershell -ExecutionPolicy Bypass -File .\\scripts\\ai_acceptance_gate.ps1 -ProviderId openai_main -Model gpt-4.1 -Preset openai -SecretRef default -DataRoot '<data_root>' -RequireReady; evidence=dist/AutoDesignMaker-rust/ai-acceptance.adm; done_when=ready=true-and-configured_ready=true; note=configures-non-mock-provider-then-writes-redacted-acceptance-report"
        ));
        assert!(handoff_readme.contains(
            "remaining_required_execution_step=5; instruction=confirm-final-delivery-package; status=waiting-for-strict-gate; estimate=0.1h-after-strict-release-gate; command=cargo run -q -p adm-cli -- finalize-handoff-package; evidence=dist/AutoDesignMaker-rust/final-handoff-manifest.adm; done_when=final-handoff-manifest-delivery_ready=true; note=requires-final-handoff-manifest-delivery_ready-true-before-full-completion"
        ));
        assert!(handoff_readme.contains(
            "instruction=configure-real-ai-provider; required=true; status=blocked; estimate=0.5-1h-if-credentials-are-ready; command=powershell -ExecutionPolicy Bypass -File .\\scripts\\ai_acceptance_gate.ps1 -ProviderId openai_main -Model gpt-4.1 -Preset openai -SecretRef default -DataRoot '<data_root>' -RequireReady; evidence=dist/AutoDesignMaker-rust/ai-acceptance.adm; note=configures-non-mock-provider-then-writes-redacted-acceptance-report"
        ));
        assert!(handoff_readme.contains(
            "instruction=run-unity-acceptance; required=true; status=blocked; estimate=1-3h-if-unity-is-installed; command=powershell -ExecutionPolicy Bypass -File .\\scripts\\unity_acceptance_gate.ps1 -ArchiveId archive_999 -UnityExe '<path-to-Unity.exe>' -DataRoot '<data_root>'; evidence=dist/unity-project/Assets/AutoDesignMaker/Generated/runtime_execution_results.adm; note=requires-compatible-unity-editor-and-unity_playmode-runtime-results"
        ));
        assert!(handoff_readme.contains("external_acceptance_data_root=.adm_rust_data"));
        assert!(handoff_readme.contains("unity_runtime_runner=cli_smoke_runner"));
        assert!(handoff_readme.contains("unity_selected=none"));
        assert!(handoff_readme.contains("unity_candidates=2"));
        assert!(handoff_readme.contains("unity_candidate_detail_count=2"));
        assert!(handoff_readme.contains(
            "unity_candidate=source=env; path=C:/Unity/Editor/Unity.exe; present=false; looks_like_unity_editor=true; ready=false"
        ));
        assert!(handoff_readme.contains(
            "unity_candidate=source=default; path=C:/Program Files/Unity/Editor/Unity.exe; present=false; looks_like_unity_editor=true; ready=false"
        ));
        assert!(handoff_readme.contains("ai_provider_id=openai_main"));
        assert!(handoff_readme.contains("ai_provider_model=gpt-4.1"));
        assert!(handoff_readme.contains("ai_diagnostic_readiness=MissingProvider"));
        assert!(handoff_readme.contains("real_ai_provider_count=0"));
        assert!(handoff_readme.contains("ready_provider_count=1"));
        assert!(handoff_readme.contains("ai_provider_detail_count=1"));
        assert!(handoff_readme.contains(
            "ai_provider=provider_id=mock; readiness=Ready; capabilities=text_generation; note=provider does not require a secret"
        ));
        assert!(handoff_readme.contains("suggested_ai_provider_preset=openai"));
        assert!(handoff_readme.contains("suggested_ai_secret_ref=default"));
        assert!(handoff_readme.contains("suggested_ai_secret_env_var=OPENAI_API_KEY"));
        assert!(handoff_readme.contains("suggested_ai_secret_requirement=env:OPENAI_API_KEY"));
        assert!(handoff_readme.contains(
            "suggested_ai_secret_check_command=powershell -NoProfile -Command \"[bool]`$env:OPENAI_API_KEY\""
        ));
        assert!(
            handoff_readme
                .contains("suggested_ai_secret_session_set_command=$env:OPENAI_API_KEY='<secret>'")
        );
        assert!(handoff_readme.contains("operator_input_count=2"));
        assert!(handoff_readme.contains(
            "operator_input=ai_secret; status=missing_secret_or_provider_config; placeholder=<secret>; requirement=env:OPENAI_API_KEY; check_command=powershell -NoProfile -Command \"[bool]`$env:OPENAI_API_KEY\"; set_command=$env:OPENAI_API_KEY='<secret>'; required_for=configure-real-ai-provider; note=provide-redacted-secret-in-receiving-shell-before-running-ai-acceptance"
        ));
        assert!(handoff_readme.contains(
            "operator_input=unity_exe; status=missing_or_not_ready; placeholder=<path-to-Unity.exe>; requirement=compatible-unity-editor-path; check_command=powershell -NoProfile -Command \"Test-Path -LiteralPath '<path-to-Unity.exe>'\"; set_command=replace-placeholder-in-unity-commands; required_for=run-unity-acceptance,rerun-external-acceptance,run-strict-release-gate; note=use-compatible-unity-editor-that-can-run-playmode-validation"
        ));
        assert!(handoff_readme.contains("suggested_unity_archive_id=archive_999"));
        assert!(handoff_readme.contains("suggested_unity_archive_source=data_root_latest_archive"));
        assert!(handoff_readme.contains(
            "suggested_ai_acceptance_command=powershell -ExecutionPolicy Bypass -File .\\scripts\\ai_acceptance_gate.ps1 -ProviderId openai_main -Model gpt-4.1 -Preset openai -SecretRef default -DataRoot '<data_root>' -RequireReady"
        ));
        assert!(handoff_readme.contains(
            "suggested_unity_acceptance_command=powershell -ExecutionPolicy Bypass -File .\\scripts\\unity_acceptance_gate.ps1 -ArchiveId archive_999 -UnityExe '<path-to-Unity.exe>' -DataRoot '<data_root>'"
        ));
        assert!(handoff_readme.contains(
            "suggested_strict_release_gate_ai_invoke_command=powershell -ExecutionPolicy Bypass -File .\\scripts\\release_gate.ps1 -RequireExternalAcceptance -RequireAiInvoke -UnityExe '<path-to-Unity.exe>' -DataRoot '<data_root>'"
        ));
        assert!(handoff_readme.contains(
            "suggested_operator_preflight_command=powershell -ExecutionPolicy Bypass -File .\\scripts\\handoff_operator_preflight.ps1 -DataRoot '<data_root>'"
        ));
        assert!(handoff_readme.contains(
            "suggested_operator_preflight_require_ready_command=powershell -ExecutionPolicy Bypass -File .\\scripts\\handoff_operator_preflight.ps1 -UnityExe '<path-to-Unity.exe>' -DataRoot '<data_root>' -RequireReady"
        ));
        assert!(
            handoff_readme.contains(
                "operator_preflight_working_dir=rust-workspace-root-with-scripts-directory"
            )
        );
        assert!(handoff_readme.contains("operator_preflight_bundle_root_supported=true"));
        assert!(handoff_readme.contains(
            "operator_preflight_bundle_root_script=source-bundle/scripts/handoff_operator_preflight.ps1"
        ));
        assert!(handoff_readme.contains(
            "operator_preflight_bundle_root_instructions_path=..\\evidence\\handoff-instructions.adm"
        ));
        assert!(handoff_readme.contains(
            "suggested_operator_preflight_bundle_root_command=powershell -ExecutionPolicy Bypass -File .\\source-bundle\\scripts\\handoff_operator_preflight.ps1 -InstructionsPath ..\\evidence\\handoff-instructions.adm -DataRoot '<data_root>'"
        ));
        assert!(handoff_readme.contains(
            "suggested_operator_preflight_bundle_root_require_ready_command=powershell -ExecutionPolicy Bypass -File .\\source-bundle\\scripts\\handoff_operator_preflight.ps1 -InstructionsPath ..\\evidence\\handoff-instructions.adm -UnityExe '<path-to-Unity.exe>' -DataRoot '<data_root>' -RequireReady"
        ));
        assert!(handoff_readme.contains("handoff_rehydration_bundle_root_supported=true"));
        assert!(handoff_readme.contains(
            "handoff_rehydration_script=source-bundle/scripts/rehydrate_handoff_workspace.ps1"
        ));
        assert!(handoff_readme.contains(
            "handoff_rehydration_destination_placeholder=<path-to-rehydrated-rust-workspace>"
        ));
        assert!(
            handoff_readme
                .contains("handoff_rehydration_manifest=dist/handoff-rehydration-manifest.adm")
        );
        assert!(handoff_readme.contains(
            "suggested_handoff_rehydration_command=powershell -ExecutionPolicy Bypass -File .\\source-bundle\\scripts\\rehydrate_handoff_workspace.ps1 -DestinationPath '<path-to-rehydrated-rust-workspace>'"
        ));
        assert!(handoff_readme.contains(
            "rehydrated_release_smoke_report=dist/AutoDesignMaker-rust/release-acceptance.adm"
        ));
        assert!(handoff_readme.contains(
            "rehydrated_release_smoke_command=.\\dist\\AutoDesignMaker-rust\\AutoDesignMaker-rust.exe --smoke"
        ));
        assert!(
            handoff_readme
                .contains("rehydrated_release_smoke_working_dir=rehydrated-rust-workspace-root")
        );
        assert!(handoff_readme.contains(
            "final_acceptance_working_dir=rust-workspace-root-after-rehydration-or-original"
        ));
        assert!(
            handoff_readme.contains("final_acceptance_script=scripts/final_handoff_acceptance.ps1")
        );
        assert!(handoff_readme.contains(
            "final_acceptance_sequence=operator-preflight,ai-acceptance,unity-acceptance,external-acceptance,strict-release-gate"
        ));
        assert!(handoff_readme.contains("final_acceptance_requires=ai_secret,unity_exe,data_root"));
        assert!(handoff_readme.contains(
            "final_acceptance_report=dist/AutoDesignMaker-rust/final-acceptance-run.adm"
        ));
        assert!(
            handoff_readme
                .contains("final_acceptance_package_refresh=after-successful-default-report-write")
        );
        assert!(handoff_readme.contains(
            "suggested_final_acceptance_command=powershell -ExecutionPolicy Bypass -File .\\scripts\\final_handoff_acceptance.ps1 -UnityExe '<path-to-Unity.exe>' -DataRoot '<data_root>'"
        ));
        assert!(handoff_readme.contains(
            "suggested_final_acceptance_ai_invoke_command=powershell -ExecutionPolicy Bypass -File .\\scripts\\final_handoff_acceptance.ps1 -UnityExe '<path-to-Unity.exe>' -DataRoot '<data_root>' -RequireAiInvoke"
        ));
        assert!(handoff_readme.contains(
            "strict_gate_command=powershell -ExecutionPolicy Bypass -File .\\scripts\\release_gate.ps1 -RequireExternalAcceptance -UnityExe '<path-to-Unity.exe>' -DataRoot '<data_root>'"
        ));
        assert!(handoff_readme.contains(
            "strict_gate_ai_invoke_command=powershell -ExecutionPolicy Bypass -File .\\scripts\\release_gate.ps1 -RequireExternalAcceptance -RequireAiInvoke -UnityExe '<path-to-Unity.exe>' -DataRoot '<data_root>'"
        ));
        assert!(handoff_readme.contains("strict_gate_requires_final_delivery=true"));
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn finalize_handoff_package_reports_delivery_ready_when_handoff_evidence_is_ready() {
        let dir = unique_test_dir("final-handoff-delivery-ready");
        let bundle_dir = dir.join("bundle");
        let report_path = dir.join("release").join("final-handoff-manifest.adm");
        fs::create_dir_all(bundle_dir.join("AutoDesignMaker-rust")).unwrap();
        fs::create_dir_all(bundle_dir.join("source-bundle")).unwrap();
        fs::create_dir_all(bundle_dir.join("evidence")).unwrap();
        fs::write(
            bundle_dir.join("AutoDesignMaker-rust").join("app.exe"),
            "exe",
        )
        .unwrap();
        fs::write(
            bundle_dir.join("source-bundle").join("Cargo.toml"),
            "[workspace]\n",
        )
        .unwrap();
        fs::write(
            bundle_dir.join("handoff-bundle-manifest.adm"),
            "# Handoff Bundle\nready=true\nbundle_hash=fnv64:bundle\n",
        )
        .unwrap();
        fs::write(
            bundle_dir
                .join("evidence")
                .join("handoff-evidence-manifest.adm"),
            "# Handoff Evidence\nready=true\nhandoff_ready=true\nexternal_acceptance_ready=true\nai_acceptance_ready=true\nevidence_hash=fnv64:evidence\n",
        )
        .unwrap();
        fs::write(
            bundle_dir.join("evidence").join("handoff-instructions.adm"),
            "ready=true\nfinal_package_ready=true\nfinal_delivery_ready=true\nfinal_handoff_ready=true\nblocker_count=0\ninstruction_count=5\n",
        )
        .unwrap();

        let report = finalize_handoff_package(bundle_dir.clone(), report_path.clone()).unwrap();
        let rendered = fs::read_to_string(&report_path).unwrap();
        let handoff_readme = fs::read_to_string(bundle_dir.join("HANDOFF_README.txt")).unwrap();
        assert!(report.ready());
        assert!(report.package_ready());
        assert!(report.delivery_ready());
        assert!(rendered.contains("delivery_ready=true"));
        assert!(rendered.contains("handoff_ready=true"));
        assert!(rendered.contains("external_acceptance_ready=true"));
        assert!(rendered.contains("ai_acceptance_ready=true"));
        assert!(rendered.contains("required_file=HANDOFF_README.txt; present=true"));
        assert!(handoff_readme.contains("handoff_ready=true"));
        assert!(handoff_readme.contains("external_acceptance_ready=true"));
        assert!(handoff_readme.contains("ai_acceptance_ready=true"));
        assert!(handoff_readme.contains("strict_gate_bundle_root_runnable=false"));
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn stage_source_bundle_copies_sources_and_excludes_generated_dirs() {
        let dir = unique_test_dir("source-bundle");
        let source_root = dir.join("source");
        let bundle_dir = dir.join("bundle");
        let report_path = dir.join("release").join("source-manifest.adm");
        fs::create_dir_all(source_root.join("apps").join("adm-cli")).unwrap();
        fs::create_dir_all(source_root.join("target")).unwrap();
        fs::create_dir_all(source_root.join("dist")).unwrap();
        fs::create_dir_all(source_root.join(".adm_rust_data")).unwrap();
        fs::write(source_root.join("Cargo.toml"), "[workspace]\n").unwrap();
        fs::write(
            source_root.join("apps").join("adm-cli").join("main.rs"),
            "fn main() {}\n",
        )
        .unwrap();
        fs::write(source_root.join("target").join("ignored.txt"), "target").unwrap();
        fs::write(source_root.join("dist").join("ignored.txt"), "dist").unwrap();
        fs::write(
            source_root.join(".adm_rust_data").join("ignored.txt"),
            "data",
        )
        .unwrap();
        fs::create_dir_all(bundle_dir.join("stale")).unwrap();
        fs::write(bundle_dir.join("stale").join("old.rs"), "old").unwrap();

        let report =
            stage_source_bundle(source_root.clone(), bundle_dir.clone(), report_path.clone())
                .unwrap();
        let rendered = fs::read_to_string(report_path).unwrap();
        assert!(report.ready());
        assert_eq!(report.files.len(), 2);
        assert!(bundle_dir.join("Cargo.toml").exists());
        assert!(
            bundle_dir
                .join("apps")
                .join("adm-cli")
                .join("main.rs")
                .exists()
        );
        assert!(!bundle_dir.join("target").join("ignored.txt").exists());
        assert!(!bundle_dir.join("dist").join("ignored.txt").exists());
        assert!(
            !bundle_dir
                .join(".adm_rust_data")
                .join("ignored.txt")
                .exists()
        );
        assert!(!bundle_dir.join("stale").join("old.rs").exists());
        assert!(rendered.contains("ready=true"));
        assert!(rendered.contains("source_handoff_mode=bundled"));
        assert!(rendered.contains("stale_cleanup=removed_existing_bundle_dir"));
        assert!(rendered.contains("file_count=2"));
        assert!(rendered.contains("bundle_hash=fnv64:"));
        assert!(rendered.contains("- path=apps/adm-cli/main.rs;"));
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn stage_source_bundle_rejects_bundle_dir_that_contains_source_root() {
        let dir = unique_test_dir("source-bundle-guard");
        let source_root = dir.join("source");
        fs::create_dir_all(&source_root).unwrap();
        fs::write(source_root.join("Cargo.toml"), "[workspace]\n").unwrap();
        let result = stage_source_bundle(
            source_root,
            dir.clone(),
            dir.join("release").join("source-manifest.adm"),
        );
        assert!(result.is_err());
        let error = result.unwrap_err().to_string();
        assert!(
            error.contains("source bundle dir must not be the source root or a parent"),
            "{error}"
        );
        let _ = fs::remove_dir_all(dir);
    }
}
