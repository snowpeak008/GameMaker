# 013 Shell 状态刷新调度器

## 目标

消除固定 `setInterval` 的请求重入和隐藏窗口轮询，降低全局 Runtime mutex 与 AI 配置读取压力。

## 原子工作

1. 新增可测试的 `createShellRefreshScheduler`，使用完成后再调度的 `setTimeout`。
2. 同一时刻只允许一个 `refreshShellState`；执行中的重复触发不启动并发请求，完成后再按固定延迟调度下一次刷新。
3. `document.hidden=true` 时不执行周期请求；恢复可见时立即刷新。
4. 返回 `stop()`，清理 timer 和 visibility listener；应用生命周期正常使用该控制器。
5. 保留启动时立即刷新、存档加载后显式刷新和关闭错误覆盖逻辑。
6. 增加无 DOM 浏览器依赖的调度器单元测试。

## 验收

- 慢请求超过刷新间隔时不会出现第二个并发 Shell IPC。
- 隐藏页面不轮询，恢复后立即同步一次。
- 底部 AI、进度和系统状态的内容与原实现一致。

## 实施结果（2026-07-14）

状态：**已完成**。`createShellRefreshScheduler` 使用递归 `setTimeout`，请求完成后才安排下一轮；页面隐藏时停止安排，恢复可见立即刷新，卸载时清理 timer 和 listener。假定时器测试确认最大并发数为 1。
