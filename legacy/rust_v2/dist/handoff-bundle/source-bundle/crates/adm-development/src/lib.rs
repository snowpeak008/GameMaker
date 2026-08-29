#![forbid(unsafe_code)]

use adm_foundation::TaskId;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DevelopmentTask {
    pub task_id: TaskId,
    pub milestone: String,
    pub scenario_id: String,
    pub source_mechanic: String,
    pub title: String,
    pub target_engine: Option<String>,
    pub implementation_layer: String,
    pub data_contracts: Vec<String>,
    pub implementation_notes: Vec<String>,
    pub validation_steps: Vec<String>,
    pub test_cases: Vec<String>,
    pub telemetry_events: Vec<String>,
    pub risk_controls: Vec<String>,
    pub acceptance: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DevelopmentPlan {
    pub tasks: Vec<DevelopmentTask>,
}

impl DevelopmentPlan {
    pub fn for_core_loop(core_loop: &[String], target_engine: Option<String>) -> Self {
        let tasks = core_loop
            .iter()
            .enumerate()
            .map(|(index, step)| {
                let step_number = index + 1;
                DevelopmentTask {
                    task_id: TaskId::generate(),
                    milestone: format!("core_loop_step_{step_number}"),
                    scenario_id: format!("scenario_core_loop_step_{step_number}"),
                    source_mechanic: format!("Core Loop Mechanic {step_number}"),
                    title: format!("Implement core loop step {step_number}: {step}"),
                    target_engine: target_engine.clone(),
                    implementation_layer: implementation_layer_for_index(index).to_string(),
                    data_contracts: vec![
                        format!("core_loop_step_{step_number}.request"),
                        format!("core_loop_step_{step_number}.state_delta"),
                        format!("core_loop_step_{step_number}.feedback_event"),
                    ],
                    implementation_notes: vec![
                        "Define input command and state transition".to_string(),
                        "Write event log entry for replayable validation".to_string(),
                        format!("Connect feedback surface for: {step}"),
                    ],
                    validation_steps: vec![
                        "Run unit-level state transition check".to_string(),
                        "Run playable smoke check for input, state, feedback".to_string(),
                    ],
                    test_cases: vec![
                        format!("test_core_loop_step_{step_number}_state_delta"),
                        format!("test_core_loop_step_{step_number}_feedback_event"),
                        format!("test_scenario_core_loop_step_{step_number}_path"),
                    ],
                    telemetry_events: vec![
                        format!("core_loop_step_{step_number}_started"),
                        format!("core_loop_step_{step_number}_completed"),
                    ],
                    risk_controls: vec!["scope_drift".to_string(), "feedback_unclear".to_string()],
                    acceptance:
                        "Input, state transition, feedback, tests, and telemetry are traceable"
                            .to_string(),
                }
            })
            .collect();
        Self { tasks }
    }

    pub fn render(&self) -> String {
        let mut document = String::from("# Development Plan\n");
        for task in &self.tasks {
            document.push_str(&format!(
                "- task_id={}; milestone={}; scenario_id={}; source_mechanic={}; title={}; target_engine={}; layer={}; data_contracts={}; notes={}; validation={}; tests={}; telemetry={}; risk_controls={}; acceptance={}\n",
                task.task_id,
                task.milestone,
                task.scenario_id,
                task.source_mechanic,
                task.title,
                task.target_engine.as_deref().unwrap_or("unspecified"),
                task.implementation_layer,
                task.data_contracts.join(" | "),
                task.implementation_notes.join(" | "),
                task.validation_steps.join(" | "),
                task.test_cases.join(" | "),
                task.telemetry_events.join(" | "),
                task.risk_controls.join(" | "),
                task.acceptance
            ));
        }
        document
    }
}

fn implementation_layer_for_index(index: usize) -> &'static str {
    match index {
        0 => "input_and_navigation",
        1 => "simulation_and_rules",
        _ => "feedback_rewards_and_progression",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn development_plan_renders_implementation_and_validation_details() {
        let plan = DevelopmentPlan::for_core_loop(
            &[
                "Explore".to_string(),
                "Fight".to_string(),
                "Collect feedback".to_string(),
            ],
            Some("Unity".to_string()),
        );

        let rendered = plan.render();

        assert_eq!(plan.tasks.len(), 3);
        assert!(rendered.contains("milestone=core_loop_step_1"));
        assert!(rendered.contains("scenario_id=scenario_core_loop_step_1"));
        assert!(rendered.contains("scenario_id=scenario_core_loop_step_3"));
        assert!(rendered.contains("source_mechanic=Core Loop Mechanic 1"));
        assert!(rendered.contains("layer=input_and_navigation"));
        assert!(rendered.contains("data_contracts=core_loop_step_1.request"));
        assert!(rendered.contains("notes=Define input command"));
        assert!(rendered.contains("validation=Run unit-level state transition check"));
        assert!(rendered.contains("tests=test_core_loop_step_1_state_delta"));
        assert!(rendered.contains("test_scenario_core_loop_step_1_path"));
        assert!(rendered.contains("telemetry=core_loop_step_1_started"));
        assert!(rendered.contains("risk_controls=scope_drift | feedback_unclear"));
    }
}
