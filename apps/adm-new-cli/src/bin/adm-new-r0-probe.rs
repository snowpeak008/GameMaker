#![forbid(unsafe_code)]

use std::collections::BTreeMap;
use std::env;
use std::fmt;
use std::fs;
use std::io::{self, Read};
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode, Stdio};
use std::thread;
use std::time::{Duration, Instant};

use adm_new_ai::adapters::CodexCliAdapter;
use adm_new_application::work_unit_executor_from_config;
use adm_new_contracts::ai::{AiConfig, ApiCategory, ApiEntry};
use adm_new_contracts::patch::{PatchRecord, PatchStatus, PatchTask};
use adm_new_foundation::process::terminate_child_process_tree;
use adm_new_foundation::{ensure_relative_path, sha256_hex, write_text_atomic};
use adm_new_game_spec::{canonicalize_game_spec, parse_game_spec, validate_game_spec};
use adm_new_patch::{CodexPatchRunner, PatchRunner};
use adm_new_pipeline::{
    SafeUnitJournal, WorkUnitKind, WorkUnitRequest, WorkUnitRunStatus, WorkUnitStopToken,
    execute_work_unit_batch,
};
use serde::Serialize;
use serde_json::{Value, json};

const REPORT_SCHEMA_VERSION: u32 = 1;
const OUTPUT_MARKER: &str = ".adm-r0-probe-root";
const INPUT_RELATIVE: &str = "ProbeInput/r0_game_spec.json";
const GENERATED_SCRIPT: &str = "Assets/Scripts/R0GeneratedProbe.cs";
const BUILD_LOG_RELATIVE: &str = "evidence/unity-build.log";
const SMOKE_LOG_RELATIVE: &str = "evidence/player-smoke.log";
const REPORT_RELATIVE: &str = "evidence/r0-probe-report.json";

const BOOTSTRAP_FILES: [&str; 3] = [
    "Assets/Editor/R0Build.cs",
    "Packages/manifest.json",
    "ProjectSettings/ProjectVersion.txt",
];

fn main() -> ExitCode {
    let arguments = env::args().collect::<Vec<_>>();
    if arguments.get(1).map(String::as_str) == Some("exec") {
        return match run_adapter_mode() {
            Ok(()) => ExitCode::SUCCESS,
            Err(message) => {
                eprintln!("R0 probe adapter failed: {message}");
                ExitCode::from(1)
            }
        };
    }
    if arguments.get(1).map(String::as_str) != Some("run") {
        eprintln!(
            "usage: adm-new-r0-probe run --workspace-root <path> --fixture <path> --unity-editor <path>"
        );
        return ExitCode::from(2);
    }
    match run_probe_command(&arguments[2..]) {
        Ok(()) => ExitCode::SUCCESS,
        Err(message) => {
            eprintln!("R0 probe failed: {message}");
            ExitCode::from(1)
        }
    }
}

fn run_probe_command(arguments: &[String]) -> Result<(), String> {
    let paths = ProbePaths::prepare(parse_run_arguments(arguments)?)?;
    let mut report = ProbeReport::new(paths.unity_editor_version.clone());
    let result = execute_probe(&paths, &mut report);
    match &result {
        Ok(()) => report.status = "passed".to_string(),
        Err(error) => {
            report.status = "failed".to_string();
            report.failure = Some(ProbeFailure {
                kind: error.kind,
                phase: error.phase.clone(),
                message: error.message.clone(),
            });
        }
    }
    report.deterministic_fingerprint = deterministic_fingerprint(&report)?;
    write_report(&paths, &report)?;
    println!("R0_REPORT={REPORT_RELATIVE}");
    println!("R0_FINGERPRINT={}", report.deterministic_fingerprint);
    result.map_err(|error| error.to_string())
}

fn execute_probe(paths: &ProbePaths, report: &mut ProbeReport) -> Result<(), ProbeError> {
    let input = run_phase(report, "fixed_spec", || load_fixed_spec(paths))?;
    report.fixture_content_hash = input.content_hash;
    report.fixture_project_id = input.project_id;

    run_phase(report, "patch_bootstrap", || bootstrap_project(paths))?;
    run_phase(report, "scope_violation_rejection", || {
        verify_scope_violation_rejection(paths)
    })?;
    run_phase(report, "development_work_unit", || {
        execute_development_work_unit(paths)
    })?;
    let build = run_phase(report, "unity_player_build", || build_player(paths))?;
    run_phase(report, "player_smoke", || smoke_player(paths, &build))?;
    Ok(())
}

fn load_fixed_spec(paths: &ProbePaths) -> Result<PhaseOutput<FixedInput>, ProbeError> {
    let input = fs::read_to_string(&paths.fixture)
        .map_err(|_| ProbeError::input("fixed_spec", "fixed GameSpec could not be read"))?;
    let spec = parse_game_spec(&input)
        .map_err(|_| ProbeError::input("fixed_spec", "fixed GameSpec did not parse strictly"))?;
    let validation = validate_game_spec(&spec);
    if !validation.is_valid() {
        return Err(ProbeError::input(
            "fixed_spec",
            "fixed GameSpec failed deterministic validation",
        ));
    }
    let canonical = canonicalize_game_spec(&spec)
        .map_err(|_| ProbeError::input("fixed_spec", "fixed GameSpec hashing failed"))?;
    let project_input = paths.project_root.join(INPUT_RELATIVE);
    write_file(&project_input, input.as_bytes(), "fixed_spec")?;
    Ok(PhaseOutput {
        value: FixedInput {
            content_hash: canonical.content_hash.clone(),
            project_id: spec.identity.project_id.to_string(),
        },
        evidence: json!({
            "canonicalHash": canonical.content_hash,
            "validationErrors": 0,
            "schemaVersion": spec.identity.schema_version,
        }),
        artifacts: vec![artifact(paths, &project_input, "fixed_spec")?],
    })
}

