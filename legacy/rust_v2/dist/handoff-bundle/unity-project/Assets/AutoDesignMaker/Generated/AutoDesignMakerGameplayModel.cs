namespace AutoDesignMaker.Generated
{
    public sealed class GeneratedMechanic
    {
        public readonly string Name;
        public readonly string PlayerAction;
        public readonly string Feedback;

        public GeneratedMechanic(string name, string playerAction, string feedback)
        {
            Name = name;
            PlayerAction = playerAction;
            Feedback = feedback;
        }
    }

    public sealed class GeneratedScenario
    {
        public readonly string ScenarioId;
        public readonly string Goal;
        public readonly string Success;
        public readonly string Failure;
        public readonly string ValidationProbe;

        public GeneratedScenario(string scenarioId, string goal, string success, string failure, string validationProbe)
        {
            ScenarioId = scenarioId;
            Goal = goal;
            Success = success;
            Failure = failure;
            ValidationProbe = validationProbe;
        }
    }

    public sealed class GeneratedDevelopmentTask
    {
        public readonly string SourceMechanic;
        public readonly string Title;
        public readonly string ImplementationLayer;
        public readonly string Acceptance;

        public GeneratedDevelopmentTask(string sourceMechanic, string title, string implementationLayer, string acceptance)
        {
            SourceMechanic = sourceMechanic;
            Title = title;
            ImplementationLayer = implementationLayer;
            Acceptance = acceptance;
        }
    }

    public sealed class GeneratedAssetFeedback
    {
        public readonly string SourceMechanic;
        public readonly string AssetKind;
        public readonly string Description;
        public readonly string Acceptance;

        public GeneratedAssetFeedback(string sourceMechanic, string assetKind, string description, string acceptance)
        {
            SourceMechanic = sourceMechanic;
            AssetKind = assetKind;
            Description = description;
            Acceptance = acceptance;
        }
    }

    public static class AutoDesignMakerGameplayModel
    {
        public static readonly string[] CoreLoop = new string[]
        {
            "探索关卡并发现风险",
            "使用核心能力解决战斗或机关",
            "获得反馈、资源和新目标",
        };

        public static readonly GeneratedMechanic[] Mechanics = new GeneratedMechanic[]
        {
            new GeneratedMechanic("Core Loop Mechanic 1", "探索关卡并发现风险", "Record state change and feedback for: 探索关卡并发现风险"),
            new GeneratedMechanic("Core Loop Mechanic 2", "使用核心能力解决战斗或机关", "Record state change and feedback for: 使用核心能力解决战斗或机关"),
            new GeneratedMechanic("Core Loop Mechanic 3", "获得反馈、资源和新目标", "Record state change and feedback for: 获得反馈、资源和新目标"),
        };

        public static readonly GeneratedScenario[] Scenarios = new GeneratedScenario[]
        {
            new GeneratedScenario("scenario_core_loop_step_1", "execute_and_understand_core_loop_step_1", "core_loop_step_1_produces_state_change_and_feedback", "player_cannot_identify_result_of_core_loop_step_1", "probe_core_loop_step_1_input_state_feedback"),
            new GeneratedScenario("scenario_core_loop_step_2", "execute_and_understand_core_loop_step_2", "core_loop_step_2_produces_state_change_and_feedback", "player_cannot_identify_result_of_core_loop_step_2", "probe_core_loop_step_2_input_state_feedback"),
            new GeneratedScenario("scenario_core_loop_step_3", "execute_and_understand_core_loop_step_3", "core_loop_step_3_produces_state_change_and_feedback", "player_cannot_identify_result_of_core_loop_step_3", "probe_core_loop_step_3_input_state_feedback"),
        };

        public static readonly GeneratedDevelopmentTask[] DevelopmentTasks = new GeneratedDevelopmentTask[]
        {
            new GeneratedDevelopmentTask("Core Loop Mechanic 1", "Implement core loop step 1: 探索关卡并发现风险", "input_and_navigation", "Input, state transition, feedback, tests, and telemetry are traceable"),
            new GeneratedDevelopmentTask("Core Loop Mechanic 2", "Implement core loop step 2: 使用核心能力解决战斗或机关", "simulation_and_rules", "Input, state transition, feedback, tests, and telemetry are traceable"),
            new GeneratedDevelopmentTask("Core Loop Mechanic 3", "Implement core loop step 3: 获得反馈、资源和新目标", "feedback_rewards_and_progression", "Input, state transition, feedback, tests, and telemetry are traceable"),
        };

        public static readonly GeneratedAssetFeedback[] AssetFeedback = new GeneratedAssetFeedback[]
        {
            new GeneratedAssetFeedback("design_pillars", "visual_style", "2D action adventure visual style guide", "Style, color, proportions, and UI tone are reusable"),
            new GeneratedAssetFeedback("state_visibility", "workbench_ui", "Core gameplay HUD and state indicator assets", "Key state, goal, and feedback information is readable"),
            new GeneratedAssetFeedback("feedback_mechanics", "audio_cues", "Input confirmation, reward, and failure audio cues", "Audio feedback complements visual feedback without disrupting control"),
            new GeneratedAssetFeedback("Core Loop Mechanic 1", "interaction_feedback", "Feedback, readability, and state cues for: 探索关卡并发现风险", "Core Loop Mechanic 1 has inspectable feedback assets and validation cues"),
            new GeneratedAssetFeedback("Core Loop Mechanic 2", "interaction_feedback", "Feedback, readability, and state cues for: 使用核心能力解决战斗或机关", "Core Loop Mechanic 2 has inspectable feedback assets and validation cues"),
            new GeneratedAssetFeedback("Core Loop Mechanic 3", "interaction_feedback", "Feedback, readability, and state cues for: 获得反馈、资源和新目标", "Core Loop Mechanic 3 has inspectable feedback assets and validation cues"),
        };
    }
}
