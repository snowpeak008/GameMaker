# 12 Step13 Editor Request 与编译启动

## 目标

生成 Unity Editor request、写入 Editor Script，并采用两段式 compile -> execute 启动序列，解决首次生成 C# 后 `executeMethod` 找不到入口的问题。

## 修改范围

```text
core/art_pipeline/unity/editor_request.py
core/art_pipeline/unity/unity_project_writer.py
core/art_pipeline/unity/generated_scripts/
pipeline/step_13_scene_assembly/plugin.py
core/tests/unit/test_step13_unity_editor_request.py
```

## 输入

- `art_handoff_manifest.json`
- `unity_import_settings_manifest.json`
- `sprite_slice_result_manifest.json`
- `ugui_prefab_contract.json`
- `ui_prefab_generation_request.json`

## 输出

```text
outputs/artifacts/stage_13/unity_editor_request.json
```

## Editor Request 必需字段

```text
schema_version
editor_scripts_written
requires_compile_pass
compile_command
compile_log_path
execute_methods
request_files
expected_reports
```

## 标准启动序列

```powershell
Unity.exe -batchmode -projectPath <generated_unity_project> -quit -logFile compile.log
Unity.exe -batchmode -projectPath <generated_unity_project> -executeMethod AutoDesignAssetImporter.Run
Unity.exe -batchmode -projectPath <generated_unity_project> -executeMethod AutoDesignPrefabBuilder.Run
Unity.exe -batchmode -projectPath <generated_unity_project> -executeMethod AutoDesignSceneBinder.Run
```

## 验收标准

- 首次写入 Editor Script 后必须 compile pass。
- execute method 不存在时 failed 或 environment_blocked。
- request 中记录 bootstrap 路径和执行方法。

## 禁止事项

- 不直接首次 `-executeMethod`。
- 不把编译失败当作 Unity 物化成功。