fn bootstrap_project(paths: &ProbePaths) -> Result<PhaseOutput<()>, ProbeError> {
    let expected = BOOTSTRAP_FILES.map(str::to_string).to_vec();
    let record = patch_record(
        "r0-bootstrap",
        format!(
            "R0_PATCH_BOOTSTRAP\nR0_UNITY_VERSION={}",
            paths.unity_editor_version
        ),
        &expected,
    );
    let runner = CodexPatchRunner::new(
        &paths.project_root,
        CodexCliAdapter {
            cli_path: paths.probe_executable.to_string_lossy().into_owned(),
        },
    )
    .with_timeout_seconds(60);
    let result = runner
        .run(&record)
        .map_err(|_| ProbeError::agent("patch_bootstrap", "CodexPatchRunner invocation failed"))?;
    if result.status != "success" {
        return Err(ProbeError::agent(
            "patch_bootstrap",
            "CodexPatchRunner did not commit the declared bootstrap outputs",
        ));
    }
    let mut changed_files = result.changed_files;
    changed_files.sort();
    let mut hashes = BTreeMap::new();
    let mut artifacts = Vec::new();
    for relative in &expected {
        let path = paths.project_root.join(relative);
        let item = artifact(paths, &path, "patch_bootstrap")?;
        hashes.insert(relative.clone(), item.sha256.clone());
        artifacts.push(item);
    }
    Ok(PhaseOutput {
        value: (),
        evidence: json!({
            "changedFiles": changed_files,
            "sourceHashes": hashes,
        }),
        artifacts,
    })
}

fn verify_scope_violation_rejection(paths: &ProbePaths) -> Result<PhaseOutput<()>, ProbeError> {
    let declared = "Assets/Editor/R0ScopeSentinel.cs";
    let unexpected = "Assets/R0UnexpectedWrite.cs";
    let expected = vec![declared.to_string()];
    let record = patch_record(
        "r0-scope-negative",
        "R0_SCOPE_VIOLATION_PHASE".to_string(),
        &expected,
    );
    let runner = CodexPatchRunner::new(
        &paths.project_root,
        CodexCliAdapter {
            cli_path: paths.probe_executable.to_string_lossy().into_owned(),
        },
    )
    .with_timeout_seconds(60);
    let result = runner.run(&record).map_err(|_| {
        ProbeError::agent(
            "scope_violation_rejection",
            "scope-negative adapter invocation failed",
        )
    })?;
    let rejected_for_scope = result.status == "failed"
        && result
            .errors
            .iter()
            .any(|message| message.contains("undeclared file"));
    let project_unchanged = !paths.project_root.join(declared).exists()
        && !paths.project_root.join(unexpected).exists();
    if !rejected_for_scope || !project_unchanged {
        return Err(ProbeError::scope(
            "scope_violation_rejection",
            "undeclared adapter output was not rejected without mutation",
        ));
    }
    Ok(PhaseOutput {
        value: (),
        evidence: json!({
            "rejected": true,
            "projectMutationCount": 0,
            "failureKind": "scope_violation",
        }),
        artifacts: Vec::new(),
    })
}

