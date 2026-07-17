use adm_new_change_kernel::{ChangeAuditRecord, ChangeKernel, SpecPatch, SpecStore};
use adm_new_foundation::sha256_hex;
use adm_new_game_spec::ProductEnvelope;
use serde_json::Value;

use crate::{CompletionAdapter, StructuredCompletionService};

use super::validation::{candidate_from_json_map, validate_candidate};
use super::{
    BOUNDED_COMPLETION_SCHEMA_VERSION, BoundedCompletionAudit, BoundedCompletionRun,
    CANDIDATE_SPEC_PATCH_SCHEMA, CompletionRunStatus, ConfirmationPolicyConfig, ConfirmationRecord,
    PromptPack, ValidatedCandidate,
};

#[derive(Debug, Clone)]
pub struct BoundedCompletionService<A> {
    adapter: A,
    policy: ConfirmationPolicyConfig,
    max_retries: u32,
}

impl<A> BoundedCompletionService<A>
where
    A: CompletionAdapter,
{
    pub fn new(adapter: A, policy: ConfirmationPolicyConfig) -> Self {
        Self {
            adapter,
            policy,
            max_retries: 1,
        }
    }

    pub fn with_max_retries(mut self, max_retries: u32) -> Self {
        self.max_retries = max_retries;
        self
    }

    pub fn run_against_store(
        &self,
        prompt_pack: &PromptPack,
        store: &SpecStore,
    ) -> BoundedCompletionRun {
        let prompt = build_prompt(prompt_pack);
        let input_hash = sha256_hex(prompt.as_bytes());
        if !self.policy.ai_enabled {
            return run_report(
                CompletionRunStatus::NotCalled,
                prompt_pack,
                input_hash,
                None,
                None,
                None,
                None,
                audit_errors(&["AI completion is disabled by project policy"]),
                0,
                None,
            );
        }

        let completion =
            StructuredCompletionService::with_max_retries(&self.adapter, self.max_retries)
                .generate_json_contract(CANDIDATE_SPEC_PATCH_SCHEMA, &prompt);
        let output_hash =
            (!completion.raw_text.is_empty()).then(|| sha256_hex(completion.raw_text.as_bytes()));
        if !completion.ok {
            return run_report(
                CompletionRunStatus::Failed,
                prompt_pack,
                input_hash,
                output_hash,
                None,
                None,
                None,
                completion.errors,
                completion.attempts,
                None,
            );
        }

        let candidate = match candidate_from_json_map(completion.data) {
            Ok(candidate) => candidate,
            Err(error) => {
                return run_report(
                    CompletionRunStatus::Rejected,
                    prompt_pack,
                    input_hash,
                    output_hash,
                    None,
                    None,
                    None,
                    vec![error.message().to_string()],
                    completion.attempts,
                    None,
                );
            }
        };
        let candidate_patch_id = Some(candidate.patch_id.clone());
        let validated =
            match validate_candidate(prompt_pack, store, candidate, output_hash.as_deref()) {
                Ok(validated) => validated,
                Err(error) => {
                    return run_report(
                        CompletionRunStatus::Rejected,
                        prompt_pack,
                        input_hash,
                        output_hash,
                        candidate_patch_id,
                        None,
                        None,
                        vec![error.message().to_string()],
                        completion.attempts,
                        None,
                    );
                }
            };

        let decision = self
            .policy
            .decision_for(validated.risk, &validated.spec_patch.declared_write_paths);
        if decision.auto_commit {
            let receipt = match store.submit(validated.spec_patch.clone()) {
                Ok(receipt) => receipt,
                Err(error) => {
                    return run_report(
                        CompletionRunStatus::Rejected,
                        prompt_pack,
                        input_hash,
                        output_hash,
                        candidate_patch_id,
                        Some(validated.clone()),
                        Some(ConfirmationRecord {
                            mode: decision.mode.as_str().to_string(),
                            accepted: false,
                            actor: "policy".to_string(),
                            reason: error.to_string(),
                            sample_size: decision.sample_size,
                        }),
                        vec![error.to_string()],
                        completion.attempts,
                        None,
                    );
                }
            };
            let status = if receipt.committed() {
                CompletionRunStatus::Committed
            } else {
                CompletionRunStatus::Rejected
            };
            let errors =
                if receipt.committed() {
                    Vec::new()
                } else {
                    vec![receipt.audit.failure_message.clone().unwrap_or_else(|| {
                        "canonical SpecStore rejected the candidate".to_string()
                    })]
                };
            return run_report(
                status,
                prompt_pack,
                input_hash,
                output_hash,
                candidate_patch_id,
                Some(validated),
                Some(ConfirmationRecord {
                    mode: decision.mode.as_str().to_string(),
                    accepted: receipt.committed(),
                    actor: "policy".to_string(),
                    reason: decision.reason,
                    sample_size: decision.sample_size,
                }),
                errors,
                completion.attempts,
                Some(receipt.audit),
            );
        }

        run_report(
            CompletionRunStatus::Confirmed,
            prompt_pack,
            input_hash,
            output_hash,
            candidate_patch_id,
            Some(validated),
            Some(ConfirmationRecord {
                mode: decision.mode.as_str().to_string(),
                accepted: false,
                actor: "human_required".to_string(),
                reason: decision.reason,
                sample_size: decision.sample_size,
            }),
            Vec::new(),
            completion.attempts,
            None,
        )
    }
}

