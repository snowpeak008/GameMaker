using System.IO;
using UnityEditor;
using UnityEditor.SceneManagement;
using UnityEngine;

namespace AutoDesignMaker
{
    public static class EditorBuild
    {
        private const string BootstrapScenePath = "Assets/AutoDesignMaker/Generated/AutoDesignMakerBootstrap.unity";

        public static void PerformBuild()
        {
            var output = "build/windows/AutoDesignMakerGame.zip";
            var directory = Path.GetDirectoryName(output);
            if (!string.IsNullOrEmpty(directory))
            {
                Directory.CreateDirectory(directory);
            }

            var scene = EditorSceneManager.NewScene(NewSceneSetup.EmptyScene, NewSceneMode.Single);
            var bootstrap = new GameObject("AutoDesignMakerBootstrap");
            bootstrap.AddComponent<AutoDesignMaker.Generated.AutoDesignMakerBootstrap>();
            EditorSceneManager.SaveScene(scene, BootstrapScenePath);

            var options = new BuildPlayerOptions
            {
                scenes = new[] { BootstrapScenePath },
                locationPathName = output,
                target = BuildTarget.StandaloneWindows64,
                options = BuildOptions.None,
            };
            BuildPipeline.BuildPlayer(options);
        }
    }
}