fn execute_development_work_unit(paths: &ProbePaths) -> Result<PhaseOutput<()>, ProbeError> {
    fs::create_dir_all(&paths.execution_object_root).map_err(|_| {
        ProbeError::tooling(
            "development_work_unit",
            "execution evidence directory could not be created",
        )
    })?;
    let entry_id = "r0-probe-adapter";
    let config = AiConfig {
        dev: ApiCategory {
            category_id: "dev".to_string(),
            entries: vec![ApiEntry {
                id: entry_id.to_string(),
                label: "R0 deterministic probe adapter".to_string(),
                config_type: "local_codex_cli".to_string(),
                extra_json: json!({
                    "cli_path": paths.probe_executable.to_string_lossy(),
                }),
                ..ApiEntry::default()
            }],
            active_entry_id: entry_id.to_string(),
        },
        ..AiConfig::default()
    };
    let executor = work_unit_executor_from_config(
        &config,
        &paths.project_root,
        Some(&paths.unity_editor),
        &paths.execution_object_root,
        "draft:r0-probe",
    )
    .map_err(|_| {
        ProbeError::tooling(
            "development_work_unit",
            "current WorkUnit executor could not be configured",
        )
    })?;
    let request = WorkUnitRequest::new(
        "11",
        "r0-generate-runtime",
        WorkUnitKind::Development,
        json!({
            "description": "R0_WORK_UNIT_PHASE",
            "artifact_locale": "en-US",
            "input_files": [INPUT_RELATIVE],
            "output_files": [GENERATED_SCRIPT],
            "allowed_write_paths": [GENERATED_SCRIPT],
            "timeout_seconds": 120,
            "unity_validation_timeout_seconds": 900,
        }),
    )
    .map_err(|_| {
        ProbeError::input(
            "development_work_unit",
            "development WorkUnit request was invalid",
        )
    })?;
    let journal = SafeUnitJournal::new(paths.evidence_root.join("work-unit-journal"));
    let batch = execute_work_unit_batch(
        vec![request],
        Some(executor.as_ref()),
        &journal,
        &WorkUnitStopToken::default(),
    )
    .map_err(|_| {
        ProbeError::tooling(
            "development_work_unit",
            "WorkUnit batch execution failed before producing a result",
        )
    })?;
    let outcome = batch.units.first().ok_or_else(|| {
        ProbeError::tooling(
            "development_work_unit",
            "WorkUnit batch produced no unit outcome",
        )
    })?;
    if outcome.status != WorkUnitRunStatus::Committed {
        let message = outcome
            .result
            .as_ref()
            .map(|result| result.message.clone())
            .or_else(|| {
                journal
                    .load(&outcome.request)
                    .ok()
                    .flatten()
                    .and_then(|record| record.result.map(|result| result.message))
            })
            .unwrap_or_else(|| outcome.message.clone());
        return Err(classify_work_unit_failure(&message));
    }
    let result = outcome.result.as_ref().ok_or_else(|| {
        ProbeError::evidence(
            "development_work_unit",
            "committed WorkUnit has no result evidence",
        )
    })?;
    let verification = result
        .verification_results
        .iter()
        .map(|item| {
            json!({
                "id": item.get("id").and_then(Value::as_str).unwrap_or("unknown"),
                "status": item.get("status").and_then(Value::as_str).unwrap_or("unknown"),
            })
        })
        .collect::<Vec<_>>();
    let generated = paths.project_root.join(GENERATED_SCRIPT);
    Ok(PhaseOutput {
        value: (),
        evidence: json!({
            "runStatus": "committed",
            "outputFiles": [GENERATED_SCRIPT],
            "verification": verification,
        }),
        artifacts: vec![artifact(paths, &generated, "development_work_unit")?],
    })
}

fn build_player(paths: &ProbePaths) -> Result<PhaseOutput<PlayerBuild>, ProbeError> {
    fs::create_dir_all(&paths.player_root).map_err(|_| {
        ProbeError::tooling(
            "unity_player_build",
            "player output directory could not be created",
        )
    })?;
    let executable = paths.player_root.join("R0Probe.exe");
    let log = paths.output_root.join(BUILD_LOG_RELATIVE);
    let editor_path = external_process_path(&paths.unity_editor);
    let project_path = external_process_path(&paths.project_root);
    let process_log = external_process_path(&log);
    let process_executable = external_process_path(&executable);
    let mut command = Command::new(editor_path);
    command
        .args(["-batchmode", "-quit", "-projectPath"])
        .arg(&project_path)
        .args(["-executeMethod", "R0Build.BuildWindows", "-logFile"])
        .arg(&process_log)
        .env("ADM_R0_OUTPUT_EXE", &process_executable)
        .current_dir(&project_path);
    let outcome = run_hidden_process(&mut command, Duration::from_secs(900), "unity_player_build")?;
    if outcome.timed_out {
        return Err(ProbeError::timeout(
            "unity_player_build",
            "Unity BuildPipeline timed out",
        ));
    }
    let log_text = read_lossy(&log);
    if outcome.exit_code != Some(0) || !log_text.contains("ADM_R0_BUILD_PASS") {
        return Err(if unity_log_has_compile_error(&log_text) {
            ProbeError::compile(
                "unity_player_build",
                "Unity BuildPipeline reported compiler errors",
            )
        } else {
            ProbeError::test(
                "unity_player_build",
                "Unity BuildPipeline did not produce complete success evidence",
            )
        });
    }
    let managed_assembly = paths
        .player_root
        .join("R0Probe_Data/Managed/Assembly-CSharp.dll");
    if !executable.is_file() || !managed_assembly.is_file() {
        return Err(ProbeError::evidence(
            "unity_player_build",
            "built player or managed gameplay assembly is missing",
        ));
    }
    Ok(PhaseOutput {
        value: PlayerBuild {
            executable: executable.clone(),
        },
        evidence: json!({
            "buildMarker": true,
            "executablePresent": true,
            "managedGameplayAssemblyPresent": true,
        }),
        artifacts: vec![
            artifact(paths, &log, "unity_player_build")?,
            artifact(paths, &executable, "unity_player_build")?,
            artifact(paths, &managed_assembly, "unity_player_build")?,
        ],
    })
}