pub fn manual_spec_patch_run(store: &SpecStore, patch: SpecPatch) -> BoundedCompletionRun {
    let input_hash = sha256_hex(serde_json::to_vec(&patch).unwrap_or_default().as_slice());
    let prompt_pack = PromptPack {
        schema_version: BOUNDED_COMPLETION_SCHEMA_VERSION.to_string(),
        task_id: "manual_spec_patch".to_string(),
        model_config_id: "manual".to_string(),
        base_revision: patch.base_revision,
        base_hash: patch.base_hash.clone(),
        product_envelope: store
            .snapshot()
            .map(|snapshot| snapshot.spec.technical.product_envelope)
            .unwrap_or_else(|_| medium_envelope()),
        relevant_subgraph: Value::Null,
        open_questions: Vec::new(),
        allowed_write_paths: patch.declared_write_paths.clone(),
        output_schema: Value::Null,
    };
    let receipt = match store.submit(patch) {
        Ok(receipt) => receipt,
        Err(error) => {
            return run_report(
                CompletionRunStatus::Rejected,
                &prompt_pack,
                input_hash,
                None,
                None,
                None,
                None,
                vec![error.to_string()],
                0,
                None,
            );
        }
    };
    run_report(
        if receipt.committed() {
            CompletionRunStatus::Committed
        } else {
            CompletionRunStatus::Rejected
        },
        &prompt_pack,
        input_hash,
        None,
        None,
        None,
        Some(ConfirmationRecord {
            mode: "attended".to_string(),
            accepted: receipt.committed(),
            actor: "human".to_string(),
            reason: "manual spec patch submission".to_string(),
            sample_size: None,
        }),
        receipt.audit.failure_message.clone().into_iter().collect(),
        0,
        Some(receipt.audit),
    )
}

fn build_prompt(prompt_pack: &PromptPack) -> String {
    let pack_json = serde_json::to_string_pretty(prompt_pack).unwrap_or_else(|_| "{}".to_string());
    format!(
        concat!(
            "You are proposing a bounded GameSpec patch.\n",
            "Return only one JSON object matching candidate_spec_patch_v1.\n",
            "Do not write files. Do not change paths outside allowedWritePaths.\n",
            "The deterministic Rust store is the only committer.\n\n",
            "{pack_json}"
        ),
        pack_json = pack_json
    )
}

fn run_report(
    status: CompletionRunStatus,
    prompt_pack: &PromptPack,
    input_hash: String,
    output_hash: Option<String>,
    candidate_patch_id: Option<String>,
    validated: Option<ValidatedCandidate>,
    confirmation: Option<ConfirmationRecord>,
    errors: Vec<String>,
    attempts: u32,
    spec_audit: Option<ChangeAuditRecord>,
) -> BoundedCompletionRun {
    let risk = validated.as_ref().map(|candidate| candidate.risk);
    let impact = validated.as_ref().map(|candidate| candidate.impact.clone());
    let validation_hash = sha256_hex(
        serde_json::to_vec(&serde_json::json!({
            "status": status,
            "candidatePatchId": candidate_patch_id,
            "risk": risk,
            "impact": impact,
            "errors": errors,
        }))
        .unwrap_or_default()
        .as_slice(),
    );
    BoundedCompletionRun {
        status,
        candidate_patch_id,
        risk,
        impact,
        audit: BoundedCompletionAudit {
            schema_version: BOUNDED_COMPLETION_SCHEMA_VERSION.to_string(),
            model_config_id: prompt_pack.model_config_id.clone(),
            input_hash,
            output_hash,
            validation_hash,
            risk,
            confirmation,
            errors,
            attempts,
            schema_name: CANDIDATE_SPEC_PATCH_SCHEMA.to_string(),
        },
        spec_audit,
    }
}

fn audit_errors(messages: &[&str]) -> Vec<String> {
    messages
        .iter()
        .map(|message| (*message).to_string())
        .collect()
}

fn medium_envelope() -> ProductEnvelope {
    use adm_new_game_spec::ProductionScale::Medium;
    ProductEnvelope {
        scene_scale: Medium,
        system_complexity: Medium,
        asset_scale: Medium,
        content_volume: Medium,
    }
}
