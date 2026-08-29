# UI Python Baseline Plan

状态：v3 UI 基线方案已细化。实际截图采集在开发阶段执行；本文件定义复刻前必须采集什么、如何比对、哪些交互不能丢。

## 1. 目标

v2 只有 Web UI 截图和 DOM 断言，不能证明 Web/Tauri UI 严格复刻 Python Tk UI。v3 UI 复刻必须先建立 Python Tk baseline，再以 Web/Tauri 实现对齐。

验收对象不是单个页面外观，而是：

- 布局结构、密度、面板层级、按钮/输入/列表/日志区域。
- 关键交互路径、后台运行状态、错误/阻断状态。
- 长文本、长列表、空状态、忙碌状态下的尺寸稳定性。
- Tauri command/service 对 UI 动作的响应语义。

## 2. Python UI File Mapping

| Python UI file | baseline surface | Web/Tauri target | required states |
| --- | --- | --- | --- |
| `core\ui\main_window.py` | app shell, close lifecycle, pipeline/design/package nav | `adm-new-web::AppShell` + `adm-new-tauri-commands::lifecycle` | normal, startup error, running lock, close while running |
| `core\ui\app_window.py` | 16-domain design workbench, L4/L5, gameplay systems, templates, export/save | `adm-new-web::pages::design_workbench` | empty project, loaded project, long domain text, template selected, save pending, export blocked |
| `core\ui\ai_interview_window.py` | standalone AI interview | `adm-new-web::features::ai_interview` | no provider, running stream, invalid payload, summary correction, saved session |
| `core\ui\embedded_interview.py` | embedded AI panel inside workbench | shared AI interview controller/component | idle, chunked stream, mapping pending, mapping failed, accepted summary |
| `core\ui\bottom_panel.py` | log/AI tabs and queue polling | `adm-new-web::components::bottom_panel` | log tab, AI tab, overflow logs, polling update |
| `core\ui\pipeline_panel.py` | Step00-Step14 run tree, range run, stop, semantic report return path | `adm-new-web::pages::pipeline` | all pending, mixed status, running, stopped, blocked, Step07 confirmation required |
| `core\ui\pipeline_step_card.py` | individual stage card | `adm-new-web::components::pipeline_step_card` | pending, running, success, failed, blocked, long message |
| `core\ui\style_confirmation_dialog.py` | manual style confirmation | `adm-new-web::modals::style_confirmation` | needs confirmation, selected style, rejection, resume after reload |
| `core\ui\style_prompt_editor.py` | style prompt override editor | `adm-new-web::modals::style_prompt_editor` | default prompt, edited prompt, validation error, long prompt |
| `core\ui\patch_panel.py` | quick patch management | `adm-new-web::pages::patches` | empty list, patch analyzed, validation failed, apply running, promoted |
| `core\ui\package_panel.py` | package/export readiness | `adm-new-web::pages::package` | ready, blocked by validation, notes visible, package complete |
| `core\ui\log_panel.py` | runtime log list/filter | `adm-new-web::components::log_panel` | empty, long list, warning/error filter, autoscroll |
| `core\ui\log_entry.py` | single log row | `adm-new-web::components::log_entry` | info, warning, error, wrapped text |
| `core\ui\sdk_panel.py` | SDK knowledge manager | `adm-new-web::pages::sdk` | no SDKs, list, detail, review status, sync error |
| `core\ui\semantic_quality_panel.py` | semantic quality report viewer | `adm-new-web::components::semantic_quality` | missing report, pass, warnings, blocking issues |
| `core\ui\save_manager_dialog.py` | save create/load/delete/current state | `adm-new-web::modals::save_manager` | empty saves, selected save, dirty save, delete confirm, load error |
| `core\ui\ai_config_unified_dialog.py` | unified AI config/profile editing | `adm-new-web::modals::ai_config` | profile list, missing key, image provider, test failed, save success |
| `core\ui\unity_config_dialog.py` | development environment config | `adm-new-web::modals::unity_config` | no path, valid path, invalid command, preflight warnings |
| `core\ui\workbench.py` | workbench facade and self-test commands | `adm-new-application::workbench_facade` + Tauri commands | command success, command failure, soft stop, range run |
| `core\ui\theme.py` | Tk color/font/spacing tokens | `adm-new-web::theme` | token parity review, contrast review |
| `core\ui\gui_app.py` | Tk app entry wrapper | Tauri desktop entry | smoke startup, fatal init error |
| `core\ui\__init__.py` | package marker | none | no screenshot; disposition review only |

## 3. Baseline Artifacts

Each UI surface must produce a baseline record before implementation is marked complete:

- `python_screenshot`: PNG or explicit manual-review note when the widget cannot be launched headlessly.
- `web_screenshot`: Playwright screenshot for desktop and one narrow viewport.
- `interaction_trace`: clicked controls, entered text, command invoked, expected event/result.
- `parity_notes`: accepted differences and required fixes.
- `command_contract`: Tauri command/service invoked by the interaction.

Target artifact path:

```text
plan/NEWrust/full_project_reproduction/ui_baselines/<surface>/<state>.json
```

## 4. Pixel Review Rules

- Pixel-level means layout and state parity, not copying Tk rendering artifacts such as native widget chrome.
- Card nesting, decorative gradients, one-note palettes and text overflow are failures even if Python Tk allowed them.
- All compact controls must have stable dimensions; running/error labels must not resize the layout.
- Long Chinese and English text must wrap without overlapping controls.
- UI review fails if a Web page implements only the happy path while the Python panel had blocked/error/running states.

## 5. Gate

`ui-parity-v3` must check:

- Python baseline or manual baseline note exists for every required state.
- Web/Tauri screenshot exists for the same state.
- DOM/interaction assertion covers the state transition.
- Tauri command/service test covers backend behavior.
- Difference table has no unapproved P0/P1 deltas.