fn smoke_player(paths: &ProbePaths, build: &PlayerBuild) -> Result<PhaseOutput<()>, ProbeError> {
    let log = paths.output_root.join(SMOKE_LOG_RELATIVE);
    let executable = external_process_path(&build.executable);
    let log_path = external_process_path(&log);
    let player_root = external_process_path(&paths.player_root);
    let mut command = Command::new(executable);
    command
        .args(["--r0-smoke", "-batchmode", "-nographics", "-logFile"])
        .arg(&log_path)
        .current_dir(&player_root);
    let outcome = run_hidden_process(&mut command, Duration::from_secs(60), "player_smoke")?;
    if outcome.timed_out {
        return Err(ProbeError::timeout(
            "player_smoke",
            "built player smoke timed out",
        ));
    }
    let log_text = read_lossy(&log);
    if outcome.exit_code != Some(0) || !log_text.contains("ADM_R0_SMOKE_PASS") {
        return Err(ProbeError::test(
            "player_smoke",
            "built player did not emit the deterministic smoke marker",
        ));
    }
    Ok(PhaseOutput {
        value: (),
        evidence: json!({
            "exitCode": 0,
            "smokeMarker": true,
        }),
        artifacts: vec![artifact(paths, &log, "player_smoke")?],
    })
}

fn run_phase<T>(
    report: &mut ProbeReport,
    id: &str,
    action: impl FnOnce() -> Result<PhaseOutput<T>, ProbeError>,
) -> Result<T, ProbeError> {
    let started = Instant::now();
    match action() {
        Ok(output) => {
            report.phases.push(PhaseEvidence {
                id: id.to_string(),
                status: "passed".to_string(),
                duration_ms: duration_ms(started.elapsed()),
                evidence: output.evidence,
            });
            report.artifacts.extend(output.artifacts);
            Ok(output.value)
        }
        Err(error) => {
            report.phases.push(PhaseEvidence {
                id: id.to_string(),
                status: "failed".to_string(),
                duration_ms: duration_ms(started.elapsed()),
                evidence: json!({ "failureKind": error.kind }),
            });
            Err(error)
        }
    }
}

fn run_adapter_mode() -> Result<(), String> {
    let mut prompt = String::new();
    io::stdin()
        .read_to_string(&mut prompt)
        .map_err(|_| "adapter prompt could not be read".to_string())?;
    let root = env::current_dir().map_err(|_| "adapter work root is unavailable".to_string())?;
    if prompt.contains("R0_PATCH_BOOTSTRAP") {
        let version = marker_value(&prompt, "R0_UNITY_VERSION=")
            .ok_or_else(|| "Unity version marker is missing".to_string())?;
        write_adapter_file(
            &root,
            "ProjectSettings/ProjectVersion.txt",
            &format!("m_EditorVersion: {version}\n"),
        )?;
        write_adapter_file(
            &root,
            "Packages/manifest.json",
            "{\n  \"dependencies\": {}\n}\n",
        )?;
        write_adapter_file(&root, "Assets/Editor/R0Build.cs", unity_build_source())?;
        println!("R0 patch bootstrap outputs generated");
        return Ok(());
    }
    if prompt.contains("R0_SCOPE_VIOLATION_PHASE") {
        write_adapter_file(
            &root,
            "Assets/Editor/R0ScopeSentinel.cs",
            "public static class R0ScopeSentinel {}\n",
        )?;
        write_adapter_file(
            &root,
            "Assets/R0UnexpectedWrite.cs",
            "public static class R0UnexpectedWrite {}\n",
        )?;
        println!("R0 intentional scope violation generated");
        return Ok(());
    }
    if prompt.contains("R0_WORK_UNIT_PHASE") {
        write_adapter_file(&root, GENERATED_SCRIPT, unity_runtime_source())?;
        println!("R0 WorkUnit runtime output generated");
        return Ok(());
    }
    Err("unrecognized deterministic probe phase".to_string())
}

fn marker_value<'a>(input: &'a str, prefix: &str) -> Option<&'a str> {
    input
        .lines()
        .find_map(|line| line.trim().strip_prefix(prefix))
}

fn write_adapter_file(root: &Path, relative: &str, content: &str) -> Result<(), String> {
    let target = ensure_relative_path(root, relative)
        .map_err(|_| "adapter output path was rejected".to_string())?;
    let parent = target
        .parent()
        .ok_or_else(|| "adapter output has no parent".to_string())?;
    fs::create_dir_all(parent).map_err(|_| "adapter output parent could not be created")?;
    write_text_atomic(&target, content)
        .map_err(|_| "adapter output could not be written atomically".to_string())
}

fn patch_record(patch_id: &str, request: String, expected_files: &[String]) -> PatchRecord {
    PatchRecord {
        patch_id: patch_id.to_string(),
        request,
        status: PatchStatus::Analyzed,
        created_at: String::new(),
        updated_at: String::new(),
        tasks: vec![PatchTask {
            task_id: format!("{patch_id}-task"),
            title: "R0 disposable technical probe".to_string(),
            description: "Generate only the declared disposable probe outputs.".to_string(),
            affected_systems: vec!["r0-probe".to_string()],
            expected_files: expected_files.to_vec(),
            validation_route: vec!["r0-harness".to_string()],
            requires_iteration: false,
        }],
        changed_files: Vec::new(),
        validation_summary: Value::Null,
        analysis_summary: String::new(),
        executor_result: Value::Null,
        promoted_iteration_spec: String::new(),
        errors: Vec::new(),
    }
}

