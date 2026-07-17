use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

use adm_new_ai::adapters::{ClaudeCliAdapter, CodexCliAdapter};
use adm_new_ai::resolution::resolve_active_ai_target;
use adm_new_ai::{
    AiAdapterKind, AiConfigCategory, AiConfigSource, CompletionAdapter, validate_allowed_outputs,
};
use adm_new_change_kernel::{
    ChangeEvidence, ChangeFailureCategory, ChangeOutcome, CommandPurpose, EvidenceStatus,
    SideEffectState, WORKSPACE_CHANGE_SET_SCHEMA_VERSION, WorkspaceChangeSet,
    WorkspaceRelativePath, WorkspaceTransactionResult,
};
use adm_new_contracts::ArtifactLocale;
use adm_new_contracts::ai::{AiConfig, ModelResultStatus, ModelTask};
use adm_new_contracts::execution_object::ExecutionObjectStatus;
use adm_new_foundation::process::terminate_child_process_tree;
use adm_new_foundation::{
    AdmError, AdmResult, StableDirectoryIdentity, acquire_project_write_lock, ensure_relative_path,
    new_stable_id, sha256_hex, write_bytes_atomic as foundation_write_bytes_atomic,
};
use adm_new_pipeline::{
    StyleImageGenerator, StyleImageRequest, StyleImageStatus, WorkUnitExecutionResult,
    WorkUnitExecutionStatus, WorkUnitExecutor, WorkUnitJournalRecord, WorkUnitKind,
    WorkUnitReconcileDecision, WorkUnitRequest,
    stages::step08_10_v2::TrustedDevelopmentTask,
    stages::step11_v2::{Step11FailureEvidence, WorkspaceTaskAgent},
};
use image::{GenericImageView, ImageFormat};
use serde_json::{Value, json};

use crate::execution_objects::{
    ExecutionFailureInput, ExecutionObjectStoreService, begin_program_task_execution_object,
    complete_art_task_execution_object, execution_object_store_path,
    record_execution_object_failure, verify_program_task_execution_object,
};
use crate::project_environment::unity_editor_file_is_valid;
use crate::style_image::style_image_generator_from_config;

#[derive(Debug, Clone)]
enum CliKind {
    Codex,
    Claude,
}

#[derive(Clone)]
/// Legacy development work-unit adapter used by the current desktop pipeline.
///
/// The GameSpec v2 Step11 path uses `WorkspaceTaskAgent` plus
/// `WorkspaceChangeSet` results; this adapter remains a compatibility bridge
/// for R0 and existing Step08-14 execution until the caller surface migrates.
pub struct AiDevelopmentWorkUnitExecutor {
    kind: CliKind,
    program: String,
    project_root: PathBuf,
    execution_objects: ExecutionObjectBinding,
    program_verifier: Arc<dyn ProgramWorkUnitVerifier>,
    execution_scope: String,
}

pub const AI_DEVELOPMENT_EXECUTOR_RETAINED_CALLERS: &[&str] = &[
    "legacy_work_unit_executor",
    "gamespec_v2_workspace_task_agent_bridge",
    "r0_harness",
];
pub const AI_DEVELOPMENT_EXECUTOR_PROHIBITED_CALLERS: &[&str] = &[
    "gamespec_v2_product_step11_direct_work_unit_commit",
    "gamespec_v2_authoritative_execution_without_workspace_change_set",
];
pub const AI_DEVELOPMENT_EXECUTOR_V2_REPLACEMENT: &str =
    "WorkspaceTaskAgent::execute_task with WorkspaceChangeSet validation";

impl fmt::Debug for AiDevelopmentWorkUnitExecutor {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("AiDevelopmentWorkUnitExecutor")
            .field("kind", &self.kind)
            .field("program_configured", &!self.program.is_empty())
            .field("project_root_configured", &true)
            .finish()
    }
}

impl WorkUnitExecutor for AiDevelopmentWorkUnitExecutor {
    fn execution_scope_fingerprint(&self) -> String {
        self.execution_scope.clone()
    }

    fn execute(&self, request: &WorkUnitRequest) -> AdmResult<WorkUnitExecutionResult> {
        if request.kind != WorkUnitKind::Development {
            return Ok(WorkUnitExecutionResult::unavailable(
                "the active development CLI does not produce art work units",
            ));
        }
        let output_files = string_array(request.payload.get("output_files"));
        if output_files.is_empty() {
            return Ok(WorkUnitExecutionResult {
                status: WorkUnitExecutionStatus::Failed,
                output_refs: Vec::new(),
                changed_files: Vec::new(),
                verification_results: Vec::new(),
                data: Value::Null,
                message: "development work unit declares no output files".to_string(),
            });
        }
        let allowed_write_paths = allowed_write_paths(&request.payload, &output_files);
        let input_files = string_array(request.payload.get("input_files"));
        for path in &input_files {
            safe_existing_file(&self.project_root, path)?;
        }
        for path in &output_files {
            safe_write_target(&self.project_root, path)?;
        }
        for path in &allowed_write_paths {
            safe_write_target(&self.project_root, path)?;
        }
        let before_hashes = output_hashes(&self.project_root, &output_files)?;
        let execution_object_output_files = program_execution_object_output_files(&output_files);
        let execution_object_before_hashes =
            output_hashes(&self.project_root, &execution_object_output_files)?;
        let input_hashes = input_hashes(&self.project_root, &input_files)?;
        if let Some(message) = self.program_verifier.preflight_failure() {
            let public_message = localized_work_unit_text(
                request,
                "当前存档未配置可用于批处理验证的 Unity 编辑器。",
                message,
            );
            let execution_object = {
                let _store_lock = acquire_project_write_lock(&self.execution_objects.root)?;
                let mut store = self.execution_objects.open_store()?;
                let executing = begin_program_task_execution_object(
                    &mut store,
                    program_execution_object_task(request, &execution_object_output_files),
                    &self.project_root,
                    request.stage_id.parse().unwrap_or(11),
                )?;
                record_execution_object_failure(
                    &mut store,
                    &executing.execution_object_id,
                    ExecutionFailureInput {
                        failure_stage: "unity_editor_preflight".to_string(),
                        written_files: Vec::new(),
                        changed_state: Vec::new(),
                        unfinished_actions: output_files.clone(),
                        retryable: false,
                        rollback_needed: false,
                        remediation_needed: true,
                        validation_needed: true,
                        error: public_message.clone(),
                    },
                )?
            };
            return Ok(WorkUnitExecutionResult {
                status: WorkUnitExecutionStatus::Failed,
                output_refs: Vec::new(),
                changed_files: Vec::new(),
                verification_results: vec![json!({
                    "id": "unity_batchmode_compile",
                    "status": "failed",
                    "evidence_complete": false,
                })],
                data: json!({
                    "side_effects_committed": false,
                    "input_hashes": input_hashes,
                    "output_hashes": before_hashes,
                    "execution_object_id": execution_object.execution_object_id,
                    "execution_object_state": execution_object.state.as_str(),
                }),
                message: public_message,
            });
        }
        let isolated = IsolatedWorkRoot::new()?;
        for path in &input_files {
            copy_project_file(&self.project_root, isolated.path(), path, true)?;
        }
        for path in &output_files {
            if !input_files.contains(path) {
                copy_project_file(&self.project_root, isolated.path(), path, false)?;
            }
        }
        let isolated_before = directory_manifest(isolated.verified_path()?)?;
        let task = ModelTask {
            task_id: request.unit_id.clone(),
            prompt: work_unit_prompt(request),
            input_files: input_files.clone(),
            output_files: output_files.clone(),
            allowed_write_paths: allowed_write_paths.clone(),
            timeout_seconds: request
                .payload
                .get("timeout_seconds")
                .and_then(Value::as_u64)
                .unwrap_or(1_800)
                .clamp(1, 3_600),
            sandbox: "workspace-write".to_string(),
            cwd: isolated.path().to_string_lossy().to_string(),
        };
        validate_allowed_outputs(&task)?;
        let model_result = match self.kind {
            CliKind::Codex => CodexCliAdapter {
                cli_path: self.program.clone(),
            }
            .generate(&task),
            CliKind::Claude => ClaudeCliAdapter {
                cli_path: self.program.clone(),
            }
            .generate(&task),
        }?;
        if model_result.status != ModelResultStatus::Succeeded {
            return Ok(WorkUnitExecutionResult {
                status: WorkUnitExecutionStatus::Failed,
                output_refs: Vec::new(),
                changed_files: Vec::new(),
                verification_results: Vec::new(),
                data: json!({
                    "side_effects_committed": false,
                    "input_hashes": input_hashes,
                }),
                message: "development CLI returned a failed result; review the private provider diagnostics"
                    .to_string(),
            });
        }
        let isolated_root = isolated.verified_path()?;
        let isolated_after = directory_manifest(isolated_root)?;
        let declared_outputs = output_files.iter().cloned().collect::<BTreeSet<_>>();
        let unexpected_changes = changed_manifest_paths(&isolated_before, &isolated_after)
            .into_iter()
            .filter(|path| !declared_outputs.contains(path))
            .collect::<Vec<_>>();
        if !unexpected_changes.is_empty() {
            return Ok(WorkUnitExecutionResult {
                status: WorkUnitExecutionStatus::Failed,
                output_refs: Vec::new(),
                changed_files: Vec::new(),
                verification_results: vec![json!({
                    "id": "isolated_write_set",
                    "status": "failed",
                    "unexpected_change_count": unexpected_changes.len(),
                })],
                data: json!({
                    "side_effects_committed": false,
                    "input_hashes": input_hashes,
                    "unexpected_changes": unexpected_changes,
                }),
                message: "development CLI changed files outside the declared output set"
                    .to_string(),
            });
        }
        let staged_hashes = output_hashes(isolated_root, &output_files)?;
        let missing = output_files
            .iter()
            .filter(|relative| !staged_hashes.contains_key(*relative))
            .cloned()
            .collect::<Vec<_>>();
        if !missing.is_empty() {
            return Ok(WorkUnitExecutionResult {
                status: WorkUnitExecutionStatus::Failed,
                output_refs: Vec::new(),
                changed_files: Vec::new(),
                verification_results: vec![json!({
                    "id": "declared_outputs_exist",
                    "status": "failed",
                    "missing_count": missing.len(),
                })],
                data: json!({
                    "side_effects_committed": false,
                    "input_hashes": input_hashes,
                }),
                message: format!(
                    "development CLI did not create {} declared output file(s)",
                    missing.len()
                ),
            });
        }
        let _project_write_lock = acquire_project_write_lock(&self.project_root)?;
        if !project_snapshot_is_current(
            &self.project_root,
            &input_files,
            &input_hashes,
            &output_files,
            &before_hashes,
        )? {
            return Ok(WorkUnitExecutionResult {
                status: WorkUnitExecutionStatus::Failed,
                output_refs: Vec::new(),
                changed_files: Vec::new(),
                verification_results: vec![json!({
                    "id": "project_compare_and_swap",
                    "status": "failed",
                })],
                data: json!({
                    "side_effects_committed": false,
                    "input_hashes": input_hashes,
                    "output_hashes": before_hashes,
                }),
                message: "project inputs or declared outputs changed while the development CLI was running; no generated files were committed"
                    .to_string(),
            });
        }
        let execution_object = {
            let _store_lock = acquire_project_write_lock(&self.execution_objects.root)?;
            let mut store = self.execution_objects.open_store()?;
            begin_program_task_execution_object(
                &mut store,
                program_execution_object_task(request, &execution_object_output_files),
                &self.project_root,
                request.stage_id.parse().unwrap_or(11),
            )?
        };
        let execution_object_id = execution_object.execution_object_id.clone();
        if let Err(error) = commit_declared_outputs(
            &self.project_root,
            &isolated,
            &output_files,
            &before_hashes,
            &staged_hashes,
        ) {
            let current_hashes = output_hashes(&self.project_root, &output_files)?;
            let changed_files = output_files
                .iter()
                .filter(|path| before_hashes.get(*path) != current_hashes.get(*path))
                .cloned()
                .collect::<Vec<_>>();
            let side_effects_committed = !changed_files.is_empty();
            let object = record_or_cancel_program_execution_object(
                &self.execution_objects,
                &execution_object_id,
                &changed_files,
                "project_output_commit",
                &localized_work_unit_text(
                    request,
                    "开发输出未能安全提交到 Unity 项目。",
                    &error.to_string(),
                ),
                side_effects_committed,
                work_unit_locale(request),
            )?;
            return Ok(WorkUnitExecutionResult {
                status: WorkUnitExecutionStatus::Failed,
                output_refs: output_files,
                changed_files,
                verification_results: vec![json!({
                    "id": "declared_output_commit",
                    "status": "failed",
                })],
                data: json!({
                    "side_effects_committed": side_effects_committed,
                    "input_hashes": input_hashes,
                    "output_hashes": current_hashes,
                    "execution_object_id": execution_object_id,
                    "execution_object_state": object.state.as_str(),
                }),
                message: localized_work_unit_text(
                    request,
                    "开发输出未能安全提交到 Unity 项目。",
                    "development outputs could not be committed safely",
                ),
            });
        }
        let after_hashes = output_hashes(&self.project_root, &output_files)?;
        let mut verification_results = vec![
            json!({
                "id": "declared_outputs_exist",
                "status": "passed",
            }),
            json!({
                "id": "isolated_write_set",
                "status": "passed",
                "unexpected_change_count": 0,
            }),
            json!({
                "id": "declared_output_hashes",
                "status": "passed",
                "file_count": after_hashes.len(),
            }),
            json!({
                "id": "project_compare_and_swap",
                "status": "passed",
            }),
        ];
        let unity_verification = self
            .program_verifier
            .verify(request, &self.project_root, &after_hashes)
            .unwrap_or_else(|_| {
                ProgramVerificationOutcome::failed(
                    "Unity batchmode verification could not be completed",
                    json!({
                        "id": "unity_batchmode_compile",
                        "status": "failed",
                        "evidence_complete": false,
                    }),
                )
            });
        verification_results.push(unity_verification.result.clone());
        let execution_object_after_hashes =
            output_hashes(&self.project_root, &execution_object_output_files)?;
        let changed_files = execution_object_output_files
            .iter()
            .filter(|path| {
                execution_object_before_hashes.get(*path)
                    != execution_object_after_hashes.get(*path)
            })
            .cloned()
            .collect::<Vec<_>>();
        if !unity_verification.passed {
            let public_message = localized_work_unit_text(
                request,
                "Unity 批处理编译验证失败；开发输出尚未通过验证。",
                &unity_verification.message,
            );
            let object = record_or_cancel_program_execution_object(
                &self.execution_objects,
                &execution_object_id,
                &changed_files,
                "unity_batchmode_compile",
                &public_message,
                true,
                work_unit_locale(request),
            )?;
            return Ok(WorkUnitExecutionResult {
                status: WorkUnitExecutionStatus::Failed,
                output_refs: output_files,
                changed_files,
                verification_results,
                data: json!({
                    "adapter": match self.kind { CliKind::Codex => "codex", CliKind::Claude => "claude" },
                    "output_hashes": after_hashes,
                    "input_hashes": input_hashes,
                    "unexpected_changes": [],
                    "side_effects_committed": true,
                    "execution_object_id": execution_object_id,
                    "execution_object_state": object.state.as_str(),
                }),
                message: public_message,
            });
        }
        let verified_execution_object = {
            let _store_lock = acquire_project_write_lock(&self.execution_objects.root)?;
            let mut store = self.execution_objects.open_store()?;
            match verify_program_task_execution_object(
                &mut store,
                &execution_object_id,
                &self.project_root,
                &execution_object_output_files,
                &changed_files,
                verification_results.clone(),
                json!({
                    "unit_id": request.unit_id,
                    "idempotency_key": request.idempotency_key,
                    "input_hashes": input_hashes,
                    "output_hashes": after_hashes,
                    "execution_object_output_hashes": execution_object_after_hashes,
                    "unity_verification": unity_verification.result,
                }),
            ) {
                Ok(object) => object,
                Err(error) => {
                    let _ = record_execution_object_failure(
                        &mut store,
                        &execution_object_id,
                        ExecutionFailureInput {
                            failure_stage: "execution_object_verification".to_string(),
                            written_files: output_files.clone(),
                            changed_state: vec![localized_work_unit_text(
                                request,
                                "Unity 项目输出已提交",
                                "Unity project outputs were committed",
                            )],
                            unfinished_actions: vec![localized_work_unit_text(
                                request,
                                "执行对象验证",
                                "execution object verification",
                            )],
                            retryable: false,
                            rollback_needed: true,
                            remediation_needed: true,
                            validation_needed: true,
                            error: localized_work_unit_text(
                                request,
                                "执行对象未能完成验证。",
                                &safe_message(&error.to_string()),
                            ),
                        },
                    );
                    return Err(error);
                }
            }
        };
        if verified_execution_object.state != ExecutionObjectStatus::Verified {
            return Err(AdmError::new(
                "program execution object did not reach verified state",
            ));
        }
        Ok(WorkUnitExecutionResult::verified(
            output_files.clone(),
            changed_files,
            verification_results,
            json!({
                "adapter": match self.kind { CliKind::Codex => "codex", CliKind::Claude => "claude" },
                "output_hashes": after_hashes,
                "input_hashes": input_hashes,
                "unexpected_changes": [],
                "side_effects_committed": true,
                "execution_object_id": verified_execution_object.execution_object_id,
                "execution_object_state": verified_execution_object.state.as_str(),
            }),
        ))
    }

