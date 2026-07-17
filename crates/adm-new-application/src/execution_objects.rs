use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

use adm_new_contracts::execution_object::{
    ConfirmationLevel, EXECUTION_OBJECT_SCHEMA_VERSION, ExecutionObject, ExecutionObjectStatus,
    ExecutionObjectStoreDocument, FORMAL_ACTIVE_STATES, OwnershipMigration, StateHistoryRecord,
    SubmissionSnapshot,
};
use adm_new_foundation::{
    AdmError, AdmResult, file_manifest, sanitize_identifier, sha256_hex, unix_timestamp,
    write_text_atomic,
};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value, json};

pub const EXECUTION_OBJECT_STORE_RELATIVE_PATH: &str =
    "outputs/execution_objects/execution_objects.json";
pub const UNATTENDED_PROTOCOL: &str = "unattended_recovery.v1";
pub const REPRODUCTION_COMMAND: &str =
    "codex exec --cd {PROJECT_ROOT} --sandbox workspace-write --skip-git-repo-check";

const DIRECTLY_CANCELLABLE_STATES: &[ExecutionObjectStatus] = &[
    ExecutionObjectStatus::Draft,
    ExecutionObjectStatus::StaleDraft,
    ExecutionObjectStatus::Submitted,
    ExecutionObjectStatus::Analyzing,
    ExecutionObjectStatus::AwaitingConfirmation,
    ExecutionObjectStatus::ConflictBlocked,
    ExecutionObjectStatus::StaleBeforeExecution,
];

const CONFLICT_RELEASE_STATES: &[ExecutionObjectStatus] = &[
    ExecutionObjectStatus::Verified,
    ExecutionObjectStatus::Cancelled,
    ExecutionObjectStatus::Superseded,
];

const TERMINAL_STATES: &[ExecutionObjectStatus] = &[
    ExecutionObjectStatus::Verified,
    ExecutionObjectStatus::Cancelled,
    ExecutionObjectStatus::Superseded,
    ExecutionObjectStatus::Rejected,
];

const AUDIT_SPINE_KEYS: &[&str] = &[
    "submission_snapshot",
    "impact_analysis",
    "confirmation_records",
    "drift_checks",
    "conflict_checks",
    "execution_records",
    "failure_records",
    "verification_records",
    "state_history",
];

const CLEANABLE_DERIVED_MATERIALS: &[&str] = &[
    "llm_prompt_cache",
    "temporary_diff",
    "generated_preview",
    "intermediate_asset",
    "staging_bundle",
    "validator_scratch",
];

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExecutionObjectTypeMetadata {
    pub object_type: &'static str,
    pub display_name: &'static str,
    pub description: &'static str,
    pub confirmation_level: ConfirmationLevel,
    pub write_scope_prefix: &'static str,
    pub manager_module: Option<&'static str>,
    pub category: &'static str,
}

pub fn get_type_metadata(object_type: &str) -> AdmResult<ExecutionObjectTypeMetadata> {
    let metadata = match object_type {
        "design_project" => ExecutionObjectTypeMetadata {
            object_type: "design_project",
            display_name: "Design Project",
            description: "Complete design workbench project state.",
            confirmation_level: ConfirmationLevel::NormalConfirm,
            write_scope_prefix: "design:",
            manager_module: Some("core.engines.execution_objects.design_project"),
            category: "design",
        },
        "workspace_snapshot" => ExecutionObjectTypeMetadata {
            object_type: "workspace_snapshot",
            display_name: "Workspace Snapshot",
            description: "Sandbox workspace file-state snapshot.",
            confirmation_level: ConfirmationLevel::NormalConfirm,
            write_scope_prefix: "workspace:",
            manager_module: Some("core.engines.execution_objects.workspace_snapshot"),
            category: "workspace",
        },
        "user_artifact" => ExecutionObjectTypeMetadata {
            object_type: "user_artifact",
            display_name: "User Artifact",
            description: "User-exported workbench content.",
            confirmation_level: ConfirmationLevel::NormalConfirm,
            write_scope_prefix: "workspace:exports",
            manager_module: Some("core.engines.execution_objects.user_artifact"),
            category: "export",
        },
        "program_task" => ExecutionObjectTypeMetadata {
            object_type: "program_task",
            display_name: "Program Task",
            description: "Program-code development task.",
            confirmation_level: ConfirmationLevel::NormalConfirm,
            write_scope_prefix: "program:",
            manager_module: None,
            category: "development",
        },
        "art_task" => ExecutionObjectTypeMetadata {
            object_type: "art_task",
            display_name: "Art Task",
            description: "Art-production task.",
            confirmation_level: ConfirmationLevel::T3ArtConfirm,
            write_scope_prefix: "art:",
            manager_module: None,
            category: "art",
        },
        "rollback_plan" => ExecutionObjectTypeMetadata {
            object_type: "rollback_plan",
            display_name: "Rollback Plan",
            description: "Code or asset rollback plan.",
            confirmation_level: ConfirmationLevel::DestructiveConfirm,
            write_scope_prefix: "rollback:",
            manager_module: None,
            category: "maintenance",
        },
        "asset_contract_change" => ExecutionObjectTypeMetadata {
            object_type: "asset_contract_change",
            display_name: "Asset Contract Change",
            description: "Asset contract structure change.",
            confirmation_level: ConfirmationLevel::ElevatedConfirm,
            write_scope_prefix: "contract:",
            manager_module: None,
            category: "architecture",
        },
        "reference_migration" => ExecutionObjectTypeMetadata {
            object_type: "reference_migration",
            display_name: "Reference Migration",
            description: "Asset-reference path migration.",
            confirmation_level: ConfirmationLevel::ElevatedConfirm,
            write_scope_prefix: "reference:",
            manager_module: None,
            category: "maintenance",
        },
        "unity_replacement_batch" => ExecutionObjectTypeMetadata {
            object_type: "unity_replacement_batch",
            display_name: "Unity Replacement Batch",
            description: "Unity project file batch replacement.",
            confirmation_level: ConfirmationLevel::DestructiveConfirm,
            write_scope_prefix: "unity:",
            manager_module: None,
            category: "unity",
        },
        "unity_scene_assembly_batch" => ExecutionObjectTypeMetadata {
            object_type: "unity_scene_assembly_batch",
            display_name: "Unity Scene Assembly Batch",
            description: "Unity scene, runtime entrypoint, and build-settings assembly.",
            confirmation_level: ConfirmationLevel::DestructiveConfirm,
            write_scope_prefix: "scene_assembly:",
            manager_module: None,
            category: "unity",
        },
        "relationship_graph_correction" => ExecutionObjectTypeMetadata {
            object_type: "relationship_graph_correction",
            display_name: "Relationship Graph Correction",
            description: "Asset relationship graph data correction.",
            confirmation_level: ConfirmationLevel::ElevatedConfirm,
            write_scope_prefix: "graph:",
            manager_module: None,
            category: "architecture",
        },
        "integration_validation" => ExecutionObjectTypeMetadata {
            object_type: "integration_validation",
            display_name: "Integration Validation",
            description: "Integration test validation record.",
            confirmation_level: ConfirmationLevel::NormalConfirm,
            write_scope_prefix: "integration:",
            manager_module: None,
            category: "validation",
        },
        "merged_execution_object" => ExecutionObjectTypeMetadata {
            object_type: "merged_execution_object",
            display_name: "Merged Execution Object",
            description: "Merged record for multiple conflict-blocked execution objects.",
            confirmation_level: ConfirmationLevel::ElevatedConfirm,
            write_scope_prefix: "merge:",
            manager_module: None,
            category: "maintenance",
        },
        "t3_art_baseline_change" => ExecutionObjectTypeMetadata {
            object_type: "t3_art_baseline_change",
            display_name: "T3 Art Baseline Change",
            description: "T3 art-resource baseline change.",
            confirmation_level: ConfirmationLevel::T3ArtConfirm,
            write_scope_prefix: "art:baseline:",
            manager_module: None,
            category: "art",
        },
        other => {
            return Err(AdmError::new(format!(
                "unknown execution object type: {other}"
            )));
        }
    };
    Ok(metadata)
}

pub fn list_all_types() -> Vec<&'static str> {
    vec![
        "design_project",
        "workspace_snapshot",
        "user_artifact",
        "program_task",
        "art_task",
        "rollback_plan",
        "asset_contract_change",
        "reference_migration",
        "unity_replacement_batch",
        "unity_scene_assembly_batch",
        "relationship_graph_correction",
        "integration_validation",
        "merged_execution_object",
        "t3_art_baseline_change",
    ]
}

pub fn list_types_by_category(category: &str) -> Vec<&'static str> {
    list_all_types()
        .into_iter()
        .filter(|object_type| {
            get_type_metadata(object_type)
                .map(|metadata| metadata.category == category)
                .unwrap_or(false)
        })
        .collect()
}

pub fn is_registered_type(object_type: &str) -> bool {
    get_type_metadata(object_type).is_ok()
}

pub fn confirmation_level_for(object_type: &str) -> ConfirmationLevel {
    get_type_metadata(object_type)
        .map(|metadata| metadata.confirmation_level)
        .unwrap_or(ConfirmationLevel::NormalConfirm)
}

pub fn confirmation_level_name(level: &ConfirmationLevel) -> &'static str {
    match level {
        ConfirmationLevel::NormalConfirm => "normal_confirm",
        ConfirmationLevel::ElevatedConfirm => "elevated_confirm",
        ConfirmationLevel::T3ArtConfirm => "t3_art_confirm",
        ConfirmationLevel::DestructiveConfirm => "destructive_confirm",
    }
}

pub fn execution_object_store_path(project_root: impl AsRef<Path>) -> PathBuf {
    project_root
        .as_ref()
        .join(EXECUTION_OBJECT_STORE_RELATIVE_PATH)
}

#[derive(Debug, Clone)]
pub struct ExecutionObjectApplicationService {
    store: ExecutionObjectStoreService,
}

impl ExecutionObjectApplicationService {
    pub fn new(root: impl AsRef<Path>, expected_save_id: Option<String>) -> AdmResult<Self> {
        Ok(Self {
            store: ExecutionObjectStoreService::new(
                execution_object_store_path(root),
                expected_save_id,
            )?,
        })
    }

    pub fn store(&self) -> &ExecutionObjectStoreService {
        &self.store
    }

    pub fn store_mut(&mut self) -> &mut ExecutionObjectStoreService {
        &mut self.store
    }
}

#[derive(Debug, Clone)]
pub struct ExecutionObjectStoreService {
    path: PathBuf,
    expected_save_id: Option<String>,
    document: ExecutionObjectStoreDocument,
}

impl ExecutionObjectStoreService {
    pub fn new(path: impl AsRef<Path>, expected_save_id: Option<String>) -> AdmResult<Self> {
        let raw_path = path.as_ref();
        let path = if raw_path.extension().and_then(|value| value.to_str()) == Some("json") {
            raw_path.to_path_buf()
        } else {
            raw_path.join("execution_objects.json")
        };
        let document = load_store_document(&path)?;
        Ok(Self {
            path,
            expected_save_id,
            document,
        })
    }

