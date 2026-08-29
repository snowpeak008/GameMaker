# Architecture Overview

状态：第一轮设计完成。

evidence=

- `python_deconstruction/02_system_map.md`
- `python_deconstruction/10_feature_parity_inventory.md`
- `python_deconstruction/16_ai_interview_and_completion_contracts.md`
- `python_deconstruction/19_ui_reproduction_specs.md`
- `python_deconstruction/20_parity_gate_test_matrix.md`

classification=NEWrust authoritative design

confidence=high

open_questions=

- Tauri/Web 具体前端框架版本在开发阶段按 lockfile 固定。
- 当前 `adm-new-*` 初始 crate 是否重命名，交由原子计划决定；设计默认延续 `adm-new-*` 命名，避免无价值迁移。

next_read_targets=

- `NEWrust/Cargo.toml`
- `NEWrust/crates/adm-new-contracts/src/lib.rs`
- `NEWrust/crates/adm-new-governance/src/lib.rs`

## 1. 分层目标

```text
Web UI
  -> Tauri commands
    -> application services
      -> domain services
        -> repositories / filesystem adapters
          -> project workspace data
```

硬边界：

- Web UI 不直接读写 `outputs/`、`saves/`、`source_artifacts/`、`patches/`、`knowledge/sdks/`。
- Tauri commands 只做参数校验、调用 service、返回 typed response。
- application services 负责事务、锁、validation、evidence、event log。
- domain services 负责纯业务规则，可单元测试。
- repositories 负责文件布局、atomic write、path safety、schema version migration。

## 2. 产品任务区

NEWrust 首屏仍是工作应用，不做 landing page。

| 任务区 | Web route/view | Rust service |
| --- | --- | --- |
| 设计工作台 | `/design` | `DesignWorkbenchService` |
| 开发流水线 | `/pipeline` | `PipelineService` |
| 补充开发 | `/patch` | `PatchService` |
| 打包阶段 | `/package` | `PackagingService` |
| 运行日志 | `/logs` | `RunLogService` |
| SDK 知识库 | `/sdk` | `SdkKnowledgeService` |
| AI 配置 | modal/dialog | `AiConfigService` |

Main shell 只保存当前 route 和状态栏 view model。

## 3. 数据根与项目根

Python 的 `.project_root` 定位需要迁移为 Rust `ProjectRootResolver`：

```text
ProjectRoot
├── knowledge/
├── settings/
├── drafts/
├── saves/
├── sandbox/
├── logs/
└── outputs/
```

Rust 不假设 cwd 等于项目根；Tauri 启动时必须 resolve root 并注入 `AppContext`。

## 4. 运行时事件

所有服务返回三类信息：

- `data`：UI 所需 view model 或 domain payload。
- `evidence`：写入了哪些文件、哪些 validation report。
- `diagnostics`：警告、blocked issues、runtime status。

失败不直接吞掉：

- validation failure -> typed error + evidence report。
- backend unavailable -> typed error + runtime event。
- lock conflict -> typed error + lock owner info。
- schema migration required -> typed error 或自动 migration evidence。

## 5. 开发顺序

```text
contracts
  -> repositories
  -> domain services
  -> application services
  -> Tauri commands
  -> web view models
  -> UI components
  -> Playwright parity gates
```

禁止先做 UI mock 再补业务。
