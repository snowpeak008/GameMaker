use adm_ai::{AiCapability, AiOutputState, AiOutputValidator, AiProvider, AiTaskRequest};
use adm_design::{
    AiInterviewMessage, DecisionState, DesignEngine, GameDesignBrief, GameplaySystemsState,
    NodeState, OptionGroupState, WorkbenchResultTabs, WorkbenchState, load_design_data_repository,
};
use adm_foundation::{AdmError, AdmResult, UtcTimestamp, read_to_string, write_string};
use serde_json::{Map, Value};
use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkbenchSnapshot {
    pub active_domain_id: String,
    pub active_node_id: String,
    pub project_name: String,
    pub profile_text: String,
    pub ai_interview_text: String,
    pub domains: Vec<WorkbenchDomainRow>,
    pub nodes: Vec<WorkbenchNodeRow>,
    pub selected_node: WorkbenchNodeDetail,
    pub checklist: Vec<WorkbenchChecklistRow>,
    pub l4_options: Vec<WorkbenchL4OptionRow>,
    pub result_tabs: WorkbenchResultTabs,
    pub dirty: bool,
    pub version: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkbenchDomainRow {
    pub id: String,
    pub name: String,
    pub description: String,
    pub node_progress: String,
    pub checklist_progress: String,
    pub l4_progress: String,
    pub focused: bool,
    pub active: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkbenchNodeRow {
    pub id: String,
    pub name: String,
    pub role_class: String,
    pub status: String,
    pub checklist_progress: String,
    pub l4_progress: String,
    pub l5_status: String,
    pub detail: String,
    pub active: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkbenchNodeDetail {
    pub id: String,
    pub name: String,
    pub role_class: String,
    pub status: String,
    pub description: String,
    pub checklist_progress: String,
    pub l4_progress: String,
    pub l5_enabled: bool,
    pub l5_json: String,
    pub l5_errors: String,
    pub design_note: String,
    pub risk_note: String,
    pub not_applicable_reason: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkbenchChecklistRow {
    pub node_id: String,
    pub item_id: String,
    pub label: String,
    pub description: String,
    pub checked: bool,
    pub l4_progress: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkbenchL4OptionRow {
    pub node_id: String,
    pub item_id: String,
    pub item_label: String,
    pub group_id: String,
    pub group_label: String,
    pub option_id: String,
    pub option_label: String,
    pub description: String,
    pub mode: String,
    pub question: String,
    pub required: bool,
    pub allow_primary: bool,
    pub selected: bool,
    pub primary: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkbenchTemplateRow {
    pub id: String,
    pub name: String,
    pub source: String,
    pub target_scale: String,
    pub quality: String,
    pub summary: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkbenchInterviewRunReport {
    pub node_id: String,
    pub provider_id: String,
    pub task_id: String,
    pub output_state: String,
    pub applied: bool,
    pub validation_notes: Vec<String>,
    pub raw_output: String,
}

#[derive(Debug, Clone)]
struct WorkbenchTemplateEntry {
    row: WorkbenchTemplateRow,
    path: std::path::PathBuf,
}

pub struct WorkbenchService {
    engine: DesignEngine,
    state: WorkbenchState,
    active_domain_id: String,
    active_node_id: String,
}

impl WorkbenchService {
    pub fn load(design_data_root: &Path) -> AdmResult<Self> {
        let repository = load_design_data_repository(design_data_root)?;
        let engine = DesignEngine::new(repository)?;
        let active_domain_id = engine.first_domain_id().to_string();
        let active_node_id = first_node_id_in_domain(&engine, &active_domain_id);
        let state = engine.empty_state();
        Ok(Self {
            engine,
            state,
            active_domain_id,
            active_node_id,
        })
    }

    pub fn from_state(design_data_root: &Path, state: WorkbenchState) -> AdmResult<Self> {
        let repository = load_design_data_repository(design_data_root)?;
        let engine = DesignEngine::new(repository)?;
        let active_domain_id = engine.first_domain_id().to_string();
        let active_node_id = first_node_id_in_domain(&engine, &active_domain_id);
        let state = engine.normalize_state(state);
        Ok(Self {
            engine,
            state,
            active_domain_id,
            active_node_id,
        })
    }

    pub fn load_or_autosave(design_data_root: &Path, autosave_path: &Path) -> AdmResult<Self> {
        if !autosave_path.exists() {
            return Self::load(design_data_root);
        }
        let raw = read_to_string(autosave_path)?;
        let state = serde_json::from_str::<WorkbenchState>(&raw).map_err(|error| {
            AdmError::validation(format!(
                "failed to parse workbench autosave {}: {error}",
                autosave_path.display()
            ))
        })?;
        let mut service = Self::from_state(design_data_root, state)?;
        service.state.dirty = false;
        Ok(service)
    }

    pub fn save_autosave(&mut self, autosave_path: &Path) -> AdmResult<()> {
        let raw = serde_json::to_string_pretty(&self.state).map_err(|error| {
            AdmError::validation(format!("failed to serialize workbench autosave: {error}"))
        })?;
        write_string(autosave_path, &raw)?;
        self.state.dirty = false;
        Ok(())
    }

    pub fn export_text(&self, format: &str) -> AdmResult<String> {
        match format.trim().to_ascii_lowercase().as_str() {
            "" | "markdown" | "md" => Ok(self.render_markdown_export()),
            "json" => serde_json::to_string_pretty(&self.state)
                .map_err(|error| AdmError::validation(format!("工作台 JSON 导出失败：{error}"))),
            "text" | "txt" => Ok(self.render_text_export()),
            "prompt" => Ok(self.render_prompt_export()),
            other => Err(AdmError::invalid_input(format!(
                "不支持的设计工作台导出格式：{other}"
            ))),
        }
    }

    pub fn export_to_file(&self, target: &Path, format: &str) -> AdmResult<std::path::PathBuf> {
        let text = self.export_text(format)?;
        write_string(target, &text)?;
        Ok(target.to_path_buf())
    }

    pub fn list_project_templates(
        builtin_root: &Path,
        custom_root: &Path,
    ) -> AdmResult<Vec<WorkbenchTemplateRow>> {
        Ok(template_entries(builtin_root, custom_root)?
            .into_iter()
            .map(|entry| entry.row)
            .collect())
    }

    pub fn import_project_template(
        &mut self,
        builtin_root: &Path,
        custom_root: &Path,
        template_id: &str,
    ) -> AdmResult<WorkbenchTemplateRow> {
        let template_id = template_id.trim();
        if template_id.is_empty() {
            return Err(AdmError::invalid_input("template id cannot be empty"));
        }
        let entry = template_entries(builtin_root, custom_root)?
            .into_iter()
            .find(|entry| entry.row.id == template_id)
            .ok_or_else(|| AdmError::invalid_input(format!("unknown template {template_id}")))?;
        let value =
            serde_json::from_str::<Value>(&read_to_string(&entry.path)?).map_err(|error| {
                AdmError::validation(format!(
                    "failed to parse template {}: {error}",
                    entry.path.display()
                ))
            })?;
        let state = parse_template_state(&value)?;
        self.state = self.engine.normalize_state(state);
        self.active_domain_id = self.engine.first_domain_id().to_string();
        self.active_node_id = first_node_id_in_domain(&self.engine, &self.active_domain_id);
        self.state.dirty = true;
        self.state.version = self.state.version.saturating_add(1);
        Ok(entry.row)
    }

    pub fn save_custom_template(
        &self,
        custom_root: &Path,
        template_name: &str,
    ) -> AdmResult<std::path::PathBuf> {
        let name = template_name.trim();
        if name.is_empty() {
            return Err(AdmError::invalid_input("template name cannot be empty"));
        }
        let id = format!("custom_{}", sanitize_id(name));
        let mut root = Map::new();
        root.insert(
            "schemaVersion".to_string(),
            Value::String("0.1.0".to_string()),
        );
        root.insert(
            "template".to_string(),
            serde_json::json!({
                "id": id,
                "source": "custom",
                "sourceLabel": "自定义模板",
                "name": name,
                "targetScale": self.state.profile.get("targetScale").cloned().unwrap_or_else(|| "unknown".to_string()),
                "qualityClaim": "workbench_snapshot",
                "summary": format!("由 Rust 设计工作台另存：{}", self.state.project_name),
            }),
        );
        let state_value = serde_json::to_value(&self.state).map_err(|error| {
            AdmError::validation(format!("failed to serialize custom template: {error}"))
        })?;
        root.insert("projectState".to_string(), state_value);
        let target = custom_root.join(format!("{id}.json"));
        let text = serde_json::to_string_pretty(&Value::Object(root)).map_err(|error| {
            AdmError::validation(format!("failed to render custom template: {error}"))
        })?;
        write_string(&target, &text)?;
        Ok(target)
    }

    pub fn engine(&self) -> &DesignEngine {
        &self.engine
    }

    pub fn state(&self) -> &WorkbenchState {
        &self.state
    }

    pub fn state_mut(&mut self) -> &mut WorkbenchState {
        &mut self.state
    }

    pub fn pipeline_brief(&self) -> AdmResult<GameDesignBrief> {
        GameDesignBrief::new(
            self.state.project_name.trim(),
            self.pipeline_genre(),
            self.pipeline_player_promise(),
            self.pipeline_core_loop(),
        )
    }

    pub fn active_domain_id(&self) -> &str {
        &self.active_domain_id
    }

    pub fn active_node_id(&self) -> &str {
        &self.active_node_id
    }

    fn pipeline_genre(&self) -> String {
        let mut parts = Vec::new();
        for field_id in [
            "targetScale",
            "primaryPlatform",
            "businessModel",
            "operationModel",
        ] {
            if let Some(label) = self.profile_value_label(field_id) {
                parts.push(label);
            }
        }
        if parts.is_empty() {
            "综合游戏设计".to_string()
        } else {
            parts.join(" / ")
        }
    }

    fn pipeline_player_promise(&self) -> String {
        let mut notes = self
            .state
            .nodes
            .values()
            .filter_map(|node| {
                let note = sanitize_brief_line(&node.design_note);
                (!note.is_empty()).then_some(note)
            })
            .take(2)
            .collect::<Vec<_>>();
        if notes.is_empty() {
            notes = self
                .state
                .gameplay_systems
                .selected
                .iter()
                .filter_map(|system_id| self.gameplay_system_label(system_id))
                .take(2)
                .collect();
        }
        if notes.is_empty() {
            notes.push(format!(
                "玩家围绕{}完成清晰目标、获得即时反馈并持续优化策略",
                sanitize_brief_line(&self.state.project_name)
            ));
        }
        notes.join("；")
    }

    fn pipeline_core_loop(&self) -> Vec<String> {
        let mut steps = self
            .state
            .gameplay_systems
            .core_loops
            .values()
            .map(|value| sanitize_brief_line(value))
            .filter(|value| !value.is_empty())
            .collect::<Vec<_>>();
        if steps.len() < 3 {
            for custom in &self.state.gameplay_systems.custom {
                push_unique(
                    &mut steps,
                    sanitize_brief_line(&format!("{}：{}", custom.name, custom.mapping_desc)),
                );
                if steps.len() >= 3 {
                    break;
                }
            }
        }
        if steps.len() < 3 {
            for system_id in &self.state.gameplay_systems.selected {
                if let Some(label) = self.gameplay_system_label(system_id) {
                    push_unique(&mut steps, format!("围绕{}做出选择并获得反馈", label));
                }
                if steps.len() >= 3 {
                    break;
                }
            }
        }
        if steps.len() < 3 {
            for node in self.engine.nodes() {
                let Some(node_state) = self.state.nodes.get(&node.id) else {
                    continue;
                };
                if matches!(
                    node_state.decision_state,
                    DecisionState::Selected | DecisionState::Completed | DecisionState::Risk
                ) {
                    push_unique(
                        &mut steps,
                        format!("确认{}并转化为玩家可感知反馈", node.name),
                    );
                }
                if steps.len() >= 3 {
                    break;
                }
            }
        }
        for fallback in [
            "理解当前目标与可用资源",
            "执行核心行动并观察系统反馈",
            "根据结果调整策略并进入下一轮挑战",
        ] {
            if steps.len() >= 3 {
                break;
            }
            push_unique(&mut steps, fallback.to_string());
        }
        steps
    }

    fn profile_value_label(&self, field_id: &str) -> Option<String> {
        let value = self.state.profile.get(field_id)?;
        if value == "unknown" || value.trim().is_empty() {
            return None;
        }
        self.engine
            .data()
            .profile_fields
            .iter()
            .find(|field| field.id == field_id)
            .and_then(|field| {
                field
                    .options
                    .iter()
                    .find(|option| option.value == *value)
                    .map(|option| option.label.clone())
            })
            .or_else(|| Some(value.clone()))
    }

    fn gameplay_system_label(&self, system_id: &str) -> Option<String> {
        self.engine
            .data()
            .gameplay_system_options
            .iter()
            .find(|system| system.id == system_id)
            .map(|system| system.name.clone())
    }

    pub fn select_domain(&mut self, domain_id: impl Into<String>) -> AdmResult<()> {
        let domain_id = domain_id.into();
        if !self
            .engine
            .domains()
            .iter()
            .any(|domain| domain.id == domain_id)
        {
            return Err(AdmError::validation(format!("unknown domain {domain_id}")));
        }
        self.active_node_id = first_node_id_in_domain(&self.engine, &domain_id);
        self.active_domain_id = domain_id;
        Ok(())
    }

    pub fn select_node(&mut self, node_id: impl Into<String>) -> AdmResult<()> {
        let node_id = node_id.into();
        let Some(domain_id) = node_domain_id(&self.engine, &node_id) else {
            return Err(AdmError::validation(format!(
                "unknown design node {node_id}"
            )));
        };
        self.active_domain_id = domain_id;
        self.active_node_id = node_id;
        Ok(())
    }

    pub fn set_project_name(&mut self, name: impl Into<String>) {
        self.state.project_name = name.into();
        self.mark_changed();
    }

    pub fn set_profile_value(&mut self, field_id: &str, value: &str) -> AdmResult<()> {
        let field = self
            .engine
            .data()
            .profile_fields
            .iter()
            .find(|field| field.id == field_id)
            .ok_or_else(|| AdmError::validation(format!("unknown profile field {field_id}")))?;
        if !field.options.iter().any(|option| option.value == value) {
            return Err(AdmError::validation(format!(
                "unknown profile value {value} for {field_id}"
            )));
        }
        self.state
            .profile
            .insert(field_id.to_string(), value.to_string());
        self.mark_changed();
        Ok(())
    }

    pub fn set_checklist_item(
        &mut self,
        node_id: &str,
        item_id: &str,
        checked: bool,
    ) -> AdmResult<()> {
        self.select_node(node_id)?;
        self.engine
            .set_checklist_item(&mut self.state, node_id, item_id, checked)
    }

    pub fn set_option_group_option(
        &mut self,
        node_id: &str,
        item_id: &str,
        group_id: &str,
        option_id: &str,
        checked: bool,
    ) -> AdmResult<()> {
        self.select_node(node_id)?;
        self.engine.set_option_group_option(
            &mut self.state,
            node_id,
            item_id,
            group_id,
            option_id,
            checked,
        )
    }

    pub fn set_option_group_primary(
        &mut self,
        node_id: &str,
        item_id: &str,
        group_id: &str,
        option_id: &str,
    ) -> AdmResult<()> {
        self.select_node(node_id)?;
        self.engine
            .set_option_group_primary(&mut self.state, node_id, item_id, group_id, option_id)
    }

    pub fn update_node_text(
        &mut self,
        node_id: &str,
        field_name: &str,
        value: impl Into<String>,
    ) -> AdmResult<()> {
        self.select_node(node_id)?;
        self.engine
            .update_node_text(&mut self.state, node_id, field_name, value)
    }

    pub fn update_node_design_entities_json(
        &mut self,
        node_id: &str,
        raw_json: &str,
    ) -> AdmResult<()> {
        self.select_node(node_id)?;
        self.engine
            .update_node_design_entities_json(&mut self.state, node_id, raw_json)
    }

    pub fn generate_interview_question(&mut self, focus: &str) -> AdmResult<String> {
        let node = node_by_id(&self.engine, &self.active_node_id)
            .ok_or_else(|| AdmError::validation("no active design node for interview"))?;
        let tabs = self.engine.result_tabs(&self.state, &self.active_domain_id);
        let focus = focus.trim();
        let focus_text = if focus.is_empty() {
            first_meaningful_line(&tabs.missing)
                .unwrap_or_else(|| "当前节点的关键设计取舍".to_string())
        } else {
            focus.to_string()
        };
        let question = format!(
            "请补充「{}」：围绕{}说明玩家目标、反馈方式、约束边界和可验证成功标准。",
            node.name, focus_text
        );
        self.state.ai_interview.candidate_node_ids = vec![node.id.clone()];
        self.state.ai_interview.route_overview = vec![
            format!("当前领域：{}", self.active_domain_id),
            format!("当前节点：{}", node.name),
            format!("访谈焦点：{}", focus_text),
        ];
        self.state.ai_interview.prompt_meter.insert(
            "last_question_chars".to_string(),
            question.chars().count() as u64,
        );
        let node_id = self.active_node_id.clone();
        self.push_ai_interview_message("assistant", &question, &node_id);
        self.mark_changed();
        Ok(question)
    }

    pub fn record_interview_reply(&mut self, reply: &str) -> AdmResult<()> {
        let reply = reply.trim();
        if reply.is_empty() {
            return Err(AdmError::invalid_input("AI 访谈回答不能为空"));
        }
        let node_id = self.active_node_id.clone();
        self.push_ai_interview_message("user", reply, &node_id);
        self.state
            .ai_interview
            .prompt_meter
            .insert("last_reply_chars".to_string(), reply.chars().count() as u64);
        self.mark_changed();
        Ok(())
    }

    pub fn run_ai_interview_with_provider(
        &mut self,
        provider: &dyn AiProvider,
        user_reply: &str,
    ) -> AdmResult<WorkbenchInterviewRunReport> {
        if !provider.supports(&AiCapability::TextGeneration) {
            return Err(AdmError::unsupported(format!(
                "AI provider {} does not support text_generation",
                provider.provider_id()
            )));
        }
        if !user_reply.trim().is_empty() {
            self.record_interview_reply(user_reply)?;
        }

        let node_id = self.active_node_id.clone();
        let node_name = node_by_id(&self.engine, &node_id)
            .map(|node| node.name.clone())
            .unwrap_or_else(|| node_id.clone());
        let prompt = self.render_interview_provider_prompt();
        let context_summary = format!(
            "AutoDesignMaker design interview | project={} | domain={} | node={}",
            self.state.project_name, self.active_domain_id, node_name
        );
        let request = AiTaskRequest::new(AiCapability::TextGeneration, prompt, context_summary)?;
        let result = provider.run(&request)?;
        let validated = result.validate(&AiOutputValidator::strict_default());
        let output_state = validated.output_state.as_str().to_string();
        let validation_notes = validated.validation_notes.clone();
        let raw_output = validated.raw_output.trim().to_string();
        let accepted = validated.output_state == AiOutputState::Validated;

        let applied = if accepted {
            let accepted_result = validated.accept()?;
            let existing_note = self
                .state
                .nodes
                .get(&node_id)
                .map(|node| node.design_note.as_str())
                .unwrap_or_default();
            let next_note = merge_ai_design_note(
                existing_note,
                &raw_output,
                accepted_result.provider_id.as_str(),
                accepted_result.task_id.as_str(),
            );
            self.engine
                .update_node_text(&mut self.state, &node_id, "design_note", next_note)?;
            true
        } else {
            false
        };

        self.push_ai_interview_message(
            "assistant",
            if raw_output.is_empty() {
                "AI 没有返回可写入内容。"
            } else {
                &raw_output
            },
            &node_id,
        );
        self.state.ai_interview.replay_records.push(format!(
            "provider={} task={} node={} output_state={} applied={} validation_notes={}",
            provider.provider_id(),
            request.task_id,
            node_id,
            output_state,
            applied,
            validation_notes.join("|")
        ));
        self.state.ai_interview.prompt_meter.insert(
            "last_prompt_chars".to_string(),
            request.prompt.chars().count() as u64,
        );
        self.state.ai_interview.prompt_meter.insert(
            "last_output_chars".to_string(),
            raw_output.chars().count() as u64,
        );
        self.mark_changed();

        Ok(WorkbenchInterviewRunReport {
            node_id,
            provider_id: provider.provider_id().as_str().to_string(),
            task_id: request.task_id.as_str().to_string(),
            output_state,
            applied,
            validation_notes,
            raw_output,
        })
    }

    pub fn reset(&mut self) {
        self.state = self.engine.empty_state();
        self.active_domain_id = self.engine.first_domain_id().to_string();
        self.active_node_id = first_node_id_in_domain(&self.engine, &self.active_domain_id);
    }

    pub fn snapshot(&self) -> WorkbenchSnapshot {
        let result_tabs = self.engine.result_tabs(&self.state, &self.active_domain_id);
        WorkbenchSnapshot {
            active_domain_id: self.active_domain_id.clone(),
            active_node_id: self.active_node_id.clone(),
            project_name: self.state.project_name.clone(),
            profile_text: self.render_profile_text(),
            ai_interview_text: self.render_ai_interview_text(),
            domains: self.domain_rows(),
            nodes: self.node_rows(),
            selected_node: self.selected_node_detail(),
            checklist: self.checklist_rows(),
            l4_options: self.l4_option_rows(),
            result_tabs,
            dirty: self.state.dirty,
            version: self.state.version,
        }
    }

    fn domain_rows(&self) -> Vec<WorkbenchDomainRow> {
        let focus = self.profile_focus_domains();
        self.engine
            .domains()
            .iter()
            .map(|domain| {
                let coverage = self.engine.domain_coverage(&domain.id, &self.state);
                let l4 = self.engine.domain_l4_progress(&domain.id, &self.state);
                WorkbenchDomainRow {
                    id: domain.id.clone(),
                    name: domain.name.clone(),
                    description: domain.description.clone(),
                    node_progress: format!("{} / {}", coverage.done_nodes, coverage.total_nodes),
                    checklist_progress: format!(
                        "{} / {}",
                        coverage.done_checklist, coverage.total_checklist
                    ),
                    l4_progress: format!("{} / {}", l4.done, l4.total),
                    focused: focus.iter().any(|item| item == &domain.id),
                    active: domain.id == self.active_domain_id,
                }
            })
            .collect()
    }

    fn node_rows(&self) -> Vec<WorkbenchNodeRow> {
        self.engine
            .domain_nodes(&self.active_domain_id)
            .into_iter()
            .map(|node| {
                let node_state = self.state.nodes.get(&node.id);
                let progress = self.engine.node_progress(node, &self.state);
                let l4 = self.engine.node_l4_progress(node, &self.state);
                let entity_count = node_state
                    .map(|state| state.design_entities.len())
                    .unwrap_or_default();
                let entity_errors = node_state
                    .map(|state| state.entity_validation_errors.len())
                    .unwrap_or_default();
                WorkbenchNodeRow {
                    id: node.id.clone(),
                    name: node.name.clone(),
                    role_class: node.role_class.clone(),
                    status: self
                        .engine
                        .effective_node_state(node, &self.state)
                        .label()
                        .to_string(),
                    checklist_progress: format!("{} / {}", progress.done, progress.total),
                    l4_progress: format!("{} / {}", l4.done, l4.total),
                    l5_status: if matches!(
                        node.role_class.as_str(),
                        "system_concrete" | "content_concrete"
                    ) {
                        format!("实体 {} / 警告 {}", entity_count, entity_errors)
                    } else {
                        "-".to_string()
                    },
                    detail: node.description.clone(),
                    active: node.id == self.active_node_id,
                }
            })
            .collect()
    }

    fn selected_node_detail(&self) -> WorkbenchNodeDetail {
        let Some(node) = node_by_id(&self.engine, &self.active_node_id) else {
            return WorkbenchNodeDetail::empty();
        };
        let node_state = self.state.nodes.get(&node.id).cloned().unwrap_or_default();
        let progress = self.engine.node_progress(node, &self.state);
        let l4 = self.engine.node_l4_progress(node, &self.state);
        let l5_json = serde_json::to_string_pretty(&node_state.design_entities)
            .unwrap_or_else(|_| "[]".to_string());
        let l5_errors = if node_state.entity_validation_errors.is_empty() {
            "通过".to_string()
        } else {
            node_state
                .entity_validation_errors
                .iter()
                .map(|error| format!("{}：{}", error.path, error.message))
                .collect::<Vec<_>>()
                .join("\n")
        };

        WorkbenchNodeDetail {
            id: node.id.clone(),
            name: node.name.clone(),
            role_class: node.role_class.clone(),
            status: self
                .engine
                .effective_node_state(node, &self.state)
                .label()
                .to_string(),
            description: node.description.clone(),
            checklist_progress: format!("{} / {}", progress.done, progress.total),
            l4_progress: format!("{} / {}", l4.done, l4.total),
            l5_enabled: is_l5_node(node.role_class.as_str()),
            l5_json,
            l5_errors,
            design_note: node_state.design_note,
            risk_note: node_state.risk_note,
            not_applicable_reason: node_state.not_applicable_reason,
        }
    }

    fn checklist_rows(&self) -> Vec<WorkbenchChecklistRow> {
        let Some(node) = node_by_id(&self.engine, &self.active_node_id) else {
            return Vec::new();
        };
        let node_state = self.state.nodes.get(&node.id);
        node.checklist
            .iter()
            .map(|item| {
                let checked = node_state
                    .and_then(|state| state.checklist.get(&item.id))
                    .copied()
                    .unwrap_or(false);
                let l4 = self.engine.item_l4_progress(node, item, &self.state);
                WorkbenchChecklistRow {
                    node_id: node.id.clone(),
                    item_id: item.id.clone(),
                    label: item.label.clone(),
                    description: item.description.clone(),
                    checked,
                    l4_progress: format!("{} / {}", l4.done, l4.total),
                }
            })
            .collect()
    }

    fn l4_option_rows(&self) -> Vec<WorkbenchL4OptionRow> {
        let Some(node) = node_by_id(&self.engine, &self.active_node_id) else {
            return Vec::new();
        };
        let node_state = self.state.nodes.get(&node.id);
        node.checklist
            .iter()
            .flat_map(|item| {
                item.option_groups.iter().flat_map(move |group| {
                    group.options.iter().map(move |option| {
                        let group_state = node_state
                            .and_then(|state| state.checklist_options.get(&item.id))
                            .and_then(|item_options| item_options.get(&group.id));
                        let selected = group_state
                            .map(|state| state.selected.iter().any(|item| item == &option.id))
                            .unwrap_or(false);
                        let primary = group_state
                            .map(|state| state.primary == option.id)
                            .unwrap_or(false);
                        WorkbenchL4OptionRow {
                            node_id: node.id.clone(),
                            item_id: item.id.clone(),
                            item_label: item.label.clone(),
                            group_id: group.id.clone(),
                            group_label: group.label.clone(),
                            option_id: option.id.clone(),
                            option_label: option.label.clone(),
                            description: option.description.clone(),
                            mode: if group.selection_mode == "single" {
                                "单选".to_string()
                            } else {
                                "多选".to_string()
                            },
                            question: group.design_question.clone(),
                            required: group.required,
                            allow_primary: group.allow_primary,
                            selected,
                            primary,
                        }
                    })
                })
            })
            .collect()
    }

    fn render_profile_text(&self) -> String {
        self.engine
            .data()
            .profile_fields
            .iter()
            .map(|field| {
                let value = self
                    .state
                    .profile
                    .get(&field.id)
                    .map(String::as_str)
                    .unwrap_or("unknown");
                let label = field
                    .options
                    .iter()
                    .find(|option| option.value == value)
                    .map(|option| option.label.as_str())
                    .unwrap_or(value);
                format!("{}：{}", field.label, label)
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    fn profile_focus_domains(&self) -> Vec<String> {
        let mut focus = Vec::new();
        if matches!(
            self.state.profile.get("businessModel").map(String::as_str),
            Some("free_to_play" | "subscription" | "premium_with_dlc")
        ) {
            focus.extend([
                "economy_monetization_design",
                "balance_design",
                "data_validation_design",
                "compliance_risk_design",
            ]);
        }
        if self.state.profile.get("operationModel").map(String::as_str) == Some("live_service") {
            focus.extend([
                "retention_lifecycle_design",
                "liveops_version_design",
                "launch_readiness_design",
            ]);
        }
        if matches!(
            self.state.profile.get("socialModel").map(String::as_str),
            Some("multiplayer" | "community_driven")
        ) {
            focus.extend(["social_community_design", "compliance_risk_design"]);
        }
        if matches!(
            self.state.profile.get("regionScope").map(String::as_str),
            Some("multi_region" | "global")
        ) {
            focus.extend(["release_growth_design", "compliance_risk_design"]);
        }
        if matches!(
            self.state.profile.get("targetScale").map(String::as_str),
            Some("3a" | "large_service")
        ) {
            focus.extend([
                "documentation_collaboration_design",
                "data_validation_design",
                "launch_readiness_design",
            ]);
        }
        focus.sort_unstable();
        focus.dedup();
        focus.into_iter().map(ToOwned::to_owned).collect()
    }

    fn mark_changed(&mut self) {
        self.state.version = self.state.version.saturating_add(1);
        self.state.dirty = true;
    }

    fn push_ai_interview_message(&mut self, role: &str, content: &str, node_id: &str) {
        self.state.ai_interview.messages.push(AiInterviewMessage {
            role: role.to_string(),
            content: content.to_string(),
            node_id: node_id.to_string(),
            created_at_millis: UtcTimestamp::now().as_millis(),
        });
    }

    fn render_ai_interview_text(&self) -> String {
        let mut lines = vec![
            "AI 访谈状态：已接入 WorkbenchState、当前节点上下文、provider 调用和设计说明写回。"
                .to_string(),
            format!("当前领域：{}", self.active_domain_id),
            format!("当前节点：{}", self.active_node_id),
        ];
        if !self.state.ai_interview.route_overview.is_empty() {
            lines.push(String::new());
            lines.push("访谈路径：".to_string());
            lines.extend(
                self.state
                    .ai_interview
                    .route_overview
                    .iter()
                    .map(|item| format!("- {item}")),
            );
        }
        if !self.state.ai_interview.messages.is_empty() {
            lines.push(String::new());
            lines.push("最近消息：".to_string());
            let start = self.state.ai_interview.messages.len().saturating_sub(6);
            for message in &self.state.ai_interview.messages[start..] {
                lines.push(format!(
                    "- {} / {}：{}",
                    message.role,
                    message.node_id,
                    one_line(&message.content, 120)
                ));
            }
        }
        if !self.state.ai_interview.replay_records.is_empty() {
            lines.push(String::new());
            lines.push("调用记录：".to_string());
            let start = self
                .state
                .ai_interview
                .replay_records
                .len()
                .saturating_sub(4);
            for record in &self.state.ai_interview.replay_records[start..] {
                lines.push(format!("- {record}"));
            }
        }
        lines.join("\n")
    }

    fn render_interview_provider_prompt(&self) -> String {
        let selected = self.selected_node_detail();
        let tabs = self.engine.result_tabs(&self.state, &self.active_domain_id);
        let recent_messages = self
            .state
            .ai_interview
            .messages
            .iter()
            .rev()
            .take(6)
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .map(|message| format!("{}：{}", message.role, message.content))
            .collect::<Vec<_>>()
            .join("\n");
        format!(
            "你是 AutoDesignMaker 的游戏设计访谈助手。请只用中文输出一段可直接写入当前节点设计说明的建议，避免寒暄，包含：玩家目标、系统反馈、约束边界、验收标准。\n\n项目：{}\n领域：{}\n节点：{} ({})\n节点说明：{}\n当前设计说明：{}\n当前风险：{}\n不适用原因：{}\n\n最近访谈：\n{}\n\n缺失项：\n{}\n\n风险：\n{}\n\n校验：\n{}",
            self.state.project_name,
            self.active_domain_id,
            selected.name,
            selected.role_class,
            selected.description,
            selected.design_note,
            selected.risk_note,
            selected.not_applicable_reason,
            if recent_messages.trim().is_empty() {
                "暂无"
            } else {
                recent_messages.as_str()
            },
            tabs.missing,
            tabs.risk,
            tabs.validation
        )
    }

    fn render_markdown_export(&self) -> String {
        let tabs = self.engine.result_tabs(&self.state, &self.active_domain_id);
        let mut lines = vec![
            format!("# {}", self.state.project_name),
            String::new(),
            "## 项目画像".to_string(),
        ];
        for field in &self.engine.data().profile_fields {
            let value = self
                .state
                .profile
                .get(&field.id)
                .map(String::as_str)
                .unwrap_or("unknown");
            let label = field
                .options
                .iter()
                .find(|option| option.value == value)
                .map(|option| option.label.as_str())
                .unwrap_or(value);
            lines.push(format!("- {}：{}", field.label, label));
        }
        lines.extend([String::new(), "## 领域与节点".to_string(), String::new()]);
        for domain in self.engine.domains() {
            let coverage = self.engine.domain_coverage(&domain.id, &self.state);
            let l4 = self.engine.domain_l4_progress(&domain.id, &self.state);
            lines.push(format!(
                "### {}（节点 {}%，决策项 {}%，L4 {}/{}）",
                domain.name, coverage.node_percent, coverage.checklist_percent, l4.done, l4.total
            ));
            if !domain.description.trim().is_empty() {
                lines.push(domain.description.clone());
            }
            for node in &domain.nodes {
                let node_state = self.state.nodes.get(&node.id).cloned().unwrap_or_default();
                lines.push(format!(
                    "- **{}**：{}；决策项 {}；L4 {}",
                    node.name,
                    self.engine.effective_node_state(node, &self.state).label(),
                    self.engine.node_progress(node, &self.state).percent,
                    self.engine.node_l4_progress(node, &self.state).percent
                ));
                if !node_state.design_note.trim().is_empty() {
                    lines.push(format!("  - 设计说明：{}", node_state.design_note));
                }
                if !node_state.risk_note.trim().is_empty() {
                    lines.push(format!("  - 风险说明：{}", node_state.risk_note));
                }
                for item in &node.checklist {
                    let checked = node_state.checklist.get(&item.id).copied().unwrap_or(false);
                    lines.push(format!(
                        "  - [{}] {}",
                        if checked { "x" } else { " " },
                        item.label
                    ));
                    if let Some(item_options) = node_state.checklist_options.get(&item.id) {
                        for group in &item.option_groups {
                            if let Some(group_state) = item_options.get(&group.id) {
                                if !group_state.selected.is_empty() {
                                    let selected = group_state
                                        .selected
                                        .iter()
                                        .map(|option_id| {
                                            group
                                                .options
                                                .iter()
                                                .find(|option| &option.id == option_id)
                                                .map(|option| option.label.as_str())
                                                .unwrap_or(option_id.as_str())
                                        })
                                        .collect::<Vec<_>>()
                                        .join(", ");
                                    lines.push(format!("    - {}：{}", group.label, selected));
                                }
                            }
                        }
                    }
                }
                if !node_state.design_entities.is_empty() {
                    lines.push(format!(
                        "  - L5 实体数量：{}；校验问题：{}",
                        node_state.design_entities.len(),
                        node_state.entity_validation_errors.len()
                    ));
                }
            }
            lines.push(String::new());
        }
        lines.extend([
            "## 摘要".to_string(),
            tabs.summary,
            String::new(),
            "## 缺失项".to_string(),
            tabs.missing,
            String::new(),
            "## 风险".to_string(),
            tabs.risk,
            String::new(),
            "## 校验".to_string(),
            tabs.validation,
        ]);
        lines.join("\n")
    }

    fn render_text_export(&self) -> String {
        self.render_markdown_export()
            .replace('#', "")
            .replace("**", "")
            .replace("- [x]", "[完成]")
            .replace("- [ ]", "[待定]")
    }

    fn render_prompt_export(&self) -> String {
        format!(
            "请基于以下 AutoDesignMaker Rust 设计工作台状态继续生成结构化游戏设计文档。\n\n{}",
            self.render_markdown_export()
        )
    }
}

impl WorkbenchNodeDetail {
    fn empty() -> Self {
        Self {
            id: String::new(),
            name: "未选择节点".to_string(),
            role_class: String::new(),
            status: "未选择".to_string(),
            description: "请先在当前领域中选择一个设计节点。".to_string(),
            checklist_progress: "0 / 0".to_string(),
            l4_progress: "0 / 0".to_string(),
            l5_enabled: false,
            l5_json: "[]".to_string(),
            l5_errors: "未选择节点。".to_string(),
            design_note: String::new(),
            risk_note: String::new(),
            not_applicable_reason: String::new(),
        }
    }
}

fn first_node_id_in_domain(engine: &DesignEngine, domain_id: &str) -> String {
    engine
        .domain_nodes(domain_id)
        .first()
        .map(|node| node.id.clone())
        .unwrap_or_default()
}

fn node_by_id<'a>(engine: &'a DesignEngine, node_id: &str) -> Option<&'a adm_design::DesignNode> {
    engine.nodes().find(|node| node.id == node_id)
}

fn node_domain_id(engine: &DesignEngine, node_id: &str) -> Option<String> {
    node_by_id(engine, node_id).map(|node| node.domain_id.clone())
}

fn is_l5_node(role_class: &str) -> bool {
    matches!(role_class, "system_concrete" | "content_concrete")
}

fn first_meaningful_line(text: &str) -> Option<String> {
    text.lines()
        .map(str::trim)
        .find(|line| {
            !line.is_empty()
                && !matches!(
                    *line,
                    "暂无缺失项。" | "暂无风险节点。" | "数据校验：" | "L5 实体校验："
                )
        })
        .map(|line| line.trim_start_matches("- ").to_string())
}

fn one_line(text: &str, limit: usize) -> String {
    let normalized = text.split_whitespace().collect::<Vec<_>>().join(" ");
    if normalized.chars().count() <= limit {
        return normalized;
    }
    normalized.chars().take(limit).collect::<String>()
}

fn merge_ai_design_note(
    existing: &str,
    suggestion: &str,
    provider_id: &str,
    task_id: &str,
) -> String {
    let suggestion = suggestion.trim();
    if existing.trim().is_empty() {
        return format!("AI 建议（provider={provider_id}, task={task_id}）：{suggestion}");
    }
    format!(
        "{}\n\nAI 建议（provider={}, task={}）：{}",
        existing.trim(),
        provider_id,
        task_id,
        suggestion
    )
}

fn template_entries(
    builtin_root: &Path,
    custom_root: &Path,
) -> AdmResult<Vec<WorkbenchTemplateEntry>> {
    let mut entries = Vec::new();
    entries.extend(builtin_template_entries(builtin_root)?);
    entries.extend(custom_template_entries(custom_root)?);
    entries.sort_by(|left, right| {
        left.row
            .source
            .cmp(&right.row.source)
            .then(left.row.target_scale.cmp(&right.row.target_scale))
            .then(left.row.name.cmp(&right.row.name))
    });
    Ok(entries)
}

fn builtin_template_entries(builtin_root: &Path) -> AdmResult<Vec<WorkbenchTemplateEntry>> {
    let index_path = builtin_root.join("template_index.json");
    if !index_path.exists() {
        return Ok(Vec::new());
    }
    let value = serde_json::from_str::<Value>(&read_to_string(&index_path)?).map_err(|error| {
        AdmError::validation(format!(
            "failed to parse template index {}: {error}",
            index_path.display()
        ))
    })?;
    let mut entries = Vec::new();
    for item in value
        .get("templates")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
    {
        if string_value(item, &["visibility"]).as_deref() != Some("public") {
            continue;
        }
        let Some(id) = string_value(item, &["id"]) else {
            continue;
        };
        let file_name =
            string_value(item, &["fileName", "file_name"]).unwrap_or_else(|| format!("{id}.json"));
        entries.push(WorkbenchTemplateEntry {
            row: WorkbenchTemplateRow {
                id: id.clone(),
                name: string_value(item, &["name"]).unwrap_or_else(|| id.clone()),
                source: "builtin".to_string(),
                target_scale: string_value(item, &["scaleLabel", "targetScale"])
                    .unwrap_or_else(|| "unknown".to_string()),
                quality: string_value(item, &["qualityClaim", "qualityTier"]).unwrap_or_default(),
                summary: string_value(item, &["dimension"]).unwrap_or_default(),
            },
            path: builtin_root.join(file_name),
        });
    }
    Ok(entries)
}

fn custom_template_entries(custom_root: &Path) -> AdmResult<Vec<WorkbenchTemplateEntry>> {
    if !custom_root.exists() {
        return Ok(Vec::new());
    }
    let mut entries = Vec::new();
    for entry in fs::read_dir(custom_root).map_err(|error| {
        AdmError::invalid_input(format!("failed to read {}: {error}", custom_root.display()))
    })? {
        let path = entry
            .map_err(|error| AdmError::invalid_input(format!("failed to read template: {error}")))?
            .path();
        if path.extension().and_then(|item| item.to_str()) != Some("json") {
            continue;
        }
        let value = serde_json::from_str::<Value>(&read_to_string(&path)?).map_err(|error| {
            AdmError::validation(format!(
                "failed to parse template {}: {error}",
                path.display()
            ))
        })?;
        let template = value.get("template").unwrap_or(&Value::Null);
        let id = string_value(template, &["id"]).unwrap_or_else(|| {
            path.file_stem()
                .and_then(|item| item.to_str())
                .unwrap_or("custom_template")
                .to_string()
        });
        entries.push(WorkbenchTemplateEntry {
            row: WorkbenchTemplateRow {
                id: id.clone(),
                name: string_value(template, &["name"]).unwrap_or_else(|| id.clone()),
                source: "custom".to_string(),
                target_scale: string_value(template, &["targetScale", "target_scale"])
                    .unwrap_or_else(|| "custom".to_string()),
                quality: string_value(template, &["qualityClaim", "quality_claim"])
                    .unwrap_or_else(|| "workbench_snapshot".to_string()),
                summary: string_value(template, &["summary"]).unwrap_or_default(),
            },
            path,
        });
    }
    Ok(entries)
}

fn parse_template_state(value: &Value) -> AdmResult<WorkbenchState> {
    let project_state = value
        .get("projectState")
        .or_else(|| value.get("project_state"))
        .ok_or_else(|| AdmError::validation("template is missing projectState".to_string()))?;
    let mut state = WorkbenchState {
        project_name: string_value(project_state, &["projectName", "project_name"])
            .unwrap_or_else(|| "未命名游戏设计项目".to_string()),
        profile: string_map(project_state.get("profile")),
        nodes: BTreeMap::new(),
        gameplay_systems: GameplaySystemsState::default(),
        ai_interview: Default::default(),
        version: number_value(project_state, &["version"]).unwrap_or_default(),
        dirty: true,
    };
    if let Some(nodes) = project_state
        .get("nodes")
        .and_then(Value::as_object)
        .or_else(|| project_state.get("nodeStates").and_then(Value::as_object))
    {
        for (node_id, node_value) in nodes {
            state
                .nodes
                .insert(node_id.clone(), parse_template_node_state(node_value));
        }
    }
    if let Some(gameplay) = project_state
        .get("gameplaySystems")
        .or_else(|| project_state.get("gameplay_systems"))
    {
        state.gameplay_systems.selected = string_array(gameplay.get("selected"));
        state.gameplay_systems.core_loops = string_map(
            gameplay
                .get("coreLoops")
                .or_else(|| gameplay.get("core_loops")),
        );
    }
    Ok(state)
}

fn parse_template_node_state(value: &Value) -> NodeState {
    let mut state = NodeState {
        decision_state: string_value(value, &["decisionState", "decision_state"])
            .map(|item| DecisionState::from_legacy(&item))
            .unwrap_or_default(),
        design_note: string_value(value, &["designNote", "design_note"]).unwrap_or_default(),
        risk_note: string_value(value, &["riskNote", "risk_note"]).unwrap_or_default(),
        not_applicable_reason: string_value(
            value,
            &["notApplicableReason", "not_applicable_reason"],
        )
        .unwrap_or_default(),
        design_entities: value
            .get("designEntities")
            .or_else(|| value.get("design_entities"))
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default(),
        entity_validation_errors: Vec::new(),
        checklist: bool_map(value.get("checklist")),
        checklist_options: BTreeMap::new(),
        option_provenance: BTreeMap::new(),
    };
    if let Some(checklist_options) = value
        .get("checklistOptions")
        .or_else(|| value.get("checklist_options"))
        .and_then(Value::as_object)
    {
        for (item_id, item_options) in checklist_options {
            let mut groups = BTreeMap::new();
            if let Some(item_options) = item_options.as_object() {
                for (group_id, group_value) in item_options {
                    groups.insert(
                        group_id.clone(),
                        OptionGroupState {
                            selected: string_array(group_value.get("selected")),
                            primary: string_value(group_value, &["primary"]).unwrap_or_default(),
                        },
                    );
                }
            }
            state.checklist_options.insert(item_id.clone(), groups);
        }
    }
    state
}

fn string_value(value: &Value, keys: &[&str]) -> Option<String> {
    keys.iter()
        .find_map(|key| value.get(*key))
        .and_then(Value::as_str)
        .map(ToOwned::to_owned)
}

fn number_value(value: &Value, keys: &[&str]) -> Option<u64> {
    keys.iter()
        .find_map(|key| value.get(*key))
        .and_then(Value::as_u64)
}

fn string_array(value: Option<&Value>) -> Vec<String> {
    value
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .map(ToOwned::to_owned)
        .collect()
}

fn string_map(value: Option<&Value>) -> BTreeMap<String, String> {
    value
        .and_then(Value::as_object)
        .into_iter()
        .flat_map(|object| object.iter())
        .filter_map(|(key, value)| value.as_str().map(|value| (key.clone(), value.to_string())))
        .collect()
}

fn bool_map(value: Option<&Value>) -> BTreeMap<String, bool> {
    value
        .and_then(Value::as_object)
        .into_iter()
        .flat_map(|object| object.iter())
        .filter_map(|(key, value)| value.as_bool().map(|value| (key.clone(), value)))
        .collect()
}

fn sanitize_id(value: &str) -> String {
    let mut output = value
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() {
                ch.to_ascii_lowercase()
            } else {
                '_'
            }
        })
        .collect::<String>();
    while output.contains("__") {
        output = output.replace("__", "_");
    }
    output = output.trim_matches('_').to_string();
    if output.is_empty() {
        "template".to_string()
    } else {
        output
    }
}

fn sanitize_brief_line(value: &str) -> String {
    value
        .replace(['\r', '\n', '\t'], " ")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .trim()
        .to_string()
}

fn push_unique(items: &mut Vec<String>, value: String) {
    let value = sanitize_brief_line(&value);
    if value.is_empty() || items.iter().any(|item| item == &value) {
        return;
    }
    items.push(value);
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn service_snapshot_reflects_l4_mutation() {
        let mut service = WorkbenchService::load(&fixture_design_data_root()).expect("service");
        let (node_id, item_id, group_id, option_id) = first_l4_selection(&service);

        service
            .set_option_group_option(&node_id, &item_id, &group_id, &option_id, true)
            .expect("select");

        let snapshot = service.snapshot();
        assert!(snapshot.dirty);
        assert!(snapshot.version > 0);
        assert!(snapshot.result_tabs.summary.contains("全项目节点覆盖率"));
        assert!(
            snapshot
                .nodes
                .iter()
                .any(|node| node.id == node_id && node.status != "未选择")
        );
    }

    #[test]
    fn service_validates_profile_values() {
        let mut service = WorkbenchService::load(&fixture_design_data_root()).expect("service");

        service
            .set_profile_value("targetScale", "indie")
            .expect("valid profile value");
        let error = service
            .set_profile_value("targetScale", "not_a_scale")
            .expect_err("invalid profile value");

        assert!(error.to_string().contains("unknown profile value"));
        assert!(service.snapshot().profile_text.contains("独立游戏"));
    }

    #[test]
    fn service_builds_pipeline_brief_from_workbench_state() {
        let mut service = WorkbenchService::load(&fixture_design_data_root()).expect("service");
        let node_id = service.engine().nodes().next().expect("node").id.clone();
        service.set_project_name("工作台导出项目");
        service
            .set_profile_value("targetScale", "indie")
            .expect("scale");
        service
            .set_profile_value("primaryPlatform", "pc_console")
            .expect("platform");
        service
            .update_node_text(&node_id, "design_note", "玩家通过短局策略形成稳定成长目标")
            .expect("note");
        service
            .state_mut()
            .gameplay_systems
            .core_loops
            .insert("loop_01".to_string(), "观察局势并选择行动".to_string());

        let brief = service.pipeline_brief().expect("brief");

        assert_eq!(brief.title, "工作台导出项目");
        assert!(brief.genre.contains("独立游戏"));
        assert!(brief.genre.contains("PC / 主机"));
        assert!(brief.player_promise.contains("短局策略"));
        assert!(brief.core_loop.contains(&"观察局势并选择行动".to_string()));
        assert!(brief.core_loop.len() >= 3);
    }

    #[test]
    fn service_snapshot_exposes_selected_node_editor_rows() {
        let mut service = WorkbenchService::load(&fixture_design_data_root()).expect("service");
        let (node_id, item_id, group_id, option_id) = first_l4_selection(&service);

        service.select_node(&node_id).expect("select node");
        let initial = service.snapshot();
        assert_eq!(initial.active_node_id, node_id);
        assert!(!initial.checklist.is_empty());
        assert!(!initial.l4_options.is_empty());
        assert_eq!(initial.selected_node.l5_json, "[]");

        service
            .set_checklist_item(&node_id, &item_id, true)
            .expect("check item");
        service
            .set_option_group_option(&node_id, &item_id, &group_id, &option_id, true)
            .expect("select l4");
        service
            .update_node_text(&node_id, "design_note", "核心节点说明")
            .expect("note");
        service
            .update_node_design_entities_json(
                &node_id,
                r#"[{"schema":"system_card_v1","kind":"system","schemaVersion":"v1","id":"movement"}]"#,
            )
            .expect("l5");

        let updated = service.snapshot();
        assert!(updated.dirty);
        assert_eq!(updated.selected_node.design_note, "核心节点说明");
        assert!(updated.selected_node.l5_json.contains("movement"));
        assert!(
            updated
                .checklist
                .iter()
                .any(|item| item.item_id == item_id && item.checked)
        );
        assert!(
            updated
                .l4_options
                .iter()
                .any(|option| option.option_id == option_id && option.selected)
        );
        assert!(
            updated
                .result_tabs
                .validation
                .contains("missing required field")
        );
    }

    #[test]
    fn service_saves_and_restores_workbench_autosave() {
        let root = std::env::temp_dir().join(format!(
            "adm_workbench_autosave_{}_{}",
            std::process::id(),
            adm_foundation::SessionId::generate()
        ));
        let autosave = root.join("design_workbench").join("workbench_state.json");
        let mut service = WorkbenchService::load(&fixture_design_data_root()).expect("service");
        let (node_id, item_id, _, _) = first_l4_selection(&service);

        service.set_project_name("自动保存项目");
        service
            .set_checklist_item(&node_id, &item_id, true)
            .expect("check item");
        service
            .update_node_text(&node_id, "design_note", "自动保存说明")
            .expect("note");
        service.save_autosave(&autosave).expect("save autosave");

        assert!(autosave.exists());
        assert!(!service.snapshot().dirty);

        let restored = WorkbenchService::load_or_autosave(&fixture_design_data_root(), &autosave)
            .expect("restore autosave");
        let snapshot = restored.snapshot();
        assert_eq!(snapshot.project_name, "自动保存项目");
        assert!(!snapshot.dirty);
        assert!(
            restored
                .state()
                .nodes
                .get(&node_id)
                .and_then(|node| node.checklist.get(&item_id))
                .copied()
                .unwrap_or(false)
        );
        assert_eq!(
            restored
                .state()
                .nodes
                .get(&node_id)
                .map(|node| node.design_note.as_str()),
            Some("自动保存说明")
        );

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn service_exports_workbench_markdown_and_json() {
        let mut service = WorkbenchService::load(&fixture_design_data_root()).expect("service");
        let (node_id, item_id, _, _) = first_l4_selection(&service);
        service.set_project_name("导出验证项目");
        service
            .set_checklist_item(&node_id, &item_id, true)
            .expect("check item");

        let markdown = service.export_text("markdown").expect("markdown");
        let json = service.export_text("json").expect("json");

        assert!(markdown.contains("# 导出验证项目"));
        assert!(markdown.contains("## 项目画像"));
        assert!(markdown.contains("## 校验"));
        assert!(json.contains("导出验证项目"));
        assert!(service.export_text("unsupported").is_err());
    }

    #[test]
    fn service_runs_ai_interview_provider_and_writes_design_note() {
        let mut service = WorkbenchService::load(&fixture_design_data_root()).expect("service");
        let provider = adm_ai::MockAiProvider::new(
            adm_foundation::ProviderId::new("mock").expect("provider id"),
            vec![adm_ai::AiCapability::TextGeneration],
        );

        let question = service
            .generate_interview_question("玩家目标与反馈")
            .expect("question");
        let report = service
            .run_ai_interview_with_provider(&provider, "玩家需要看清挑战结果。")
            .expect("provider run");
        let snapshot = service.snapshot();

        assert!(question.contains("玩家目标与反馈"));
        assert_eq!(report.provider_id, "mock");
        assert!(report.applied);
        assert_eq!(report.output_state, "validated");
        assert!(snapshot.selected_node.design_note.contains("AI 建议"));
        assert!(snapshot.ai_interview_text.contains("调用记录"));
        assert!(
            service
                .state()
                .ai_interview
                .replay_records
                .iter()
                .any(|record| record.contains("applied=true"))
        );
    }

    #[test]
    fn service_lists_and_imports_builtin_project_template() {
        let mut service = WorkbenchService::load(&fixture_design_data_root()).expect("service");
        let custom_root = std::env::temp_dir().join(format!(
            "adm_workbench_template_empty_{}_{}",
            std::process::id(),
            adm_foundation::SessionId::generate()
        ));
        let templates = WorkbenchService::list_project_templates(
            &fixture_project_templates_root(),
            &custom_root,
        )
        .expect("templates");
        let template = templates
            .iter()
            .find(|template| template.source == "builtin")
            .expect("builtin template")
            .clone();

        let imported = service
            .import_project_template(
                &fixture_project_templates_root(),
                &custom_root,
                &template.id,
            )
            .expect("import");

        let snapshot = service.snapshot();
        assert_eq!(imported.id, template.id);
        assert!(snapshot.project_name.starts_with("范本："));
        assert!(snapshot.dirty);
        assert!(
            service
                .state()
                .nodes
                .values()
                .any(|node| node.decision_state != DecisionState::NotStarted)
        );

        let _ = std::fs::remove_dir_all(custom_root);
    }

    #[test]
    fn service_saves_custom_project_template() {
        let custom_root = std::env::temp_dir().join(format!(
            "adm_workbench_custom_template_{}_{}",
            std::process::id(),
            adm_foundation::SessionId::generate()
        ));
        let mut service = WorkbenchService::load(&fixture_design_data_root()).expect("service");
        service.set_project_name("自定义模板项目");

        let path = service
            .save_custom_template(&custom_root, "自定义模板项目")
            .expect("save custom template");
        let templates = WorkbenchService::list_project_templates(
            &fixture_project_templates_root(),
            &custom_root,
        )
        .expect("templates");

        assert!(path.exists());
        assert!(
            templates
                .iter()
                .any(|template| template.source == "custom" && template.name == "自定义模板项目")
        );

        let _ = std::fs::remove_dir_all(custom_root);
    }

    fn fixture_design_data_root() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join("..")
            .join("..")
            .join("knowledge")
            .join("design_data")
    }

    fn fixture_project_templates_root() -> PathBuf {
        fixture_design_data_root().join("project_templates")
    }

    fn first_l4_selection(service: &WorkbenchService) -> (String, String, String, String) {
        for node in service.engine.nodes() {
            for item in &node.checklist {
                for group in &item.option_groups {
                    if let Some(option) = group.options.first() {
                        return (
                            node.id.clone(),
                            item.id.clone(),
                            group.id.clone(),
                            option.id.clone(),
                        );
                    }
                }
            }
        }
        panic!("expected L4 option group");
    }
}