    fn reconcile(
        &self,
        request: &WorkUnitRequest,
        record: &WorkUnitJournalRecord,
    ) -> AdmResult<WorkUnitReconcileDecision> {
        if record.result.is_none() {
            return Ok(
                if request_has_non_discarded_execution_object(&self.execution_objects, request)? {
                    WorkUnitReconcileDecision::Unknown
                } else {
                    // This executor persists the EO before its first Unity-project commit.
                    // No matching EO therefore proves this Started journal did not commit.
                    WorkUnitReconcileDecision::SafeToRetry
                },
            );
        }
        if record
            .result
            .as_ref()
            .and_then(|result| result.data.get("side_effects_committed"))
            .and_then(Value::as_bool)
            == Some(false)
        {
            return Ok(
                if request_has_unresolved_execution_object(&self.execution_objects, request)? {
                    WorkUnitReconcileDecision::Unknown
                } else {
                    WorkUnitReconcileDecision::SafeToRetry
                },
            );
        }
        let input_files = string_array(request.payload.get("input_files"));
        let current_input_hashes = input_hashes(&self.project_root, &input_files)?;
        let recorded_input_hashes = record
            .result
            .as_ref()
            .and_then(|result| result.data.get("input_hashes"))
            .and_then(Value::as_object)
            .map(json_hash_map);
        if recorded_input_hashes.as_ref() != Some(&current_input_hashes) {
            return Ok(
                if request_has_unresolved_execution_object(&self.execution_objects, request)? {
                    WorkUnitReconcileDecision::Unknown
                } else {
                    WorkUnitReconcileDecision::SafeToRetry
                },
            );
        }
        let output_files = record
            .result
            .as_ref()
            .map(|result| result.output_refs.clone())
            .filter(|items| !items.is_empty())
            .unwrap_or_else(|| string_array(request.payload.get("output_files")));
        if output_files.is_empty() {
            return Ok(WorkUnitReconcileDecision::Unknown);
        }
        let current_hashes = output_hashes(&self.project_root, &output_files)?;
        if current_hashes.is_empty() {
            Ok(
                if request_has_unresolved_execution_object(&self.execution_objects, request)? {
                    WorkUnitReconcileDecision::Unknown
                } else {
                    WorkUnitReconcileDecision::SafeToRetry
                },
            )
        } else if current_hashes.len() != output_files.len() {
            Ok(WorkUnitReconcileDecision::Unknown)
        } else {
            let recorded_hashes = record
                .result
                .as_ref()
                .filter(|result| result.status == WorkUnitExecutionStatus::Verified)
                .and_then(|result| result.data.get("output_hashes"))
                .and_then(Value::as_object)
                .map(json_hash_map);
            match recorded_hashes {
                Some(recorded)
                    if recorded == current_hashes
                        && verified_execution_object_id(
                            &self.execution_objects,
                            request,
                            record,
                        )?
                        .is_some() =>
                {
                    Ok(WorkUnitReconcileDecision::Verified)
                }
                _ => Ok(WorkUnitReconcileDecision::Unknown),
            }
        }
    }
}

impl WorkspaceTaskAgent for AiDevelopmentWorkUnitExecutor {
    fn execute_task(
        &self,
        task: &TrustedDevelopmentTask,
        attempt: u32,
        previous_failure: Option<&Step11FailureEvidence>,
    ) -> AdmResult<WorkspaceTransactionResult> {
        let request = workspace_task_to_work_unit_request(
            task,
            attempt,
            previous_failure,
            self.execution_scope_fingerprint(),
        )?;
        let execution = <Self as WorkUnitExecutor>::execute(self, &request)?;
        workspace_transaction_from_work_unit(task, attempt, &execution)
    }
}

fn workspace_task_to_work_unit_request(
    task: &TrustedDevelopmentTask,
    attempt: u32,
    previous_failure: Option<&Step11FailureEvidence>,
    execution_scope: String,
) -> AdmResult<WorkUnitRequest> {
    let contract = &task.workspace_contract;
    let output_files = if task.declared_write_paths.is_empty() {
        workspace_path_strings(&contract.agent_write_paths)
    } else {
        task.declared_write_paths.clone()
    };
    let payload = json!({
        "schema_version": "workspace_task_agent_bridge.v1",
        "artifact_locale": "en-US",
        "task_id": task.task_id,
        "title": task.title,
        "architecture_system_id": task.architecture_system_id,
        "ordinal_size": task.ordinal_size,
        "attempt": attempt,
        "dependencies": task.dependencies,
        "input_files": workspace_path_strings(&contract.read_paths),
        "output_files": output_files,
        "allowed_write_paths": workspace_path_strings(&contract.agent_write_paths),
        "workspace_contract_hash": contract.contract_hash().map_err(|error| {
            AdmError::new(format!("failed to hash WorkspaceChangeSet: {error}"))
        })?,
        "workspace_contract": contract,
        "machine_checks": task.machine_checks,
        "previous_failure": previous_failure.map(|failure| json!({
            "failure_kind": format!("{:?}", failure.failure_kind).to_ascii_lowercase(),
            "reason": failure.reason,
            "issue_codes": failure.issue_codes,
        })),
        "instructions": [
            "Modify only files listed in output_files / allowed_write_paths.",
            "Do not modify trusted tests or files outside the declared WorkspaceChangeSet.",
            "Return only after compile and test checks are satisfied."
        ],
    });
    let mut request =
        WorkUnitRequest::new("11", &task.task_id, WorkUnitKind::Development, payload)?;
    request.execution_scope = execution_scope;
    request.idempotency_key = sha256_hex(
        format!(
            "workspace-task-agent-v1:{}:{}:{}",
            request.idempotency_key, request.execution_scope, attempt
        )
        .as_bytes(),
    );
    Ok(request)
}

fn workspace_transaction_from_work_unit(
    task: &TrustedDevelopmentTask,
    attempt: u32,
    execution: &WorkUnitExecutionResult,
) -> AdmResult<WorkspaceTransactionResult> {
    let contract = &task.workspace_contract;
    let contract_hash = contract
        .contract_hash()
        .map_err(|error| AdmError::new(format!("failed to hash WorkspaceChangeSet: {error}")))?;
    let agent_changed_paths = workspace_paths_from_strings(&execution.changed_files)?;
    let side_effects_committed = execution
        .data
        .get("side_effects_committed")
        .and_then(Value::as_bool)
        .unwrap_or(execution.status == WorkUnitExecutionStatus::Verified);
    let status_passed = execution.status == WorkUnitExecutionStatus::Verified;
    let failure_category = if status_passed {
        None
    } else {
        Some(work_unit_failure_category(execution))
    };
    let side_effect_state = if status_passed {
        SideEffectState::Committed
    } else if side_effects_committed {
        SideEffectState::CommittedRecoveryBlocked
    } else {
        SideEffectState::None
    };
    let resulting_tree_hash = if side_effects_committed || status_passed {
        Some(sha256_hex(
            serde_json::to_vec(&json!({
                "taskId": task.task_id,
                "attempt": attempt,
                "changedFiles": execution.changed_files,
                "data": execution.data,
            }))
            .map_err(|error| {
                AdmError::new(format!("failed to encode workspace result hash: {error}"))
            })?
            .as_slice(),
        ))
    } else {
        None
    };
    Ok(WorkspaceTransactionResult {
        schema_version: WORKSPACE_CHANGE_SET_SCHEMA_VERSION.to_string(),
        change_set_id: contract.change_set_id.clone(),
        contract_sha256: contract_hash,
        base_tree_hash: contract.base_tree_hash.clone(),
        outcome: if status_passed {
            ChangeOutcome::Committed
        } else {
            ChangeOutcome::Rejected
        },
        failure_category,
        side_effect_state,
        stage: "ai_development_work_unit_workspace_agent".to_string(),
        resulting_tree_hash,
        agent_changed_paths,
        trusted_tool_changed_paths: BTreeSet::new(),
        build_output_changed_paths: BTreeSet::new(),
        trusted_test_hashes: contract
            .trusted_tests
            .iter()
            .map(|test| (test.test_id.clone(), test.baseline_sha256.clone()))
            .collect(),
        evidence: vec![ChangeEvidence::from_bytes(
            "ai_development_work_unit_result",
            "step11",
            if status_passed {
                EvidenceStatus::Passed
            } else {
                EvidenceStatus::Failed
            },
            serde_json::to_vec(&json!({
                "taskId": task.task_id,
                "attempt": attempt,
                "status": format!("{:?}", execution.status),
                "verificationResults": execution.verification_results,
                "message": execution.message,
            }))
            .map_err(|error| {
                AdmError::new(format!("failed to encode work-unit evidence: {error}"))
            })?
            .as_slice(),
        )],
    })
}

fn workspace_path_strings(paths: &BTreeSet<WorkspaceRelativePath>) -> Vec<String> {
    paths.iter().map(|path| path.as_str().to_string()).collect()
}

fn workspace_paths_from_strings(paths: &[String]) -> AdmResult<BTreeSet<WorkspaceRelativePath>> {
    paths
        .iter()
        .map(|path| {
            WorkspaceRelativePath::parse(path).map_err(|error| {
                AdmError::new(format!("invalid WorkspaceChangeSet result path: {error}"))
            })
        })
        .collect()
}

fn work_unit_failure_category(execution: &WorkUnitExecutionResult) -> ChangeFailureCategory {
    if execution.status == WorkUnitExecutionStatus::Unavailable {
        return ChangeFailureCategory::Tooling;
    }
    let text = serde_json::to_string(&execution.verification_results)
        .unwrap_or_default()
        .to_ascii_lowercase();
    if text.contains("trusted") || text.contains("test") {
        ChangeFailureCategory::Test
    } else if text.contains("compile") || text.contains("unity") {
        ChangeFailureCategory::Compile
    } else if text.contains("scope") || text.contains("write_set") {
        ChangeFailureCategory::ScopeViolation
    } else {
        ChangeFailureCategory::AgentError
    }
}

#[derive(Debug, Clone)]
struct UnavailableWorkUnitExecutor {
    message: String,
}

#[derive(Clone)]
struct AiArtWorkUnitExecutor {
    generator: Arc<dyn StyleImageGenerator>,
    project_root: PathBuf,
    execution_objects: ExecutionObjectBinding,
    execution_scope: String,
}

#[derive(Debug, Clone)]
struct ExecutionObjectBinding {
    root: PathBuf,
    owner_id: String,
}

#[derive(Debug, Clone)]
struct ProgramVerificationOutcome {
    passed: bool,
    result: Value,
    message: String,
}

impl ProgramVerificationOutcome {
    fn passed(result: Value) -> Self {
        Self {
            passed: true,
            result,
            message: String::new(),
        }
    }

    fn failed(message: impl Into<String>, result: Value) -> Self {
        Self {
            passed: false,
            result,
            message: message.into(),
        }
    }
}

trait ProgramWorkUnitVerifier: Send + Sync + fmt::Debug {
    fn execution_scope_fingerprint(&self) -> String;

    fn preflight_failure(&self) -> Option<&str> {
        None
    }

    fn verify(
        &self,
        request: &WorkUnitRequest,
        project_root: &Path,
        expected_output_hashes: &BTreeMap<String, String>,
    ) -> AdmResult<ProgramVerificationOutcome>;
}

#[derive(Debug, Clone)]
struct UnavailableProgramVerifier {
    message: String,
}

impl ProgramWorkUnitVerifier for UnavailableProgramVerifier {
    fn execution_scope_fingerprint(&self) -> String {
        "unity-batchmode-verifier-unavailable-v1".to_string()
    }

    fn preflight_failure(&self) -> Option<&str> {
        Some(&self.message)
    }

