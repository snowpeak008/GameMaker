use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::Path;

use adm_new_foundation::{AdmError, AdmResult, io, sha256_hex, unix_timestamp};
use adm_new_game_spec::{GameSpec, canonicalize_game_spec};
use serde::{Deserialize, Serialize};

use crate::stages::step13_v2::{ScenarioExecutionStatus, Step13AcceptanceOutput, Step13Status};

pub const STEP14_V2_COMPILER_VERSION: &str = "game_spec_step14_r1_packaging_gate.v1";
pub const STANDALONE_RELEASE_EVIDENCE_RELATIVE_PATH: &str =
    "gates/standalone-release-evidence.json";
pub const R1_PIPELINE_GATE_EVIDENCE_FILE: &str = "r1_pipeline_gate_evidence.json";
pub const R1_USER_PLAYTEST_SIGNATURE_FILE: &str = "r1_user_playtest_signature.json";
pub const R1_GATE_EVIDENCE_SOURCE_REPORT_FILE: &str = "r1_gate_evidence_source_report.json";
pub const R1_PIPELINE_GATE_EVIDENCE_SCHEMA_VERSION: &str = "r1_pipeline_gate_evidence.v1";
pub const R1_USER_PLAYTEST_SIGNATURE_SCHEMA_VERSION: &str = "r1_user_playtest_signature.v1";

