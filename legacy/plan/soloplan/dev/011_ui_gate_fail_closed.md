# 011 UI 门禁 Fail-Closed

## 目标

保证 UI 门禁只在完整 Playwright 语义检查后通过，并在任何失败路径快速退出、释放浏览器和静态服务器。

## 原子工作

1. 移除 `ui-gate.mjs` 的系统 Chrome `spawnSync` 降级截图路径。
2. Playwright 模块缺失时在启动服务器前报出 `npm ci` 修复提示。
3. Chromium 启动失败时直接失败，不生成弱化证据。
4. 浏览器和静态服务器放入 `try/finally`；页面继续由页面级 `finally` 关闭。
5. 为页面导航、DOM 状态和截图增加有界超时，禁止无限等待。
6. 证据清单只接受 `playwright:chromium`，不声明未执行的检查。
7. 增加静态/运行门禁测试，确认脚本不再包含同步浏览器 fallback。

## 验收

- 未安装依赖时快速失败，不产生孤立 Chrome/Node 进程。
- 正常环境仍生成 2 语言 × 15 状态 × 3 视口的 90 张截图。
- 93 条 UI baseline 继续通过。

## 实施结果（2026-07-14）

状态：**已完成**。同步 Chrome fallback 已移除；依赖、浏览器、导航、动作和截图失败均有界退出；浏览器与服务器统一清理。实际门禁完成 90/90 张 Playwright 截图，93 条 UI baseline 通过。