    fn verify(
        &self,
        _request: &WorkUnitRequest,
        _project_root: &Path,
        _expected_output_hashes: &BTreeMap<String, String>,
    ) -> AdmResult<ProgramVerificationOutcome> {
        Ok(ProgramVerificationOutcome::failed(
            self.message.clone(),
            json!({
                "id": "unity_batchmode_compile",
                "status": "failed",
                "evidence_complete": false,
            }),
        ))
    }
}

#[derive(Debug, Clone)]
struct UnityBatchmodeProgramVerifier {
    editor_path: PathBuf,
    draft_root: PathBuf,
    log_root: PathBuf,
    execution_scope: String,
}

impl UnityBatchmodeProgramVerifier {
    fn new(editor_path: &Path, execution_object_root: &Path) -> AdmResult<Self> {
        let editor_path = fs::canonicalize(editor_path)
            .map_err(|_| AdmError::new("configured Unity editor is unavailable"))?;
        if !unity_editor_file_is_valid(&editor_path) {
            return Err(AdmError::new(
                "configured Unity editor is not a compatible Unity executable",
            ));
        }
        let log_root = execution_object_root
            .join("outputs")
            .join("execution_objects")
            .join("unity_logs");
        fs::create_dir_all(&log_root)
            .map_err(|_| AdmError::new("Unity verification log directory could not be created"))?;
        let log_root = fs::canonicalize(log_root)
            .map_err(|_| AdmError::new("Unity verification log directory is unavailable"))?;
        if !log_root.starts_with(execution_object_root) {
            return Err(AdmError::new(
                "Unity verification log directory escapes the execution object draft root",
            ));
        }
        let execution_scope = sha256_hex(
            format!(
                "unity-batchmode-verifier-v1|{}|{}",
                editor_path.to_string_lossy(),
                log_root.to_string_lossy()
            )
            .as_bytes(),
        );
        Ok(Self {
            editor_path,
            draft_root: execution_object_root.to_path_buf(),
            log_root,
            execution_scope,
        })
    }
}

#[derive(Debug, Clone)]
struct UnityValidationPlan {
    command_id: String,
    purpose: CommandPurpose,
    argument_template: Vec<String>,
}

#[derive(Debug, Clone)]
struct UnityValidationCheck {
    purpose: CommandPurpose,
    passed: bool,
    result: Value,
}

impl ProgramWorkUnitVerifier for UnityBatchmodeProgramVerifier {
    fn execution_scope_fingerprint(&self) -> String {
        self.execution_scope.clone()
    }

    fn verify(
        &self,
        request: &WorkUnitRequest,
        project_root: &Path,
        expected_output_hashes: &BTreeMap<String, String>,
    ) -> AdmResult<ProgramVerificationOutcome> {
        let timeout_seconds = request
            .payload
            .get("unity_validation_timeout_seconds")
            .and_then(Value::as_u64)
            .unwrap_or(600)
            .clamp(30, 1_800);
        let requires_smoke = request
            .payload
            .get("schema_version")
            .and_then(Value::as_str)
            == Some("workspace_task_agent_bridge.v1");
        let plans = unity_validation_plans(request)?;
        let has_compile_plan = plans
            .iter()
            .any(|plan| plan.purpose == CommandPurpose::Compile);
        let has_smoke_plan = plans
            .iter()
            .any(|plan| matches!(plan.purpose, CommandPurpose::Test | CommandPurpose::Smoke));
        if !has_compile_plan || (requires_smoke && !has_smoke_plan) {
            let result = json!({
                "id": "unity_batchmode_compile_and_smoke",
                "status": "failed",
                "compile_plan_present": has_compile_plan,
                "smoke_plan_present": has_smoke_plan,
                "requires_smoke": requires_smoke,
            });
            return Ok(ProgramVerificationOutcome::failed(
                "Workspace task contract did not declare both compile and trusted smoke/test checks",
                result,
            ));
        }

        let mut checks = Vec::new();
        for plan in &plans {
            checks.push(self.run_validation_plan(
                request,
                project_root,
                expected_output_hashes,
                timeout_seconds,
                plan,
            )?);
        }
        let compile_passed = checks
            .iter()
            .any(|check| check.purpose == CommandPurpose::Compile && check.passed);
        let smoke_passed = if requires_smoke {
            checks.iter().any(|check| {
                matches!(check.purpose, CommandPurpose::Test | CommandPurpose::Smoke)
                    && check.passed
            })
        } else {
            true
        };
        let passed = compile_passed && smoke_passed;
        let result = json!({
            "id": "unity_batchmode_compile_and_smoke",
            "status": if passed { "passed" } else { "failed" },
            "requires_smoke": requires_smoke,
            "compile_passed": compile_passed,
            "smoke_passed": smoke_passed,
            "check_count": checks.len(),
            "checks": checks.iter().map(|check| check.result.clone()).collect::<Vec<_>>(),
        });
        Ok(if passed {
            ProgramVerificationOutcome::passed(result)
        } else {
            ProgramVerificationOutcome::failed(
                if !compile_passed {
                    "Unity batchmode compile verification failed"
                } else {
                    "Unity trusted smoke/test verification failed"
                },
                result,
            )
        })
    }
}

impl UnityBatchmodeProgramVerifier {
    fn run_validation_plan(
        &self,
        request: &WorkUnitRequest,
        project_root: &Path,
        expected_output_hashes: &BTreeMap<String, String>,
        timeout_seconds: u64,
        plan: &UnityValidationPlan,
    ) -> AdmResult<UnityValidationCheck> {
        let log_name = format!(
            "{}-{}-{}.log",
            sha256_hex(request.unit_id.as_bytes())
                .chars()
                .take(16)
                .collect::<String>(),
            plan.command_id,
            new_stable_id("unity-verify")?
        );
        let log_path = self.log_root.join(log_name);
        let editor_path = unity_external_process_path(&self.editor_path);
        let project_path = unity_external_process_path(project_root);
        let process_log_path = unity_external_process_path(&log_path);
        let mut command = Command::new(editor_path);
        command
            .args(["-batchmode", "-quit", "-projectPath"])
            .arg(&project_path)
            .arg("-logFile")
            .arg(&process_log_path)
            .current_dir(&project_path)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        if !matches!(plan.purpose, CommandPurpose::Compile) {
            for argument in &plan.argument_template {
                command.arg(resolve_unity_argument(
                    argument,
                    &project_path,
                    &process_log_path,
                ));
            }
        }
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            const CREATE_NO_WINDOW: u32 = 0x0800_0000;
            command.creation_flags(CREATE_NO_WINDOW);
        }
        let mut child = command
            .spawn()
            .map_err(|_| AdmError::new("Unity batchmode verification could not be started"))?;
        let started = Instant::now();
        let mut timed_out = false;
        let status = loop {
            match child.try_wait() {
                Ok(Some(status)) => break status,
                Ok(None) => {}
                Err(_) => {
                    let _ = terminate_child_process_tree(&mut child);
                    return Err(AdmError::new("Unity batchmode verification wait failed"));
                }
            }
            if started.elapsed() >= Duration::from_secs(timeout_seconds) {
                timed_out = true;
                break terminate_child_process_tree(&mut child).map_err(|_| {
                    AdmError::new("Unity batchmode verification timeout cleanup failed")
                })?;
            }
            thread::sleep(Duration::from_millis(100));
        };
        let log_bytes = if log_path.is_file() {
            read_bounded_file(
                &log_path,
                16 * 1024 * 1024,
                "Unity batchmode verification log exceeded the size limit",
            )?
        } else {
            Vec::new()
        };
        let log_text = String::from_utf8_lossy(&log_bytes).to_ascii_lowercase();
        let compile_error = [
            ": error cs",
            "scripts have compiler errors",
            "compilation failed",
            "compile errors in player scripts",
        ]
        .iter()
        .any(|marker| log_text.contains(marker));
        let success_marker = [
            "exiting batchmode successfully",
            "batchmode quit successfully",
        ]
        .iter()
        .any(|marker| log_text.contains(marker));
        let test_success_marker = [
            "test run finished",
            "run finished",
            "0 failed",
            "passed",
            "tests passed",
        ]
        .iter()
        .any(|marker| log_text.contains(marker));
        let markers_verified = project_root.join("Assets").is_dir()
            && project_root.join("ProjectSettings").is_dir()
            && project_root.join("Packages/manifest.json").is_file();
        let output_files = expected_output_hashes.keys().cloned().collect::<Vec<_>>();
        let current_output_hashes = output_hashes(project_root, &output_files)?;
        let output_hashes_verified =
            !expected_output_hashes.is_empty() && current_output_hashes == *expected_output_hashes;
        let log_ref = log_path
            .strip_prefix(&self.draft_root)
            .unwrap_or(&log_path)
            .to_string_lossy()
            .replace('\\', "/");
        let marker_passed = match plan.purpose {
            CommandPurpose::Compile => success_marker,
            CommandPurpose::Test | CommandPurpose::Smoke => success_marker || test_success_marker,
            CommandPurpose::Tooling => success_marker,
        };
        let passed = status.success()
            && !timed_out
            && !log_bytes.is_empty()
            && !compile_error
            && marker_passed
            && markers_verified
            && output_hashes_verified;
        let result = json!({
            "id": format!("unity_batchmode_{}", plan.command_id),
            "command_id": plan.command_id,
            "purpose": format!("{:?}", plan.purpose).to_ascii_lowercase(),
            "status": if passed { "passed" } else { "failed" },
            "exit_code": status.code(),
            "timed_out": timed_out,
            "log_ref": log_ref,
            "log_sha256": if log_bytes.is_empty() { String::new() } else { sha256_hex(&log_bytes) },
            "log_bytes": log_bytes.len(),
            "success_marker_found": success_marker,
            "test_success_marker_found": test_success_marker,
            "compiler_error_found": compile_error,
            "project_markers_verified": markers_verified,
            "output_hashes_verified": output_hashes_verified,
        });
        Ok(UnityValidationCheck {
            purpose: plan.purpose,
            passed,
            result,
        })
    }
}

impl fmt::Debug for AiArtWorkUnitExecutor {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("AiArtWorkUnitExecutor")
            .field("generator", &"configured")
            .field("project_root_configured", &true)
            .finish()
    }
}

impl WorkUnitExecutor for AiArtWorkUnitExecutor {
    fn execution_scope_fingerprint(&self) -> String {
        self.execution_scope.clone()
    }

    fn execute(&self, request: &WorkUnitRequest) -> AdmResult<WorkUnitExecutionResult> {
        if request.kind != WorkUnitKind::Art {
            return Ok(WorkUnitExecutionResult::unavailable(
                "the active image provider only executes art work units",
            ));
        }
        let target = request
            .payload
            .get("unity_target_path")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .ok_or_else(|| AdmError::new("art work unit has no target path"))?;
        if !target.to_ascii_lowercase().ends_with(".png") {
            return Ok(WorkUnitExecutionResult {
                status: WorkUnitExecutionStatus::Failed,
                output_refs: vec![target.to_string()],
                changed_files: Vec::new(),
                verification_results: Vec::new(),
                data: Value::Null,
                message: "art work unit target must be a PNG file".to_string(),
            });
        }
        safe_write_target(&self.project_root, target)?;
        let target_files = vec![target.to_string()];
        let before_hashes = output_hashes(&self.project_root, &target_files)?;
        let (width, height) = parse_dimensions(
            request
                .payload
                .get("dimensions")
                .and_then(Value::as_str)
                .unwrap_or("512x512"),
        );
        let prompt = request
            .payload
            .get("generation_prompt")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .unwrap_or_else(|| {
                if work_unit_locale(request) == ArtifactLocale::ZhCn {
                    "生成已声明的游戏美术资产。"
                } else {
                    "Create the declared game art asset."
                }
            });
        let generated = self.generator.generate(&StyleImageRequest {
            unit_id: request.unit_id.clone(),
            style_id: request.task_id.clone(),
            prompt: prompt.to_string(),
            project_label: request
                .payload
                .get("asset_id")
                .and_then(Value::as_str)
                .unwrap_or("art_asset")
                .to_string(),
            requested_width: width,
            requested_height: height,
            output_format: "png".to_string(),
        })?;
        if generated.status != StyleImageStatus::Generated {
            return Ok(WorkUnitExecutionResult {
                status: if generated.status == StyleImageStatus::Unavailable {
                    WorkUnitExecutionStatus::Unavailable
                } else {
                    WorkUnitExecutionStatus::Failed
                },
                output_refs: vec![target.to_string()],
                changed_files: Vec::new(),
                verification_results: Vec::new(),
                data: Value::Null,
                message: if generated.safe_message.trim().is_empty() {
                    "image provider did not produce a verified PNG".to_string()
                } else {
                    safe_message(&generated.safe_message)
                },
            });
        }
        let bytes = generated.validate_generated()?.to_vec();
        let decoded = image::load_from_memory_with_format(&bytes, ImageFormat::Png)
            .map_err(|_| AdmError::new("art provider returned an incomplete PNG"))?;
        let (decoded_width, decoded_height) = decoded.dimensions();
        if decoded_width <= 1
            || decoded_height <= 1
            || decoded_width != generated.width
            || decoded_height != generated.height
            || u64::from(decoded_width) * u64::from(decoded_height) > 16 * 1024 * 1024
        {
            return Err(AdmError::new(
                "art provider returned invalid or inconsistent PNG dimensions",
            ));
        }
        let _project_write_lock = acquire_project_write_lock(&self.project_root)?;
        if output_hashes(&self.project_root, &target_files)? != before_hashes {
            return Ok(WorkUnitExecutionResult {
                status: WorkUnitExecutionStatus::Failed,
                output_refs: Vec::new(),
                changed_files: Vec::new(),
                verification_results: vec![json!({
                    "id": "project_compare_and_swap",
                    "status": "failed",
                })],
                data: json!({
                    "side_effects_committed": false,
                    "output_hashes": before_hashes,
                }),
                message: "the declared art target changed while image generation was running; the generated image was not committed"
                    .to_string(),
            });
        }
        write_binary_atomic(&self.project_root, target, &bytes)?;
        let hash = sha256_hex(&bytes);
        let changed_files = if before_hashes.get(target) == Some(&hash) {
            Vec::new()
        } else {
            vec![target.to_string()]
        };
        let verification_results = vec![
            json!({
                "id": "generated_png_committed",
                "status": "passed",
                "width": generated.width,
                "height": generated.height,
            }),
            json!({
                "id": "project_compare_and_swap",
                "status": "passed",
            }),
        ];
        let produced_record = json!({
            "task_id": request.task_id,
            "unit_id": request.unit_id,
            "idempotency_key": request.idempotency_key,
            "path": target,
            "output_hash": hash.clone(),
            "provider": generated.provider.clone(),
            "model": generated.model.clone(),
            "width": generated.width,
            "height": generated.height,
            "verification_results": verification_results.clone(),
        });
        let execution_object = {
            let _store_lock = acquire_project_write_lock(&self.execution_objects.root)?;
            let mut store = self.execution_objects.open_store()?;
            complete_art_task_execution_object(
                &mut store,
                execution_object_task(request),
                produced_record,
                request.stage_id.parse().unwrap_or(12),
            )?
        };
        if execution_object.state != ExecutionObjectStatus::Verified {
            return Err(AdmError::new(
                "art execution object did not reach verified state",
            ));
        }
        Ok(WorkUnitExecutionResult::verified(
            vec![target.to_string()],
            changed_files,
            verification_results,
            json!({
                "output_hashes": { target: hash },
                "provider": generated.provider,
                "model": generated.model,
                "side_effects_committed": true,
                "execution_object_id": execution_object.execution_object_id,
                "execution_object_state": execution_object.state.as_str(),
            }),
        ))
    }

