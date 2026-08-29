using AutoDesignMaker.Generated;
using UnityEngine;

namespace AutoDesignMaker.Runtime
{
    public sealed class AutoDesignMakerGameplayController : MonoBehaviour
    {
        private AutoDesignMakerInputRouter inputRouter;
        private int activeMechanicIndex;
        private string lastFeedback = "Waiting for player input";

        public int ActiveMechanicIndex => activeMechanicIndex;

        public string ActiveMechanicName
        {
            get
            {
                var mechanic = ActiveMechanic();
                return mechanic == null ? "none" : mechanic.Name;
            }
        }

        private void Awake()
        {
            inputRouter = GetComponent<AutoDesignMakerInputRouter>();
            if (inputRouter == null)
            {
                inputRouter = gameObject.AddComponent<AutoDesignMakerInputRouter>();
            }
        }

        private void Update()
        {
            if (inputRouter != null && inputRouter.ConfirmPressed)
            {
                AdvanceMechanic();
            }
            if (inputRouter != null && inputRouter.CancelPressed)
            {
                ResetLoop();
            }
        }

        public void AdvanceMechanic()
        {
            if (AutoDesignMakerGameplayModel.Mechanics.Length == 0)
            {
                lastFeedback = "No generated mechanics are available";
                return;
            }

            activeMechanicIndex = (activeMechanicIndex + 1) % AutoDesignMakerGameplayModel.Mechanics.Length;
            var mechanic = ActiveMechanic();
            lastFeedback = mechanic == null ? "No generated feedback" : mechanic.Feedback;
        }

        public void ResetLoop()
        {
            activeMechanicIndex = 0;
            var mechanic = ActiveMechanic();
            lastFeedback = mechanic == null ? "Loop reset" : mechanic.Feedback;
        }

        private GeneratedMechanic ActiveMechanic()
        {
            if (AutoDesignMakerGameplayModel.Mechanics.Length == 0)
            {
                return null;
            }
            var index = Mathf.Clamp(activeMechanicIndex, 0, AutoDesignMakerGameplayModel.Mechanics.Length - 1);
            return AutoDesignMakerGameplayModel.Mechanics[index];
        }

        private GeneratedScenario ActiveScenario()
        {
            return AutoDesignMakerGameplayModel.Scenarios.Length == 0 ? null : AutoDesignMakerGameplayModel.Scenarios[0];
        }

        private GeneratedDevelopmentTask MatchingTask(string mechanicName)
        {
            for (var i = 0; i < AutoDesignMakerGameplayModel.DevelopmentTasks.Length; i++)
            {
                var task = AutoDesignMakerGameplayModel.DevelopmentTasks[i];
                if (task.SourceMechanic == mechanicName)
                {
                    return task;
                }
            }
            return null;
        }

        private GeneratedAssetFeedback MatchingAsset(string mechanicName)
        {
            for (var i = 0; i < AutoDesignMakerGameplayModel.AssetFeedback.Length; i++)
            {
                var asset = AutoDesignMakerGameplayModel.AssetFeedback[i];
                if (asset.SourceMechanic == mechanicName)
                {
                    return asset;
                }
            }
            return null;
        }

        private void OnGUI()
        {
            var mechanic = ActiveMechanic();
            var scenario = ActiveScenario();
            var task = MatchingTask(mechanic == null ? string.Empty : mechanic.Name);
            var asset = MatchingAsset(mechanic == null ? string.Empty : mechanic.Name);

            GUILayout.BeginArea(new Rect(20, 212, 680, 252), "Generated Gameplay Loop", GUI.skin.window);
            GUILayout.Label("Confirm advances the generated mechanic. Escape resets the loop.");
            GUILayout.Label($"Mechanic: {(mechanic == null ? "none" : mechanic.Name)}");
            GUILayout.Label($"Player Action: {(mechanic == null ? "none" : mechanic.PlayerAction)}");
            GUILayout.Label($"Feedback: {lastFeedback}");
            GUILayout.Label($"Scenario Goal: {(scenario == null ? "none" : scenario.Goal)}");
            GUILayout.Label($"Success: {(scenario == null ? "none" : scenario.Success)}");
            GUILayout.Label($"Development: {(task == null ? "none" : task.Title + " | " + task.ImplementationLayer)}");
            GUILayout.Label($"Asset Feedback: {(asset == null ? "none" : asset.AssetKind + " | " + asset.Description)}");
            GUILayout.EndArea();
        }
    }
}
