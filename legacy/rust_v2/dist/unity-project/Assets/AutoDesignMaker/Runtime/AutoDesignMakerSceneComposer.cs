using AutoDesignMaker.Generated;
using UnityEngine;

namespace AutoDesignMaker.Runtime
{
    public sealed class AutoDesignMakerSceneComposer : MonoBehaviour
    {
        private const string RootName = "AutoDesignMakerGeneratedScene";
        private bool composed;

        private void Start()
        {
            ComposeScene();
        }

        public void ComposeScene()
        {
            if (composed || GameObject.Find(RootName) != null)
            {
                composed = true;
                return;
            }

            composed = true;
            var root = new GameObject(RootName);
            EnsureCamera(root.transform);
            CreateDirectionalLight(root.transform);
            CreateFloor(root.transform);
            CreateMechanicNodes(root.transform);
            CreateGoalMarker(root.transform);
            CreateScenarioBoard(root.transform);
        }

        private void EnsureCamera(Transform root)
        {
            var mainCamera = Camera.main;
            if (mainCamera == null)
            {
                var cameraObject = new GameObject("ADM_MainCamera");
                cameraObject.transform.SetParent(root);
                mainCamera = cameraObject.AddComponent<Camera>();
                cameraObject.tag = "MainCamera";
            }
            mainCamera.transform.position = new Vector3(0f, 6.5f, -10f);
            mainCamera.transform.rotation = Quaternion.Euler(55f, 0f, 0f);
            mainCamera.fieldOfView = 45f;
            mainCamera.clearFlags = CameraClearFlags.Skybox;
        }

        private void CreateDirectionalLight(Transform root)
        {
            var lightObject = new GameObject("ADM_KeyLight");
            lightObject.transform.SetParent(root);
            lightObject.transform.rotation = Quaternion.Euler(50f, -35f, 0f);
            var light = lightObject.AddComponent<Light>();
            light.type = LightType.Directional;
            light.intensity = 1.1f;
        }

        private void CreateFloor(Transform root)
        {
            var floor = GameObject.CreatePrimitive(PrimitiveType.Plane);
            floor.name = "ADM_WorkbenchFloor";
            floor.transform.SetParent(root);
            floor.transform.localScale = new Vector3(1.4f, 1f, 1.0f);
            ApplyColor(floor, new Color(0.18f, 0.20f, 0.22f));
        }

        private void CreateMechanicNodes(Transform root)
        {
            var count = Mathf.Max(1, AutoDesignMakerGameplayModel.Mechanics.Length);
            var positions = new Vector3[count];
            var spacing = 2.35f;
            var startX = -((count - 1) * spacing) * 0.5f;

            for (var i = 0; i < count; i++)
            {
                var mechanic = AutoDesignMakerGameplayModel.Mechanics.Length == 0
                    ? null
                    : AutoDesignMakerGameplayModel.Mechanics[i];
                var node = GameObject.CreatePrimitive(PrimitiveType.Cube);
                node.name = "ADM_Mechanic_" + i.ToString("00");
                node.transform.SetParent(root);
                node.transform.position = new Vector3(startX + i * spacing, 0.45f, 0f);
                node.transform.localScale = new Vector3(1.45f, 0.55f, 1.15f);
                ApplyColor(node, MechanicColor(i));
                positions[i] = node.transform.position;

                var title = mechanic == null ? "Generated Mechanic" : mechanic.Name;
                var action = mechanic == null ? "Awaiting generated input" : mechanic.PlayerAction;
                CreateLabel(node.transform, title + "\n" + action, new Vector3(0f, 0.8f, 0f), 0.18f);

                var task = MatchingTask(title);
                var asset = MatchingAsset(title);
                var details = (task == null ? "No task" : task.ImplementationLayer)
                    + "\n"
                    + (asset == null ? "No asset" : asset.AssetKind);
                CreateLabel(node.transform, details, new Vector3(0f, -0.7f, 0f), 0.14f);
            }

            CreateLoopLinks(root, positions);
        }

        private void CreateLoopLinks(Transform root, Vector3[] positions)
        {
            for (var i = 0; i + 1 < positions.Length; i++)
            {
                var lineObject = new GameObject("ADM_LoopLink_" + i.ToString("00"));
                lineObject.transform.SetParent(root);
                var line = lineObject.AddComponent<LineRenderer>();
                line.positionCount = 2;
                line.useWorldSpace = true;
                line.startWidth = 0.06f;
                line.endWidth = 0.06f;
                line.material = CreateMaterial("ADM_LoopLinkMaterial", new Color(0.86f, 0.86f, 0.78f));
                line.SetPosition(0, positions[i] + new Vector3(0.8f, 0.2f, 0f));
                line.SetPosition(1, positions[i + 1] + new Vector3(-0.8f, 0.2f, 0f));
            }
        }