fn classify_work_unit_failure(message: &str) -> ProbeError {
    let lower = message.to_ascii_lowercase();
    if lower.contains("outside the declared") || lower.contains("undeclared") {
        ProbeError::scope(
            "development_work_unit",
            "WorkUnit rejected an output scope violation",
        )
    } else if lower.contains("timed out") || lower.contains("timeout") {
        ProbeError::timeout("development_work_unit", "WorkUnit execution timed out")
    } else if lower.contains("unity") || lower.contains("compiler") {
        ProbeError::compile(
            "development_work_unit",
            "WorkUnit Unity compilation did not pass",
        )
    } else if lower.contains("cli") || lower.contains("adapter") {
        ProbeError::agent(
            "development_work_unit",
            "WorkUnit adapter did not produce a verified result",
        )
    } else {
        ProbeError::test(
            "development_work_unit",
            "WorkUnit result did not satisfy the technical probe contract",
        )
    }
}

fn run_hidden_process(
    command: &mut Command,
    timeout: Duration,
    phase: &str,
) -> Result<ProcessOutcome, ProbeError> {
    command
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        command.creation_flags(CREATE_NO_WINDOW);
    }
    let mut child = command
        .spawn()
        .map_err(|_| ProbeError::tooling(phase, "required probe process could not be started"))?;
    let started = Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                return Ok(ProcessOutcome {
                    exit_code: status.code(),
                    timed_out: false,
                });
            }
            Ok(None) => {}
            Err(_) => {
                let _ = terminate_child_process_tree(&mut child);
                return Err(ProbeError::tooling(
                    phase,
                    "required probe process could not be observed",
                ));
            }
        }
        if started.elapsed() >= timeout {
            let status = terminate_child_process_tree(&mut child).map_err(|_| {
                ProbeError::tooling(phase, "timed-out probe process could not be terminated")
            })?;
            return Ok(ProcessOutcome {
                exit_code: status.code(),
                timed_out: true,
            });
        }
        thread::sleep(Duration::from_millis(100));
    }
}

fn unity_log_has_compile_error(log: &str) -> bool {
    let lower = log.to_ascii_lowercase();
    [
        ": error cs",
        "scripts have compiler errors",
        "compilation failed",
        "compile errors in player scripts",
    ]
    .iter()
    .any(|marker| lower.contains(marker))
}

fn read_lossy(path: &Path) -> String {
    fs::read(path)
        .map(|bytes| String::from_utf8_lossy(&bytes).into_owned())
        .unwrap_or_default()
}

fn external_process_path(path: &Path) -> PathBuf {
    let value = path.to_string_lossy();
    if let Some(unc) = value.strip_prefix(r"\\?\UNC\") {
        PathBuf::from(format!(r"\\{unc}"))
    } else if let Some(local) = value.strip_prefix(r"\\?\") {
        PathBuf::from(local)
    } else {
        path.to_path_buf()
    }
}

fn write_file(path: &Path, bytes: &[u8], phase: &str) -> Result<(), ProbeError> {
    let parent = path
        .parent()
        .ok_or_else(|| ProbeError::tooling(phase, "probe output has no parent directory"))?;
    fs::create_dir_all(parent)
        .map_err(|_| ProbeError::tooling(phase, "probe output parent could not be created"))?;
    fs::write(path, bytes)
        .map_err(|_| ProbeError::tooling(phase, "probe output could not be written"))
}

fn artifact(paths: &ProbePaths, path: &Path, phase: &str) -> Result<ArtifactEvidence, ProbeError> {
    let canonical = fs::canonicalize(path)
        .map_err(|_| ProbeError::evidence(phase, "expected probe artifact is unavailable"))?;
    if !canonical.starts_with(&paths.output_root) || !canonical.is_file() {
        return Err(ProbeError::evidence(
            phase,
            "expected probe artifact escaped the generated output root",
        ));
    }
    let bytes = fs::read(&canonical)
        .map_err(|_| ProbeError::evidence(phase, "expected probe artifact could not be read"))?;
    let relative_path = canonical
        .strip_prefix(&paths.output_root)
        .map_err(|_| ProbeError::evidence(phase, "artifact path could not be normalized"))?
        .to_string_lossy()
        .replace('\\', "/");
    Ok(ArtifactEvidence {
        relative_path,
        sha256: sha256_hex(&bytes),
        bytes: bytes.len() as u64,
    })
}

fn deterministic_fingerprint(report: &ProbeReport) -> Result<String, String> {
    let phases = report
        .phases
        .iter()
        .map(|phase| {
            json!({
                "id": phase.id,
                "status": phase.status,
                "evidence": phase.evidence,
            })
        })
        .collect::<Vec<_>>();
    let stable = json!({
        "schemaVersion": report.schema_version,
        "status": report.status,
        "fixtureContentHash": report.fixture_content_hash,
        "fixtureProjectId": report.fixture_project_id,
        "unityEditorVersion": report.unity_editor_version,
        "phases": phases,
        "failureKind": report.failure.as_ref().map(|failure| failure.kind),
    });
    serde_json::to_vec(&stable)
        .map(|bytes| sha256_hex(&bytes))
        .map_err(|_| "probe fingerprint could not be serialized".to_string())
}

fn write_report(paths: &ProbePaths, report: &ProbeReport) -> Result<(), String> {
    let path = paths.output_root.join(REPORT_RELATIVE);
    let mut text = serde_json::to_string_pretty(report)
        .map_err(|_| "probe report could not be serialized".to_string())?;
    text.push('\n');
    write_text_atomic(&path, &text)
        .map_err(|_| "probe report could not be written atomically".to_string())
}