    fn reconcile(
        &self,
        request: &WorkUnitRequest,
        record: &WorkUnitJournalRecord,
    ) -> AdmResult<WorkUnitReconcileDecision> {
        reconcile_execution_object_and_outputs(
            &self.project_root,
            &self.execution_objects,
            request,
            record,
            "unity_target_path",
        )
    }
}

#[derive(Clone)]
struct RoutedWorkUnitExecutor {
    development: Arc<dyn WorkUnitExecutor>,
    art: Arc<dyn WorkUnitExecutor>,
    execution_scope: String,
}

impl fmt::Debug for RoutedWorkUnitExecutor {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("RoutedWorkUnitExecutor")
            .field("development", &"configured")
            .field("art", &"configured")
            .finish()
    }
}

impl WorkUnitExecutor for RoutedWorkUnitExecutor {
    fn execution_scope_fingerprint(&self) -> String {
        self.execution_scope.clone()
    }

    fn execute(&self, request: &WorkUnitRequest) -> AdmResult<WorkUnitExecutionResult> {
        match request.kind {
            WorkUnitKind::Development => self.development.execute(request),
            WorkUnitKind::Art => self.art.execute(request),
        }
    }

    fn reconcile(
        &self,
        request: &WorkUnitRequest,
        record: &WorkUnitJournalRecord,
    ) -> AdmResult<WorkUnitReconcileDecision> {
        match request.kind {
            WorkUnitKind::Development => self.development.reconcile(request, record),
            WorkUnitKind::Art => self.art.reconcile(request, record),
        }
    }
}

impl WorkUnitExecutor for UnavailableWorkUnitExecutor {
    fn execution_scope_fingerprint(&self) -> String {
        "unavailable-work-unit-executor-v1".to_string()
    }

    fn execute(&self, _request: &WorkUnitRequest) -> AdmResult<WorkUnitExecutionResult> {
        Ok(WorkUnitExecutionResult::unavailable(self.message.clone()))
    }

    fn reconcile(
        &self,
        _request: &WorkUnitRequest,
        _record: &WorkUnitJournalRecord,
    ) -> AdmResult<WorkUnitReconcileDecision> {
        Ok(WorkUnitReconcileDecision::Unknown)
    }
}

pub fn work_unit_executor_from_config(
    config: &AiConfig,
    project_root: &Path,
    unity_editor_path: Option<&Path>,
    execution_object_root: &Path,
    execution_object_owner_id: &str,
) -> AdmResult<Arc<dyn WorkUnitExecutor>> {
    let canonical_root = canonical_project_root(project_root)?;
    let execution_objects = ExecutionObjectBinding::new(
        &canonical_root,
        execution_object_root,
        execution_object_owner_id,
    )?;
    let development = development_work_unit_executor_from_config(
        config,
        &canonical_root,
        unity_editor_path,
        execution_objects.clone(),
    )?;
    let generator = style_image_generator_from_config(config, &canonical_root)?;
    let art_scope = sha256_hex(
        format!(
            "ai-art-work-unit-v3-eo|{}|{}|{}|{}",
            canonical_root.to_string_lossy(),
            generator.execution_scope_fingerprint(),
            execution_objects.root.to_string_lossy(),
            execution_objects.owner_id,
        )
        .as_bytes(),
    );
    let art: Arc<dyn WorkUnitExecutor> = Arc::new(AiArtWorkUnitExecutor {
        generator,
        project_root: canonical_root.clone(),
        execution_objects,
        execution_scope: art_scope,
    });
    let execution_scope = sha256_hex(
        format!(
            "routed-work-unit-v1|{}|{}",
            development.execution_scope_fingerprint(),
            art.execution_scope_fingerprint(),
        )
        .as_bytes(),
    );
    Ok(Arc::new(RoutedWorkUnitExecutor {
        development,
        art,
        execution_scope,
    }))
}

pub fn workspace_task_agent_from_config(
    config: &AiConfig,
    project_root: &Path,
    unity_editor_path: Option<&Path>,
    execution_object_root: &Path,
    execution_object_owner_id: &str,
) -> AdmResult<Arc<dyn WorkspaceTaskAgent>> {
    let canonical_root = canonical_project_root(project_root)?;
    let execution_objects = ExecutionObjectBinding::new(
        &canonical_root,
        execution_object_root,
        execution_object_owner_id,
    )?;
    Ok(Arc::new(development_workspace_task_agent_from_config(
        config,
        &canonical_root,
        unity_editor_path,
        execution_objects,
    )?))
}

fn development_work_unit_executor_from_config(
    config: &AiConfig,
    project_root: &Path,
    unity_editor_path: Option<&Path>,
    execution_objects: ExecutionObjectBinding,
) -> AdmResult<Arc<dyn WorkUnitExecutor>> {
    let target = resolve_active_ai_target(config, AiConfigCategory::Dev)?;
    if !target.is_available() {
        return Ok(unavailable(
            "active development configuration is unavailable",
        ));
    }
    if target.descriptor().source != AiConfigSource::Cli {
        return Ok(unavailable(
            "development work units currently require a local Codex or Claude CLI",
        ));
    }
    let kind = match target.descriptor().adapter {
        AiAdapterKind::Codex => CliKind::Codex,
        AiAdapterKind::Claude => CliKind::Claude,
        _ => {
            return Ok(unavailable(
                "the active development adapter cannot execute file work units",
            ));
        }
    };
    let program = target
        .program()
        .ok_or_else(|| AdmError::new("resolved development CLI program is missing"))?;
    let canonical_root = canonical_project_root(project_root)?;
    let program_verifier: Arc<dyn ProgramWorkUnitVerifier> = match unity_editor_path {
        Some(path) => match UnityBatchmodeProgramVerifier::new(path, &execution_objects.root) {
            Ok(verifier) => Arc::new(verifier),
            Err(_) => Arc::new(UnavailableProgramVerifier {
                message: "the configured Unity editor cannot run batchmode verification"
                    .to_string(),
            }),
        },
        None => Arc::new(UnavailableProgramVerifier {
            message: "the current save has no bound Unity editor for batchmode verification"
                .to_string(),
        }),
    };
    let execution_scope = sha256_hex(
        format!(
            "ai-development-work-unit-v5-isolated-eo-unity|{:?}|{}|{}|{}|{}|{}",
            kind,
            canonical_root.to_string_lossy(),
            program,
            execution_objects.root.to_string_lossy(),
            execution_objects.owner_id,
            program_verifier.execution_scope_fingerprint(),
        )
        .as_bytes(),
    );
    Ok(Arc::new(AiDevelopmentWorkUnitExecutor {
        kind,
        program: program.to_string(),
        project_root: canonical_root,
        execution_objects,
        program_verifier,
        execution_scope,
    }))
}

fn development_workspace_task_agent_from_config(
    config: &AiConfig,
    project_root: &Path,
    unity_editor_path: Option<&Path>,
    execution_objects: ExecutionObjectBinding,
) -> AdmResult<AiDevelopmentWorkUnitExecutor> {
    let target = resolve_active_ai_target(config, AiConfigCategory::Dev)?;
    if !target.is_available() {
        return Err(AdmError::new(
            "active development configuration is unavailable",
        ));
    }
    if target.descriptor().source != AiConfigSource::Cli {
        return Err(AdmError::new(
            "workspace task agents require a local Codex or Claude CLI",
        ));
    }
    let kind = match target.descriptor().adapter {
        AiAdapterKind::Codex => CliKind::Codex,
        AiAdapterKind::Claude => CliKind::Claude,
        _ => {
            return Err(AdmError::new(
                "the active development adapter cannot execute workspace task agents",
            ));
        }
    };
    let program = target
        .program()
        .ok_or_else(|| AdmError::new("resolved development CLI program is missing"))?;
    let canonical_root = canonical_project_root(project_root)?;
    let program_verifier: Arc<dyn ProgramWorkUnitVerifier> = match unity_editor_path {
        Some(path) => match UnityBatchmodeProgramVerifier::new(path, &execution_objects.root) {
            Ok(verifier) => Arc::new(verifier),
            Err(_) => Arc::new(UnavailableProgramVerifier {
                message: "the configured Unity editor cannot run batchmode verification"
                    .to_string(),
            }),
        },
        None => Arc::new(UnavailableProgramVerifier {
            message: "the current save has no bound Unity editor for batchmode verification"
                .to_string(),
        }),
    };
    let execution_scope = sha256_hex(
        format!(
            "workspace-task-agent-v1-ai-development|{:?}|{}|{}|{}|{}|{}",
            kind,
            canonical_root.to_string_lossy(),
            program,
            execution_objects.root.to_string_lossy(),
            execution_objects.owner_id,
            program_verifier.execution_scope_fingerprint(),
        )
        .as_bytes(),
    );
    Ok(AiDevelopmentWorkUnitExecutor {
        kind,
        program: program.to_string(),
        project_root: canonical_root,
        execution_objects,
        program_verifier,
        execution_scope,
    })
}

fn reconcile_declared_outputs(
    root: &Path,
    request: &WorkUnitRequest,
    record: &WorkUnitJournalRecord,
    single_path_key: &str,
) -> AdmResult<WorkUnitReconcileDecision> {
    let mut output_files = record
        .result
        .as_ref()
        .map(|result| result.output_refs.clone())
        .filter(|items| !items.is_empty())
        .unwrap_or_else(|| string_array(request.payload.get("output_files")));
    if output_files.is_empty()
        && let Some(path) = request
            .payload
            .get(single_path_key)
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
    {
        output_files.push(path.to_string());
    }
    if output_files.is_empty() {
        return Ok(WorkUnitReconcileDecision::Unknown);
    }
    let current_hashes = output_hashes(root, &output_files)?;
    if current_hashes.is_empty() {
        return Ok(WorkUnitReconcileDecision::SafeToRetry);
    }
    if current_hashes.len() != output_files.len() {
        return Ok(WorkUnitReconcileDecision::Unknown);
    }
    let recorded_hashes = record
        .result
        .as_ref()
        .filter(|result| result.status == WorkUnitExecutionStatus::Verified)
        .and_then(|result| result.data.get("output_hashes"))
        .and_then(Value::as_object)
        .map(|items| {
            items
                .iter()
                .filter_map(|(path, hash)| {
                    hash.as_str().map(|hash| (path.clone(), hash.to_string()))
                })
                .collect::<BTreeMap<_, _>>()
        });
    Ok(match recorded_hashes {
        Some(recorded) if recorded == current_hashes => WorkUnitReconcileDecision::Verified,
        _ => WorkUnitReconcileDecision::Unknown,
    })
}

fn reconcile_execution_object_and_outputs(
    root: &Path,
    binding: &ExecutionObjectBinding,
    request: &WorkUnitRequest,
    record: &WorkUnitJournalRecord,
    single_path_key: &str,
) -> AdmResult<WorkUnitReconcileDecision> {
    if record
        .result
        .as_ref()
        .is_some_and(|result| result.status == WorkUnitExecutionStatus::Verified)
    {
        if verified_execution_object_id(binding, request, record)?.is_none() {
            return Ok(WorkUnitReconcileDecision::Unknown);
        }
        return reconcile_declared_outputs(root, request, record, single_path_key);
    }
    let outputs = reconcile_declared_outputs(root, request, record, single_path_key)?;
    if request_has_unresolved_execution_object(binding, request)? {
        return Ok(WorkUnitReconcileDecision::Unknown);
    }
    Ok(outputs)
}

fn parse_dimensions(value: &str) -> (u32, u32) {
    let parsed = value
        .to_ascii_lowercase()
        .split_once('x')
        .and_then(|(width, height)| {
            Some((width.trim().parse().ok()?, height.trim().parse().ok()?))
        });
    parsed
        .map(|(width, height): (u32, u32)| (width.clamp(64, 2048), height.clamp(64, 2048)))
        .unwrap_or((512, 512))
}

fn canonical_project_root(root: &Path) -> AdmResult<PathBuf> {
    let canonical = fs::canonicalize(root)
        .map_err(|_| AdmError::new("configured development project path is unavailable"))?;
    if !canonical.is_dir() {
        return Err(AdmError::new(
            "configured development project path is not a directory",
        ));
    }
    Ok(canonical)
}

impl ExecutionObjectBinding {
    fn new(project_root: &Path, root: &Path, owner_id: &str) -> AdmResult<Self> {
        let owner_id = owner_id.trim();
        if owner_id.is_empty() {
            return Err(AdmError::new("execution object owner id must not be empty"));
        }
        fs::create_dir_all(root)
            .map_err(|_| AdmError::new("execution object draft root could not be created"))?;
        let root = fs::canonicalize(root)
            .map_err(|_| AdmError::new("execution object draft root is unavailable"))?;
        if !root.is_dir() {
            return Err(AdmError::new(
                "execution object draft root is not a directory",
            ));
        }
        let project_root = canonical_project_root(project_root)?;
        if root == project_root
            || root.starts_with(&project_root)
            || project_root.starts_with(&root)
        {
            return Err(AdmError::new(
                "Unity work root and execution object draft root must be separate directory trees",
            ));
        }
        let binding = Self {
            root,
            owner_id: owner_id.to_string(),
        };
        binding.open_store()?;
        Ok(binding)
    }

