# W1.5 Legacy 执行入口保留边界

状态：已冻结（2026-07-17）
适用范围：A08c Step11 执行引擎完成后的旧执行入口处置

## 结论

W1.5 不采用“删除全部旧实现”的路线。原因是 `game_spec_v2` 仍是项目级渐进开关，旧项目在开关关闭时必须继续使用 legacy Step08-14；R0 技术探针与 CLI patch 工作流也仍依赖现有补丁执行面。强行迁移或删除会违反总计划 1.1 第 6 条“渐进式替换”。

采用的路线是：**v2 产品路径全部切换到 `WorkspaceChangeSet` / `WorkspaceTaskAgent` 权威执行模型；旧实现只保留在明确列出的兼容入口，并用代码级边界常量与测试防止误接回 v2 产品执行。**

## 保留边界

| 旧入口 | 保留原因 | 禁止用途 | v2 权威替代 |
|---|---|---|---|
| `adm-new-pipeline::WorkUnitExecutor` | legacy Step08-14、Step07 图片任务、旧 checkpoint 恢复仍需兼容 | 不得作为 GameSpec v2 Step11 产品执行模型 | `adm-new-pipeline::stages::step11_v2::WorkspaceTaskAgent` |
| `adm-new-application::AiDevelopmentWorkUnitExecutor` | 同一个真实 Codex/Claude CLI 适配器同时服务 legacy work-unit 与 v2 `WorkspaceTaskAgent` bridge | 不得绕过 `WorkspaceChangeSet` 直接为 v2 产品 Step11 提交结果 | `WorkspaceTaskAgent::execute_task(...)` + `WorkspaceChangeSet` 验证 |
| `adm-new-patch::CodexPatchRunner` | CLI patch 工作流与 R0 技术探针仍需独立补丁面 | 不得作为 GameSpec v2 Step11 / Step12 / Step13 / Step14 产品路径执行器 | A08b/A08c 的任务合同与 Step11 v2 执行内核 |

## 强制规则

1. `game_spec_v2=true` 的产品/UI Step11 必须要求真实 `WorkspaceTaskAgent`，不可 fallback 到 legacy `WorkUnitExecutor` 或 `CodexPatchRunner`。
2. legacy `WorkUnitExecutor` 可继续服务 `game_spec_v2=false` 的旧流水线，但其输出不能提升为 v2 权威执行证据。
3. `CodexPatchRunner` 只能由 CLI patch / R0 harness 调用；新生产流水线不得新增对它的 Step11 调用。
4. 保留策略必须在源码中有机器可测试的边界描述；文档注释不足以关闭 W1.5。

## 验收

- 代码公开边界常量，描述每个保留入口的 allowed/prohibited/replacement。
- 测试验证 `WorkUnitExecutor`、`AiDevelopmentWorkUnitExecutor`、`CodexPatchRunner` 的禁止用途包含 GameSpec v2 产品 Step11。
- 产品路径已有 `require_workspace_task_agent()` 与无 agent fail-closed 测试，保证 v2 Step11 不会退回 legacy 执行面。

## 影响面

- 不删除 legacy Step08-14，不影响旧项目和 `game_spec_v2=false` 默认路径。
- 不改变 CLI patch 命令和 R0 探针行为。
- W1.5 关闭后，剩余工作转入 W4/W5 的真实 VLM、真实引擎加载、场景执行、EXE smoke 和人工签署发布门。
