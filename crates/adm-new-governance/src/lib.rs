#![forbid(unsafe_code)]

use adm_new_contracts::package::{
    PackageManifest, PackageStatus, PackageValidationReport, REQUIRED_INTEGRATION_CHECKS,
};
use adm_new_foundation::source_root::{SOURCE_PROJECT_ID, SourceProjectRoot};
use adm_new_foundation::{
    AdmError, AdmResult, GateReport, hash_text, sha256_hex, unix_timestamp, write_text_atomic,
};
use adm_new_packaging::{
    PACKAGE_DIR, PackageRunResult, PackagingService, PackagingSources, measure_resource_tree,
    verify_portable_resource_root, verify_source_resource_manifest,
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Component, Path, PathBuf};
use std::process::Command;

#[cfg(windows)]
use std::os::windows::fs::MetadataExt;

pub const REQUIRED_PLAN_FILES: &[&str] = &[
    "README.md",
    "00_execution_protocol.md",
    "03_ui_spec.md",
    "07_scorecard_and_optimization.md",
    "python_deconstruction/README.md",
    "python_deconstruction/scorecard.md",
    "newrust_design/README.md",
    "newrust_design/scorecard.md",
    "atomic_backlog/README.md",
    "atomic_backlog/scorecard.md",
];

pub const REQUIRED_SCORECARDS: &[&str] = &[
    "python_deconstruction/scorecard.md",
    "newrust_design/scorecard.md",
    "atomic_backlog/scorecard.md",
];

pub const MIN_ROLE_SCORE: u8 = 90;
pub const MIN_WEIGHTED_SCORE: f32 = 95.0;
pub const EXPECTED_ROLE_COUNT: usize = 7;

pub const REQUIRED_WORKSPACE_MEMBERS: &[&str] = &[
    "apps/adm-new-cli",
    "apps/desktop-tauri",
    "crates/adm-new-foundation",
    "crates/adm-new-contracts",
    "crates/adm-new-governance",
    "crates/adm-new-config",
    "crates/adm-new-storage",
    "crates/adm-new-knowledge",
    "crates/adm-new-design",
    "crates/adm-new-save",
    "crates/adm-new-ai",
    "crates/adm-new-pipeline",
    "crates/adm-new-artifact",
    "crates/adm-new-packaging",
    "crates/adm-new-patch",
    "crates/adm-new-sdk",
    "crates/adm-new-application",
    "crates/adm-new-tauri-commands",
];

pub const REQUIRED_RELEASE_CHECKS: &[&str] = &[
    "cargo_fmt_check",
    "cargo_check_workspace",
    "cargo_test_workspace",
    "web_unit",
    "web_i18n",
    "web_design_content",
    "web_build",
    "web_e2e",
    "web_language_gate",
    "web_ui_gate",
    "web_ui_baseline_gate",
    "package_contract_self_test",
    "resource_manifest",
    "standalone_boundary_gate",
    "portable_build",
    "portable_smoke",
    "portable_integrity",
    "pe_architecture_crt",
    "clean_clone_relocation",
    "anti_fake_scan",
    "generated_cleanup",
];