    fn open_store(&self) -> AdmResult<ExecutionObjectStoreService> {
        let store = ExecutionObjectStoreService::new(
            execution_object_store_path(&self.root),
            Some(self.owner_id.clone()),
        )?;
        if let Some(existing) = store.document().save_id.as_deref()
            && existing != self.owner_id
        {
            return Err(AdmError::new(format!(
                "execution object store owner {existing:?} does not match requested owner {:?}; ownership transfer must be explicit",
                self.owner_id
            )));
        }
        Ok(store)
    }
}

fn execution_object_task(request: &WorkUnitRequest) -> Value {
    let mut task = request.payload.clone();
    if let Some(object) = task.as_object_mut() {
        object.insert(
            "work_unit_idempotency_key".to_string(),
            Value::String(request.idempotency_key.clone()),
        );
        object.insert(
            "work_unit_unit_id".to_string(),
            Value::String(request.unit_id.clone()),
        );
    }
    task
}

fn program_execution_object_output_files(output_files: &[String]) -> Vec<String> {
    let mut files = output_files.iter().cloned().collect::<BTreeSet<_>>();
    for path in output_files {
        let normalized = path.replace('\\', "/");
        if normalized
            .get(..7)
            .is_some_and(|prefix| prefix.eq_ignore_ascii_case("Assets/"))
            && !normalized.to_ascii_lowercase().ends_with(".meta")
        {
            files.insert(format!("{normalized}.meta"));
        }
    }
    files.into_iter().collect()
}

fn program_execution_object_task(
    request: &WorkUnitRequest,
    execution_object_output_files: &[String],
) -> Value {
    let mut task = execution_object_task(request);
    if let Some(object) = task.as_object_mut() {
        object.insert(
            "output_files".to_string(),
            json!(execution_object_output_files),
        );
        object.insert(
            "declared_work_unit_output_files".to_string(),
            request
                .payload
                .get("output_files")
                .cloned()
                .unwrap_or_else(|| json!([])),
        );
    }
    task
}

fn execution_object_matches_request(
    object: &adm_new_contracts::execution_object::ExecutionObject,
    request: &WorkUnitRequest,
) -> bool {
    let content = object
        .final_submitted_content
        .as_ref()
        .unwrap_or(&object.prefilled_content);
    content
        .get("task")
        .and_then(|task| task.get("work_unit_idempotency_key"))
        .and_then(Value::as_str)
        == Some(request.idempotency_key.as_str())
}

fn verified_execution_object_id(
    binding: &ExecutionObjectBinding,
    request: &WorkUnitRequest,
    record: &WorkUnitJournalRecord,
) -> AdmResult<Option<String>> {
    let Some(result) = record.result.as_ref() else {
        return Ok(None);
    };
    let Some(execution_object_id) = result
        .data
        .get("execution_object_id")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
    else {
        return Ok(None);
    };
    if result
        .data
        .get("execution_object_state")
        .and_then(Value::as_str)
        != Some("verified")
    {
        return Ok(None);
    }
    let store = binding.open_store()?;
    let object = match store.get(execution_object_id) {
        Ok(object) => object,
        Err(_) => return Ok(None),
    };
    Ok((object.state == ExecutionObjectStatus::Verified
        && execution_object_matches_request(object, request))
    .then(|| execution_object_id.to_string()))
}

fn request_has_unresolved_execution_object(
    binding: &ExecutionObjectBinding,
    request: &WorkUnitRequest,
) -> AdmResult<bool> {
    let store = binding.open_store()?;
    Ok(store.document().objects.iter().any(|object| {
        execution_object_matches_request(object, request)
            && !matches!(
                object.state,
                ExecutionObjectStatus::Verified
                    | ExecutionObjectStatus::Cancelled
                    | ExecutionObjectStatus::Rejected
                    | ExecutionObjectStatus::Superseded
            )
    }))
}

fn request_has_non_discarded_execution_object(
    binding: &ExecutionObjectBinding,
    request: &WorkUnitRequest,
) -> AdmResult<bool> {
    let store = binding.open_store()?;
    Ok(store.document().objects.iter().any(|object| {
        execution_object_matches_request(object, request)
            && !matches!(
                object.state,
                ExecutionObjectStatus::Cancelled
                    | ExecutionObjectStatus::Rejected
                    | ExecutionObjectStatus::Superseded
            )
    }))
}

fn record_or_cancel_program_execution_object(
    binding: &ExecutionObjectBinding,
    execution_object_id: &str,
    changed_files: &[String],
    failure_stage: &str,
    error: &str,
    side_effects_committed: bool,
    locale: ArtifactLocale,
) -> AdmResult<adm_new_contracts::execution_object::ExecutionObject> {
    let _store_lock = acquire_project_write_lock(&binding.root)?;
    let mut store = binding.open_store()?;
    if !side_effects_committed {
        return store.force_cancel(
            execution_object_id,
            if locale == ArtifactLocale::ZhCn {
                "工作单元在提交项目副作用前失败"
            } else {
                "work unit failed before a project side effect was committed"
            },
        );
    }
    record_execution_object_failure(
        &mut store,
        execution_object_id,
        ExecutionFailureInput {
            failure_stage: failure_stage.to_string(),
            written_files: changed_files.to_vec(),
            changed_state: vec![if locale == ArtifactLocale::ZhCn {
                "Unity 项目输出已发生变化".to_string()
            } else {
                "Unity project outputs changed".to_string()
            }],
            unfinished_actions: vec![if locale == ArtifactLocale::ZhCn {
                "完成已验证的执行对象".to_string()
            } else {
                "verified execution object completion".to_string()
            }],
            retryable: false,
            rollback_needed: true,
            remediation_needed: true,
            validation_needed: true,
            error: safe_message(error),
        },
    )
}

fn safe_existing_file(root: &Path, relative: &str) -> AdmResult<PathBuf> {
    let canonical_root = canonical_project_root(root)?;
    let lexical = ensure_relative_path(&canonical_root, relative)?;
    reject_link_components(&canonical_root, &lexical)?;
    let canonical = fs::canonicalize(&lexical)
        .map_err(|_| AdmError::new("declared project file could not be resolved"))?;
    if !canonical.starts_with(&canonical_root) || !canonical.is_file() {
        return Err(AdmError::new(
            "declared project file escapes the configured project root",
        ));
    }
    Ok(canonical)
}

fn safe_write_target(root: &Path, relative: &str) -> AdmResult<PathBuf> {
    let canonical_root = canonical_project_root(root)?;
    let lexical = ensure_relative_path(&canonical_root, relative)?;
    if lexical == canonical_root {
        return Err(AdmError::new(
            "declared write target must name a child path",
        ));
    }
    reject_link_components(&canonical_root, &lexical)?;
    let mut ancestor = lexical.as_path();
    while fs::symlink_metadata(ancestor).is_err() {
        ancestor = ancestor
            .parent()
            .ok_or_else(|| AdmError::new("declared write target has no safe parent"))?;
    }
    let canonical_ancestor = fs::canonicalize(ancestor)
        .map_err(|_| AdmError::new("declared write target parent could not be resolved"))?;
    if !canonical_ancestor.starts_with(&canonical_root) {
        return Err(AdmError::new(
            "declared write target escapes the configured project root",
        ));
    }
    Ok(lexical)
}

fn reject_link_components(root: &Path, target: &Path) -> AdmResult<()> {
    let relative = target
        .strip_prefix(root)
        .map_err(|_| AdmError::new("declared project path escapes its configured root"))?;
    let mut current = root.to_path_buf();
    for component in relative.components() {
        current.push(component.as_os_str());
        match fs::symlink_metadata(&current) {
            Ok(metadata) if is_link_or_reparse_point(&metadata) => {
                return Err(AdmError::new(
                    "declared project path must not traverse a symbolic link or junction",
                ));
            }
            Ok(_) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => break,
            Err(_) => {
                return Err(AdmError::new(
                    "declared project path components could not be verified",
                ));
            }
        }
    }
    Ok(())
}

fn is_link_or_reparse_point(metadata: &fs::Metadata) -> bool {
    if metadata.file_type().is_symlink() {
        return true;
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x0000_0400;
        return metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0;
    }
    #[cfg(not(windows))]
    false
}

fn create_project_directories(root: &Path, directory: &Path) -> AdmResult<()> {
    let canonical_root = canonical_project_root(root)?;
    let relative = directory
        .strip_prefix(&canonical_root)
        .map_err(|_| AdmError::new("declared output parent escapes the project root"))?;
    let mut current = canonical_root.clone();
    for component in relative.components() {
        current.push(component.as_os_str());
        match fs::symlink_metadata(&current) {
            Ok(metadata) => {
                if is_link_or_reparse_point(&metadata) || !metadata.is_dir() {
                    return Err(AdmError::new(
                        "declared output parent contains an unsafe path component",
                    ));
                }
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                match fs::create_dir(&current) {
                    Ok(()) => {}
                    Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
                    Err(_) => {
                        return Err(AdmError::new("declared output parent could not be created"));
                    }
                }
                let metadata = fs::symlink_metadata(&current)
                    .map_err(|_| AdmError::new("declared output parent could not be verified"))?;
                if is_link_or_reparse_point(&metadata) || !metadata.is_dir() {
                    return Err(AdmError::new(
                        "declared output parent contains an unsafe path component",
                    ));
                }
            }
            Err(_) => {
                return Err(AdmError::new(
                    "declared output parent could not be verified",
                ));
            }
        }
    }
    Ok(())
}

fn write_binary_atomic(root: &Path, relative: &str, bytes: &[u8]) -> AdmResult<()> {
    let path = safe_write_target(root, relative)?;
    let parent = path
        .parent()
        .ok_or_else(|| AdmError::new("declared output has no parent directory"))?;
    create_project_directories(root, parent)?;
    let path = safe_write_target(root, relative)?;
    let parent = path
        .parent()
        .ok_or_else(|| AdmError::new("declared output has no parent directory"))?;
    let canonical_root = canonical_project_root(root)?;
    let canonical_parent = fs::canonicalize(parent)
        .map_err(|_| AdmError::new("declared output parent could not be resolved"))?;
    if !canonical_parent.starts_with(&canonical_root) {
        return Err(AdmError::new(
            "declared output parent escapes the configured project root",
        ));
    }
    let file_name = path
        .file_name()
        .ok_or_else(|| AdmError::new("declared output has no file name"))?;
    let path = canonical_parent.join(file_name);
    foundation_write_bytes_atomic(&path, bytes)
}

fn output_hashes(root: &Path, output_files: &[String]) -> AdmResult<BTreeMap<String, String>> {
    const MAX_HASHED_OUTPUT_BYTES: u64 = 32 * 1024 * 1024;
    let mut hashes = BTreeMap::new();
    for relative in output_files {
        let lexical = safe_write_target(root, relative)?;
        if !lexical.exists() {
            continue;
        }
        let path = safe_existing_file(root, relative)?;
        let metadata = fs::metadata(&path)?;
        if metadata.len() > MAX_HASHED_OUTPUT_BYTES {
            return Err(AdmError::new(format!(
                "declared output exceeds verification size limit: {relative}"
            )));
        }
        hashes.insert(
            relative.clone(),
            sha256_hex(&read_bounded_file(
                &path,
                MAX_HASHED_OUTPUT_BYTES,
                "declared output exceeds the verification size limit",
            )?),
        );
    }
    Ok(hashes)
}

fn read_bounded_file(path: &Path, max_bytes: u64, error_message: &str) -> AdmResult<Vec<u8>> {
    use std::io::Read;

    let file = fs::File::open(path).map_err(|_| AdmError::new(error_message))?;
    let metadata = file.metadata().map_err(|_| AdmError::new(error_message))?;
    if !metadata.is_file() || metadata.len() > max_bytes {
        return Err(AdmError::new(error_message));
    }
    let mut bytes = Vec::with_capacity(metadata.len() as usize);
    file.take(max_bytes.saturating_add(1))
        .read_to_end(&mut bytes)
        .map_err(|_| AdmError::new(error_message))?;
    if bytes.len() as u64 > max_bytes {
        return Err(AdmError::new(error_message));
    }
    Ok(bytes)
}

fn write_new_file(path: &Path, bytes: &[u8]) -> AdmResult<()> {
    use std::io::Write;

    let result = (|| -> std::io::Result<()> {
        let mut file = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(path)?;
        file.write_all(bytes)?;
        file.sync_all()
    })();
    if result.is_err() {
        let _ = fs::remove_file(path);
        return Err(AdmError::new("rollback target could not be restored"));
    }
    Ok(())
}

fn input_hashes(root: &Path, input_files: &[String]) -> AdmResult<BTreeMap<String, String>> {
    const MAX_HASHED_INPUT_BYTES: u64 = 32 * 1024 * 1024;
    let mut hashes = BTreeMap::new();
    for relative in input_files {
        let path = safe_existing_file(root, relative)?;
        let metadata = fs::metadata(&path)?;
        if metadata.len() > MAX_HASHED_INPUT_BYTES {
            return Err(AdmError::new(
                "declared input exceeds the verification size limit",
            ));
        }
        hashes.insert(
            relative.clone(),
            sha256_hex(&read_bounded_file(
                &path,
                MAX_HASHED_INPUT_BYTES,
                "declared input exceeds the verification size limit",
            )?),
        );
    }
    Ok(hashes)
}

fn project_snapshot_is_current(
    root: &Path,
    input_files: &[String],
    expected_inputs: &BTreeMap<String, String>,
    output_files: &[String],
    expected_outputs: &BTreeMap<String, String>,
) -> AdmResult<bool> {
    let current_inputs = input_hashes(root, input_files)?;
    let current_outputs = output_hashes(root, output_files)?;
    Ok(&current_inputs == expected_inputs && &current_outputs == expected_outputs)
}

fn json_hash_map(items: &serde_json::Map<String, Value>) -> BTreeMap<String, String> {
    items
        .iter()
        .filter_map(|(path, hash)| hash.as_str().map(|hash| (path.clone(), hash.to_string())))
        .collect()
}

struct IsolatedWorkRoot {
    path: PathBuf,
    identity: StableDirectoryIdentity,
}

impl IsolatedWorkRoot {
    fn new() -> AdmResult<Self> {
        let path = std::env::temp_dir().join(new_stable_id("adm-work-unit")?);
        fs::create_dir(&path)?;
        let path = fs::canonicalize(path)
            .map_err(|_| AdmError::new("isolated work root could not be resolved"))?;
        let metadata = fs::symlink_metadata(&path)
            .map_err(|_| AdmError::new("isolated work root could not be resolved"))?;
        if is_link_or_reparse_point(&metadata) || !metadata.is_dir() {
            return Err(AdmError::new("isolated work root is unsafe"));
        }
        let identity = StableDirectoryIdentity::capture(&path)?;
        Ok(Self { path, identity })
    }