fn parse_run_arguments(arguments: &[String]) -> Result<RunArguments, String> {
    let mut values = BTreeMap::new();
    let mut index = 0;
    while index < arguments.len() {
        let key = arguments[index].as_str();
        if !matches!(key, "--workspace-root" | "--fixture" | "--unity-editor") {
            return Err(format!("unknown R0 probe argument: {key}"));
        }
        let value = arguments
            .get(index + 1)
            .ok_or_else(|| format!("missing value for {key}"))?;
        values.insert(key.to_string(), value.clone());
        index += 2;
    }
    let required = |key: &str| {
        values
            .get(key)
            .filter(|value| !value.trim().is_empty())
            .map(PathBuf::from)
            .ok_or_else(|| format!("required R0 probe argument is missing: {key}"))
    };
    Ok(RunArguments {
        workspace_root: required("--workspace-root")?,
        fixture: required("--fixture")?,
        unity_editor: required("--unity-editor")?,
    })
}

impl ProbePaths {
    fn prepare(arguments: RunArguments) -> Result<Self, String> {
        let workspace_root = fs::canonicalize(arguments.workspace_root)
            .map_err(|_| "workspace root is unavailable".to_string())?;
        if !workspace_root.join("Cargo.toml").is_file() {
            return Err("workspace root has no Cargo manifest".to_string());
        }
        let fixture = fs::canonicalize(arguments.fixture)
            .map_err(|_| "fixed GameSpec fixture is unavailable".to_string())?;
        if !fixture.starts_with(&workspace_root) || !fixture.is_file() {
            return Err("fixed GameSpec fixture must be a workspace file".to_string());
        }
        let unity_editor = fs::canonicalize(arguments.unity_editor)
            .map_err(|_| "Unity editor is unavailable".to_string())?;
        let valid_editor_name = unity_editor
            .file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| name.eq_ignore_ascii_case("Unity.exe"));
        if !unity_editor.is_file() || !valid_editor_name {
            return Err("Unity editor path must name Unity.exe".to_string());
        }
        let unity_editor_version = unity_editor
            .parent()
            .and_then(Path::parent)
            .and_then(Path::file_name)
            .and_then(|name| name.to_str())
            .filter(|version| version.starts_with(|character: char| character.is_ascii_digit()))
            .ok_or_else(|| "Unity editor version could not be derived".to_string())?
            .to_string();
        let probe_executable = fs::canonicalize(
            env::current_exe().map_err(|_| "probe executable is unavailable".to_string())?,
        )
        .map_err(|_| "probe executable could not be resolved".to_string())?;
        let target_root = workspace_root.join("target");
        fs::create_dir_all(&target_root)
            .map_err(|_| "workspace target directory could not be created".to_string())?;
        let target_root = fs::canonicalize(target_root)
            .map_err(|_| "workspace target directory is unavailable".to_string())?;
        let output_root = target_root.join("r0-probe");
        reset_output_root(&target_root, &output_root)?;
        let output_root = fs::canonicalize(output_root)
            .map_err(|_| "probe output root is unavailable".to_string())?;
        let project_root = output_root.join("project");
        let evidence_root = output_root.join("evidence");
        let execution_object_root = output_root.join("execution-objects");
        let player_root = output_root.join("player");
        for directory in [
            &project_root,
            &evidence_root,
            &execution_object_root,
            &player_root,
        ] {
            fs::create_dir_all(directory)
                .map_err(|_| "probe output directory could not be created".to_string())?;
        }
        Ok(Self {
            workspace_root,
            fixture,
            unity_editor,
            unity_editor_version,
            probe_executable,
            output_root,
            project_root,
            evidence_root,
            execution_object_root,
            player_root,
        })
    }
}

fn reset_output_root(target_root: &Path, output_root: &Path) -> Result<(), String> {
    if output_root.exists() {
        let canonical = fs::canonicalize(output_root)
            .map_err(|_| "existing probe output root is unavailable".to_string())?;
        if canonical.parent() != Some(target_root) || !canonical.join(OUTPUT_MARKER).is_file() {
            return Err("existing probe output root has no valid ownership marker".to_string());
        }
        fs::remove_dir_all(&canonical)
            .map_err(|_| "owned probe output root could not be reset".to_string())?;
    }
    fs::create_dir_all(output_root)
        .map_err(|_| "probe output root could not be created".to_string())?;
    write_text_atomic(&output_root.join(OUTPUT_MARKER), "adm-r0-probe-v1\n")
        .map_err(|_| "probe ownership marker could not be written".to_string())
}

fn duration_ms(duration: Duration) -> u64 {
    duration.as_millis().try_into().unwrap_or(u64::MAX)
}

fn unity_build_source() -> &'static str {
    r#"using System;
using System.IO;
using UnityEditor;
using UnityEditor.Build.Reporting;
using UnityEditor.SceneManagement;
using UnityEngine;
using UnityEngine.SceneManagement;

