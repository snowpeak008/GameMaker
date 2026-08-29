# 008 根目录原生 EXE 启动入口

## 目标

同步 Python 项目的入口职责：用户在项目根目录双击 `AutoDesignMaker.exe` 即可启动 GUI，不需要 CMD，不需要知道内部产品目录，也不得读取父级 Python 项目的运行数据。

## 设计依据

- Python 项目将 `AutoDesignMaker.exe` 放在根目录，并通过根标记定位本项目；`gui_app.py` 只是同职责的源码包装入口。
- Rust 桌面产品必须与 `build-manifest.json`、`portable-resource-manifest.json` 和资源树保持同目录，不能把产品 EXE 单独复制到根目录。
- 因此根 EXE 是独立的薄启动器，产品 EXE 仍位于 `dist/AutoDesignMaker-NEWrust/AutoDesignMaker.exe`。这是入口职责同步，不是复制 Python 运行时或破坏 Rust 发布根。

## 原子开发顺序

1. 新增 workspace package `adm-new-root-launcher`，编译为 Windows GUI 子系统的 `AutoDesignMaker.exe`。
2. 仅以启动器自身目录作为 NEWrust 根，固定解析 `dist/AutoDesignMaker-NEWrust`，不搜索父目录。
3. 启动前校验 `.project_root`、产品 EXE、两份发布清单、Rust 资源清单和 artifact registry。
4. 枚举系统级、用户级和显式 fixed-runtime WebView2；缺失时写根目录诊断文件并打开该文件。
5. 强制 `ADM_NEWRUST_DATA_DIR=dist/AutoDesignMaker-NEWrust/user_data`；启动策略无显式值时固定为 `blank`，语言无显式值时固定为 `zh-CN`。
6. 直接 `spawn` Tauri 产品 EXE并转发参数，不调用 `Start-AutoDesignMaker.cmd`。
7. 复用桌面产品图标，增加路径、缺失清单、WebView2 枚举、默认环境和显式恢复环境测试。
8. 新增 `tools/build-root-launcher.ps1`：locked release 构建、x64/static CRT 检查、候选入口自检、同卷原子替换。
9. 删除源码根目录 CMD 入口；便携包内部 CMD 暂作为发布清单兼容支持文件，不再是源码根入口依赖。
10. 实际生成根 `AutoDesignMaker.exe`，运行 `--check-launcher`，双击语义启动并验证真实子进程、窗口和默认空白项目。

## 验收

- 根目录存在可双击的 `AutoDesignMaker.exe`，不存在根目录 `Start-AutoDesignMaker.cmd`。
- 根 EXE 的子进程路径严格为 `NEWrust/dist/AutoDesignMaker-NEWrust/AutoDesignMaker.exe`。
- 启动器源码与运行环境不包含 Python 依赖或父级路径搜索。
- 默认启动继续得到未绑定空白草稿；`ADM_NEWRUST_STARTUP_PROJECT=restore` 仍是显式恢复路径。
- `cargo test -p adm-new-root-launcher --locked`、安装脚本自检、实际 GUI 启动验证通过。

## 实施结果

- 根入口已生成到 `NEWrust/AutoDesignMaker.exe`，398,336 字节，SHA-256 为 `79bf065bc863cb7d010f85702582972bec6962f1555d5cc1a9bd3c4f8182955a`。
- `--check-launcher` 退出码为 0；PE 检查确认 x64 且没有动态 MSVC/UCRT 依赖。
- 根入口退出码为 0，真实子进程 PID 21204，路径为 `NEWrust/dist/AutoDesignMaker-NEWrust/AutoDesignMaker.exe`，窗口标题为 `AutoDesignMaker NEWrust` 且句柄有效。
- 新草稿 `desktop_21204_1783959319_0` 包含 103 个节点且全部为 `not_started`；设计备注、玩法系统、AI 消息均为 0；流水线为 `idle` 且阶段数为 0。
- 该验证窗口随后正常关闭，运行日志包含空白项目初始化与 `desktop runtime stopped cleanly`，当前没有遗留产品进程。
- `cargo test -p adm-new-root-launcher --locked`：6 passed；`cargo check --workspace --locked`、`cargo test --workspace --locked` 和启动器专属 Clippy `-D warnings` 均通过。
- `standalone-boundary-gate`：`status=passed`、扫描 218 个文件、`forbidden_hit_count=0`。
