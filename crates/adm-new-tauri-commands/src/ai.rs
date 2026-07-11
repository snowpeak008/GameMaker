use adm_new_application::{AiInterviewApplicationService, AiInterviewTurnReport};
use adm_new_contracts::ai::{AiInterviewState, AiResponseMode, AiResponsePayload, AiSchemaMode};
use adm_new_contracts::project::ProjectState;
use adm_new_foundation::{AdmError, AdmResult};
use serde::{Deserialize, Serialize};

use crate::{CommandAdapterResult, handle_command};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SubmitAiTurnRequest {
    #[serde(default)]
    pub user_message: String,
    pub schema_mode: AiSchemaMode,
    #[serde(default)]
    pub payload_json: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ForceAiOutputRequest {
    pub schema_mode: AiSchemaMode,
    #[serde(default)]
    pub payload: Option<AiResponsePayload>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MarkAiInaccurateRequest {
    pub node_id: String,
    pub reason: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SaveAiArchiveRequest {
    #[serde(default)]
    pub archive_path: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AiTurnReportView {
    pub mode: AiResponseMode,
    pub applied_project_state: bool,
    pub archive_path: String,
    pub memory_events: Vec<String>,
    pub diagnostics: Vec<String>,
}

impl From<AiInterviewTurnReport> for AiTurnReportView {
    fn from(value: AiInterviewTurnReport) -> Self {
        Self {
            mode: value.mode,
            applied_project_state: value.applied_project_state,
            archive_path: value.archive_path,
            memory_events: value.memory_events,
            diagnostics: value.diagnostics,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AiStreamEventView {
    pub stage: String,
    pub turn_id: String,
    pub message: String,
    pub running: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AiBackgroundJobStatus {
    pub mapping_status: String,
    pub summary_correction_status: String,
    pub active_job_count: usize,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AiInterviewCommandView {
    pub report: AiTurnReportView,
    pub state: AiInterviewState,
    pub stream_events: Vec<AiStreamEventView>,
    pub background_jobs: AiBackgroundJobStatus,
}

pub trait AiCommandService {
    fn load_ai_interview(&self, state: &ProjectState) -> AdmResult<AiInterviewState>;

    fn submit_ai_turn(
        &self,
        state: &mut ProjectState,
        request: &SubmitAiTurnRequest,
    ) -> AdmResult<AiInterviewCommandView>;

    fn force_ai_output(
        &self,
        state: &mut ProjectState,
        request: &ForceAiOutputRequest,
    ) -> AdmResult<AiInterviewCommandView>;

    fn mark_ai_inaccurate(
        &self,
        state: &mut ProjectState,
        request: &MarkAiInaccurateRequest,
    ) -> AdmResult<AiInterviewState>;

    fn save_ai_archive(
        &self,
        state: &mut ProjectState,
        request: &SaveAiArchiveRequest,
    ) -> AdmResult<AiInterviewState>;
}

impl AiCommandService for AiInterviewApplicationService {
    fn load_ai_interview(&self, state: &ProjectState) -> AdmResult<AiInterviewState> {
        Ok(state.ai_interview.clone())
    }

    fn submit_ai_turn(
        &self,
        state: &mut ProjectState,
        request: &SubmitAiTurnRequest,
    ) -> AdmResult<AiInterviewCommandView> {
        let Some(payload_json) = request.payload_json.as_ref() else {
            return Err(AdmError::new(
                "AI backend unavailable: submit_ai_turn requires provider payload_json",
            ));
        };
        let report = self.handle_payload_json(state, request.schema_mode.clone(), payload_json)?;
        let stream_events = ai_stream_events_for_state(&state.ai_interview);
        let background_jobs = ai_background_status_for_state(&state.ai_interview);
        Ok(AiInterviewCommandView {
            report: report.into(),
            state: state.ai_interview.clone(),
            stream_events,
            background_jobs,
        })
    }

    fn force_ai_output(
        &self,
        state: &mut ProjectState,
        request: &ForceAiOutputRequest,
    ) -> AdmResult<AiInterviewCommandView> {
        let payload = request.payload.clone().ok_or_else(|| {
            AdmError::new("AI backend unavailable: force_ai_output requires provider payload")
        })?;
        let report = self.handle_payload(state, request.schema_mode.clone(), payload)?;
        let stream_events = ai_stream_events_for_state(&state.ai_interview);
        let background_jobs = ai_background_status_for_state(&state.ai_interview);
        Ok(AiInterviewCommandView {
            report: report.into(),
            state: state.ai_interview.clone(),
            stream_events,
            background_jobs,
        })
    }

    fn mark_ai_inaccurate(
        &self,
        state: &mut ProjectState,
        request: &MarkAiInaccurateRequest,
    ) -> AdmResult<AiInterviewState> {
        self.mark_inaccurate(state, &request.node_id, &request.reason)
    }

    fn save_ai_archive(
        &self,
        state: &mut ProjectState,
        request: &SaveAiArchiveRequest,
    ) -> AdmResult<AiInterviewState> {
        self.save_manual_archive_marker(state, request.archive_path.as_deref())
    }
}

pub fn load_ai_interview<S>(
    service: &S,
    state: &ProjectState,
) -> CommandAdapterResult<AiInterviewState>
where
    S: AiCommandService,
{
    handle_command(|| service.load_ai_interview(state))
}

pub fn submit_ai_turn<S>(
    service: &S,
    state: &mut ProjectState,
    request: SubmitAiTurnRequest,
) -> CommandAdapterResult<AiInterviewCommandView>
where
    S: AiCommandService,
{
    handle_command(|| service.submit_ai_turn(state, &request))
}

pub fn force_ai_output<S>(
    service: &S,
    state: &mut ProjectState,
    request: ForceAiOutputRequest,
) -> CommandAdapterResult<AiInterviewCommandView>
where
    S: AiCommandService,
{
    handle_command(|| service.force_ai_output(state, &request))
}

pub fn mark_ai_inaccurate<S>(
    service: &S,
    state: &mut ProjectState,
    request: MarkAiInaccurateRequest,
) -> CommandAdapterResult<AiInterviewState>
where
    S: AiCommandService,
{
    handle_command(|| service.mark_ai_inaccurate(state, &request))
}

pub fn save_ai_archive<S>(
    service: &S,
    state: &mut ProjectState,
    request: SaveAiArchiveRequest,
) -> CommandAdapterResult<AiInterviewState>
where
    S: AiCommandService,
{
    handle_command(|| service.save_ai_archive(state, &request))
}

fn ai_stream_events_for_state(state: &AiInterviewState) -> Vec<AiStreamEventView> {
    let running = matches!(
        state.status.as_str(),
        "running" | "processing_result" | "queued"
    ) || matches!(
        state.backend_stage.as_str(),
        "queued" | "building_prompt" | "calling_codex" | "waiting_codex" | "validating"
    );
    let stage = if state.backend_stage.trim().is_empty() {
        state.status.clone()
    } else {
        state.backend_stage.clone()
    };
    vec![AiStreamEventView {
        stage: stage.clone(),
        turn_id: state.active_turn_id.clone(),
        message: if state.last_error.is_empty() {
            format!("AI interview stage: {stage}")
        } else {
            state.last_error.clone()
        },
        running,
    }]
}

fn ai_background_status_for_state(state: &AiInterviewState) -> AiBackgroundJobStatus {
    let mapping_status = if !state.active_turn_id.is_empty()
        && matches!(state.status.as_str(), "running" | "processing_result")
    {
        "pending"
    } else {
        "idle"
    };
    let has_corrections = !state.summary.v1.last_user_corrections.is_empty();
    let summary_correction_status = if has_corrections {
        "needs_revision"
    } else {
        "idle"
    };
    let active_job_count = usize::from(mapping_status == "pending")
        + usize::from(summary_correction_status == "needs_revision");
    AiBackgroundJobStatus {
        mapping_status: mapping_status.to_string(),
        summary_correction_status: summary_correction_status.to_string(),
        active_job_count,
    }
}

#[cfg(test)]
mod tests {
    use std::cell::Cell;

    use super::*;
    use adm_new_application::{
        AiInterviewApplicationService, DesignChecklistItemSpec, DesignNodeSpec,
    };

    #[test]
    fn ai_submit_turn_maps_backend_unavailable() {
        let service = sample_service();
        let mut state = ProjectState::empty();
        let loaded = load_ai_interview(&service, &state);
        assert!(loaded.ok);
        assert_eq!(loaded.data.unwrap().status, "idle");

        let response = submit_ai_turn(
            &service,
            &mut state,
            SubmitAiTurnRequest {
                user_message: "Need questions.".to_string(),
                schema_mode: AiSchemaMode::Turn,
                payload_json: None,
            },
        );
        assert!(!response.ok);
        assert_eq!(response.error.unwrap().code, "BACKEND_UNAVAILABLE");
    }

    #[test]
    fn ai_force_output_calls_real_service_and_serializes_state() {
        let service = sample_service();
        let mut state = ProjectState::empty();
        let response = force_ai_output(
            &service,
            &mut state,
            ForceAiOutputRequest {
                schema_mode: AiSchemaMode::Turn,
                payload: Some(AiResponsePayload {
                    mode: AiResponseMode::QuestionGroup,
                    assistant_message: "What is the main promise?".to_string(),
                    question_group: Some(serde_json::json!({
                        "questions": ["What is the main promise?"]
                    })),
                    ..sample_payload_defaults()
                }),
            },
        );
        assert!(response.ok);
        let view = response.data.unwrap();
        assert_eq!(view.report.mode, AiResponseMode::QuestionGroup);
        assert!(view.state.awaiting_user_answer);
        assert_eq!(view.stream_events[0].stage, "completed");
        assert_eq!(view.background_jobs.mapping_status, "idle");
        let json = serde_json::to_value(view).unwrap();
        assert_eq!(json["state"]["status"], "completed");
        assert_eq!(json["stream_events"][0]["stage"], "completed");
    }

    #[test]
    fn ai_force_output_accepts_null_payload_for_desktop_provider_generation() {
        let request: ForceAiOutputRequest = serde_json::from_value(serde_json::json!({
            "schema_mode": "full_output",
            "payload": null
        }))
        .unwrap();
        assert_eq!(request.schema_mode, AiSchemaMode::FullOutput);
        assert!(request.payload.is_none());

        let service = sample_service();
        let mut state = ProjectState::empty();
        let response = force_ai_output(&service, &mut state, request);
        assert!(!response.ok);
        assert_eq!(response.error.unwrap().code, "BACKEND_UNAVAILABLE");
    }

    #[test]
    fn ai_schema_validation_errors_are_serialized() {
        let service = sample_service();
        let mut state = ProjectState::empty();
        let response = force_ai_output(
            &service,
            &mut state,
            ForceAiOutputRequest {
                schema_mode: AiSchemaMode::Readiness,
                payload: Some(AiResponsePayload {
                    mode: AiResponseMode::FullProjectOutput,
                    assistant_message: "invalid mode".to_string(),
                    ..sample_payload_defaults()
                }),
            },
        );
        assert!(!response.ok);
        assert_eq!(response.error.unwrap().code, "VALIDATION_FAILED");
    }

    #[test]
    fn mark_ai_inaccurate_updates_interview_state_without_file_writes() {
        let service = sample_service();
        let mut state = ProjectState::empty();
        let response = mark_ai_inaccurate(
            &service,
            &mut state,
            MarkAiInaccurateRequest {
                node_id: "mechanics".to_string(),
                reason: "The suggested loop ignores constraints.".to_string(),
            },
        );
        assert!(response.ok);
        let interview = response.data.unwrap();
        assert_eq!(interview.status, "needs_revision");
        assert_eq!(interview.summary.v1.last_user_corrections.len(), 1);
        let background = ai_background_status_for_state(&interview);
        assert_eq!(background.summary_correction_status, "needs_revision");
        assert_eq!(background.active_job_count, 1);
    }

    #[test]
    fn save_ai_archive_updates_manual_archive_state() {
        let service = sample_service();
        let mut state = ProjectState::empty();
        let response = save_ai_archive(
            &service,
            &mut state,
            SaveAiArchiveRequest {
                archive_path: Some("ai_archives/manual/test.json".to_string()),
            },
        );
        assert!(response.ok);
        let interview = response.data.unwrap();
        assert_eq!(
            interview.last_manual_archive_path,
            "ai_archives/manual/test.json"
        );
        assert!(interview.last_archived_at.starts_with("unix:"));
    }

    #[test]
    fn ai_command_wrapper_calls_service_trait_mock() {
        let service = MockAiService {
            force_calls: Cell::new(0),
        };
        let mut state = ProjectState::empty();
        let response = force_ai_output(
            &service,
            &mut state,
            ForceAiOutputRequest {
                schema_mode: AiSchemaMode::Turn,
                payload: Some(AiResponsePayload {
                    mode: AiResponseMode::Maintenance,
                    ..sample_payload_defaults()
                }),
            },
        );
        assert!(response.ok);
        assert_eq!(service.force_calls.get(), 1);
    }

    struct MockAiService {
        force_calls: Cell<usize>,
    }

    impl AiCommandService for MockAiService {
        fn load_ai_interview(&self, state: &ProjectState) -> AdmResult<AiInterviewState> {
            Ok(state.ai_interview.clone())
        }

        fn submit_ai_turn(
            &self,
            _: &mut ProjectState,
            _: &SubmitAiTurnRequest,
        ) -> AdmResult<AiInterviewCommandView> {
            Err(AdmError::new("mock submit not implemented"))
        }

        fn force_ai_output(
            &self,
            state: &mut ProjectState,
            _: &ForceAiOutputRequest,
        ) -> AdmResult<AiInterviewCommandView> {
            self.force_calls.set(self.force_calls.get() + 1);
            Ok(AiInterviewCommandView {
                report: AiTurnReportView {
                    mode: AiResponseMode::Maintenance,
                    applied_project_state: false,
                    archive_path: String::new(),
                    memory_events: Vec::new(),
                    diagnostics: Vec::new(),
                },
                state: state.ai_interview.clone(),
                stream_events: ai_stream_events_for_state(&state.ai_interview),
                background_jobs: ai_background_status_for_state(&state.ai_interview),
            })
        }

        fn mark_ai_inaccurate(
            &self,
            _: &mut ProjectState,
            _: &MarkAiInaccurateRequest,
        ) -> AdmResult<AiInterviewState> {
            Err(AdmError::new("mock mark not implemented"))
        }

        fn save_ai_archive(
            &self,
            state: &mut ProjectState,
            _: &SaveAiArchiveRequest,
        ) -> AdmResult<AiInterviewState> {
            Ok(state.ai_interview.clone())
        }
    }

    fn sample_service() -> AiInterviewApplicationService {
        AiInterviewApplicationService::new(vec![DesignNodeSpec {
            node_id: "mechanics".to_string(),
            domain_id: "core".to_string(),
            name: "Mechanics".to_string(),
            description: String::new(),
            role_class: String::new(),
            checklist: vec![DesignChecklistItemSpec {
                item_id: "core_loop".to_string(),
                label: "Core Loop".to_string(),
                option_groups: Vec::new(),
            }],
        }])
    }

    fn sample_payload_defaults() -> AiResponsePayload {
        serde_json::from_value(serde_json::json!({
            "schemaVersion": "1.0",
            "mode": "maintenance"
        }))
        .unwrap()
    }
}