public static class R0Build
{
    public static void BuildWindows()
    {
        var output = Environment.GetEnvironmentVariable("ADM_R0_OUTPUT_EXE");
        if (string.IsNullOrWhiteSpace(output))
        {
            throw new InvalidOperationException("ADM_R0_OUTPUT_EXE is required.");
        }

        Directory.CreateDirectory(Path.GetDirectoryName(output));
        var scene = EditorSceneManager.NewScene(NewSceneSetup.EmptyScene, NewSceneMode.Single);
        var root = new GameObject("R0ProbeRoot");
        root.AddComponent<R0GeneratedProbe>();
        const string scenePath = "Assets/R0Probe.unity";
        if (!EditorSceneManager.SaveScene(scene, scenePath))
        {
            throw new InvalidOperationException("R0 scene could not be saved.");
        }

        PlayerSettings.companyName = "AutoDesignMaker";
        PlayerSettings.productName = "ADM R0 Probe";
        PlayerSettings.defaultScreenWidth = 960;
        PlayerSettings.defaultScreenHeight = 540;
        PlayerSettings.runInBackground = true;
        EditorBuildSettings.scenes = new[] { new EditorBuildSettingsScene(scenePath, true) };
        var options = new BuildPlayerOptions
        {
            scenes = new[] { scenePath },
            locationPathName = output,
            target = BuildTarget.StandaloneWindows64,
            options = BuildOptions.None
        };
        var report = BuildPipeline.BuildPlayer(options);
        if (report.summary.result != BuildResult.Succeeded)
        {
            throw new InvalidOperationException("R0 player build failed: " + report.summary.result);
        }
        Debug.Log("ADM_R0_BUILD_PASS");
    }
}
"#
}

fn unity_runtime_source() -> &'static str {
    r#"using System;
using UnityEngine;

public sealed class R0GeneratedProbe : MonoBehaviour
{
    private bool smokeMode;
    private int defenders = 1;
    private float threatX = 6.0f;
    private GameObject threat;

    private void Awake()
    {
        foreach (var argument in Environment.GetCommandLineArgs())
        {
            if (argument == "--r0-smoke") smokeMode = true;
        }

        var cameraObject = new GameObject("ProbeCamera");
        cameraObject.tag = "MainCamera";
        var camera = cameraObject.AddComponent<Camera>();
        camera.orthographic = true;
        camera.orthographicSize = 4.5f;
        cameraObject.transform.position = new Vector3(0.0f, 0.0f, -10.0f);

        for (var lane = -1; lane <= 1; lane++)
        {
            CreateToken("Lane" + lane, new Vector3(0.0f, lane * 1.8f, 0.0f),
                new Vector3(14.0f, 1.45f, 0.2f), new Color(0.18f, 0.25f, 0.22f));
        }
        CreateLabel("Title", new Vector3(-6.4f, 3.5f, -0.6f),
            "AutoDesignMaker R0 Technical Probe", 48, 0.065f);
        CreateLabel("Input", new Vector3(-6.4f, 3.0f, -0.6f),
            "Press SPACE to add a defender", 32, 0.055f);
        CreateDefender(-4.5f, 0.0f);
        threat = CreateToken("Threat", new Vector3(threatX, 0.0f, -0.5f),
            new Vector3(0.8f, 0.8f, 0.8f), new Color(0.78f, 0.22f, 0.20f));
    }

    private void Update()
    {
        threatX -= Time.deltaTime * 0.8f;
        threat.transform.position = new Vector3(threatX, 0.0f, -0.5f);
        if (Input.GetKeyDown(KeyCode.Space)) AddDefender();
        if (smokeMode && Time.frameCount >= 10)
        {
            Debug.Log("ADM_R0_SMOKE_PASS");
            Application.Quit(0);
        }
    }

    private void AddDefender()
    {
        defenders++;
        CreateDefender(-4.5f + (defenders - 1) * 0.85f, 0.0f);
    }

    private static void CreateDefender(float x, float y)
    {
        CreateToken("Defender", new Vector3(x, y, -0.5f),
            new Vector3(0.7f, 1.0f, 0.7f), new Color(0.18f, 0.72f, 0.38f));
    }

    private static GameObject CreateToken(string name, Vector3 position, Vector3 scale, Color color)
    {
        var token = GameObject.CreatePrimitive(PrimitiveType.Cube);
        token.name = name;
        token.transform.position = position;
        token.transform.localScale = scale;
        token.GetComponent<Renderer>().material.color = color;
        return token;
    }

    private static void CreateLabel(string name, Vector3 position, string text, int fontSize,
        float characterSize)
    {
        var label = new GameObject(name);
        label.transform.position = position;
        var mesh = label.AddComponent<TextMesh>();
        mesh.text = text;
        mesh.fontSize = fontSize;
        mesh.characterSize = characterSize;
        mesh.color = Color.white;
        mesh.anchor = TextAnchor.MiddleLeft;
    }
}
"#
}

#[derive(Debug)]
struct RunArguments {
    workspace_root: PathBuf,
    fixture: PathBuf,
    unity_editor: PathBuf,
}

#[derive(Debug)]
struct ProbePaths {
    #[allow(dead_code)]
    workspace_root: PathBuf,
    fixture: PathBuf,
    unity_editor: PathBuf,
    unity_editor_version: String,
    probe_executable: PathBuf,
    output_root: PathBuf,
    project_root: PathBuf,
    evidence_root: PathBuf,
    execution_object_root: PathBuf,
    player_root: PathBuf,
}