        private void CreateGoalMarker(Transform root)
        {
            var scenario = AutoDesignMakerGameplayModel.Scenarios.Length == 0
                ? null
                : AutoDesignMakerGameplayModel.Scenarios[0];
            var goal = GameObject.CreatePrimitive(PrimitiveType.Cylinder);
            goal.name = "ADM_GoalMarker";
            goal.transform.SetParent(root);
            goal.transform.position = new Vector3(0f, 0.7f, 2.6f);
            goal.transform.localScale = new Vector3(0.85f, 0.45f, 0.85f);
            ApplyColor(goal, new Color(0.92f, 0.76f, 0.20f));

            var goalText = scenario == null ? "Scenario Goal" : scenario.Goal;
            var successText = scenario == null ? "Success condition" : scenario.Success;
            CreateLabel(goal.transform, "Goal\n" + goalText + "\n" + successText, new Vector3(0f, 1.0f, 0f), 0.16f);
        }

        private void CreateScenarioBoard(Transform root)
        {
            var board = GameObject.CreatePrimitive(PrimitiveType.Cube);
            board.name = "ADM_ScenarioBoard";
            board.transform.SetParent(root);
            board.transform.position = new Vector3(0f, 1.35f, -2.75f);
            board.transform.localScale = new Vector3(5.5f, 1.5f, 0.16f);
            ApplyColor(board, new Color(0.10f, 0.12f, 0.14f));

            var scenario = AutoDesignMakerGameplayModel.Scenarios.Length == 0
                ? null
                : AutoDesignMakerGameplayModel.Scenarios[0];
            var loop = AutoDesignMakerGameplayModel.CoreLoop.Length == 0
                ? "No generated core loop"
                : string.Join(" > ", AutoDesignMakerGameplayModel.CoreLoop);
            var text = "Generated Runtime\n"
                + (scenario == null ? "Scenario: none" : "Scenario: " + scenario.ScenarioId)
                + "\nLoop: "
                + loop;
            CreateLabel(board.transform, text, new Vector3(0f, 0.08f, -0.12f), 0.15f);
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

        private void CreateLabel(Transform parent, string text, Vector3 localPosition, float size)
        {
            var label = new GameObject(parent.name + "_Label");
            label.transform.SetParent(parent);
            label.transform.localPosition = localPosition;
            label.transform.localRotation = Quaternion.Euler(65f, 0f, 0f);
            var mesh = label.AddComponent<TextMesh>();
            mesh.text = TrimForLabel(text, 120);
            mesh.anchor = TextAnchor.MiddleCenter;
            mesh.alignment = TextAlignment.Center;
            mesh.characterSize = size;
            mesh.fontSize = 48;
            mesh.color = Color.white;
        }

        private string TrimForLabel(string value, int maxLength)
        {
            if (string.IsNullOrEmpty(value) || value.Length <= maxLength)
            {
                return value;
            }
            return value.Substring(0, maxLength - 3) + "...";
        }

        private Color MechanicColor(int index)
        {
            var palette = new Color[]
            {
                new Color(0.22f, 0.49f, 0.78f),
                new Color(0.32f, 0.62f, 0.46f),
                new Color(0.76f, 0.42f, 0.29f),
                new Color(0.54f, 0.46f, 0.76f),
                new Color(0.75f, 0.61f, 0.28f),
            };
            return palette[index % palette.Length];
        }

        private void ApplyColor(GameObject target, Color color)
        {
            var objectRenderer = target.GetComponent<Renderer>();
            if (objectRenderer != null)
            {
                objectRenderer.sharedMaterial = CreateMaterial(target.name + "_Material", color);
            }
        }

        private Material CreateMaterial(string name, Color color)
        {
            var shader = Shader.Find("Standard");
            if (shader == null)
            {
                shader = Shader.Find("Diffuse");
            }
            if (shader == null)
            {
                shader = Shader.Find("Sprites/Default");
            }
            var material = new Material(shader);
            material.name = name;
            material.color = color;
            return material;
        }
    }
}
