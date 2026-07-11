use adm_new_application::{
    DesignAutosaveReport, DesignTemplateDeleteReport, DesignTemplateListReport,
    DesignTemplateSaveReport, DesignTemplateSelectionReport, DesignWorkbenchService,
    DesignWorkbenchView, GameplaySystemUpdateRequest,
};
use adm_new_contracts::{ArtifactLocale, project::ProjectState};
use adm_new_foundation::AdmResult;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::{
    CommandAdapterResult, command_error, command_error_from_adm, command_failure, command_success,
    handle_command,
};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DesignNodeUpdateRequest {
    pub node_id: String,
    #[serde(default)]
    pub design_note: Option<String>,
    #[serde(default)]
    pub risk_note: Option<String>,
    #[serde(default)]
    pub not_applicable_reason: Option<String>,
    #[serde(default)]
    pub checklist: Vec<ChecklistItemUpdate>,
    #[serde(default)]
    pub option_updates: Vec<OptionGroupSelectionUpdate>,
    #[serde(default)]
    pub primary_updates: Vec<OptionGroupPrimaryUpdate>,
    #[serde(default)]
    pub design_entities: Option<Vec<Value>>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChecklistItemUpdate {
    pub item_id: String,
    pub checked: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OptionGroupSelectionUpdate {
    pub item_id: String,
    pub group_id: String,
    pub option_id: String,
    pub selected: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OptionGroupPrimaryUpdate {
    pub item_id: String,
    pub group_id: String,
    pub option_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExportDesignRequest {
    pub format: String,
    #[serde(default)]
    pub artifact_locale: ArtifactLocale,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AutosaveDesignRequest {
    #[serde(default)]
    pub autosave_file: Option<String>,
    #[serde(default)]
    pub dirty: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TemplateSelectionRequest {
    pub template_id: String,
    #[serde(default)]
    pub project_name_prefix: String,
    #[serde(default)]
    pub project_state: Option<Value>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct ListTemplatesRequest {
    #[serde(default)]
    pub include_internal: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SaveTemplateRequest {
    pub template_name: String,
    pub target_scale: String,
    #[serde(default)]
    pub overwrite: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeleteTemplateRequest {
    pub template_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DesignExport {
    pub format: String,
    pub content: String,
}

pub trait DesignCommandService {
    fn load_design_workbench(&self, state: &ProjectState) -> AdmResult<DesignWorkbenchView>;

    fn update_node(
        &self,
        state: &mut ProjectState,
        request: &DesignNodeUpdateRequest,
    ) -> AdmResult<DesignWorkbenchView>;

    fn export_design(
        &self,
        state: &ProjectState,
        request: &ExportDesignRequest,
    ) -> AdmResult<DesignExport>;

    fn autosave_design(
        &self,
        state: &ProjectState,
        request: &AutosaveDesignRequest,
    ) -> AdmResult<DesignAutosaveReport>;

    fn list_templates(&self, request: &ListTemplatesRequest)
    -> AdmResult<DesignTemplateListReport>;

    fn select_template(
        &self,
        state: &mut ProjectState,
        request: &TemplateSelectionRequest,
    ) -> AdmResult<DesignTemplateSelectionReport>;

    fn save_template(
        &self,
        state: &ProjectState,
        request: &SaveTemplateRequest,
    ) -> AdmResult<DesignTemplateSaveReport>;

    fn delete_template(
        &self,
        request: &DeleteTemplateRequest,
    ) -> AdmResult<DesignTemplateDeleteReport>;

    fn update_gameplay_system(
        &self,
        state: &mut ProjectState,
        request: &GameplaySystemUpdateRequest,
    ) -> AdmResult<DesignWorkbenchView>;

    fn reset_design(&self, state: &mut ProjectState) -> AdmResult<DesignWorkbenchView>;
}

impl DesignCommandService for DesignWorkbenchService {
    fn load_design_workbench(&self, state: &ProjectState) -> AdmResult<DesignWorkbenchView> {
        Ok(self.view_model(state))
    }

    fn update_node(
        &self,
        state: &mut ProjectState,
        request: &DesignNodeUpdateRequest,
    ) -> AdmResult<DesignWorkbenchView> {
        let mut view = self.view_model(state);
        if request.design_note.is_some()
            || request.risk_note.is_some()
            || request.not_applicable_reason.is_some()
        {
            view = self.update_node_text(
                state,
                &request.node_id,
                request.design_note.as_deref(),
                request.risk_note.as_deref(),
                request.not_applicable_reason.as_deref(),
            )?;
        }
        if let Some(entities) = &request.design_entities {
            view = self.replace_design_entities(state, &request.node_id, entities.clone())?;
        }
        for item in &request.checklist {
            view = self.set_checklist_item(state, &request.node_id, &item.item_id, item.checked)?;
        }
        for update in &request.option_updates {
            view = self.set_option_group_option(
                state,
                &request.node_id,
                &update.item_id,
                &update.group_id,
                &update.option_id,
                update.selected,
            )?;
        }
        for update in &request.primary_updates {
            view = self.set_option_group_primary(
                state,
                &request.node_id,
                &update.item_id,
                &update.group_id,
                &update.option_id,
            )?;
        }
        Ok(view)
    }

    fn export_design(
        &self,
        state: &ProjectState,
        request: &ExportDesignRequest,
    ) -> AdmResult<DesignExport> {
        Ok(DesignExport {
            format: request.format.clone(),
            content: DesignWorkbenchService::export_design_with_locale(
                self,
                state,
                &request.format,
                request.artifact_locale,
            )?,
        })
    }

    fn autosave_design(
        &self,
        state: &ProjectState,
        request: &AutosaveDesignRequest,
    ) -> AdmResult<DesignAutosaveReport> {
        self.autosave_state_summary(state, request.autosave_file.as_deref(), request.dirty)
    }

    fn list_templates(
        &self,
        request: &ListTemplatesRequest,
    ) -> AdmResult<DesignTemplateListReport> {
        self.list_project_templates(request.include_internal)
    }

    fn select_template(
        &self,
        state: &mut ProjectState,
        request: &TemplateSelectionRequest,
    ) -> AdmResult<DesignTemplateSelectionReport> {
        self.apply_project_template(state, &request.template_id, &request.project_name_prefix)
    }

    fn save_template(
        &self,
        state: &ProjectState,
        request: &SaveTemplateRequest,
    ) -> AdmResult<DesignTemplateSaveReport> {
        self.save_project_template(
            state,
            &request.template_name,
            &request.target_scale,
            request.overwrite,
        )
    }

    fn delete_template(
        &self,
        request: &DeleteTemplateRequest,
    ) -> AdmResult<DesignTemplateDeleteReport> {
        self.delete_project_template(&request.template_id)
    }

    fn update_gameplay_system(
        &self,
        state: &mut ProjectState,
        request: &GameplaySystemUpdateRequest,
    ) -> AdmResult<DesignWorkbenchView> {
        DesignWorkbenchService::update_gameplay_system(self, state, request)
    }

    fn reset_design(&self, state: &mut ProjectState) -> AdmResult<DesignWorkbenchView> {
        Ok(self.reset_project_state(state))
    }
}

pub fn load_design_workbench<S>(
    service: &S,
    state: &ProjectState,
) -> CommandAdapterResult<DesignWorkbenchView>
where
    S: DesignCommandService,
{
    handle_command(|| service.load_design_workbench(state))
}

pub fn update_node<S>(
    service: &S,
    state: &mut ProjectState,
    request: DesignNodeUpdateRequest,
) -> CommandAdapterResult<DesignWorkbenchView>
where
    S: DesignCommandService,
{
    handle_command(|| service.update_node(state, &request))
}

pub fn export_design<S>(
    service: &S,
    state: &ProjectState,
    request: ExportDesignRequest,
) -> CommandAdapterResult<DesignExport>
where
    S: DesignCommandService,
{
    handle_command(|| service.export_design(state, &request))
}

pub fn autosave_design<S>(
    service: &S,
    state: &ProjectState,
    request: AutosaveDesignRequest,
) -> CommandAdapterResult<DesignAutosaveReport>
where
    S: DesignCommandService,
{
    handle_command(|| service.autosave_design(state, &request))
}

pub fn list_templates<S>(
    service: &S,
    request: ListTemplatesRequest,
) -> CommandAdapterResult<DesignTemplateListReport>
where
    S: DesignCommandService,
{
    handle_template_command(|| service.list_templates(&request))
}

pub fn select_template<S>(
    service: &S,
    state: &mut ProjectState,
    request: TemplateSelectionRequest,
) -> CommandAdapterResult<DesignTemplateSelectionReport>
where
    S: DesignCommandService,
{
    handle_template_command(|| service.select_template(state, &request))
}

pub fn save_template<S>(
    service: &S,
    state: &ProjectState,
    request: SaveTemplateRequest,
) -> CommandAdapterResult<DesignTemplateSaveReport>
where
    S: DesignCommandService,
{
    handle_template_command(|| service.save_template(state, &request))
}

pub fn delete_template<S>(
    service: &S,
    request: DeleteTemplateRequest,
) -> CommandAdapterResult<DesignTemplateDeleteReport>
where
    S: DesignCommandService,
{
    handle_template_command(|| service.delete_template(&request))
}

pub fn update_gameplay_system<S>(
    service: &S,
    state: &mut ProjectState,
    request: GameplaySystemUpdateRequest,
) -> CommandAdapterResult<DesignWorkbenchView>
where
    S: DesignCommandService,
{
    handle_command(|| service.update_gameplay_system(state, &request))
}

pub fn reset_design<S>(
    service: &S,
    state: &mut ProjectState,
) -> CommandAdapterResult<DesignWorkbenchView>
where
    S: DesignCommandService,
{
    handle_command(|| service.reset_design(state))
}

fn handle_template_command<T, F>(handler: F) -> CommandAdapterResult<T>
where
    F: FnOnce() -> AdmResult<T>,
{
    match handler() {
        Ok(data) => command_success(data),
        Err(error) => {
            let lower = error.message().to_ascii_lowercase();
            let code = if lower.contains("project template not found") {
                Some("TEMPLATE_NOT_FOUND")
            } else if lower.contains("cannot replace builtin project template") {
                Some("TEMPLATE_BUILTIN_CONFLICT")
            } else if lower.contains("custom project template already exists")
                || lower.contains("custom project template file already exists")
            {
                Some("TEMPLATE_ALREADY_EXISTS")
            } else if lower.contains("cannot delete builtin project template") {
                Some("TEMPLATE_DELETE_FORBIDDEN")
            } else {
                None
            };
            match code {
                Some(code) => command_failure(command_error(code, error.to_string())),
                None => command_failure(command_error_from_adm(error)),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::cell::Cell;
    use std::fs;
    use std::path::PathBuf;

    use super::*;
    use adm_new_application::{
        DesignChecklistItemSpec, DesignDataLoader, DesignNodeSpec, DesignOptionGroupSpec,
        DesignWorkbenchService,
    };
    use adm_new_foundation::{AdmError, AdmResult, new_stable_id};

    #[test]
    fn export_design_request_defaults_artifact_locale_and_accepts_explicit_locale() {
        let legacy: ExportDesignRequest =
            serde_json::from_value(serde_json::json!({ "format": "markdown" })).unwrap();
        assert_eq!(legacy.artifact_locale, ArtifactLocale::ZhCn);

        let english: ExportDesignRequest = serde_json::from_value(serde_json::json!({
            "format": "markdown",
            "artifact_locale": "en-US"
        }))
        .unwrap();
        assert_eq!(english.artifact_locale, ArtifactLocale::EnUs);
    }

    #[test]
    fn design_commands_call_real_service_and_serialize_view() {
        let service = sample_service();
        let mut state = service.empty_project_state();
        let response = load_design_workbench(&service, &state);
        assert!(response.ok);
        assert_eq!(response.data.as_ref().unwrap().nodes.len(), 1);

        let response = update_node(
            &service,
            &mut state,
            DesignNodeUpdateRequest {
                node_id: "combat_loop".to_string(),
                design_note: Some("Readable tactical exchanges.".to_string()),
                risk_note: None,
                not_applicable_reason: None,
                checklist: Vec::new(),
                option_updates: vec![OptionGroupSelectionUpdate {
                    item_id: "core_loop".to_string(),
                    group_id: "loop_type".to_string(),
                    option_id: "turn_based".to_string(),
                    selected: true,
                }],
                primary_updates: vec![OptionGroupPrimaryUpdate {
                    item_id: "core_loop".to_string(),
                    group_id: "loop_type".to_string(),
                    option_id: "turn_based".to_string(),
                }],
                design_entities: Some(vec![serde_json::json!({"kind": "loop"})]),
            },
        );
        assert!(response.ok);
        let view = response.data.unwrap();
        assert_eq!(view.nodes[0].progress.percent, 100);
        assert_eq!(view.nodes[0].l5_entity_count, 1);
        let json = serde_json::to_value(view).unwrap();
        assert_eq!(json["nodes"][0]["node_id"], "combat_loop");
    }

    #[test]
    fn design_command_errors_are_mapped_by_adapter() {
        let service = sample_service();
        let mut state = service.empty_project_state();
        let response = update_node(
            &service,
            &mut state,
            DesignNodeUpdateRequest {
                node_id: "missing".to_string(),
                design_note: Some("No node.".to_string()),
                risk_note: None,
                not_applicable_reason: None,
                checklist: Vec::new(),
                option_updates: Vec::new(),
                primary_updates: Vec::new(),
                design_entities: None,
            },
        );
        assert!(!response.ok);
        assert_eq!(response.error.unwrap().code, "NOT_FOUND");
    }

    #[test]
    fn export_design_uses_service_output() {
        let service = sample_service();
        let state = service.empty_project_state();
        let response = export_design(
            &service,
            &state,
            ExportDesignRequest {
                format: "markdown".to_string(),
                artifact_locale: ArtifactLocale::default(),
            },
        );
        assert!(response.ok);
        let export = response.data.unwrap();
        assert_eq!(export.format, "markdown");
        assert!(export.content.contains("Combat Loop"));
    }

    #[test]
    fn design_workbench_commands_cover_templates_autosave_gameplay_and_reset() {
        let root = temp_root("adapter_templates");
        let loader =
            DesignDataLoader::new(&root).with_runtime_root(root.join("drafts").join("current"));
        fs::create_dir_all(loader.project_templates_dir()).unwrap();
        fs::write(
            loader
                .project_templates_dir()
                .join("builtin_indie_Builtin.json"),
            serde_json::to_string_pretty(&serde_json::json!({
                "template": {
                    "id": "builtin_indie_Builtin",
                    "name": "Builtin",
                    "targetScale": "indie"
                },
                "projectState": {"projectName": "Builtin"}
            }))
            .unwrap(),
        )
        .unwrap();
        let service = sample_service().with_template_loader(loader);
        let mut state = service.empty_project_state();
        state.project_name = "Demo".to_string();

        let autosave = autosave_design(
            &service,
            &state,
            AutosaveDesignRequest {
                autosave_file: Some("drafts/current/autosave_state.json".to_string()),
                dirty: true,
            },
        );
        assert!(autosave.ok);
        let autosave = autosave.data.unwrap();
        assert!(autosave.dirty);
        assert_eq!(autosave.project_name, "Demo");
        assert!(autosave.state_hash.starts_with("fnv64:"));

        let gameplay = update_gameplay_system(
            &service,
            &mut state,
            GameplaySystemUpdateRequest {
                system_id: "combat".to_string(),
                selected: Some(true),
                weight: Some(serde_json::json!(55)),
                core_loop: Some("read intent and resolve tactical exchange".to_string()),
                custom_name: None,
                delete_custom: false,
                interview_answers: vec!["prioritize tactical readability".to_string()],
            },
        );
        assert!(gameplay.ok);
        assert_eq!(state.gameplay_systems.selected, vec!["combat".to_string()]);
        assert_eq!(
            state.gameplay_systems.core_loops["combat"],
            "read intent and resolve tactical exchange"
        );

        let listed = list_templates(
            &service,
            ListTemplatesRequest {
                include_internal: true,
            },
        );
        assert!(listed.ok);
        assert_eq!(listed.data.unwrap().templates.len(), 1);

        let builtin_conflict = save_template(
            &service,
            &state,
            SaveTemplateRequest {
                template_name: "Builtin".to_string(),
                target_scale: "indie".to_string(),
                overwrite: true,
            },
        );
        assert_eq!(
            builtin_conflict.error.unwrap().code,
            "TEMPLATE_BUILTIN_CONFLICT"
        );

        let save = save_template(
            &service,
            &state,
            SaveTemplateRequest {
                template_name: "Tactical Demo".to_string(),
                target_scale: "indie".to_string(),
                overwrite: false,
            },
        );
        assert!(save.ok);
        let save = save.data.unwrap();
        assert_eq!(save.template_id, "custom_indie_Tactical_Demo");
        assert!(save.target_file_name.ends_with(".json"));

        let duplicate = save_template(
            &service,
            &state,
            SaveTemplateRequest {
                template_name: "Tactical Demo".to_string(),
                target_scale: "indie".to_string(),
                overwrite: false,
            },
        );
        assert_eq!(duplicate.error.unwrap().code, "TEMPLATE_ALREADY_EXISTS");

        let collision_owner = save_template(
            &service,
            &state,
            SaveTemplateRequest {
                template_name: "A B".to_string(),
                target_scale: "midcore".to_string(),
                overwrite: false,
            },
        )
        .data
        .unwrap();
        let filename_collision = save_template(
            &service,
            &state,
            SaveTemplateRequest {
                template_name: "A:B".to_string(),
                target_scale: "midcore".to_string(),
                overwrite: true,
            },
        );
        assert_eq!(
            filename_collision.error.unwrap().code,
            "TEMPLATE_ALREADY_EXISTS"
        );
        assert!(
            delete_template(
                &service,
                DeleteTemplateRequest {
                    template_id: collision_owner.template_id,
                },
            )
            .ok
        );

        let selected = select_template(
            &service,
            &mut state,
            TemplateSelectionRequest {
                template_id: save.template_id.clone(),
                project_name_prefix: "Template: ".to_string(),
                project_state: Some(serde_json::json!({
                    "projectName": "Forged Client State"
                })),
            },
        );
        assert!(selected.ok);
        assert_eq!(state.project_name, "Template: Tactical Demo");

        let missing = select_template(
            &service,
            &mut state,
            TemplateSelectionRequest {
                template_id: "missing_template".to_string(),
                project_name_prefix: "范本：".to_string(),
                project_state: None,
            },
        );
        assert_eq!(missing.error.unwrap().code, "TEMPLATE_NOT_FOUND");

        let builtin_delete = delete_template(
            &service,
            DeleteTemplateRequest {
                template_id: "builtin_indie_Builtin".to_string(),
            },
        );
        assert_eq!(
            builtin_delete.error.unwrap().code,
            "TEMPLATE_DELETE_FORBIDDEN"
        );
        let deleted = delete_template(
            &service,
            DeleteTemplateRequest {
                template_id: save.template_id,
            },
        );
        assert!(deleted.ok);

        let reset = reset_design(&service, &mut state);
        assert!(reset.ok);
        assert_eq!(state.project_name, "未命名游戏设计项目");
        cleanup(root);
    }

    #[test]
    fn template_requests_accept_legacy_state_and_default_new_fields() {
        let select: TemplateSelectionRequest = serde_json::from_value(serde_json::json!({
            "template_id": "legacy",
            "project_state": {"projectName": "ignored"}
        }))
        .unwrap();
        assert_eq!(select.template_id, "legacy");
        assert!(select.project_name_prefix.is_empty());
        assert!(select.project_state.is_some());

        let save: SaveTemplateRequest = serde_json::from_value(serde_json::json!({
            "template_name": "Legacy",
            "target_scale": "indie"
        }))
        .unwrap();
        assert!(!save.overwrite);
    }

    #[test]
    fn design_command_wrapper_calls_service_trait_mock() {
        let service = MockDesignService {
            load_calls: Cell::new(0),
        };
        let state = ProjectState::empty();
        let response = load_design_workbench(&service, &state);
        assert!(response.ok);
        assert_eq!(service.load_calls.get(), 1);
    }

    struct MockDesignService {
        load_calls: Cell<usize>,
    }

    impl DesignCommandService for MockDesignService {
        fn load_design_workbench(&self, state: &ProjectState) -> AdmResult<DesignWorkbenchView> {
            self.load_calls.set(self.load_calls.get() + 1);
            Ok(sample_service().view_model(state))
        }

        fn update_node(
            &self,
            _: &mut ProjectState,
            _: &DesignNodeUpdateRequest,
        ) -> AdmResult<DesignWorkbenchView> {
            Err(AdmError::new("mock update not implemented"))
        }

        fn export_design(
            &self,
            _: &ProjectState,
            _: &ExportDesignRequest,
        ) -> AdmResult<DesignExport> {
            Err(AdmError::new("mock export not implemented"))
        }

        fn autosave_design(
            &self,
            _: &ProjectState,
            _: &AutosaveDesignRequest,
        ) -> AdmResult<DesignAutosaveReport> {
            Err(AdmError::new("mock autosave not implemented"))
        }

        fn list_templates(&self, _: &ListTemplatesRequest) -> AdmResult<DesignTemplateListReport> {
            Err(AdmError::new("mock list templates not implemented"))
        }

        fn select_template(
            &self,
            _: &mut ProjectState,
            _: &TemplateSelectionRequest,
        ) -> AdmResult<DesignTemplateSelectionReport> {
            Err(AdmError::new("mock template not implemented"))
        }

        fn save_template(
            &self,
            _: &ProjectState,
            _: &SaveTemplateRequest,
        ) -> AdmResult<DesignTemplateSaveReport> {
            Err(AdmError::new("mock save template not implemented"))
        }

        fn delete_template(
            &self,
            _: &DeleteTemplateRequest,
        ) -> AdmResult<DesignTemplateDeleteReport> {
            Err(AdmError::new("mock delete template not implemented"))
        }

        fn update_gameplay_system(
            &self,
            _: &mut ProjectState,
            _: &GameplaySystemUpdateRequest,
        ) -> AdmResult<DesignWorkbenchView> {
            Err(AdmError::new("mock gameplay not implemented"))
        }

        fn reset_design(&self, _: &mut ProjectState) -> AdmResult<DesignWorkbenchView> {
            Err(AdmError::new("mock reset not implemented"))
        }
    }

    fn sample_service() -> DesignWorkbenchService {
        DesignWorkbenchService::new(vec![DesignNodeSpec {
            node_id: "combat_loop".to_string(),
            domain_id: "mechanics".to_string(),
            name: "Combat Loop".to_string(),
            description: "Define combat.".to_string(),
            role_class: "system_concrete".to_string(),
            checklist: vec![DesignChecklistItemSpec {
                item_id: "core_loop".to_string(),
                label: "Core Loop".to_string(),
                option_groups: vec![DesignOptionGroupSpec {
                    group_id: "loop_type".to_string(),
                    selection_mode: "single".to_string(),
                    allow_primary: true,
                    options: vec!["turn_based".to_string(), "real_time".to_string()],
                }],
            }],
        }])
    }

    fn temp_root(label: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "adm-newrust-design-command-{label}-{}",
            new_stable_id("test").unwrap()
        ))
    }

    fn cleanup(root: PathBuf) {
        let _ = fs::remove_dir_all(root);
    }
}
