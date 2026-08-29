#![forbid(unsafe_code)]

use adm_foundation::TaskId;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AssetTask {
    pub task_id: TaskId,
    pub pipeline_stage: String,
    pub source_mechanic: String,
    pub asset_kind: String,
    pub description: String,
    pub dependencies: Vec<String>,
    pub validation_steps: Vec<String>,
    pub risk_controls: Vec<String>,
    pub acceptance: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AssetPlan {
    pub tasks: Vec<AssetTask>,
}

impl AssetPlan {
    pub fn for_genre(genre: &str) -> Self {
        let mut tasks = base_asset_tasks(genre);
        tasks.push(AssetTask {
            task_id: TaskId::generate(),
            pipeline_stage: "feedback".to_string(),
            source_mechanic: "all_core_mechanics".to_string(),
            asset_kind: "interaction_feedback".to_string(),
            description: "Core interaction feedback asset list".to_string(),
            dependencies: vec![
                "design/project.adm".to_string(),
                "development/plan.adm".to_string(),
            ],
            validation_steps: vec![
                "Map one feedback asset to every core loop step".to_string(),
                "Confirm visual or audio response for each key action".to_string(),
            ],
            risk_controls: vec!["feedback_unclear".to_string()],
            acceptance: "Every key action has visual or audio feedback definition".to_string(),
        });
        Self { tasks }
    }

    pub fn for_core_loop(genre: &str, core_loop: &[String]) -> Self {
        let mut tasks = base_asset_tasks(genre);
        tasks.extend(core_loop.iter().enumerate().map(|(index, step)| {
            let step_number = index + 1;
            AssetTask {
                task_id: TaskId::generate(),
                pipeline_stage: "mechanic_feedback".to_string(),
                source_mechanic: format!("Core Loop Mechanic {step_number}"),
                asset_kind: "interaction_feedback".to_string(),
                description: format!("Feedback, readability, and state cues for: {step}"),
                dependencies: vec![
                    "design/project.adm".to_string(),
                    "development/plan.adm".to_string(),
                ],
                validation_steps: vec![
                    format!("Confirm feedback is visible or audible for: {step}"),
                    format!("Confirm state transition can be recognized after: {step}"),
                    format!("Confirm cue is covered by scenario_core_loop_step_{step_number}"),
                ],
                risk_controls: vec!["feedback_unclear".to_string(), "scope_drift".to_string()],
                acceptance: format!(
                    "Core Loop Mechanic {step_number} has inspectable feedback assets and validation cues"
                ),
            }
        }));
        Self { tasks }
    }

    pub fn render(&self) -> String {
        let mut document = String::from("# Asset Plan\n");
        for task in &self.tasks {
            document.push_str(&format!(
                "- task_id={}; stage={}; source_mechanic={}; kind={}; description={}; dependencies={}; validation={}; risk_controls={}; acceptance={}\n",
                task.task_id,
                task.pipeline_stage,
                task.source_mechanic,
                task.asset_kind,
                task.description,
                task.dependencies.join(" | "),
                task.validation_steps.join(" | "),
                task.risk_controls.join(" | "),
                task.acceptance
            ));
        }
        document
    }
}

fn base_asset_tasks(genre: &str) -> Vec<AssetTask> {
    vec![
        AssetTask {
            task_id: TaskId::generate(),
            pipeline_stage: "concept".to_string(),
            source_mechanic: "design_pillars".to_string(),
            asset_kind: "visual_style".to_string(),
            description: format!("{genre} visual style guide"),
            dependencies: vec!["design/project.adm".to_string()],
            validation_steps: vec![
                "Compare against genre and player promise".to_string(),
                "Confirm reusable palette, silhouettes, and UI tone".to_string(),
            ],
            risk_controls: vec!["scope_drift".to_string()],
            acceptance: "Style, color, proportions, and UI tone are reusable".to_string(),
        },
        AssetTask {
            task_id: TaskId::generate(),
            pipeline_stage: "ui".to_string(),
            source_mechanic: "state_visibility".to_string(),
            asset_kind: "workbench_ui".to_string(),
            description: "Core gameplay HUD and state indicator assets".to_string(),
            dependencies: vec!["development/plan.adm".to_string()],
            validation_steps: vec![
                "Confirm state information is visible during the loop".to_string(),
                "Confirm readable hierarchy for repeated play sessions".to_string(),
            ],
            risk_controls: vec!["feedback_unclear".to_string()],
            acceptance: "Key state, goal, and feedback information is readable".to_string(),
        },
        AssetTask {
            task_id: TaskId::generate(),
            pipeline_stage: "audio".to_string(),
            source_mechanic: "feedback_mechanics".to_string(),
            asset_kind: "audio_cues".to_string(),
            description: "Input confirmation, reward, and failure audio cues".to_string(),
            dependencies: vec!["assets/plan.adm".to_string()],
            validation_steps: vec![
                "Confirm each cue has trigger condition".to_string(),
                "Confirm volume and repetition rules are documented".to_string(),
            ],
            risk_controls: vec!["feedback_unclear".to_string()],
            acceptance: "Audio feedback complements visual feedback without disrupting control"
                .to_string(),
        },
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn asset_plan_renders_pipeline_dependencies_and_validation() {
        let plan = AssetPlan::for_genre("Action");
        let rendered = plan.render();

        assert_eq!(plan.tasks.len(), 4);
        assert!(rendered.contains("stage=concept"));
        assert!(rendered.contains("source_mechanic=design_pillars"));
        assert!(rendered.contains("source_mechanic=all_core_mechanics"));
        assert!(rendered.contains("dependencies=design/project.adm"));
        assert!(rendered.contains("validation=Compare against genre"));
        assert!(rendered.contains("risk_controls=feedback_unclear"));
        assert!(rendered.contains("kind=audio_cues"));
    }

    #[test]
    fn asset_plan_maps_each_core_loop_mechanic_to_feedback_task() {
        let plan = AssetPlan::for_core_loop(
            "Action",
            &[
                "Explore".to_string(),
                "Fight".to_string(),
                "Collect feedback".to_string(),
            ],
        );
        let rendered = plan.render();

        assert_eq!(plan.tasks.len(), 6);
        assert!(rendered.contains("stage=mechanic_feedback"));
        assert!(rendered.contains("source_mechanic=Core Loop Mechanic 1"));
        assert!(rendered.contains("source_mechanic=Core Loop Mechanic 3"));
        assert!(rendered.contains("scenario_core_loop_step_1"));
        assert!(rendered.contains("scenario_core_loop_step_3"));
        assert!(rendered.contains("Core Loop Mechanic 3 has inspectable feedback assets"));
    }
}