    fn path(&self) -> &Path {
        &self.path
    }

    fn verified_path(&self) -> AdmResult<&Path> {
        let metadata = fs::symlink_metadata(&self.path)
            .map_err(|_| AdmError::new("isolated work root changed during execution"))?;
        if is_link_or_reparse_point(&metadata)
            || !metadata.is_dir()
            || !self.identity.matches_path(&self.path)?
            || fs::canonicalize(&self.path)
                .map_err(|_| AdmError::new("isolated work root changed during execution"))?
                != self.path
        {
            return Err(AdmError::new("isolated work root changed during execution"));
        }
        Ok(&self.path)
    }
}

impl Drop for IsolatedWorkRoot {
    fn drop(&mut self) {
        let safe = self.verified_path().is_ok();
        self.identity.release();
        if safe {
            let _ = fs::remove_dir_all(&self.path);
        }
    }
}

fn copy_project_file(
    project_root: &Path,
    isolated_root: &Path,
    relative: &str,
    required: bool,
) -> AdmResult<()> {
    let lexical = safe_write_target(project_root, relative)?;
    if !lexical.exists() {
        if required {
            return Err(AdmError::new("declared project input is missing"));
        }
        return Ok(());
    }
    let source = safe_existing_file(project_root, relative)?;
    let target = ensure_relative_path(isolated_root, relative)?;
    let parent = target
        .parent()
        .ok_or_else(|| AdmError::new("isolated work file has no parent"))?;
    fs::create_dir_all(parent)?;
    fs::copy(source, target)?;
    Ok(())
}

fn directory_manifest(root: &Path) -> AdmResult<BTreeMap<String, String>> {
    const MAX_FILE_BYTES: u64 = 32 * 1024 * 1024;
    const MAX_TOTAL_BYTES: u64 = 128 * 1024 * 1024;
    const MAX_ENTRIES: usize = 4_096;
    const MAX_DEPTH: usize = 32;
    fn visit(
        root: &Path,
        directory: &Path,
        depth: usize,
        entries: &mut usize,
        total: &mut u64,
        manifest: &mut BTreeMap<String, String>,
    ) -> AdmResult<()> {
        if depth > MAX_DEPTH {
            return Err(AdmError::new(
                "isolated CLI output tree exceeded the depth limit",
            ));
        }
        let resolved = fs::canonicalize(directory)
            .map_err(|_| AdmError::new("isolated work directory could not be resolved"))?;
        if !resolved.starts_with(root) {
            return Err(AdmError::new(
                "isolated CLI created a directory link outside its work root",
            ));
        }
        for entry in fs::read_dir(directory)? {
            let entry = entry?;
            *entries = entries.saturating_add(1);
            if *entries > MAX_ENTRIES {
                return Err(AdmError::new(
                    "isolated CLI output tree exceeded the entry limit",
                ));
            }
            let path = entry.path();
            let metadata = fs::symlink_metadata(&path)?;
            if metadata.file_type().is_symlink() {
                return Err(AdmError::new("isolated CLI created a symbolic link"));
            }
            if metadata.is_dir() {
                visit(root, &path, depth + 1, entries, total, manifest)?;
            } else if metadata.is_file() {
                if metadata.len() > MAX_FILE_BYTES {
                    return Err(AdmError::new(
                        "isolated CLI output exceeded the per-file verification limit",
                    ));
                }
                let bytes = read_bounded_file(
                    &path,
                    MAX_FILE_BYTES,
                    "isolated CLI output exceeded the per-file verification limit",
                )?;
                *total = total.saturating_add(bytes.len() as u64);
                if *total > MAX_TOTAL_BYTES {
                    return Err(AdmError::new(
                        "isolated CLI outputs exceeded the total verification limit",
                    ));
                }
                let relative = path
                    .strip_prefix(root)
                    .map_err(|_| AdmError::new("isolated CLI output escaped its work root"))?
                    .to_string_lossy()
                    .replace('\\', "/");
                manifest.insert(relative, sha256_hex(&bytes));
            }
        }
        Ok(())
    }

    let canonical_root = fs::canonicalize(root)
        .map_err(|_| AdmError::new("isolated work root could not be resolved"))?;
    let mut manifest = BTreeMap::new();
    let mut entries = 0;
    let mut total = 0;
    visit(
        &canonical_root,
        &canonical_root,
        0,
        &mut entries,
        &mut total,
        &mut manifest,
    )?;
    Ok(manifest)
}

fn changed_manifest_paths(
    before: &BTreeMap<String, String>,
    after: &BTreeMap<String, String>,
) -> Vec<String> {
    before
        .keys()
        .chain(after.keys())
        .cloned()
        .collect::<BTreeSet<_>>()
        .into_iter()
        .filter(|path| before.get(path) != after.get(path))
        .collect()
}

fn commit_declared_outputs(
    project_root: &Path,
    isolated: &IsolatedWorkRoot,
    output_files: &[String],
    expected_hashes: &BTreeMap<String, String>,
    staged_hashes: &BTreeMap<String, String>,
) -> AdmResult<()> {
    let mut prepared = Vec::new();
    for relative in output_files {
        let isolated_root = isolated.verified_path()?;
        let source = safe_existing_file(isolated_root, relative)?;
        let captured = source.parent().unwrap_or(isolated_root).join(format!(
            ".{}.captured",
            new_stable_id("work-unit-staged-output")?
        ));
        isolated.verified_path()?;
        fs::rename(&source, &captured)
            .map_err(|_| AdmError::new("declared output could not be sealed for commit"))?;
        let metadata = fs::symlink_metadata(&captured)
            .map_err(|_| AdmError::new("sealed declared output is unavailable"))?;
        if is_link_or_reparse_point(&metadata) {
            return Err(AdmError::new("sealed declared output is an unsafe link"));
        }
        let bytes = read_bounded_file(
            &captured,
            32 * 1024 * 1024,
            "sealed declared output exceeds the verification limit",
        )?;
        if staged_hashes.get(relative) != Some(&sha256_hex(&bytes)) {
            return Err(AdmError::new(
                "declared output changed after isolated verification",
            ));
        }
        let target = safe_write_target(project_root, relative)?;
        let previous = match target.exists() {
            true => Some(read_bounded_file(
                &safe_existing_file(project_root, relative)?,
                32 * 1024 * 1024,
                "declared output exceeds the verification size limit",
            )?),
            false => None,
        };
        let current_hash = previous.as_ref().map(|contents| sha256_hex(contents));
        if current_hash.as_ref() != expected_hashes.get(relative) {
            return Err(AdmError::new(
                "declared output changed before generated files could be committed",
            ));
        }
        prepared.push((relative.clone(), bytes, previous));
    }

    let mut committed = 0;
    for (relative, bytes, previous) in &prepared {
        let current = match safe_write_target(project_root, relative)? {
            path if path.exists() => Some(read_bounded_file(
                &safe_existing_file(project_root, relative)?,
                32 * 1024 * 1024,
                "declared output exceeds the verification size limit",
            )?),
            _ => None,
        };
        if current.as_ref() != previous.as_ref() {
            if rollback_declared_outputs(project_root, &prepared[..committed]).is_err() {
                return Err(AdmError::new(
                    "declared output changed during commit and rollback failed",
                ));
            }
            return Err(AdmError::new(
                "declared output changed during generated file commit",
            ));
        }
        if let Err(error) = write_binary_atomic(project_root, relative, bytes) {
            if rollback_declared_outputs(project_root, &prepared[..committed]).is_err() {
                return Err(AdmError::new(
                    "declared output commit failed and rollback was incomplete",
                ));
            }
            return Err(error);
        }
        committed += 1;
    }
    Ok(())
}

fn rollback_declared_outputs(
    project_root: &Path,
    committed: &[(String, Vec<u8>, Option<Vec<u8>>)],
) -> AdmResult<()> {
    let mut rollback_failed = false;
    for (relative, generated, previous) in committed.iter().rev() {
        let Ok(target) = safe_write_target(project_root, relative) else {
            rollback_failed = true;
            continue;
        };
        if !target.exists() {
            continue;
        }
        let parent = target.parent().unwrap_or(project_root);
        let quarantine = parent.join(format!(
            ".{}.rollback",
            match new_stable_id("work-unit-rollback") {
                Ok(id) => id,
                Err(_) => {
                    rollback_failed = true;
                    continue;
                }
            }
        ));
        if fs::rename(&target, &quarantine).is_err() {
            rollback_failed = true;
            continue;
        }
        let captured = match read_bounded_file(
            &quarantine,
            32 * 1024 * 1024,
            "rollback source could not be verified",
        ) {
            Ok(bytes) => bytes,
            Err(_) => {
                rollback_failed = true;
                continue;
            }
        };
        let restore = if &captured == generated {
            previous.as_deref()
        } else {
            Some(captured.as_slice())
        };
        if let Some(restore) = restore
            && write_new_file(&target, restore).is_err()
        {
            rollback_failed = true;
            continue;
        }
        if fs::remove_file(&quarantine).is_err() {
            rollback_failed = true;
        }
    }
    if rollback_failed {
        Err(AdmError::new("generated output rollback was incomplete"))
    } else {
        Ok(())
    }
}

fn unavailable(message: &str) -> Arc<dyn WorkUnitExecutor> {
    Arc::new(UnavailableWorkUnitExecutor {
        message: message.to_string(),
    })
}

fn work_unit_prompt(request: &WorkUnitRequest) -> String {
    if work_unit_locale(request) == ArtifactLocale::ZhCn {
        format!(
            "仅执行一个开发工作单元。\n单元 ID: {}\n幂等键: {}\n只能写入已声明的输出文件，返回前必须完成验证。\n\n任务 JSON:\n{}",
            request.unit_id, request.idempotency_key, request.payload
        )
    } else {
        format!(
            "Execute exactly one development work unit.\nUnit ID: {}\nIdempotency key: {}\nWrite only declared output files and verify them before returning.\n\nTask JSON:\n{}",
            request.unit_id, request.idempotency_key, request.payload
        )
    }
}

fn work_unit_locale(request: &WorkUnitRequest) -> ArtifactLocale {
    ArtifactLocale::normalize(
        request
            .payload
            .get("artifact_locale")
            .and_then(Value::as_str),
    )
}

fn localized_work_unit_text(request: &WorkUnitRequest, zh_cn: &str, en_us: &str) -> String {
    if work_unit_locale(request) == ArtifactLocale::ZhCn {
        zh_cn.to_string()
    } else {
        en_us.to_string()
    }
}

fn unity_external_process_path(path: &Path) -> PathBuf {
    let value = path.to_string_lossy();
    if let Some(unc) = value.strip_prefix(r"\\?\UNC\") {
        PathBuf::from(format!(r"\\{unc}"))
    } else if let Some(local) = value.strip_prefix(r"\\?\") {
        PathBuf::from(local)
    } else {
        path.to_path_buf()
    }
}

fn unity_validation_plans(request: &WorkUnitRequest) -> AdmResult<Vec<UnityValidationPlan>> {
    let mut plans = Vec::new();
    if let Some(value) = request.payload.get("workspace_contract") {
        let contract: WorkspaceChangeSet =
            serde_json::from_value(value.clone()).map_err(|error| {
                AdmError::new(format!(
                    "workspace task contract is invalid for Unity verification: {error}"
                ))
            })?;
        for command in contract.command_permissions {
            if matches!(
                command.purpose,
                CommandPurpose::Compile | CommandPurpose::Test | CommandPurpose::Smoke
            ) {
                plans.push(UnityValidationPlan {
                    command_id: command.command_id,
                    purpose: command.purpose,
                    argument_template: command.argument_template,
                });
            }
        }
    }
    if !plans
        .iter()
        .any(|plan| plan.purpose == CommandPurpose::Compile)
    {
        plans.insert(
            0,
            UnityValidationPlan {
                command_id: "implicit_compile".to_string(),
                purpose: CommandPurpose::Compile,
                argument_template: Vec::new(),
            },
        );
    }
    Ok(plans)
}

fn resolve_unity_argument(argument: &str, project_path: &Path, log_path: &Path) -> String {
    argument
        .replace("{workspace}", &project_path.to_string_lossy())
        .replace("{project_path}", &project_path.to_string_lossy())
        .replace("{log_file}", &log_path.to_string_lossy())
}

fn allowed_write_paths(payload: &Value, output_files: &[String]) -> Vec<String> {
    let explicit = string_array(
        payload
            .get("allowed_write_paths")
            .or_else(|| payload.get("allowedWritePaths")),
    );
    if !explicit.is_empty() {
        return explicit;
    }
    let mut paths = output_files
        .iter()
        .filter_map(|path| Path::new(path).parent())
        .map(|path| path.to_string_lossy().replace('\\', "/"))
        .filter(|path| !path.is_empty())
        .collect::<Vec<_>>();
    paths.sort();
    paths.dedup();
    paths
}

fn string_array(value: Option<&Value>) -> Vec<String> {
    value
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(Value::as_str)
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_default()
}

fn safe_message(message: &str) -> String {
    let flattened = message.split_whitespace().collect::<Vec<_>>().join(" ");
    let mut safe = flattened.chars().take(240).collect::<String>();
    for token in safe
        .split_whitespace()
        .filter(|token| token.len() >= 12 && token.starts_with("sk-"))
        .map(str::to_string)
        .collect::<Vec<_>>()
    {
        safe = safe.replace(&token, "[REDACTED]");
    }
    safe
}

#[cfg(test)]
mod tests {
    use super::*;
    use adm_new_change_kernel::{
        CommandPermission, CommandPurpose, TrustedTestContract, WorkspaceChangeSet,
        WorkspaceFileExpectation, WorkspaceFilePayload, WorkspaceOperation,
        WorkspaceResourceBudget,
    };
    use adm_new_contracts::ai::{ApiCategory, ApiEntry};
    use image::{DynamicImage, RgbImage};
    use serde_json::json;
    use std::io::Cursor;

    #[test]
    fn unity_process_paths_remove_windows_verbatim_prefixes() {
        let local = format!(r"\\?\{}:\Unity\project", "C");
        let expected_local = format!(r"{}:\Unity\project", "C");
        assert_eq!(
            unity_external_process_path(Path::new(&local)),
            PathBuf::from(expected_local)
        );

        let unc = format!(r"{}?\UNC\server\share\project", r"\\");
        let expected_unc = format!(r"{}server\share\project", r"\\");
        assert_eq!(
            unity_external_process_path(Path::new(&unc)),
            PathBuf::from(expected_unc)
        );
    }

    #[derive(Debug)]
    struct FakeArtGenerator;

