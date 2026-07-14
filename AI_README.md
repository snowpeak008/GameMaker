# AutoDesignMaker NEWrust AI 开发入口

本目录是独立的 Rust/Tauri 项目。AI 或开发工具进入后按以下顺序读取：

1. `AGENTS.md`
2. `knowledge/ai_memory/INDEX.md`
3. `README.md`
4. `docs/independence/README.md`
5. 与当前任务直接相关的源码和测试

## 项目边界

- 产品源码、构建输入、资源、测试和运行数据必须位于本仓库或显式运行时数据目录。
- 根 `AutoDesignMaker.exe` 只启动 `dist/AutoDesignMaker-NEWrust/AutoDesignMaker.exe`。
- 不从父目录读取 Python 项目的业务数据、Schema、配置、存档、计划或测试基线。
- `knowledge/ai_memory` 是开发期静态记忆，不进入资源清单，不随产品运行时加载。

## 记忆系统

`knowledge/ai_memory` 包含两类信息：

- 当前 NEWrust 的架构、决策、开发记录和交付状态。
- 从旧 Python 项目复制的历史会话，用于追溯需求来源、已知问题和迁移背景。

旧 Python 记录是只读历史证据。若历史描述与当前 Rust 代码、测试或独立性文档冲突，以当前仓库为准，不得据此重新建立父目录运行依赖。

每次需要持久化重要开发结论时：

1. 在 `knowledge/ai_memory/session_history/` 新增会话文件。
2. 更新 `knowledge/ai_memory/INDEX.md`。
3. 更新 `knowledge/ai_memory/session_history/index.json`。
4. 运行 `tools/memory/Update-MemoryFreshness.ps1`。
5. 运行 `tools/memory/Test-MemoryFreshness.ps1`。

维护工具使用 PowerShell，不要求目标电脑或开发环境安装 Python。

## 工程验证

代码变更按风险运行 Rust workspace、Web、独立性和便携门禁。正式发布必须从干净 Git 树运行 `tools/verify-standalone.ps1`，不得用历史记忆代替当前验证证据。
