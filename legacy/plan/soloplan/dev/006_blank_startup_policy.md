# 006 默认空项目启动策略

## 目标

修复 NEWrust 在应用启动时自动恢复上一次草稿或当前存档的问题。产品默认每次启动进入新的空白、未绑定项目；历史存档和旧草稿保留，用户仍可通过存档管理器显式打开。

## 触达范围

- `NEWrust/apps/desktop-tauri/src/runtime.rs`
- `NEWrust/apps/desktop-tauri/src/lib.rs`
- `NEWrust/apps/desktop-tauri/src/commands/mod.rs`
- `NEWrust/crates/adm-new-tauri-commands/src/shell.rs`
- `NEWrust/web/src/main.js`
- `NEWrust/apps/root-launcher/`
- `NEWrust/AutoDesignMaker.exe`（生成入口，不纳入源码版本控制）
- `NEWrust/tools/portable/Start-AutoDesignMaker.cmd`
- 相关 Rust/前端测试

## 原子工作

1. 定义 `Blank` 与 `Restore` 两种启动项目策略，产品构造入口默认使用 `Blank`。
2. `Blank` 策略分配新的独立 desktop draft session，写入 `DesignWorkbenchService::empty_project_state()`；活动草稿保持未绑定，`list_saves.current_save_id` 为空，且不读取旧草稿或存档内容。
3. 新会话使流水线、patch、package、日志等草稿作用域数据自然为空；不得删除旧 draft 或 `saves/`。
4. 保留原 `restore_project_state` 算法作为显式 `Restore` 策略，覆盖崩溃恢复和兼容测试。
5. 后端 shell state、桌面静态配置和前端 fallback 全部将 `auto_restore_current_save` 改为 `false`，避免跨层契约互相矛盾。
6. 增加测试：默认重启为空、正式存档仍存在且可显式加载、显式恢复模式仍能恢复、并行窗口使用独立草稿。
7. 增加关闭边界测试：默认空白窗口在未加载存档时直接关闭，不得借用旧全局索引把正式存档覆盖为空状态。
8. 保持根目录稳定启动入口：原生 EXE 校验便携产品与清单、检测 WebView2、直接转发到产品 EXE；提供无启动副作用的 `--check-launcher`，不依赖根目录 CMD。

## 验收

- 在已有项目和当前存档的数据根上重新启动，设计项目名、节点状态和流水线状态均为空白初始值。
- `list_saves` 仍列出启动前的正式存档，用户显式 `load_save` 后恢复完整项目。
- 默认 shell 报告 `autoRestoreCurrentSave=false`。
- 启动过程不删除或覆盖历史 `saves/` 和旧 draft。