    impl StyleImageGenerator for FakeArtGenerator {
        fn execution_scope_fingerprint(&self) -> String {
            "fake-art-v1".to_string()
        }

        fn generate(
            &self,
            _request: &StyleImageRequest,
        ) -> AdmResult<adm_new_pipeline::StyleImageResult> {
            let mut cursor = Cursor::new(Vec::new());
            DynamicImage::ImageRgb8(RgbImage::new(8, 8))
                .write_to(&mut cursor, ImageFormat::Png)
                .unwrap();
            Ok(adm_new_pipeline::StyleImageResult::generated(
                cursor.into_inner(),
                "fake",
                "fake-image",
                8,
                8,
            ))
        }
    }

    #[derive(Debug)]
    struct ConcurrentEditArtGenerator {
        target: PathBuf,
    }

    #[derive(Debug)]
    struct FakeProgramVerifier {
        passed: bool,
    }

    impl ProgramWorkUnitVerifier for FakeProgramVerifier {
        fn execution_scope_fingerprint(&self) -> String {
            format!("fake-program-verifier-{}", self.passed)
        }

        fn verify(
            &self,
            _request: &WorkUnitRequest,
            _project_root: &Path,
            _expected_output_hashes: &BTreeMap<String, String>,
        ) -> AdmResult<ProgramVerificationOutcome> {
            let result = json!({
                "id": "unity_batchmode_compile",
                "status": if self.passed { "passed" } else { "failed" },
                "runner": "fake",
            });
            Ok(if self.passed {
                ProgramVerificationOutcome::passed(result)
            } else {
                ProgramVerificationOutcome::failed("fake Unity failure", result)
            })
        }
    }

    fn test_execution_objects(
        project_root: &Path,
        label: &str,
    ) -> (PathBuf, ExecutionObjectBinding) {
        let root = std::env::temp_dir().join(
            adm_new_foundation::new_stable_id(&format!("{label}-execution-objects")).unwrap(),
        );
        fs::create_dir_all(&root).unwrap();
        let binding = ExecutionObjectBinding::new(project_root, &root, "save-test").unwrap();
        (root, binding)
    }

    impl StyleImageGenerator for ConcurrentEditArtGenerator {
        fn execution_scope_fingerprint(&self) -> String {
            "concurrent-edit-art-v1".to_string()
        }

        fn generate(
            &self,
            request: &StyleImageRequest,
        ) -> AdmResult<adm_new_pipeline::StyleImageResult> {
            fs::write(&self.target, b"player-edited-during-generation").unwrap();
            FakeArtGenerator.generate(request)
        }
    }

    #[test]
    fn execution_object_binding_rejects_overlapping_roots_and_owner_replacement() {
        let project_root = std::env::temp_dir()
            .join(adm_new_foundation::new_stable_id("eo-binding-project").unwrap());
        let store_root = std::env::temp_dir()
            .join(adm_new_foundation::new_stable_id("eo-binding-store").unwrap());
        fs::create_dir_all(&project_root).unwrap();
        fs::create_dir_all(&store_root).unwrap();

        assert!(ExecutionObjectBinding::new(&project_root, &project_root, "save-a").is_err());
        let binding = ExecutionObjectBinding::new(&project_root, &store_root, "save-a").unwrap();
        let mut store = binding.open_store().unwrap();
        store.save().unwrap();
        assert!(ExecutionObjectBinding::new(&project_root, &store_root, "save-b").is_err());

        let _ = fs::remove_dir_all(project_root);
        let _ = fs::remove_dir_all(store_root);
    }

    #[test]
    fn api_dev_target_is_explicitly_unavailable_for_file_side_effect_units() {
        let config = AiConfig {
            dev: ApiCategory {
                category_id: "dev".to_string(),
                active_entry_id: "api".to_string(),
                entries: vec![ApiEntry {
                    id: "api".to_string(),
                    config_type: "openai_dev_api".to_string(),
                    api_url: "https://api.example.test/v1".to_string(),
                    api_key: "sk-secret".to_string(),
                    ..ApiEntry::default()
                }],
            },
            ..AiConfig::default()
        };
        let project_root = std::env::temp_dir()
            .join(adm_new_foundation::new_stable_id("api-work-unit-project").unwrap());
        let execution_object_root = std::env::temp_dir()
            .join(adm_new_foundation::new_stable_id("api-work-unit-execution-objects").unwrap());
        fs::create_dir_all(&project_root).unwrap();
        let executor = work_unit_executor_from_config(
            &config,
            &project_root,
            None,
            &execution_object_root,
            "draft:test-api",
        )
        .unwrap();
        let request =
            WorkUnitRequest::new("11", "task", WorkUnitKind::Development, json!({})).unwrap();
        let result = executor.execute(&request).unwrap();
        assert_eq!(result.status, WorkUnitExecutionStatus::Unavailable);
        assert!(!format!("{executor:?}").contains("sk-secret"));
        let _ = fs::remove_dir_all(project_root);
        let _ = fs::remove_dir_all(execution_object_root);
    }

    #[test]
    fn development_executor_maps_workspace_task_preflight_failure_to_v2_transaction() {
        let root = std::env::temp_dir()
            .join(adm_new_foundation::new_stable_id("workspace-task-agent-preflight").unwrap());
        fs::create_dir_all(root.join("tests")).unwrap();
        fs::write(root.join("tests/trusted_smoke.cs"), b"trusted baseline").unwrap();
        let (execution_object_root, execution_objects) =
            test_execution_objects(&root, "workspace-task-agent-preflight");
        let executor = AiDevelopmentWorkUnitExecutor {
            kind: CliKind::Codex,
            program: "codex".to_string(),
            project_root: root.clone(),
            execution_objects,
            program_verifier: Arc::new(UnavailableProgramVerifier {
                message: "test Unity preflight unavailable".to_string(),
            }),
            execution_scope: "test-workspace-agent".to_string(),
        };
        let contract = workspace_test_contract();
        let task = TrustedDevelopmentTask {
            task_id: "runtime.scaffold".to_string(),
            title: "Runtime scaffold".to_string(),
            ordinal_size: "S".to_string(),
            architecture_system_id: "runtime_bootstrap".to_string(),
            declared_write_paths: vec![
                "assets/autodesign/scripts/runtime/bootstrap.cs".to_string(),
            ],
            machine_checks: Vec::new(),
            dependencies: Vec::new(),
            rollback_boundary: Vec::new(),
            workspace_contract: contract.clone(),
        };

        let result = executor.execute_task(&task, 1, None).unwrap();

        assert_eq!(result.outcome, ChangeOutcome::Rejected);
        assert_eq!(
            result.failure_category,
            Some(ChangeFailureCategory::Compile)
        );
        assert_eq!(result.side_effect_state, SideEffectState::None);
        assert!(result.validate_against(&contract).is_valid());
        assert!(
            !root
                .join("assets/autodesign/scripts/runtime/bootstrap.cs")
                .exists()
        );
        let _ = fs::remove_dir_all(root);
        let _ = fs::remove_dir_all(execution_object_root);
    }

    #[test]
    fn workspace_task_bridge_declares_compile_and_trusted_smoke_unity_plans() {
        let contract = workspace_test_contract();
        let task = TrustedDevelopmentTask {
            task_id: "runtime.scaffold".to_string(),
            title: "Runtime scaffold".to_string(),
            ordinal_size: "S".to_string(),
            architecture_system_id: "runtime_bootstrap".to_string(),
            declared_write_paths: vec![
                "assets/autodesign/scripts/runtime/bootstrap.cs".to_string(),
            ],
            machine_checks: Vec::new(),
            dependencies: Vec::new(),
            rollback_boundary: Vec::new(),
            workspace_contract: contract,
        };
        let request =
            workspace_task_to_work_unit_request(&task, 1, None, "test-scope".to_string()).unwrap();
        let plans = unity_validation_plans(&request).unwrap();

        assert!(
            plans
                .iter()
                .any(|plan| plan.command_id == "compile_workspace"
                    && plan.purpose == CommandPurpose::Compile)
        );
        assert!(plans.iter().any(|plan| plan.command_id == "trusted_test"
            && plan.purpose == CommandPurpose::Test
            && plan.argument_template.contains(&"-runTests".to_string())));
    }

    #[test]
    fn legacy_development_work_units_keep_implicit_compile_only_unity_plan() {
        let request =
            WorkUnitRequest::new("11", "legacy-task", WorkUnitKind::Development, json!({}))
                .unwrap();
        let plans = unity_validation_plans(&request).unwrap();

        assert_eq!(plans.len(), 1);
        assert_eq!(plans[0].command_id, "implicit_compile");
        assert_eq!(plans[0].purpose, CommandPurpose::Compile);
    }

    #[test]
    fn development_executor_boundary_allows_v2_bridge_but_not_direct_v2_commit() {
        assert!(
            AI_DEVELOPMENT_EXECUTOR_RETAINED_CALLERS
                .contains(&"gamespec_v2_workspace_task_agent_bridge")
        );
        assert!(
            AI_DEVELOPMENT_EXECUTOR_PROHIBITED_CALLERS
                .contains(&"gamespec_v2_product_step11_direct_work_unit_commit")
        );
        assert!(AI_DEVELOPMENT_EXECUTOR_V2_REPLACEMENT.contains("WorkspaceChangeSet"));
    }

    #[test]
    fn art_work_unit_routes_verified_png_into_the_declared_project_target() {
        let root =
            std::env::temp_dir().join(adm_new_foundation::new_stable_id("art-work-unit").unwrap());
        fs::create_dir_all(&root).unwrap();
        let (execution_object_root, execution_objects) =
            test_execution_objects(&root, "art-work-unit");
        let executor = AiArtWorkUnitExecutor {
            generator: Arc::new(FakeArtGenerator),
            project_root: root.clone(),
            execution_objects,
            execution_scope: "test-art-scope".to_string(),
        };
        let request = WorkUnitRequest::new(
            "12",
            "ART-001",
            WorkUnitKind::Art,
            json!({
                "asset_id": "ASSET-001",
                "unity_target_path": "Assets/AutoDesign/Art/Source/ASSET-001.png",
                "dimensions": "8x8",
                "generation_prompt": "test asset"
            }),
        )
        .unwrap();
        let result = executor.execute(&request).unwrap();
        assert_eq!(result.status, WorkUnitExecutionStatus::Verified);
        assert_eq!(result.data["execution_object_state"], "verified");
        let store = ExecutionObjectStoreService::new(
            execution_object_store_path(&execution_object_root),
            Some("save-test".to_string()),
        )
        .unwrap();
        let execution_object = store
            .get(result.data["execution_object_id"].as_str().unwrap())
            .unwrap();
        assert_eq!(execution_object.state, ExecutionObjectStatus::Verified);
        assert_eq!(store.document().save_id.as_deref(), Some("save-test"));
        let target = root.join("Assets/AutoDesign/Art/Source/ASSET-001.png");
        assert!(target.is_file());
        assert_eq!(
            image::load_from_memory(&fs::read(target).unwrap())
                .unwrap()
                .dimensions(),
            (8, 8)
        );
        let _ = fs::remove_dir_all(root);
        let _ = fs::remove_dir_all(execution_object_root);
    }

    fn workspace_test_contract() -> WorkspaceChangeSet {
        let write_path =
            WorkspaceRelativePath::parse("assets/autodesign/scripts/runtime/bootstrap.cs").unwrap();
        let trusted_test = WorkspaceRelativePath::parse("tests/trusted_smoke.cs").unwrap();
        WorkspaceChangeSet {
            schema_version: WORKSPACE_CHANGE_SET_SCHEMA_VERSION.to_string(),
            change_set_id: "workspace_agent_bridge_test".to_string(),
            base_tree_hash: sha256_hex(b"workspace-agent-bridge-base"),
            read_paths: BTreeSet::from([trusted_test.clone()]),
            agent_write_paths: BTreeSet::from([write_path.clone()]),
            trusted_tool_write_paths: BTreeSet::new(),
            build_output_paths: BTreeSet::from([WorkspaceRelativePath::parse("build").unwrap()]),
            operations: vec![WorkspaceOperation::WriteFile {
                path: write_path,
                expected: WorkspaceFileExpectation::Missing,
                payload: WorkspaceFilePayload::utf8("// generated by v2 workspace agent\n"),
            }],
            command_permissions: vec![
                CommandPermission {
                    command_id: "compile_workspace".to_string(),
                    tool_binding_id: "unity_batchmode".to_string(),
                    purpose: CommandPurpose::Compile,
                    argument_template: vec!["-runEditorCompilation".to_string()],
                    working_directory: None,
                    timeout_ms: 10_000,
                    allow_network: false,
                },
                CommandPermission {
                    command_id: "trusted_test".to_string(),
                    tool_binding_id: "unity_batchmode".to_string(),
                    purpose: CommandPurpose::Test,
                    argument_template: vec!["-runTests".to_string()],
                    working_directory: None,
                    timeout_ms: 10_000,
                    allow_network: false,
                },
            ],
            trusted_tests: vec![TrustedTestContract {
                test_id: "trusted_runtime_smoke".to_string(),
                path: trusted_test,
                baseline_sha256: sha256_hex(b"trusted baseline"),
                command_id: "trusted_test".to_string(),
            }],
            resource_budget: WorkspaceResourceBudget {
                max_duration_ms: 30_000,
                max_processes: 2,
                max_write_bytes: 512_000,
                max_file_count: 8,
                max_retries: 2,
            },
            evidence: vec![ChangeEvidence::from_bytes(
                "workspace_agent_bridge_contract",
                "test",
                EvidenceStatus::Observed,
                b"test contract",
            )],
        }
    }

    #[test]
    fn art_work_unit_does_not_overwrite_a_concurrent_project_edit() {
        let root = std::env::temp_dir().join(adm_new_foundation::new_stable_id("art-cas").unwrap());
        let target = root.join("Assets/AutoDesign/Art/Source/ASSET-001.png");
        fs::create_dir_all(target.parent().unwrap()).unwrap();
        fs::write(&target, b"original").unwrap();
        let (execution_object_root, execution_objects) = test_execution_objects(&root, "art-cas");
        let executor = AiArtWorkUnitExecutor {
            generator: Arc::new(ConcurrentEditArtGenerator {
                target: target.clone(),
            }),
            project_root: root.clone(),
            execution_objects,
            execution_scope: "test-art-cas".to_string(),
        };
        let request = WorkUnitRequest::new(
            "12",
            "ART-CAS",
            WorkUnitKind::Art,
            json!({
                "asset_id": "ASSET-001",
                "unity_target_path": "Assets/AutoDesign/Art/Source/ASSET-001.png",
                "dimensions": "8x8",
                "generation_prompt": "test asset"
            }),
        )
        .unwrap();

        let result = executor.execute(&request).unwrap();
        assert_eq!(result.status, WorkUnitExecutionStatus::Failed);
        assert_eq!(
            fs::read(&target).unwrap(),
            b"player-edited-during-generation"
        );
        assert_eq!(result.data["side_effects_committed"], false);
        assert!(!execution_object_store_path(&execution_object_root).exists());
        let _ = fs::remove_dir_all(root);
        let _ = fs::remove_dir_all(execution_object_root);
    }

