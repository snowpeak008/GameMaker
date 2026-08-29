# Dynamic Reachability Index

????????????? v3 ???????????? `tools/build/*.py`?

## 1. ??

| ?? | ?? |
| --- | ---: |
| `pipeline/_registry.json` module entries | 19 |
| `__main__` entry files | 42 |
| `importlib` text hits | 7 |
| `subprocess` / `Popen` text hits | 98 |

## 2. Pipeline Registry Entries

`pipeline/_registry.json` ? authoritative dynamic loading source??? 19 ? stage modules ???? pipeline ?????

| stage | module |
| --- | --- |
| D1 | `pipeline.step_d1_project_portrait.plugin` |
| D2 | `pipeline.step_d2_design_decisions.plugin` |
| D3 | `pipeline.step_d3_design_validation.plugin` |
| D4 | `pipeline.step_d4_devflow_handoff.plugin` |
| 00 | `pipeline.step_00_idea_intake.plugin` |
| 01 | `pipeline.step_01_gameplay_framework.plugin` |
| 02 | `pipeline.step_02_design_review_freeze.plugin` |
| 03 | `pipeline.step_03_program_requirements.plugin` |
| 04 | `pipeline.step_04_art_requirements.plugin` |
| 05 | `pipeline.step_05_program_review.plugin` |
| 06 | `pipeline.step_06_art_review.plugin` |
| 07 | `pipeline.step_07_art_style_generation.plugin` |
| 08 | `pipeline.step_08_design_to_plan.plugin` |
| 09 | `pipeline.step_09_art_plan.plugin` |
| 10 | `pipeline.step_10_asset_alignment.plugin` |
| 11 | `pipeline.step_11_dev_execution.plugin` |
| 12 | `pipeline.step_12_art_production.plugin` |
| 13 | `pipeline.step_13_scene_assembly.plugin` |
| 14 | `pipeline.step_14_integration_validation.plugin` |

## 3. Dynamic Import Hotspots

| file | hit_count | v3 action |
| --- | ---: | --- |
| `core/plugin_manager.py` | 2 | typed Rust registry or explicit plugin boundary; no silent dynamic import loss |
| `core/tests/unit/test_pytest_config.py` | 5 | typed Rust registry or explicit plugin boundary; no silent dynamic import loss |

## 4. Subprocess Hotspots

Subprocess usage appears in AI adapters, AI backend, image tooling, build/dev validators, workbench helpers, and save/open-file helpers.

| file | hit_count | v3 action |
| --- | ---: | --- |
| `core/adapters/claude_code_adapter.py` | 4 | classify as product runtime, dev tool, gate, or drop; migrate behind Rust service/CLI/xtask boundary |
| `core/adapters/claude_code_model_adapter.py` | 4 | classify as product runtime, dev tool, gate, or drop; migrate behind Rust service/CLI/xtask boundary |
| `core/adapters/codex/executor.py` | 4 | classify as product runtime, dev tool, gate, or drop; migrate behind Rust service/CLI/xtask boundary |
| `core/config/validator.py` | 4 | classify as product runtime, dev tool, gate, or drop; migrate behind Rust service/CLI/xtask boundary |
| `core/design/ai_backend.py` | 14 | classify as product runtime, dev tool, gate, or drop; migrate behind Rust service/CLI/xtask boundary |
| `core/engines/generation.py` | 6 | classify as product runtime, dev tool, gate, or drop; migrate behind Rust service/CLI/xtask boundary |
| `core/tests/unit/test_codex_image_tool.py` | 3 | classify as product runtime, dev tool, gate, or drop; migrate behind Rust service/CLI/xtask boundary |
| `core/tests/unit/test_config_validator.py` | 1 | classify as product runtime, dev tool, gate, or drop; migrate behind Rust service/CLI/xtask boundary |
| `core/tests/unit/test_model_adapters.py` | 3 | classify as product runtime, dev tool, gate, or drop; migrate behind Rust service/CLI/xtask boundary |
| `core/tests/unit/test_pytest_config.py` | 2 | classify as product runtime, dev tool, gate, or drop; migrate behind Rust service/CLI/xtask boundary |
| `core/ui/save_manager_dialog.py` | 2 | classify as product runtime, dev tool, gate, or drop; migrate behind Rust service/CLI/xtask boundary |
| `core/ui/workbench.py` | 22 | classify as product runtime, dev tool, gate, or drop; migrate behind Rust service/CLI/xtask boundary |
| `core/utils/process_utils.py` | 5 | classify as product runtime, dev tool, gate, or drop; migrate behind Rust service/CLI/xtask boundary |
| `tools/asset_production/codex_image_tool.py` | 4 | classify as product runtime, dev tool, gate, or drop; migrate behind Rust service/CLI/xtask boundary |
| `tools/build/build.py` | 2 | classify as product runtime, dev tool, gate, or drop; migrate behind Rust service/CLI/xtask boundary |
| `tools/dev/git_tool.py` | 4 | classify as product runtime, dev tool, gate, or drop; migrate behind Rust service/CLI/xtask boundary |
| `tools/validators/compile_checker.py` | 4 | classify as product runtime, dev tool, gate, or drop; migrate behind Rust service/CLI/xtask boundary |
| `tools/validators/environment_checker.py` | 10 | classify as product runtime, dev tool, gate, or drop; migrate behind Rust service/CLI/xtask boundary |

## 5. GUI Callback Reachability

Tk callbacks are dynamic entry points and cannot be inferred from imports alone. The UI files already dispositioned as Web/Tauri targets remain authoritative for P4 pixel/UI parity.

Hot files:

- `core/ui/main_window.py`
- `core/ui/app_window.py`
- `core/ui/pipeline_panel.py`
- `core/ui/embedded_interview.py`
- `core/ui/ai_interview_window.py`
- `core/ui/ai_config_unified_dialog.py`
- `core/ui/package_panel.py`
- `core/ui/patch_panel.py`
- `core/ui/log_panel.py`
- `core/ui/sdk_panel.py`
- `core/ui/style_confirmation_dialog.py`
- `core/ui/style_prompt_editor.py`
- `core/ui/save_manager_dialog.py`
- `core/ui/unity_config_dialog.py`
- `core/ui/workbench.py`
- `core/ui/bottom_panel.py`

## 6. Current Gate Status

This index now supports the v3 gate because static orphan candidates have a completed decision matrix. Development is still blocked until the multirole scorecard and v3 atomic development plan pass.