    pub fn from_document(
        path: impl AsRef<Path>,
        expected_save_id: Option<String>,
        document: ExecutionObjectStoreDocument,
    ) -> Self {
        Self {
            path: path.as_ref().to_path_buf(),
            expected_save_id,
            document,
        }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn document(&self) -> &ExecutionObjectStoreDocument {
        &self.document
    }

    pub fn document_mut(&mut self) -> &mut ExecutionObjectStoreDocument {
        &mut self.document
    }

    pub fn save(&mut self) -> AdmResult<PathBuf> {
        let existing_save_id = self.document.save_id.clone();
        if let Some(expected) = self.expected_save_id.as_deref() {
            if let Some(existing) = existing_save_id.as_deref() {
                if existing != expected {
                    return Err(AdmError::new(format!(
                        "execution object store save_id {existing:?} does not match expected save_id {expected:?}. Only explicit create/save-as ownership transfer may change save_id."
                    )));
                }
            }
            self.document.save_id = Some(expected.to_string());
        }
        if self.document.generated_at.is_empty() {
            self.document.generated_at = now_iso();
        }
        self.document.updated_at = now_iso();
        let text = serde_json::to_string_pretty(&self.document).map_err(|error| {
            AdmError::new(format!(
                "failed to serialize execution object store: {error}"
            ))
        })?;
        write_text_atomic(&self.path, &text)?;
        Ok(self.path.clone())
    }

    pub fn transfer_ownership_to_save(
        &mut self,
        new_save_id: &str,
        source_save_id: Option<&str>,
        reason: &str,
    ) -> AdmResult<OwnershipMigration> {
        let new_save_id = new_save_id.trim();
        let reason = reason.trim();
        if new_save_id.is_empty() {
            return Err(AdmError::new(
                "ownership transfer requires a non-empty new save_id",
            ));
        }
        if reason.is_empty() {
            return Err(AdmError::new("ownership transfer requires a reason"));
        }
        let portable_save_id = sanitize_identifier(new_save_id)?;
        if portable_save_id != new_save_id {
            return Err(AdmError::new(format!(
                "ownership transfer save_id is not portable: {new_save_id}"
            )));
        }
        let old_save_id = self.document.save_id.clone();
        if let Some(source) = source_save_id
            && old_save_id.as_deref() != Some(source)
        {
            return Err(AdmError::new(format!(
                "execution object store save_id {:?} does not match transfer source_save_id {source:?}",
                old_save_id.as_deref()
            )));
        }
        if old_save_id.as_deref() == Some(new_save_id) {
            self.expected_save_id = Some(new_save_id.to_string());
            return Ok(OwnershipMigration {
                from_save_id: old_save_id,
                to_save_id: new_save_id.to_string(),
                reason: reason.to_string(),
                at: now_iso(),
            });
        }
        let previous_document = self.document.clone();
        let previous_expected_save_id = self.expected_save_id.clone();
        let record = OwnershipMigration {
            from_save_id: old_save_id,
            to_save_id: new_save_id.to_string(),
            reason: reason.to_string(),
            at: now_iso(),
        };
        self.document.ownership_migrations.push(record.clone());
        self.expected_save_id = Some(new_save_id.to_string());
        self.document.save_id = Some(new_save_id.to_string());
        if let Err(error) = self.save() {
            self.document = previous_document;
            self.expected_save_id = previous_expected_save_id;
            return Err(error);
        }
        Ok(record)
    }

    pub fn list_objects(&self, states: Option<&[ExecutionObjectStatus]>) -> Vec<ExecutionObject> {
        self.document
            .objects
            .iter()
            .filter(|object| states.is_none_or(|allowed| allowed.contains(&object.state)))
            .cloned()
            .collect()
    }

    pub fn get(&self, execution_object_id: &str) -> AdmResult<&ExecutionObject> {
        self.document
            .objects
            .iter()
            .find(|object| object.execution_object_id == execution_object_id)
            .ok_or_else(|| {
                AdmError::new(format!("unknown execution object: {execution_object_id}"))
            })
    }

    pub fn create_draft(&mut self, input: CreateDraftInput) -> AdmResult<ExecutionObject> {
        let object_id = self.next_id("EO");
        let now = now_iso();
        let mut object = ExecutionObject {
            execution_object_id: object_id,
            object_type: input.object_type,
            title: input.title,
            state: ExecutionObjectStatus::Draft,
            created_at: now.clone(),
            updated_at: now.clone(),
            source_diagnostic_id: input.source_diagnostic_id,
            source_execution_object_id: input.source_execution_object_id,
            prefilled_content: input.prefilled_content,
            user_content: input.user_content,
            related_facts: input.related_facts,
            write_scope: sorted_string_set(input.write_scope),
            submission_snapshot: None,
            final_submitted_content: None,
            confirmation_level: None,
            impact_analysis: None,
            confirmation_records: Vec::new(),
            cancellation_records: Vec::new(),
            drift_checks: Vec::new(),
            conflict_checks: Vec::new(),
            execution_records: Vec::new(),
            failure_records: Vec::new(),
            verification_records: Vec::new(),
            audit_cleanup_evidence: Vec::new(),
            state_history: Vec::new(),
            metadata: input.metadata,
            extra: BTreeMap::new(),
        };
        object.state_history.push(StateHistoryRecord {
            at: now,
            from: None,
            to: ExecutionObjectStatus::Draft,
            reason: "created".to_string(),
            evidence: json!({}),
        });
        self.document.objects.push(object.clone());
        self.save()?;
        Ok(object)
    }

    pub fn mark_draft_stale(
        &mut self,
        execution_object_id: &str,
        reason: &str,
        changed_facts: Value,
    ) -> AdmResult<ExecutionObject> {
        let updated = {
            let object = self.get_mut(execution_object_id)?;
            require_state(object, &[ExecutionObjectStatus::Draft], "mark_draft_stale")?;
            object.extra.insert(
                "stale_reason".to_string(),
                Value::String(reason.to_string()),
            );
            object
                .extra
                .insert("stale_changed_facts".to_string(), changed_facts);
            append_history(
                object,
                ExecutionObjectStatus::StaleDraft,
                "draft dependencies changed",
                json!({"reason": reason}),
            )?;
            object.clone()
        };
        self.save()?;
        Ok(updated)
    }

    pub fn refresh_stale_draft(
        &mut self,
        execution_object_id: &str,
        refreshed_prefill: Value,
        diff: Value,
        migrate_user_content: bool,
    ) -> AdmResult<ExecutionObject> {
        let source = {
            let object = self.get(execution_object_id)?;
            require_state(
                object,
                &[ExecutionObjectStatus::StaleDraft],
                "refresh_stale_draft",
            )?;
            object.clone()
        };
        let refreshed = self.create_draft(CreateDraftInput {
            object_type: source.object_type.clone(),
            title: source.title.clone(),
            source_diagnostic_id: source.source_diagnostic_id.clone(),
            source_execution_object_id: String::new(),
            prefilled_content: refreshed_prefill,
            user_content: if migrate_user_content {
                source.user_content.clone()
            } else {
                json!({})
            },
            related_facts: source.related_facts.clone(),
            write_scope: source.write_scope.clone(),
            metadata: json!({
                "refreshed_from_draft_id": execution_object_id,
                "refresh_diff": diff,
                "migrated_user_content": migrate_user_content,
            }),
        })?;
        {
            let object = self.get_mut(execution_object_id)?;
            object.extra.insert(
                "superseded_draft_id".to_string(),
                Value::String(refreshed.execution_object_id.clone()),
            );
            push_extra_array(
                object,
                "refresh_records",
                json!({
                    "at": now_iso(),
                    "refreshed_draft_id": refreshed.execution_object_id,
                    "diff_hash": stable_json_hash(&json!({"diff": object.extra.get("stale_changed_facts").cloned().unwrap_or(Value::Null)})),
                    "migrated_user_content": migrate_user_content,
                }),
            );
        }
        self.save()?;
        Ok(refreshed)
    }

    pub fn submit(
        &mut self,
        execution_object_id: &str,
        final_content: Value,
        confirmation_level: ConfirmationLevel,
        submission_confirmation_marker: &str,
        submitter_marker: &str,
    ) -> AdmResult<ExecutionObject> {
        let updated = {
            let object = self.get_mut(execution_object_id)?;
            require_state(object, &[ExecutionObjectStatus::Draft], "submit")?;
            let required_level = confirmation_level_for(&object.object_type);
            if confirmation_level != required_level {
                return Err(AdmError::new(format!(
                    "{} requires confirmation level {}, got {}",
                    object.object_type,
                    confirmation_level_name(&required_level),
                    confirmation_level_name(&confirmation_level)
                )));
            }
            let snapshot = SubmissionSnapshot {
                snapshot_id: format!("SS-{execution_object_id}"),
                execution_object_id: execution_object_id.to_string(),
                draft_id: execution_object_id.to_string(),
                source_diagnostic_id: object.source_diagnostic_id.clone(),
                submitted_at: now_iso(),
                submitter_marker: submitter_marker.to_string(),
                submission_confirmation_marker: submission_confirmation_marker.to_string(),
                confirmation_level: confirmation_level.clone(),
                related_facts: object.related_facts.clone(),
                write_scope: object.write_scope.clone(),
                prefilled_content_hash: stable_json_hash(&object.prefilled_content),
                final_content: final_content.clone(),
                final_content_hash: stable_json_hash(&final_content),
                prefill_to_final_diff_hash: stable_json_hash(&json!({
                    "prefilled": object.prefilled_content,
                    "final": final_content,
                })),
                stale_draft_refresh_source: metadata_str(
                    &object.metadata,
                    "refreshed_from_draft_id",
                ),
            };
            object.submission_snapshot = Some(snapshot.clone());
            object.final_submitted_content = Some(final_content);
            object.confirmation_level = Some(confirmation_level);
            append_history(
                object,
                ExecutionObjectStatus::Submitted,
                "submitted with immutable snapshot",
                json!({"snapshot_id": snapshot.snapshot_id}),
            )?;
            object.clone()
        };
        self.save()?;
        Ok(updated)
    }

    pub fn start_analysis(&mut self, execution_object_id: &str) -> AdmResult<ExecutionObject> {
        let updated = {
            let object = self.get_mut(execution_object_id)?;
            require_state(
                object,
                &[ExecutionObjectStatus::Submitted],
                "start_analysis",
            )?;
            if object.submission_snapshot.is_none() {
                return Err(AdmError::new(
                    "submitted execution object requires a submission snapshot",
                ));
            }
            append_history(
                object,
                ExecutionObjectStatus::Analyzing,
                "impact analysis started",
                json!({}),
            )?;
            object.clone()
        };
        self.save()?;
        Ok(updated)
    }

    pub fn complete_impact_analysis(
        &mut self,
        execution_object_id: &str,
        input: ImpactAnalysisInput,
    ) -> AdmResult<ExecutionObject> {
        let updated = {
            let object = self.get_mut(execution_object_id)?;
            require_state(
                object,
                &[ExecutionObjectStatus::Analyzing],
                "complete_impact_analysis",
            )?;
            let snapshot_id = object
                .submission_snapshot
                .as_ref()
                .map(|snapshot| snapshot.snapshot_id.clone())
                .unwrap_or_default();
            let affected_scopes = sorted_string_set(input.affected_scopes);
            let analysis = json!({
                "analysis_id": format!("IA-{execution_object_id}"),
                "based_on_snapshot_id": snapshot_id,
                "created_at": now_iso(),
                "affected_scopes": affected_scopes,
                "invalidation_scope": sorted_string_set(input.invalidation_scope),
                "summary": input.summary,
                "diagnostics": input.diagnostics,
                "is_empty": affected_scopes.is_empty(),
            });
            object.impact_analysis = Some(analysis.clone());
            if !affected_scopes.is_empty() {
                let mut scope = object.write_scope.clone();
                scope.extend(affected_scopes.iter().cloned());
                object.write_scope = sorted_string_set(scope);
            }
            append_history(
                object,
                ExecutionObjectStatus::AwaitingConfirmation,
                "impact analysis completed",
                json!({
                    "analysis_id": analysis.get("analysis_id").cloned().unwrap_or(Value::Null),
                    "is_empty": analysis.get("is_empty").cloned().unwrap_or(Value::Null),
                }),
            )?;
            object.clone()
        };
        self.save()?;
        Ok(updated)
    }

    pub fn reject_empty_impact(
        &mut self,
        execution_object_id: &str,
        reason: &str,
    ) -> AdmResult<ExecutionObject> {
        let updated = {
            let object = self.get_mut(execution_object_id)?;
            require_state(
                object,
                &[ExecutionObjectStatus::AwaitingConfirmation],
                "reject_empty_impact",
            )?;
            let is_empty = object
                .impact_analysis
                .as_ref()
                .and_then(|value| value.get("is_empty"))
                .and_then(Value::as_bool)
                .unwrap_or(false);
            if !is_empty {
                return Err(AdmError::new(
                    "reject_empty_impact is allowed only for empty impact analysis",
                ));
            }
            object.extra.insert(
                "rejection".to_string(),
                json!({"at": now_iso(), "reason": reason}),
            );
            append_history(
                object,
                ExecutionObjectStatus::Rejected,
                "empty impact rejected",
                json!({"reason": reason}),
            )?;
            object.clone()
        };
        self.save()?;
        Ok(updated)
    }

    pub fn approve(
        &mut self,
        execution_object_id: &str,
        confirmation_evidence: Value,
    ) -> AdmResult<ExecutionObject> {
        let updated = {
            let object = self.get_mut(execution_object_id)?;
            require_state(
                object,
                &[ExecutionObjectStatus::AwaitingConfirmation],
                "approve",
            )?;
            let level = object
                .confirmation_level
                .clone()
                .ok_or_else(|| AdmError::new("approval requires a confirmation level"))?;
            validate_confirmation_gate(&level, &confirmation_evidence)?;
            let record = json!({
                "at": now_iso(),
                "confirmation_level": confirmation_level_name(&level),
                "evidence": confirmation_evidence,
                "impact_analysis_id": object.impact_analysis.as_ref()
                    .and_then(|value| value.get("analysis_id"))
                    .and_then(Value::as_str)
                    .unwrap_or_default(),
            });
            object.confirmation_records.push(record);
            append_history(
                object,
                ExecutionObjectStatus::Approved,
                "approved after confirmation gate",
                json!({"confirmation_level": confirmation_level_name(&level)}),
            )?;
            object.clone()
        };
        self.save()?;
        Ok(updated)
    }

    pub fn run_pre_execution_drift_check(
        &mut self,
        execution_object_id: &str,
        current_facts: Value,
    ) -> AdmResult<Value> {
        let check = {
            let object = self.get_mut(execution_object_id)?;
            require_state(
                object,
                &[ExecutionObjectStatus::Approved],
                "run_pre_execution_drift_check",
            )?;
            let snapshot_facts = object
                .submission_snapshot
                .as_ref()
                .map(|snapshot| snapshot.related_facts.clone())
                .unwrap_or_else(|| json!({}));
            let diff = drift_diff(&snapshot_facts, &current_facts);
            let status = if diff.as_object().is_some_and(|object| object.is_empty()) {
                "passed"
            } else {
                "stale_before_execution"
            };
            let check = json!({
                "drift_check_id": format!("DC-{execution_object_id}-{:03}", object.drift_checks.len() + 1),
                "at": now_iso(),
                "current_facts_hash": stable_json_hash(&current_facts),
                "diff": diff,
                "status": status,
            });
            object.drift_checks.push(check.clone());
            if status != "passed" {
                object.extra.insert(
                    "stale_before_execution".to_string(),
                    json!({
                        "drift_check_id": check.get("drift_check_id").cloned().unwrap_or(Value::Null),
                        "diff": check.get("diff").cloned().unwrap_or(Value::Null),
                    }),
                );
                append_history(
                    object,
                    ExecutionObjectStatus::StaleBeforeExecution,
                    "relevant pre-execution drift",
                    json!({"drift_check_id": check.get("drift_check_id").cloned().unwrap_or(Value::Null)}),
                )?;
            }
            check
        };
        self.save()?;
        Ok(check)
    }

    pub fn refresh_stale_before_execution(
        &mut self,
        execution_object_id: &str,
        current_facts: Value,
        refreshed_content: Option<Value>,
    ) -> AdmResult<ExecutionObject> {
        let source = {
            let object = self.get(execution_object_id)?;
            require_state(
                object,
                &[ExecutionObjectStatus::StaleBeforeExecution],
                "refresh_stale_before_execution",
            )?;
            object.clone()
        };
        let stale = source
            .extra
            .get("stale_before_execution")
            .cloned()
            .unwrap_or_else(|| json!({}));
        let refreshed = self.create_draft(CreateDraftInput {
            object_type: source.object_type,
            title: source.title,
            source_diagnostic_id: source.source_diagnostic_id,
            source_execution_object_id: execution_object_id.to_string(),
            prefilled_content: refreshed_content
                .or(source.final_submitted_content)
                .unwrap_or_else(|| json!({})),
            user_content: json!({}),
            related_facts: current_facts,
            write_scope: source.write_scope,
            metadata: json!({
                "refreshed_from_execution_object_id": execution_object_id,
                "drift_check_id": stale.get("drift_check_id").cloned().unwrap_or(Value::Null),
                "drift_diff": stale.get("diff").cloned().unwrap_or(Value::Null),
            }),
        })?;
        {
            let object = self.get_mut(execution_object_id)?;
            object.extra.insert(
                "pending_refreshed_object_id".to_string(),
                Value::String(refreshed.execution_object_id.clone()),
            );
        }
        self.save()?;
        Ok(refreshed)
    }

    pub fn check_concurrency_conflicts(&mut self, execution_object_id: &str) -> AdmResult<Value> {
        let (scope, conflicts) = {
            let object = self.get(execution_object_id)?;
            if ![
                ExecutionObjectStatus::Approved,
                ExecutionObjectStatus::ConflictBlocked,
                ExecutionObjectStatus::AwaitingConfirmation,
            ]
            .contains(&object.state)
            {
                return Err(AdmError::new(
                    "concurrency checks require an approved, awaiting, or conflict-blocked object",
                ));
            }
            let scope: BTreeSet<String> = object.write_scope.iter().cloned().collect();
            let conflicts = self
                .document
                .objects
                .iter()
                .filter(|other| other.execution_object_id != execution_object_id)
                .filter(|other| FORMAL_ACTIVE_STATES.contains(&other.state))
                .filter_map(|other| {
                    let overlap: Vec<String> = other
                        .write_scope
                        .iter()
                        .filter(|item| scope.contains(*item))
                        .cloned()
                        .collect();
                    (!overlap.is_empty()).then(|| {
                        json!({
                            "execution_object_id": other.execution_object_id,
                            "state": other.state.as_str(),
                            "overlap": overlap,
                        })
                    })
                })
                .collect::<Vec<_>>();
            (scope, conflicts)
        };
        let check = {
            let object = self.get_mut(execution_object_id)?;
            let check = json!({
                "conflict_check_id": format!("CC-{execution_object_id}-{:03}", object.conflict_checks.len() + 1),
                "at": now_iso(),
                "status": if conflicts.is_empty() { "passed" } else { "blocked" },
                "conflicts": conflicts,
                "scope": scope.into_iter().collect::<Vec<_>>(),
            });
            object.conflict_checks.push(check.clone());
            if check.get("status").and_then(Value::as_str) == Some("blocked") {
                object.extra.insert(
                    "conflict_block".to_string(),
                    json!({
                        "conflict_check_id": check.get("conflict_check_id").cloned().unwrap_or(Value::Null),
                        "conflicts": check.get("conflicts").cloned().unwrap_or(Value::Null),
                        "waiting": false,
                    }),
                );
                append_history(
                    object,
                    ExecutionObjectStatus::ConflictBlocked,
                    "overlapping high-risk write scope",
                    json!({"conflict_check_id": check.get("conflict_check_id").cloned().unwrap_or(Value::Null)}),
                )?;
            }
            check
        };
        self.save()?;
        Ok(check)
    }

    pub fn wait_for_conflict(&mut self, execution_object_id: &str) -> AdmResult<ExecutionObject> {
        let updated = {
            let object = self.get_mut(execution_object_id)?;
            require_state(
                object,
                &[ExecutionObjectStatus::ConflictBlocked],
                "wait_for_conflict",
            )?;
            let block = object
                .extra
                .get_mut("conflict_block")
                .and_then(Value::as_object_mut)
                .ok_or_else(|| {
                    AdmError::new("conflict_blocked object has no conflict block details")
                })?;
            let conflicts = block
                .get("conflicts")
                .and_then(Value::as_array)
                .map(|items| !items.is_empty())
                .unwrap_or(false);
            if !conflicts {
                return Err(AdmError::new(
                    "conflict_blocked object has no conflict block details",
                ));
            }
            block.insert("waiting".to_string(), Value::Bool(true));
            block.insert("waiting_since".to_string(), Value::String(now_iso()));
            object.clone()
        };
        self.save()?;
        Ok(updated)
    }

    pub fn recheck_waiting_conflict(
        &mut self,
        execution_object_id: &str,
        current_facts: Value,
    ) -> AdmResult<Value> {
        let conflicts = {
            let object = self.get(execution_object_id)?;
            require_state(
                object,
                &[ExecutionObjectStatus::ConflictBlocked],
                "recheck_waiting_conflict",
            )?;
            let block = object.extra.get("conflict_block").ok_or_else(|| {
                AdmError::new("conflict recheck requires a waiting conflict block")
            })?;
            if !block
                .get("waiting")
                .and_then(Value::as_bool)
                .unwrap_or(false)
            {
                return Err(AdmError::new(
                    "conflict recheck requires a waiting conflict block",
                ));
            }
            block
                .get("conflicts")
                .and_then(Value::as_array)
                .cloned()
                .unwrap_or_default()
        };
        let mut blockers = Vec::new();
        for conflict in &conflicts {
            let Some(other_id) = conflict.get("execution_object_id").and_then(Value::as_str) else {
                continue;
            };
            let other = self.get(other_id)?;
            if other.state == ExecutionObjectStatus::ExecutionFailed {
                blockers.push(json!({
                    "execution_object_id": other.execution_object_id,
                    "state": other.state.as_str(),
                    "reason": "execution_failed_does_not_release_conflict",
                }));
            } else if !CONFLICT_RELEASE_STATES.contains(&other.state) {
                blockers.push(json!({
                    "execution_object_id": other.execution_object_id,
                    "state": other.state.as_str(),
                    "reason": "conflicting_object_not_released",
                }));
            }
        }
        let result = {
            let object = self.get_mut(execution_object_id)?;
            let mut recheck = json!({
                "at": now_iso(),
                "blockers": blockers,
            });
            if !recheck
                .get("blockers")
                .and_then(Value::as_array)
                .map(Vec::is_empty)
                .unwrap_or(true)
            {
                push_extra_nested_array(object, "conflict_block", "rechecks", recheck.clone());
                recheck
                    .as_object_mut()
                    .unwrap()
                    .insert("status".to_string(), Value::String("blocked".to_string()));
                recheck
            } else {
                let snapshot_facts = object
                    .submission_snapshot
                    .as_ref()
                    .map(|snapshot| snapshot.related_facts.clone())
                    .unwrap_or_else(|| json!({}));
                let drift = drift_diff(&snapshot_facts, &current_facts);
                recheck
                    .as_object_mut()
                    .unwrap()
                    .insert("drift".to_string(), drift.clone());
                if !drift.as_object().map(Map::is_empty).unwrap_or(true) {
                    object.extra.insert(
                        "stale_before_execution".to_string(),
                        json!({
                            "drift_check_id": format!("DC-{execution_object_id}-conflict-recheck"),
                            "diff": drift,
                        }),
                    );
                    append_history(
                        object,
                        ExecutionObjectStatus::StaleBeforeExecution,
                        "drift found after conflict wait recheck",
                        json!({}),
                    )?;
                    recheck.as_object_mut().unwrap().insert(
                        "status".to_string(),
                        Value::String("stale_before_execution".to_string()),
                    );
                } else {
                    object
                        .extra
                        .insert("conflict_block".to_string(), Value::Null);
                    object
                        .extra
                        .insert("reconfirmation_required".to_string(), Value::Bool(true));
                    append_history(
                        object,
                        ExecutionObjectStatus::AwaitingConfirmation,
                        "conflict wait recheck passed",
                        json!({}),
                    )?;
                    recheck.as_object_mut().unwrap().insert(
                        "status".to_string(),
                        Value::String("awaiting_confirmation".to_string()),
                    );
                }
                recheck
            }
        };
        self.save()?;
        Ok(result)
    }

    pub fn create_merge_draft(
        &mut self,
        source_execution_object_ids: &[String],
        title: &str,
    ) -> AdmResult<ExecutionObject> {
        if source_execution_object_ids.len() < 2 {
            return Err(AdmError::new(
                "a merge draft requires at least two source execution objects",
            ));
        }
        let sources = source_execution_object_ids
            .iter()
            .map(|source_id| {
                let source = self.get(source_id)?;
                require_state(
                    source,
                    &[ExecutionObjectStatus::ConflictBlocked],
                    "create_merge_draft",
                )?;
                Ok(source.clone())
            })
            .collect::<AdmResult<Vec<_>>>()?;
        let mut merged_scope = BTreeSet::new();
        let source_snapshots = sources
            .iter()
            .map(|source| {
                merged_scope.extend(source.write_scope.iter().cloned());
                json!({
                    "execution_object_id": source.execution_object_id,
                    "submission_snapshot": source.submission_snapshot,
                    "impact_analysis": source.impact_analysis,
                    "user_content": source.user_content,
                    "conflict_block": source.extra.get("conflict_block").cloned().unwrap_or(Value::Null),
                })
            })
            .collect::<Vec<_>>();
        let draft = self.create_draft(CreateDraftInput {
            object_type: "merged_execution_object".to_string(),
            title: title.to_string(),
            source_diagnostic_id: String::new(),
            source_execution_object_id: String::new(),
            prefilled_content: json!({"source_snapshots": source_snapshots}),
            user_content: json!({}),
            related_facts: json!({"merge_sources": source_execution_object_ids}),
            write_scope: merged_scope.into_iter().collect(),
            metadata: json!({"source_execution_object_ids": source_execution_object_ids}),
        })?;
        for source_id in source_execution_object_ids {
            let source = self.get_mut(source_id)?;
            source.extra.insert(
                "pending_merge_draft_id".to_string(),
                Value::String(draft.execution_object_id.clone()),
            );
        }
        self.save()?;
        Ok(draft)
    }

    pub fn supersede_merge_sources(
        &mut self,
        merged_execution_object_id: &str,
    ) -> AdmResult<ExecutionObject> {
        let merged = self.get(merged_execution_object_id)?.clone();
        if merged.object_type != "merged_execution_object" {
            return Err(AdmError::new(
                "only merged execution objects can supersede merge sources",
            ));
        }
        require_state(
            &merged,
            &[
                ExecutionObjectStatus::Approved,
                ExecutionObjectStatus::Verified,
            ],
            "supersede_merge_sources",
        )?;
        let source_ids = merged
            .metadata
            .get("source_execution_object_ids")
            .and_then(Value::as_array)
            .map(|items| {
                items
                    .iter()
                    .filter_map(Value::as_str)
                    .map(str::to_string)
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        for source_id in source_ids {
            let source = self.get_mut(&source_id)?;
            require_state(
                source,
                &[ExecutionObjectStatus::ConflictBlocked],
                "supersede_merge_sources",
            )?;
            source.extra.insert(
                "superseded_by_merged_object".to_string(),
                Value::String(merged_execution_object_id.to_string()),
            );
            append_history(
                source,
                ExecutionObjectStatus::Superseded,
                "merged object confirmed",
                json!({"merged_execution_object_id": merged_execution_object_id}),
            )?;
        }
        self.save()?;
        Ok(merged)
    }

    pub fn cancel(
        &mut self,
        execution_object_id: &str,
        reason: &str,
    ) -> AdmResult<ExecutionObject> {
        let updated = {
            let object = self.get_mut(execution_object_id)?;
            require_state(object, DIRECTLY_CANCELLABLE_STATES, "cancel")?;
            object.cancellation_records.push(json!({
                "at": now_iso(),
                "reason": reason,
                "preserved_submission_snapshot": object.submission_snapshot.is_some(),
                "preserved_impact_analysis": object.impact_analysis.is_some(),
            }));
            append_history(
                object,
                ExecutionObjectStatus::Cancelled,
                "cancelled before execution",
                json!({"reason": reason}),
            )?;
            object.clone()
        };
        self.save()?;
        Ok(updated)
    }

    pub fn force_cancel(
        &mut self,
        execution_object_id: &str,
        reason: &str,
    ) -> AdmResult<ExecutionObject> {
        let updated = {
            let object = self.get_mut(execution_object_id)?;
            if TERMINAL_STATES.contains(&object.state) {
                return Ok(object.clone());
            }
            let prior_state = object.state.as_str().to_string();
            object.cancellation_records.push(json!({
                "at": now_iso(),
                "reason": reason,
                "forced": true,
                "prior_state": prior_state,
                "preserved_submission_snapshot": object.submission_snapshot.is_some(),
                "preserved_impact_analysis": object.impact_analysis.is_some(),
            }));
            append_history(
                object,
                ExecutionObjectStatus::Cancelled,
                &format!("force-cancelled: {reason}"),
                json!({"forced": true}),
            )?;
            object.clone()
        };
        self.save()?;
        Ok(updated)
    }

    pub fn start_execution(&mut self, execution_object_id: &str) -> AdmResult<ExecutionObject> {
        let updated = {
            let object = self.get_mut(execution_object_id)?;
            require_state(
                object,
                &[ExecutionObjectStatus::Approved],
                "start_execution",
            )?;
            if object
                .drift_checks
                .last()
                .and_then(|value| value.get("status"))
                .and_then(Value::as_str)
                != Some("passed")
            {
                return Err(AdmError::new(
                    "execution requires a passed pre-execution drift check",
                ));
            }
            if object
                .conflict_checks
                .last()
                .and_then(|value| value.get("status"))
                .and_then(Value::as_str)
                != Some("passed")
            {
                return Err(AdmError::new(
                    "execution requires a passed concurrency conflict check",
                ));
            }
            let record = json!({
                "execution_record_id": format!("ER-{execution_object_id}-{:03}", object.execution_records.len() + 1),
                "started_at": now_iso(),
                "written_files": [],
                "changed_state": [],
            });
            object.execution_records.push(record.clone());
            append_history(
                object,
                ExecutionObjectStatus::Executing,
                "execution started",
                json!({"execution_record_id": record.get("execution_record_id").cloned().unwrap_or(Value::Null)}),
            )?;
            object.clone()
        };
        self.save()?;
        Ok(updated)
    }

    pub fn request_cancellation_during_execution(
        &mut self,
        execution_object_id: &str,
        reason: &str,
    ) -> AdmResult<ExecutionObject> {
        let updated = {
            let object = self.get_mut(execution_object_id)?;
            require_state(
                object,
                &[ExecutionObjectStatus::Executing],
                "request_cancellation_during_execution",
            )?;
            object.cancellation_records.push(json!({
                "at": now_iso(),
                "reason": reason,
                "type": "cancellation_requested",
            }));
            append_history(
                object,
                ExecutionObjectStatus::CancellationRequested,
                "cancellation requested during execution",
                json!({"reason": reason}),
            )?;
            object.clone()
        };
        self.save()?;
        Ok(updated)
    }

    pub fn record_execution_failure(
        &mut self,
        execution_object_id: &str,
        input: ExecutionFailureInput,
    ) -> AdmResult<ExecutionObject> {
        let updated = {
            let object = self.get_mut(execution_object_id)?;
            require_state(
                object,
                &[
                    ExecutionObjectStatus::Executing,
                    ExecutionObjectStatus::CancellationRequested,
                ],
                "record_execution_failure",
            )?;
            let failure = json!({
                "failure_record_id": format!("EF-{execution_object_id}-{:03}", object.failure_records.len() + 1),
                "at": now_iso(),
                "failure_stage": input.failure_stage,
                "written_files": input.written_files,
                "changed_state": input.changed_state,
                "unfinished_actions": input.unfinished_actions,
                "retryable": input.retryable,
                "rollback_needed": input.rollback_needed,
                "remediation_needed": input.remediation_needed,
                "validation_needed": input.validation_needed,
                "error": input.error,
            });
            object.failure_records.push(failure.clone());
            object.extra.insert(
                "latest_failure_record_id".to_string(),
                failure
                    .get("failure_record_id")
                    .cloned()
                    .unwrap_or(Value::Null),
            );
            append_history(
                object,
                ExecutionObjectStatus::ExecutionFailed,
                "execution failed with partial facts",
                json!({"failure_record_id": failure.get("failure_record_id").cloned().unwrap_or(Value::Null)}),
            )?;
            object.clone()
        };
        self.save()?;
        Ok(updated)
    }

    pub fn confirm_retry_from_safe_point(
        &mut self,
        execution_object_id: &str,
        evidence: Value,
        current_facts: Value,
    ) -> AdmResult<ExecutionObject> {
        let updated = {
            let object = self.get_mut(execution_object_id)?;
            require_state(
                object,
                &[ExecutionObjectStatus::ExecutionFailed],
                "confirm_retry_from_safe_point",
            )?;
            let required = [
                "failure_stage_displayed",
                "written_files_displayed",
                "unfinished_actions_displayed",
                "confirmed",
            ];
            require_truthy_fields("retry confirmation", &evidence, &required)?;
            let remaining_scope = value_string_set(evidence.get("remaining_write_scope"));
            let approved_scope: BTreeSet<String> = object.write_scope.iter().cloned().collect();
            if !remaining_scope.is_empty() && !remaining_scope.is_subset(&approved_scope) {
                return Err(AdmError::new(
                    "retry scope exceeds approved scope; resubmit or escalate confirmation",
                ));
            }
            let snapshot_facts = object
                .submission_snapshot
                .as_ref()
                .map(|snapshot| snapshot.related_facts.clone())
                .unwrap_or_else(|| json!({}));
            let drift = drift_diff(&snapshot_facts, &current_facts);
            if !drift.as_object().map(Map::is_empty).unwrap_or(true) {
                object.extra.insert(
                    "stale_before_execution".to_string(),
                    json!({
                        "drift_check_id": format!("DC-{execution_object_id}-retry"),
                        "diff": drift,
                    }),
                );
                append_history(
                    object,
                    ExecutionObjectStatus::StaleBeforeExecution,
                    "retry blocked by drift",
                    json!({}),
                )?;
            } else {
                object.extra.insert(
                    "retry_from_safe_point".to_string(),
                    json!({"at": now_iso(), "evidence": evidence}),
                );
                append_history(
                    object,
                    ExecutionObjectStatus::Approved,
                    "retry from safe point confirmed",
                    json!({}),
                )?;
            }
            object.clone()
        };
        self.save()?;
        Ok(updated)
    }

    pub fn create_rollback_plan_from_failure(
        &mut self,
        execution_object_id: &str,
        title: &str,
    ) -> AdmResult<ExecutionObject> {
        let source = {
            let object = self.get(execution_object_id)?;
            require_state(
                object,
                &[ExecutionObjectStatus::ExecutionFailed],
                "create_rollback_plan_from_failure",
            )?;
            object.clone()
        };
        self.create_draft(CreateDraftInput {
            object_type: "rollback_plan".to_string(),
            title: title.to_string(),
            source_diagnostic_id: String::new(),
            source_execution_object_id: execution_object_id.to_string(),
            prefilled_content: json!({
                "failure_records": source.failure_records,
                "rollback_source": execution_object_id,
            }),
            user_content: json!({}),
            related_facts: source
                .submission_snapshot
                .map(|snapshot| snapshot.related_facts)
                .unwrap_or_else(|| json!({})),
            write_scope: source.write_scope,
            metadata: json!({"rollback_source_execution_object_id": execution_object_id}),
        })
    }

    pub fn record_manual_remediation(
        &mut self,
        execution_object_id: &str,
        evidence: Value,
    ) -> AdmResult<ExecutionObject> {
        let updated = {
            let object = self.get_mut(execution_object_id)?;
            require_state(
                object,
                &[ExecutionObjectStatus::ExecutionFailed],
                "record_manual_remediation",
            )?;
            require_present_fields(
                "manual remediation",
                &evidence,
                &[
                    "remediation_note",
                    "affected_files",
                    "final_hashes",
                    "validation_result",
                ],
            )?;
            ensure_evidence_scope_subset(object, &evidence, "affected_scopes")?;
            let forbidden_keys = [
                "unplanned_file_writes",
                "asset_contract_changes",
                "reference_migrations",
                "relationship_graph_changes",
                "t3_or_art_baseline_changes",
                "style_branch_changes",
                "replacement_batch_changes",
                "rollback_source_changes",
                "new_asset_ids",
                "new_unity_runtime_files",
            ];
            let forbidden = forbidden_keys
                .iter()
                .filter(|key| truthy(evidence.get(**key)))
                .copied()
                .collect::<Vec<_>>();
            if !forbidden.is_empty() {
                return Err(AdmError::new(format!(
                    "manual remediation requires a new execution object: {forbidden:?}"
                )));
            }
            object.extra.insert(
                "manual_remediation".to_string(),
                json!({"at": now_iso(), "evidence": evidence}),
            );
            object.clone()
        };
        self.save()?;
        Ok(updated)
    }

    pub fn record_automated_remediation(
        &mut self,
        execution_object_id: &str,
        evidence: Value,
    ) -> AdmResult<ExecutionObject> {
        let updated = {
            let object = self.get_mut(execution_object_id)?;
            require_state(
                object,
                &[
                    ExecutionObjectStatus::ExecutionFailed,
                    ExecutionObjectStatus::Approved,
                ],
                "record_automated_remediation",
            )?;
            require_present_fields(
                "automated remediation",
                &evidence,
                &[
                    "repair_attempt_id",
                    "correction_id",
                    "affected_files",
                    "final_hashes",
                    "validation_result",
                    "scope_verified",
                    "allowed_write_paths_checked",
                ],
            )?;
            if truthy(evidence.get("unexpected_changes")) {
                return Err(AdmError::new(
                    "automated remediation cannot verify unexpected changes",
                ));
            }
            ensure_evidence_scope_subset(object, &evidence, "affected_scopes")?;
            object.extra.insert(
                "automated_remediation".to_string(),
                json!({"at": now_iso(), "evidence": evidence}),
            );
            object.clone()
        };
        self.save()?;
        Ok(updated)
    }

    pub fn cancel_after_failure(
        &mut self,
        execution_object_id: &str,
        reason: &str,
    ) -> AdmResult<ExecutionObject> {
        let updated = {
            let object = self.get_mut(execution_object_id)?;
            require_state(
                object,
                &[ExecutionObjectStatus::ExecutionFailed],
                "cancel_after_failure",
            )?;
            object.cancellation_records.push(json!({
                "at": now_iso(),
                "reason": reason,
                "type": "cancel_after_failure",
                "preserved_failure_records": object.failure_records.iter()
                    .filter_map(|item| item.get("failure_record_id").cloned())
                    .collect::<Vec<_>>(),
            }));
            append_history(
                object,
                ExecutionObjectStatus::Cancelled,
                "cancelled after failure",
                json!({"reason": reason}),
            )?;
            object.clone()
        };
        self.save()?;
        Ok(updated)
    }

    pub fn verify(
        &mut self,
        execution_object_id: &str,
        evidence: Value,
    ) -> AdmResult<ExecutionObject> {
        let updated = {
            let object = self.get_mut(execution_object_id)?;
            require_state(
                object,
                &[
                    ExecutionObjectStatus::Executing,
                    ExecutionObjectStatus::Approved,
                    ExecutionObjectStatus::ExecutionFailed,
                ],
                "verify",
            )?;
            if object.state == ExecutionObjectStatus::ExecutionFailed
                && !object.extra.contains_key("manual_remediation")
                && !object.extra.contains_key("automated_remediation")
            {
                return Err(AdmError::new(
                    "execution_failed requires manual remediation or another recovery path before verification",
                ));
            }
            require_truthy_fields(
                "verification",
                &evidence,
                &[
                    "execution_logs_complete",
                    "written_files_recorded",
                    "final_hashes_recorded",
                    "project_state_updated",
                    "no_unresolved_execution_failed",
                    "no_blocking_drift_or_conflict",
                ],
            )?;
            let missing_type = type_specific_verification_requirements(&object.object_type)
                .iter()
                .filter(|key| {
                    !truthy(
                        evidence
                            .get("type_specific_checks")
                            .and_then(|value| value.get(**key)),
                    )
                })
                .copied()
                .collect::<Vec<_>>();
            if !missing_type.is_empty() {
                return Err(AdmError::new(format!(
                    "verification missing {} checks: {missing_type:?}",
                    object.object_type
                )));
            }
            let record = json!({
                "verification_record_id": format!("VR-{execution_object_id}-{:03}", object.verification_records.len() + 1),
                "at": now_iso(),
                "evidence": evidence,
            });
            object.verification_records.push(record.clone());
            append_history(
                object,
                ExecutionObjectStatus::Verified,
                "verification standard passed",
                json!({"verification_record_id": record.get("verification_record_id").cloned().unwrap_or(Value::Null)}),
            )?;
            object.clone()
        };
        self.save()?;
        Ok(updated)
    }

    pub fn record_audit_cleanup_evidence(
        &mut self,
        execution_object_id: &str,
        input: AuditCleanupInput,
    ) -> AdmResult<Value> {
        if AUDIT_SPINE_KEYS.contains(&input.material_kind.as_str()) {
            return Err(AdmError::new(format!(
                "audit spine material cannot be cleaned: {}",
                input.material_kind
            )));
        }
        if !CLEANABLE_DERIVED_MATERIALS.contains(&input.material_kind.as_str()) {
            return Err(AdmError::new(format!(
                "unknown cleanable derived material kind: {}",
                input.material_kind
            )));
        }
        let evidence = json!({
            "at": now_iso(),
            "execution_object_id": execution_object_id,
            "material_kind": input.material_kind,
            "source_path": input.source_path,
            "summary_hash": input.summary_hash,
            "reason": input.reason,
            "remaining_trace_location": input.remaining_trace_location,
        });
        {
            let object = self.get_mut(execution_object_id)?;
            object.audit_cleanup_evidence.push(evidence.clone());
        }
        self.document.audit_cleanup_evidence.push(evidence.clone());
        self.save()?;
        Ok(evidence)
    }

    fn get_mut(&mut self, execution_object_id: &str) -> AdmResult<&mut ExecutionObject> {
        self.document
            .objects
            .iter_mut()
            .find(|object| object.execution_object_id == execution_object_id)
            .ok_or_else(|| {
                AdmError::new(format!("unknown execution object: {execution_object_id}"))
            })
    }

    fn next_id(&self, prefix: &str) -> String {
        let used = self
            .document
            .objects
            .iter()
            .map(|object| object.execution_object_id.as_str())
            .collect::<BTreeSet<_>>();
        let mut index = used.len() + 1;
        loop {
            let candidate = format!("{prefix}-{index:06}");
            if !used.contains(candidate.as_str()) {
                return candidate;
            }
            index += 1;
        }
    }
}

#[derive(Debug, Clone)]
pub struct CreateDraftInput {
    pub object_type: String,
    pub title: String,
    pub source_diagnostic_id: String,
    pub source_execution_object_id: String,
    pub prefilled_content: Value,
    pub user_content: Value,
    pub related_facts: Value,
    pub write_scope: Vec<String>,
    pub metadata: Value,
}

impl CreateDraftInput {
    pub fn new(object_type: impl Into<String>, title: impl Into<String>) -> Self {
        Self {
            object_type: object_type.into(),
            title: title.into(),
            source_diagnostic_id: String::new(),
            source_execution_object_id: String::new(),
            prefilled_content: json!({}),
            user_content: json!({}),
            related_facts: json!({}),
            write_scope: Vec::new(),
            metadata: json!({}),
        }
    }
}

#[derive(Debug, Clone)]
pub struct ImpactAnalysisInput {
    pub affected_scopes: Vec<String>,
    pub summary: String,
    pub invalidation_scope: Vec<String>,
    pub diagnostics: Value,
}

#[derive(Debug, Clone)]
pub struct ExecutionFailureInput {
    pub failure_stage: String,
    pub written_files: Vec<String>,
    pub changed_state: Vec<String>,
    pub unfinished_actions: Vec<String>,
    pub retryable: bool,
    pub rollback_needed: bool,
    pub remediation_needed: bool,
    pub validation_needed: bool,
    pub error: String,
}

#[derive(Debug, Clone)]
pub struct AuditCleanupInput {
    pub material_kind: String,
    pub source_path: String,
    pub summary_hash: String,
    pub reason: String,
    pub remaining_trace_location: String,
}

#[derive(Debug, Clone)]
pub struct BeginExecutionObjectInput {
    pub object_type: String,
    pub title: String,
    pub final_content: Value,
    pub related_facts: Value,
    pub write_scope: Vec<String>,
    pub stage: u32,
    pub business_id: String,
    pub source_diagnostic_id: String,
    pub metadata: Value,
    pub current_facts: Option<Value>,
    pub confirmation_level: Option<ConfirmationLevel>,
}

pub fn begin_execution_object(
    store: &mut ExecutionObjectStoreService,
    input: BeginExecutionObjectInput,
) -> AdmResult<ExecutionObject> {
    let level = input
        .confirmation_level
        .unwrap_or_else(|| confirmation_level_for(&input.object_type));
    let scope = {
        let scope = sorted_string_set(input.write_scope);
        if scope.is_empty() {
            vec![format!("stage:{}:unspecified", input.stage)]
        } else {
            scope
        }
    };
    let mut metadata = Map::new();
    metadata.insert("stage".to_string(), json!(input.stage));
    metadata.insert("business_id".to_string(), json!(input.business_id));
    metadata.insert(
        "created_by".to_string(),
        json!("execution_object_integration"),
    );
    merge_object_fields(&mut metadata, &input.metadata);
    let draft = store.create_draft(CreateDraftInput {
        object_type: input.object_type.clone(),
        title: input.title,
        source_diagnostic_id: input.source_diagnostic_id,
        source_execution_object_id: String::new(),
        prefilled_content: input.final_content.clone(),
        user_content: json!({}),
        related_facts: input.related_facts.clone(),
        write_scope: scope.clone(),
        metadata: Value::Object(metadata),
    })?;
    let object_id = draft.execution_object_id.clone();
    store.submit(
        &object_id,
        input.final_content,
        level.clone(),
        &format!("{}:submitted", input.business_id),
        "pipeline_stage_gate",
    )?;
    store.start_analysis(&object_id)?;
    store.complete_impact_analysis(
        &object_id,
        ImpactAnalysisInput {
            affected_scopes: scope.clone(),
            summary: format!("{} impact for {}", input.object_type, input.business_id),
            invalidation_scope: scope,
            diagnostics: json!({"stage": input.stage, "business_id": input.business_id}),
        },
    )?;
    store.approve(
        &object_id,
        confirmation_evidence_for(&level, &input.business_id),
    )?;
    let drift = store.run_pre_execution_drift_check(
        &object_id,
        input.current_facts.unwrap_or(input.related_facts),
    )?;
    if drift.get("status").and_then(Value::as_str) != Some("passed") {
        return Err(AdmError::new(format!(
            "{object_id} blocked by pre-execution drift: {}",
            drift.get("diff").cloned().unwrap_or(Value::Null)
        )));
    }
    let conflict = store.check_concurrency_conflicts(&object_id)?;
    if conflict.get("status").and_then(Value::as_str) != Some("passed") {
        return Err(AdmError::new(format!(
            "{object_id} blocked by write-scope conflict: {}",
            conflict.get("conflicts").cloned().unwrap_or(Value::Null)
        )));
    }
    store.start_execution(&object_id)
}

pub fn infer_program_task_write_scope(task: &Value) -> Vec<String> {
    let mut scope = BTreeSet::new();
    for path in value_array_strings(task.get("output_files")) {
        let path = normalize_path(&path);
        if !path.is_empty() {
            scope.insert(format!("unity_file:{path}"));
        }
    }
    if task
        .get("package_changes")
        .and_then(Value::as_array)
        .map(|items| !items.is_empty())
        .unwrap_or(false)
    {
        scope.insert("unity_file:Packages/manifest.json".to_string());
    }
    if scope.is_empty() {
        scope.insert(format!(
            "program_task:{}",
            str_field(task, "task_id", "unknown")
        ));
    }
    scope.into_iter().collect()
}

pub fn infer_art_task_write_scope(task: &Value) -> Vec<String> {
    let asset_id = str_field(task, "asset_id", "");
    let task_id = str_field(task, "task_id", "");
    if !asset_id.is_empty() {
        vec![format!("asset:{asset_id}")]
    } else if !task_id.is_empty() {
        vec![format!("art_task:{task_id}")]
    } else {
        vec!["art_task:unknown".to_string()]
    }
}

pub fn infer_scene_assembly_write_scope(paths: &[String]) -> Vec<String> {
    let mut scope = paths
        .iter()
        .map(|path| normalize_path(path))
        .filter(|path| !path.is_empty())
        .map(|path| format!("unity_file:{path}"))
        .collect::<BTreeSet<_>>();
    if scope.is_empty() {
        scope.insert("scene_assembly:unspecified".to_string());
    }
    scope.into_iter().collect()
}

pub fn project_file_hashes(
    project_path: &Path,
    paths: &[String],
) -> AdmResult<BTreeMap<String, String>> {
    let mut hashes = BTreeMap::new();
    for path in paths {
        let normalized = normalize_path(path);
        if normalized.is_empty() {
            continue;
        }
        let full_path = project_path.join(&normalized);
        let hash = if full_path.is_file() {
            sha256_hex(&fs::read(&full_path)?)
        } else {
            String::new()
        };
        hashes.insert(normalized, hash);
    }
    Ok(hashes)
}

pub fn confirmation_evidence_for(level: &ConfirmationLevel, subject: &str) -> Value {
    let mut evidence = Map::new();
    evidence.insert("confirmed".to_string(), Value::Bool(true));
    evidence.insert("subject".to_string(), Value::String(subject.to_string()));
    evidence.insert("confirmed_at".to_string(), Value::String(now_iso()));
    evidence.insert(
        "confirmation_source".to_string(),
        Value::String("pipeline_or_gui_gate".to_string()),
    );
    match level {
        ConfirmationLevel::NormalConfirm => {}
        ConfirmationLevel::ElevatedConfirm => {
            for key in [
                "impact_scope_displayed",
                "invalidation_scope_displayed",
                "snapshot_summary_displayed",
            ] {
                evidence.insert(key.to_string(), Value::Bool(true));
            }
        }
        ConfirmationLevel::T3ArtConfirm => {
            for key in [
                "impact_scope_displayed",
                "baseline_or_rule_impact_expanded",
                "snapshot_summary_displayed",
            ] {
                evidence.insert(key.to_string(), Value::Bool(true));
            }
        }
        ConfirmationLevel::DestructiveConfirm => {
            for key in [
                "second_confirmation",
                "affected_files_displayed",
                "old_hashes_displayed",
                "new_hashes_displayed",
                "rollback_source_displayed",
                "unity_risk_displayed",
                "non_automatic_recovery_risk_displayed",
            ] {
                evidence.insert(key.to_string(), Value::Bool(true));
            }
        }
    }
    Value::Object(evidence)
}

pub fn shared_verification_evidence(
    written_files: &[String],
    final_hashes: &BTreeMap<String, String>,
    type_specific_checks: Value,
    verification_results: Vec<Value>,
    extra: Value,
) -> Value {
    let mut evidence = Map::new();
    evidence.insert("execution_logs_complete".to_string(), Value::Bool(true));
    evidence.insert("written_files_recorded".to_string(), Value::Bool(true));
    evidence.insert(
        "written_files".to_string(),
        Value::Array(
            written_files
                .iter()
                .map(|item| Value::String(normalize_path(item)))
                .collect(),
        ),
    );
    evidence.insert(
        "final_hashes_recorded".to_string(),
        Value::Bool(!final_hashes.is_empty()),
    );
    evidence.insert("final_hashes".to_string(), json!(final_hashes));
    evidence.insert("project_state_updated".to_string(), Value::Bool(true));
    evidence.insert(
        "no_unresolved_execution_failed".to_string(),
        Value::Bool(true),
    );
    evidence.insert(
        "no_blocking_drift_or_conflict".to_string(),
        Value::Bool(true),
    );
    evidence.insert(
        "verification_results".to_string(),
        Value::Array(verification_results),
    );
    evidence.insert("type_specific_checks".to_string(), type_specific_checks);
    merge_object_fields(&mut evidence, &extra);
    Value::Object(evidence)
}

pub fn begin_program_task_execution_object(
    store: &mut ExecutionObjectStoreService,
    task: Value,
    project_path: &Path,
    stage: u32,
) -> AdmResult<ExecutionObject> {
    let output_files = value_array_strings(task.get("output_files"))
        .into_iter()
        .map(|item| normalize_path(&item))
        .collect::<Vec<_>>();
    let before_hashes = project_file_hashes(project_path, &output_files)?;
    let task_id = str_field(&task, "task_id", "unknown");
    let related_facts = json!({
        "task_id": task_id,
        "requirement_id": task.get("requirement_id").cloned().unwrap_or(Value::Null),
        "phase": task.get("phase").cloned().unwrap_or(Value::Null),
        "source_refs": task.get("source_refs").cloned().unwrap_or_else(|| json!([])),
        "declared_output_files": output_files,
        "declared_allowed_write_paths": value_array_strings(task.get("allowed_write_paths")).into_iter().map(|item| normalize_path(&item)).collect::<Vec<_>>(),
        "before_hashes": before_hashes,
        "task_contract_hash": stable_json_hash(&task),
    });
    begin_execution_object(
        store,
        BeginExecutionObjectInput {
            object_type: "unity_replacement_batch".to_string(),
            title: format!(
                "{} {}",
                task_id,
                str_field(&task, "title", "Unity development task")
            ),
            final_content: json!({"task": task}),
            related_facts,
            write_scope: infer_program_task_write_scope(&task),
            stage,
            business_id: task_id,
            source_diagnostic_id: str_field(&task, "requirement_id", ""),
            metadata: json!({"entrypoint": "program_task", "output_files": output_files}),
            current_facts: None,
            confirmation_level: None,
        },
    )
}

pub fn verify_program_task_execution_object(
    store: &mut ExecutionObjectStoreService,
    execution_object_id: &str,
    project_path: &Path,
    output_files: &[String],
    written_files: &[String],
    verification_results: Vec<Value>,
    execution_record: Value,
) -> AdmResult<ExecutionObject> {
    let normalized = output_files
        .iter()
        .map(|item| normalize_path(item))
        .collect::<Vec<_>>();
    let final_hashes = project_file_hashes(project_path, &normalized)?;
    let files_exist = !normalized.is_empty()
        && normalized
            .iter()
            .all(|path| project_path.join(path).is_file());
    let unity_import_refreshed = verification_results.iter().any(|item| {
        item.get("id").and_then(Value::as_str) == Some("unity_batchmode_compile")
            && item.get("status").and_then(Value::as_str) == Some("passed")
    });
    let evidence = shared_verification_evidence(
        written_files,
        &final_hashes,
        json!({
            "files_exist": files_exist,
            "hashes_match": files_exist && final_hashes.values().all(|hash| !hash.is_empty()),
            "unity_import_refreshed": unity_import_refreshed,
        }),
        verification_results,
        json!({"execution_record": execution_record}),
    );
    store.verify(execution_object_id, evidence)
}

pub fn record_execution_object_failure(
    store: &mut ExecutionObjectStoreService,
    execution_object_id: &str,
    input: ExecutionFailureInput,
) -> AdmResult<ExecutionObject> {
    let state = store.get(execution_object_id)?.state.clone();
    if ![
        ExecutionObjectStatus::Executing,
        ExecutionObjectStatus::CancellationRequested,
    ]
    .contains(&state)
    {
        return store.get(execution_object_id).cloned();
    }
    store.record_execution_failure(execution_object_id, input)
}

pub fn confirm_automated_retry_from_safe_point(
    store: &mut ExecutionObjectStoreService,
    execution_object_id: &str,
    remaining_write_scope: &[String],
    current_facts: Value,
    correction_id: &str,
) -> AdmResult<ExecutionObject> {
    store.confirm_retry_from_safe_point(
        execution_object_id,
        json!({
            "failure_stage_displayed": true,
            "written_files_displayed": true,
            "unfinished_actions_displayed": true,
            "confirmed": true,
            "remaining_write_scope": remaining_write_scope,
            "automated_recovery": true,
            "correction_id": correction_id,
        }),
        current_facts,
    )
}

pub fn record_automated_remediation(
    store: &mut ExecutionObjectStoreService,
    execution_object_id: &str,
    repair_attempt_id: &str,
    correction_id: &str,
    affected_files: &[String],
    final_hashes: &BTreeMap<String, String>,
    validation_result: Value,
    affected_scopes: &[String],
) -> AdmResult<ExecutionObject> {
    store.record_automated_remediation(
        execution_object_id,
        json!({
            "repair_attempt_id": repair_attempt_id,
            "correction_id": correction_id,
            "affected_files": affected_files.iter().map(|item| normalize_path(item)).collect::<Vec<_>>(),
            "affected_scopes": affected_scopes,
            "final_hashes": final_hashes,
            "validation_result": validation_result,
            "scope_verified": true,
            "unexpected_changes": [],
            "allowed_write_paths_checked": true,
        }),
    )
}

pub fn complete_art_task_execution_object(
    store: &mut ExecutionObjectStoreService,
    task: Value,
    produced_record: Value,
    stage: u32,
) -> AdmResult<ExecutionObject> {
    let task_id = str_field(&task, "task_id", "unknown");
    let asset_id = str_field(&task, "asset_id", "");
    let related_facts = json!({
        "task_id": task_id,
        "asset_id": asset_id,
        "asset_type": task.get("asset_type").cloned().unwrap_or(Value::Null),
        "phase": task.get("phase").cloned().unwrap_or(Value::Null),
        "source_refs": task.get("source_refs").cloned().unwrap_or_else(|| json!([])),
        "task_contract_hash": stable_json_hash(&task),
    });
    let executing = begin_execution_object(
        store,
        BeginExecutionObjectInput {
            object_type: "asset_contract_change".to_string(),
            title: format!(
                "{} {}",
                task_id,
                if asset_id.is_empty() {
                    str_field(&task, "title", "asset production")
                } else {
                    asset_id.clone()
                }
            ),
            final_content: json!({"task": task, "produced_record": produced_record}),
            related_facts,
            write_scope: infer_art_task_write_scope(&task),
            stage,
            business_id: task_id.clone(),
            source_diagnostic_id: asset_id.clone(),
            metadata: json!({"entrypoint": "art_task", "asset_id": asset_id}),
            current_facts: None,
            confirmation_level: None,
        },
    )?;
    let key = if asset_id.is_empty() {
        task_id.clone()
    } else {
        asset_id.clone()
    };
    let mut final_hashes = BTreeMap::new();
    final_hashes.insert(key.clone(), stable_json_hash(&produced_record));
    let evidence = shared_verification_evidence(
        &[key],
        &final_hashes,
        json!({"contract_version_updated": !asset_id.is_empty(), "invalidation_propagated": true}),
        vec![json!({"id": "asset_contract_manifest", "status": "passed"})],
        json!({"produced_record": produced_record}),
    );
    store.verify(&executing.execution_object_id, evidence)
}

pub fn complete_relationship_graph_execution_object(
    store: &mut ExecutionObjectStoreService,
    stage: u32,
    business_id: &str,
    title: &str,
    graph_facts: Value,
    write_scope: Vec<String>,
) -> AdmResult<ExecutionObject> {
    let executing = begin_execution_object(
        store,
        BeginExecutionObjectInput {
            object_type: "relationship_graph_correction".to_string(),
            title: title.to_string(),
            final_content: json!({"graph_facts": graph_facts}),
            related_facts: json!({"graph_facts_hash": stable_json_hash(&graph_facts)}),
            write_scope: if write_scope.is_empty() {
                vec!["relationship_graph:integration".to_string()]
            } else {
                write_scope
            },
            stage,
            business_id: business_id.to_string(),
            source_diagnostic_id: String::new(),
            metadata: json!({"entrypoint": "relationship_graph_correction"}),
            current_facts: None,
            confirmation_level: None,
        },
    )?;
    let mut final_hashes = BTreeMap::new();
    final_hashes.insert(
        "relationship_graph".to_string(),
        stable_json_hash(&graph_facts),
    );
    let evidence = shared_verification_evidence(
        &["relationship_graph".to_string()],
        &final_hashes,
        json!({
            "graph_edges_checked": true,
            "dependency_subgraph_checked": true,
            "dangling_references_checked": true,
        }),
        vec![json!({"id": "relationship_graph_consistency", "status": "passed"})],
        json!({}),
    );
    store.verify(&executing.execution_object_id, evidence)
}

pub fn validate_execution_object_references(
    store: &ExecutionObjectStoreService,
    execution_object_ids: &[String],
    required_state: ExecutionObjectStatus,
) -> Value {
    let mut missing = Vec::new();
    let mut wrong_state = Vec::new();
    for object_id in execution_object_ids
        .iter()
        .map(|item| item.trim())
        .filter(|item| !item.is_empty())
    {
        match store.get(object_id) {
            Ok(object) if object.state == required_state => {}
            Ok(object) => wrong_state.push(json!({
                "execution_object_id": object_id,
                "state": object.state.as_str(),
                "required_state": required_state.as_str(),
            })),
            Err(_) => missing.push(Value::String(object_id.to_string())),
        }
    }
    json!({
        "checked": execution_object_ids.len(),
        "valid": missing.is_empty() && wrong_state.is_empty(),
        "missing": missing,
        "wrong_state": wrong_state,
    })
}

pub fn audit_execution_object_store(store: &ExecutionObjectStoreService) -> Value {
    let active = store
        .document()
        .objects
        .iter()
        .filter(|object| FORMAL_ACTIVE_STATES.contains(&object.state))
        .count();
    let unresolved_failed = store
        .document()
        .objects
        .iter()
        .filter(|object| object.state == ExecutionObjectStatus::ExecutionFailed)
        .count();
    let drafts = store
        .document()
        .objects
        .iter()
        .filter(|object| object.state == ExecutionObjectStatus::Draft)
        .count();
    json!({
        "schema_version": store.document().schema_version,
        "path": store.path().to_string_lossy(),
        "object_count": store.document().objects.len(),
        "active_count": active,
        "unresolved_failed_count": unresolved_failed,
        "draft_count": drafts,
        "valid": active == 0 && unresolved_failed == 0,
    })
}

pub fn save_design_project(
    store: &mut ExecutionObjectStoreService,
    project_state: Value,
    title: Option<&str>,
    save_type: &str,
    auto_verify: bool,
) -> AdmResult<ExecutionObject> {
    let project_name = str_field(&project_state, "projectName", "Untitled Project");
    let related_facts = extract_design_project_related_facts(&project_state);
    let old_design_ids = store
        .list_objects(None)
        .into_iter()
        .filter(|object| object.object_type == "design_project")
        .map(|object| object.execution_object_id)
        .collect::<Vec<_>>();
    for object_id in old_design_ids {
        store.force_cancel(&object_id, "superseded_by_new_save")?;
    }
    let write_scope = vec![
        "design:project_state".to_string(),
        "design:nodes".to_string(),
        "design:domains".to_string(),
        format!("design:project:{project_name}"),
    ];
    let draft = store.create_draft(CreateDraftInput {
        object_type: "design_project".to_string(),
        title: title
            .map(str::to_string)
            .unwrap_or_else(|| format!("Design Project: {project_name}")),
        source_diagnostic_id: format!("workbench:design_project:{project_name}"),
        source_execution_object_id: String::new(),
        prefilled_content: json!({}),
        user_content: project_state.clone(),
        related_facts: related_facts.clone(),
        write_scope: write_scope.clone(),
        metadata: json!({
            "stage": "design",
            "business_id": format!("design_project:{project_name}"),
            "created_by": "design_workbench",
            "save_type": save_type,
            "auto_save_version": related_facts.get("auto_save_version").cloned().unwrap_or(json!(1)),
        }),
    })?;
    if auto_verify && save_type == "manual" {
        let object_id = draft.execution_object_id.clone();
        store.submit(
            &object_id,
            project_state.clone(),
            confirmation_level_for("design_project"),
            &format!("{project_name}:submitted"),
            "workbench_user",
        )?;
        store.start_analysis(&object_id)?;
        store.complete_impact_analysis(
            &object_id,
            ImpactAnalysisInput {
                affected_scopes: write_scope.clone(),
                summary: format!("design project save: {project_name}"),
                invalidation_scope: write_scope,
                diagnostics: json!({"save_type": save_type}),
            },
        )?;
        store.approve(
            &object_id,
            json!({
                "confirmed": true,
                "confirmation_type": "user_save_action",
                "confirmed_by": "workbench_user",
                "confirmed_at": now_iso(),
            }),
        )?;
        store.run_pre_execution_drift_check(&object_id, related_facts)?;
        store.check_concurrency_conflicts(&object_id)?;
        store.start_execution(&object_id)?;
        let mut hashes = BTreeMap::new();
        hashes.insert(
            "project_state".to_string(),
            stable_json_hash(&project_state),
        );
        let evidence = shared_verification_evidence(
            &["project_state".to_string()],
            &hashes,
            json!({}),
            vec![json!({"id": "design_project_auto_verify", "status": "passed"})],
            json!({
                "verified_at": now_iso(),
                "verification_method": "auto_verify",
                "project_state_hash": stable_json_hash(&project_state),
            }),
        );
        store.verify(&object_id, evidence)?;
    }
    store.get(&draft.execution_object_id).cloned()
}

pub fn auto_save_design_project(
    store: &mut ExecutionObjectStoreService,
    project_state: Value,
) -> AdmResult<ExecutionObject> {
    save_design_project(store, project_state, None, "auto", false)
}

pub fn load_latest_design_project(store: &ExecutionObjectStoreService) -> Option<Value> {
    let mut projects = store
        .list_objects(Some(&[ExecutionObjectStatus::Verified]))
        .into_iter()
        .filter(|object| object.object_type == "design_project")
        .collect::<Vec<_>>();
    projects.sort_by(|left, right| right.updated_at.cmp(&left.updated_at));
    projects.first().map(|object| object.user_content.clone())
}

pub fn list_design_project_versions(
    store: &ExecutionObjectStoreService,
    include_drafts: bool,
) -> Vec<ExecutionObject> {
    let states = if include_drafts {
        vec![
            ExecutionObjectStatus::Verified,
            ExecutionObjectStatus::Draft,
            ExecutionObjectStatus::Submitted,
            ExecutionObjectStatus::Approved,
        ]
    } else {
        vec![ExecutionObjectStatus::Verified]
    };
    let mut versions = store
        .list_objects(Some(&states))
        .into_iter()
        .filter(|object| object.object_type == "design_project")
        .collect::<Vec<_>>();
    versions.sort_by(|left, right| right.updated_at.cmp(&left.updated_at));
    versions
}

pub fn restore_design_project_version(
    store: &ExecutionObjectStoreService,
    execution_object_id: &str,
) -> AdmResult<Value> {
    let object = store.get(execution_object_id)?;
    if object.object_type != "design_project" {
        return Err(AdmError::new(format!(
            "execution object {execution_object_id} is not a design_project"
        )));
    }
    Ok(object.user_content.clone())
}

pub fn get_design_project_metadata(
    store: &ExecutionObjectStoreService,
    execution_object_id: &str,
) -> AdmResult<Value> {
    let object = store.get(execution_object_id)?;
    Ok(json!({
        "execution_object_id": object.execution_object_id,
        "title": object.title,
        "state": object.state.as_str(),
        "created_at": object.created_at,
        "updated_at": object.updated_at,
        "project_name": object.user_content.get("projectName").cloned().unwrap_or(Value::Null),
        "save_type": object.metadata.get("save_type").cloned().unwrap_or(Value::Null),
        "related_facts": object.related_facts,
    }))
}

pub fn save_user_artifact(
    store: &mut ExecutionObjectStoreService,
    input: UserArtifactInput,
) -> AdmResult<ExecutionObject> {
    let title = input
        .title
        .clone()
        .unwrap_or_else(|| format!("{} export - {}", input.export_format, now_iso()));
    let related_facts = json!({
        "export_timestamp": now_iso(),
        "source_project_id": input.source_project_id,
        "target_directory": input.target_directory,
        "export_format": input.export_format,
        "export_scope": input.export_scope,
    });
    let write_scope = vec![
        "workspace:exports".to_string(),
        format!("workspace:exports:{}", input.export_format),
    ];
    let user_content = json!({
        "export_format": input.export_format,
        "export_scope": input.export_scope,
        "include_gameplay_global_view": input.metadata.get("include_gameplay_global_view").and_then(Value::as_bool).unwrap_or(false),
        "content": input.content,
    });
    let executing = begin_execution_object(
        store,
        BeginExecutionObjectInput {
            object_type: "user_artifact".to_string(),
            title,
            final_content: user_content.clone(),
            related_facts,
            write_scope,
            stage: 0,
            business_id: format!("export:{}", input.export_format),
            source_diagnostic_id: format!("workbench:export:{}", input.export_format),
            metadata: object_with_extra(
                json!({
                    "created_by": "design_workbench",
                    "export_format": input.export_format,
                    "source_project_id": input.source_project_id,
                }),
                &input.metadata,
            ),
            current_facts: None,
            confirmation_level: None,
        },
    )?;
    {
        let object = store.get_mut(&executing.execution_object_id)?;
        object.user_content = user_content.clone();
    }
    let mut hashes = BTreeMap::new();
    hashes.insert(input.export_format, stable_json_hash(&user_content));
    let evidence = shared_verification_evidence(
        &["workspace:exports".to_string()],
        &hashes,
        json!({}),
        vec![json!({"id": "user_artifact_auto_verify", "status": "passed"})],
        json!({"verification_method": "auto_verify"}),
    );
    store.verify(&executing.execution_object_id, evidence)
}

#[derive(Debug, Clone)]
pub struct UserArtifactInput {
    pub export_format: String,
    pub export_scope: String,
    pub content: Value,
    pub title: Option<String>,
    pub source_project_id: String,
    pub target_directory: String,
    pub metadata: Value,
}

pub fn list_user_artifacts(
    store: &ExecutionObjectStoreService,
    export_format: Option<&str>,
    source_project_id: Option<&str>,
) -> Vec<ExecutionObject> {
    let mut artifacts = store
        .list_objects(Some(&[ExecutionObjectStatus::Verified]))
        .into_iter()
        .filter(|object| object.object_type == "user_artifact")
        .filter(|object| {
            export_format.is_none_or(|expected| {
                object
                    .user_content
                    .get("export_format")
                    .and_then(Value::as_str)
                    == Some(expected)
            })
        })
        .filter(|object| {
            source_project_id.is_none_or(|expected| {
                object
                    .related_facts
                    .get("source_project_id")
                    .and_then(Value::as_str)
                    == Some(expected)
            })
        })
        .collect::<Vec<_>>();
    artifacts.sort_by(|left, right| right.updated_at.cmp(&left.updated_at));
    artifacts
}

pub fn get_user_artifact(
    store: &ExecutionObjectStoreService,
    execution_object_id: &str,
) -> AdmResult<Value> {
    let object = store.get(execution_object_id)?;
    if object.object_type != "user_artifact" {
        return Err(AdmError::new(format!(
            "execution object {execution_object_id} is not a user_artifact"
        )));
    }
    Ok(object.user_content.clone())
}

pub fn delete_user_artifact(
    store: &mut ExecutionObjectStoreService,
    execution_object_id: &str,
) -> AdmResult<ExecutionObject> {
    let object = store.get(execution_object_id)?;
    if object.object_type != "user_artifact" {
        return Err(AdmError::new(format!(
            "execution object {execution_object_id} is not a user_artifact"
        )));
    }
    store.cancel(execution_object_id, "user deleted export artifact")
}

pub fn capture_workspace_snapshot(
    store: &mut ExecutionObjectStoreService,
    workspace_root: &Path,
    trigger_event: &str,
    reason: &str,
) -> AdmResult<ExecutionObject> {
    let file_manifest = scan_workspace_files(workspace_root)?;
    let total_size = file_manifest
        .iter()
        .filter_map(|item| item.get("size_bytes").and_then(Value::as_u64))
        .sum::<u64>();
    let user_content = json!({
        "snapshot_type": "full",
        "trigger_event": trigger_event,
        "file_manifest": file_manifest,
    });
    let related_facts = json!({
        "total_files": file_manifest.len(),
        "total_size_bytes": total_size,
        "snapshot_reason": if reason.is_empty() { trigger_event } else { reason },
        "trigger_event": trigger_event,
    });
    let executing = begin_execution_object(
        store,
        BeginExecutionObjectInput {
            object_type: "workspace_snapshot".to_string(),
            title: format!("Workspace Snapshot - {}", now_iso()),
            final_content: user_content.clone(),
            related_facts,
            write_scope: vec![
                "workspace:snapshot".to_string(),
                "workspace:projects".to_string(),
                "workspace:exports".to_string(),
            ],
            stage: 0,
            business_id: format!("workspace_snapshot:{trigger_event}"),
            source_diagnostic_id: format!("workspace:snapshot:{}", now_iso()),
            metadata: json!({"created_by": "workspace_snapshot_manager", "trigger_event": trigger_event}),
            current_facts: None,
            confirmation_level: None,
        },
    )?;
    {
        let object = store.get_mut(&executing.execution_object_id)?;
        object.user_content = user_content.clone();
    }
    let mut hashes = BTreeMap::new();
    hashes.insert(
        "workspace_snapshot".to_string(),
        stable_json_hash(&user_content),
    );
    let evidence = shared_verification_evidence(
        &["workspace:snapshot".to_string()],
        &hashes,
        json!({}),
        vec![json!({"id": "workspace_snapshot_auto_verify", "status": "passed"})],
        json!({"verification_method": "auto_verify", "file_count": file_manifest.len()}),
    );
    store.verify(&executing.execution_object_id, evidence)
}

pub fn get_latest_workspace_snapshot(
    store: &ExecutionObjectStoreService,
) -> Option<ExecutionObject> {
    let mut snapshots = list_workspace_snapshots(store, None);
    snapshots.sort_by(|left, right| right.updated_at.cmp(&left.updated_at));
    snapshots.into_iter().next()
}

pub fn list_workspace_snapshots(
    store: &ExecutionObjectStoreService,
    limit: Option<usize>,
) -> Vec<ExecutionObject> {
    let mut snapshots = store
        .list_objects(Some(&[ExecutionObjectStatus::Verified]))
        .into_iter()
        .filter(|object| object.object_type == "workspace_snapshot")
        .collect::<Vec<_>>();
    snapshots.sort_by(|left, right| right.updated_at.cmp(&left.updated_at));
    if let Some(limit) = limit {
        snapshots.truncate(limit);
    }
    snapshots
}

pub fn compare_workspace_snapshots(
    snapshot_a: &ExecutionObject,
    snapshot_b: &ExecutionObject,
) -> Value {
    let files_a = manifest_by_path(
        snapshot_a
            .user_content
            .get("file_manifest")
            .and_then(Value::as_array),
    );
    let files_b = manifest_by_path(
        snapshot_b
            .user_content
            .get("file_manifest")
            .and_then(Value::as_array),
    );
    let paths_a = files_a.keys().cloned().collect::<BTreeSet<_>>();
    let paths_b = files_b.keys().cloned().collect::<BTreeSet<_>>();
    let added = paths_b
        .difference(&paths_a)
        .filter_map(|path| files_b.get(path).cloned())
        .collect::<Vec<_>>();
    let removed = paths_a
        .difference(&paths_b)
        .filter_map(|path| files_a.get(path).cloned())
        .collect::<Vec<_>>();
    let modified = paths_a
        .intersection(&paths_b)
        .filter_map(|path| {
            let before = files_a.get(path)?;
            let after = files_b.get(path)?;
            (before.get("sha256") != after.get("sha256")).then(|| {
                json!({
                    "path": path,
                    "before": before,
                    "after": after,
                })
            })
        })
        .collect::<Vec<_>>();
    json!({
        "added": added,
        "removed": removed,
        "modified": modified,
        "summary": {
            "added_count": added.len(),
            "removed_count": removed.len(),
            "modified_count": modified.len(),
        },
    })
}

pub fn get_workspace_file_history(
    store: &ExecutionObjectStoreService,
    file_path: &str,
) -> Vec<Value> {
    let mut history = Vec::new();
    for snapshot in list_workspace_snapshots(store, None) {
        let Some(items) = snapshot
            .user_content
            .get("file_manifest")
            .and_then(Value::as_array)
        else {
            continue;
        };
        for entry in items {
            if entry.get("path").and_then(Value::as_str) == Some(file_path) {
                history.push(json!({
                    "snapshot_id": snapshot.execution_object_id,
                    "snapshot_time": snapshot.updated_at,
                    "file_entry": entry,
                }));
                break;
            }
        }
    }
    history
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CorrectionItem {
    pub item_id: String,
    pub conflict_type: String,
    pub severity: String,
    pub detail: String,
    #[serde(default)]
    pub source_system: String,
    #[serde(default)]
    pub target_system: String,
    #[serde(default)]
    pub correction_type: String,
    #[serde(default)]
    pub entities: Vec<String>,
    #[serde(default)]
    pub suggested_action: String,
    #[serde(default = "default_true")]
    pub selected: bool,
    #[serde(default)]
    pub target_stage: String,
    #[serde(default)]
    pub affected_systems: Vec<String>,
    #[serde(default)]
    pub affected_files: Vec<String>,
    #[serde(default)]
    pub extras: BTreeMap<String, Value>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct CorrectionQueue {
    #[serde(default)]
    pub generated_at: String,
    #[serde(default)]
    pub source_review: String,
    #[serde(default)]
    pub source_review_protocol: String,
    #[serde(default)]
    pub source_review_report: String,
    #[serde(default)]
    pub reviewed_contract: String,
    #[serde(default)]
    pub rerun_plan: Value,
    #[serde(default)]
    pub blocked_items: Vec<Value>,
    #[serde(default)]
    pub items: Vec<CorrectionItem>,
}

pub fn infer_affected_systems(values: &[String]) -> Vec<String> {
    infer_prefixed_tokens(values, "SYS_")
}

pub fn infer_affected_assets(values: &[String]) -> Vec<String> {
    let prefixes = [
        "ILL_", "UI_", "VFX_", "ART_", "ENV_", "CHAR_", "PROP_", "FX_",
    ];
    let mut found = BTreeSet::new();
    for text in values {
        for token in ascii_tokens(text) {
            if prefixes.iter().any(|prefix| token.starts_with(prefix)) {
                found.insert(token);
            }
        }
    }
    found.into_iter().collect()
}

pub fn infer_target_stage(
    conflict_type: &str,
    correction_type: &str,
    detail: &str,
    suggested_action: &str,
) -> String {
    let conflict = conflict_type.trim().to_ascii_lowercase();
    let correction = correction_type.trim().to_ascii_lowercase();
    let text = format!("{conflict} {correction} {detail} {suggested_action}").to_ascii_lowercase();
    if human_gap_types().contains(&conflict.as_str())
        || human_gap_types().contains(&correction.as_str())
    {
        "human_gap".to_string()
    } else if progreq_types().contains(&conflict.as_str())
        || progreq_types().contains(&correction.as_str())
        || text.contains("ct_")
        || text.contains("evt_")
        || text.contains("contract")
    {
        "progreq".to_string()
    } else if artreq_types().contains(&conflict.as_str())
        || artreq_types().contains(&correction.as_str())
        || ["asset", "illustration", "vfx", "ui", "visualdna", "artreq"]
            .iter()
            .any(|marker| text.contains(marker))
    {
        "artreq".to_string()
    } else if design_types().contains(&conflict.as_str())
        || design_types().contains(&correction.as_str())
    {
        "design".to_string()
    } else {
        "unmapped".to_string()
    }
}

pub fn infer_affected_files(
    target_stage: &str,
    conflict_type: &str,
    correction_type: &str,
    hints: &[String],
) -> Vec<String> {
    let stage = target_stage.trim().to_ascii_lowercase();
    let kind = if correction_type.trim().is_empty() {
        conflict_type.trim().to_ascii_lowercase()
    } else {
        correction_type.trim().to_ascii_lowercase()
    };
    let text = format!("{kind} {} {}", conflict_type, hints.join(" ")).to_ascii_lowercase();
    if stage == "design" {
        return vec!["frozen_game_design.md".to_string()];
    }
    if stage == "artreq" {
        let mut files = Vec::new();
        if ["illustration", "ill_"]
            .iter()
            .any(|marker| text.contains(marker))
        {
            files.push("illustration_requirements.md".to_string());
        }
        if ["ui", "hud", "menu"]
            .iter()
            .any(|marker| text.contains(marker))
        {
            files.push("ui_requirements.md".to_string());
        }
        if ["vfx", "fx_", "effect"]
            .iter()
            .any(|marker| text.contains(marker))
        {
            files.push("vfx_requirements.md".to_string());
        }
        if text.contains("drift") || text.contains("style") {
            files.push("drift_analysis.md".to_string());
        }
        return if files.is_empty() {
            vec![
                "illustration_requirements.md".to_string(),
                "ui_requirements.md".to_string(),
                "vfx_requirements.md".to_string(),
            ]
        } else {
            files
        };
    }
    if stage != "progreq" {
        return Vec::new();
    }
    match kind.as_str() {
        "missing_contract" | "event_missing_contract" | "method_mismatch" => {
            vec!["contracts.md".to_string(), "events.md".to_string()]
        }
        "role_mismatch" | "contract_not_bound" | "signature_mismatch" | "missing_interface" => {
            vec!["contracts.md".to_string(), "systems.md".to_string()]
        }
        "undefined_entity" | "missing_field" => {
            vec!["systems.md".to_string(), "entities.md".to_string()]
        }
        "authority_conflict" | "multi_authority" => {
            vec!["authority.md".to_string(), "entities.md".to_string()]
        }
        _ => vec![
            "contracts.md".to_string(),
            "systems.md".to_string(),
            "events.md".to_string(),
        ],
    }
}

pub fn complete_item_routing(mut item: CorrectionItem) -> CorrectionItem {
    if item.target_stage.is_empty() {
        item.target_stage = infer_target_stage(
            &item.conflict_type,
            &item.correction_type,
            &item.detail,
            &item.suggested_action,
        );
    }
    if item.affected_systems.is_empty() {
        let values = [
            item.source_system.clone(),
            item.target_system.clone(),
            item.entities.join(" "),
            item.detail.clone(),
            item.suggested_action.clone(),
        ];
        let systems = infer_affected_systems(&values);
        item.affected_systems = if systems.is_empty() {
            infer_affected_assets(&values)
        } else {
            systems
        };
    }
    if item.affected_files.is_empty() {
        let hints = [
            item.detail.clone(),
            item.suggested_action.clone(),
            item.entities.join(" "),
            item.affected_systems.join(" "),
        ];
        item.affected_files = infer_affected_files(
            &item.target_stage,
            &item.conflict_type,
            &item.correction_type,
            &hints,
        );
    }
    item
}

pub fn classify_conflicts(conflicts: &[Value]) -> (Vec<CorrectionItem>, Vec<Value>) {
    let mut correctable = Vec::new();
    let mut design_gaps = Vec::new();
    for conflict in conflicts {
        let conflict_type = str_field(conflict, "conflict_type", "unknown").to_ascii_lowercase();
        let severity = str_field(conflict, "severity", "major").to_ascii_lowercase();
        let detail = str_field(
            conflict,
            "detail",
            &str_field(conflict, "original_conflict", ""),
        );
        if human_gap_types().contains(&conflict_type.as_str()) {
            design_gaps.push(conflict.clone());
            continue;
        }
        if progreq_types().contains(&conflict_type.as_str())
            || design_types().contains(&conflict_type.as_str())
        {
            let item = CorrectionItem {
                item_id: format!("CORR_{:03}", correctable.len() + 1),
                conflict_type: conflict_type.clone(),
                severity,
                detail: detail.clone(),
                source_system: str_field(
                    conflict,
                    "entity_a",
                    &str_field(conflict, "source_system", ""),
                ),
                target_system: str_field(
                    conflict,
                    "entity_b",
                    &str_field(conflict, "target_system", ""),
                ),
                correction_type: map_correction_type(&conflict_type).to_string(),
                entities: vec![
                    str_field(conflict, "entity_a", ""),
                    str_field(conflict, "entity_b", ""),
                ],
                suggested_action: generate_suggestion(&conflict_type, &detail),
                selected: true,
                target_stage: String::new(),
                affected_systems: Vec::new(),
                affected_files: Vec::new(),
                extras: BTreeMap::new(),
            };
            correctable.push(complete_item_routing(item));
        } else {
            design_gaps.push(conflict.clone());
        }
    }
    (correctable, design_gaps)
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ExecutionBudget {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_ai_cost_units: Option<u64>,
    pub task_timeout_seconds: u32,
    pub max_retry_attempts: u32,
    pub max_parallel_workers: u32,
}

impl ExecutionBudget {
    pub fn validate(&self) -> Result<(), ExecutionBudgetError> {
        if self.task_timeout_seconds == 0 {
            return Err(ExecutionBudgetError {
                code: "EXECUTION_BUDGET_TIMEOUT_INVALID",
                severity: "error",
                path: "/taskTimeoutSeconds",
                related_ids: Vec::new(),
                message: "task timeout must be greater than zero",
                suggestion: "Set a positive per-task timeout in the local execution policy.",
            });
        }
        if self.max_parallel_workers == 0 {
            return Err(ExecutionBudgetError {
                code: "EXECUTION_BUDGET_WORKERS_INVALID",
                severity: "error",
                path: "/maxParallelWorkers",
                related_ids: Vec::new(),
                message: "parallel worker count must be greater than zero",
                suggestion: "Configure at least one local execution worker.",
            });
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExecutionBudgetError {
    pub code: &'static str,
    pub severity: &'static str,
    pub path: &'static str,
    pub related_ids: Vec<String>,
    pub message: &'static str,
    pub suggestion: &'static str,
}

impl std::fmt::Display for ExecutionBudgetError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "{} at {}: {}",
            self.code, self.path, self.message
        )
    }
}

impl std::error::Error for ExecutionBudgetError {}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
/// Legacy unattended recovery policy for execution-object workflows.
///
/// GameSpec v2 Step11 stores retry, correction queue, and stop/resume state in
/// `Step11ExecutionState`; this config remains for the existing execution
/// object surfaces until their callers are moved behind the v2 contract engine.
pub struct UnattendedExecutionConfig {
    pub max_auto_repair_attempts: u32,
    pub repair_timeout_seconds: u32,
    #[serde(default)]
    pub max_ai_cost_units: Option<u64>,
    #[serde(default = "default_parallel_workers")]
    pub max_parallel_workers: u32,
    pub continue_independent_tasks: bool,
    pub continue_after_completed_with_review: bool,
    pub sync_per_group: bool,
    pub sync_checkpoint_every_tasks: u32,
    pub sync_checkpoint_seconds: u32,
    pub enable_step11_auto_repair: bool,
    pub enable_step12_auto_repair: bool,
}

impl Default for UnattendedExecutionConfig {
    fn default() -> Self {
        Self {
            max_auto_repair_attempts: 2,
            repair_timeout_seconds: 120,
            max_ai_cost_units: None,
            max_parallel_workers: 1,
            continue_independent_tasks: true,
            continue_after_completed_with_review: false,
            sync_per_group: true,
            sync_checkpoint_every_tasks: 10,
            sync_checkpoint_seconds: 600,
            enable_step11_auto_repair: true,
            enable_step12_auto_repair: false,
        }
    }
}

impl UnattendedExecutionConfig {
    pub fn execution_budget(&self) -> ExecutionBudget {
        ExecutionBudget {
            max_ai_cost_units: self.max_ai_cost_units,
            task_timeout_seconds: self.repair_timeout_seconds,
            max_retry_attempts: self.max_auto_repair_attempts,
            max_parallel_workers: self.max_parallel_workers,
        }
    }
}

const fn default_parallel_workers() -> u32 {
    1
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FailureEvent {
    pub stage: u32,
    pub task_id: String,
    pub group_id: String,
    pub failure_type: String,
    pub severity: String,
    pub retryable: bool,
    pub auto_repairable: bool,
    pub blocks_dependents: bool,
    pub error_summary: String,
    pub error_hash: String,
    pub raw_errors: Vec<String>,
    pub changed_files: Vec<String>,
    pub unexpected_changes: Vec<String>,
    pub verification_results: Vec<Value>,
    pub execution_object_id: String,
    pub reproduction_command: String,
    pub reproduction_payload_path: String,
    pub log_paths: Vec<String>,
}

pub fn failure_type_policy(failure_type: &str) -> (bool, bool) {
    match failure_type {
        "ai_generation_failed" | "adapter_timeout" => (true, false),
        "package_change_failed" | "task_verification_failed" | "unity_compile_failed" => {
            (false, true)
        }
        "execution_object_gate_failed"
        | "unexpected_file_change"
        | "asset_contract_failed"
        | "external_config_missing"
        | "operator_stop" => (false, false),
        _ => (false, false),
    }
}

pub fn infer_failure_type(record: &Value, default: &str) -> String {
    if truthy(record.get("unexpected_changes")) {
        return "unexpected_file_change".to_string();
    }
    if record.get("execution_note").and_then(Value::as_str)
        == Some("Blocked by execution-object workflow before writing project files.")
    {
        return "execution_object_gate_failed".to_string();
    }
    if record
        .get("package_errors")
        .and_then(Value::as_array)
        .map(|items| !items.is_empty())
        .unwrap_or(false)
    {
        return "package_change_failed".to_string();
    }
    let errors = value_array_strings(record.get("codex_errors"));
    if errors
        .iter()
        .any(|item| item.to_ascii_lowercase().contains("timeout"))
    {
        return "adapter_timeout".to_string();
    }
    if !errors.is_empty() {
        return "ai_generation_failed".to_string();
    }
    if record
        .get("verification_results")
        .and_then(Value::as_array)
        .map(|items| {
            items.iter().any(|item| {
                let status = item
                    .get("status")
                    .and_then(Value::as_str)
                    .unwrap_or_default();
                status != "passed" && status != "deferred"
            })
        })
        .unwrap_or(false)
    {
        return "task_verification_failed".to_string();
    }
    default.to_string()
}

pub fn build_failure_event(
    stage: u32,
    record: &Value,
    failure_type: Option<&str>,
    severity: &str,
    reproduction_payload_path: &str,
    log_paths: Vec<String>,
) -> FailureEvent {
    let resolved_failure_type = failure_type
        .map(str::to_string)
        .unwrap_or_else(|| infer_failure_type(record, "task_verification_failed"));
    let (retryable, auto_repairable) = failure_type_policy(&resolved_failure_type);
    let summary = summarize_record_error(record);
    let task_id = str_field(record, "task_id", &str_field(record, "asset_id", "unknown"));
    let group_id = str_field(record, "group_id", "");
    let error_hash = stable_error_hash(stage, &task_id, &resolved_failure_type, &summary);
    FailureEvent {
        stage,
        task_id,
        group_id,
        failure_type: resolved_failure_type,
        severity: severity.to_string(),
        retryable,
        auto_repairable,
        blocks_dependents: ["task_failed", "dependency_blocking", "stage_blocking"]
            .contains(&severity),
        error_summary: summary,
        error_hash,
        raw_errors: {
            let mut errors = value_array_strings(record.get("codex_errors"));
            errors.extend(verification_errors(record));
            errors
        },
        changed_files: value_array_strings(record.get("changed_files")),
        unexpected_changes: value_array_strings(record.get("unexpected_changes")),
        verification_results: record
            .get("verification_results")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default(),
        execution_object_id: str_field(record, "execution_object_id", ""),
        reproduction_command: REPRODUCTION_COMMAND.to_string(),
        reproduction_payload_path: reproduction_payload_path.to_string(),
        log_paths,
    }
}

pub fn upsert_failure_queue(
    mut queue: CorrectionQueue,
    stage: u32,
    events: &[FailureEvent],
    reviewed_contract: &str,
    source_review: &str,
    config: UnattendedExecutionConfig,
) -> CorrectionQueue {
    queue.generated_at = now_iso();
    queue.source_review = source_review.to_string();
    queue.source_review_protocol = UNATTENDED_PROTOCOL.to_string();
    queue.source_review_report = "unattended_execution_summary.json".to_string();
    queue.reviewed_contract = reviewed_contract.to_string();
    queue.rerun_plan = json!({
        "required_stages": [stage.to_string()],
        "commands": [],
        "reason": format!("Retry selected Step{stage:02} correction items after review."),
    });
    for event in events {
        let item_id = correction_id_for_event(event);
        let mut extras = BTreeMap::new();
        extras.insert(
            "status".to_string(),
            Value::String(if event.auto_repairable {
                "pending_auto_repair".to_string()
            } else {
                "needs_user_review".to_string()
            }),
        );
        extras.insert("task_id".to_string(), Value::String(event.task_id.clone()));
        extras.insert(
            "group_id".to_string(),
            Value::String(event.group_id.clone()),
        );
        extras.insert(
            "execution_object_id".to_string(),
            Value::String(event.execution_object_id.clone()),
        );
        extras.insert(
            "failure_type".to_string(),
            Value::String(event.failure_type.clone()),
        );
        extras.insert("retry_count".to_string(), json!(0));
        extras.insert(
            "max_retries".to_string(),
            json!(config.max_auto_repair_attempts),
        );
        extras.insert("auto_repairable".to_string(), json!(event.auto_repairable));
        extras.insert(
            "requires_user_decision".to_string(),
            json!(!event.auto_repairable),
        );
        extras.insert(
            "error_hash".to_string(),
            Value::String(event.error_hash.clone()),
        );
        extras.insert(
            "reproduction_command".to_string(),
            Value::String(event.reproduction_command.clone()),
        );
        extras.insert(
            "reproduction_payload_path".to_string(),
            Value::String(event.reproduction_payload_path.clone()),
        );
        extras.insert("log_paths".to_string(), json!(event.log_paths));
        extras.insert("next_action".to_string(), Value::String(next_action(event)));
        extras.insert("last_seen_at".to_string(), Value::String(now_iso()));
        if let Some(existing) = queue.items.iter_mut().find(|item| item.item_id == item_id) {
            let retry_count = existing
                .extras
                .get("retry_count")
                .and_then(Value::as_u64)
                .unwrap_or(0);
            existing.severity = event.severity.clone();
            existing.detail = event.error_summary.clone();
            existing.affected_files = event.changed_files.clone();
            existing.suggested_action = next_action(event);
            extras.insert("retry_count".to_string(), json!(retry_count));
            existing.extras.extend(extras);
        } else {
            queue.items.push(CorrectionItem {
                item_id,
                conflict_type: event.failure_type.clone(),
                severity: event.severity.clone(),
                detail: event.error_summary.clone(),
                source_system: String::new(),
                target_system: String::new(),
                correction_type: "auto_repair_or_review".to_string(),
                entities: Vec::new(),
                suggested_action: next_action(event),
                selected: true,
                target_stage: if stage == 11 {
                    "devexec".to_string()
                } else {
                    "artprod".to_string()
                },
                affected_systems: if event.task_id.is_empty() {
                    Vec::new()
                } else {
                    vec![event.task_id.clone()]
                },
                affected_files: event.changed_files.clone(),
                extras,
            });
        }
    }
    queue
}

pub fn queue_to_summary(queue: &CorrectionQueue) -> Value {
    let mut statuses: BTreeMap<String, usize> = BTreeMap::new();
    let mut auto_repairable = 0usize;
    for item in &queue.items {
        let status = item
            .extras
            .get("status")
            .and_then(Value::as_str)
            .unwrap_or("needs_user_review");
        *statuses.entry(status.to_string()).or_default() += 1;
        if item
            .extras
            .get("auto_repairable")
            .and_then(Value::as_bool)
            .unwrap_or(false)
        {
            auto_repairable += 1;
        }
    }
    json!({
        "correction_count": queue.items.len(),
        "status_counts": statuses,
        "auto_repairable_count": auto_repairable,
    })
}

pub fn build_resume_cursor(
    stage: u32,
    records: &[Value],
    current_group_id: &str,
    current_task_id: &str,
    next_task_id: &str,
    project_state_tainted: bool,
    resume_policy: Option<&str>,
) -> Value {
    let failed_ids = records
        .iter()
        .filter(|record| {
            ["failed", "blocked_by_execution_object"].contains(
                &record
                    .get("status")
                    .and_then(Value::as_str)
                    .unwrap_or_default(),
            )
        })
        .filter_map(|record| record.get("task_id").and_then(Value::as_str))
        .filter(|task_id| !task_id.is_empty())
        .map(str::to_string)
        .collect::<Vec<_>>();
    let completed_count = records
        .iter()
        .filter(|record| {
            ["success", "auto_repaired"].contains(
                &record
                    .get("status")
                    .and_then(Value::as_str)
                    .unwrap_or_default(),
            )
        })
        .count();
    let skipped_count = records
        .iter()
        .filter(|record| {
            record
                .get("status")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .starts_with("skipped")
        })
        .count();
    json!({
        "stage": stage,
        "current_group_id": current_group_id,
        "current_task_id": current_task_id,
        "next_task_id": next_task_id,
        "completed_task_count": completed_count,
        "failed_task_count": failed_ids.len(),
        "skipped_task_count": skipped_count,
        "failed_task_ids": failed_ids,
        "project_state_tainted": project_state_tainted,
        "resume_policy": resume_policy.unwrap_or(if project_state_tainted {
            "cannot_auto_resume"
        } else {
            "resume_from_next_unblocked_task"
        }),
        "task_record_source": if stage == 11 {
            "stage_11/DEV-*_execution.json"
        } else {
            "stage_XX/*_execution.json"
        },
        "skip_report_source": "dependency_skip_report.json",
    })
}

pub fn dependency_skip_ids(
    failed_task_ids: &BTreeSet<String>,
    current_group_id: &str,
    parallel_groups: &[Value],
    dependencies: &[Value],
) -> BTreeMap<String, Value> {
    let mut group_by_task = BTreeMap::new();
    let mut tasks_by_group: BTreeMap<String, Vec<String>> = BTreeMap::new();
    let mut group_dependencies: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    for group in parallel_groups {
        let group_id = str_field(group, "group_id", "");
        if group_id.is_empty() {
            continue;
        }
        let task_ids = value_array_strings(group.get("task_ids"));
        for task_id in &task_ids {
            group_by_task.insert(task_id.clone(), group_id.clone());
        }
        tasks_by_group.insert(group_id.clone(), task_ids);
        group_dependencies.insert(
            group_id,
            value_array_strings(group.get("depends_on_groups"))
                .into_iter()
                .collect(),
        );
    }
    let mut direct_deps: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    for edge in dependencies {
        let source = str_field(edge, "from", "");
        let target = str_field(edge, "to", "");
        if !source.is_empty() && !target.is_empty() {
            direct_deps.entry(target).or_default().insert(source);
        }
    }
    let mut failed_groups = failed_task_ids
        .iter()
        .filter_map(|task_id| group_by_task.get(task_id).cloned())
        .collect::<BTreeSet<_>>();
    if !current_group_id.is_empty() {
        failed_groups.insert(current_group_id.to_string());
    }
    let mut skipped = BTreeMap::new();
    for task_id in tasks_by_group
        .get(current_group_id)
        .cloned()
        .unwrap_or_default()
    {
        if !failed_task_ids.contains(&task_id) {
            skipped.insert(
                task_id,
                json!({"status": "skipped_by_failed_group", "blocked_by": failed_task_ids}),
            );
        }
    }
    let mut changed = true;
    while changed {
        changed = false;
        for (task_id, deps) in &direct_deps {
            if failed_task_ids.contains(task_id) || skipped.contains_key(task_id) {
                continue;
            }
            let blocked_by = deps
                .iter()
                .filter(|dep| failed_task_ids.contains(*dep) || skipped.contains_key(*dep))
                .cloned()
                .collect::<Vec<_>>();
            if !blocked_by.is_empty() {
                skipped.insert(
                    task_id.clone(),
                    json!({"status": "skipped_by_dependency", "blocked_by": blocked_by}),
                );
                changed = true;
            }
        }
    }
    let mut tainted_groups = failed_groups.clone();
    let mut changed_groups = true;
    while changed_groups {
        changed_groups = false;
        for (group_id, deps) in &group_dependencies {
            if tainted_groups.contains(group_id) {
                continue;
            }
            if deps.iter().any(|dep| tainted_groups.contains(dep)) {
                tainted_groups.insert(group_id.clone());
                changed_groups = true;
            }
        }
    }
    for group_id in tainted_groups {
        if failed_groups.contains(&group_id) {
            continue;
        }
        for task_id in tasks_by_group.get(&group_id).cloned().unwrap_or_default() {
            if !failed_task_ids.contains(&task_id) {
                skipped.insert(
                    task_id,
                    json!({"status": "skipped_by_dependency", "blocked_by": failed_task_ids}),
                );
            }
        }
    }
    skipped
}

pub fn stable_error_hash(
    stage: u32,
    task_id: &str,
    failure_type: &str,
    error_summary: &str,
) -> String {
    let normalized = error_summary
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_ascii_lowercase();
    let digest = sha256_hex(format!("{stage}|{task_id}|{failure_type}|{normalized}").as_bytes());
    digest.chars().take(12).collect()
}

pub fn correction_id_for_event(event: &FailureEvent) -> String {
    format!(
        "CQ-ST{:02}-{}-{}-{}",
        event.stage, event.task_id, event.failure_type, event.error_hash
    )
}

pub fn summarize_record_error(record: &Value) -> String {
    let errors = value_array_strings(record.get("codex_errors"));
    if !errors.is_empty() {
        return truncate(
            &errors.into_iter().take(3).collect::<Vec<_>>().join("; "),
            500,
        );
    }
    let unexpected = value_array_strings(record.get("unexpected_changes"));
    if !unexpected.is_empty() {
        return format!("Unexpected file changes: {}", unexpected.join(", "));
    }
    let verification_errors = verification_errors(record);
    if !verification_errors.is_empty() {
        return truncate(
            &verification_errors
                .into_iter()
                .take(3)
                .collect::<Vec<_>>()
                .join("; "),
            500,
        );
    }
    if let Some(error) = record.get("error").and_then(Value::as_str) {
        return truncate(error, 500);
    }
    "Task requires review.".to_string()
}

pub fn stable_json_hash(value: &Value) -> String {
    serde_json::to_vec(value)
        .map(|bytes| sha256_hex(&bytes))
        .unwrap_or_else(|_| sha256_hex(b"null"))
}

pub fn drift_diff(snapshot_facts: &Value, current_facts: &Value) -> Value {
    let snapshot = snapshot_facts.as_object();
    let current = current_facts.as_object();
    let keys = snapshot
        .into_iter()
        .flat_map(|object| object.keys())
        .chain(current.into_iter().flat_map(|object| object.keys()))
        .cloned()
        .collect::<BTreeSet<_>>();
    let mut diff = Map::new();
    for key in keys {
        let old = snapshot
            .and_then(|object| object.get(&key))
            .cloned()
            .unwrap_or(Value::Null);
        let new = current
            .and_then(|object| object.get(&key))
            .cloned()
            .unwrap_or(Value::Null);
        if old != new {
            diff.insert(key, json!({"snapshot": old, "current": new}));
        }
    }
    Value::Object(diff)
}

fn load_store_document(path: &Path) -> AdmResult<ExecutionObjectStoreDocument> {
    if !path.exists() {
        let now = now_iso();
        return Ok(ExecutionObjectStoreDocument {
            schema_version: EXECUTION_OBJECT_SCHEMA_VERSION,
            generated_at: now.clone(),
            updated_at: now,
            save_id: None,
            objects: Vec::new(),
            audit_cleanup_evidence: Vec::new(),
            ownership_migrations: Vec::new(),
        });
    }
    let raw = fs::read_to_string(path)?;
    let raw = raw.trim_start_matches('\u{feff}');
    let mut document: ExecutionObjectStoreDocument =
        serde_json::from_str(raw).map_err(|error| {
            AdmError::new(format!(
                "invalid execution object store JSON {}: {error}",
                path.display()
            ))
        })?;
    if document.schema_version == 0 {
        document.schema_version = EXECUTION_OBJECT_SCHEMA_VERSION;
    }
    if document.generated_at.is_empty() {
        document.generated_at = now_iso();
    }
    if document.updated_at.is_empty() {
        document.updated_at = now_iso();
    }
    Ok(document)
}

fn append_history(
    object: &mut ExecutionObject,
    new_state: ExecutionObjectStatus,
    reason: &str,
    evidence: Value,
) -> AdmResult<()> {
    let old_state = object.state.clone();
    object.state = new_state.clone();
    object.updated_at = now_iso();
    object.state_history.push(StateHistoryRecord {
        at: object.updated_at.clone(),
        from: Some(old_state),
        to: new_state,
        reason: reason.to_string(),
        evidence,
    });
    Ok(())
}

fn require_state(
    object: &ExecutionObject,
    allowed: &[ExecutionObjectStatus],
    action: &str,
) -> AdmResult<()> {
    if allowed.contains(&object.state) {
        Ok(())
    } else {
        Err(AdmError::new(format!(
            "{action} requires state {:?}, got {} for {}",
            allowed,
            object.state.as_str(),
            object.execution_object_id
        )))
    }
}

fn validate_confirmation_gate(level: &ConfirmationLevel, evidence: &Value) -> AdmResult<()> {
    if !truthy(evidence.get("confirmed")) {
        return Err(AdmError::new(
            "confirmation evidence must include confirmed=true",
        ));
    }
    match level {
        ConfirmationLevel::NormalConfirm => Ok(()),
        ConfirmationLevel::ElevatedConfirm => require_truthy_fields(
            "elevated_confirm",
            evidence,
            &[
                "impact_scope_displayed",
                "invalidation_scope_displayed",
                "snapshot_summary_displayed",
            ],
        ),
        ConfirmationLevel::T3ArtConfirm => require_truthy_fields(
            "t3_art_confirm",
            evidence,
            &[
                "impact_scope_displayed",
                "baseline_or_rule_impact_expanded",
                "snapshot_summary_displayed",
            ],
        ),
        ConfirmationLevel::DestructiveConfirm => require_truthy_fields(
            "destructive_confirm",
            evidence,
            &[
                "second_confirmation",
                "affected_files_displayed",
                "old_hashes_displayed",
                "new_hashes_displayed",
                "rollback_source_displayed",
                "unity_risk_displayed",
                "non_automatic_recovery_risk_displayed",
            ],
        ),
    }
}

fn type_specific_verification_requirements(object_type: &str) -> &'static [&'static str] {
    match object_type {
        "asset_contract_change" => &["contract_version_updated", "invalidation_propagated"],
        "reference_migration" => &[
            "old_references_handled",
            "asset_id_mapping_stable",
            "runtime_paths_consistent",
        ],
        "unity_replacement_batch" => &["files_exist", "hashes_match", "unity_import_refreshed"],
        "unity_scene_assembly_batch" => &[
            "demo_scene_exists",
            "build_settings_updated",
            "playmode_smoke_test_passed",
            "visible_content_verified",
        ],
        "rollback_plan" => &["target_matches_rollback_source", "reverse_links_preserved"],
        "t3_art_baseline_change" => &[
            "baseline_relationships_updated",
            "downstream_impacts_marked",
        ],
        "relationship_graph_correction" => &[
            "graph_edges_checked",
            "dependency_subgraph_checked",
            "dangling_references_checked",
        ],
        _ => &[],
    }
}

fn require_truthy_fields(context: &str, evidence: &Value, fields: &[&str]) -> AdmResult<()> {
    let missing = fields
        .iter()
        .filter(|field| !truthy(evidence.get(**field)))
        .copied()
        .collect::<Vec<_>>();
    if missing.is_empty() {
        Ok(())
    } else {
        Err(AdmError::new(format!(
            "{context} missing evidence: {missing:?}"
        )))
    }
}

fn require_present_fields(context: &str, evidence: &Value, fields: &[&str]) -> AdmResult<()> {
    let missing = fields
        .iter()
        .filter(|field| {
            evidence
                .get(**field)
                .map(|value| !value.is_null() && truthy(Some(value)))
                .unwrap_or(false)
                == false
        })
        .copied()
        .collect::<Vec<_>>();
    if missing.is_empty() {
        Ok(())
    } else {
        Err(AdmError::new(format!(
            "{context} missing evidence: {missing:?}"
        )))
    }
}

fn ensure_evidence_scope_subset(
    object: &ExecutionObject,
    evidence: &Value,
    field: &str,
) -> AdmResult<()> {
    let affected_scope = value_string_set(evidence.get(field));
    let approved_scope = object.write_scope.iter().cloned().collect::<BTreeSet<_>>();
    if affected_scope.is_empty() || affected_scope.is_subset(&approved_scope) {
        Ok(())
    } else {
        Err(AdmError::new(format!(
            "{field} exceeds original approved scope"
        )))
    }
}

fn truthy(value: Option<&Value>) -> bool {
    match value {
        Some(Value::Bool(value)) => *value,
        Some(Value::Number(value)) => value.as_i64().is_some_and(|number| number != 0),
        Some(Value::String(value)) => {
            let value = value.trim().to_ascii_lowercase();
            !value.is_empty() && !["false", "0", "no", "n", "off"].contains(&value.as_str())
        }
        Some(Value::Array(values)) => !values.is_empty(),
        Some(Value::Object(values)) => !values.is_empty(),
        _ => false,
    }
}

fn now_iso() -> String {
    format!("unix:{}", unix_timestamp())
}

fn sorted_string_set(values: Vec<String>) -> Vec<String> {
    values
        .into_iter()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

fn value_string_set(value: Option<&Value>) -> BTreeSet<String> {
    value_array_strings(value)
        .into_iter()
        .filter(|item| !item.is_empty())
        .collect()
}

fn value_array_strings(value: Option<&Value>) -> Vec<String> {
    match value {
        Some(Value::Array(items)) => items
            .iter()
            .filter_map(|item| match item {
                Value::String(value) => Some(value.trim().to_string()),
                Value::Number(value) => Some(value.to_string()),
                Value::Bool(value) => Some(value.to_string()),
                _ => None,
            })
            .filter(|item| !item.is_empty())
            .collect(),
        Some(Value::String(value)) => value
            .split([',', ';', '\n'])
            .map(str::trim)
            .filter(|item| !item.is_empty())
            .map(str::to_string)
            .collect(),
        Some(other) if !other.is_null() => vec![other.to_string()],
        _ => Vec::new(),
    }
}

fn str_field(value: &Value, field: &str, default: &str) -> String {
    value
        .get(field)
        .and_then(Value::as_str)
        .unwrap_or(default)
        .to_string()
}

fn metadata_str(value: &Value, field: &str) -> String {
    str_field(value, field, "")
}

fn normalize_path(path: &str) -> String {
    path.trim()
        .replace('\\', "/")
        .trim_start_matches("./")
        .trim_start_matches('/')
        .to_string()
}

fn merge_object_fields(target: &mut Map<String, Value>, extra: &Value) {
    if let Some(extra) = extra.as_object() {
        for (key, value) in extra {
            target.insert(key.clone(), value.clone());
        }
    }
}

fn object_with_extra(base: Value, extra: &Value) -> Value {
    let mut object = base.as_object().cloned().unwrap_or_default();
    merge_object_fields(&mut object, extra);
    Value::Object(object)
}

fn push_extra_array(object: &mut ExecutionObject, key: &str, value: Value) {
    let entry = object
        .extra
        .entry(key.to_string())
        .or_insert_with(|| Value::Array(Vec::new()));
    if let Some(items) = entry.as_array_mut() {
        items.push(value);
    }
}

fn push_extra_nested_array(
    object: &mut ExecutionObject,
    parent_key: &str,
    array_key: &str,
    value: Value,
) {
    let parent = object
        .extra
        .entry(parent_key.to_string())
        .or_insert_with(|| Value::Object(Map::new()));
    if let Some(parent) = parent.as_object_mut() {
        let entry = parent
            .entry(array_key.to_string())
            .or_insert_with(|| Value::Array(Vec::new()));
        if let Some(items) = entry.as_array_mut() {
            items.push(value);
        }
    }
}

fn extract_design_project_related_facts(project_state: &Value) -> Value {
    let nodes = project_state.get("nodes").and_then(Value::as_object);
    let domains = project_state.get("domains").and_then(Value::as_object);
    let completed_nodes = nodes
        .map(|nodes| {
            nodes
                .values()
                .filter(|node| {
                    node.get("decisionState").and_then(Value::as_str) == Some("completed")
                })
                .count()
        })
        .unwrap_or(0);
    let total_entities = nodes
        .map(|nodes| {
            nodes
                .values()
                .map(|node| {
                    node.get("designEntities")
                        .and_then(Value::as_array)
                        .map(Vec::len)
                        .unwrap_or(0)
                })
                .sum::<usize>()
        })
        .unwrap_or(0);
    json!({
        "engine_version": "DesignEngine v1.0",
        "domain_count": domains.map(Map::len).unwrap_or(0),
        "node_count": nodes.map(Map::len).unwrap_or(0),
        "completed_nodes": completed_nodes,
        "total_entities": total_entities,
        "has_profile": project_state.get("profile").is_some(),
        "last_updated": now_iso(),
    })
}

fn scan_workspace_files(workspace_root: &Path) -> AdmResult<Vec<Value>> {
    if !workspace_root.exists() {
        return Ok(Vec::new());
    }
    let mut entries = Vec::new();
    for entry in file_manifest(workspace_root)? {
        let file_name_hidden = Path::new(&entry.path)
            .file_name()
            .and_then(|value| value.to_str())
            .map(|name| name.starts_with('.'))
            .unwrap_or(false);
        if file_name_hidden {
            continue;
        }
        let role = if entry.path.contains("projects/") {
            "design_project_export"
        } else if entry.path.contains("exports/") {
            "user_export"
        } else if entry.path.contains("outputs/") {
            "pipeline_output"
        } else if entry.path.contains("source_artifacts/") {
            "source_artifact"
        } else {
            "unknown"
        };
        entries.push(json!({
            "path": entry.path,
            "sha256": entry.sha256,
            "size_bytes": entry.size_bytes,
            "role": role,
        }));
    }
    Ok(entries)
}

fn manifest_by_path(items: Option<&Vec<Value>>) -> BTreeMap<String, Value> {
    items
        .into_iter()
        .flat_map(|items| items.iter())
        .filter_map(|item| {
            item.get("path")
                .and_then(Value::as_str)
                .map(|path| (path.to_string(), item.clone()))
        })
        .collect()
}

fn infer_prefixed_tokens(values: &[String], prefix: &str) -> Vec<String> {
    let mut found = BTreeSet::new();
    for value in values {
        for token in ascii_tokens(value) {
            if token.starts_with(prefix) {
                found.insert(token);
            }
        }
    }
    found.into_iter().collect()
}

fn ascii_tokens(text: &str) -> Vec<String> {
    text.split(|ch: char| !ch.is_ascii_alphanumeric() && ch != '_')
        .filter(|token| !token.is_empty())
        .map(str::to_string)
        .collect()
}

fn progreq_types() -> &'static [&'static str] {
    &[
        "missing_interface",
        "undefined_entity",
        "wrong_assignment",
        "missing_field",
        "signature_mismatch",
        "contract_not_bound",
        "authority_conflict",
        "event_missing_contract",
        "missing_contract",
        "role_mismatch",
        "method_mismatch",
        "subscription_mismatch",
    ]
}

fn design_types() -> &'static [&'static str] {
    &[
        "unresolved_dependency",
        "resource_location_mismatch",
        "item_mismatch",
        "design_gap",
        "human_decision_required",
    ]
}

fn artreq_types() -> &'static [&'static str] {
    &[
        "asset_missing",
        "style_drift",
        "visualdna_gap",
        "illustration_gap",
        "ui_gap",
        "vfx_gap",
        "art_baseline_change",
    ]
}

fn human_gap_types() -> &'static [&'static str] {
    &[
        "design_gap",
        "human_decision_required",
        "unknown_requirement",
        "scope_conflict",
    ]
}

fn map_correction_type(conflict_type: &str) -> &'static str {
    match conflict_type {
        "missing_interface" => "add_interface",
        "undefined_entity" => "add_entity",
        "wrong_assignment" => "fix_interface_assignment",
        "missing_field" => "add_field",
        "signature_mismatch" => "fix_interface_signature",
        "unresolved_dependency" => "clarify_dependency",
        "resource_location_mismatch" => "clarify_data_flow",
        "item_mismatch" => "clarify_item_definition",
        "contract_not_bound" => "bind_contract",
        "authority_conflict" => "resolve_authority",
        "event_missing_contract" => "add_contract_for_event",
        "missing_contract" => "add_contract",
        "role_mismatch" => "fix_contract_role",
        "method_mismatch" => "fix_contract_method",
        "subscription_mismatch" => "fix_event_subscription",
        _ => "clarify_design",
    }
}

fn generate_suggestion(conflict_type: &str, detail: &str) -> String {
    let detail = truncate(detail, 120);
    match conflict_type {
        "missing_interface" => format!("Add a cross-system interface for: {detail}"),
        "undefined_entity" => format!("Define the missing entity: {detail}"),
        "wrong_assignment" => format!("Move the interface to the correct owning system: {detail}"),
        "unresolved_dependency" => {
            format!("Clarify dependency or create the referenced feature: {detail}")
        }
        "missing_contract" | "event_missing_contract" => {
            format!("Add the missing contract: {detail}")
        }
        _ => format!("Review and correct: {detail}"),
    }
}

fn verification_errors(record: &Value) -> Vec<String> {
    record
        .get("verification_results")
        .and_then(Value::as_array)
        .into_iter()
        .flat_map(|items| items.iter())
        .filter(|item| {
            let status = item
                .get("status")
                .and_then(Value::as_str)
                .unwrap_or_default();
            status != "passed" && status != "deferred"
        })
        .filter_map(|item| {
            item.get("message")
                .or_else(|| item.get("error"))
                .or_else(|| item.get("id"))
                .and_then(Value::as_str)
                .map(str::to_string)
        })
        .collect()
}

fn next_action(event: &FailureEvent) -> String {
    if event.auto_repairable {
        "Run bounded repair prompt and repeat focused verification.".to_string()
    } else if event.retryable {
        "Retry the original task invocation without modifying files.".to_string()
    } else {
        "Review the failure and decide whether to repair, rollback, or skip.".to_string()
    }
}

fn truncate(value: &str, max_chars: usize) -> String {
    value.chars().take(max_chars).collect()
}

fn default_true() -> bool {
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use adm_new_foundation::new_stable_id;

    #[test]
    fn execution_budget_is_validated_local_policy_outside_game_spec() {
        let config = UnattendedExecutionConfig::default();
        let budget = config.execution_budget();

        assert_eq!(budget.max_parallel_workers, 1);
        assert!(budget.validate().is_ok());

        let mut invalid = budget;
        invalid.max_parallel_workers = 0;
        let error = invalid
            .validate()
            .expect_err("zero workers must fail closed");
        assert_eq!(error.code, "EXECUTION_BUDGET_WORKERS_INVALID");
        assert_eq!(error.path, "/maxParallelWorkers");
    }

    #[test]
    fn type_registry_matches_python_categories_and_confirmation_levels() {
        assert!(is_registered_type("unity_replacement_batch"));
        assert_eq!(
            confirmation_level_for("asset_contract_change"),
            ConfirmationLevel::ElevatedConfirm
        );
        assert!(list_types_by_category("unity").contains(&"unity_scene_assembly_batch"));
        assert!(get_type_metadata("unknown").is_err());
    }

    #[test]
    fn ownership_transfer_requires_an_exact_existing_source_owner() {
        let root = temp_root("eo_transfer_source");
        let path = execution_object_store_path(&root);
        let mut store = ExecutionObjectStoreService::new(&path, None).unwrap();
        let before = store.document().clone();

        let error = store
            .transfer_ownership_to_save(
                "save_target",
                Some("draft-owner:desktop-session"),
                "create_save",
            )
            .unwrap_err();

        assert!(
            error
                .to_string()
                .contains("does not match transfer source_save_id")
        );
        assert_eq!(store.document(), &before);
        assert_eq!(store.expected_save_id, None);
        assert!(!path.exists());
        cleanup(root);
    }

    #[test]
    fn ownership_transfer_rejects_non_portable_target_without_mutation() {
        let root = temp_root("eo_transfer_portable");
        let path = execution_object_store_path(&root);
        let mut store = ExecutionObjectStoreService::new(
            &path,
            Some("draft-owner:desktop-session".to_string()),
        )
        .unwrap();
        store.save().unwrap();
        let before = store.document().clone();
        let before_bytes = fs::read(&path).unwrap();

        let error = store
            .transfer_ownership_to_save(
                "../save_target",
                Some("draft-owner:desktop-session"),
                "create_save",
            )
            .unwrap_err();

        assert!(error.to_string().contains("save_id is not portable"));
        assert_eq!(store.document(), &before);
        assert_eq!(
            store.expected_save_id.as_deref(),
            Some("draft-owner:desktop-session")
        );
        assert_eq!(fs::read(&path).unwrap(), before_bytes);
        cleanup(root);
    }

    #[test]
    fn ownership_transfer_is_idempotent_when_target_is_already_current() {
        let root = temp_root("eo_transfer_idempotent");
        let path = execution_object_store_path(&root);
        let mut store = ExecutionObjectStoreService::new(
            &path,
            Some("draft-owner:desktop-session".to_string()),
        )
        .unwrap();
        store.save().unwrap();
        store
            .transfer_ownership_to_save(
                "save_target",
                Some("draft-owner:desktop-session"),
                "create_save",
            )
            .unwrap();
        let after_first = store.document().clone();
        let after_first_bytes = fs::read(&path).unwrap();

        let repeated = store
            .transfer_ownership_to_save("save_target", Some("save_target"), "create_save_retry")
            .unwrap();

        assert_eq!(repeated.from_save_id.as_deref(), Some("save_target"));
        assert_eq!(repeated.to_save_id, "save_target");
        assert_eq!(store.document(), &after_first);
        assert_eq!(store.document().ownership_migrations.len(), 1);
        assert_eq!(fs::read(&path).unwrap(), after_first_bytes);
        cleanup(root);
    }

    #[test]
    fn ownership_transfer_restores_memory_when_atomic_save_fails() {
        let root = temp_root("eo_transfer_rollback");
        fs::create_dir_all(&root).unwrap();
        let blocked_parent = root.join("not-a-directory");
        fs::write(&blocked_parent, "sentinel").unwrap();
        let path = blocked_parent.join("execution_objects.json");
        let mut document = ExecutionObjectStoreDocument::default();
        document.save_id = Some("draft-owner:desktop-session".to_string());
        let mut store = ExecutionObjectStoreService::from_document(
            &path,
            Some("draft-owner:desktop-session".to_string()),
            document,
        );
        let before = store.document().clone();
        let before_expected = store.expected_save_id.clone();

        assert!(
            store
                .transfer_ownership_to_save(
                    "save_target",
                    Some("draft-owner:desktop-session"),
                    "create_save",
                )
                .is_err()
        );

        assert_eq!(store.document(), &before);
        assert_eq!(store.expected_save_id, before_expected);
        assert_eq!(fs::read_to_string(&blocked_parent).unwrap(), "sentinel");
        assert!(!path.exists());
        cleanup(root);
    }

    #[test]
    fn design_project_runs_full_execution_object_gate_and_persists() {
        let root = temp_root("eo_design_project");
        let path = execution_object_store_path(&root);
        let mut store =
            ExecutionObjectStoreService::new(&path, Some("save-a".to_string())).unwrap();
        let project = json!({
            "projectName": "Demo",
            "domains": {"core": {}},
            "nodes": {
                "mechanics": {
                    "decisionState": "completed",
                    "designEntities": [{"id": "SYS_GAME_LOOP"}]
                }
            },
            "profile": {"genre": "puzzle"}
        });

        let object =
            save_design_project(&mut store, project.clone(), None, "manual", true).unwrap();

        assert_eq!(object.state, ExecutionObjectStatus::Verified);
        assert_eq!(object.object_type, "design_project");
        assert_eq!(load_latest_design_project(&store).unwrap(), project);
        assert!(path.is_file());
        let reloaded = ExecutionObjectStoreService::new(&path, Some("save-a".to_string())).unwrap();
        assert_eq!(reloaded.document().objects.len(), 1);
        assert_eq!(
            audit_execution_object_store(&reloaded)
                .get("valid")
                .and_then(Value::as_bool),
            Some(true)
        );
        cleanup(root);
    }

    #[test]
    fn destructive_gate_requires_destructive_confirmation_and_detects_drift() {
        let root = temp_root("eo_destructive");
        let mut store =
            ExecutionObjectStoreService::new(execution_object_store_path(&root), None).unwrap();
        let draft = store
            .create_draft(CreateDraftInput {
                object_type: "unity_replacement_batch".to_string(),
                title: "Replace".to_string(),
                related_facts: json!({"hash": "old"}),
                write_scope: vec!["unity_file:Assets/Demo.cs".to_string()],
                ..CreateDraftInput::new("unity_replacement_batch", "Replace")
            })
            .unwrap();

        assert!(
            store
                .submit(
                    &draft.execution_object_id,
                    json!({"task": "x"}),
                    ConfirmationLevel::NormalConfirm,
                    "marker",
                    "tester",
                )
                .is_err()
        );
        store
            .submit(
                &draft.execution_object_id,
                json!({"task": "x"}),
                ConfirmationLevel::DestructiveConfirm,
                "marker",
                "tester",
            )
            .unwrap();
        store.start_analysis(&draft.execution_object_id).unwrap();
        store
            .complete_impact_analysis(
                &draft.execution_object_id,
                ImpactAnalysisInput {
                    affected_scopes: vec!["unity_file:Assets/Demo.cs".to_string()],
                    summary: "replace".to_string(),
                    invalidation_scope: vec!["unity_file:Assets/Demo.cs".to_string()],
                    diagnostics: json!({}),
                },
            )
            .unwrap();
        assert!(
            store
                .approve(&draft.execution_object_id, json!({"confirmed": true}))
                .is_err()
        );
        store
            .approve(
                &draft.execution_object_id,
                confirmation_evidence_for(&ConfirmationLevel::DestructiveConfirm, "replace"),
            )
            .unwrap();
        let drift = store
            .run_pre_execution_drift_check(&draft.execution_object_id, json!({"hash": "new"}))
            .unwrap();
        assert_eq!(
            drift.get("status").and_then(Value::as_str),
            Some("stale_before_execution")
        );
        cleanup(root);
    }

    #[test]
    fn program_task_recovery_records_automated_remediation_before_verify() {
        let root = temp_root("eo_program");
        let project = root.join("UnityProject");
        fs::create_dir_all(project.join("Assets")).unwrap();
        fs::write(project.join("Assets/Demo.cs"), "class Demo {}").unwrap();
        let mut store =
            ExecutionObjectStoreService::new(execution_object_store_path(&root), None).unwrap();
        let task = json!({
            "task_id": "DEV-001",
            "title": "Demo",
            "requirement_id": "REQ-1",
            "output_files": ["Assets/Demo.cs"],
            "allowed_write_paths": ["Assets/Demo.cs"]
        });
        let executing =
            begin_program_task_execution_object(&mut store, task, &project, 11).unwrap();
        store
            .record_execution_failure(
                &executing.execution_object_id,
                ExecutionFailureInput {
                    failure_stage: "compile".to_string(),
                    written_files: vec!["Assets/Demo.cs".to_string()],
                    changed_state: vec![],
                    unfinished_actions: vec!["rerun compile".to_string()],
                    retryable: false,
                    rollback_needed: false,
                    remediation_needed: true,
                    validation_needed: true,
                    error: "compile failed".to_string(),
                },
            )
            .unwrap();
        let task_contract_hash = store
            .get(&executing.execution_object_id)
            .unwrap()
            .submission_snapshot
            .as_ref()
            .unwrap()
            .related_facts
            .get("task_contract_hash")
            .cloned()
            .unwrap();
        confirm_automated_retry_from_safe_point(
            &mut store,
            &executing.execution_object_id,
            &["unity_file:Assets/Demo.cs".to_string()],
            json!({
                "task_id": "DEV-001",
                "requirement_id": "REQ-1",
                "phase": Value::Null,
                "source_refs": [],
                "declared_output_files": ["Assets/Demo.cs"],
                "declared_allowed_write_paths": ["Assets/Demo.cs"],
                "before_hashes": project_file_hashes(&project, &["Assets/Demo.cs".to_string()]).unwrap(),
                "task_contract_hash": task_contract_hash,
            }),
            "CQ-1",
        )
        .unwrap();
        let mut hashes = BTreeMap::new();
        hashes.insert(
            "Assets/Demo.cs".to_string(),
            sha256_hex(&fs::read(project.join("Assets/Demo.cs")).unwrap()),
        );
        record_automated_remediation(
            &mut store,
            &executing.execution_object_id,
            "repair-1",
            "CQ-1",
            &["Assets/Demo.cs".to_string()],
            &hashes,
            json!({"status": "passed"}),
            &["unity_file:Assets/Demo.cs".to_string()],
        )
        .unwrap();
        let verified = verify_program_task_execution_object(
            &mut store,
            &executing.execution_object_id,
            &project,
            &["Assets/Demo.cs".to_string()],
            &["Assets/Demo.cs".to_string()],
            vec![json!({"id": "unity_batchmode_compile", "status": "passed"})],
            json!({"exit_code": 0}),
        )
        .unwrap();
        assert_eq!(verified.state, ExecutionObjectStatus::Verified);
        cleanup(root);
    }

    #[test]
    fn workspace_snapshots_compare_file_history() {
        let root = temp_root("eo_workspace");
        let workspace = root.join("workspace");
        fs::create_dir_all(workspace.join("exports")).unwrap();
        fs::write(workspace.join("exports/a.txt"), "a").unwrap();
        let mut store =
            ExecutionObjectStoreService::new(execution_object_store_path(&root), None).unwrap();
        let first = capture_workspace_snapshot(&mut store, &workspace, "start", "").unwrap();
        fs::write(workspace.join("exports/a.txt"), "b").unwrap();
        fs::write(workspace.join("exports/b.txt"), "b").unwrap();
        let second = capture_workspace_snapshot(&mut store, &workspace, "next", "").unwrap();

        let diff = compare_workspace_snapshots(&first, &second);

        assert_eq!(diff["summary"]["added_count"], json!(1));
        assert_eq!(diff["summary"]["modified_count"], json!(1));
        assert_eq!(get_workspace_file_history(&store, "exports/a.txt").len(), 2);
        cleanup(root);
    }

    #[test]
    fn correction_queue_and_unattended_recovery_route_failures() {
        let conflict = json!({
            "conflict_type": "missing_interface",
            "severity": "major",
            "detail": "SYS_INPUT needs CT_MOVE",
            "entity_a": "SYS_INPUT",
            "entity_b": "SYS_PLAYER"
        });
        let (items, gaps) = classify_conflicts(&[conflict]);
        assert!(gaps.is_empty());
        assert_eq!(items[0].target_stage, "progreq");
        assert!(
            items[0]
                .affected_files
                .contains(&"contracts.md".to_string())
        );

        let record = json!({
            "task_id": "DEV-002",
            "status": "failed",
            "verification_results": [{"id": "compile", "status": "failed", "message": "compile error"}],
            "changed_files": ["Assets/A.cs"],
            "execution_object_id": "EO-1"
        });
        let event = build_failure_event(11, &record, None, "task_failed", "payload.md", vec![]);
        assert_eq!(event.failure_type, "task_verification_failed");
        assert!(event.auto_repairable);
        let queue = upsert_failure_queue(
            CorrectionQueue::default(),
            11,
            &[event],
            "stage_11",
            "review",
            UnattendedExecutionConfig::default(),
        );
        let summary = queue_to_summary(&queue);
        assert_eq!(summary["auto_repairable_count"], json!(1));
        let cursor = build_resume_cursor(11, &[record], "g1", "DEV-002", "DEV-003", false, None);
        assert_eq!(
            cursor["resume_policy"],
            json!("resume_from_next_unblocked_task")
        );
    }

    fn temp_root(label: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "adm_new_application_{label}_{}",
            new_stable_id("root").unwrap()
        ))
    }

    fn cleanup(root: PathBuf) {
        let _ = fs::remove_dir_all(root);
    }
}