pub const SOURCE_ROOT_MARKER: &str = ".project_root";
pub const RESOURCE_MANIFEST_PATH: &str = "knowledge/resource-manifest.json";
pub const RELEASE_EVIDENCE_PATH: &str = "gates/standalone-release-evidence.json";
pub const RELEASE_EVIDENCE_SCHEMA_VERSION: u32 = 2;
pub const RELEASE_EVIDENCE_PRODUCER: &str = "tools/verify-standalone.ps1/v2";
pub const RELEASE_PORTABLE_ROOT: &str = "dist/AutoDesignMaker-NEWrust-release";
pub const RELEASE_PORTABLE_OUTPUT_NAME: &str = "AutoDesignMaker-NEWrust-release";
pub const RELEASE_EVIDENCE_MAX_LIFETIME_SECONDS: u64 = 86_400;
pub const RELEASE_EVIDENCE_CLOCK_SKEW_SECONDS: u64 = 300;
pub const LEGACY_PLAN_GATE_DEPRECATED: &str = "legacy_plan_gate_deprecated";
pub const LEGACY_HANDOFF_GATE_DEPRECATED: &str = "legacy_handoff_gate_deprecated";
pub const LEGACY_PYTHON_GATE_DEPRECATED: &str = "legacy_python_gate_deprecated";

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct StandaloneReleaseEvidence {
    schema_version: u32,
    producer: String,
    evidence_id: String,
    project_id: String,
    status: String,
    git_commit: String,
    source_tree_clean: bool,
    generated_at_unix: u64,
    expires_at_unix: u64,
    checks: BTreeMap<String, ReleaseCommandEvidence>,
    portable: Option<ReleasePortableEvidence>,
    errors: Vec<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ReleaseCommandEvidence {
    status: String,
    command: String,
    exit_code: i32,
    duration_ms: u64,
    output_sha256: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ReleasePortableEvidence {
    root: String,
    executable: String,
    executable_sha256: String,
    build_manifest_sha256: String,
    resource_manifest_sha256: String,
    git_commit: String,
    swap_receipt: String,
    swap_receipt_sha256: String,
    transaction_id: String,
    transaction_status: String,
}

pub const REQUIRED_UI_PARITY_V3_RECORD_COUNT: usize = 93;
pub const REQUIRED_UNIT_TEST_MIGRATION_COUNT: usize = 68;
pub const REQUIRED_INTEGRATION_TEST_MIGRATION_COUNT: usize = 5;
pub const REQUIRED_FULL_PROJECT_PYTHON_FILE_COUNT: usize = 379;
pub const REQUIRED_FULL_PROJECT_TEST_MIGRATION_COUNT: usize = 73;
pub const REQUIRED_DATA_ASSET_MIGRATION_COUNT: usize = 727;
pub const REQUIRED_FINAL_HANDOFF_V3_COMPLETED_ATOMS: usize = 40;

pub const REQUIRED_UNIT_TEST_TARGET_DOMAINS: &[&str] = &[
    "adm-new-ai",
    "adm-new-application",
    "adm-new-artifact",
    "adm-new-cli",
    "adm-new-config",
    "adm-new-design",
    "adm-new-foundation",
    "adm-new-patch",
    "adm-new-pipeline",
    "adm-new-save",
    "adm-new-sdk",
    "adm-new-web",
    "gates",
];

pub const REQUIRED_INTEGRATION_TEST_TARGET_DOMAINS: &[&str] = &[
    "Rust/Web test fixtures",
    "adm-new-ai",
    "adm-new-pipeline",
    "adm-new-save",
    "adm-new-design",
    "gates",
];

pub const REQUIRED_FINAL_HANDOFF_V3_GATE_REFS: &[&str] = &[
    "gates/plan-gate.adm",
    "gates/parity-gate.adm",
    "gates/package-gate.adm",
    "gates/release-gate.adm",
    "gates/validation-gate.adm",
    "gates/iteration-gate.adm",
    "gates/ui-gate.adm",
    "gates/ui-shell-gate.adm",
    "gates/ui-workbench-gate.adm",
    "gates/ui-ai-gate.adm",
    "gates/ui-pipeline-gate.adm",
    "gates/ui-utility-gate.adm",
    "gates/ui-settings-style-gate.adm",
    "gates/ui-parity-v3-gate.adm",
    "gates/unit-test-migration-gate.adm",
    "gates/integration-test-migration-gate.adm",
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ValidationMarkerCheck {
    pub id: &'static str,
    pub category: &'static str,
    pub relative_path: &'static str,
    pub marker: &'static str,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct IterationMarkerCheck {
    pub id: &'static str,
    pub category: &'static str,
    pub relative_path: &'static str,
    pub marker: &'static str,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UiShellMarkerCheck {
    pub id: &'static str,
    pub category: &'static str,
    pub relative_path: &'static str,
    pub marker: &'static str,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UiWorkbenchMarkerCheck {
    pub id: &'static str,
    pub category: &'static str,
    pub relative_path: &'static str,
    pub marker: &'static str,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UiAiMarkerCheck {
    pub id: &'static str,
    pub category: &'static str,
    pub relative_path: &'static str,
    pub marker: &'static str,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UiPipelineMarkerCheck {
    pub id: &'static str,
    pub category: &'static str,
    pub relative_path: &'static str,
    pub marker: &'static str,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UiUtilityMarkerCheck {
    pub id: &'static str,
    pub category: &'static str,
    pub relative_path: &'static str,
    pub marker: &'static str,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UiSettingsStyleMarkerCheck {
    pub id: &'static str,
    pub category: &'static str,
    pub relative_path: &'static str,
    pub marker: &'static str,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UnitTestMigrationMarkerCheck {
    pub id: &'static str,
    pub category: &'static str,
    pub relative_path: &'static str,
    pub marker: &'static str,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct IntegrationTestMigrationMarkerCheck {
    pub id: &'static str,
    pub category: &'static str,
    pub relative_path: &'static str,
    pub marker: &'static str,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ParityCheck {
    pub id: &'static str,
    pub layer: &'static str,
    pub relative_path: &'static str,
    pub marker: &'static str,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HandoffManifest {
    pub schema_version: u32,
    pub generated_at: String,
    pub status: String,
    pub plan_root: String,
    pub newrust_root: String,
    pub gate_report_dir: String,
    pub entries: Vec<HandoffEntry>,
    pub blockers: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HandoffEntry {
    pub feature_id: String,
    pub feature_name: String,
    pub python_evidence: Vec<String>,
    pub newrust_files: Vec<String>,
    pub tests: Vec<String>,
    pub gate_refs: Vec<String>,
    pub evidence_level: String,
    pub status: String,
}

pub const REQUIRED_VALIDATION_MARKERS: &[ValidationMarkerCheck] = &[
    ValidationMarkerCheck {
        id: "config_validator",
        category: "application",
        relative_path: "crates/adm-new-application/src/validation_tools.rs",
        marker: "pub fn validate_config_tables",
    },
    ValidationMarkerCheck {
        id: "context_lint",
        category: "application",
        relative_path: "crates/adm-new-application/src/validation_tools.rs",
        marker: "pub fn lint_context_file",
    },
    ValidationMarkerCheck {
        id: "contract_validator",
        category: "application",
        relative_path: "crates/adm-new-application/src/validation_tools.rs",
        marker: "pub fn validate_contract_file_report",
    },
    ValidationMarkerCheck {
        id: "output_validator",
        category: "application",
        relative_path: "crates/adm-new-application/src/validation_tools.rs",
        marker: "pub fn validate_agent_output",
    },
    ValidationMarkerCheck {
        id: "pipeline_quality_metrics",
        category: "application",
        relative_path: "crates/adm-new-application/src/validation_tools.rs",
        marker: "pub fn collect_pipeline_quality_metrics",
    },
    ValidationMarkerCheck {
        id: "pipeline_quality_plan_002",
        category: "application",
        relative_path: "crates/adm-new-application/src/validation_tools.rs",
        marker: "pub fn check_pipeline_plan_002",
    },
    ValidationMarkerCheck {
        id: "design_semantic_quality",
        category: "application",
        relative_path: "crates/adm-new-application/src/validation_tools.rs",
        marker: "pub fn collect_design_semantic_quality",
    },
    ValidationMarkerCheck {
        id: "design_semantic_quality_outputs",
        category: "application",
        relative_path: "crates/adm-new-application/src/validation_tools.rs",
        marker: "pub fn write_design_semantic_quality_outputs",
    },
    ValidationMarkerCheck {
        id: "cli_validate_config",
        category: "cli",
        relative_path: "apps/adm-new-cli/src/main.rs",
        marker: "\"config\" =>",
    },
    ValidationMarkerCheck {
        id: "cli_validate_context",
        category: "cli",
        relative_path: "apps/adm-new-cli/src/main.rs",
        marker: "\"context\" =>",
    },
    ValidationMarkerCheck {
        id: "cli_validate_contract",
        category: "cli",
        relative_path: "apps/adm-new-cli/src/main.rs",
        marker: "\"contract\" =>",
    },
    ValidationMarkerCheck {
        id: "cli_validate_output",
        category: "cli",
        relative_path: "apps/adm-new-cli/src/main.rs",
        marker: "\"output\" =>",
    },
    ValidationMarkerCheck {
        id: "cli_validate_pipeline_quality",
        category: "cli",
        relative_path: "apps/adm-new-cli/src/main.rs",
        marker: "\"pipeline-quality\" =>",
    },
    ValidationMarkerCheck {
        id: "cli_validate_design_semantic_quality",
        category: "cli",
        relative_path: "apps/adm-new-cli/src/main.rs",
        marker: "\"design-semantic-quality\" =>",
    },
    ValidationMarkerCheck {
        id: "cli_validation_a29_test",
        category: "cli-test",
        relative_path: "apps/adm-new-cli/src/main.rs",
        marker: "validation_cli_commands_cover_a29_validators",
    },
];

pub const REQUIRED_ITERATION_MARKERS: &[IterationMarkerCheck] = &[
    IterationMarkerCheck {
        id: "iteration_spec_parser",
        category: "application",
        relative_path: "crates/adm-new-application/src/iteration.rs",
        marker: "pub fn parse_iteration_spec_text",
    },
    IterationMarkerCheck {
        id: "iteration_spec_discovery",
        category: "application",
        relative_path: "crates/adm-new-application/src/iteration.rs",
        marker: "pub fn discover_iteration_specs",
    },
    IterationMarkerCheck {
        id: "delta_scheduler",
        category: "application",
        relative_path: "crates/adm-new-application/src/iteration.rs",
        marker: "pub fn build_delta_execution_plan",
    },
    IterationMarkerCheck {
        id: "artifact_inheritor",
        category: "application",
        relative_path: "crates/adm-new-application/src/iteration.rs",
        marker: "pub fn inherit_skipped_artifacts",
    },
    IterationMarkerCheck {
        id: "iteration_prepare",
        category: "application",
        relative_path: "crates/adm-new-application/src/iteration.rs",
        marker: "pub fn prepare_iteration",
    },
    IterationMarkerCheck {
        id: "iteration_resume_summary",
        category: "application",
        relative_path: "crates/adm-new-application/src/iteration.rs",
        marker: "pub fn summarize_iteration_resume_plan",
    },
    IterationMarkerCheck {
        id: "cli_iteration_command",
        category: "cli",
        relative_path: "apps/adm-new-cli/src/main.rs",
        marker: "\"iteration\" | \"iterate\" => run_iteration_command",
    },
    IterationMarkerCheck {
        id: "cli_iteration_help",
        category: "cli",
        relative_path: "apps/adm-new-cli/src/main.rs",
        marker: "fn print_iteration_help",
    },
    IterationMarkerCheck {
        id: "cli_iteration_a30_test",
        category: "cli-test",
        relative_path: "apps/adm-new-cli/src/main.rs",
        marker: "iteration_cli_commands_cover_a30_iteration_flow",
    },
];

pub const REQUIRED_UI_SHELL_MARKERS: &[UiShellMarkerCheck] = &[
    UiShellMarkerCheck {
        id: "desktop_shell_config",
        category: "desktop-tauri",
        relative_path: "apps/desktop-tauri/src/lib.rs",
        marker: "pub fn default_shell_config",
    },
    UiShellMarkerCheck {
        id: "desktop_smoke_report",
        category: "desktop-tauri",
        relative_path: "apps/desktop-tauri/src/lib.rs",
        marker: "pub fn desktop_smoke_report",
    },
    UiShellMarkerCheck {
        id: "desktop_center_window",
        category: "desktop-tauri",
        relative_path: "apps/desktop-tauri/src/lib.rs",
        marker: "pub fn center_window_position",
    },
    UiShellMarkerCheck {
        id: "tauri_min_width",
        category: "desktop-tauri",
        relative_path: "apps/desktop-tauri/tauri.conf.json",
        marker: "\"minWidth\": 1180",
    },
    UiShellMarkerCheck {
        id: "shell_command_theme_state",
        category: "tauri-command",
        relative_path: "crates/adm-new-tauri-commands/src/shell.rs",
        marker: "pub struct ShellThemeToken",
    },
    UiShellMarkerCheck {
        id: "shell_command_startup_state",
        category: "tauri-command",
        relative_path: "crates/adm-new-tauri-commands/src/shell.rs",
        marker: "pub struct ShellStartupState",
    },
    UiShellMarkerCheck {
        id: "web_theme_module",
        category: "web",
        relative_path: "web/src/theme.js",
        marker: "export const THEME_TOKENS",
    },
    UiShellMarkerCheck {
        id: "web_theme_apply",
        category: "web",
        relative_path: "web/src/theme.js",
        marker: "export function applyThemeTokens",
    },
    UiShellMarkerCheck {
        id: "web_shell_theme_marker",
        category: "web",
        relative_path: "web/src/index.html",
        marker: "data-theme=\"adm-light\"",
    },
    UiShellMarkerCheck {
        id: "web_shell_unit_test",
        category: "web-test",
        relative_path: "web/scripts/test.mjs",
        marker: "minimum desktop width should match Python shell",
    },
];

pub const REQUIRED_UI_WORKBENCH_MARKERS: &[UiWorkbenchMarkerCheck] = &[
    UiWorkbenchMarkerCheck {
        id: "python_workbench_template_viewer",
        category: "python-source",
        relative_path: "core/ui/app_window.py",
        marker: "def open_template_viewer",
    },
    UiWorkbenchMarkerCheck {
        id: "python_workbench_autosave",
        category: "python-source",
        relative_path: "core/ui/app_window.py",
        marker: "def _do_autosave",
    },
    UiWorkbenchMarkerCheck {
        id: "python_workbench_gameplay_systems",
        category: "python-source",
        relative_path: "core/ui/app_window.py",
        marker: "def make_gameplay_systems_panel",
    },
    UiWorkbenchMarkerCheck {
        id: "application_autosave_report",
        category: "application",
        relative_path: "crates/adm-new-application/src/lib.rs",
        marker: "pub struct DesignAutosaveReport",
    },
    UiWorkbenchMarkerCheck {
        id: "application_gameplay_update",
        category: "application",
        relative_path: "crates/adm-new-application/src/lib.rs",
        marker: "pub struct GameplaySystemUpdateRequest",
    },
    UiWorkbenchMarkerCheck {
        id: "application_template_snapshot",
        category: "application",
        relative_path: "crates/adm-new-application/src/lib.rs",
        marker: "pub fn save_template_snapshot",
    },
    UiWorkbenchMarkerCheck {
        id: "tauri_autosave_command",
        category: "tauri-command",
        relative_path: "crates/adm-new-tauri-commands/src/design.rs",
        marker: "pub struct AutosaveDesignRequest",
    },
    UiWorkbenchMarkerCheck {
        id: "tauri_template_command",
        category: "tauri-command",
        relative_path: "crates/adm-new-tauri-commands/src/design.rs",
        marker: "pub struct TemplateSelectionRequest",
    },
    UiWorkbenchMarkerCheck {
        id: "tauri_gameplay_command",
        category: "tauri-command",
        relative_path: "crates/adm-new-tauri-commands/src/design.rs",
        marker: "pub fn update_gameplay_system",
    },
    UiWorkbenchMarkerCheck {
        id: "web_workbench_builders",
        category: "web",
        relative_path: "web/src/features/design.js",
        marker: "export function buildAutosaveDesignRequest",
    },
    UiWorkbenchMarkerCheck {
        id: "web_workbench_gameplay_marker",
        category: "web",
        relative_path: "web/src/index.html",
        marker: "data-role=\"gameplay-systems\"",
    },
    UiWorkbenchMarkerCheck {
        id: "web_workbench_unit_test",
        category: "web-test",
        relative_path: "web/scripts/test.mjs",
        marker: "gameplay weights should summarize",
    },
    UiWorkbenchMarkerCheck {
        id: "cli_ui_workbench_gate",
        category: "cli",
        relative_path: "apps/adm-new-cli/src/main.rs",
        marker: "\"ui-workbench-gate\" => run_gate(\"ui-workbench\", ui_workbench_gate_report)",
    },
];

pub const REQUIRED_UI_AI_MARKERS: &[UiAiMarkerCheck] = &[
    UiAiMarkerCheck {
        id: "python_ai_interview_window",
        category: "python-source",
        relative_path: "core/ui/ai_interview_window.py",
        marker: "class AIInterviewWindow",
    },
    UiAiMarkerCheck {
        id: "python_ai_background_mapping",
        category: "python-source",
        relative_path: "core/ui/ai_interview_window.py",
        marker: "def schedule_background_mapping_if_needed",
    },
    UiAiMarkerCheck {
        id: "python_ai_summary_correction",
        category: "python-source",
        relative_path: "core/ui/ai_interview_window.py",
        marker: "def handle_summary_correction_result",
    },
    UiAiMarkerCheck {
        id: "python_embedded_interview_panel",
        category: "python-source",
        relative_path: "core/ui/embedded_interview.py",
        marker: "class EmbeddedInterviewPanel",
    },
    UiAiMarkerCheck {
        id: "python_bottom_panel_tabs",
        category: "python-source",
        relative_path: "core/ui/bottom_panel.py",
        marker: "class BottomPanel",
    },
    UiAiMarkerCheck {
        id: "tauri_ai_stream_view",
        category: "tauri-command",
        relative_path: "crates/adm-new-tauri-commands/src/ai.rs",
        marker: "pub struct AiStreamEventView",
    },
    UiAiMarkerCheck {
        id: "tauri_ai_background_jobs",
        category: "tauri-command",
        relative_path: "crates/adm-new-tauri-commands/src/ai.rs",
        marker: "pub struct AiBackgroundJobStatus",
    },
    UiAiMarkerCheck {
        id: "web_ai_controller",
        category: "web",
        relative_path: "web/src/features/ai-interview.js",
        marker: "export class AiInterviewController",
    },
    UiAiMarkerCheck {
        id: "web_ai_stream_normalizer",
        category: "web",
        relative_path: "web/src/features/ai-interview.js",
        marker: "export function normalizeAiStreamEvents",
    },
    UiAiMarkerCheck {
        id: "web_ai_stream_marker",
        category: "web",
        relative_path: "web/src/index.html",
        marker: "data-role=\"ai-stream-timeline\"",
    },
    UiAiMarkerCheck {
        id: "web_bottom_ai_tab",
        category: "web",
        relative_path: "web/src/index.html",
        marker: "data-bottom-tab=\"ai\"",
    },
    UiAiMarkerCheck {
        id: "web_ai_unit_test",
        category: "web-test",
        relative_path: "web/scripts/test.mjs",
        marker: "controller should apply command state",
    },
    UiAiMarkerCheck {
        id: "cli_ui_ai_gate",
        category: "cli",
        relative_path: "apps/adm-new-cli/src/main.rs",
        marker: "\"ui-ai-gate\" => run_gate(\"ui-ai\", ui_ai_gate_report)",
    },
];

pub const REQUIRED_UI_PIPELINE_MARKERS: &[UiPipelineMarkerCheck] = &[
    UiPipelineMarkerCheck {
        id: "python_pipeline_panel",
        category: "python-source",
        relative_path: "core/ui/pipeline_panel.py",
        marker: "class PipelinePanel",
    },
    UiPipelineMarkerCheck {
        id: "python_pipeline_run_range",
        category: "python-source",
        relative_path: "core/ui/pipeline_panel.py",
        marker: "def _exec_range",
    },
    UiPipelineMarkerCheck {
        id: "python_pipeline_stop",
        category: "python-source",
        relative_path: "core/ui/pipeline_panel.py",
        marker: "def _stop",
    },
    UiPipelineMarkerCheck {
        id: "python_pipeline_step_card",
        category: "python-source",
        relative_path: "core/ui/pipeline_step_card.py",
        marker: "class StepCard",
    },
    UiPipelineMarkerCheck {
        id: "python_semantic_quality_panel",
        category: "python-source",
        relative_path: "core/ui/semantic_quality_panel.py",
        marker: "def render_semantic_quality_panel",
    },
    UiPipelineMarkerCheck {
        id: "tauri_pipeline_semantic_quality_view",
        category: "tauri-command",
        relative_path: "crates/adm-new-tauri-commands/src/pipeline.rs",
        marker: "pub struct PipelineSemanticQualityView",
    },
    UiPipelineMarkerCheck {
        id: "tauri_pipeline_issue_return_view",
        category: "tauri-command",
        relative_path: "crates/adm-new-tauri-commands/src/pipeline.rs",
        marker: "pub struct PipelineIssueReturnView",
    },
    UiPipelineMarkerCheck {
        id: "web_pipeline_semantic_normalizer",
        category: "web",
        relative_path: "web/src/features/pipeline.js",
        marker: "export function normalizeSemanticQuality",
    },
    UiPipelineMarkerCheck {
        id: "web_pipeline_semantic_panel_marker",
        category: "web",
        relative_path: "web/src/features/pipeline.js",
        marker: "setAttribute(\"data-role\", \"semantic-quality-panel\")",
    },
    UiPipelineMarkerCheck {
        id: "web_pipeline_semantic_css",
        category: "web",
        relative_path: "web/src/styles.css",
        marker: ".semantic-quality-panel",
    },
    UiPipelineMarkerCheck {
        id: "web_pipeline_full_tree_test",
        category: "web-test",
        relative_path: "web/scripts/test.mjs",
        marker: "pipeline stages should normalize full Step00-14 tree",
    },
    UiPipelineMarkerCheck {
        id: "web_pipeline_semantic_return_test",
        category: "web-test",
        relative_path: "web/scripts/test.mjs",
        marker: "semantic return targets should normalize",
    },
    UiPipelineMarkerCheck {
        id: "cli_ui_pipeline_gate",
        category: "cli",
        relative_path: "apps/adm-new-cli/src/main.rs",
        marker: "\"ui-pipeline-gate\" => run_gate(\"ui-pipeline\", ui_pipeline_gate_report)",
    },
];

pub const REQUIRED_UI_UTILITY_MARKERS: &[UiUtilityMarkerCheck] = &[
    UiUtilityMarkerCheck {
        id: "python_patch_panel",
        category: "python-source",
        relative_path: "core/ui/patch_panel.py",
        marker: "class PatchPanel",
    },
    UiUtilityMarkerCheck {
        id: "python_package_panel",
        category: "python-source",
        relative_path: "core/ui/package_panel.py",
        marker: "class PackagePanel",
    },
    UiUtilityMarkerCheck {
        id: "python_sdk_panel",
        category: "python-source",
        relative_path: "core/ui/sdk_panel.py",
        marker: "class SdkPanel",
    },
    UiUtilityMarkerCheck {
        id: "python_save_manager_dialog",
        category: "python-source",
        relative_path: "core/ui/save_manager_dialog.py",
        marker: "class SaveManagerDialog",
    },
    UiUtilityMarkerCheck {
        id: "tauri_save_commands",
        category: "tauri-command",
        relative_path: "crates/adm-new-tauri-commands/src/save.rs",
        marker: "pub trait SaveCommandService",
    },
    UiUtilityMarkerCheck {
        id: "tauri_save_crud",
        category: "tauri-command",
        relative_path: "crates/adm-new-tauri-commands/src/save.rs",
        marker: "fn save_commands_call_real_service_and_serialize_reports",
    },
    UiUtilityMarkerCheck {
        id: "web_save_index_normalizer",
        category: "web",
        relative_path: "web/src/features/utility-panels.js",
        marker: "export function normalizeSaveIndex",
    },
    UiUtilityMarkerCheck {
        id: "web_project_state_converter",
        category: "web",
        relative_path: "web/src/features/utility-panels.js",
        marker: "export function buildProjectStateFromDesignView",
    },
    UiUtilityMarkerCheck {
        id: "web_save_dialog_renderer",
        category: "web",
        relative_path: "web/src/features/utility-panels.js",
        marker: "export function renderSaveManagerDialog",
    },
    UiUtilityMarkerCheck {
        id: "web_save_manager_html",
        category: "web",
        relative_path: "web/src/index.html",
        marker: "data-role=\"save-manager-dialog\"",
    },
    UiUtilityMarkerCheck {
        id: "web_save_table_html",
        category: "web",
        relative_path: "web/src/index.html",
        marker: "data-role=\"save-table\"",
    },
    UiUtilityMarkerCheck {
        id: "web_save_manager_css",
        category: "web",
        relative_path: "web/src/styles.css",
        marker: ".save-manager-dialog",
    },
    UiUtilityMarkerCheck {
        id: "web_save_conversion_test",
        category: "web-test",
        relative_path: "web/scripts/test.mjs",
        marker: "checklist should map by item id",
    },
    UiUtilityMarkerCheck {
        id: "web_save_manager_screenshot_gate",
        category: "web-test",
        relative_path: "web/scripts/ui-gate.mjs",
        marker: "save_manager",
    },
    UiUtilityMarkerCheck {
        id: "web_save_fixture",
        category: "web-test",
        relative_path: "web/scripts/fixtures.mjs",
        marker: "export function sampleSaveIndex",
    },
    UiUtilityMarkerCheck {
        id: "cli_ui_utility_gate",
        category: "cli",
        relative_path: "apps/adm-new-cli/src/main.rs",
        marker: "\"ui-utility-gate\" => run_gate(\"ui-utility\", ui_utility_gate_report)",
    },
];

pub const REQUIRED_UI_SETTINGS_STYLE_MARKERS: &[UiSettingsStyleMarkerCheck] = &[
    UiSettingsStyleMarkerCheck {
        id: "python_ai_config_unified_dialog",
        category: "python-source",
        relative_path: "core/ui/ai_config_unified_dialog.py",
        marker: "class AIConfigUnifiedDialog",
    },
    UiSettingsStyleMarkerCheck {
        id: "python_project_config_dialog",
        category: "python-source",
        relative_path: "core/ui/unity_config_dialog.py",
        marker: "class ProjectConfigDialog",
    },
    UiSettingsStyleMarkerCheck {
        id: "python_style_confirmation_dialog",
        category: "python-source",
        relative_path: "core/ui/style_confirmation_dialog.py",
        marker: "class StyleConfirmationDialog",
    },
    UiSettingsStyleMarkerCheck {
        id: "python_style_prompt_editor",
        category: "python-source",
        relative_path: "core/ui/style_prompt_editor.py",
        marker: "class StylePromptEditorDialog",
    },
    UiSettingsStyleMarkerCheck {
        id: "tauri_project_config_request",
        category: "tauri-command",
        relative_path: "crates/adm-new-tauri-commands/src/config.rs",
        marker: "pub struct SaveProjectConfigRequest",
    },
    UiSettingsStyleMarkerCheck {
        id: "tauri_project_preflight_command",
        category: "tauri-command",
        relative_path: "crates/adm-new-tauri-commands/src/config.rs",
        marker: "pub fn run_project_preflight",
    },
    UiSettingsStyleMarkerCheck {
        id: "web_settings_style_module",
        category: "web",
        relative_path: "web/src/features/settings-style.js",
        marker: "export function buildProjectConfigSaveRequest",
    },
    UiSettingsStyleMarkerCheck {
        id: "web_style_prompt_parser",
        category: "web",
        relative_path: "web/src/features/settings-style.js",
        marker: "export function parseStylePromptResponse",
    },
    UiSettingsStyleMarkerCheck {
        id: "web_style_prompt_override",
        category: "web",
        relative_path: "web/src/features/settings-style.js",
        marker: "export function buildStylePromptOverrideRequest",
    },
    UiSettingsStyleMarkerCheck {
        id: "web_pipeline_prompt_editor_action",
        category: "web",
        relative_path: "web/src/features/pipeline.js",
        marker: "open-style-prompt-editor",
    },
    UiSettingsStyleMarkerCheck {
        id: "web_project_config_html",
        category: "web",
        relative_path: "web/src/index.html",
        marker: "data-role=\"project-config-modal\"",
    },
    UiSettingsStyleMarkerCheck {
        id: "web_style_prompt_html",
        category: "web",
        relative_path: "web/src/index.html",
        marker: "data-role=\"style-prompt-editor-modal\"",
    },
    UiSettingsStyleMarkerCheck {
        id: "web_project_config_css",
        category: "web",
        relative_path: "web/src/styles.css",
        marker: ".project-config-dialog",
    },
    UiSettingsStyleMarkerCheck {
        id: "web_style_prompt_css",
        category: "web",
        relative_path: "web/src/styles.css",
        marker: ".style-prompt-dialog",
    },
    UiSettingsStyleMarkerCheck {
        id: "web_settings_style_unit_test",
        category: "web-test",
        relative_path: "web/scripts/test.mjs",
        marker: "style prompt parser should honor valid ids",
    },
    UiSettingsStyleMarkerCheck {
        id: "web_settings_style_e2e",
        category: "web-test",
        relative_path: "web/scripts/e2e.mjs",
        marker: "web settings-style e2e passed",
    },
    UiSettingsStyleMarkerCheck {
        id: "web_project_config_screenshot_gate",
        category: "web-test",
        relative_path: "web/scripts/ui-gate.mjs",
        marker: "project_config",
    },
    UiSettingsStyleMarkerCheck {
        id: "web_style_prompt_screenshot_gate",
        category: "web-test",
        relative_path: "web/scripts/ui-gate.mjs",
        marker: "style_prompt_editor",
    },
    UiSettingsStyleMarkerCheck {
        id: "cli_ui_settings_style_gate",
        category: "cli",
        relative_path: "apps/adm-new-cli/src/main.rs",
        marker: "\"ui-settings-style-gate\" => run_gate(\"ui-settings-style\", ui_settings_style_gate_report)",
    },
];

pub const REQUIRED_UNIT_TEST_MIGRATION_MARKERS: &[UnitTestMigrationMarkerCheck] = &[
    UnitTestMigrationMarkerCheck {
        id: "config_defaults",
        category: "adm-new-config",
        relative_path: "crates/adm-new-config/src/lib.rs",
        marker: "default_ai_config_has_three_categories_and_compat_profiles",
    },
    UnitTestMigrationMarkerCheck {
        id: "config_loader",
        category: "adm-new-config",
        relative_path: "crates/adm-new-config/src/lib.rs",
        marker: "app_config_loader_deep_merges_toml_and_project_settings",
    },
    UnitTestMigrationMarkerCheck {
        id: "config_validator",
        category: "adm-new-config",
        relative_path: "crates/adm-new-config/src/lib.rs",
        marker: "validation_checks_only_active_credentials_and_duplicate_ids",
    },
    UnitTestMigrationMarkerCheck {
        id: "foundation_paths",
        category: "adm-new-foundation",
        relative_path: "crates/adm-new-foundation/src/paths.rs",
        marker: "project_paths_match_python_draft_policy",
    },
    UnitTestMigrationMarkerCheck {
        id: "foundation_pycache",
        category: "adm-new-foundation",
        relative_path: "crates/adm-new-foundation/src/paths.rs",
        marker: "pycache_prefix_uses_project_cache_by_default",
    },
    UnitTestMigrationMarkerCheck {
        id: "foundation_structured_md",
        category: "adm-new-foundation",
        relative_path: "crates/adm-new-foundation/src/structured_md.rs",
        marker: "loads_data_accepts_json_fence_yaml_fence_and_raw_json",
    },
    UnitTestMigrationMarkerCheck {
        id: "ai_completion_service",
        category: "adm-new-ai",
        relative_path: "crates/adm-new-ai/src/lib.rs",
        marker: "completion_service_accepts_pure_fenced_and_embedded_json_with_retry_hint",
    },
    UnitTestMigrationMarkerCheck {
        id: "ai_schema_contracts",
        category: "adm-new-ai",
        relative_path: "crates/adm-new-ai/src/design_contracts.rs",
        marker: "schema_shape_validator_checks_required_fields_mode_and_version",
    },
    UnitTestMigrationMarkerCheck {
        id: "ai_model_adapters",
        category: "adm-new-ai",
        relative_path: "crates/adm-new-ai/src/adapters.rs",
        marker: "adapter_registry_matches_python_names",
    },
    UnitTestMigrationMarkerCheck {
        id: "ai_openai_request",
        category: "adm-new-ai",
        relative_path: "crates/adm-new-ai/src/adapters.rs",
        marker: "openai_request_normalizes_endpoint_and_appends_input_files",
    },
    UnitTestMigrationMarkerCheck {
        id: "ai_codex_image",
        category: "adm-new-ai",
        relative_path: "crates/adm-new-ai/src/image.rs",
        marker: "codex_image_command_and_output_parsers_follow_python_shape",
    },
    UnitTestMigrationMarkerCheck {
        id: "contracts_schema_registry",
        category: "adm-new-contracts",
        relative_path: "crates/adm-new-contracts/src/schema.rs",
        marker: "schema_registry_discovers_all_project_schema_files",
    },
    UnitTestMigrationMarkerCheck {
        id: "contracts_project_state",
        category: "adm-new-contracts",
        relative_path: "crates/adm-new-contracts/src/project.rs",
        marker: "project_state_serde_roundtrip_preserves_option_provenance_and_l4_l5",
    },
    UnitTestMigrationMarkerCheck {
        id: "design_engine_view",
        category: "adm-new-design",
        relative_path: "crates/adm-new-design/src/lib.rs",
        marker: "design_view_model_reports_coverage_l4_l5_quality_palette",
    },
    UnitTestMigrationMarkerCheck {
        id: "design_data_loader",
        category: "adm-new-design",
        relative_path: "crates/adm-new-design/src/data_loader/mod.rs",
        marker: "data_asset_inventory_v3_counts_real_design_data",
    },
    UnitTestMigrationMarkerCheck {
        id: "design_template_registry",
        category: "adm-new-design",
        relative_path: "crates/adm-new-design/src/data_loader/mod.rs",
        marker: "template_registry_v3_skips_index_and_archives",
    },
    UnitTestMigrationMarkerCheck {
        id: "design_identity_dna_open_questions",
        category: "adm-new-design",
        relative_path: "crates/adm-new-design/src/contracts/mod.rs",
        marker: "project_dna_freeze_blocks_without_demo_flow_and_passes_with_bundle",
    },
    UnitTestMigrationMarkerCheck {
        id: "design_playable_contracts",
        category: "adm-new-design",
        relative_path: "crates/adm-new-design/src/contracts/mod.rs",
        marker: "playable_bundle_validator_and_tasks_cover_runtime_surface",
    },
    UnitTestMigrationMarkerCheck {
        id: "design_semantic_archetypes",
        category: "adm-new-design",
        relative_path: "crates/adm-new-design/src/semantic_pipeline.rs",
        marker: "archetype_detection_and_requirements_match_python_tests",
    },
    UnitTestMigrationMarkerCheck {
        id: "design_semantic_program_art",
        category: "adm-new-design",
        relative_path: "crates/adm-new-design/src/semantic_pipeline.rs",
        marker: "program_and_art_semantic_contracts_cover_gate_codes",
    },
    UnitTestMigrationMarkerCheck {
        id: "design_semantic_alignment_style_tasks",
        category: "adm-new-design",
        relative_path: "crates/adm-new-design/src/semantic_pipeline.rs",
        marker: "alignment_style_and_task_semanticizers_match_python_tests",
    },
    UnitTestMigrationMarkerCheck {
        id: "design_art_pipeline",
        category: "adm-new-design",
        relative_path: "crates/adm-new-design/src/art_pipeline/mod.rs",
        marker: "stage13_materialization_reports_and_stage14_acceptance_are_schema_valid",
    },
    UnitTestMigrationMarkerCheck {
        id: "design_structured_context",
        category: "adm-new-design",
        relative_path: "crates/adm-new-design/src/handoff.rs",
        marker: "structured_context_prefers_stage2_and_falls_back_to_d4_candidate",
    },
    UnitTestMigrationMarkerCheck {
        id: "design_structured_handoff",
        category: "adm-new-design",
        relative_path: "crates/adm-new-design/src/handoff.rs",
        marker: "structured_handoff_writes_manifest_decisions_and_candidates",
    },
    UnitTestMigrationMarkerCheck {
        id: "pipeline_registry",
        category: "adm-new-pipeline",
        relative_path: "crates/adm-new-pipeline/src/lib.rs",
        marker: "pipeline_run_range_executes_registry_order_and_records_state",
    },
    UnitTestMigrationMarkerCheck {
        id: "pipeline_source_reference_manifest",
        category: "adm-new-pipeline",
        relative_path: "crates/adm-new-pipeline/src/source.rs",
        marker: "import_step_copies_sources_writes_reports_and_reference_manifest",
    },
    UnitTestMigrationMarkerCheck {
        id: "pipeline_generation",
        category: "adm-new-pipeline",
        relative_path: "crates/adm-new-pipeline/src/generation.rs",
        marker: "apply_development_plan_outputs_writes_contract_report_and_indexes",
    },
    UnitTestMigrationMarkerCheck {
        id: "pipeline_design_flow",
        category: "adm-new-pipeline",
        relative_path: "crates/adm-new-pipeline/src/design_flow.rs",
        marker: "d3_blocks_empty_project_but_test_mode_keeps_plugin_success",
    },
    UnitTestMigrationMarkerCheck {
        id: "pipeline_step00_02",
        category: "adm-new-pipeline",
        relative_path: "crates/adm-new-pipeline/src/stages/step00_02.rs",
        marker: "step02_extracts_l5_entities_and_supplements_missing_nodes",
    },
    UnitTestMigrationMarkerCheck {
        id: "pipeline_step03_06",
        category: "adm-new-pipeline",
        relative_path: "crates/adm-new-pipeline/src/stages/step03_06.rs",
        marker: "step03_converts_entities_binds_systems_and_builds_schema_valid_contract",
    },
    UnitTestMigrationMarkerCheck {
        id: "pipeline_step03_06_reviewer",
        category: "adm-new-pipeline",
        relative_path: "crates/adm-new-pipeline/src/stages/step03_06.rs",
        marker: "reviewer_reports_verdict_and_stable_issue_codes",
    },
    UnitTestMigrationMarkerCheck {
        id: "pipeline_step07",
        category: "adm-new-pipeline",
        relative_path: "crates/adm-new-pipeline/src/stages/step07.rs",
        marker: "prompt_override_is_consumed_and_limits_generated_options",
    },
    UnitTestMigrationMarkerCheck {
        id: "pipeline_step08_14",
        category: "adm-new-pipeline",
        relative_path: "crates/adm-new-pipeline/src/stages/step08_14.rs",
        marker: "step08_to_step14_registry_artifacts_match_declared_schemas",
    },
    UnitTestMigrationMarkerCheck {
        id: "pipeline_step13",
        category: "adm-new-pipeline",
        relative_path: "crates/adm-new-pipeline/src/stages/step08_14.rs",
        marker: "step13_and_step14_accept_correlated_unity_execution_evidence",
    },
    UnitTestMigrationMarkerCheck {
        id: "pipeline_step14",
        category: "adm-new-pipeline",
        relative_path: "crates/adm-new-pipeline/src/stages/step08_14.rs",
        marker: "step14_rejects_stage13_without_verified_unity_evidence",
    },
    UnitTestMigrationMarkerCheck {
        id: "artifact_registry",
        category: "adm-new-artifact",
        relative_path: "crates/adm-new-artifact/src/lib.rs",
        marker: "artifact_dependency_graph_uses_registry_and_detects_errors",
    },
    UnitTestMigrationMarkerCheck {
        id: "artifact_validation_paths",
        category: "adm-new-artifact",
        relative_path: "crates/adm-new-artifact/src/lib.rs",
        marker: "artifact_validation_fails_when_review_failed_or_schema_target_missing",
    },
    UnitTestMigrationMarkerCheck {
        id: "save_manager",
        category: "adm-new-save",
        relative_path: "crates/adm-new-save/src/lib.rs",
        marker: "save_service_autosaves_and_creates_formal_archive_snapshot",
    },
    UnitTestMigrationMarkerCheck {
        id: "save_parallel_isolation",
        category: "adm-new-save",
        relative_path: "crates/adm-new-save/src/lib.rs",
        marker: "parallel_isolation_audit_and_repair_draft_meta_mismatch",
    },
    UnitTestMigrationMarkerCheck {
        id: "application_runtime",
        category: "adm-new-application",
        relative_path: "crates/adm-new-application/src/runtime.rs",
        marker: "execution_config_planner_and_state_match_python_shapes",
    },
    UnitTestMigrationMarkerCheck {
        id: "application_parallel_runtime",
        category: "adm-new-application",
        relative_path: "crates/adm-new-application/src/runtime.rs",
        marker: "preflight_run_context_identity_and_pipeline_state_round_trip",
    },
    UnitTestMigrationMarkerCheck {
        id: "application_execution_objects",
        category: "adm-new-application",
        relative_path: "crates/adm-new-application/src/execution_objects.rs",
        marker: "design_project_runs_full_execution_object_gate_and_persists",
    },
    UnitTestMigrationMarkerCheck {
        id: "application_unattended_recovery",
        category: "adm-new-application",
        relative_path: "crates/adm-new-application/src/execution_objects.rs",
        marker: "correction_queue_and_unattended_recovery_route_failures",
    },
    UnitTestMigrationMarkerCheck {
        id: "application_iteration",
        category: "adm-new-application",
        relative_path: "crates/adm-new-application/src/iteration.rs",
        marker: "prepare_iteration_creates_iteration_save_and_inherits_skipped_artifacts",
    },
    UnitTestMigrationMarkerCheck {
        id: "application_semantic_quality",
        category: "adm-new-application",
        relative_path: "crates/adm-new-application/src/validation_tools.rs",
        marker: "design_semantic_quality_detects_cross_context_and_generic_tasks",
    },
    UnitTestMigrationMarkerCheck {
        id: "application_structured_logging",
        category: "adm-new-application",
        relative_path: "crates/adm-new-application/src/lib.rs",
        marker: "logs_application_service_filters_latest_clears_and_exports_jsonl",
    },
    UnitTestMigrationMarkerCheck {
        id: "patch_channel",
        category: "adm-new-patch",
        relative_path: "crates/adm-new-patch/src/lib.rs",
        marker: "patch_executor_apply_validate_and_promote_updates_store",
    },
    UnitTestMigrationMarkerCheck {
        id: "sdk_knowledge_base",
        category: "adm-new-sdk",
        relative_path: "crates/adm-new-sdk/src/knowledge_base.rs",
        marker: "sdk_file_store_add_review_and_context_matches_python_contract",
    },
    UnitTestMigrationMarkerCheck {
        id: "sdk_ai_extractor",
        category: "adm-new-sdk",
        relative_path: "crates/adm-new-sdk/src/ai_extractor.rs",
        marker: "adapter_extraction_uses_read_only_json_contract_prompt",
    },
    UnitTestMigrationMarkerCheck {
        id: "cli_iteration",
        category: "adm-new-cli",
        relative_path: "apps/adm-new-cli/src/main.rs",
        marker: "iteration_cli_commands_cover_a30_iteration_flow",
    },
    UnitTestMigrationMarkerCheck {
        id: "cli_patch",
        category: "adm-new-cli",
        relative_path: "apps/adm-new-cli/src/main.rs",
        marker: "patch_cli_promote_marks_record",
    },
    UnitTestMigrationMarkerCheck {
        id: "cli_validation",
        category: "adm-new-cli",
        relative_path: "apps/adm-new-cli/src/main.rs",
        marker: "validation_cli_commands_cover_a29_validators",
    },
    UnitTestMigrationMarkerCheck {
        id: "web_design_unit",
        category: "adm-new-web",
        relative_path: "web/scripts/test.mjs",
        marker: "default route should be design",
    },
    UnitTestMigrationMarkerCheck {
        id: "web_pipeline_semantic",
        category: "adm-new-web",
        relative_path: "web/scripts/test.mjs",
        marker: "semantic return fallback should resolve",
    },
    UnitTestMigrationMarkerCheck {
        id: "web_settings_style_unit",
        category: "adm-new-web",
        relative_path: "web/scripts/test.mjs",
        marker: "pipeline style option should normalize to style id",
    },
    UnitTestMigrationMarkerCheck {
        id: "gate_pytest_environment",
        category: "gates",
        relative_path: "crates/adm-new-governance/src/lib.rs",
        marker: "unit_test_migration_gate_report",
    },
];

pub const REQUIRED_INTEGRATION_TEST_MIGRATION_MARKERS: &[IntegrationTestMigrationMarkerCheck] = &[
    IntegrationTestMigrationMarkerCheck {
        id: "python_conftest_ai_config_fixture",
        category: "python-source",
        relative_path: "core/tests/conftest.py",
        marker: "def isolate_local_ai_config",
    },
    IntegrationTestMigrationMarkerCheck {
        id: "python_conftest_template_cache_fixture",
        category: "python-source",
        relative_path: "core/tests/conftest.py",
        marker: "def clear_step01_template_cache",
    },
    IntegrationTestMigrationMarkerCheck {
        id: "rust_config_temp_roots",
        category: "adm-new-config",
        relative_path: "crates/adm-new-config/src/lib.rs",
        marker: "fn temp_root(prefix: &str) -> PathBuf",
    },
    IntegrationTestMigrationMarkerCheck {
        id: "ai_config_active_profile",
        category: "adm-new-ai",
        relative_path: "crates/adm-new-ai/src/lib.rs",
        marker: "ai_config_service_saves_normalized_v3_config_and_active_profile",
    },
    IntegrationTestMigrationMarkerCheck {
        id: "ai_adapter_registry",
        category: "adm-new-ai",
        relative_path: "crates/adm-new-ai/src/adapters.rs",
        marker: "adapter_registry_matches_python_names",
    },
    IntegrationTestMigrationMarkerCheck {
        id: "ai_openai_request",
        category: "adm-new-ai",
        relative_path: "crates/adm-new-ai/src/adapters.rs",
        marker: "openai_request_normalizes_endpoint_and_appends_input_files",
    },
    IntegrationTestMigrationMarkerCheck {
        id: "pipeline_registry_order",
        category: "adm-new-pipeline",
        relative_path: "crates/adm-new-pipeline/src/lib.rs",
        marker: "pipeline_run_range_executes_registry_order_and_records_state",
    },
    IntegrationTestMigrationMarkerCheck {
        id: "pipeline_design_flow_registry",
        category: "adm-new-pipeline",
        relative_path: "crates/adm-new-pipeline/src/design_flow.rs",
        marker: "plugin_specs_and_stage_specs_match_python_registry",
    },
    IntegrationTestMigrationMarkerCheck {
        id: "pipeline_design_flow_d4_handoff",
        category: "adm-new-pipeline",
        relative_path: "crates/adm-new-pipeline/src/design_flow.rs",
        marker: "d4_exports_concept_package_and_propagates_handoff_blockers",
    },
    IntegrationTestMigrationMarkerCheck {
        id: "pipeline_step00_02_generators",
        category: "adm-new-pipeline",
        relative_path: "crates/adm-new-pipeline/src/stages/step00_02.rs",
        marker: "stage_generators_write_step00_02_outputs",
    },
    IntegrationTestMigrationMarkerCheck {
        id: "pipeline_step03_06_generators",
        category: "adm-new-pipeline",
        relative_path: "crates/adm-new-pipeline/src/stages/step03_06.rs",
        marker: "stage_generators_write_step03_06_outputs",
    },
    IntegrationTestMigrationMarkerCheck {
        id: "pipeline_step08_14_semantic_chain",
        category: "adm-new-pipeline",
        relative_path: "crates/adm-new-pipeline/src/stages/step08_14.rs",
        marker: "step08_to_step14_registry_artifacts_match_declared_schemas",
    },
    IntegrationTestMigrationMarkerCheck {
        id: "application_pipeline_quality_gate",
        category: "adm-new-application",
        relative_path: "crates/adm-new-application/src/validation_tools.rs",
        marker: "pipeline_quality_plan_002_check_uses_stage_metrics",
    },
    IntegrationTestMigrationMarkerCheck {
        id: "application_design_semantic_quality_gate",
        category: "adm-new-application",
        relative_path: "crates/adm-new-application/src/validation_tools.rs",
        marker: "design_semantic_quality_detects_cross_context_and_generic_tasks",
    },
    IntegrationTestMigrationMarkerCheck {
        id: "save_parallel_draft_meta_isolation",
        category: "adm-new-save",
        relative_path: "crates/adm-new-save/src/lib.rs",
        marker: "parallel_isolation_audit_and_repair_draft_meta_mismatch",
    },
    IntegrationTestMigrationMarkerCheck {
        id: "save_parallel_artifact_isolation",
        category: "adm-new-save",
        relative_path: "crates/adm-new-save/src/lib.rs",
        marker: "parallel_isolation_audit_detects_save_artifact_mismatch",
    },
    IntegrationTestMigrationMarkerCheck {
        id: "design_view_model_quality",
        category: "adm-new-design",
        relative_path: "crates/adm-new-design/src/lib.rs",
        marker: "design_view_model_reports_coverage_l4_l5_quality_palette",
    },
    IntegrationTestMigrationMarkerCheck {
        id: "gate_integration_test_migration",
        category: "gates",
        relative_path: "crates/adm-new-governance/src/lib.rs",
        marker: "integration_test_migration_gate_report",
    },
];

pub const REQUIRED_PARITY_CHECKS: &[ParityCheck] = &[
    ParityCheck {
        id: "contracts_project_state",
        layer: "contract",
        relative_path: "crates/adm-new-contracts/src/project.rs",
        marker: "project_state_serde_roundtrip_preserves_option_provenance_and_l4_l5",
    },
    ParityCheck {
        id: "contracts_save_state",
        layer: "contract",
        relative_path: "crates/adm-new-contracts/src/save.rs",
        marker: "save_index_roundtrip_preserves_current_and_progress",
    },
    ParityCheck {
        id: "contracts_pipeline_state",
        layer: "contract",
        relative_path: "crates/adm-new-contracts/src/pipeline.rs",
        marker: "pipeline_registry_and_run_state_roundtrip",
    },
    ParityCheck {
        id: "contracts_ai_config",
        layer: "contract",
        relative_path: "crates/adm-new-contracts/src/ai.rs",
        marker: "ai_config_v3_roundtrip_preserves_categories_and_active_profile",
    },
    ParityCheck {
        id: "contracts_package",
        layer: "contract",
        relative_path: "crates/adm-new-contracts/src/package.rs",
        marker: "package_validation_report_roundtrip_preserves_required_checks",
    },
    ParityCheck {
        id: "storage_repository_roundtrip",
        layer: "storage",
        relative_path: "crates/adm-new-storage/src/lib.rs",
        marker: "storage_typed_json_repository_reads_missing_and_roundtrips",
    },
    ParityCheck {
        id: "domain_design_view_model",
        layer: "domain",
        relative_path: "crates/adm-new-design/src/lib.rs",
        marker: "design_view_model_reports_coverage_l4_l5_quality_palette",
    },
    ParityCheck {
        id: "domain_ai_writeback",
        layer: "domain",
        relative_path: "crates/adm-new-ai/src/lib.rs",
        marker: "interview_high_confidence_full_output_writes_and_archives_state",
    },
    ParityCheck {
        id: "domain_pipeline_runtime",
        layer: "domain",
        relative_path: "crates/adm-new-pipeline/src/lib.rs",
        marker: "pipeline_run_range_executes_registry_order_and_records_state",
    },
    ParityCheck {
        id: "domain_artifact_blockers",
        layer: "domain",
        relative_path: "crates/adm-new-artifact/src/lib.rs",
        marker: "artifact_preflight_blocks_missing_schema_refs_unknown_reviewer_and_upstream_failure",
    },
    ParityCheck {
        id: "domain_package_blockers",
        layer: "domain",
        relative_path: "crates/adm-new-packaging/src/lib.rs",
        marker: "package_missing_changed_files_blocks_even_when_step14_succeeded",
    },
    ParityCheck {
        id: "domain_patch_validation",
        layer: "domain",
        relative_path: "crates/adm-new-patch/src/lib.rs",
        marker: "patch_service_rejects_empty_request_and_lists_by_updated_at_desc",
    },
    ParityCheck {
        id: "domain_sdk_review",
        layer: "domain",
        relative_path: "crates/adm-new-sdk/src/lib.rs",
        marker: "sdk_service_add_placeholder_and_review_status",
    },
    ParityCheck {
        id: "application_save",
        layer: "application",
        relative_path: "crates/adm-new-application/src/lib.rs",
        marker: "save_application_service_delegates_create_and_load",
    },
    ParityCheck {
        id: "application_logs",
        layer: "application",
        relative_path: "crates/adm-new-application/src/lib.rs",
        marker: "logs_application_service_filters_latest_clears_and_exports_jsonl",
    },
    ParityCheck {
        id: "commands_error_mapping",
        layer: "command",
        relative_path: "crates/adm-new-tauri-commands/src/lib.rs",
        marker: "adm_error_maps_to_command_error_without_service_logic",
    },
    ParityCheck {
        id: "commands_package_blocked",
        layer: "command",
        relative_path: "crates/adm-new-tauri-commands/src/package.rs",
        marker: "package_command_blocks_without_skipping_validation",
    },
    ParityCheck {
        id: "commands_config_no_key_exposure",
        layer: "command",
        relative_path: "crates/adm-new-tauri-commands/src/config.rs",
        marker: "config_commands_load_save_and_validate_without_exposing_keys",
    },
];

#[derive(Debug, Clone, PartialEq)]
pub struct Score {
    pub area: String,
    pub value: u8,
    pub weight: u8,
    pub confidence: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ScorecardEvaluation {
    pub relative_path: String,
    pub status_line: String,
    pub scores: Vec<Score>,
    pub weighted_score: f32,
    pub declared_weighted_score: Option<f32>,
}

pub fn all_scores_meet_threshold(scores: &[Score], threshold: u8) -> bool {
    scores.iter().all(|score| score.value >= threshold)
}

pub fn weighted_score(scores: &[Score]) -> AdmResult<f32> {
    let total_weight: u32 = scores.iter().map(|score| u32::from(score.weight)).sum();
    if total_weight == 0 {
        return Err(AdmError::new("score weights must not sum to zero"));
    }
    let weighted_sum: u32 = scores
        .iter()
        .map(|score| u32::from(score.value) * u32::from(score.weight))
        .sum();
    Ok(weighted_sum as f32 / total_weight as f32)
}

pub fn evaluate_scorecard(relative_path: &str, text: &str) -> AdmResult<ScorecardEvaluation> {
    let status_line = text
        .lines()
        .find(|line| line.trim_start().starts_with("状态："))
        .map(|line| line.trim().to_string())
        .unwrap_or_default();
    let mut rows = parse_score_rows(text);
    if rows.len() < EXPECTED_ROLE_COUNT {
        return Err(AdmError::new(format!(
            "{relative_path} contains fewer than {EXPECTED_ROLE_COUNT} score rows"
        )));
    }
    rows = rows.split_off(rows.len() - EXPECTED_ROLE_COUNT);
    let weighted = weighted_score(&rows)?;
    Ok(ScorecardEvaluation {
        relative_path: relative_path.to_string(),
        status_line,
        scores: rows,
        weighted_score: weighted,
        declared_weighted_score: parse_last_weighted_score(text),
    })
}

fn parse_score_rows(text: &str) -> Vec<Score> {
    text.lines()
        .filter_map(|line| {
            let trimmed = line.trim();
            if !trimmed.starts_with('|') || trimmed.contains("---") {
                return None;
            }
            let columns: Vec<&str> = trimmed
                .trim_matches('|')
                .split('|')
                .map(str::trim)
                .collect();
            if columns.len() < 5 || columns[0] == "角色" {
                return None;
            }
            let value = columns[2].parse::<u8>().ok()?;
            let weight = columns[3].parse::<u8>().ok()?;
            Some(Score {
                area: columns[0].to_string(),
                value,
                weight,
                confidence: columns[4].to_string(),
            })
        })
        .collect()
}

fn parse_last_weighted_score(text: &str) -> Option<f32> {
    let mut found = None;
    for line in text.lines() {
        if !line.contains("加权综合分") {
            continue;
        }
        found = first_number(line);
    }
    found
}

fn first_number(line: &str) -> Option<f32> {
    let mut buf = String::new();
    let mut started = false;
    for ch in line.chars() {
        if ch.is_ascii_digit() || (ch == '.' && started) {
            started = true;
            buf.push(ch);
        } else if started {
            break;
        }
    }
    if buf.is_empty() {
        None
    } else {
        buf.parse::<f32>().ok()
    }
}

pub fn plan_gate_report(_repo_root: &Path) -> AdmResult<GateReport> {
    Ok(deprecated_gate_report(
        "Legacy Plan Gate",
        LEGACY_PLAN_GATE_DEPRECATED,
    ))
}

pub fn parity_gate_report(repo_root: &Path) -> AdmResult<GateReport> {
    let mut report = GateReport::new("NEWrust Rust Parity Gate");
    report.add_row("project_root", repo_root.display().to_string());
    report.add_row(
        "required_execution",
        "cargo test --workspace --quiet".to_string(),
    );

    if !is_standalone_repo_root(repo_root) {
        report.add_blocker("standalone_project_root_invalid");
        return Ok(report);
    }

    let workspace_toml = repo_root.join("Cargo.toml");
    if !workspace_toml.is_file() {
        report.add_blocker("newrust_workspace_cargo_missing");
    } else {
        let cargo_text = fs::read_to_string(&workspace_toml)?;
        for member in REQUIRED_WORKSPACE_MEMBERS {
            report.add_row(format!("workspace_member:{member}"), "required");
            if !cargo_text.contains(&format!("\"{member}\"")) {
                report.add_blocker(format!("workspace_member_missing:{member}"));
            }
        }
    }

    let mut combined = String::new();
    for check in REQUIRED_PARITY_CHECKS {
        let path = repo_root.join(check.relative_path);
        report.add_row(
            format!("parity_check:{}:layer", check.id),
            check.layer.to_string(),
        );
        report.add_row(
            format!("parity_check:{}:path", check.id),
            check.relative_path.to_string(),
        );
        if !path.is_file() {
            report.add_blocker(format!("parity_check_file_missing:{}", check.id));
            continue;
        }
        let text = fs::read_to_string(&path)?;
        combined.push_str(&text);
        if !text.contains(check.marker) {
            report.add_blocker(format!("parity_check_marker_missing:{}", check.id));
        }
    }

    let ignored_tests = ignored_test_markers(repo_root)?;
    report.add_row("ignored_test_marker_count", ignored_tests.len().to_string());
    for ignored in ignored_tests {
        report.add_blocker(format!("ignored_test_marker:{ignored}"));
    }
    report.add_row(
        "required_parity_check_count",
        REQUIRED_PARITY_CHECKS.len().to_string(),
    );
    report.add_row("parity_source_hash", hash_text(&combined));
    Ok(report)
}

pub fn validation_gate_report(repo_root: &Path) -> AdmResult<GateReport> {
    let mut report = GateReport::new("NEWrust Validation Tool Gate");
    report.add_row(
        "source_contract",
        "crates/adm-new-application/src/validation_tools.rs",
    );
    report.add_row(
        "required_execution",
        "cargo test -p adm-new-application validation_tools; cargo test -p adm-new-cli validation_cli_commands_cover_a29_validators; cargo test -p adm-new-governance validation_gate",
    );

    let mut combined = String::new();
    for check in REQUIRED_VALIDATION_MARKERS {
        let path = repo_root.join(check.relative_path);
        report.add_row(
            format!("validation_check:{}:category", check.id),
            check.category.to_string(),
        );
        report.add_row(
            format!("validation_check:{}:path", check.id),
            check.relative_path.to_string(),
        );
        if !path.is_file() {
            report.add_blocker(format!("validation_check_file_missing:{}", check.id));
            continue;
        }
        let text = fs::read_to_string(&path)?;
        combined.push_str(&text);
        if !text.contains(check.marker) {
            report.add_blocker(format!("validation_check_marker_missing:{}", check.id));
        }
    }

    report.add_row(
        "required_validation_check_count",
        REQUIRED_VALIDATION_MARKERS.len().to_string(),
    );
    report.add_row("validation_source_hash", hash_text(&combined));
    Ok(report)
}

pub fn iteration_gate_report(repo_root: &Path) -> AdmResult<GateReport> {
    let mut report = GateReport::new("NEWrust Iteration Tool Gate");
    report.add_row(
        "source_contract",
        "crates/adm-new-application/src/iteration.rs",
    );
    report.add_row(
        "required_execution",
        "cargo test -p adm-new-application iteration; cargo test -p adm-new-cli iteration_cli_commands_cover_a30_iteration_flow; cargo test -p adm-new-governance iteration_gate",
    );

    let mut combined = String::new();
    for check in REQUIRED_ITERATION_MARKERS {
        let path = repo_root.join(check.relative_path);
        report.add_row(
            format!("iteration_check:{}:category", check.id),
            check.category.to_string(),
        );
        report.add_row(
            format!("iteration_check:{}:path", check.id),
            check.relative_path.to_string(),
        );
        if !path.is_file() {
            report.add_blocker(format!("iteration_check_file_missing:{}", check.id));
            continue;
        }
        let text = fs::read_to_string(&path)?;
        combined.push_str(&text);
        if !text.contains(check.marker) {
            report.add_blocker(format!("iteration_check_marker_missing:{}", check.id));
        }
    }

    report.add_row(
        "required_iteration_check_count",
        REQUIRED_ITERATION_MARKERS.len().to_string(),
    );
    report.add_row("iteration_source_hash", hash_text(&combined));
    Ok(report)
}

pub fn ui_shell_gate_report(repo_root: &Path) -> AdmResult<GateReport> {
    let mut report = GateReport::new("NEWrust UI Shell Gate");
    report.add_row(
        "source_contract",
        "apps/desktop-tauri; crates/adm-new-tauri-commands/src/shell.rs; web/src",
    );
    report.add_row(
        "required_execution",
        "cargo test -p desktop-tauri; cargo test -p adm-new-tauri-commands shell; npm test -- shell; npm run build; npm run ui-gate",
    );

    let mut combined = String::new();
    for check in REQUIRED_UI_SHELL_MARKERS {
        let path = repo_root.join(check.relative_path);
        report.add_row(
            format!("ui_shell_check:{}:category", check.id),
            check.category.to_string(),
        );
        report.add_row(
            format!("ui_shell_check:{}:path", check.id),
            check.relative_path.to_string(),
        );
        if !path.is_file() {
            report.add_blocker(format!("ui_shell_check_file_missing:{}", check.id));
            continue;
        }
        let text = fs::read_to_string(&path)?;
        combined.push_str(&text);
        if !text.contains(check.marker) {
            report.add_blocker(format!("ui_shell_check_marker_missing:{}", check.id));
        }
    }

    report.add_row(
        "required_ui_shell_check_count",
        REQUIRED_UI_SHELL_MARKERS.len().to_string(),
    );
    report.add_row("ui_shell_source_hash", hash_text(&combined));
    Ok(report)
}

pub fn ui_workbench_gate_report(_repo_root: &Path) -> AdmResult<GateReport> {
    Ok(deprecated_gate_report(
        "Legacy Python UI Workbench Gate",
        LEGACY_PYTHON_GATE_DEPRECATED,
    ))
}

#[allow(dead_code)]
fn legacy_ui_workbench_gate_report(repo_root: &Path) -> AdmResult<GateReport> {
    let mut report = GateReport::new("NEWrust UI Workbench Gate");
    report.add_row(
        "source_contract",
        "core/ui/app_window.py; design workbench UI; Web design workbench + Tauri design commands",
    );
    report.add_row(
        "required_execution",
        "cargo test -p adm-new-application design_workbench_service_covers_a32_state_templates_gameplay_autosave; cargo test -p adm-new-tauri-commands design_workbench_commands_cover_templates_autosave_gameplay_and_reset; npm test -- design; npm run build; npm run ui-gate",
    );

    let mut combined = String::new();
    for check in REQUIRED_UI_WORKBENCH_MARKERS {
        let path = repo_root.join(check.relative_path);
        report.add_row(
            format!("ui_workbench_check:{}:category", check.id),
            check.category.to_string(),
        );
        report.add_row(
            format!("ui_workbench_check:{}:path", check.id),
            check.relative_path.to_string(),
        );
        if !path.is_file() {
            report.add_blocker(format!("ui_workbench_check_file_missing:{}", check.id));
            continue;
        }
        let text = fs::read_to_string(&path)?;
        combined.push_str(&text);
        if !text.contains(check.marker) {
            report.add_blocker(format!("ui_workbench_check_marker_missing:{}", check.id));
        }
    }

    report.add_row(
        "required_ui_workbench_check_count",
        REQUIRED_UI_WORKBENCH_MARKERS.len().to_string(),
    );
    report.add_row("ui_workbench_source_hash", hash_text(&combined));
    Ok(report)
}

pub fn ui_ai_gate_report(_repo_root: &Path) -> AdmResult<GateReport> {
    Ok(deprecated_gate_report(
        "Legacy Python UI AI Gate",
        LEGACY_PYTHON_GATE_DEPRECATED,
    ))
}

#[allow(dead_code)]
fn legacy_ui_ai_gate_report(repo_root: &Path) -> AdmResult<GateReport> {
    let mut report = GateReport::new("NEWrust UI AI Gate");
    report.add_row(
        "source_contract",
        "core/ui/ai_interview_window.py; core/ui/embedded_interview.py; core/ui/bottom_panel.py",
    );
    report.add_row(
        "required_execution",
        "cargo test -p adm-new-tauri-commands ai; cargo test -p adm-new-governance ui_ai_gate; npm test -- ai-interview; cargo run -p adm-new-cli -- ui-ai-gate; npm run build; npm run ui-gate",
    );

    let mut combined = String::new();
    for check in REQUIRED_UI_AI_MARKERS {
        let path = repo_root.join(check.relative_path);
        report.add_row(
            format!("ui_ai_check:{}:category", check.id),
            check.category.to_string(),
        );
        report.add_row(
            format!("ui_ai_check:{}:path", check.id),
            check.relative_path.to_string(),
        );
        if !path.is_file() {
            report.add_blocker(format!("ui_ai_check_file_missing:{}", check.id));
            continue;
        }
        let text = fs::read_to_string(&path)?;
        combined.push_str(&text);
        if !text.contains(check.marker) {
            report.add_blocker(format!("ui_ai_check_marker_missing:{}", check.id));
        }
    }

    report.add_row(
        "required_ui_ai_check_count",
        REQUIRED_UI_AI_MARKERS.len().to_string(),
    );
    report.add_row("ui_ai_source_hash", hash_text(&combined));
    Ok(report)
}

pub fn ui_pipeline_gate_report(_repo_root: &Path) -> AdmResult<GateReport> {
    Ok(deprecated_gate_report(
        "Legacy Python UI Pipeline Gate",
        LEGACY_PYTHON_GATE_DEPRECATED,
    ))
}

#[allow(dead_code)]
fn legacy_ui_pipeline_gate_report(repo_root: &Path) -> AdmResult<GateReport> {
    let mut report = GateReport::new("NEWrust UI Pipeline Gate");
    report.add_row(
        "source_contract",
        "core/ui/pipeline_panel.py; core/ui/pipeline_step_card.py; core/ui/semantic_quality_panel.py",
    );
    report.add_row(
        "required_execution",
        "cargo test -p adm-new-tauri-commands pipeline; cargo test -p adm-new-governance ui_pipeline_gate; npm test -- pipeline; cargo run -p adm-new-cli -- ui-pipeline-gate; npm run build; npm run ui-gate",
    );

    let mut combined = String::new();
    for check in REQUIRED_UI_PIPELINE_MARKERS {
        let path = repo_root.join(check.relative_path);
        report.add_row(
            format!("ui_pipeline_check:{}:category", check.id),
            check.category.to_string(),
        );
        report.add_row(
            format!("ui_pipeline_check:{}:path", check.id),
            check.relative_path.to_string(),
        );
        if !path.is_file() {
            report.add_blocker(format!("ui_pipeline_check_file_missing:{}", check.id));
            continue;
        }
        let text = fs::read_to_string(&path)?;
        combined.push_str(&text);
        if !text.contains(check.marker) {
            report.add_blocker(format!("ui_pipeline_check_marker_missing:{}", check.id));
        }
    }

    report.add_row(
        "required_ui_pipeline_check_count",
        REQUIRED_UI_PIPELINE_MARKERS.len().to_string(),
    );
    report.add_row("ui_pipeline_source_hash", hash_text(&combined));
    Ok(report)
}

pub fn ui_utility_gate_report(_repo_root: &Path) -> AdmResult<GateReport> {
    Ok(deprecated_gate_report(
        "Legacy Python UI Utility Gate",
        LEGACY_PYTHON_GATE_DEPRECATED,
    ))
}

#[allow(dead_code)]
fn legacy_ui_utility_gate_report(repo_root: &Path) -> AdmResult<GateReport> {
    let mut report = GateReport::new("NEWrust UI Utility Gate");
    report.add_row(
        "source_contract",
        "core/ui/patch_panel.py; core/ui/package_panel.py; core/ui/sdk_panel.py; core/ui/save_manager_dialog.py",
    );
    report.add_row(
        "required_execution",
        "cargo test -p adm-new-tauri-commands save; cargo test -p adm-new-tauri-commands patch; cargo test -p adm-new-tauri-commands package; cargo test -p adm-new-tauri-commands sdk; cargo test -p adm-new-governance ui_utility_gate; npm test -- utility-panels; cargo run -p adm-new-cli -- ui-utility-gate; npm run build; npm run ui-gate",
    );

    let mut combined = String::new();
    for check in REQUIRED_UI_UTILITY_MARKERS {
        let path = repo_root.join(check.relative_path);
        report.add_row(
            format!("ui_utility_check:{}:category", check.id),
            check.category.to_string(),
        );
        report.add_row(
            format!("ui_utility_check:{}:path", check.id),
            check.relative_path.to_string(),
        );
        if !path.is_file() {
            report.add_blocker(format!("ui_utility_check_file_missing:{}", check.id));
            continue;
        }
        let text = fs::read_to_string(&path)?;
        combined.push_str(&text);
        if !text.contains(check.marker) {
            report.add_blocker(format!("ui_utility_check_marker_missing:{}", check.id));
        }
    }

    report.add_row(
        "required_ui_utility_check_count",
        REQUIRED_UI_UTILITY_MARKERS.len().to_string(),
    );
    report.add_row("ui_utility_source_hash", hash_text(&combined));
    Ok(report)
}

pub fn ui_settings_style_gate_report(_repo_root: &Path) -> AdmResult<GateReport> {
    Ok(deprecated_gate_report(
        "Legacy Python UI Settings/Style Gate",
        LEGACY_PYTHON_GATE_DEPRECATED,
    ))
}

#[allow(dead_code)]
fn legacy_ui_settings_style_gate_report(repo_root: &Path) -> AdmResult<GateReport> {
    let mut report = GateReport::new("NEWrust UI Settings Style Gate");
    report.add_row(
        "source_contract",
        "core/ui/ai_config_unified_dialog.py; core/ui/unity_config_dialog.py; core/ui/style_confirmation_dialog.py; core/ui/style_prompt_editor.py",
    );
    report.add_row(
        "required_execution",
        "cargo test -p adm-new-tauri-commands config; cargo test -p adm-new-governance ui_settings_style_gate; npm test -- settings-style; npm run e2e -- settings-style; cargo run -p adm-new-cli -- ui-settings-style-gate; npm run build; npm run ui-gate",
    );

    let mut combined = String::new();
    for check in REQUIRED_UI_SETTINGS_STYLE_MARKERS {
        let path = repo_root.join(check.relative_path);
        report.add_row(
            format!("ui_settings_style_check:{}:category", check.id),
            check.category.to_string(),
        );
        report.add_row(
            format!("ui_settings_style_check:{}:path", check.id),
            check.relative_path.to_string(),
        );
        if !path.is_file() {
            report.add_blocker(format!("ui_settings_style_check_file_missing:{}", check.id));
            continue;
        }
        let text = fs::read_to_string(&path)?;
        combined.push_str(&text);
        if !text.contains(check.marker) {
            report.add_blocker(format!(
                "ui_settings_style_check_marker_missing:{}",
                check.id
            ));
        }
    }

    report.add_row(
        "required_ui_settings_style_check_count",
        REQUIRED_UI_SETTINGS_STYLE_MARKERS.len().to_string(),
    );
    report.add_row("ui_settings_style_source_hash", hash_text(&combined));
    Ok(report)
}

pub fn ui_parity_v3_gate_report(_repo_root: &Path) -> AdmResult<GateReport> {
    Ok(deprecated_gate_report(
        "Legacy Python UI Parity V3 Gate",
        LEGACY_PYTHON_GATE_DEPRECATED,
    ))
}

#[allow(dead_code)]
fn legacy_ui_parity_v3_gate_report(repo_root: &Path) -> AdmResult<GateReport> {
    let mut report = GateReport::new("NEWrust UI Parity V3 Gate");
    report.add_row(
        "source_contract",
        "docs/migration-history/full_project_reproduction/07_ui_python_baseline_plan.md",
    );
    report.add_row(
        "required_execution",
        "npm run build; npm run ui-gate; npm run ui-baseline-gate; cargo test -p adm-new-governance ui_parity_v3_gate; cargo run -p adm-new-cli -- ui-parity-v3-gate",
    );

    let baseline_root = repo_root.join("testdata").join("ui_baselines");
    let index_path = baseline_root.join("index.json");
    report.add_row("baseline_root", baseline_root.display().to_string());
    report.add_row("baseline_index", index_path.display().to_string());
    if !index_path.is_file() {
        report.add_blocker("ui_parity_v3_index_missing".to_string());
        return Ok(report);
    }

    let index = read_json_file(&index_path)?;
    let declared_count = index
        .get("required_record_count")
        .and_then(Value::as_u64)
        .unwrap_or(0) as usize;
    report.add_row("required_record_count", declared_count.to_string());
    if declared_count != REQUIRED_UI_PARITY_V3_RECORD_COUNT {
        report.add_blocker(format!(
            "ui_parity_v3_required_count_mismatch:{declared_count}"
        ));
    }
    if string_value(&index, "gate") != "ui-parity-v3" {
        report.add_blocker("ui_parity_v3_index_gate_mismatch".to_string());
    }

    let records = index
        .get("records")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    report.add_row("indexed_record_count", records.len().to_string());
    if records.len() != REQUIRED_UI_PARITY_V3_RECORD_COUNT {
        report.add_blocker(format!(
            "ui_parity_v3_indexed_count_mismatch:{}",
            records.len()
        ));
    }

    let mut screenshot_required_count = 0usize;
    let mut manual_note_count = 0usize;
    let mut desktop_screenshot_count = 0usize;
    let mut narrow_screenshot_count = 0usize;
    let mut record_hash_input = String::new();

    for record_ref in records {
        let surface = string_value(&record_ref, "surface");
        let state = string_value(&record_ref, "state");
        let path_text = string_value(&record_ref, "path");
        let id = format!("{surface}:{state}");
        if surface.is_empty() || state.is_empty() || path_text.is_empty() {
            report.add_blocker(format!("ui_parity_v3_record_ref_invalid:{id}"));
            continue;
        }
        let path = repo_root.join(&path_text);
        if !path.is_file() {
            report.add_blocker(format!("ui_parity_v3_record_missing:{id}"));
            continue;
        }
        let text = fs::read_to_string(&path)?;
        record_hash_input.push_str(&text);
        let record: Value = serde_json::from_str(&text).map_err(|error| {
            AdmError::new(format!(
                "failed to parse UI baseline record {}: {error}",
                path.display()
            ))
        })?;
        if string_value(&record, "surface") != surface || string_value(&record, "state") != state {
            report.add_blocker(format!("ui_parity_v3_record_identity_mismatch:{id}"));
        }
        if string_value(&record, "python_source").is_empty() {
            report.add_blocker(format!("ui_parity_v3_python_source_missing:{id}"));
        }
        let python_screenshot = record.get("python_screenshot").unwrap_or(&Value::Null);
        if !string_value(python_screenshot, "path").is_empty()
            || !string_value(python_screenshot, "manual_review_note").is_empty()
        {
            manual_note_count += 1;
        } else {
            report.add_blocker(format!("ui_parity_v3_python_baseline_missing:{id}"));
        }
        let screenshot_required = record
            .get("screenshot_required")
            .and_then(Value::as_bool)
            .unwrap_or(true);
        if screenshot_required {
            screenshot_required_count += 1;
            if verify_record_screenshot(repo_root, &record, "desktop", &mut report, &id)? {
                desktop_screenshot_count += 1;
            }
            if verify_record_screenshot(repo_root, &record, "narrow", &mut report, &id)? {
                narrow_screenshot_count += 1;
            }
        }
        if record
            .get("interaction_trace")
            .and_then(Value::as_array)
            .map(Vec::is_empty)
            .unwrap_or(true)
        {
            report.add_blocker(format!("ui_parity_v3_trace_missing:{id}"));
        }
        if string_value(&record, "command_contract").is_empty() {
            report.add_blocker(format!("ui_parity_v3_command_contract_missing:{id}"));
        }
        let open_p0_p1 = record
            .pointer("/parity_notes/open_p0_p1_deltas")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        if !open_p0_p1.is_empty() {
            report.add_blocker(format!("ui_parity_v3_unapproved_delta:{id}"));
        }
    }

    report.add_row(
        "expected_ui_parity_v3_record_count",
        REQUIRED_UI_PARITY_V3_RECORD_COUNT.to_string(),
    );
    report.add_row(
        "screenshot_required_count",
        screenshot_required_count.to_string(),
    );
    report.add_row("python_baseline_note_count", manual_note_count.to_string());
    report.add_row(
        "desktop_screenshot_count",
        desktop_screenshot_count.to_string(),
    );
    report.add_row(
        "narrow_screenshot_count",
        narrow_screenshot_count.to_string(),
    );
    report.add_row("ui_parity_v3_record_hash", hash_text(&record_hash_input));
    Ok(report)
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct UnitTestMigrationRow {
    python_file: String,
    target: String,
    evidence: String,
    gate: String,
    status: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct IntegrationTestMigrationRow {
    python_file: String,
    target: String,
    evidence: String,
    gate: String,
    status: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct FinalMatrixSummary {
    name: &'static str,
    relative_path: &'static str,
    row_count: usize,
    decided_count: usize,
    pending_count: usize,
    drop_with_reason_count: usize,
    missing_target_count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct FinalGateStatus {
    relative_path: &'static str,
    status: String,
    hash: String,
}

pub fn unit_test_migration_gate_report(_repo_root: &Path) -> AdmResult<GateReport> {
    Ok(deprecated_gate_report(
        "Legacy Python Unit-Test Migration Gate",
        LEGACY_PYTHON_GATE_DEPRECATED,
    ))
}

#[allow(dead_code)]
fn legacy_unit_test_migration_gate_report(repo_root: &Path) -> AdmResult<GateReport> {
    let mut report = GateReport::new("NEWrust Unit Test Migration Gate");
    report.add_row(
        "source_contract",
        "core/tests/unit/*; 09_test_migration_matrix.md",
    );
    report.add_row(
        "required_execution",
        "cargo test --workspace; npm test; cargo test -p adm-new-governance unit_test_migration_gate; cargo run -p adm-new-cli -- unit-test-migration-gate",
    );

    let matrix_path = repo_root
        .join("docs")
        .join("migration-history")
        .join("full_project_reproduction")
        .join("09_test_migration_matrix.md");
    report.add_row("matrix_path", matrix_path.display().to_string());
    if !matrix_path.is_file() {
        report.add_blocker("unit_test_migration_matrix_missing");
        return Ok(report);
    }

    let matrix_text = fs::read_to_string(&matrix_path)?;
    let rows = parse_unit_test_migration_rows(&matrix_text);
    report.add_row("matrix_unit_row_count", rows.len().to_string());
    if rows.len() != REQUIRED_UNIT_TEST_MIGRATION_COUNT {
        report.add_blocker(format!(
            "unit_test_migration_row_count_mismatch:{}",
            rows.len()
        ));
    }

    let unit_dir = repo_root.join("core").join("tests").join("unit");
    report.add_row("python_unit_dir", unit_dir.display().to_string());
    let actual_files = if unit_dir.is_dir() {
        unit_python_test_files(&unit_dir)?
    } else {
        report.add_blocker("python_unit_dir_missing");
        Vec::new()
    };
    report.add_row("python_unit_file_count", actual_files.len().to_string());
    if actual_files.len() != REQUIRED_UNIT_TEST_MIGRATION_COUNT {
        report.add_blocker(format!(
            "python_unit_file_count_mismatch:{}",
            actual_files.len()
        ));
    }

    let mut row_files = BTreeSet::new();
    let mut duplicate_files = BTreeSet::new();
    let mut target_text = String::new();
    for row in &rows {
        if !row_files.insert(row.python_file.clone()) {
            duplicate_files.insert(row.python_file.clone());
        }
        if row.target.trim().is_empty() {
            report.add_blocker(format!("unit_test_target_missing:{}", row.python_file));
        }
        if row.evidence.trim().is_empty() {
            report.add_blocker(format!("unit_test_evidence_missing:{}", row.python_file));
        }
        if row.gate != "test gate" {
            report.add_blocker(format!(
                "unit_test_gate_mismatch:{}:{}",
                row.python_file, row.gate
            ));
        }
        if row.status != "decided" {
            report.add_blocker(format!(
                "unit_test_status_not_decided:{}:{}",
                row.python_file, row.status
            ));
        }
        target_text.push_str(&row.target);
        target_text.push('\n');
    }
    for duplicate in duplicate_files {
        report.add_blocker(format!("unit_test_duplicate_matrix_row:{duplicate}"));
    }

    let actual_set = actual_files.into_iter().collect::<BTreeSet<_>>();
    for missing in actual_set.difference(&row_files) {
        report.add_blocker(format!("unit_test_missing_matrix_row:{missing}"));
    }
    for extra in row_files.difference(&actual_set) {
        report.add_blocker(format!("unit_test_matrix_row_without_file:{extra}"));
    }

    for domain in REQUIRED_UNIT_TEST_TARGET_DOMAINS {
        report.add_row(format!("unit_test_target_domain:{domain}"), "required");
        if !target_text.contains(domain) {
            report.add_blocker(format!("unit_test_target_domain_missing:{domain}"));
        }
    }

    let mut combined = matrix_text;
    for check in REQUIRED_UNIT_TEST_MIGRATION_MARKERS {
        let path = repo_root.join(check.relative_path);
        report.add_row(
            format!("unit_test_marker:{}:category", check.id),
            check.category.to_string(),
        );
        report.add_row(
            format!("unit_test_marker:{}:path", check.id),
            check.relative_path.to_string(),
        );
        if !path.is_file() {
            report.add_blocker(format!("unit_test_marker_file_missing:{}", check.id));
            continue;
        }
        let text = fs::read_to_string(&path)?;
        combined.push_str(&text);
        if !text.contains(check.marker) {
            report.add_blocker(format!("unit_test_marker_missing:{}", check.id));
        }
    }

    let ignored_tests = ignored_test_markers(repo_root)?;
    report.add_row("ignored_test_marker_count", ignored_tests.len().to_string());
    for ignored in ignored_tests {
        report.add_blocker(format!("ignored_test_marker:{ignored}"));
    }
    report.add_row(
        "expected_unit_test_migration_count",
        REQUIRED_UNIT_TEST_MIGRATION_COUNT.to_string(),
    );
    report.add_row(
        "required_unit_test_marker_count",
        REQUIRED_UNIT_TEST_MIGRATION_MARKERS.len().to_string(),
    );
    report.add_row("unit_test_migration_source_hash", hash_text(&combined));
    Ok(report)
}

pub fn integration_test_migration_gate_report(_repo_root: &Path) -> AdmResult<GateReport> {
    Ok(deprecated_gate_report(
        "Legacy Python Integration-Test Migration Gate",
        LEGACY_PYTHON_GATE_DEPRECATED,
    ))
}

#[allow(dead_code)]
fn legacy_integration_test_migration_gate_report(repo_root: &Path) -> AdmResult<GateReport> {
    let mut report = GateReport::new("NEWrust Integration Test Migration Gate");
    report.add_row(
        "source_contract",
        "core/tests/integration/*; core/tests/conftest.py; 09_test_migration_matrix.md",
    );
    report.add_row(
        "required_execution",
        "cargo test --workspace; npm test; cargo test -p adm-new-governance integration_test_migration_gate; cargo run -p adm-new-cli -- integration-test-migration-gate",
    );

    let matrix_path = repo_root
        .join("docs")
        .join("migration-history")
        .join("full_project_reproduction")
        .join("09_test_migration_matrix.md");
    report.add_row("matrix_path", matrix_path.display().to_string());
    if !matrix_path.is_file() {
        report.add_blocker("integration_test_migration_matrix_missing");
        return Ok(report);
    }

    let matrix_text = fs::read_to_string(&matrix_path)?;
    let rows = parse_integration_test_migration_rows(&matrix_text);
    report.add_row("matrix_integration_row_count", rows.len().to_string());
    if rows.len() != REQUIRED_INTEGRATION_TEST_MIGRATION_COUNT {
        report.add_blocker(format!(
            "integration_test_migration_row_count_mismatch:{}",
            rows.len()
        ));
    }

    let actual_files = integration_python_test_files(repo_root, &mut report)?;
    report.add_row(
        "python_integration_file_count",
        actual_files.len().to_string(),
    );
    if actual_files.len() != REQUIRED_INTEGRATION_TEST_MIGRATION_COUNT {
        report.add_blocker(format!(
            "python_integration_file_count_mismatch:{}",
            actual_files.len()
        ));
    }

    let mut row_files = BTreeSet::new();
    let mut duplicate_files = BTreeSet::new();
    let mut target_text = String::new();
    for row in &rows {
        if !row_files.insert(row.python_file.clone()) {
            duplicate_files.insert(row.python_file.clone());
        }
        if row.target.trim().is_empty() {
            report.add_blocker(format!(
                "integration_test_target_missing:{}",
                row.python_file
            ));
        }
        if row.evidence.trim().is_empty() {
            report.add_blocker(format!(
                "integration_test_evidence_missing:{}",
                row.python_file
            ));
        }
        if row.gate != "test gate" {
            report.add_blocker(format!(
                "integration_test_gate_mismatch:{}:{}",
                row.python_file, row.gate
            ));
        }
        if row.status != "decided" {
            report.add_blocker(format!(
                "integration_test_status_not_decided:{}:{}",
                row.python_file, row.status
            ));
        }
        target_text.push_str(&row.target);
        target_text.push('\n');
    }
    for duplicate in duplicate_files {
        report.add_blocker(format!("integration_test_duplicate_matrix_row:{duplicate}"));
    }

    let actual_set = actual_files.into_iter().collect::<BTreeSet<_>>();
    for missing in actual_set.difference(&row_files) {
        report.add_blocker(format!("integration_test_missing_matrix_row:{missing}"));
    }
    for extra in row_files.difference(&actual_set) {
        report.add_blocker(format!("integration_test_matrix_row_without_file:{extra}"));
    }

    for domain in REQUIRED_INTEGRATION_TEST_TARGET_DOMAINS {
        report.add_row(
            format!("integration_test_target_domain:{domain}"),
            "required",
        );
        if !target_text.contains(domain) {
            report.add_blocker(format!("integration_test_target_domain_missing:{domain}"));
        }
    }

    let mut combined = matrix_text;
    for check in REQUIRED_INTEGRATION_TEST_MIGRATION_MARKERS {
        let path = repo_root.join(check.relative_path);
        report.add_row(
            format!("integration_test_marker:{}:category", check.id),
            check.category.to_string(),
        );
        report.add_row(
            format!("integration_test_marker:{}:path", check.id),
            check.relative_path.to_string(),
        );
        if !path.is_file() {
            report.add_blocker(format!("integration_test_marker_file_missing:{}", check.id));
            continue;
        }
        let text = fs::read_to_string(&path)?;
        combined.push_str(&text);
        if !text.contains(check.marker) {
            report.add_blocker(format!("integration_test_marker_missing:{}", check.id));
        }
    }

    let ignored_tests = ignored_test_markers(repo_root)?;
    report.add_row("ignored_test_marker_count", ignored_tests.len().to_string());
    for ignored in ignored_tests {
        report.add_blocker(format!("ignored_test_marker:{ignored}"));
    }
    report.add_row(
        "expected_integration_test_migration_count",
        REQUIRED_INTEGRATION_TEST_MIGRATION_COUNT.to_string(),
    );
    report.add_row(
        "required_integration_test_marker_count",
        REQUIRED_INTEGRATION_TEST_MIGRATION_MARKERS
            .len()
            .to_string(),
    );
    report.add_row(
        "integration_test_migration_source_hash",
        hash_text(&combined),
    );
    Ok(report)
}

fn parse_unit_test_migration_rows(text: &str) -> Vec<UnitTestMigrationRow> {
    text.lines()
        .filter_map(|line| {
            let trimmed = line.trim();
            if !trimmed.starts_with('|') || !trimmed.contains("core\\tests\\unit\\") {
                return None;
            }
            let columns = trimmed
                .trim_matches('|')
                .split('|')
                .map(markdown_cell_text)
                .collect::<Vec<_>>();
            if columns.len() < 5 {
                return None;
            }
            Some(UnitTestMigrationRow {
                python_file: normalize_matrix_path(&columns[0]),
                target: columns[1].clone(),
                evidence: columns[2].clone(),
                gate: columns[3].clone(),
                status: columns[4].clone(),
            })
        })
        .collect()
}

fn parse_integration_test_migration_rows(text: &str) -> Vec<IntegrationTestMigrationRow> {
    text.lines()
        .filter_map(|line| {
            let trimmed = line.trim();
            if !trimmed.starts_with('|')
                || !(trimmed.contains("core\\tests\\integration\\")
                    || trimmed.contains("core\\tests\\conftest.py"))
            {
                return None;
            }
            let columns = trimmed
                .trim_matches('|')
                .split('|')
                .map(markdown_cell_text)
                .collect::<Vec<_>>();
            if columns.len() < 5 {
                return None;
            }
            Some(IntegrationTestMigrationRow {
                python_file: normalize_matrix_path(&columns[0]),
                target: columns[1].clone(),
                evidence: columns[2].clone(),
                gate: columns[3].clone(),
                status: columns[4].clone(),
            })
        })
        .collect()
}

fn markdown_cell_text(value: &str) -> String {
    value.trim().trim_matches('`').trim().to_string()
}

fn normalize_matrix_path(value: &str) -> String {
    value.replace('/', "\\")
}

fn unit_python_test_files(unit_dir: &Path) -> AdmResult<Vec<String>> {
    let mut files = Vec::new();
    for entry in fs::read_dir(unit_dir)? {
        let entry = entry?;
        if !entry.file_type()?.is_file() {
            continue;
        }
        let filename = entry.file_name().to_string_lossy().to_string();
        if filename.ends_with(".py") {
            files.push(format!("core\\tests\\unit\\{filename}"));
        }
    }
    files.sort();
    Ok(files)
}

fn integration_python_test_files(
    repo_root: &Path,
    report: &mut GateReport,
) -> AdmResult<Vec<String>> {
    let mut files = Vec::new();
    let conftest_path = repo_root.join("core").join("tests").join("conftest.py");
    report.add_row("python_conftest", conftest_path.display().to_string());
    if conftest_path.is_file() {
        files.push("core\\tests\\conftest.py".to_string());
    } else {
        report.add_blocker("python_conftest_missing");
    }

    let integration_dir = repo_root.join("core").join("tests").join("integration");
    report.add_row(
        "python_integration_dir",
        integration_dir.display().to_string(),
    );
    if !integration_dir.is_dir() {
        report.add_blocker("python_integration_dir_missing");
        files.sort();
        return Ok(files);
    }

    for entry in fs::read_dir(integration_dir)? {
        let entry = entry?;
        if !entry.file_type()?.is_file() {
            continue;
        }
        let filename = entry.file_name().to_string_lossy().to_string();
        if filename.ends_with(".py") {
            files.push(format!("core\\tests\\integration\\{filename}"));
        }
    }
    files.sort();
    Ok(files)
}

fn verify_record_screenshot(
    repo_root: &Path,
    record: &Value,
    viewport: &str,
    report: &mut GateReport,
    id: &str,
) -> AdmResult<bool> {
    let Some(screenshot) = record
        .get("web_screenshot")
        .and_then(|value| value.get(viewport))
    else {
        report.add_blocker(format!("ui_parity_v3_{viewport}_screenshot_missing:{id}"));
        return Ok(false);
    };
    let path_text = string_value(screenshot, "path");
    if path_text.is_empty() {
        report.add_blocker(format!(
            "ui_parity_v3_{viewport}_screenshot_path_missing:{id}"
        ));
        return Ok(false);
    }
    let path = repo_root.join(&path_text);
    if !path.is_file() {
        report.add_blocker(format!(
            "ui_parity_v3_{viewport}_screenshot_file_missing:{id}"
        ));
        return Ok(false);
    }
    let bytes = fs::read(&path)?;
    if !looks_like_png(&bytes) || bytes.len() < 10_000 {
        report.add_blocker(format!("ui_parity_v3_{viewport}_screenshot_invalid:{id}"));
        return Ok(false);
    }
    Ok(true)
}

fn read_json_file(path: &Path) -> AdmResult<Value> {
    let text = fs::read_to_string(path)?;
    serde_json::from_str(&text)
        .map_err(|error| AdmError::new(format!("failed to parse JSON {}: {error}", path.display())))
}

fn string_value(value: &Value, field: &str) -> String {
    value
        .get(field)
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string()
}

fn looks_like_png(bytes: &[u8]) -> bool {
    bytes.starts_with(b"\x89PNG\r\n\x1a\n")
}

pub fn package_gate_report(repo_root: &Path) -> AdmResult<GateReport> {
    let mut report = GateReport::new("NEWrust Package Contract Self-Test");
    report.add_row("project_root", repo_root.display().to_string());
    report.add_row("scope", "synthetic-contract-fixtures-not-release-artifact");
    report.add_row(
        "required_execution",
        "cargo test -p adm-new-packaging; cargo test -p adm-new-tauri-commands package".to_string(),
    );
    report.add_row(
        "source_contract",
        "crates/adm-new-contracts/src/package.rs; crates/adm-new-packaging/src/lib.rs".to_string(),
    );

    if !is_standalone_repo_root(repo_root) {
        report.add_blocker("standalone_project_root_invalid");
        return Ok(report);
    }

    let service = PackagingService::new();

    let success = service.run_package(package_success_sources());
    add_package_result_rows(&mut report, "success", &success);
    require_package_success(&mut report, "success", &success);

    let missing_changed_files = service.run_package(package_sources_with_missing_changed_files());
    add_package_result_rows(&mut report, "missing_changed_files", &missing_changed_files);
    require_package_blocker(
        &mut report,
        "missing_changed_files",
        &missing_changed_files,
        "PACKAGE-NO-ACTUAL-PROJECT-CHANGES",
    );

    let missing_unity_summary = service.run_package(package_sources_with_missing_unity_summary());
    add_package_result_rows(&mut report, "missing_unity_summary", &missing_unity_summary);
    require_package_blocker(
        &mut report,
        "missing_unity_summary",
        &missing_unity_summary,
        "PACKAGE-UNITY-VALIDATION-MISSING",
    );

    Ok(report)
}

pub fn standalone_boundary_gate_report(repo_root: &Path) -> AdmResult<GateReport> {
    let mut report = GateReport::new("NEWrust Standalone Boundary Gate");
    report.add_row("project_root", repo_root.display().to_string());
    report.add_row("root_contract", SOURCE_ROOT_MARKER);
    report.add_row("resource_contract", RESOURCE_MANIFEST_PATH);
    report.add_row("portable_contract", "tools/build-portable.ps1");
    report.add_row("relocation_contract", "root-name-independent");

    let required_files = [
        SOURCE_ROOT_MARKER,
        "Cargo.toml",
        "Cargo.lock",
        RESOURCE_MANIFEST_PATH,
        "pipeline/artifact_layer/registry.json",
        "web/package.json",
        "tools/build-portable.ps1",
        "tools/Finalize-PortableSwap.ps1",
        "tools/clean-generated.ps1",
        "tools/verify-standalone.ps1",
    ];
    for relative_path in required_files {
        let path = repo_root.join(relative_path);
        report.add_row(
            format!("required_file:{relative_path}"),
            path.is_file().to_string(),
        );
        if !path.is_file() {
            report.add_blocker(format!("standalone_required_file_missing:{relative_path}"));
        }
    }

    if !is_standalone_repo_root(repo_root) {
        report.add_blocker("standalone_project_root_invalid");
        return Ok(report);
    }

    let marker = read_json_file(&repo_root.join(SOURCE_ROOT_MARKER))?;
    if string_value(&marker, "kind") != "source-project-root" {
        report.add_blocker("source_root_marker_kind_invalid");
    }
    if string_value(&marker, "workspaceManifest") != "Cargo.toml" {
        report.add_blocker("source_root_workspace_manifest_invalid");
    }
    if string_value(&marker, "resourceManifest") != RESOURCE_MANIFEST_PATH {
        report.add_blocker("source_root_resource_manifest_invalid");
    }

    let resource_manifest = read_json_file(&repo_root.join(RESOURCE_MANIFEST_PATH))?;
    let project_id = string_value(&resource_manifest, "projectId");
    report.add_row("resource_manifest:project_id", project_id.clone());
    if project_id.is_empty() {
        report.add_blocker("resource_manifest_project_id_missing");
    }
    let group_count = resource_manifest
        .get("groups")
        .and_then(Value::as_array)
        .map(Vec::len)
        .unwrap_or_default();
    report.add_row("resource_manifest:group_count", group_count.to_string());
    if group_count == 0 {
        report.add_blocker("resource_manifest_groups_missing");
    }
    let resource_verification = verify_source_resource_manifest(repo_root);
    report.add_row(
        "resource_manifest:integrity_status",
        resource_verification.status.clone(),
    );
    report.add_row(
        "resource_manifest:verified_group_count",
        resource_verification.groups.len().to_string(),
    );
    for blocker in resource_verification.blockers {
        let path = blocker.path.unwrap_or_default();
        report.add_blocker(format!(
            "source_resource_integrity:{}:{}:{}",
            blocker.code, path, blocker.message
        ));
    }

    let nested = repo_root.join("apps").join("adm-new-cli").join("src");
    match find_repo_root(&nested) {
        Some(found) if same_path(&found, repo_root) => {
            report.add_row("relocation_contract:subdirectory_resolution", "passed");
        }
        Some(found) => report.add_blocker(format!(
            "relocation_contract_wrong_root:{}",
            found.display()
        )),
        None => report.add_blocker("relocation_contract_root_not_found"),
    }

    for relative_path in [
        "crates/adm-new-governance/src/lib.rs",
        "apps/adm-new-cli/src/main.rs",
    ] {
        let path = repo_root.join(relative_path);
        if !path.is_file() {
            continue;
        }
        let source = fs::read_to_string(path)?;
        let forbidden_patterns = [
            ["repo_root.join(\"", "NEWrust", "\")"].concat(),
            ["join(\"plan\")", ".join(\"NEWrust\")"].concat(),
            ["containing plan", "/NEWrust"].concat(),
        ];
        for forbidden in forbidden_patterns {
            if source.contains(&forbidden) {
                report.add_blocker(format!(
                    "standalone_parent_path_dependency:{relative_path}:{forbidden}"
                ));
            }
        }
    }

    Ok(report)
}

pub fn release_gate_report(repo_root: &Path) -> AdmResult<GateReport> {
    let mut report = GateReport::new("NEWrust Release Gate");
    let web_root = repo_root.join("web");
    report.add_row("project_root", repo_root.display().to_string());
    report.add_row("web_root", web_root.display().to_string());
    report.add_row(
        "required_release_check_count",
        REQUIRED_RELEASE_CHECKS.len().to_string(),
    );
    for check in REQUIRED_RELEASE_CHECKS {
        report.add_row(format!("release_check:{check}"), "evidence-required");
    }

    if !is_standalone_repo_root(repo_root) {
        report.add_blocker("standalone_project_root_invalid");
        return Ok(report);
    }
    if !web_root.is_dir() {
        report.add_blocker("web_root_missing");
        return Ok(report);
    }

    add_release_evidence(&mut report, repo_root)?;

    add_nested_gate_result(
        &mut report,
        "standalone_boundary_gate",
        standalone_boundary_gate_report(repo_root)?,
    );
    add_nested_gate_result(
        &mut report,
        "package_contract_self_test",
        package_gate_report(repo_root)?,
    );

    let forbidden_markers = collect_forbidden_evidence_markers(repo_root)?;
    report.add_row(
        "anti_fake_scan:marker_count",
        forbidden_markers.len().to_string(),
    );
    for marker in forbidden_markers {
        report.add_blocker(format!("anti_fake_marker:{marker}"));
    }

    Ok(report)
}

fn add_release_evidence(report: &mut GateReport, repo_root: &Path) -> AdmResult<()> {
    let evidence_path = repo_root.join(RELEASE_EVIDENCE_PATH);
    report.add_row("release_evidence_path", RELEASE_EVIDENCE_PATH.to_string());
    if !evidence_path.is_file() {
        report.add_blocker("standalone_release_evidence_missing");
        for check in REQUIRED_RELEASE_CHECKS {
            report.add_row(format!("release_evidence:{check}"), "missing");
        }
        return Ok(());
    }
    let metadata = fs::symlink_metadata(&evidence_path)?;
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        report.add_blocker("standalone_release_evidence_must_be_regular_file");
        return Ok(());
    }
    let evidence_bytes = fs::read(&evidence_path)?;
    let evidence: StandaloneReleaseEvidence = match serde_json::from_slice(&evidence_bytes) {
        Ok(evidence) => evidence,
        Err(error) => {
            report.add_blocker(format!("standalone_release_evidence_json_invalid:{error}"));
            return Ok(());
        }
    };
    let current_commit = git_output(repo_root, &["rev-parse", "HEAD"]);
    let current_status = git_output(
        repo_root,
        &["status", "--porcelain=v1", "--untracked-files=all"],
    );
    let current_commit_text = current_commit.clone().unwrap_or_default();
    report.add_row(
        "release_evidence:trust_model",
        "same-user-stale-or-accidental-misuse-guard-not-cryptographic-attestation",
    );
    report.add_row("release_evidence:producer", evidence.producer.clone());
    report.add_row("release_evidence:evidence_id", evidence.evidence_id.clone());
    report.add_row("release_evidence:status", evidence.status.clone());
    report.add_row("release_evidence:git_commit", evidence.git_commit.clone());
    for blocker in
        validate_release_evidence_structure(&evidence, unix_timestamp(), &current_commit_text)
    {
        report.add_blocker(blocker);
    }
    match current_commit {
        Some(current) if current == evidence.git_commit => {
            report.add_row("release_evidence:current_head_matches", "true");
        }
        Some(current) => {
            report.add_row("release_evidence:current_head", current);
            report.add_blocker("standalone_release_evidence_head_mismatch");
        }
        None => report.add_blocker("standalone_release_git_head_unavailable"),
    }
    match current_status {
        Some(status_text) if status_text.is_empty() => {
            report.add_row("release_evidence:current_tree_clean", "true");
        }
        Some(_) => report.add_blocker("standalone_release_current_tree_dirty"),
        None => report.add_blocker("standalone_release_git_status_unavailable"),
    }

    for check in REQUIRED_RELEASE_CHECKS {
        let command_evidence = evidence.checks.get(*check);
        let passed = command_evidence.is_some_and(|value| {
            value.status == "passed"
                && value.exit_code == 0
                && !value.command.trim().is_empty()
                && is_sha256(&value.output_sha256)
        });
        report.add_row(
            format!("release_evidence:{check}"),
            if passed {
                "passed"
            } else {
                "missing-or-failed"
            },
        );
        if let Some(command) = command_evidence {
            report.add_row(
                format!("release_evidence:{check}:command"),
                command.command.clone(),
            );
            report.add_row(
                format!("release_evidence:{check}:output_sha256"),
                command.output_sha256.clone(),
            );
        }
        if !passed {
            report.add_blocker(format!(
                "standalone_release_check_missing_or_failed:{check}"
            ));
        }
    }
    if let Some(portable) = &evidence.portable {
        add_release_portable_verification(report, repo_root, &evidence, portable)?;
    } else {
        report.add_blocker("standalone_release_portable_evidence_missing");
    }
    Ok(())
}

fn validate_release_evidence_structure(
    evidence: &StandaloneReleaseEvidence,
    now: u64,
    current_commit: &str,
) -> Vec<String> {
    let mut blockers = Vec::new();
    if evidence.schema_version != RELEASE_EVIDENCE_SCHEMA_VERSION {
        blockers.push("standalone_release_evidence_schema_invalid".to_string());
    }
    if evidence.producer != RELEASE_EVIDENCE_PRODUCER {
        blockers.push("standalone_release_evidence_producer_invalid".to_string());
    }
    if evidence.evidence_id.len() != 32
        || !evidence
            .evidence_id
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit())
    {
        blockers.push("standalone_release_evidence_id_invalid".to_string());
    }
    if evidence.project_id != SOURCE_PROJECT_ID {
        blockers.push("standalone_release_evidence_project_id_invalid".to_string());
    }
    if evidence.status != "passed" {
        blockers.push("standalone_release_evidence_not_passed".to_string());
    }
    if !evidence.source_tree_clean {
        blockers.push("standalone_release_evidence_source_tree_not_clean".to_string());
    }
    if !current_commit.is_empty() && evidence.git_commit != current_commit {
        blockers.push("standalone_release_evidence_head_mismatch".to_string());
    }
    if !evidence.errors.is_empty() {
        blockers.push("standalone_release_evidence_contains_errors".to_string());
    }
    if evidence.generated_at_unix > now.saturating_add(RELEASE_EVIDENCE_CLOCK_SKEW_SECONDS)
        || evidence.expires_at_unix <= now
        || evidence.expires_at_unix <= evidence.generated_at_unix
        || evidence.expires_at_unix - evidence.generated_at_unix
            > RELEASE_EVIDENCE_MAX_LIFETIME_SECONDS
    {
        blockers.push("standalone_release_evidence_freshness_invalid".to_string());
    }
    let required = REQUIRED_RELEASE_CHECKS
        .iter()
        .copied()
        .collect::<BTreeSet<_>>();
    let actual = evidence
        .checks
        .keys()
        .map(String::as_str)
        .collect::<BTreeSet<_>>();
    if actual != required {
        blockers.push("standalone_release_evidence_check_set_invalid".to_string());
    }
    for check in REQUIRED_RELEASE_CHECKS {
        match evidence.checks.get(*check) {
            Some(value)
                if value.status == "passed"
                    && value.exit_code == 0
                    && !value.command.trim().is_empty()
                    && is_sha256(&value.output_sha256) =>
            {
                let _ = value.duration_ms;
            }
            _ => blockers.push(format!("standalone_release_check_invalid:{check}")),
        }
    }
    match &evidence.portable {
        Some(portable)
            if portable.root == RELEASE_PORTABLE_ROOT
                && portable.executable == "AutoDesignMaker.exe"
                && portable.git_commit == evidence.git_commit
                && is_sha256(&portable.executable_sha256)
                && is_sha256(&portable.build_manifest_sha256)
                && is_sha256(&portable.resource_manifest_sha256)
                && is_sha256(&portable.swap_receipt_sha256)
                && portable.transaction_id.len() == 32
                && portable
                    .transaction_id
                    .bytes()
                    .all(|byte| byte.is_ascii_hexdigit())
                && portable.transaction_status == "finalized"
                && portable.swap_receipt
                    == format!(
                        "dist/.{RELEASE_PORTABLE_OUTPUT_NAME}.swap-{}.json",
                        portable.transaction_id
                    )
                && is_safe_project_relative_path(&portable.swap_receipt) => {}
        _ => blockers.push("standalone_release_portable_evidence_invalid".to_string()),
    }
    blockers
}

fn is_safe_project_relative_path(value: &str) -> bool {
    let path = Path::new(value);
    !value.trim().is_empty()
        && !path.is_absolute()
        && path
            .components()
            .all(|component| matches!(component, Component::Normal(_)))
}

fn is_sha256(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn add_release_portable_verification(
    report: &mut GateReport,
    repo_root: &Path,
    evidence: &StandaloneReleaseEvidence,
    portable: &ReleasePortableEvidence,
) -> AdmResult<()> {
    if portable.root != RELEASE_PORTABLE_ROOT {
        report.add_blocker("standalone_release_portable_root_invalid");
        return Ok(());
    }
    let expected_receipt = format!(
        "dist/.{RELEASE_PORTABLE_OUTPUT_NAME}.swap-{}.json",
        portable.transaction_id
    );
    if !is_safe_project_relative_path(&portable.swap_receipt)
        || portable.swap_receipt != expected_receipt
    {
        report.add_blocker("standalone_release_portable_receipt_path_invalid");
        return Ok(());
    }
    let root = repo_root.join(RELEASE_PORTABLE_ROOT);
    let receipt_path = repo_root.join(&portable.swap_receipt);
    let receipt_metadata = match fs::symlink_metadata(&receipt_path) {
        Ok(metadata)
            if metadata.is_file() && !release_metadata_is_reparse_or_symlink(&metadata) =>
        {
            metadata
        }
        Ok(_) => {
            report.add_blocker("release_portable_swap_receipt_must_be_regular_file");
            return Ok(());
        }
        Err(error) => {
            report.add_blocker(format!("release_portable_swap_receipt_unavailable:{error}"));
            return Ok(());
        }
    };
    let _ = receipt_metadata.len();
    let receipt_bytes = fs::read(&receipt_path)?;
    let receipt_hash = sha256_hex(&receipt_bytes);
    report.add_row(
        "release_portable:swap_receipt",
        portable.swap_receipt.clone(),
    );
    report.add_row(
        "release_portable:transaction_id",
        portable.transaction_id.clone(),
    );
    if receipt_hash != portable.swap_receipt_sha256 {
        report.add_blocker("release_portable_swap_receipt_evidence_hash_mismatch");
    }
    let receipt: Value = match serde_json::from_slice(&receipt_bytes) {
        Ok(receipt) => receipt,
        Err(error) => {
            report.add_blocker(format!("release_portable_swap_receipt_invalid:{error}"));
            return Ok(());
        }
    };
    let verification = verify_portable_resource_root(&root);
    report.add_row(
        "release_portable:integrity_status",
        verification.status.clone(),
    );
    for blocker in verification.blockers {
        report.add_blocker(format!(
            "release_portable_integrity:{}:{}",
            blocker.code, blocker.message
        ));
    }
    let build_path = root.join("build-manifest.json");
    let resource_path = root.join("portable-resource-manifest.json");
    let executable_path = root.join("AutoDesignMaker.exe");
    let build_bytes = match fs::read(&build_path) {
        Ok(bytes) => bytes,
        Err(error) => {
            report.add_blocker(format!(
                "release_portable_build_manifest_unavailable:{error}"
            ));
            return Ok(());
        }
    };
    let resource_bytes = fs::read(&resource_path)?;
    let executable_bytes = fs::read(&executable_path)?;
    let build_hash = sha256_hex(&build_bytes);
    let resource_hash = sha256_hex(&resource_bytes);
    let executable_hash = sha256_hex(&executable_bytes);
    for (actual, expected, code) in [
        (
            &build_hash,
            &portable.build_manifest_sha256,
            "release_portable_build_manifest_evidence_hash_mismatch",
        ),
        (
            &resource_hash,
            &portable.resource_manifest_sha256,
            "release_portable_resource_manifest_evidence_hash_mismatch",
        ),
        (
            &executable_hash,
            &portable.executable_sha256,
            "release_portable_executable_evidence_hash_mismatch",
        ),
    ] {
        if actual != expected {
            report.add_blocker(code);
        }
    }
    let manifest: Value = serde_json::from_slice(&build_bytes)
        .map_err(|error| AdmError::new(format!("invalid portable build manifest: {error}")))?;
    for blocker in
        validate_portable_transaction_contract(repo_root, evidence, portable, &receipt, &manifest)
    {
        report.add_blocker(blocker);
    }
    match measure_release_immutable_tree(&root) {
        Ok((files, bytes, digest)) => {
            let expected = receipt.get("staged_immutable_tree");
            if expected
                .and_then(|value| value.get("FileCount"))
                .and_then(Value::as_u64)
                != Some(files)
                || expected
                    .and_then(|value| value.get("Bytes"))
                    .and_then(Value::as_u64)
                    != Some(bytes)
                || expected
                    .and_then(|value| value.get("Digest"))
                    .and_then(Value::as_str)
                    != Some(digest.as_str())
            {
                report.add_blocker("release_portable_immutable_tree_mismatch");
            }
        }
        Err(error) => report.add_blocker(format!(
            "release_portable_immutable_tree_unavailable:{error}"
        )),
    }
    for blocker in collect_unresolved_portable_output_state(
        repo_root,
        RELEASE_PORTABLE_OUTPUT_NAME,
        &portable.swap_receipt,
    )? {
        report.add_blocker(blocker);
    }
    let manifest_commit = string_value(&manifest, "git_commit");
    if manifest_commit != evidence.git_commit || manifest_commit != portable.git_commit {
        report.add_blocker("release_portable_git_commit_mismatch");
    }
    for (field, expected, code) in [
        (
            "release_mode",
            "formal",
            "release_portable_release_mode_invalid",
        ),
        (
            "crt_linkage",
            "static-msvc",
            "release_portable_crt_linkage_invalid",
        ),
        (
            "pe_machine",
            "x86_64",
            "release_portable_pe_machine_invalid",
        ),
        (
            "user_data_mode",
            "clean_release",
            "release_portable_user_data_mode_invalid",
        ),
        (
            "executable_sha256",
            executable_hash.as_str(),
            "release_portable_manifest_executable_hash_mismatch",
        ),
        (
            "resource_manifest_sha256",
            resource_hash.as_str(),
            "release_portable_manifest_resource_hash_mismatch",
        ),
    ] {
        if string_value(&manifest, field) != expected {
            report.add_blocker(code);
        }
    }
    if manifest
        .get("development_snapshot")
        .and_then(Value::as_bool)
        != Some(false)
    {
        report.add_blocker("release_portable_development_snapshot_invalid");
    }
    if manifest
        .get("dynamic_crt_dependencies")
        .and_then(Value::as_array)
        .is_none_or(|items| !items.is_empty())
    {
        report.add_blocker("release_portable_dynamic_crt_dependencies_present");
    }
    if manifest.get("user_data_files").and_then(Value::as_u64) != Some(0)
        || manifest.get("user_data_bytes").and_then(Value::as_u64) != Some(0)
        || string_value(&manifest, "user_data_digest")
            != "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
    {
        report.add_blocker("release_portable_declared_user_data_not_empty");
    }
    if manifest
        .get("pe_dependencies")
        .and_then(Value::as_array)
        .is_none()
    {
        report.add_blocker("release_portable_pe_dependencies_missing");
    }
    match measure_resource_tree(root.join("user_data")) {
        Ok(measure) => {
            let expected = receipt.get("staged_user_data");
            if expected
                .and_then(|value| value.get("FileCount"))
                .and_then(Value::as_u64)
                != Some(measure.files)
                || expected
                    .and_then(|value| value.get("Bytes"))
                    .and_then(Value::as_u64)
                    != Some(measure.bytes)
                || expected
                    .and_then(|value| value.get("Digest"))
                    .and_then(Value::as_str)
                    != Some(measure.tree_sha256.as_str())
            {
                report.add_blocker("release_portable_transaction_user_data_mismatch");
            }
            if measure.files != 0 || measure.bytes != 0 {
                report.add_blocker("release_portable_actual_user_data_not_empty");
            }
        }
        Err(error) => report.add_blocker(format!("release_portable_user_data_unavailable:{error}")),
    }
    Ok(())
}

fn measure_release_immutable_tree(root: &Path) -> AdmResult<(u64, u64, String)> {
    fn collect(
        root: &Path,
        current: &Path,
        records: &mut Vec<(String, u64, String)>,
    ) -> AdmResult<()> {
        for entry in fs::read_dir(current)? {
            let entry = entry?;
            let path = entry.path();
            let metadata = fs::symlink_metadata(&path)?;
            if release_metadata_is_reparse_or_symlink(&metadata) {
                return Err(AdmError::new(format!(
                    "immutable portable tree contains a symlink: {}",
                    path.display()
                )));
            }
            let relative = path
                .strip_prefix(root)
                .map_err(|_| AdmError::new("immutable portable path escaped its root"))?
                .to_string_lossy()
                .replace('\\', "/");
            if relative == "user_data" || relative.starts_with("user_data/") {
                continue;
            }
            if metadata.is_dir() {
                collect(root, &path, records)?;
            } else if metadata.is_file() && relative != ".portable-update.lock" {
                let content = fs::read(&path)?;
                let bytes = u64::try_from(content.len())
                    .map_err(|_| AdmError::new("immutable portable file size overflow"))?;
                records.push((relative, bytes, sha256_hex(&content)));
            } else if !metadata.is_file() {
                return Err(AdmError::new(format!(
                    "immutable portable tree contains an unsupported entry: {}",
                    path.display()
                )));
            }
        }
        Ok(())
    }

    let metadata = fs::symlink_metadata(root)?;
    if release_metadata_is_reparse_or_symlink(&metadata) || !metadata.is_dir() {
        return Err(AdmError::new(
            "immutable portable root is not a regular directory",
        ));
    }
    let mut records = Vec::new();
    collect(root, root, &mut records)?;
    records.sort_by(|left, right| {
        left.0
            .to_ascii_lowercase()
            .cmp(&right.0.to_ascii_lowercase())
            .then_with(|| left.0.cmp(&right.0))
    });
    let mut total_bytes = 0_u64;
    let lines = records
        .iter()
        .map(|(path, bytes, hash)| {
            total_bytes = total_bytes
                .checked_add(*bytes)
                .ok_or_else(|| AdmError::new("immutable portable tree size overflow"))?;
            Ok(format!("{path}|{bytes}|{hash}"))
        })
        .collect::<AdmResult<Vec<_>>>()?;
    let files = u64::try_from(records.len())
        .map_err(|_| AdmError::new("immutable portable file count overflow"))?;
    Ok((files, total_bytes, sha256_hex(lines.join("\n").as_bytes())))
}

#[cfg(windows)]
fn release_metadata_is_reparse_or_symlink(metadata: &fs::Metadata) -> bool {
    const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x0400;
    metadata.file_type().is_symlink()
        || metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0
}

#[cfg(not(windows))]
fn release_metadata_is_reparse_or_symlink(metadata: &fs::Metadata) -> bool {
    metadata.file_type().is_symlink()
}

fn validate_portable_transaction_contract(
    repo_root: &Path,
    evidence: &StandaloneReleaseEvidence,
    portable: &ReleasePortableEvidence,
    receipt: &Value,
    build_manifest: &Value,
) -> Vec<String> {
    let mut blockers = Vec::new();
    let transaction_id = string_value(receipt, "transaction_id");
    if receipt.get("schema_version").and_then(Value::as_u64) != Some(1)
        || string_value(receipt, "kind") != "portable-swap-transaction"
    {
        blockers.push("release_portable_swap_receipt_identity_invalid".to_string());
    }
    if transaction_id != portable.transaction_id
        || string_value(build_manifest, "transaction_id") != portable.transaction_id
    {
        blockers.push("release_portable_transaction_id_mismatch".to_string());
    }
    if string_value(receipt, "status") != "finalized"
        || portable.transaction_status != "finalized"
        || string_value(receipt, "smoke_status") != "passed"
        || string_value(receipt, "finalized_at_utc").trim().is_empty()
    {
        blockers.push("release_portable_transaction_not_finalized".to_string());
    }
    if string_value(receipt, "output_name") != RELEASE_PORTABLE_OUTPUT_NAME
        || string_value(receipt, "release_mode") != "formal"
    {
        blockers.push("release_portable_transaction_output_invalid".to_string());
    }
    let dist = repo_root.join("dist");
    let live = repo_root.join(RELEASE_PORTABLE_ROOT);
    let stage = dist.join(format!(
        ".{RELEASE_PORTABLE_OUTPUT_NAME}.stage-{}",
        portable.transaction_id
    ));
    let previous = dist.join(format!(
        ".{RELEASE_PORTABLE_OUTPUT_NAME}.previous-{}",
        portable.transaction_id
    ));
    let failed = dist.join(format!(
        ".{RELEASE_PORTABLE_OUTPUT_NAME}.failed-{}",
        portable.transaction_id
    ));
    for (field, expected, code) in [
        (
            "dist_root",
            dist.as_path(),
            "release_portable_transaction_dist_root_mismatch",
        ),
        (
            "live_root",
            live.as_path(),
            "release_portable_transaction_live_root_mismatch",
        ),
        (
            "stage_root",
            stage.as_path(),
            "release_portable_transaction_stage_root_mismatch",
        ),
        (
            "backup_root",
            previous.as_path(),
            "release_portable_transaction_backup_root_mismatch",
        ),
        (
            "failed_root",
            failed.as_path(),
            "release_portable_transaction_failed_root_mismatch",
        ),
    ] {
        let actual = string_value(receipt, field);
        if actual.is_empty() || !same_path(Path::new(&actual), expected) {
            blockers.push(code.to_string());
        }
    }
    for (field, suffix, code) in [
        (
            "backup_tombstone_root",
            "retired-backup",
            "release_portable_transaction_backup_tombstone_root_mismatch",
        ),
        (
            "failed_tombstone_root",
            "retired-failed",
            "release_portable_transaction_failed_tombstone_root_mismatch",
        ),
    ] {
        let actual = string_value(receipt, field);
        let expected = dist.join(format!(
            ".{RELEASE_PORTABLE_OUTPUT_NAME}.{suffix}-{}",
            portable.transaction_id
        ));
        if actual.is_empty() || !same_path(Path::new(&actual), &expected) {
            blockers.push(code.to_string());
        }
    }
    let immutable = receipt.get("staged_immutable_tree");
    if immutable
        .and_then(|value| value.get("Exists"))
        .and_then(Value::as_bool)
        != Some(true)
        || immutable
            .and_then(|value| value.get("FileCount"))
            .and_then(Value::as_u64)
            .is_none()
        || immutable
            .and_then(|value| value.get("Bytes"))
            .and_then(Value::as_u64)
            .is_none()
        || immutable
            .and_then(|value| value.get("Digest"))
            .and_then(Value::as_str)
            .is_none_or(|digest| !is_sha256(digest))
    {
        blockers.push("release_portable_transaction_immutable_tree_invalid".to_string());
    }
    let staged_user_data = receipt.get("staged_user_data");
    if staged_user_data
        .and_then(|value| value.get("Exists"))
        .and_then(Value::as_bool)
        != Some(true)
        || staged_user_data
            .and_then(|value| value.get("FileCount"))
            .and_then(Value::as_u64)
            .is_none()
        || staged_user_data
            .and_then(|value| value.get("Bytes"))
            .and_then(Value::as_u64)
            .is_none()
        || staged_user_data
            .and_then(|value| value.get("Digest"))
            .and_then(Value::as_str)
            .is_none_or(|digest| !is_sha256(digest))
    {
        blockers.push("release_portable_transaction_user_data_measure_invalid".to_string());
    }
    let had_previous_live = receipt.get("had_previous_live").and_then(Value::as_bool);
    let backup_deleted = receipt.get("backup_deleted").and_then(Value::as_bool);
    if had_previous_live.is_none() || backup_deleted != had_previous_live {
        blockers.push("release_portable_transaction_backup_finalization_invalid".to_string());
    }
    let manifest_commit = string_value(build_manifest, "git_commit");
    if manifest_commit != evidence.git_commit || manifest_commit != portable.git_commit {
        blockers.push("release_portable_transaction_head_mismatch".to_string());
    }
    blockers
}

fn collect_unresolved_portable_output_state(
    repo_root: &Path,
    output_name: &str,
    current_receipt: &str,
) -> AdmResult<Vec<String>> {
    let mut blockers = Vec::new();
    let dist = repo_root.join("dist");
    let live = dist.join(output_name);
    let operation_lock = format!(".{output_name}.operation.lock");
    let receipt_prefix = format!(".{output_name}.swap-");
    let unresolved_prefixes = [
        format!(".{output_name}.stage-"),
        format!(".{output_name}.previous-"),
        format!(".{output_name}.backup-"),
        format!(".{output_name}.failed-"),
        format!(".{output_name}.retired-backup-"),
        format!(".{output_name}.retired-failed-"),
    ];
    let mut current_receipt_count = 0usize;
    let mut current_transaction_id = String::new();
    for entry in fs::read_dir(&dist)? {
        let entry = entry?;
        let name = entry.file_name().to_string_lossy().to_string();
        if unresolved_prefixes
            .iter()
            .any(|prefix| name.starts_with(prefix))
        {
            blockers.push(format!(
                "release_portable_unresolved_output_artifact:{name}"
            ));
            continue;
        }
        if !name.starts_with(&receipt_prefix) || !name.ends_with(".json") {
            continue;
        }
        let relative = format!("dist/{name}");
        let metadata = fs::symlink_metadata(entry.path())?;
        if release_metadata_is_reparse_or_symlink(&metadata) || !metadata.is_file() {
            blockers.push(format!("release_portable_receipt_not_regular:{name}"));
            continue;
        }
        let record: Value = match fs::read(entry.path())
            .ok()
            .and_then(|bytes| serde_json::from_slice(&bytes).ok())
        {
            Some(record) => record,
            None => {
                blockers.push(format!("release_portable_receipt_invalid:{name}"));
                continue;
            }
        };
        let status = string_value(&record, "status");
        let resolved = matches!(status.as_str(), "finalized" | "failure_artifact_finalized");
        let name_transaction_id = name
            .strip_prefix(&receipt_prefix)
            .and_then(|value| value.strip_suffix(".json"))
            .unwrap_or_default();
        let completion_present = match status.as_str() {
            "finalized" => !string_value(&record, "finalized_at_utc").is_empty(),
            "failure_artifact_finalized" => {
                !string_value(&record, "failed_artifact_deleted_at_utc").is_empty()
            }
            _ => false,
        };
        if record.get("schema_version").and_then(Value::as_u64) != Some(1)
            || string_value(&record, "kind") != "portable-swap-transaction"
            || string_value(&record, "output_name") != output_name
            || name_transaction_id.len() != 32
            || !name_transaction_id
                .bytes()
                .all(|byte| byte.is_ascii_hexdigit())
            || string_value(&record, "transaction_id") != name_transaction_id
            || !resolved
            || !completion_present
        {
            blockers.push(format!("release_portable_unresolved_receipt:{name}"));
        }
        if relative == current_receipt {
            current_receipt_count += 1;
            current_transaction_id = string_value(&record, "transaction_id");
            if string_value(&record, "status") != "finalized" {
                blockers.push("release_portable_current_receipt_not_finalized".to_string());
            }
        }
    }
    if current_receipt_count != 1 {
        blockers.push("release_portable_current_receipt_count_invalid".to_string());
    }
    let operation_lock_path = dist.join(&operation_lock);
    match fs::symlink_metadata(&operation_lock_path) {
        Ok(metadata)
            if release_metadata_is_reparse_or_symlink(&metadata) || !metadata.is_file() =>
        {
            blockers.push("release_portable_operation_lock_not_regular".to_string());
        }
        Ok(_) => {
            // The portable coordinator opens this file with FileShare.None; an
            // active operation therefore makes this read fail closed on Windows.
            match fs::read(&operation_lock_path)
                .ok()
                .and_then(|bytes| serde_json::from_slice::<Value>(&bytes).ok())
            {
                Some(lock)
                    if lock.get("schema_version").and_then(Value::as_u64) == Some(1)
                        && string_value(&lock, "kind") == "portable-output-operation-lock"
                        && string_value(&lock, "output_name") == output_name
                        && string_value(&lock, "transaction_id") == current_transaction_id => {}
                _ => blockers.push("release_portable_operation_lock_active_or_stale".to_string()),
            }
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => {
            return Err(AdmError::new(format!(
                "portable operation lock metadata unavailable: {error}"
            )));
        }
    }
    if live.join(".portable-update.lock").exists() {
        blockers.push("release_portable_live_update_lock_present".to_string());
    }
    Ok(blockers)
}

fn git_output(repo_root: &Path, args: &[&str]) -> Option<String> {
    let output = Command::new("git")
        .arg("-C")
        .arg(repo_root)
        .args(args)
        .output()
        .ok()?;
    output
        .status
        .success()
        .then(|| String::from_utf8_lossy(&output.stdout).trim().to_string())
}

pub fn handoff_report(_repo_root: &Path) -> AdmResult<GateReport> {
    Ok(deprecated_gate_report(
        "Legacy Development Handoff Report",
        LEGACY_HANDOFF_GATE_DEPRECATED,
    ))
}

pub fn final_handoff_v3_gate_report(_repo_root: &Path) -> AdmResult<GateReport> {
    Ok(deprecated_gate_report(
        "Legacy File-Level Handoff Gate",
        LEGACY_HANDOFF_GATE_DEPRECATED,
    ))
}

#[allow(dead_code)]
fn legacy_final_handoff_v3_gate_report(repo_root: &Path) -> AdmResult<GateReport> {
    let mut report = GateReport::new("NEWrust v3 Final File-Level Handoff Gate");
    report.add_row(
        "source_contract",
        "docs/migration-history/full_project_reproduction/final_handoff_v3.md; file-level matrices; A00-A39 gates",
    );
    report.add_row(
        "required_execution",
        "cargo run -p adm-new-cli -- release-gate; cargo run -p adm-new-cli -- final-handoff-v3-gate; cargo test --workspace; npm test; npm run build",
    );

    let plan_root = repo_root
        .join("docs")
        .join("migration-history")
        .join("full_project_reproduction");
    let inventory = analyze_final_inventory_matrix(
        "inventory",
        "docs/migration-history/full_project_reproduction/01_full_python_file_inventory.md",
        &repo_root.join(
            "docs/migration-history/full_project_reproduction/01_full_python_file_inventory.md",
        ),
    )?;
    let disposition = analyze_final_disposition_matrix(
        "disposition",
        "docs/migration-history/full_project_reproduction/03_file_disposition_matrix.md",
        &repo_root
            .join("docs/migration-history/full_project_reproduction/03_file_disposition_matrix.md"),
    )?;
    let mapping = analyze_final_mapping_matrix(
        "rust_target_mapping",
        "docs/migration-history/full_project_reproduction/04_rust_target_mapping.md",
        &repo_root
            .join("docs/migration-history/full_project_reproduction/04_rust_target_mapping.md"),
    )?;
    let test_matrix = analyze_final_test_matrix(
        "test_migration",
        "docs/migration-history/full_project_reproduction/09_test_migration_matrix.md",
        &repo_root
            .join("docs/migration-history/full_project_reproduction/09_test_migration_matrix.md"),
    )?;

    add_final_matrix_rows(
        &mut report,
        &inventory,
        REQUIRED_FULL_PROJECT_PYTHON_FILE_COUNT,
    );
    add_final_matrix_rows(
        &mut report,
        &disposition,
        REQUIRED_FULL_PROJECT_PYTHON_FILE_COUNT,
    );
    add_final_matrix_rows(
        &mut report,
        &mapping,
        REQUIRED_FULL_PROJECT_PYTHON_FILE_COUNT,
    );
    add_final_matrix_rows(
        &mut report,
        &test_matrix,
        REQUIRED_FULL_PROJECT_TEST_MIGRATION_COUNT,
    );

    let data_asset_count =
        data_asset_migration_total(&plan_root.join("11_data_asset_migration_matrix.md"))?;
    report.add_row("data_asset_migration_count", data_asset_count.to_string());
    if data_asset_count != REQUIRED_DATA_ASSET_MIGRATION_COUNT {
        report.add_blocker(format!(
            "data_asset_migration_count_mismatch:{data_asset_count}"
        ));
    }

    let ui_baseline_count =
        ui_baseline_record_count(&plan_root.join("ui_baselines").join("index.json"))?;
    report.add_row("ui_baseline_record_count", ui_baseline_count.to_string());
    if ui_baseline_count != REQUIRED_UI_PARITY_V3_RECORD_COUNT {
        report.add_blocker(format!(
            "ui_baseline_record_count_mismatch:{ui_baseline_count}"
        ));
    }

    let completed_atoms =
        completed_atoms_before_a40(&plan_root.join("10_v3_atomic_development_plan.md"))?;
    report.add_row(
        "a00_a39_completed_atom_count",
        completed_atoms.len().to_string(),
    );
    for index in 0..REQUIRED_FINAL_HANDOFF_V3_COMPLETED_ATOMS {
        let atom = format!("A{index:02}");
        if !completed_atoms.contains(&atom) {
            report.add_blocker(format!("final_handoff_atom_not_completed:{atom}"));
        }
    }

    let gate_statuses = final_handoff_gate_statuses(repo_root)?;
    for status in &gate_statuses {
        report.add_row(
            format!("gate:{}:status", status.relative_path),
            status.status.clone(),
        );
        report.add_row(
            format!("gate:{}:hash", status.relative_path),
            status.hash.clone(),
        );
        if status.status != "passed" {
            report.add_blocker(format!(
                "final_handoff_required_gate_not_passed:{}:{}",
                status.relative_path, status.status
            ));
        }
    }

    if inventory.row_count != REQUIRED_FULL_PROJECT_PYTHON_FILE_COUNT {
        report.add_blocker(format!(
            "inventory_row_count_mismatch:{}",
            inventory.row_count
        ));
    }
    if disposition.row_count != REQUIRED_FULL_PROJECT_PYTHON_FILE_COUNT {
        report.add_blocker(format!(
            "disposition_row_count_mismatch:{}",
            disposition.row_count
        ));
    }
    if mapping.row_count != REQUIRED_FULL_PROJECT_PYTHON_FILE_COUNT {
        report.add_blocker(format!("mapping_row_count_mismatch:{}", mapping.row_count));
    }
    if test_matrix.row_count != REQUIRED_FULL_PROJECT_TEST_MIGRATION_COUNT {
        report.add_blocker(format!(
            "test_migration_row_count_mismatch:{}",
            test_matrix.row_count
        ));
    }
    for matrix in [&disposition, &mapping, &test_matrix] {
        if matrix.pending_count > 0 {
            report.add_blocker(format!(
                "{}_has_pending_or_undecided_rows:{}",
                matrix.name, matrix.pending_count
            ));
        }
        if matrix.missing_target_count > 0 {
            report.add_blocker(format!(
                "{}_has_missing_targets:{}",
                matrix.name, matrix.missing_target_count
            ));
        }
    }

    let markdown = render_final_handoff_v3_markdown(
        &inventory,
        &disposition,
        &mapping,
        &test_matrix,
        data_asset_count,
        ui_baseline_count,
        &completed_atoms,
        &gate_statuses,
    );
    let final_path = plan_root.join("final_handoff_v3.md");
    write_text_atomic(&final_path, &markdown)?;
    report.add_row(
        "final_handoff_v3_path",
        "docs/migration-history/full_project_reproduction/final_handoff_v3.md",
    );
    report.add_row("final_handoff_v3_hash", hash_text(&markdown));
    Ok(report)
}

#[allow(dead_code)]
fn build_handoff_manifest(repo_root: &Path) -> AdmResult<HandoffManifest> {
    let entries = handoff_entries();
    let mut blockers = Vec::new();
    for entry in &entries {
        validate_handoff_entry(repo_root, entry, &mut blockers)?;
    }
    let status = if blockers.is_empty() {
        "passed"
    } else {
        "blocked"
    };
    Ok(HandoffManifest {
        schema_version: 1,
        generated_at: format!("unix:{}", unix_timestamp()),
        status: status.to_string(),
        plan_root: "docs/migration-history/NEWrust".to_string(),
        newrust_root: ".".to_string(),
        gate_report_dir: "gates".to_string(),
        entries,
        blockers,
    })
}

#[allow(dead_code)]
fn validate_handoff_entry(
    repo_root: &Path,
    entry: &HandoffEntry,
    blockers: &mut Vec<String>,
) -> AdmResult<()> {
    if entry.python_evidence.is_empty() {
        blockers.push(format!("{}:missing_python_evidence_refs", entry.feature_id));
    }
    if entry.newrust_files.is_empty() {
        blockers.push(format!("{}:missing_newrust_file_refs", entry.feature_id));
    }
    if entry.tests.is_empty() {
        blockers.push(format!("{}:missing_test_refs", entry.feature_id));
    }
    if entry.gate_refs.is_empty() {
        blockers.push(format!("{}:missing_gate_refs", entry.feature_id));
    }
    for relative in &entry.python_evidence {
        if !repo_root.join(relative).is_file() {
            blockers.push(format!(
                "{}:python_evidence_missing:{}",
                entry.feature_id, relative
            ));
        }
    }
    for relative in &entry.newrust_files {
        if !repo_root.join(relative).is_file() {
            blockers.push(format!(
                "{}:newrust_file_missing:{}",
                entry.feature_id, relative
            ));
        }
    }
    for relative in &entry.gate_refs {
        let path = repo_root.join(relative);
        if !path.is_file() {
            blockers.push(format!(
                "{}:gate_ref_missing:{}",
                entry.feature_id, relative
            ));
            continue;
        }
        if path.extension().and_then(|value| value.to_str()) == Some("adm") {
            let text = fs::read_to_string(&path)?;
            if !text.contains("status=passed") {
                blockers.push(format!(
                    "{}:gate_ref_not_passed:{}",
                    entry.feature_id, relative
                ));
            }
        }
    }
    Ok(())
}

#[allow(dead_code)]
fn handoff_entries() -> Vec<HandoffEntry> {
    vec![
        handoff_entry(
            "source_authority",
            "Python source authority and garbage isolation",
            &[
                "docs/migration-history/python_deconstruction/01_source_authority_index.md",
                "docs/migration-history/python_deconstruction/18_garbage_isolation_draft.md",
            ],
            &[
                "crates/adm-new-governance/src/lib.rs",
                "apps/adm-new-cli/src/main.rs",
            ],
            &["cargo run -p adm-new-cli -- plan-gate"],
            &["gates/plan-gate.adm"],
        ),
        handoff_entry(
            "data_contracts",
            "Typed data contracts and storage",
            &[
                "docs/migration-history/python_deconstruction/04_data_model_and_storage.md",
                "docs/migration-history/python_deconstruction/13_save_and_execution_object_contracts.md",
                "docs/migration-history/python_deconstruction/20_parity_gate_test_matrix.md",
            ],
            &[
                "crates/adm-new-contracts/src/project.rs",
                "crates/adm-new-contracts/src/save.rs",
                "crates/adm-new-contracts/src/package.rs",
                "crates/adm-new-storage/src/lib.rs",
            ],
            &[
                "cargo test -p adm-new-contracts",
                "cargo test -p adm-new-storage",
            ],
            &["gates/parity-gate.adm"],
        ),
        handoff_entry(
            "design_workbench",
            "Design workbench parity",
            &[
                "docs/migration-history/python_deconstruction/11_design_engine_contracts.md",
                "docs/migration-history/python_deconstruction/19_ui_reproduction_specs.md",
            ],
            &[
                "crates/adm-new-design/src/lib.rs",
                "crates/adm-new-application/src/lib.rs",
                "crates/adm-new-tauri-commands/src/design.rs",
                "web/src/features/design.js",
            ],
            &[
                "cargo test -p adm-new-design",
                "cargo test -p adm-new-tauri-commands design",
                "npm.cmd run e2e -- design",
            ],
            &["gates/ui-gate.adm", "gates/ui-evidence-manifest.json"],
        ),
        handoff_entry(
            "ai_config_interview",
            "AI config and embedded interview parity",
            &[
                "docs/migration-history/python_deconstruction/14_ai_config_adapter_log_contracts.md",
                "docs/migration-history/python_deconstruction/16_ai_interview_and_completion_contracts.md",
                "docs/migration-history/python_deconstruction/19_ui_reproduction_specs.md",
            ],
            &[
                "crates/adm-new-ai/src/lib.rs",
                "crates/adm-new-tauri-commands/src/ai.rs",
                "crates/adm-new-tauri-commands/src/config.rs",
                "web/src/features/ai-config.js",
            ],
            &[
                "cargo test -p adm-new-ai",
                "cargo test -p adm-new-tauri-commands ai",
                "cargo test -p adm-new-tauri-commands config",
                "npm.cmd run e2e -- ai-config",
            ],
            &["gates/ui-gate.adm", "gates/parity-gate.adm"],
        ),
        handoff_entry(
            "pipeline_artifacts",
            "Pipeline runtime and artifact gates",
            &[
                "docs/migration-history/python_deconstruction/07_pipeline_step_contracts.md",
                "docs/migration-history/python_deconstruction/09_artifact_validation_flow.md",
                "docs/migration-history/python_deconstruction/15_artifact_schema_refs_map.md",
            ],
            &[
                "crates/adm-new-pipeline/src/lib.rs",
                "crates/adm-new-artifact/src/lib.rs",
                "crates/adm-new-tauri-commands/src/pipeline.rs",
                "web/src/features/pipeline.js",
            ],
            &[
                "cargo test -p adm-new-pipeline",
                "cargo test -p adm-new-artifact",
                "cargo test -p adm-new-tauri-commands pipeline",
                "npm.cmd run e2e -- pipeline",
            ],
            &["gates/parity-gate.adm", "gates/ui-gate.adm"],
        ),
        handoff_entry(
            "save_runtime",
            "Save archive, locks, snapshots, and runtime state",
            &[
                "docs/migration-history/python_deconstruction/13_save_and_execution_object_contracts.md",
                "docs/migration-history/python_deconstruction/08_runtime_save_ai_package_flow.md",
            ],
            &[
                "crates/adm-new-save/src/lib.rs",
                "crates/adm-new-storage/src/lib.rs",
                "crates/adm-new-tauri-commands/src/save.rs",
            ],
            &[
                "cargo test -p adm-new-save",
                "cargo test -p adm-new-storage",
                "cargo test -p adm-new-tauri-commands save",
            ],
            &["gates/parity-gate.adm"],
        ),
        handoff_entry(
            "utility_panels",
            "Patch, package, logs, and SDK utility panels",
            &[
                "docs/migration-history/python_deconstruction/14_ai_config_adapter_log_contracts.md",
                "docs/migration-history/python_deconstruction/17_packaging_contracts.md",
                "docs/migration-history/python_deconstruction/20_parity_gate_test_matrix.md",
            ],
            &[
                "crates/adm-new-patch/src/lib.rs",
                "crates/adm-new-sdk/src/lib.rs",
                "crates/adm-new-packaging/src/lib.rs",
                "crates/adm-new-tauri-commands/src/package.rs",
                "crates/adm-new-tauri-commands/src/logs.rs",
                "crates/adm-new-tauri-commands/src/sdk.rs",
                "web/src/features/utility-panels.js",
            ],
            &[
                "cargo test -p adm-new-patch",
                "cargo test -p adm-new-sdk",
                "cargo test -p adm-new-packaging",
                "cargo test -p adm-new-tauri-commands package",
                "npm.cmd run e2e -- utility-panels",
            ],
            &["gates/package-gate.adm", "gates/ui-gate.adm"],
        ),
        handoff_entry(
            "ui_parity",
            "Web UI pixel and interaction parity",
            &[
                "docs/migration-history/python_deconstruction/19_ui_reproduction_specs.md",
                "docs/migration-history/python_deconstruction/20_parity_gate_test_matrix.md",
            ],
            &[
                "web/src/index.html",
                "web/src/main.js",
                "web/src/styles.css",
                "web/scripts/e2e.mjs",
                "web/scripts/ui-gate.mjs",
            ],
            &[
                "npm.cmd run build",
                "npm.cmd run test",
                "npm.cmd run e2e",
                "npm.cmd run ui-gate",
            ],
            &["gates/ui-gate.adm", "gates/ui-evidence-manifest.json"],
        ),
        handoff_entry(
            "release_governance",
            "Plan, parity, package, and release gates",
            &[
                "docs/migration-history/newrust_design/09_testing_gates_release_design.md",
                "docs/migration-history/newrust_design/10_risk_register.md",
                "docs/migration-history/06_validation_and_release_gates.md",
            ],
            &[
                "crates/adm-new-governance/src/lib.rs",
                "apps/adm-new-cli/src/main.rs",
            ],
            &[
                "cargo run -p adm-new-cli -- plan-gate",
                "cargo run -p adm-new-cli -- parity-gate",
                "cargo run -p adm-new-cli -- package-gate",
                "cargo run -p adm-new-cli -- release-gate",
            ],
            &[
                "gates/plan-gate.adm",
                "gates/parity-gate.adm",
                "gates/package-gate.adm",
                "gates/release-gate.adm",
            ],
        ),
    ]
}

#[allow(dead_code)]
fn handoff_entry(
    feature_id: &str,
    feature_name: &str,
    python_evidence: &[&str],
    newrust_files: &[&str],
    tests: &[&str],
    gate_refs: &[&str],
) -> HandoffEntry {
    HandoffEntry {
        feature_id: feature_id.to_string(),
        feature_name: feature_name.to_string(),
        python_evidence: python_evidence
            .iter()
            .map(|value| (*value).to_string())
            .collect(),
        newrust_files: newrust_files
            .iter()
            .map(|value| (*value).to_string())
            .collect(),
        tests: tests.iter().map(|value| (*value).to_string()).collect(),
        gate_refs: gate_refs.iter().map(|value| (*value).to_string()).collect(),
        evidence_level: "local_release".to_string(),
        status: "covered".to_string(),
    }
}

#[allow(dead_code)]
fn render_handoff_markdown(manifest: &HandoffManifest) -> String {
    let mut text = String::new();
    text.push_str("# NEWrust Final Handoff Manifest\n\n");
    text.push_str(&format!("- status: {}\n", manifest.status));
    text.push_str(&format!("- generated_at: {}\n", manifest.generated_at));
    text.push_str(&format!("- entry_count: {}\n", manifest.entries.len()));
    text.push_str(&format!("- blocker_count: {}\n\n", manifest.blockers.len()));
    if !manifest.blockers.is_empty() {
        text.push_str("## Blockers\n\n");
        for blocker in &manifest.blockers {
            text.push_str(&format!("- {blocker}\n"));
        }
        text.push('\n');
    }
    text.push_str("## Evidence Map\n\n");
    text.push_str("| feature | status | python evidence | NEWrust files | tests | gates |\n");
    text.push_str("| --- | --- | ---: | ---: | ---: | ---: |\n");
    for entry in &manifest.entries {
        text.push_str(&format!(
            "| {} | {} | {} | {} | {} | {} |\n",
            entry.feature_id,
            entry.status,
            entry.python_evidence.len(),
            entry.newrust_files.len(),
            entry.tests.len(),
            entry.gate_refs.len()
        ));
    }
    text.push_str("\n## Details\n\n");
    for entry in &manifest.entries {
        text.push_str(&format!("### {}\n\n", entry.feature_name));
        text.push_str("Python evidence:\n");
        for item in &entry.python_evidence {
            text.push_str(&format!("- `{item}`\n"));
        }
        text.push_str("\nNEWrust files:\n");
        for item in &entry.newrust_files {
            text.push_str(&format!("- `{item}`\n"));
        }
        text.push_str("\nTests:\n");
        for item in &entry.tests {
            text.push_str(&format!("- `{item}`\n"));
        }
        text.push_str("\nGates:\n");
        for item in &entry.gate_refs {
            text.push_str(&format!("- `{item}`\n"));
        }
        text.push('\n');
    }
    text
}

fn analyze_final_inventory_matrix(
    name: &'static str,
    relative_path: &'static str,
    path: &Path,
) -> AdmResult<FinalMatrixSummary> {
    let text = fs::read_to_string(path)?;
    let rows = markdown_table_rows(&text)
        .into_iter()
        .filter(|columns| matrix_path_cell_is_python(columns.first()))
        .collect::<Vec<_>>();
    Ok(FinalMatrixSummary {
        name,
        relative_path,
        row_count: rows.len(),
        decided_count: rows.len(),
        pending_count: 0,
        drop_with_reason_count: 0,
        missing_target_count: 0,
    })
}

fn analyze_final_disposition_matrix(
    name: &'static str,
    relative_path: &'static str,
    path: &Path,
) -> AdmResult<FinalMatrixSummary> {
    let text = fs::read_to_string(path)?;
    let mut row_count = 0usize;
    let mut decided_count = 0usize;
    let mut pending_count = 0usize;
    let mut drop_with_reason_count = 0usize;
    let mut missing_target_count = 0usize;
    for columns in markdown_table_rows(&text) {
        if !matrix_path_cell_is_python(columns.first()) || columns.len() < 6 {
            continue;
        }
        row_count += 1;
        let final_disposition = columns.get(2).map(String::as_str).unwrap_or_default();
        let target = columns.get(3).map(String::as_str).unwrap_or_default();
        let status = columns.get(5).map(String::as_str).unwrap_or_default();
        if status == "decided" {
            decided_count += 1;
        } else {
            pending_count += 1;
        }
        if final_disposition == "drop_with_reason" {
            drop_with_reason_count += 1;
        } else if target.trim().is_empty() || target == "none" {
            missing_target_count += 1;
        }
        if final_disposition_pending(final_disposition) {
            pending_count += 1;
        }
    }
    Ok(FinalMatrixSummary {
        name,
        relative_path,
        row_count,
        decided_count,
        pending_count,
        drop_with_reason_count,
        missing_target_count,
    })
}

fn analyze_final_mapping_matrix(
    name: &'static str,
    relative_path: &'static str,
    path: &Path,
) -> AdmResult<FinalMatrixSummary> {
    let text = fs::read_to_string(path)?;
    let mut row_count = 0usize;
    let mut decided_count = 0usize;
    let mut pending_count = 0usize;
    let mut drop_with_reason_count = 0usize;
    let mut missing_target_count = 0usize;
    for columns in markdown_table_rows(&text) {
        if !matrix_path_cell_is_python(columns.first()) || columns.len() < 6 {
            continue;
        }
        row_count += 1;
        let final_disposition = columns.get(1).map(String::as_str).unwrap_or_default();
        let target = columns.get(2).map(String::as_str).unwrap_or_default();
        let status = columns.get(5).map(String::as_str).unwrap_or_default();
        if status == "decided" {
            decided_count += 1;
        } else {
            pending_count += 1;
        }
        if final_disposition == "drop_with_reason" {
            drop_with_reason_count += 1;
        } else if target.trim().is_empty() || target == "none" {
            missing_target_count += 1;
        }
        if final_disposition_pending(final_disposition) {
            pending_count += 1;
        }
    }
    Ok(FinalMatrixSummary {
        name,
        relative_path,
        row_count,
        decided_count,
        pending_count,
        drop_with_reason_count,
        missing_target_count,
    })
}

fn analyze_final_test_matrix(
    name: &'static str,
    relative_path: &'static str,
    path: &Path,
) -> AdmResult<FinalMatrixSummary> {
    let text = fs::read_to_string(path)?;
    let mut row_count = 0usize;
    let mut decided_count = 0usize;
    let mut pending_count = 0usize;
    let mut missing_target_count = 0usize;
    for columns in markdown_table_rows(&text) {
        let Some(python_file) = columns.first() else {
            continue;
        };
        if !normalize_matrix_path(python_file).contains("core\\tests\\") || columns.len() < 5 {
            continue;
        }
        row_count += 1;
        let target = columns.get(1).map(String::as_str).unwrap_or_default();
        let status = columns.get(4).map(String::as_str).unwrap_or_default();
        if status == "decided" {
            decided_count += 1;
        } else {
            pending_count += 1;
        }
        if target.trim().is_empty() || target == "none" {
            missing_target_count += 1;
        }
    }
    Ok(FinalMatrixSummary {
        name,
        relative_path,
        row_count,
        decided_count,
        pending_count,
        drop_with_reason_count: 0,
        missing_target_count,
    })
}

fn add_final_matrix_rows(
    report: &mut GateReport,
    summary: &FinalMatrixSummary,
    expected_count: usize,
) {
    report.add_row(
        format!("matrix:{}:path", summary.name),
        summary.relative_path.to_string(),
    );
    report.add_row(
        format!("matrix:{}:row_count", summary.name),
        summary.row_count.to_string(),
    );
    report.add_row(
        format!("matrix:{}:expected_count", summary.name),
        expected_count.to_string(),
    );
    report.add_row(
        format!("matrix:{}:decided_count", summary.name),
        summary.decided_count.to_string(),
    );
    report.add_row(
        format!("matrix:{}:pending_count", summary.name),
        summary.pending_count.to_string(),
    );
    report.add_row(
        format!("matrix:{}:drop_with_reason_count", summary.name),
        summary.drop_with_reason_count.to_string(),
    );
    report.add_row(
        format!("matrix:{}:missing_target_count", summary.name),
        summary.missing_target_count.to_string(),
    );
}

fn markdown_table_rows(text: &str) -> Vec<Vec<String>> {
    text.lines()
        .filter_map(|line| {
            let trimmed = line.trim();
            if !trimmed.starts_with('|') {
                return None;
            }
            let columns = trimmed
                .trim_matches('|')
                .split('|')
                .map(markdown_cell_text)
                .collect::<Vec<_>>();
            if columns.iter().all(|column| {
                !column.is_empty() && column.chars().all(|ch| matches!(ch, '-' | ':' | ' '))
            }) {
                return None;
            }
            Some(columns)
        })
        .collect()
}

fn matrix_path_cell_is_python(value: Option<&String>) -> bool {
    value
        .map(|cell| normalize_matrix_path(cell).ends_with(".py"))
        .unwrap_or(false)
}

fn final_disposition_pending(value: &str) -> bool {
    let normalized = value.trim().to_ascii_lowercase();
    normalized.is_empty()
        || normalized == "pending"
        || normalized == "partial"
        || normalized == "defer"
        || normalized == "unclassified"
        || normalized.ends_with("_pending")
}

fn data_asset_migration_total(path: &Path) -> AdmResult<usize> {
    let text = fs::read_to_string(path)?;
    for line in text.lines() {
        if !line.contains("Total in this matrix:") {
            continue;
        }
        let digits = line
            .chars()
            .filter(|ch| ch.is_ascii_digit())
            .collect::<String>();
        if !digits.is_empty() {
            return digits
                .parse::<usize>()
                .map_err(|error| AdmError::new(format!("invalid data asset total: {error}")));
        }
    }
    Err(AdmError::new(
        "11_data_asset_migration_matrix.md missing total count",
    ))
}

fn ui_baseline_record_count(path: &Path) -> AdmResult<usize> {
    let text = fs::read_to_string(path)?;
    let value: Value = serde_json::from_str(&text)
        .map_err(|error| AdmError::new(format!("invalid ui baseline index JSON: {error}")))?;
    Ok(value
        .get("records")
        .and_then(Value::as_array)
        .map(Vec::len)
        .unwrap_or(0))
}

fn completed_atoms_before_a40(path: &Path) -> AdmResult<BTreeSet<String>> {
    let text = fs::read_to_string(path)?;
    let mut completed = BTreeSet::new();
    for columns in markdown_table_rows(&text) {
        let Some(atom) = columns.first() else {
            continue;
        };
        if !is_atom_id(atom) || atom == "A40" {
            continue;
        }
        let status = columns.get(1).map(String::as_str).unwrap_or_default();
        if status == "completed" {
            completed.insert(atom.clone());
        }
    }
    Ok(completed)
}

fn is_atom_id(value: &str) -> bool {
    let bytes = value.as_bytes();
    bytes.len() == 3 && bytes[0] == b'A' && bytes[1].is_ascii_digit() && bytes[2].is_ascii_digit()
}

fn final_handoff_gate_statuses(repo_root: &Path) -> AdmResult<Vec<FinalGateStatus>> {
    let mut statuses = Vec::new();
    for relative_path in REQUIRED_FINAL_HANDOFF_V3_GATE_REFS {
        let path = repo_root.join(relative_path);
        if !path.is_file() {
            statuses.push(FinalGateStatus {
                relative_path,
                status: "missing".to_string(),
                hash: "missing".to_string(),
            });
            continue;
        }
        let text = fs::read_to_string(&path)?;
        let status = text
            .lines()
            .find_map(|line| line.strip_prefix("status="))
            .unwrap_or("missing_status")
            .to_string();
        statuses.push(FinalGateStatus {
            relative_path,
            status,
            hash: hash_text(&text),
        });
    }
    Ok(statuses)
}

fn render_final_handoff_v3_markdown(
    inventory: &FinalMatrixSummary,
    disposition: &FinalMatrixSummary,
    mapping: &FinalMatrixSummary,
    test_matrix: &FinalMatrixSummary,
    data_asset_count: usize,
    ui_baseline_count: usize,
    completed_atoms: &BTreeSet<String>,
    gate_statuses: &[FinalGateStatus],
) -> String {
    let mut text = String::new();
    text.push_str("# NEWrust v3 Final Handoff Evidence\n\n");
    text.push_str(&format!("- generated_at: unix:{}\n", unix_timestamp()));
    text.push_str("- status: local_file_level_complete\n");
    text.push_str("- scope: full-project Rust/Web/Tauri reproduction evidence\n");
    text.push_str("- note: legacy feature-domain handoff is evidence only; this v3 handoff is file-level.\n\n");

    text.push_str("## File-Level Implementation Status\n\n");
    text.push_str("| axis | actual | required | decided | pending | accepted drops | missing targets | evidence |\n");
    text.push_str("| --- | ---: | ---: | ---: | ---: | ---: | ---: | --- |\n");
    text.push_str(&format!(
        "| Python file inventory rows | {} | {} | {} | {} | {} | {} | `{}` |\n",
        inventory.row_count,
        REQUIRED_FULL_PROJECT_PYTHON_FILE_COUNT,
        inventory.decided_count,
        inventory.pending_count,
        inventory.drop_with_reason_count,
        inventory.missing_target_count,
        inventory.relative_path
    ));
    text.push_str(&format!(
        "| Final disposition rows | {} | {} | {} | {} | {} | {} | `{}` |\n",
        disposition.row_count,
        REQUIRED_FULL_PROJECT_PYTHON_FILE_COUNT,
        disposition.decided_count,
        disposition.pending_count,
        disposition.drop_with_reason_count,
        disposition.missing_target_count,
        disposition.relative_path
    ));
    text.push_str(&format!(
        "| Rust target mapping rows | {} | {} | {} | {} | {} | {} | `{}` |\n",
        mapping.row_count,
        REQUIRED_FULL_PROJECT_PYTHON_FILE_COUNT,
        mapping.decided_count,
        mapping.pending_count,
        mapping.drop_with_reason_count,
        mapping.missing_target_count,
        mapping.relative_path
    ));
    text.push_str(&format!(
        "| Test migration rows | {} | {} | {} | {} | {} | {} | `{}` |\n",
        test_matrix.row_count,
        REQUIRED_FULL_PROJECT_TEST_MIGRATION_COUNT,
        test_matrix.decided_count,
        test_matrix.pending_count,
        test_matrix.drop_with_reason_count,
        test_matrix.missing_target_count,
        test_matrix.relative_path
    ));
    text.push_str(&format!(
        "| Data asset migration files | {data_asset_count} | {} | {data_asset_count} | 0 | 0 | 0 | `docs/migration-history/full_project_reproduction/11_data_asset_migration_matrix.md` |\n",
        REQUIRED_DATA_ASSET_MIGRATION_COUNT
    ));
    text.push_str(&format!(
        "| UI baseline records | {ui_baseline_count} | {} | {ui_baseline_count} | 0 | 0 | 0 | `docs/migration-history/full_project_reproduction/ui_baselines/index.json` |\n\n",
        REQUIRED_UI_PARITY_V3_RECORD_COUNT
    ));

    text.push_str("## Atom Closure\n\n");
    text.push_str(&format!(
        "- A00-A39 completed atoms: {} / {}\n",
        completed_atoms.len(),
        REQUIRED_FINAL_HANDOFF_V3_COMPLETED_ATOMS
    ));
    text.push_str("- A40 owns this final handoff evidence and final gate.\n\n");

    text.push_str("## Gate Logs\n\n");
    text.push_str("| gate | status | hash |\n");
    text.push_str("| --- | --- | --- |\n");
    for status in gate_statuses {
        text.push_str(&format!(
            "| `{}` | {} | {} |\n",
            status.relative_path, status.status, status.hash
        ));
    }
    text.push('\n');

    text.push_str("## Known Accepted Deltas\n\n");
    text.push_str(&format!(
        "- Non-behavioral Python package markers or obsolete helpers are accepted only where `03_file_disposition_matrix.md` and `04_rust_target_mapping.md` mark `drop_with_reason`; current accepted drop count is {}.\n",
        disposition.drop_with_reason_count
    ));
    text.push_str("- UI v3 parity has zero approved P0/P1 deltas; desktop and narrow Web screenshots are recorded under `ui_baselines/` and `gates/`.\n");
    text.push_str("- Release status is local deterministic gate pass. This file-level reproduction handoff does not assert live external AI provider credentials or a real Unity Editor runtime beyond the deterministic gates recorded here.\n\n");

    text.push_str("## Release Status\n\n");
    let release_status = gate_statuses
        .iter()
        .find(|status| status.relative_path == "gates/release-gate.adm")
        .map(|status| status.status.as_str())
        .unwrap_or("missing");
    text.push_str(&format!(
        "- release_gate_status: {release_status}\n- final_handoff_gate: generated by `adm-new-cli final-handoff-v3-gate`\n"
    ));
    text
}

fn add_nested_gate_result(report: &mut GateReport, key: &str, nested: GateReport) {
    report.add_row(format!("{key}_status"), nested.status().as_str());
    if !nested.passed() {
        report.add_blocker(format!("{key}_failed"));
    }
}

fn collect_forbidden_evidence_markers(root: &Path) -> AdmResult<Vec<String>> {
    let mut found = Vec::new();
    collect_forbidden_evidence_markers_inner(root, root, &mut found)?;
    Ok(found)
}

fn collect_forbidden_evidence_markers_inner(
    root: &Path,
    current: &Path,
    found: &mut Vec<String>,
) -> AdmResult<()> {
    if !current.is_dir() {
        return Ok(());
    }
    for entry in fs::read_dir(current)? {
        let entry = entry?;
        let path = entry.path();
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if path.is_dir() {
            if matches!(name.as_ref(), "target" | "node_modules" | "dist" | "gates") {
                continue;
            }
            collect_forbidden_evidence_markers_inner(root, &path, found)?;
            continue;
        }
        if !is_text_source_file(&path) {
            continue;
        }
        let text = fs::read_to_string(&path)?;
        let lower = text.to_ascii_lowercase();
        for marker in forbidden_evidence_markers() {
            if lower.contains(&marker) {
                let relative = path
                    .strip_prefix(root)
                    .unwrap_or(&path)
                    .display()
                    .to_string();
                found.push(format!("{relative}:{marker}"));
            }
        }
    }
    Ok(())
}

fn is_text_source_file(path: &Path) -> bool {
    matches!(
        path.extension().and_then(|value| value.to_str()),
        Some("rs" | "js" | "mjs" | "html" | "css" | "toml" | "json" | "md")
    )
}

fn forbidden_evidence_markers() -> Vec<String> {
    vec![
        ["fake", " evidence"].concat(),
        ["fake", " screenshot"].concat(),
        ["synthetic", " screenshot"].concat(),
        ["blank", " screenshot accepted"].concat(),
        ["static", " evidence pass"].concat(),
    ]
}

fn package_success_sources() -> PackagingSources {
    let checks = REQUIRED_INTEGRATION_CHECKS
        .iter()
        .map(|id| ((*id).to_string(), Value::Bool(true)))
        .collect::<serde_json::Map<_, _>>();
    PackagingSources {
        integration: json!({
            "status": "success",
            "checks": Value::Object(checks)
        }),
        actual_project_file_audit: json!({
            "development_path": "UnityProject",
            "actual_changed_files": ["Assets/DemoScene.unity"]
        }),
        unity_validation_summary: json!({
            "valid": true,
            "unity_editor_path": "Unity.exe",
            "validation_count": 3,
            "failed_validation_count": 0
        }),
    }
}

fn package_sources_with_missing_changed_files() -> PackagingSources {
    let mut sources = package_success_sources();
    sources.actual_project_file_audit = json!({
        "development_path": "UnityProject",
        "actual_changed_files": []
    });
    sources
}

fn package_sources_with_missing_unity_summary() -> PackagingSources {
    let mut sources = package_success_sources();
    sources.unity_validation_summary = json!({});
    sources
}

fn add_package_result_rows(report: &mut GateReport, scenario: &str, result: &PackageRunResult) {
    report.add_row(
        format!("package:{scenario}:validation_status"),
        result.validation_report.status.as_str().to_string(),
    );
    report.add_row(
        format!("package:{scenario}:build_status"),
        result.build_report.status.as_str().to_string(),
    );
    report.add_row(
        format!("package:{scenario}:manifest_status"),
        result.manifest.status.as_str().to_string(),
    );
    report.add_row(
        format!("package:{scenario}:check_count"),
        result.validation_report.checks.len().to_string(),
    );
    report.add_row(
        format!("package:{scenario}:changed_file_count"),
        result.validation_report.changed_files.len().to_string(),
    );
    report.add_row(
        format!("package:{scenario}:blocker_count"),
        result.validation_report.blocking_issues.len().to_string(),
    );
    report.add_row(
        format!("package:{scenario}:manifest_package_dir"),
        result.manifest.outputs.package_dir.clone(),
    );
    report.add_row(
        format!("package:{scenario}:manifest_validation_report"),
        result.manifest.outputs.package_validation_report.clone(),
    );
    report.add_row(
        format!("package:{scenario}:notes_bytes"),
        result.package_notes.len().to_string(),
    );
    for issue in &result.validation_report.blocking_issues {
        report.add_row(
            format!("package:{scenario}:issue:{}", issue.id),
            issue.message.clone(),
        );
    }
}

fn require_package_success(report: &mut GateReport, scenario: &str, result: &PackageRunResult) {
    require_status(
        report,
        scenario,
        "validation",
        &result.validation_report.status,
        PackageStatus::Success,
    );
    require_status(
        report,
        scenario,
        "build",
        &result.build_report.status,
        PackageStatus::Success,
    );
    require_status(
        report,
        scenario,
        "manifest",
        &result.manifest.status,
        PackageStatus::Success,
    );
    if !result.validation_report.blocking_issues.is_empty() {
        report.add_blocker(format!("{scenario}:unexpected_blocking_issues"));
    }
    if !result.validation_report.changed_files_present() {
        report.add_blocker(format!("{scenario}:missing_changed_files"));
    }
    require_required_checks(report, scenario, &result.validation_report);
    require_manifest_outputs(report, scenario, &result.manifest);
    if !result
        .package_notes
        .contains("passed packaging readiness checks")
    {
        report.add_blocker(format!("{scenario}:missing_success_notes"));
    }
}

fn require_package_blocker(
    report: &mut GateReport,
    scenario: &str,
    result: &PackageRunResult,
    required_issue: &str,
) {
    require_status(
        report,
        scenario,
        "validation",
        &result.validation_report.status,
        PackageStatus::Blocked,
    );
    require_status(
        report,
        scenario,
        "build",
        &result.build_report.status,
        PackageStatus::Blocked,
    );
    require_status(
        report,
        scenario,
        "manifest",
        &result.manifest.status,
        PackageStatus::Blocked,
    );
    require_required_checks(report, scenario, &result.validation_report);
    require_manifest_outputs(report, scenario, &result.manifest);
    if !result
        .validation_report
        .blocking_issues
        .iter()
        .any(|issue| issue.id == required_issue)
    {
        report.add_blocker(format!(
            "{scenario}:missing_required_issue:{required_issue}"
        ));
    }
    if !result.package_notes.contains(required_issue) {
        report.add_blocker(format!("{scenario}:notes_missing_issue:{required_issue}"));
    }
}

fn require_status(
    report: &mut GateReport,
    scenario: &str,
    area: &str,
    actual: &PackageStatus,
    expected: PackageStatus,
) {
    if *actual != expected {
        report.add_blocker(format!(
            "{scenario}:{area}_status_expected_{}",
            expected.as_str()
        ));
    }
}

fn require_required_checks(
    report: &mut GateReport,
    scenario: &str,
    report_value: &PackageValidationReport,
) {
    if report_value.checks.len() != REQUIRED_INTEGRATION_CHECKS.len() {
        report.add_blocker(format!("{scenario}:required_check_count_mismatch"));
    }
    for id in REQUIRED_INTEGRATION_CHECKS {
        if !report_value.checks.iter().any(|check| check.id == *id) {
            report.add_blocker(format!("{scenario}:required_check_missing:{id}"));
        }
    }
}

fn require_manifest_outputs(report: &mut GateReport, scenario: &str, manifest: &PackageManifest) {
    if manifest.schema_version != 1 {
        report.add_blocker(format!("{scenario}:manifest_schema_version_invalid"));
    }
    if manifest.package_type != "current_project_build_package" {
        report.add_blocker(format!("{scenario}:manifest_package_type_invalid"));
    }
    if manifest.source_stage != 14 || manifest.source_stage_name != "integration_validation" {
        report.add_blocker(format!("{scenario}:manifest_source_stage_invalid"));
    }
    if manifest.outputs.package_dir != PACKAGE_DIR {
        report.add_blocker(format!("{scenario}:manifest_package_dir_invalid"));
    }
    if manifest.outputs.build_report != format!("{PACKAGE_DIR}/build_report.json") {
        report.add_blocker(format!("{scenario}:manifest_build_report_path_invalid"));
    }
    if manifest.outputs.package_validation_report
        != format!("{PACKAGE_DIR}/package_validation_report.json")
    {
        report.add_blocker(format!(
            "{scenario}:manifest_validation_report_path_invalid"
        ));
    }
    if manifest.outputs.package_notes != format!("{PACKAGE_DIR}/PACKAGE_NOTES.md") {
        report.add_blocker(format!("{scenario}:manifest_notes_path_invalid"));
    }
}

fn ignored_test_markers(root: &Path) -> AdmResult<Vec<String>> {
    let mut found = Vec::new();
    collect_ignored_test_markers(root, root, &mut found)?;
    Ok(found)
}

fn collect_ignored_test_markers(
    root: &Path,
    current: &Path,
    found: &mut Vec<String>,
) -> AdmResult<()> {
    if !current.is_dir() {
        return Ok(());
    }
    for entry in fs::read_dir(current)? {
        let entry = entry?;
        let path = entry.path();
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if path.is_dir() {
            if name == "target" || name == "node_modules" || name == "dist" {
                continue;
            }
            collect_ignored_test_markers(root, &path, found)?;
            continue;
        }
        if path.extension().and_then(|value| value.to_str()) != Some("rs") {
            continue;
        }
        let text = fs::read_to_string(&path)?;
        let marker = format!("#[{}]", "ignore");
        if text.contains(&marker) {
            let relative = path
                .strip_prefix(root)
                .unwrap_or(&path)
                .display()
                .to_string();
            found.push(relative);
        }
    }
    Ok(())
}

#[allow(dead_code)]
fn add_scorecard_rows(report: &mut GateReport, evaluation: &ScorecardEvaluation, text: &str) {
    let key = evaluation.relative_path.replace(['/', '\\'], ":");
    report.add_row(
        format!("scorecard:{key}:status"),
        evaluation.status_line.clone(),
    );
    report.add_row(
        format!("scorecard:{key}:weighted"),
        format!("{:.1}", evaluation.weighted_score),
    );
    if let Some(declared) = evaluation.declared_weighted_score {
        report.add_row(
            format!("scorecard:{key}:declared_weighted"),
            format!("{declared:.1}"),
        );
        if (declared - evaluation.weighted_score).abs() > 0.15 {
            report.add_blocker(format!(
                "scorecard_weighted_mismatch:{}",
                evaluation.relative_path
            ));
        }
    } else {
        report.add_blocker(format!(
            "scorecard_missing_weighted_score:{}",
            evaluation.relative_path
        ));
    }
    if !evaluation.status_line.contains("通过") {
        report.add_blocker(format!(
            "scorecard_status_not_passed:{}",
            evaluation.relative_path
        ));
    }
    if !text.contains("无硬门禁失败") {
        report.add_blocker(format!(
            "scorecard_missing_no_hard_gate_failure:{}",
            evaluation.relative_path
        ));
    }
    if evaluation.weighted_score < MIN_WEIGHTED_SCORE {
        report.add_blocker(format!(
            "scorecard_weighted_below_95:{}",
            evaluation.relative_path
        ));
    }
    for score in &evaluation.scores {
        report.add_row(
            format!("scorecard:{key}:role:{}", score.area),
            format!("{}:{}", score.value, score.confidence),
        );
        if score.value < MIN_ROLE_SCORE {
            report.add_blocker(format!(
                "scorecard_role_below_90:{}:{}",
                evaluation.relative_path, score.area
            ));
        }
        if score.confidence.eq_ignore_ascii_case("low") {
            report.add_blocker(format!(
                "scorecard_confidence_low:{}:{}",
                evaluation.relative_path, score.area
            ));
        }
    }
}

fn deprecated_gate_report(name: &str, code: &str) -> GateReport {
    let mut report = GateReport::new(name);
    report.add_row("lifecycle", "deprecated");
    report.add_row(
        "replacement",
        "doctor; standalone-boundary-gate; release-gate",
    );
    report.add_blocker(code);
    report
}

pub fn is_standalone_repo_root(path: &Path) -> bool {
    SourceProjectRoot::open(path).is_ok()
}

fn same_path(left: &Path, right: &Path) -> bool {
    let left = left.canonicalize().unwrap_or_else(|_| left.to_path_buf());
    let right = right.canonicalize().unwrap_or_else(|_| right.to_path_buf());
    left == right
}

pub fn find_repo_root(start: &Path) -> Option<PathBuf> {
    SourceProjectRoot::discover(start)
        .ok()
        .map(SourceProjectRoot::into_path)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn assert_gate_deprecated(report: GateReport, code: &str) {
        let rendered = report.render();
        assert!(!report.passed(), "{rendered}");
        assert!(rendered.contains("lifecycle=deprecated"), "{rendered}");
        assert!(rendered.contains(code), "{rendered}");
    }

    fn write_standalone_root_fixture(root: &Path) {
        fs::create_dir_all(root.join("knowledge")).unwrap();
        fs::create_dir_all(root.join("web")).unwrap();
        fs::create_dir_all(root.join("nested/deeper")).unwrap();
        fs::write(root.join("Cargo.toml"), "[workspace]\nmembers = []\n").unwrap();
        fs::write(root.join("Cargo.lock"), "# fixture\n").unwrap();
        fs::write(root.join("web/package-lock.json"), "{}\n").unwrap();
        fs::write(
            root.join(SOURCE_ROOT_MARKER),
            r#"{"schemaVersion":1,"kind":"source-project-root","projectId":"autodesignmaker-rust-v2","workspaceManifest":"Cargo.toml","lockfiles":["Cargo.lock","web/package-lock.json"],"resourceManifest":"knowledge/resource-manifest.json"}"#,
        )
        .unwrap();
        fs::write(
            root.join(RESOURCE_MANIFEST_PATH),
            r#"{"schemaVersion":1,"projectId":"autodesignmaker-rust-v2","groups":[{"path":"knowledge"}]}"#,
        )
        .unwrap();
    }

    fn valid_release_evidence_fixture(now: u64) -> StandaloneReleaseEvidence {
        let checks = REQUIRED_RELEASE_CHECKS
            .iter()
            .map(|id| {
                (
                    (*id).to_string(),
                    ReleaseCommandEvidence {
                        status: "passed".to_string(),
                        command: format!("fixture:{id}"),
                        exit_code: 0,
                        duration_ms: 1,
                        output_sha256: "a".repeat(64),
                    },
                )
            })
            .collect();
        StandaloneReleaseEvidence {
            schema_version: RELEASE_EVIDENCE_SCHEMA_VERSION,
            producer: RELEASE_EVIDENCE_PRODUCER.to_string(),
            evidence_id: "1".repeat(32),
            project_id: SOURCE_PROJECT_ID.to_string(),
            status: "passed".to_string(),
            git_commit: "b".repeat(40),
            source_tree_clean: true,
            generated_at_unix: now - 60,
            expires_at_unix: now + 60,
            checks,
            portable: Some(ReleasePortableEvidence {
                root: RELEASE_PORTABLE_ROOT.to_string(),
                executable: "AutoDesignMaker.exe".to_string(),
                executable_sha256: "c".repeat(64),
                build_manifest_sha256: "d".repeat(64),
                resource_manifest_sha256: "e".repeat(64),
                git_commit: "b".repeat(40),
                swap_receipt: format!(
                    "dist/.{RELEASE_PORTABLE_OUTPUT_NAME}.swap-{}.json",
                    "2".repeat(32)
                ),
                swap_receipt_sha256: "f".repeat(64),
                transaction_id: "2".repeat(32),
                transaction_status: "finalized".to_string(),
            }),
            errors: Vec::new(),
        }
    }

    #[test]
    fn standalone_root_resolution_is_name_independent_and_walks_from_subdirectory() {
        let root =
            std::env::temp_dir().join(format!("renamed standalone 项目 {}", std::process::id()));
        write_standalone_root_fixture(&root);
        let found = find_repo_root(&root.join("nested/deeper")).unwrap();
        assert!(same_path(&found, &root));
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn parent_plan_directory_is_not_a_project_root() {
        let root = std::env::temp_dir().join(format!("legacy_parent_{}", std::process::id()));
        let legacy_child_name = ["NEW", "rust"].concat();
        let nested = root.join("plan").join(legacy_child_name).join("child");
        fs::create_dir_all(&nested).unwrap();
        assert!(find_repo_root(&nested).is_none());
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn nearest_invalid_marker_blocks_outer_root_fallback() {
        let root =
            std::env::temp_dir().join(format!("standalone_nearest_marker_{}", std::process::id()));
        write_standalone_root_fixture(&root);
        let inner = root.join("nested");
        fs::write(inner.join(SOURCE_ROOT_MARKER), "{\"kind\":\"invalid\"}").unwrap();
        assert!(find_repo_root(&inner.join("deeper")).is_none());
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn standalone_boundary_gate_passes_for_current_repo() {
        let cwd = std::env::current_dir().unwrap();
        let repo_root = find_repo_root(&cwd).unwrap();
        let report = standalone_boundary_gate_report(&repo_root).unwrap();
        assert!(report.passed(), "{}", report.render());
    }

    #[test]
    fn plan_and_handoff_gates_are_stably_deprecated() {
        assert_gate_deprecated(
            plan_gate_report(Path::new("unused")).unwrap(),
            LEGACY_PLAN_GATE_DEPRECATED,
        );
        assert_gate_deprecated(
            handoff_report(Path::new("unused")).unwrap(),
            LEGACY_HANDOFF_GATE_DEPRECATED,
        );
    }

    #[test]
    fn final_scores_meet_new_threshold_rule() {
        let scores = vec![
            Score {
                area: "A".to_string(),
                value: 90,
                weight: 50,
                confidence: "medium".to_string(),
            },
            Score {
                area: "B".to_string(),
                value: 100,
                weight: 50,
                confidence: "high".to_string(),
            },
        ];
        assert!(all_scores_meet_threshold(&scores, 90));
        assert_eq!(weighted_score(&scores).unwrap(), 95.0);
    }

    #[test]
    fn required_plan_file_list_is_stable() {
        assert!(REQUIRED_PLAN_FILES.contains(&"README.md"));
        assert!(REQUIRED_PLAN_FILES.contains(&"00_execution_protocol.md"));
        assert!(REQUIRED_PLAN_FILES.contains(&"python_deconstruction/scorecard.md"));
        assert!(REQUIRED_PLAN_FILES.contains(&"newrust_design/scorecard.md"));
        assert!(REQUIRED_PLAN_FILES.contains(&"atomic_backlog/scorecard.md"));
    }

    #[test]
    fn parity_gate_declares_contract_storage_domain_application_checks() {
        let layers = REQUIRED_PARITY_CHECKS
            .iter()
            .map(|check| check.layer)
            .collect::<Vec<_>>();
        assert!(layers.contains(&"contract"));
        assert!(layers.contains(&"storage"));
        assert!(layers.contains(&"domain"));
        assert!(layers.contains(&"application"));
        assert!(layers.contains(&"command"));
        assert!(
            REQUIRED_PARITY_CHECKS
                .iter()
                .any(|check| check.id == "commands_config_no_key_exposure")
        );
    }

    #[test]
    fn parity_gate_current_repo_has_required_markers() {
        let cwd = std::env::current_dir().unwrap();
        let repo_root = find_repo_root(&cwd).unwrap();
        let report = parity_gate_report(&repo_root).unwrap();
        assert!(report.passed(), "{}", report.render());
        assert!(
            report
                .render()
                .contains("required_execution=cargo test --workspace --quiet")
        );
    }

    #[test]
    fn validation_gate_declares_a29_validator_markers() {
        let ids = REQUIRED_VALIDATION_MARKERS
            .iter()
            .map(|check| check.id)
            .collect::<Vec<_>>();
        assert!(ids.contains(&"config_validator"));
        assert!(ids.contains(&"context_lint"));
        assert!(ids.contains(&"contract_validator"));
        assert!(ids.contains(&"output_validator"));
        assert!(ids.contains(&"pipeline_quality_plan_002"));
        assert!(ids.contains(&"design_semantic_quality"));
        assert!(ids.contains(&"cli_validation_a29_test"));
    }

    #[test]
    fn validation_gate_current_repo_has_required_markers() {
        let cwd = std::env::current_dir().unwrap();
        let repo_root = find_repo_root(&cwd).unwrap();
        let report = validation_gate_report(&repo_root).unwrap();
        let rendered = report.render();
        assert!(report.passed(), "{rendered}");
        assert!(
            rendered.contains("source_contract=crates/adm-new-application/src/validation_tools.rs")
        );
        assert!(rendered.contains("required_validation_check_count=15"));
    }

    #[test]
    fn iteration_gate_declares_a30_iteration_markers() {
        let ids = REQUIRED_ITERATION_MARKERS
            .iter()
            .map(|check| check.id)
            .collect::<Vec<_>>();
        assert!(ids.contains(&"iteration_spec_parser"));
        assert!(ids.contains(&"delta_scheduler"));
        assert!(ids.contains(&"artifact_inheritor"));
        assert!(ids.contains(&"iteration_prepare"));
        assert!(ids.contains(&"iteration_resume_summary"));
        assert!(ids.contains(&"cli_iteration_a30_test"));
    }

    #[test]
    fn iteration_gate_current_repo_has_required_markers() {
        let cwd = std::env::current_dir().unwrap();
        let repo_root = find_repo_root(&cwd).unwrap();
        let report = iteration_gate_report(&repo_root).unwrap();
        let rendered = report.render();
        assert!(report.passed(), "{rendered}");
        assert!(rendered.contains("source_contract=crates/adm-new-application/src/iteration.rs"));
        assert!(rendered.contains("required_iteration_check_count=9"));
    }

    #[test]
    fn ui_shell_gate_declares_a31_shell_markers() {
        let ids = REQUIRED_UI_SHELL_MARKERS
            .iter()
            .map(|check| check.id)
            .collect::<Vec<_>>();
        assert!(ids.contains(&"desktop_shell_config"));
        assert!(ids.contains(&"desktop_smoke_report"));
        assert!(ids.contains(&"shell_command_theme_state"));
        assert!(ids.contains(&"web_theme_module"));
        assert!(ids.contains(&"web_shell_unit_test"));
    }

    #[test]
    fn ui_shell_gate_current_repo_has_required_markers() {
        let cwd = std::env::current_dir().unwrap();
        let repo_root = find_repo_root(&cwd).unwrap();
        let report = ui_shell_gate_report(&repo_root).unwrap();
        let rendered = report.render();
        assert!(report.passed(), "{rendered}");
        assert!(rendered.contains(
            "source_contract=apps/desktop-tauri; crates/adm-new-tauri-commands/src/shell.rs; web/src"
        ));
        assert!(rendered.contains("required_ui_shell_check_count=10"));
    }

    #[test]
    fn ui_workbench_gate_declares_a32_workbench_markers() {
        let ids = REQUIRED_UI_WORKBENCH_MARKERS
            .iter()
            .map(|check| check.id)
            .collect::<Vec<_>>();
        assert!(ids.contains(&"python_workbench_template_viewer"));
        assert!(ids.contains(&"python_workbench_gameplay_systems"));
        assert!(ids.contains(&"application_autosave_report"));
        assert!(ids.contains(&"tauri_template_command"));
        assert!(ids.contains(&"web_workbench_builders"));
        assert!(ids.contains(&"web_workbench_unit_test"));
        assert!(ids.contains(&"cli_ui_workbench_gate"));
    }

    #[test]
    fn ui_workbench_gate_current_repo_has_required_markers() {
        let cwd = std::env::current_dir().unwrap();
        let repo_root = find_repo_root(&cwd).unwrap();
        let report = ui_workbench_gate_report(&repo_root).unwrap();
        assert_gate_deprecated(report, LEGACY_PYTHON_GATE_DEPRECATED);
    }

    #[test]
    fn ui_ai_gate_declares_a33_ai_markers() {
        let ids = REQUIRED_UI_AI_MARKERS
            .iter()
            .map(|check| check.id)
            .collect::<Vec<_>>();
        assert!(ids.contains(&"python_ai_interview_window"));
        assert!(ids.contains(&"python_ai_background_mapping"));
        assert!(ids.contains(&"python_embedded_interview_panel"));
        assert!(ids.contains(&"tauri_ai_stream_view"));
        assert!(ids.contains(&"web_ai_controller"));
        assert!(ids.contains(&"web_bottom_ai_tab"));
        assert!(ids.contains(&"cli_ui_ai_gate"));
    }

    #[test]
    fn ui_ai_gate_current_repo_has_required_markers() {
        let cwd = std::env::current_dir().unwrap();
        let repo_root = find_repo_root(&cwd).unwrap();
        let report = ui_ai_gate_report(&repo_root).unwrap();
        assert_gate_deprecated(report, LEGACY_PYTHON_GATE_DEPRECATED);
    }

    #[test]
    fn ui_pipeline_gate_declares_a34_pipeline_markers() {
        let ids = REQUIRED_UI_PIPELINE_MARKERS
            .iter()
            .map(|check| check.id)
            .collect::<Vec<_>>();
        assert!(ids.contains(&"python_pipeline_panel"));
        assert!(ids.contains(&"python_pipeline_step_card"));
        assert!(ids.contains(&"python_semantic_quality_panel"));
        assert!(ids.contains(&"tauri_pipeline_semantic_quality_view"));
        assert!(ids.contains(&"web_pipeline_semantic_normalizer"));
        assert!(ids.contains(&"web_pipeline_full_tree_test"));
        assert!(ids.contains(&"cli_ui_pipeline_gate"));
    }

    #[test]
    fn ui_pipeline_gate_current_repo_has_required_markers() {
        let cwd = std::env::current_dir().unwrap();
        let repo_root = find_repo_root(&cwd).unwrap();
        let report = ui_pipeline_gate_report(&repo_root).unwrap();
        assert_gate_deprecated(report, LEGACY_PYTHON_GATE_DEPRECATED);
    }

    #[test]
    fn ui_utility_gate_declares_a35_utility_markers() {
        let ids = REQUIRED_UI_UTILITY_MARKERS
            .iter()
            .map(|check| check.id)
            .collect::<Vec<_>>();
        assert!(ids.contains(&"python_patch_panel"));
        assert!(ids.contains(&"python_package_panel"));
        assert!(ids.contains(&"python_sdk_panel"));
        assert!(ids.contains(&"python_save_manager_dialog"));
        assert!(ids.contains(&"tauri_save_commands"));
        assert!(ids.contains(&"web_save_index_normalizer"));
        assert!(ids.contains(&"web_project_state_converter"));
        assert!(ids.contains(&"web_save_dialog_renderer"));
        assert!(ids.contains(&"web_save_manager_screenshot_gate"));
        assert!(ids.contains(&"cli_ui_utility_gate"));
    }

    #[test]
    fn ui_utility_gate_current_repo_has_required_markers() {
        let cwd = std::env::current_dir().unwrap();
        let repo_root = find_repo_root(&cwd).unwrap();
        let report = ui_utility_gate_report(&repo_root).unwrap();
        assert_gate_deprecated(report, LEGACY_PYTHON_GATE_DEPRECATED);
    }

    #[test]
    fn ui_settings_style_gate_declares_a36_settings_style_markers() {
        let ids = REQUIRED_UI_SETTINGS_STYLE_MARKERS
            .iter()
            .map(|check| check.id)
            .collect::<Vec<_>>();
        assert!(ids.contains(&"python_ai_config_unified_dialog"));
        assert!(ids.contains(&"python_project_config_dialog"));
        assert!(ids.contains(&"python_style_confirmation_dialog"));
        assert!(ids.contains(&"python_style_prompt_editor"));
        assert!(ids.contains(&"tauri_project_config_request"));
        assert!(ids.contains(&"tauri_project_preflight_command"));
        assert!(ids.contains(&"web_settings_style_module"));
        assert!(ids.contains(&"web_style_prompt_parser"));
        assert!(ids.contains(&"web_project_config_screenshot_gate"));
        assert!(ids.contains(&"web_style_prompt_screenshot_gate"));
        assert!(ids.contains(&"cli_ui_settings_style_gate"));
    }

    #[test]
    fn ui_settings_style_gate_current_repo_has_required_markers() {
        let cwd = std::env::current_dir().unwrap();
        let repo_root = find_repo_root(&cwd).unwrap();
        let report = ui_settings_style_gate_report(&repo_root).unwrap();
        assert_gate_deprecated(report, LEGACY_PYTHON_GATE_DEPRECATED);
    }

    #[test]
    fn ui_parity_v3_gate_declares_a37_baseline_record_count() {
        assert_eq!(REQUIRED_UI_PARITY_V3_RECORD_COUNT, 93);
    }

    #[test]
    fn ui_parity_v3_gate_current_repo_has_required_baselines() {
        let cwd = std::env::current_dir().unwrap();
        let repo_root = find_repo_root(&cwd).unwrap();
        let report = ui_parity_v3_gate_report(&repo_root).unwrap();
        assert_gate_deprecated(report, LEGACY_PYTHON_GATE_DEPRECATED);
    }

    #[test]
    fn unit_test_migration_gate_declares_a38_unit_test_count() {
        assert_eq!(REQUIRED_UNIT_TEST_MIGRATION_COUNT, 68);
        let ids = REQUIRED_UNIT_TEST_MIGRATION_MARKERS
            .iter()
            .map(|check| check.id)
            .collect::<Vec<_>>();
        assert!(ids.contains(&"ai_completion_service"));
        assert!(ids.contains(&"design_semantic_alignment_style_tasks"));
        assert!(ids.contains(&"pipeline_step14"));
        assert!(ids.contains(&"application_unattended_recovery"));
        assert!(ids.contains(&"web_pipeline_semantic"));
        assert!(ids.contains(&"gate_pytest_environment"));
    }

    #[test]
    fn unit_test_migration_gate_current_repo_maps_all_unit_tests() {
        let cwd = std::env::current_dir().unwrap();
        let repo_root = find_repo_root(&cwd).unwrap();
        let report = unit_test_migration_gate_report(&repo_root).unwrap();
        assert_gate_deprecated(report, LEGACY_PYTHON_GATE_DEPRECATED);
    }

    #[test]
    fn integration_test_migration_gate_declares_a39_integration_test_count() {
        assert_eq!(REQUIRED_INTEGRATION_TEST_MIGRATION_COUNT, 5);
        let ids = REQUIRED_INTEGRATION_TEST_MIGRATION_MARKERS
            .iter()
            .map(|check| check.id)
            .collect::<Vec<_>>();
        assert!(ids.contains(&"python_conftest_ai_config_fixture"));
        assert!(ids.contains(&"ai_config_active_profile"));
        assert!(ids.contains(&"pipeline_design_flow_registry"));
        assert!(ids.contains(&"pipeline_step08_14_semantic_chain"));
        assert!(ids.contains(&"save_parallel_artifact_isolation"));
        assert!(ids.contains(&"gate_integration_test_migration"));
    }

    #[test]
    fn integration_test_migration_gate_current_repo_maps_all_integration_tests() {
        let cwd = std::env::current_dir().unwrap();
        let repo_root = find_repo_root(&cwd).unwrap();
        let report = integration_test_migration_gate_report(&repo_root).unwrap();
        assert_gate_deprecated(report, LEGACY_PYTHON_GATE_DEPRECATED);
    }

    #[test]
    fn final_handoff_v3_gate_declares_file_level_completion_requirements() {
        assert_eq!(REQUIRED_FULL_PROJECT_PYTHON_FILE_COUNT, 379);
        assert_eq!(REQUIRED_FULL_PROJECT_TEST_MIGRATION_COUNT, 73);
        assert_eq!(REQUIRED_DATA_ASSET_MIGRATION_COUNT, 727);
        assert_eq!(REQUIRED_UI_PARITY_V3_RECORD_COUNT, 93);
        assert_eq!(REQUIRED_FINAL_HANDOFF_V3_COMPLETED_ATOMS, 40);
        assert!(REQUIRED_FINAL_HANDOFF_V3_GATE_REFS.contains(&"gates/release-gate.adm"));
        assert!(
            REQUIRED_FINAL_HANDOFF_V3_GATE_REFS
                .contains(&"gates/integration-test-migration-gate.adm")
        );
    }

    #[test]
    fn final_handoff_v3_gate_is_stably_deprecated() {
        let cwd = std::env::current_dir().unwrap();
        let repo_root = find_repo_root(&cwd).unwrap();
        let report = final_handoff_v3_gate_report(&repo_root).unwrap();
        assert_gate_deprecated(report, LEGACY_HANDOFF_GATE_DEPRECATED);
    }

    #[test]
    fn package_gate_verifies_success_and_blocked_manifests() {
        let cwd = std::env::current_dir().unwrap();
        let repo_root = find_repo_root(&cwd).unwrap();
        let report = package_gate_report(&repo_root).unwrap();
        let rendered = report.render();
        assert!(report.passed(), "{rendered}");
        assert!(rendered.contains("package:success:validation_status=success"));
        assert!(rendered.contains("package:missing_changed_files:validation_status=blocked"));
        assert!(rendered.contains("PACKAGE-NO-ACTUAL-PROJECT-CHANGES"));
        assert!(rendered.contains("package:missing_unity_summary:validation_status=blocked"));
        assert!(rendered.contains("PACKAGE-UNITY-VALIDATION-MISSING"));
        assert!(rendered.contains("manifest_package_dir=outputs/package/current"));
    }

    #[test]
    fn release_gate_requires_commit_bound_standalone_evidence_for_every_check() {
        assert_eq!(REQUIRED_RELEASE_CHECKS.len(), 21);
        assert!(REQUIRED_RELEASE_CHECKS.contains(&"cargo_fmt_check"));
        assert!(REQUIRED_RELEASE_CHECKS.contains(&"cargo_check_workspace"));
        assert!(REQUIRED_RELEASE_CHECKS.contains(&"cargo_test_workspace"));
        assert!(REQUIRED_RELEASE_CHECKS.contains(&"web_unit"));
        assert!(REQUIRED_RELEASE_CHECKS.contains(&"web_i18n"));
        assert!(REQUIRED_RELEASE_CHECKS.contains(&"web_design_content"));
        assert!(REQUIRED_RELEASE_CHECKS.contains(&"web_build"));
        assert!(REQUIRED_RELEASE_CHECKS.contains(&"web_e2e"));
        assert!(REQUIRED_RELEASE_CHECKS.contains(&"web_language_gate"));
        assert!(REQUIRED_RELEASE_CHECKS.contains(&"web_ui_gate"));
        assert!(REQUIRED_RELEASE_CHECKS.contains(&"web_ui_baseline_gate"));
        assert!(REQUIRED_RELEASE_CHECKS.contains(&"package_contract_self_test"));
        assert!(REQUIRED_RELEASE_CHECKS.contains(&"resource_manifest"));
        assert!(REQUIRED_RELEASE_CHECKS.contains(&"standalone_boundary_gate"));
        assert!(REQUIRED_RELEASE_CHECKS.contains(&"portable_build"));
        assert!(REQUIRED_RELEASE_CHECKS.contains(&"portable_smoke"));
        assert!(REQUIRED_RELEASE_CHECKS.contains(&"portable_integrity"));
        assert!(REQUIRED_RELEASE_CHECKS.contains(&"pe_architecture_crt"));
        assert!(REQUIRED_RELEASE_CHECKS.contains(&"clean_clone_relocation"));
        assert!(REQUIRED_RELEASE_CHECKS.contains(&"anti_fake_scan"));
        assert!(REQUIRED_RELEASE_CHECKS.contains(&"generated_cleanup"));
        assert!(!REQUIRED_RELEASE_CHECKS.contains(&"plan_gate"));
        assert!(!REQUIRED_RELEASE_CHECKS.contains(&"parity_gate"));
    }

    #[test]
    fn structured_release_evidence_accepts_only_complete_fresh_current_head_fixture() {
        let now = 1_800_000_000;
        let evidence = valid_release_evidence_fixture(now);
        assert!(
            validate_release_evidence_structure(&evidence, now, &evidence.git_commit).is_empty()
        );
    }

    #[test]
    fn structured_release_evidence_rejects_stale_missing_and_error_claims() {
        let now = 1_800_000_000;
        let mut evidence = valid_release_evidence_fixture(now);
        evidence.expires_at_unix = now;
        evidence.checks.remove("portable_integrity");
        evidence.errors.push("fixture failure".to_string());
        let blockers = validate_release_evidence_structure(&evidence, now, &evidence.git_commit);
        assert!(
            blockers
                .iter()
                .any(|code| code == "standalone_release_evidence_freshness_invalid")
        );
        assert!(
            blockers
                .iter()
                .any(|code| code == "standalone_release_evidence_check_set_invalid")
        );
        assert!(
            blockers
                .iter()
                .any(|code| code == "standalone_release_evidence_contains_errors")
        );
    }

    #[test]
    fn structured_release_evidence_rejects_running_status_and_old_portable_shape() {
        let now = 1_800_000_000;
        let mut running = valid_release_evidence_fixture(now);
        running.status = "running".to_string();
        let blockers = validate_release_evidence_structure(&running, now, &running.git_commit);
        assert!(
            blockers
                .iter()
                .any(|code| code == "standalone_release_evidence_not_passed")
        );

        let mut old_shape = serde_json::to_value(valid_release_evidence_fixture(now)).unwrap();
        old_shape["portable"]
            .as_object_mut()
            .unwrap()
            .remove("swapReceipt");
        assert!(serde_json::from_value::<StandaloneReleaseEvidence>(old_shape).is_err());
    }

    #[test]
    fn portable_transaction_contract_binds_receipt_output_root_head_and_transaction() {
        let root = std::env::temp_dir().join(format!(
            "governance_portable_transaction_contract_{}",
            std::process::id()
        ));
        let evidence = valid_release_evidence_fixture(1_800_000_000);
        let portable = evidence.portable.as_ref().unwrap();
        let dist = root.join("dist");
        let transaction_id = &portable.transaction_id;
        let receipt = json!({
            "schema_version": 1,
            "kind": "portable-swap-transaction",
            "transaction_id": transaction_id,
            "output_name": RELEASE_PORTABLE_OUTPUT_NAME,
            "release_mode": "formal",
            "dist_root": dist,
            "live_root": root.join(RELEASE_PORTABLE_ROOT),
            "stage_root": dist.join(format!(".{RELEASE_PORTABLE_OUTPUT_NAME}.stage-{transaction_id}")),
            "backup_root": dist.join(format!(".{RELEASE_PORTABLE_OUTPUT_NAME}.previous-{transaction_id}")),
            "failed_root": dist.join(format!(".{RELEASE_PORTABLE_OUTPUT_NAME}.failed-{transaction_id}")),
            "backup_tombstone_root": dist.join(format!(".{RELEASE_PORTABLE_OUTPUT_NAME}.retired-backup-{transaction_id}")),
            "failed_tombstone_root": dist.join(format!(".{RELEASE_PORTABLE_OUTPUT_NAME}.retired-failed-{transaction_id}")),
            "staged_immutable_tree": {
                "Exists": true,
                "FileCount": 1,
                "Bytes": 1,
                "Digest": "8".repeat(64),
            },
            "staged_user_data": {
                "Exists": true,
                "FileCount": 0,
                "Bytes": 0,
                "Digest": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
            },
            "had_previous_live": false,
            "backup_deleted": false,
            "status": "finalized",
            "smoke_status": "passed",
            "finalized_at_utc": "2026-07-12T00:00:00Z",
        });
        let build = json!({
            "transaction_id": transaction_id,
            "git_commit": evidence.git_commit,
        });
        assert!(
            validate_portable_transaction_contract(&root, &evidence, portable, &receipt, &build)
                .is_empty()
        );

        let mut wrong_build = build.clone();
        wrong_build["transaction_id"] = json!("3".repeat(32));
        assert!(
            validate_portable_transaction_contract(
                &root,
                &evidence,
                portable,
                &receipt,
                &wrong_build,
            )
            .iter()
            .any(|code| code == "release_portable_transaction_id_mismatch")
        );

        let mut inconsistent_receipt = receipt;
        inconsistent_receipt["backup_deleted"] = json!(true);
        assert!(
            validate_portable_transaction_contract(
                &root,
                &evidence,
                portable,
                &inconsistent_receipt,
                &build,
            )
            .iter()
            .any(|code| code == "release_portable_transaction_backup_finalization_invalid")
        );
    }

    #[test]
    fn portable_immutable_measure_excludes_user_data_and_update_lock_only() {
        let root = std::env::temp_dir().join(format!(
            "governance_portable_immutable_measure_{}",
            std::process::id()
        ));
        fs::create_dir_all(root.join("knowledge")).unwrap();
        fs::create_dir_all(root.join("user_data")).unwrap();
        fs::write(root.join("AutoDesignMaker.exe"), b"exe").unwrap();
        fs::write(root.join("knowledge/data.json"), b"data").unwrap();
        fs::write(root.join("user_data/save.json"), b"save-one").unwrap();
        fs::write(root.join(".portable-update.lock"), b"lock-one").unwrap();

        let before = measure_release_immutable_tree(&root).unwrap();
        fs::write(root.join("user_data/save.json"), b"save-two").unwrap();
        fs::write(root.join(".portable-update.lock"), b"lock-two").unwrap();
        let after = measure_release_immutable_tree(&root).unwrap();

        assert_eq!(before, after);
        assert_eq!(before.0, 2);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn portable_output_state_scan_rejects_unresolved_receipts_artifacts_and_tombstones() {
        let root = std::env::temp_dir().join(format!(
            "governance_portable_output_state_{}",
            std::process::id()
        ));
        let dist = root.join("dist");
        fs::create_dir_all(&dist).unwrap();
        let current_id = "4".repeat(32);
        let current_name = format!(".{RELEASE_PORTABLE_OUTPUT_NAME}.swap-{current_id}.json");
        let current_relative = format!("dist/{current_name}");
        fs::write(
            dist.join(&current_name),
            serde_json::to_vec(&json!({
                "schema_version": 1,
                "kind": "portable-swap-transaction",
                "transaction_id": current_id,
                "output_name": RELEASE_PORTABLE_OUTPUT_NAME,
                "status": "finalized",
                "finalized_at_utc": "2026-07-12T00:00:00Z",
            }))
            .unwrap(),
        )
        .unwrap();
        fs::write(
            dist.join(format!(".{RELEASE_PORTABLE_OUTPUT_NAME}.operation.lock")),
            serde_json::to_vec(&json!({
                "schema_version": 1,
                "kind": "portable-output-operation-lock",
                "output_name": RELEASE_PORTABLE_OUTPUT_NAME,
                "transaction_id": current_id,
            }))
            .unwrap(),
        )
        .unwrap();
        assert!(
            collect_unresolved_portable_output_state(
                &root,
                RELEASE_PORTABLE_OUTPUT_NAME,
                &current_relative,
            )
            .unwrap()
            .is_empty()
        );

        fs::create_dir_all(dist.join(format!(
            ".{RELEASE_PORTABLE_OUTPUT_NAME}.stage-{}",
            "5".repeat(32)
        )))
        .unwrap();
        fs::create_dir_all(dist.join(format!(
            ".{RELEASE_PORTABLE_OUTPUT_NAME}.retired-backup-{}",
            "6".repeat(32)
        )))
        .unwrap();
        fs::write(
            dist.join(format!(
                ".{RELEASE_PORTABLE_OUTPUT_NAME}.swap-{}.json",
                "7".repeat(32)
            )),
            serde_json::to_vec(&json!({
                "schema_version": 1,
                "kind": "portable-swap-transaction",
                "output_name": RELEASE_PORTABLE_OUTPUT_NAME,
                "status": "finalizing",
            }))
            .unwrap(),
        )
        .unwrap();
        fs::write(
            dist.join(format!(".{RELEASE_PORTABLE_OUTPUT_NAME}.operation.lock")),
            b"locked",
        )
        .unwrap();
        let blockers = collect_unresolved_portable_output_state(
            &root,
            RELEASE_PORTABLE_OUTPUT_NAME,
            &current_relative,
        )
        .unwrap();
        assert!(
            blockers
                .iter()
                .any(|code| code.contains("unresolved_output_artifact"))
        );
        assert!(
            blockers
                .iter()
                .any(|code| code.contains("unresolved_receipt"))
        );
        assert!(
            blockers
                .iter()
                .any(|code| code.contains("operation_lock_active_or_stale"))
        );
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn structured_release_evidence_parser_rejects_bom_and_legacy_boolean_checks() {
        let bom_json = b"\xef\xbb\xbf{}";
        assert!(serde_json::from_slice::<StandaloneReleaseEvidence>(bom_json).is_err());
        let legacy = json!({
            "schemaVersion": 1,
            "projectId": SOURCE_PROJECT_ID,
            "status": "passed",
            "gitCommit": "b".repeat(40),
            "sourceTreeClean": true,
            "checks": { "cargo_fmt_check": true }
        });
        assert!(serde_json::from_value::<StandaloneReleaseEvidence>(legacy).is_err());
    }

    #[test]
    fn handoff_entries_have_required_evidence_axes() {
        let entries = handoff_entries();
        assert!(entries.iter().any(|entry| entry.feature_id == "ui_parity"));
        assert!(
            entries
                .iter()
                .any(|entry| entry.feature_id == "release_governance")
        );
        for entry in entries {
            assert!(!entry.python_evidence.is_empty(), "{}", entry.feature_id);
            assert!(!entry.newrust_files.is_empty(), "{}", entry.feature_id);
            assert!(!entry.tests.is_empty(), "{}", entry.feature_id);
            assert!(!entry.gate_refs.is_empty(), "{}", entry.feature_id);
            assert_eq!(entry.status, "covered");
        }
    }

    #[test]
    fn scorecard_parser_uses_last_role_table() {
        let text = r#"
# Scorecard

状态：第二轮评分通过。

| 角色 | 领域 | 分数 | 权重 | confidence | evidence | issues | required_action |
| --- | --- | ---: | ---: | --- | --- | --- | --- |
| A | first | 90 | 10 | medium | e | i | a |
| B | first | 90 | 15 | medium | e | i | a |
| C | first | 90 | 15 | medium | e | i | a |
| D | first | 90 | 15 | medium | e | i | a |
| E | first | 90 | 15 | medium | e | i | a |
| F | first | 90 | 15 | medium | e | i | a |
| G | first | 90 | 15 | medium | e | i | a |

| 角色 | 领域 | 分数 | 权重 | confidence | evidence | issues | required_action |
| --- | --- | ---: | ---: | --- | --- | --- | --- |
| A | final | 96 | 10 | high | e | i | a |
| B | final | 96 | 15 | high | e | i | a |
| C | final | 97 | 15 | high | e | i | a |
| D | final | 96 | 15 | high | e | i | a |
| E | final | 97 | 15 | high | e | i | a |
| F | final | 96 | 15 | high | e | i | a |
| G | final | 95 | 15 | high | e | i | a |

第一轮加权综合分：`90.0`。
第二轮加权综合分：`96.2`。
第一轮结论：合格。单项均 `>=90`，综合 `>=95`，无硬门禁失败，confidence 均非 low。
"#;
        let evaluation = evaluate_scorecard("scorecard.md", text).unwrap();
        assert_eq!(evaluation.scores.len(), EXPECTED_ROLE_COUNT);
        assert_eq!(evaluation.scores[0].value, 96);
        assert!(evaluation.weighted_score >= 96.1);
        assert_eq!(evaluation.declared_weighted_score, Some(96.2));
    }

    #[test]
    fn scorecard_parser_rejects_missing_rows() {
        let error = evaluate_scorecard("bad.md", "状态：通过").unwrap_err();
        assert!(error.message().contains("fewer than"));
    }
}
