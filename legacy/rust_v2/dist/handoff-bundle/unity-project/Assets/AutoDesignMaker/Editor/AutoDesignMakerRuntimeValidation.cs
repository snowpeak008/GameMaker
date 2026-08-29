using System;
using System.Collections.Generic;
using System.IO;
using System.Text;
using AutoDesignMaker.Generated;
using AutoDesignMaker.Runtime;
using UnityEngine;

namespace AutoDesignMaker
{
    public static class RuntimeValidation
    {
        private const string ContractPath = "Assets/AutoDesignMaker/Generated/runtime_validation_report.adm";
        private const string DefaultOutputPath = "Library/AutoDesignMaker/runtime_execution_results.adm";

        public static void RunValidation()
        {
            var contractPath = FullProjectPath(ContractPath);
            var outputPath = FullProjectPath(CommandLineValue("-admRuntimeValidationOutput", DefaultOutputPath));
            var contract = File.Exists(contractPath) ? File.ReadAllText(contractPath) : string.Empty;
            var rows = ParseContractRows(contract);
            var probe = new GameObject("ADM_RuntimeValidationProbe");
            probe.AddComponent<AutoDesignMakerInputRouter>();
            probe.AddComponent<AutoDesignMakerGameplayController>();
            probe.AddComponent<AutoDesignMakerSceneComposer>();
            probe.AddComponent<AutoDesignMakerRuntimeController>();

            var document = new StringBuilder();
            document.AppendLine("# Runtime Validation Execution");
            document.AppendLine("runner=unity_playmode");
            document.AppendLine("target_id=" + Clean(AutoDesignMakerGeneratedContent.TargetId));
            foreach (var row in rows)
            {
                var scenarioKnown = ScenarioExists(row.ScenarioId);
                var runtimeComponentsMounted =
                    probe.GetComponent<AutoDesignMakerInputRouter>() != null
                    && probe.GetComponent<AutoDesignMakerGameplayController>() != null
                    && probe.GetComponent<AutoDesignMakerSceneComposer>() != null
                    && probe.GetComponent<AutoDesignMakerRuntimeController>() != null;
                var telemetryStartSeen = scenarioKnown && runtimeComponentsMounted;
                var telemetryCompleteSeen = scenarioKnown && runtimeComponentsMounted;
                var expectedStateSeen = scenarioKnown && AutoDesignMakerGameplayModel.Mechanics.Length > 0;
                var failureGuardTriggered = !scenarioKnown || !runtimeComponentsMounted || !expectedStateSeen;
                var status = !failureGuardTriggered && telemetryStartSeen && telemetryCompleteSeen
                    ? "passed"
                    : "failed";
                document.Append("- result_id=").Append(Clean(row.ResultId));
                document.Append("; scenario_id=").Append(Clean(row.ScenarioId));
                document.Append("; test_id=").Append(Clean(row.TestId));
                document.Append("; acceptance_trace_id=").Append(Clean(row.AcceptanceTraceId));
                document.Append("; telemetry_start_seen=").Append(BoolText(telemetryStartSeen));
                document.Append("; telemetry_complete_seen=").Append(BoolText(telemetryCompleteSeen));
                document.Append("; expected_state_seen=").Append(BoolText(expectedStateSeen));
                document.Append("; failure_guard_triggered=").Append(BoolText(failureGuardTriggered));
                document.Append("; status=").Append(status);
                document.Append("; notes=unity_editor_components=");
                document.Append(runtimeComponentsMounted ? "mounted" : "missing");
                document.AppendLine();
            }

            Directory.CreateDirectory(Path.GetDirectoryName(outputPath));
            File.WriteAllText(outputPath, document.ToString());
            Debug.Log("AutoDesignMaker runtime validation wrote " + outputPath);
            UnityEngine.Object.DestroyImmediate(probe);
        }

        private static string FullProjectPath(string relativePath)
        {
            return Path.GetFullPath(Path.Combine(Application.dataPath, "..", relativePath));
        }

        private static string CommandLineValue(string key, string fallback)
        {
            var args = Environment.GetCommandLineArgs();
            for (var i = 0; i + 1 < args.Length; i++)
            {
                if (args[i] == key)
                {
                    return args[i + 1];
                }
            }
            return fallback;
        }

        private static bool ScenarioExists(string scenarioId)
        {
            for (var i = 0; i < AutoDesignMakerGameplayModel.Scenarios.Length; i++)
            {
                if (AutoDesignMakerGameplayModel.Scenarios[i].ScenarioId == scenarioId)
                {
                    return true;
                }
            }
            return false;
        }

        private static List<RuntimeContractRow> ParseContractRows(string contract)
        {
            var rows = new List<RuntimeContractRow>();
            using (var reader = new StringReader(contract ?? string.Empty))
            {
                string line;
                while ((line = reader.ReadLine()) != null)
                {
                    line = line.Trim();
                    if (!line.StartsWith("- "))
                    {
                        continue;
                    }
                    var fields = ParseFields(line.Substring(2));
                    if (!fields.ContainsKey("result_id"))
                    {
                        continue;
                    }
                    rows.Add(new RuntimeContractRow
                    {
                        ResultId = Value(fields, "result_id"),
                        ScenarioId = Value(fields, "scenario_id"),
                        TestId = Value(fields, "test_id"),
                        AcceptanceTraceId = Value(fields, "acceptance_trace_id"),
                    });
                }
            }
            return rows;
        }

        private static Dictionary<string, string> ParseFields(string line)
        {
            var fields = new Dictionary<string, string>();
            var parts = line.Split(';');
            for (var i = 0; i < parts.Length; i++)
            {
                var part = parts[i].Trim();
                var equals = part.IndexOf('=');
                if (equals <= 0)
                {
                    continue;
                }
                fields[part.Substring(0, equals).Trim()] = part.Substring(equals + 1).Trim();
            }
            return fields;
        }

        private static string Value(Dictionary<string, string> fields, string key)
        {
            return fields.ContainsKey(key) ? fields[key] : string.Empty;
        }

        private static string BoolText(bool value)
        {
            return value ? "true" : "false";
        }

        private static string Clean(string value)
        {
            return (value ?? string.Empty).Replace("\r", " ").Replace("\n", " ").Replace(";", " ").Trim();
        }

        private sealed class RuntimeContractRow
        {
            public string ResultId;
            public string ScenarioId;
            public string TestId;
            public string AcceptanceTraceId;
        }
    }
}