#[derive(Debug)]
struct FixedInput {
    content_hash: String,
    project_id: String,
}

#[derive(Debug)]
struct PlayerBuild {
    executable: PathBuf,
}

#[derive(Debug)]
struct ProcessOutcome {
    exit_code: Option<i32>,
    timed_out: bool,
}

#[derive(Debug)]
struct PhaseOutput<T> {
    value: T,
    evidence: Value,
    artifacts: Vec<ArtifactEvidence>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
enum FailureKind {
    Input,
    AgentError,
    ScopeViolation,
    Compile,
    Test,
    Timeout,
    Tooling,
    Evidence,
}

#[derive(Debug, Clone)]
struct ProbeError {
    kind: FailureKind,
    phase: String,
    message: String,
}

impl ProbeError {
    fn new(kind: FailureKind, phase: &str, message: &str) -> Self {
        Self {
            kind,
            phase: phase.to_string(),
            message: message.to_string(),
        }
    }

    fn input(phase: &str, message: &str) -> Self {
        Self::new(FailureKind::Input, phase, message)
    }

    fn agent(phase: &str, message: &str) -> Self {
        Self::new(FailureKind::AgentError, phase, message)
    }

    fn scope(phase: &str, message: &str) -> Self {
        Self::new(FailureKind::ScopeViolation, phase, message)
    }

    fn compile(phase: &str, message: &str) -> Self {
        Self::new(FailureKind::Compile, phase, message)
    }

    fn test(phase: &str, message: &str) -> Self {
        Self::new(FailureKind::Test, phase, message)
    }

    fn timeout(phase: &str, message: &str) -> Self {
        Self::new(FailureKind::Timeout, phase, message)
    }

    fn tooling(phase: &str, message: &str) -> Self {
        Self::new(FailureKind::Tooling, phase, message)
    }

    fn evidence(phase: &str, message: &str) -> Self {
        Self::new(FailureKind::Evidence, phase, message)
    }
}

impl fmt::Display for ProbeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}: {}", self.phase, self.message)
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ProbeReport {
    schema_version: u32,
    status: String,
    fixture_content_hash: String,
    fixture_project_id: String,
    unity_editor_version: String,
    deterministic_fingerprint: String,
    phases: Vec<PhaseEvidence>,
    artifacts: Vec<ArtifactEvidence>,
    #[serde(skip_serializing_if = "Option::is_none")]
    failure: Option<ProbeFailure>,
}

impl ProbeReport {
    fn new(unity_editor_version: String) -> Self {
        Self {
            schema_version: REPORT_SCHEMA_VERSION,
            status: "running".to_string(),
            fixture_content_hash: String::new(),
            fixture_project_id: String::new(),
            unity_editor_version,
            deterministic_fingerprint: String::new(),
            phases: Vec::new(),
            artifacts: Vec::new(),
            failure: None,
        }
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct PhaseEvidence {
    id: String,
    status: String,
    duration_ms: u64,
    evidence: Value,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ArtifactEvidence {
    relative_path: String,
    sha256: String,
    bytes: u64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ProbeFailure {
    kind: FailureKind,
    phase: String,
    message: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn marker_values_are_read_without_parsing_provider_output() {
        assert_eq!(
            marker_value("header\nR0_UNITY_VERSION=2022.3.1f1\n", "R0_UNITY_VERSION="),
            Some("2022.3.1f1")
        );
    }

    #[test]
    fn work_unit_failures_have_stable_categories() {
        assert_eq!(
            classify_work_unit_failure("changed files outside the declared output set").kind,
            FailureKind::ScopeViolation
        );
        assert_eq!(
            classify_work_unit_failure("Unity batchmode reported compiler errors").kind,
            FailureKind::Compile
        );
        assert_eq!(
            classify_work_unit_failure("development CLI returned a failed result").kind,
            FailureKind::AgentError
        );
    }

    #[test]
    fn deterministic_fingerprint_ignores_duration_and_artifact_bytes() {
        let mut first = ProbeReport::new("2022.3.1f1".to_string());
        first.status = "passed".to_string();
        first.fixture_content_hash = "a".repeat(64);
        first.fixture_project_id = "probe".to_string();
        first.phases.push(PhaseEvidence {
            id: "phase".to_string(),
            status: "passed".to_string(),
            duration_ms: 1,
            evidence: json!({"marker": true}),
        });
        first.artifacts.push(ArtifactEvidence {
            relative_path: "player.exe".to_string(),
            sha256: "b".repeat(64),
            bytes: 1,
        });
        let mut second = ProbeReport::new("2022.3.1f1".to_string());
        second.status = "passed".to_string();
        second.fixture_content_hash = "a".repeat(64);
        second.fixture_project_id = "probe".to_string();
        second.phases.push(PhaseEvidence {
            id: "phase".to_string(),
            status: "passed".to_string(),
            duration_ms: 999,
            evidence: json!({"marker": true}),
        });
        second.artifacts.push(ArtifactEvidence {
            relative_path: "player.exe".to_string(),
            sha256: "c".repeat(64),
            bytes: 999,
        });

        assert_eq!(
            deterministic_fingerprint(&first).unwrap(),
            deterministic_fingerprint(&second).unwrap()
        );
    }
}
