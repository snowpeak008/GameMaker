# Packaging Contracts

状态：第一轮闭环解构完成。

证据文件：

- `core/ui/package_panel.py`
- `core/packaging/service.py`
- `core/packaging/validation.py`
- `core/packaging/manifest.py`
- `core/runtime/pipeline_state.py`
- `core/paths.py`

## 1. 产品位置

打包是独立顶层阶段，不属于 Step00-14 开发流水线步骤。

UI 入口：

```text
MainWindow
  -> PackagePanel
      -> refresh()
      -> run_package()
```

`PackagePanel.refresh()` 只读取 `get_step_state(PROJECT_ROOT, 14)`：

- `status == "success"`：启用“生成打包资料”按钮。
- 其他状态：禁用按钮，提示 Step14 未通过。

这只是 UI 前置条件；真正打包服务仍会二次验证 Step14 产物。

## 2. 服务入口

`run_package()`：

```text
run_package()
  -> output_dir = OUTPUTS_DIR/package/current
  -> load_packaging_sources(ARTIFACTS_DIR)
  -> validate_packaging_sources(sources)
  -> build_report
  -> write build_report.json
  -> write package_validation_report.json
  -> write PACKAGE_NOTES.md
  -> build_package_manifest()
  -> write package_manifest.json
  -> return UI result payload
```

输出目录固定为：

```text
outputs/package/current/
```

## 3. 输入产物

`load_packaging_sources()` 只读 `ARTIFACTS_DIR/stage_14`：

| 文件 | 字段用途 |
| --- | --- |
| `integration.json` | Step14 状态和 REQUIRED_INTEGRATION_CHECKS。 |
| `actual_project_file_audit.json` | `development_path`, `actual_changed_files`。 |
| `unity_validation_summary.json` | `valid`, `unity_editor_path`, validation counts。 |

缺文件时 `read_json(...,{})` 会给空对象，随后进入 blocked。

## 4. 必须通过的检查

`REQUIRED_INTEGRATION_CHECKS` 是打包前置硬门禁：

| check id | 含义 |
| --- | --- |
| `actual_development_succeeded` | 程序执行结果已通过。 |
| `scene_assembly_succeeded` | 场景组装已通过。 |
| `demo_scene_exists` | DemoScene 已确认存在。 |
| `visible_content_verified` | Demo 可见内容已确认。 |
| `build_settings_contains_demo_scene` | Build Settings 包含 Demo 场景。 |
| `playmode_smoke_passed` | PlayMode 冒烟测试通过。 |
| `unity_batchmode_validation_passed` | Unity batchmode 验证通过。 |
| `assets_traced` | 美术/资源追踪通过。 |
| `execution_objects_verified` | 执行对象闭环已验证。 |

额外硬门禁：

- `integration.status == "success"`
- `actual_project_file_audit.actual_changed_files` 是非空 list
- `unity_validation_summary.valid is True`

任一失败都会产生 `blocking_issues`，整体 `status="blocked"`。

## 5. package_validation_report.json

字段契约：

| 字段 | 类型 | 来源 |
| --- | --- | --- |
| `schema_version` | int | 固定 `1` |
| `generated_at` | string | `now_iso()` |
| `status` | string | `success` 或 `blocked` |
| `source_stage` | int | 固定 `14` |
| `source_stage_name` | string | 固定 `integration_validation` |
| `blocking_issues` | array | 门禁失败列表 |
| `checks` | array | 9 个 required check 的 passed 状态 |
| `development_path` | string | actual project audit |
| `changed_files` | array | actual changed files |
| `unity_validation` | object | Unity editor path 和 validation counts |

`blocking_issues` item：

```json
{
  "id": "PACKAGE-...",
  "message": "..."
}
```

## 6. build_report.json

字段契约：

| 字段 | 类型 |
| --- | --- |
| `schema_version` | int |
| `generated_at` | string |
| `status` | string |
| `package_type` | `current_project_build_package` |
| `source_stage` | `14` |
| `source_stage_name` | `integration_validation` |
| `development_path` | string |
| `changed_files` | array |
| `unity_validation` | object |
| `blocking_issues` | array |

## 7. package_manifest.json

`build_package_manifest()` 输出：

| 字段 | 说明 |
| --- | --- |
| `schema_version` | `PACKAGE_SCHEMA_VERSION = 1` |
| `generated_at` | 生成时间 |
| `package_type` | `current_project_build_package` |
| `status` | validation report status |
| `development_path` | Unity/项目路径 |
| `changed_files` | 实际变更文件 |
| `source_stage` | 14 |
| `source_stage_name` | `integration_validation` |
| `outputs.package_dir` | 相对路径 |
| `outputs.build_report` | 相对路径 |
| `outputs.package_validation_report` | 相对路径 |
| `outputs.package_notes` | 相对路径 |

## 8. PACKAGE_NOTES.md

`_package_notes()` 行为：

- 总是写 `# Package Notes`。
- 写 status 和 source stage。
- 有 blockers 时列出 `## Blocking Issues`。
- 无 blockers 时写 “The current project passed packaging readiness checks.”

## 9. UI 异步行为

`PackagePanel.run_package()`：

- 先检查 `_busy`。
- 再检查 Step14 pipeline state。
- 设置 `_busy=True`，禁用按钮。
- 后台线程调用 `run_package_service()`。
- 完成后 `_finish()` 展示 JSON result 或 error。
- `_finish()` 会重新 `refresh()`，因此按钮状态最终仍由 Step14 state 决定。

## 10. NEWrust 设计要求

- Tauri command `package_current_project` 必须在 Rust 后端重复执行全部 packaging validation，不依赖 UI 按钮状态。
- Web UI 只能根据 Step14 state 做按钮启禁和显示 result。
- Validation report、build report、notes、manifest 字段必须 typed model 化。
- `outputs/package/current` 是覆盖式当前打包目录；若未来需要历史包，必须另建 versioned package，不改变 current 语义。
- `changed_files` 为空时即使 Step14 state 成功也必须 blocked，避免伪打包。
