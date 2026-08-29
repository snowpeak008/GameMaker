using System.IO;
using AutoDesignMaker.Generated;
using UnityEngine;

namespace AutoDesignMaker.Runtime
{
    public sealed class AutoDesignMakerRuntimeController : MonoBehaviour
    {
        private AutoDesignMakerInputRouter inputRouter;
        private AutoDesignMakerGameplayController gameplayController;
        private AutoDesignMakerSceneComposer sceneComposer;
        private float elapsedSeconds;
        private int autosaveFrame;
        private string lastSavePath = "not saved";

        public string RuntimeState => "running";

        private void Awake()
        {
            inputRouter = GetComponent<AutoDesignMakerInputRouter>();
            if (inputRouter == null)
            {
                inputRouter = gameObject.AddComponent<AutoDesignMakerInputRouter>();
            }
            gameplayController = GetComponent<AutoDesignMakerGameplayController>();
            if (gameplayController == null)
            {
                gameplayController = gameObject.AddComponent<AutoDesignMakerGameplayController>();
            }
            sceneComposer = GetComponent<AutoDesignMakerSceneComposer>();
            if (sceneComposer == null)
            {
                sceneComposer = gameObject.AddComponent<AutoDesignMakerSceneComposer>();
            }
            DontDestroyOnLoad(gameObject);
        }

        private void Update()
        {
            elapsedSeconds += Time.deltaTime;
            if (Time.frameCount > 0 && Time.frameCount % 300 == 0)
            {
                SaveRuntimeSnapshot();
            }
        }

        public void SaveRuntimeSnapshot()
        {
            var saveData = new AutoDesignMakerSaveData
            {
                target_id = AutoDesignMakerGeneratedContent.TargetId,
                build_profile = AutoDesignMakerGeneratedContent.BuildProfile,
                session_seconds = elapsedSeconds,
                autosave_frame = Time.frameCount,
                last_input_axis = inputRouter == null ? "0,0" : inputRouter.AxisText,
                active_mechanic = gameplayController == null ? "none" : gameplayController.ActiveMechanicName,
                mechanic_index = gameplayController == null ? 0 : gameplayController.ActiveMechanicIndex,
                pipeline_artifacts = AutoDesignMakerGeneratedContent.PipelineArtifactPaths,
            };

            var directory = Path.Combine(Application.persistentDataPath, "AutoDesignMaker");
            Directory.CreateDirectory(directory);
            lastSavePath = Path.Combine(directory, "runtime-save.json");
            File.WriteAllText(lastSavePath, JsonUtility.ToJson(saveData, true));
            autosaveFrame = Time.frameCount;
        }

        private void OnGUI()
        {
            GUILayout.BeginArea(new Rect(20, 20, 560, 176), "AutoDesignMaker Runtime", GUI.skin.window);
            GUILayout.Label($"Target: {AutoDesignMakerGeneratedContent.TargetId}");
            GUILayout.Label($"Profile: {AutoDesignMakerGeneratedContent.BuildProfile}");
            GUILayout.Label($"State: {RuntimeState} | Seconds: {elapsedSeconds:0.0} | Last autosave frame: {autosaveFrame}");
            GUILayout.Label($"Input: {(inputRouter == null ? "0,0" : inputRouter.AxisText)}");
            GUILayout.Label($"Save: {lastSavePath}");
            if (GUILayout.Button("Save Runtime Snapshot"))
            {
                SaveRuntimeSnapshot();
            }
            GUILayout.EndArea();
        }
    }
}
