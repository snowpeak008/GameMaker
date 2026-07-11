# ADR-006：checkpoint 与显式恢复

- 状态：Accepted
- 日期：2026-07-10

## 决策

1. 恢复默认 `explicit_only`；启动、刷新和 IPC 重连只发现候选，不自动续跑。
2. checkpoint 原子写入，验证 schema、revision、run/attempt lineage、canonical range、项目/草稿/配置/计划/应用 fingerprints。
3. 已 committed/skipped 单元可复用；running/unknown 单元必须先 reconcile。无法证明安全时进入 `recovery_blocked`，不提供强制继续。
4. resume 新建 attempt，从 `next_unit` 开始；普通 run 不隐式读取旧 checkpoint。
5. Step07 `waiting_confirmation` 是人工门禁，不是 recoverable；确认按钮和恢复按钮互斥。
6. checkpoint、idempotency key 和内部 output ref 不进入 artifact UI 或普通日志。

## 后果

- whole-stage 仅适用于短且可安全整体提交的步骤。
- Step11/12 及图片调用等外部副作用必须定义稳定 unit ID、commit 与 reconcile 策略后才能宣称可恢复。
- checkpoint 缺失或损坏必须给出结构化不可恢复原因。

