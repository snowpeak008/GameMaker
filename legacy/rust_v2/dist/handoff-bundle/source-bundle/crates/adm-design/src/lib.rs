#![forbid(unsafe_code)]

mod data_repository;
mod design_engine;
mod workbench_reference;
mod workbench_state;

use adm_foundation::{AdmError, AdmResult, ProjectId};
pub use data_repository::{
    ChecklistItem, DesignDataRepository, DesignDomain, DesignNode, EntitySchema,
    GameplaySystemOption, OptionGroup, OptionItem, OptionRef, OptionRelation, ProfileField,
    ProfileOption, default_profile_fields, load_design_data_repository,
};
pub use design_engine::{
    CoverageSummary, DesignEngine, L4ProgressSummary, OptionConflict, ProgressSummary,
    WorkbenchResultTabs,
};
pub use workbench_reference::{
    WorkbenchDomainOverview, WorkbenchNodeOverview, WorkbenchOptionGroupOverview,
    WorkbenchReference, load_workbench_reference,
};
pub use workbench_state::{
    AiInterviewMessage, AiInterviewState, CustomGameplaySystem, DecisionState,
    EntityValidationError, GameplayInterviewState, GameplaySystemWeight, GameplaySystemsState,
    NodeState, OptionGroupState, OptionProvenance, WorkbenchState,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DesignProject {
    pub project_id: ProjectId,
    pub working_title: String,
    pub genre: String,
    pub player_promise: String,
    pub core_loop: Vec<String>,
    pub design_pillars: Vec<DesignPillar>,
    pub gameplay_mechanics: Vec<GameplayMechanic>,
    pub playable_scenarios: Vec<PlayableScenario>,
    pub acceptance_risks: Vec<DesignRisk>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DesignPillar {
    pub name: String,
    pub rationale: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GameplayMechanic {
    pub name: String,
    pub player_action: String,
    pub feedback: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlayableScenario {
    pub scenario_id: String,
    pub entry_condition: String,
    pub player_goal: String,
    pub critical_path: Vec<String>,
    pub success_state: String,
    pub failure_state: String,
    pub validation_probe: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DesignRisk {
    pub risk: String,
    pub mitigation: String,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DesignQualityScore {
    pub value: f32,
}

impl DesignQualityScore {
    pub fn is_insufficient(&self, threshold: f32) -> bool {
        self.value < threshold
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GameDesignBrief {
    pub title: String,
    pub genre: String,
    pub player_promise: String,
    pub core_loop: Vec<String>,
}

impl GameDesignBrief {
    pub fn new(
        title: impl Into<String>,
        genre: impl Into<String>,
        player_promise: impl Into<String>,
        core_loop: Vec<String>,
    ) -> AdmResult<Self> {
        let brief = Self {
            title: title.into(),
            genre: genre.into(),
            player_promise: player_promise.into(),
            core_loop,
        };
        brief.validate()?;
        Ok(brief)
    }

    pub fn validate(&self) -> AdmResult<()> {
        if self.title.trim().is_empty() {
            return Err(AdmError::validation("design title cannot be empty"));
        }
        if self.genre.trim().is_empty() {
            return Err(AdmError::validation("design genre cannot be empty"));
        }
        if self.player_promise.trim().is_empty() {
            return Err(AdmError::validation("player promise cannot be empty"));
        }
        if self.core_loop.is_empty() {
            return Err(AdmError::validation("core loop cannot be empty"));
        }
        Ok(())
    }

    pub fn to_project(&self) -> DesignProject {
        DesignProject {
            project_id: ProjectId::generate(),
            working_title: self.title.clone(),
            genre: self.genre.clone(),
            player_promise: self.player_promise.clone(),
            core_loop: self.core_loop.clone(),
            design_pillars: derive_design_pillars(&self.genre, &self.player_promise),
            gameplay_mechanics: derive_gameplay_mechanics(&self.core_loop),
            playable_scenarios: derive_playable_scenarios(&self.core_loop),
            acceptance_risks: derive_design_risks(&self.core_loop),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct DesignEvaluation {
    pub score: DesignQualityScore,
    pub missing_topics: Vec<String>,
    pub requires_ai_intervention: bool,
}

pub fn evaluate_design(project: &DesignProject) -> DesignEvaluation {
    let mut score = 0.35_f32;
    let mut missing_topics = Vec::new();

    if !project.genre.trim().is_empty() {
        score += 0.15;
    } else {
        missing_topics.push("genre".to_string());
    }

    if project.player_promise.chars().count() >= 12 {
        score += 0.2;
    } else {
        missing_topics.push("player_promise_depth".to_string());
    }

    if project.core_loop.len() >= 3 {
        score += 0.2;
    } else {
        missing_topics.push("core_loop_steps".to_string());
    }

    if project
        .core_loop
        .iter()
        .any(|step| step.contains("反馈") || step.to_ascii_lowercase().contains("feedback"))
    {
        score += 0.1;
    } else {
        missing_topics.push("feedback_loop".to_string());
    }

    let score = DesignQualityScore {
        value: score.min(1.0),
    };
    DesignEvaluation {
        requires_ai_intervention: score.is_insufficient(0.75),
        score,
        missing_topics,
    }
}

pub fn render_design_document(project: &DesignProject, evaluation: &DesignEvaluation) -> String {
    let mut document = String::new();
    document.push_str("# Design Project\n");
    document.push_str(&format!("project_id={}\n", project.project_id));
    document.push_str(&format!("title={}\n", project.working_title));
    document.push_str(&format!("genre={}\n", project.genre));
    document.push_str(&format!("player_promise={}\n", project.player_promise));
    document.push_str(&format!("quality_score={:.2}\n", evaluation.score.value));
    document.push_str(&format!(
        "requires_ai_intervention={}\n",
        evaluation.requires_ai_intervention
    ));
    document.push_str("\n## Core Loop\n");
    for (index, step) in project.core_loop.iter().enumerate() {
        document.push_str(&format!("{}. {}\n", index + 1, step));
    }
    document.push_str("\n## Design Pillars\n");
    for pillar in &project.design_pillars {
        document.push_str(&format!(
            "- name={}; rationale={}\n",
            pillar.name, pillar.rationale
        ));
    }
    document.push_str("\n## Gameplay Mechanics\n");
    for mechanic in &project.gameplay_mechanics {
        document.push_str(&format!(
            "- name={}; player_action={}; feedback={}\n",
            mechanic.name, mechanic.player_action, mechanic.feedback
        ));
    }
    document.push_str("\n## Playable Scenarios\n");
    for scenario in &project.playable_scenarios {
        document.push_str(&format!(
            "- scenario_id={}; entry={}; goal={}; critical_path={}; success={}; failure={}; validation_probe={}\n",
            scenario.scenario_id,
            scenario.entry_condition,
            scenario.player_goal,
            scenario.critical_path.join(" | "),
            scenario.success_state,
            scenario.failure_state,
            scenario.validation_probe
        ));
    }
    document.push_str("\n## Acceptance Risks\n");
    for risk in &project.acceptance_risks {
        document.push_str(&format!(
            "- risk={}; mitigation={}\n",
            risk.risk, risk.mitigation
        ));
    }
    if !evaluation.missing_topics.is_empty() {
        document.push_str("\n## Missing Topics\n");
        for topic in &evaluation.missing_topics {
            document.push_str(&format!("- {topic}\n"));
        }
    }
    document
}

fn derive_design_pillars(genre: &str, player_promise: &str) -> Vec<DesignPillar> {
    vec![
        DesignPillar {
            name: "Player Promise".to_string(),
            rationale: player_promise.trim().to_string(),
        },
        DesignPillar {
            name: "Genre Fit".to_string(),
            rationale: format!("{genre} systems must reinforce the core loop instead of adding unrelated scope"),
        },
        DesignPillar {
            name: "Readable Feedback".to_string(),
            rationale: "Every important action must produce visible state change and player-facing feedback".to_string(),
        },
    ]
}

fn derive_gameplay_mechanics(core_loop: &[String]) -> Vec<GameplayMechanic> {
    core_loop
        .iter()
        .enumerate()
        .map(|(index, step)| GameplayMechanic {
            name: format!("Core Loop Mechanic {}", index + 1),
            player_action: step.clone(),
            feedback: format!("Record state change and feedback for: {step}"),
        })
        .collect()
}

fn derive_playable_scenarios(core_loop: &[String]) -> Vec<PlayableScenario> {
    if core_loop.is_empty() {
        return vec![PlayableScenario {
            scenario_id: "scenario_core_loop_empty".to_string(),
            entry_condition: "fresh_session_with_default_content".to_string(),
            player_goal: "declare_at_least_one_core_loop_step".to_string(),
            critical_path: vec!["No core loop declared".to_string()],
            success_state: "designer_adds_core_loop_step".to_string(),
            failure_state: "project_has_no_playable_action_to_validate".to_string(),
            validation_probe: "probe_core_loop_empty".to_string(),
        }];
    }

    core_loop
        .iter()
        .enumerate()
        .map(|(index, step)| {
            let step_number = index + 1;
            let prior_step = if index == 0 {
                "fresh_session_with_default_content".to_string()
            } else {
                format!("after_core_loop_step_{index}")
            };
            PlayableScenario {
                scenario_id: scenario_id_for_step(step_number),
                entry_condition: prior_step,
                player_goal: format!("execute_and_understand_core_loop_step_{step_number}"),
                critical_path: vec![step.clone()],
                success_state: format!(
                    "core_loop_step_{step_number}_produces_state_change_and_feedback"
                ),
                failure_state: format!(
                    "player_cannot_identify_result_of_core_loop_step_{step_number}"
                ),
                validation_probe: validation_probe_for_step(step_number),
            }
        })
        .collect()
}

fn scenario_id_for_step(step_number: usize) -> String {
    format!("scenario_core_loop_step_{step_number}")
}

fn validation_probe_for_step(step_number: usize) -> String {
    format!("probe_core_loop_step_{step_number}_input_state_feedback")
}

fn derive_design_risks(core_loop: &[String]) -> Vec<DesignRisk> {
    let mut risks = vec![
        DesignRisk {
            risk: "scope_drift".to_string(),
            mitigation: "Reject mechanics that do not support the declared player promise"
                .to_string(),
        },
        DesignRisk {
            risk: "feedback_unclear".to_string(),
            mitigation: "Require each loop step to declare visible or audio feedback".to_string(),
        },
    ];
    if core_loop.len() < 3 {
        risks.push(DesignRisk {
            risk: "loop_too_short".to_string(),
            mitigation: "Add decision, execution, and feedback steps before production".to_string(),
        });
    }
    risks
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sparse_design_requests_ai_intervention() {
        let brief = GameDesignBrief::new("Demo", "Action", "short", vec!["探索".to_string()])
            .expect("brief");
        let project = brief.to_project();
        let evaluation = evaluate_design(&project);
        assert!(evaluation.requires_ai_intervention);
    }

    #[test]
    fn design_document_includes_production_ready_sections() {
        let brief = GameDesignBrief::new(
            "Demo",
            "Action",
            "玩家通过探索、战斗和反馈形成稳定成长目标",
            vec![
                "探索".to_string(),
                "战斗".to_string(),
                "获得反馈".to_string(),
            ],
        )
        .expect("brief");
        let project = brief.to_project();
        let evaluation = evaluate_design(&project);
        let rendered = render_design_document(&project, &evaluation);

        assert!(rendered.contains("## Design Pillars"));
        assert!(rendered.contains("## Gameplay Mechanics"));
        assert!(rendered.contains("## Playable Scenarios"));
        assert!(rendered.contains("scenario_core_loop_step_1"));
        assert!(rendered.contains("scenario_core_loop_step_3"));
        assert!(rendered.contains("probe_core_loop_step_1_input_state_feedback"));
        assert!(rendered.contains("## Acceptance Risks"));
        assert!(rendered.contains("name=Core Loop Mechanic 1"));
    }
}