const STANDALONE_RELEASE_EVIDENCE_SCHEMA_VERSION: u32 = 2;
const STANDALONE_RELEASE_EVIDENCE_PRODUCER: &str = "tools/verify-standalone.ps1/v2";
const STANDALONE_PROJECT_ID: &str = "autodesignmaker-rust-v2";
const STANDALONE_PORTABLE_ROOT: &str = "dist/AutoDesignMaker-NEWrust-release";
const STANDALONE_PORTABLE_OUTPUT_NAME: &str = "AutoDesignMaker-NEWrust-release";
const STANDALONE_RELEASE_EVIDENCE_MAX_LIFETIME_SECONDS: u64 = 86_400;
const STANDALONE_RELEASE_EVIDENCE_CLOCK_SKEW_SECONDS: u64 = 300;
const R1_USER_PLAYTEST_SIGNATURE_KIND: &str = "manual_user_playtest";
const REQUIRED_STANDALONE_RELEASE_CHECKS: &[&str] = &[
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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum R1GateStatus {
    Passed,
    Blocked,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct R1GateEvidence {
    pub reproducible_build: bool,
    pub integrity_passed: bool,
    pub exe_smoke_passed: bool,
    pub standalone_boundary_passed: bool,
    pub ai_usage_evidence_present: bool,
    pub ai_off_flow_supported: bool,
    pub anti_overfit_gates_passed: bool,
    pub content_complete: bool,
    pub user_playtest_signed: bool,
}

impl R1GateEvidence {
    pub fn all_passed_for_tests() -> Self {
        Self {
            reproducible_build: true,
            integrity_passed: true,
            exe_smoke_passed: true,
            standalone_boundary_passed: true,
            ai_usage_evidence_present: true,
            ai_off_flow_supported: true,
            anti_overfit_gates_passed: true,
            content_complete: true,
            user_playtest_signed: true,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct R1PipelineGateEvidence {
    pub schema_version: String,
    pub content_complete: bool,
    pub ai_usage_evidence_present: bool,
    pub ai_off_flow_supported: bool,
    pub anti_overfit_gates_passed: bool,
    pub evidence_refs: BTreeMap<String, String>,
}

impl R1PipelineGateEvidence {
    pub fn all_passed_for_tests() -> Self {
        Self {
            schema_version: R1_PIPELINE_GATE_EVIDENCE_SCHEMA_VERSION.to_string(),
            content_complete: true,
            ai_usage_evidence_present: true,
            ai_off_flow_supported: true,
            anti_overfit_gates_passed: true,
            evidence_refs: BTreeMap::from([
                (
                    "contentComplete".to_string(),
                    "stage_13/step13_acceptance_output.json".to_string(),
                ),
                (
                    "aiUsageEvidence".to_string(),
                    "stage_11/step11_execution_report.json".to_string(),
                ),
                (
                    "aiOffFlow".to_string(),
                    "stage_11/manual_fallback_contract.json".to_string(),
                ),
                (
                    "antiOverfitGates".to_string(),
                    "a09_cross_genre_evaluation_report.json".to_string(),
                ),
            ]),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct R1UserPlaytestSignature {
    pub schema_version: String,
    pub signature_kind: String,
    pub signed: bool,
    pub signer: String,
    pub signed_at_unix: u64,
    pub standalone_evidence_id: String,
    pub acknowledgement: String,
}

impl R1UserPlaytestSignature {
    pub fn manual_for_tests(standalone_evidence_id: &str) -> Self {
        Self {
            schema_version: R1_USER_PLAYTEST_SIGNATURE_SCHEMA_VERSION.to_string(),
            signature_kind: R1_USER_PLAYTEST_SIGNATURE_KIND.to_string(),
            signed: true,
            signer: "test-user".to_string(),
            signed_at_unix: unix_timestamp(),
            standalone_evidence_id: standalone_evidence_id.to_string(),
            acknowledgement: "manual R1 playable build accepted".to_string(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct R1GateEvidenceSourceReport {
    pub schema_version: String,
    pub compiler_version: String,
    pub standalone_release_evidence_path: String,
    pub pipeline_gate_evidence_path: String,
    pub user_playtest_signature_path: String,
    pub standalone_evidence_id: Option<String>,
    pub standalone_tool_checks: BTreeMap<String, bool>,
    pub derived_evidence: R1GateEvidence,
    pub blockers: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct R1GateEvidenceDerivation {
    pub evidence: R1GateEvidence,
    pub report: R1GateEvidenceSourceReport,
}

#[derive(Debug, Clone, Deserialize)]
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

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ReleaseCommandEvidence {
    status: String,
    command: String,
    exit_code: i32,
    duration_ms: u64,
    output_sha256: String,
}

#[derive(Debug, Clone, Deserialize)]
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

#[derive(Debug, Clone)]
struct StandaloneEvidenceAssessment {
    release_valid: bool,
    evidence_id: Option<String>,
    check_results: BTreeMap<String, bool>,
    blockers: Vec<String>,
}

#[derive(Debug, Clone)]
struct PipelineEvidenceAssessment {
    content_complete: bool,
    ai_usage_evidence_present: bool,
    ai_off_flow_supported: bool,
    anti_overfit_gates_passed: bool,
    blockers: Vec<String>,
}

#[derive(Debug, Clone)]
struct PlaytestSignatureAssessment {
    user_playtest_signed: bool,
    blockers: Vec<String>,
}

pub fn derive_r1_gate_evidence_from_sources(
    project_root: &Path,
    stage_dir: &Path,
) -> R1GateEvidenceDerivation {
    derive_r1_gate_evidence_from_sources_at(project_root, stage_dir, unix_timestamp())
}

fn derive_r1_gate_evidence_from_sources_at(
    project_root: &Path,
    stage_dir: &Path,
    now_unix: u64,
) -> R1GateEvidenceDerivation {
    let standalone_path = project_root.join(STANDALONE_RELEASE_EVIDENCE_RELATIVE_PATH);
    let pipeline_path = stage_dir.join(R1_PIPELINE_GATE_EVIDENCE_FILE);
    let signature_path = stage_dir.join(R1_USER_PLAYTEST_SIGNATURE_FILE);
    let standalone = assess_standalone_release_evidence(&standalone_path, now_unix);
    let pipeline = assess_pipeline_gate_evidence(&pipeline_path);
    let signature =
        assess_user_playtest_signature(&signature_path, standalone.evidence_id.as_deref());

    let tool_release_valid = standalone.release_valid;
    let tool = |check_id: &str| {
        tool_release_valid
            && standalone
                .check_results
                .get(check_id)
                .copied()
                .unwrap_or(false)
    };
    let evidence = R1GateEvidence {
        reproducible_build: tool("cargo_fmt_check")
            && tool("cargo_check_workspace")
            && tool("cargo_test_workspace")
            && tool("web_build")
            && tool("portable_build")
            && tool("clean_clone_relocation"),
        integrity_passed: tool("package_contract_self_test")
            && tool("resource_manifest")
            && tool("portable_integrity")
            && tool("pe_architecture_crt")
            && standalone.release_valid,
        exe_smoke_passed: tool("portable_smoke"),
        standalone_boundary_passed: tool("standalone_boundary_gate")
            && tool("anti_fake_scan")
            && tool("clean_clone_relocation"),
        ai_usage_evidence_present: pipeline.ai_usage_evidence_present,
        ai_off_flow_supported: pipeline.ai_off_flow_supported,
        anti_overfit_gates_passed: pipeline.anti_overfit_gates_passed,
        content_complete: pipeline.content_complete,
        user_playtest_signed: signature.user_playtest_signed,
    };

    let mut blockers = Vec::new();
    blockers.extend(standalone.blockers.clone());
    blockers.extend(pipeline.blockers.clone());
    blockers.extend(signature.blockers.clone());
    blockers.sort();
    blockers.dedup();

    let report = R1GateEvidenceSourceReport {
        schema_version: "r1_gate_evidence_source_report.v1".to_string(),
        compiler_version: STEP14_V2_COMPILER_VERSION.to_string(),
        standalone_release_evidence_path: path_string(&standalone_path),
        pipeline_gate_evidence_path: path_string(&pipeline_path),
        user_playtest_signature_path: path_string(&signature_path),
        standalone_evidence_id: standalone.evidence_id,
        standalone_tool_checks: standalone.check_results,
        derived_evidence: evidence.clone(),
        blockers,
    };
    R1GateEvidenceDerivation { evidence, report }
}

fn assess_standalone_release_evidence(
    evidence_path: &Path,
    now_unix: u64,
) -> StandaloneEvidenceAssessment {
    let mut check_results = REQUIRED_STANDALONE_RELEASE_CHECKS
        .iter()
        .map(|check| ((*check).to_string(), false))
        .collect::<BTreeMap<_, _>>();
    let mut blockers = Vec::new();
    if !evidence_path.exists() {
        blockers.push("standalone_release_evidence_missing".to_string());
        return StandaloneEvidenceAssessment {
            release_valid: false,
            evidence_id: None,
            check_results,
            blockers,
        };
    }
    let Ok(metadata) = fs::symlink_metadata(evidence_path) else {
        blockers.push("standalone_release_evidence_unreadable".to_string());
        return StandaloneEvidenceAssessment {
            release_valid: false,
            evidence_id: None,
            check_results,
            blockers,
        };
    };
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        blockers.push("standalone_release_evidence_must_be_regular_file".to_string());
        return StandaloneEvidenceAssessment {
            release_valid: false,
            evidence_id: None,
            check_results,
            blockers,
        };
    }
    let bytes = match fs::read(evidence_path) {
        Ok(bytes) => bytes,
        Err(_) => {
            blockers.push("standalone_release_evidence_unreadable".to_string());
            return StandaloneEvidenceAssessment {
                release_valid: false,
                evidence_id: None,
                check_results,
                blockers,
            };
        }
    };
    let evidence: StandaloneReleaseEvidence = match serde_json::from_slice(&bytes) {
        Ok(evidence) => evidence,
        Err(error) => {
            blockers.push(format!("standalone_release_evidence_json_invalid:{error}"));
            return StandaloneEvidenceAssessment {
                release_valid: false,
                evidence_id: None,
                check_results,
                blockers,
            };
        }
    };

    if evidence.schema_version != STANDALONE_RELEASE_EVIDENCE_SCHEMA_VERSION {
        blockers.push("standalone_release_evidence_schema_invalid".to_string());
    }
    if evidence.producer != STANDALONE_RELEASE_EVIDENCE_PRODUCER {
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
    if evidence.project_id != STANDALONE_PROJECT_ID {
        blockers.push("standalone_release_evidence_project_id_invalid".to_string());
    }
    if evidence.status != "passed" {
        blockers.push("standalone_release_evidence_not_passed".to_string());
    }
    if !evidence.source_tree_clean {
        blockers.push("standalone_release_evidence_source_tree_not_clean".to_string());
    }
    if !evidence.errors.is_empty() {
        blockers.push("standalone_release_evidence_contains_errors".to_string());
    }
    if evidence.generated_at_unix
        > now_unix.saturating_add(STANDALONE_RELEASE_EVIDENCE_CLOCK_SKEW_SECONDS)
        || evidence.expires_at_unix <= now_unix
        || evidence.expires_at_unix <= evidence.generated_at_unix
        || evidence.expires_at_unix - evidence.generated_at_unix
            > STANDALONE_RELEASE_EVIDENCE_MAX_LIFETIME_SECONDS
    {
        blockers.push("standalone_release_evidence_freshness_invalid".to_string());
    }

    let required = REQUIRED_STANDALONE_RELEASE_CHECKS
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
    for check in REQUIRED_STANDALONE_RELEASE_CHECKS {
        let passed = evidence.checks.get(*check).is_some_and(valid_release_check);
        check_results.insert((*check).to_string(), passed);
        if !passed {
            blockers.push(format!(
                "standalone_release_check_missing_or_failed:{check}"
            ));
        }
    }
    if !valid_portable_evidence(&evidence) {
        blockers.push("standalone_release_portable_evidence_invalid".to_string());
    }

    blockers.sort();
    blockers.dedup();
    StandaloneEvidenceAssessment {
        release_valid: blockers.is_empty(),
        evidence_id: Some(evidence.evidence_id),
        check_results,
        blockers,
    }
}

fn valid_release_check(value: &ReleaseCommandEvidence) -> bool {
    let _ = value.duration_ms;
    value.status == "passed"
        && value.exit_code == 0
        && !value.command.trim().is_empty()
        && is_sha256(&value.output_sha256)
}

fn valid_portable_evidence(evidence: &StandaloneReleaseEvidence) -> bool {
    let Some(portable) = &evidence.portable else {
        return false;
    };
    portable.root == STANDALONE_PORTABLE_ROOT
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
                "dist/.{STANDALONE_PORTABLE_OUTPUT_NAME}.swap-{}.json",
                portable.transaction_id
            )
        && is_safe_project_relative_path(&portable.swap_receipt)
}

fn assess_pipeline_gate_evidence(path: &Path) -> PipelineEvidenceAssessment {
    let mut blockers = Vec::new();
    let bytes = match fs::read(path) {
        Ok(bytes) => bytes,
        Err(_) => {
            return PipelineEvidenceAssessment {
                content_complete: false,
                ai_usage_evidence_present: false,
                ai_off_flow_supported: false,
                anti_overfit_gates_passed: false,
                blockers: vec!["r1_pipeline_gate_evidence_missing".to_string()],
            };
        }
    };
    let evidence: R1PipelineGateEvidence = match serde_json::from_slice(&bytes) {
        Ok(evidence) => evidence,
        Err(error) => {
            return PipelineEvidenceAssessment {
                content_complete: false,
                ai_usage_evidence_present: false,
                ai_off_flow_supported: false,
                anti_overfit_gates_passed: false,
                blockers: vec![format!("r1_pipeline_gate_evidence_json_invalid:{error}")],
            };
        }
    };
    if evidence.schema_version != R1_PIPELINE_GATE_EVIDENCE_SCHEMA_VERSION {
        blockers.push("r1_pipeline_gate_evidence_schema_invalid".to_string());
    }
    let content_complete = gate_bool_with_ref(
        &evidence,
        "contentComplete",
        evidence.content_complete,
        &mut blockers,
    );
    let ai_usage_evidence_present = gate_bool_with_ref(
        &evidence,
        "aiUsageEvidence",
        evidence.ai_usage_evidence_present,
        &mut blockers,
    );
    let ai_off_flow_supported = gate_bool_with_ref(
        &evidence,
        "aiOffFlow",
        evidence.ai_off_flow_supported,
        &mut blockers,
    );
    let anti_overfit_gates_passed = gate_bool_with_ref(
        &evidence,
        "antiOverfitGates",
        evidence.anti_overfit_gates_passed,
        &mut blockers,
    );
    PipelineEvidenceAssessment {
        content_complete,
        ai_usage_evidence_present,
        ai_off_flow_supported,
        anti_overfit_gates_passed,
        blockers,
    }
}

fn gate_bool_with_ref(
    evidence: &R1PipelineGateEvidence,
    key: &str,
    value: bool,
    blockers: &mut Vec<String>,
) -> bool {
    if !value {
        blockers.push(format!("r1_pipeline_gate_failed:{key}"));
        return false;
    }
    if evidence
        .evidence_refs
        .get(key)
        .is_none_or(|reference| reference.trim().is_empty())
    {
        blockers.push(format!("r1_pipeline_gate_evidence_ref_missing:{key}"));
        return false;
    }
    true
}

fn assess_user_playtest_signature(
    path: &Path,
    standalone_evidence_id: Option<&str>,
) -> PlaytestSignatureAssessment {
    let mut blockers = Vec::new();
    let bytes = match fs::read(path) {
        Ok(bytes) => bytes,
        Err(_) => {
            return PlaytestSignatureAssessment {
                user_playtest_signed: false,
                blockers: vec!["r1_user_playtest_signature_missing".to_string()],
            };
        }
    };
    let signature: R1UserPlaytestSignature = match serde_json::from_slice(&bytes) {
        Ok(signature) => signature,
        Err(error) => {
            return PlaytestSignatureAssessment {
                user_playtest_signed: false,
                blockers: vec![format!("r1_user_playtest_signature_json_invalid:{error}")],
            };
        }
    };
    if signature.schema_version != R1_USER_PLAYTEST_SIGNATURE_SCHEMA_VERSION {
        blockers.push("r1_user_playtest_signature_schema_invalid".to_string());
    }
    if signature.signature_kind != R1_USER_PLAYTEST_SIGNATURE_KIND {
        blockers.push("r1_user_playtest_signature_kind_invalid".to_string());
    }
    if !signature.signed {
        blockers.push("r1_user_playtest_signature_not_signed".to_string());
    }
    if signature.signer.trim().is_empty() {
        blockers.push("r1_user_playtest_signature_signer_missing".to_string());
    }
    if signature.signed_at_unix == 0 {
        blockers.push("r1_user_playtest_signature_time_missing".to_string());
    }
    if signature.acknowledgement.trim().is_empty() {
        blockers.push("r1_user_playtest_signature_acknowledgement_missing".to_string());
    }
    match standalone_evidence_id {
        Some(expected) if signature.standalone_evidence_id == expected => {}
        Some(_) => {
            blockers.push("r1_user_playtest_signature_release_evidence_mismatch".to_string())
        }
        None => blockers.push("r1_user_playtest_signature_unbound_release_evidence".to_string()),
    }
    PlaytestSignatureAssessment {
        user_playtest_signed: blockers.is_empty(),
        blockers,
    }
}

fn is_safe_project_relative_path(value: &str) -> bool {
    let path = Path::new(value);
    !value.trim().is_empty()
        && !path.is_absolute()
        && path
            .components()
            .all(|component| matches!(component, std::path::Component::Normal(_)))
}

fn is_sha256(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct R1StopGateCheck {
    pub check_id: String,
    pub passed: bool,
    pub evidence: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct R1ReleaseManifest {
    pub schema_version: String,
    pub compiler_version: String,
    pub status: R1GateStatus,
    pub spec_hash: String,
    pub build_hash: String,
    pub scenario_count: usize,
    pub release_signing: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Step14R1PackagingOutput {
    pub schema_version: String,
    pub compiler_version: String,
    pub status: R1GateStatus,
    pub release_manifest: R1ReleaseManifest,
    pub stop_gate_checks: Vec<R1StopGateCheck>,
    pub blockers: Vec<String>,
    pub output_paths: BTreeMap<String, String>,
}

pub fn run_step14_r1_packaging_gate(
    spec: &GameSpec,
    step13: &Step13AcceptanceOutput,
    evidence: &R1GateEvidence,
    out_dir: &Path,
) -> AdmResult<Step14R1PackagingOutput> {
    std::fs::create_dir_all(out_dir)?;
    let spec_hash = canonicalize_game_spec(spec)
        .map_err(|error| AdmError::new(format!("GameSpec hash failed: {error}")))?
        .content_hash;
    let build_hash = release_build_hash(&spec_hash, step13, evidence);
    let checks = stop_gate_checks(step13, evidence);
    let blockers = checks
        .iter()
        .filter(|check| !check.passed)
        .map(|check| check.check_id.clone())
        .collect::<Vec<_>>();
    let status = if blockers.is_empty() {
        R1GateStatus::Passed
    } else {
        R1GateStatus::Blocked
    };
    let release_manifest = R1ReleaseManifest {
        schema_version: "r1_release_manifest.v1".to_string(),
        compiler_version: STEP14_V2_COMPILER_VERSION.to_string(),
        status: status.clone(),
        spec_hash,
        build_hash,
        scenario_count: step13.scenario_results.len(),
        release_signing: "manual_required_for_external_release".to_string(),
    };
    let manifest_path =
        io::write_json_serializable(&out_dir.join("r1_release_manifest.json"), &release_manifest)?;
    let gate_path =
        io::write_json_serializable(&out_dir.join("r1_stop_gate_report.json"), &checks)?;
    let integrity_path = io::write_json_serializable(
        &out_dir.join("package_integrity_report.json"),
        &BTreeMap::from([
            ("reproducibleBuild".to_string(), evidence.reproducible_build),
            ("integrityPassed".to_string(), evidence.integrity_passed),
            (
                "standaloneBoundaryPassed".to_string(),
                evidence.standalone_boundary_passed,
            ),
        ]),
    )?;
    let smoke_path = io::write_json_serializable(
        &out_dir.join("exe_smoke_report.json"),
        &BTreeMap::from([("exeSmokePassed".to_string(), evidence.exe_smoke_passed)]),
    )?;
    let ai_path = io::write_json_serializable(
        &out_dir.join("ai_usage_evidence_summary.json"),
        &BTreeMap::from([
            (
                "aiUsageEvidencePresent".to_string(),
                evidence.ai_usage_evidence_present,
            ),
            (
                "aiOffFlowSupported".to_string(),
                evidence.ai_off_flow_supported,
            ),
        ]),
    )?;
    let output = Step14R1PackagingOutput {
        schema_version: "step14_r1_packaging_output.v1".to_string(),
        compiler_version: STEP14_V2_COMPILER_VERSION.to_string(),
        status,
        release_manifest,
        stop_gate_checks: checks,
        blockers,
        output_paths: BTreeMap::from([
            ("r1ReleaseManifest".to_string(), path_string(&manifest_path)),
            ("r1StopGateReport".to_string(), path_string(&gate_path)),
            (
                "packageIntegrityReport".to_string(),
                path_string(&integrity_path),
            ),
            ("exeSmokeReport".to_string(), path_string(&smoke_path)),
            ("aiUsageEvidenceSummary".to_string(), path_string(&ai_path)),
        ]),
    };
    io::write_json_serializable(&out_dir.join("step14_r1_packaging_output.json"), &output)?;
    Ok(output)
}

fn stop_gate_checks(
    step13: &Step13AcceptanceOutput,
    evidence: &R1GateEvidence,
) -> Vec<R1StopGateCheck> {
    vec![
        check(
            "r1_acceptance_scenarios_passed",
            step13.status == Step13Status::Passed
                && step13
                    .scenario_results
                    .iter()
                    .all(|scenario| scenario.status == ScenarioExecutionStatus::Passed),
            "A08e step13_acceptance_output.json",
        ),
        check(
            "r1_content_complete",
            evidence.content_complete,
            "R1-C0 content charter completion evidence",
        ),
        check(
            "r1_reproducible_build",
            evidence.reproducible_build,
            "repeatable local build evidence",
        ),
        check(
            "r1_integrity_and_standalone",
            evidence.integrity_passed && evidence.standalone_boundary_passed,
            "package integrity and standalone boundary evidence",
        ),
        check(
            "r1_exe_smoke_passed",
            evidence.exe_smoke_passed,
            "double-click EXE smoke evidence",
        ),
        check(
            "r1_ai_evidence_and_ai_off_flow",
            evidence.ai_usage_evidence_present && evidence.ai_off_flow_supported,
            "AI usage audit and manual fallback evidence",
        ),
        check(
            "r1_anti_overfit_gates_passed",
            evidence.anti_overfit_gates_passed,
            "GameSpec anti-overfit gates",
        ),
        check(
            "r1_user_playtest_signed",
            evidence.user_playtest_signed,
            "manual user playtest signature",
        ),
    ]
}

fn check(check_id: &str, passed: bool, evidence: &str) -> R1StopGateCheck {
    R1StopGateCheck {
        check_id: check_id.to_string(),
        passed,
        evidence: evidence.to_string(),
    }
}

fn release_build_hash(
    spec_hash: &str,
    step13: &Step13AcceptanceOutput,
    evidence: &R1GateEvidence,
) -> String {
    sha256_hex(
        format!(
            "{}:{}:{}:{}:{}:{}",
            spec_hash,
            step13.build_hash,
            step13.scenario_results.len(),
            evidence.reproducible_build,
            evidence.exe_smoke_passed,
            evidence.user_playtest_signed
        )
        .as_bytes(),
    )
}

fn path_string(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}