    #[test]
    fn missing_unity_editor_records_execution_object_failure_without_claiming_verified() {
        let root = std::env::temp_dir()
            .join(adm_new_foundation::new_stable_id("dev-missing-unity").unwrap());
        fs::create_dir_all(root.join("Assets/Scripts")).unwrap();
        let root = fs::canonicalize(root).unwrap();
        let (execution_object_root, execution_objects) =
            test_execution_objects(&root, "dev-missing-unity");
        let executor = AiDevelopmentWorkUnitExecutor {
            kind: CliKind::Codex,
            program: "must-not-run".to_string(),
            project_root: root.clone(),
            execution_objects,
            program_verifier: Arc::new(UnavailableProgramVerifier {
                message: "Unity editor missing".to_string(),
            }),
            execution_scope: "test-missing-unity".to_string(),
        };
        let request = WorkUnitRequest::new(
            "11",
            "TASK-NO-UNITY",
            WorkUnitKind::Development,
            json!({
                "output_files": ["Assets/Scripts/Generated.cs"],
                "allowed_write_paths": ["Assets/Scripts"]
            }),
        )
        .unwrap();

        let result = executor.execute(&request).unwrap();

        assert_eq!(result.status, WorkUnitExecutionStatus::Failed);
        assert_eq!(result.data["side_effects_committed"], false);
        assert_eq!(result.data["execution_object_state"], "execution_failed");
        assert_ne!(result.data["execution_object_state"], "verified");
        let store = ExecutionObjectStoreService::new(
            execution_object_store_path(&execution_object_root),
            Some("save-test".to_string()),
        )
        .unwrap();
        assert_eq!(
            store
                .get(result.data["execution_object_id"].as_str().unwrap())
                .unwrap()
                .state,
            ExecutionObjectStatus::ExecutionFailed
        );
        let _ = fs::remove_dir_all(root);
        let _ = fs::remove_dir_all(execution_object_root);
    }

    #[test]
    fn unity_verification_failure_keeps_committed_program_eo_unverified() {
        let root = std::env::temp_dir()
            .join(adm_new_foundation::new_stable_id("dev-unity-failure").unwrap());
        fs::create_dir_all(root.join("Assets/Scripts")).unwrap();
        fs::write(
            root.join("Assets/Scripts/Generated.cs"),
            b"class Generated {}",
        )
        .unwrap();
        let root = fs::canonicalize(root).unwrap();
        let (execution_object_root, execution_objects) =
            test_execution_objects(&root, "dev-unity-failure");
        let request = WorkUnitRequest::new(
            "11",
            "TASK-UNITY-FAIL",
            WorkUnitKind::Development,
            json!({
                "output_files": ["Assets/Scripts/Generated.cs"],
                "allowed_write_paths": ["Assets/Scripts"]
            }),
        )
        .unwrap();
        let executing = {
            let mut store = execution_objects.open_store().unwrap();
            begin_program_task_execution_object(
                &mut store,
                execution_object_task(&request),
                &root,
                11,
            )
            .unwrap()
        };

        let failed = record_or_cancel_program_execution_object(
            &execution_objects,
            &executing.execution_object_id,
            &["Assets/Scripts/Generated.cs".to_string()],
            "unity_batchmode_compile",
            "Unity batchmode reported compiler errors",
            true,
            ArtifactLocale::EnUs,
        )
        .unwrap();

        assert_eq!(failed.state, ExecutionObjectStatus::ExecutionFailed);
        assert_ne!(failed.state, ExecutionObjectStatus::Verified);
        assert!(!failed.failure_records.is_empty());
        let _ = fs::remove_dir_all(root);
        let _ = fs::remove_dir_all(execution_object_root);
    }

    #[test]
    fn development_reconciliation_retries_when_declared_input_content_changes() {
        let root =
            std::env::temp_dir().join(adm_new_foundation::new_stable_id("dev-input-hash").unwrap());
        fs::create_dir_all(root.join("Inputs")).unwrap();
        fs::create_dir_all(root.join("Assets/Scripts")).unwrap();
        fs::write(root.join("Inputs/spec.json"), b"version-1").unwrap();
        fs::write(root.join("Assets/Scripts/A.cs"), b"output-1").unwrap();
        let root = fs::canonicalize(root).unwrap();
        let (execution_object_root, execution_objects) =
            test_execution_objects(&root, "dev-input-hash");
        let executor = AiDevelopmentWorkUnitExecutor {
            kind: CliKind::Codex,
            program: "codex".to_string(),
            project_root: root.clone(),
            execution_objects,
            program_verifier: Arc::new(FakeProgramVerifier { passed: true }),
            execution_scope: "test-scope".to_string(),
        };
        let request = WorkUnitRequest::new(
            "11",
            "TASK-INPUT",
            WorkUnitKind::Development,
            json!({
                "input_files": ["Inputs/spec.json"],
                "output_files": ["Assets/Scripts/A.cs"],
                "allowed_write_paths": ["Assets/Scripts"]
            }),
        )
        .unwrap();
        let started_record = WorkUnitJournalRecord {
            schema_version: 1,
            revision: 1,
            stage_id: request.stage_id.clone(),
            task_id: request.task_id.clone(),
            unit_id: request.unit_id.clone(),
            idempotency_key: request.idempotency_key.clone(),
            request_fingerprint: String::new(),
            phase: adm_new_pipeline::WorkUnitJournalPhase::Started,
            result: None,
            result_fingerprint: String::new(),
            failure_message: String::new(),
            updated_at: String::new(),
        };
        assert_eq!(
            executor.reconcile(&request, &started_record).unwrap(),
            WorkUnitReconcileDecision::SafeToRetry
        );
        let output_files = vec!["Assets/Scripts/A.cs".to_string()];
        let current_output_hashes = output_hashes(&root, &output_files).unwrap();
        let unity_verification = executor
            .program_verifier
            .verify(&request, &root, &current_output_hashes)
            .unwrap();
        let execution_object = {
            let mut store = executor.execution_objects.open_store().unwrap();
            let executing = begin_program_task_execution_object(
                &mut store,
                execution_object_task(&request),
                &root,
                11,
            )
            .unwrap();
            verify_program_task_execution_object(
                &mut store,
                &executing.execution_object_id,
                &root,
                &output_files,
                &output_files,
                vec![unity_verification.result],
                json!({"test": true}),
            )
            .unwrap()
        };
        assert_eq!(
            executor.reconcile(&request, &started_record).unwrap(),
            WorkUnitReconcileDecision::Unknown,
            "a crash after EO verification but before journal result persistence must block blind retry"
        );
        let result = WorkUnitExecutionResult::verified(
            vec!["Assets/Scripts/A.cs".to_string()],
            vec!["Assets/Scripts/A.cs".to_string()],
            Vec::new(),
            json!({
                "side_effects_committed": true,
                "input_hashes": input_hashes(&root, &["Inputs/spec.json".to_string()]).unwrap(),
                "output_hashes": output_hashes(&root, &["Assets/Scripts/A.cs".to_string()]).unwrap(),
                "execution_object_id": execution_object.execution_object_id,
                "execution_object_state": execution_object.state.as_str(),
            }),
        );
        let record = WorkUnitJournalRecord {
            schema_version: 1,
            revision: 1,
            stage_id: request.stage_id.clone(),
            task_id: request.task_id.clone(),
            unit_id: request.unit_id.clone(),
            idempotency_key: request.idempotency_key.clone(),
            request_fingerprint: String::new(),
            phase: adm_new_pipeline::WorkUnitJournalPhase::Committed,
            result: Some(result),
            result_fingerprint: String::new(),
            failure_message: String::new(),
            updated_at: String::new(),
        };

        assert_eq!(
            executor.reconcile(&request, &record).unwrap(),
            WorkUnitReconcileDecision::Verified
        );
        fs::write(root.join("Inputs/spec.json"), b"version-2").unwrap();
        assert_eq!(
            executor.reconcile(&request, &record).unwrap(),
            WorkUnitReconcileDecision::SafeToRetry
        );
        let _ = fs::remove_dir_all(root);
        let _ = fs::remove_dir_all(execution_object_root);
    }

    #[test]
    fn development_commit_snapshot_detects_concurrent_project_edits() {
        let root = std::env::temp_dir()
            .join(adm_new_foundation::new_stable_id("dev-project-compare-and-swap").unwrap());
        fs::create_dir_all(root.join("Inputs")).unwrap();
        fs::create_dir_all(root.join("Assets/Scripts")).unwrap();
        fs::write(root.join("Inputs/spec.json"), b"input-v1").unwrap();
        fs::write(root.join("Assets/Scripts/A.cs"), b"output-v1").unwrap();
        let root = fs::canonicalize(root).unwrap();
        let inputs = vec!["Inputs/spec.json".to_string()];
        let outputs = vec!["Assets/Scripts/A.cs".to_string()];
        let expected_inputs = input_hashes(&root, &inputs).unwrap();
        let expected_outputs = output_hashes(&root, &outputs).unwrap();

        assert!(
            project_snapshot_is_current(
                &root,
                &inputs,
                &expected_inputs,
                &outputs,
                &expected_outputs,
            )
            .unwrap()
        );

        fs::write(root.join("Assets/Scripts/A.cs"), b"player-edit").unwrap();
        assert!(
            !project_snapshot_is_current(
                &root,
                &inputs,
                &expected_inputs,
                &outputs,
                &expected_outputs,
            )
            .unwrap()
        );

        fs::write(root.join("Assets/Scripts/A.cs"), b"output-v1").unwrap();
        fs::write(root.join("Inputs/spec.json"), b"unity-edit").unwrap();
        assert!(
            !project_snapshot_is_current(
                &root,
                &inputs,
                &expected_inputs,
                &outputs,
                &expected_outputs,
            )
            .unwrap()
        );
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn development_commit_rejects_staging_changes_after_verification() {
        let root =
            std::env::temp_dir().join(adm_new_foundation::new_stable_id("dev-staged-cas").unwrap());
        fs::create_dir_all(root.join("Assets/Scripts")).unwrap();
        fs::write(root.join("Assets/Scripts/A.cs"), b"project-original").unwrap();
        let root = fs::canonicalize(root).unwrap();
        let isolated = IsolatedWorkRoot::new().unwrap();
        fs::create_dir_all(isolated.path().join("Assets/Scripts")).unwrap();
        fs::write(isolated.path().join("Assets/Scripts/A.cs"), b"verified").unwrap();
        let outputs = vec!["Assets/Scripts/A.cs".to_string()];
        let before = output_hashes(&root, &outputs).unwrap();
        let staged = output_hashes(isolated.path(), &outputs).unwrap();
        fs::write(isolated.path().join("Assets/Scripts/A.cs"), b"tampered").unwrap();

        let error =
            commit_declared_outputs(&root, &isolated, &outputs, &before, &staged).unwrap_err();

        assert!(error.message().contains("after isolated verification"));
        assert_eq!(
            fs::read(root.join("Assets/Scripts/A.cs")).unwrap(),
            b"project-original"
        );
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn isolated_manifest_reports_every_change_outside_declared_outputs() {
        let isolated = IsolatedWorkRoot::new().unwrap();
        fs::create_dir_all(isolated.path().join("Assets/Scripts")).unwrap();
        fs::write(isolated.path().join("Assets/Scripts/A.cs"), b"before").unwrap();
        let before = directory_manifest(isolated.path()).unwrap();
        fs::write(isolated.path().join("Assets/Scripts/A.cs"), b"after").unwrap();
        fs::create_dir_all(isolated.path().join("ProjectSettings")).unwrap();
        fs::write(
            isolated.path().join("ProjectSettings/ProjectVersion.txt"),
            b"rogue",
        )
        .unwrap();
        let changed =
            changed_manifest_paths(&before, &directory_manifest(isolated.path()).unwrap());
        assert_eq!(
            changed,
            vec![
                "Assets/Scripts/A.cs".to_string(),
                "ProjectSettings/ProjectVersion.txt".to_string()
            ]
        );
    }

    #[test]
    fn isolated_manifest_rejects_excessive_directory_depth() {
        let isolated = IsolatedWorkRoot::new().unwrap();
        let mut directory = isolated.path().to_path_buf();
        for index in 0..34 {
            directory.push(format!("d{index}"));
            fs::create_dir(&directory).unwrap();
        }
        fs::write(directory.join("out.txt"), b"deep").unwrap();

        let error = directory_manifest(isolated.path()).unwrap_err();

        assert!(error.message().contains("depth limit"));
    }

    #[cfg(windows)]
    #[test]
    fn write_target_rejects_a_directory_link_that_escapes_the_project() {
        let root =
            std::env::temp_dir().join(adm_new_foundation::new_stable_id("junction-root").unwrap());
        let outside = std::env::temp_dir()
            .join(adm_new_foundation::new_stable_id("junction-outside").unwrap());
        fs::create_dir_all(root.join("Assets")).unwrap();
        fs::create_dir_all(&outside).unwrap();
        let link = root.join("Assets/External");
        if std::os::windows::fs::symlink_dir(&outside, &link).is_ok() {
            let error = safe_write_target(&root, "Assets/External/out.png").unwrap_err();
            assert!(error.message().contains("symbolic link or junction"));
        }
        let _ = fs::remove_dir_all(root);
        let _ = fs::remove_dir_all(outside);
    }

    #[cfg(windows)]
    #[test]
    fn write_target_rejects_a_directory_link_within_the_project() {
        let root = std::env::temp_dir()
            .join(adm_new_foundation::new_stable_id("internal-junction-root").unwrap());
        let real = root.join("Generated");
        fs::create_dir_all(root.join("Assets")).unwrap();
        fs::create_dir_all(&real).unwrap();
        let link = root.join("Assets/Alias");
        if std::os::windows::fs::symlink_dir(&real, &link).is_ok() {
            let error = safe_write_target(&root, "Assets/Alias/out.png").unwrap_err();
            assert!(error.message().contains("symbolic link or junction"));
        }
        let _ = fs::remove_dir_all(root);
    }
}
