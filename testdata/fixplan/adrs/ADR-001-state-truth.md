# ADR-001：状态真相与用户可见投影

- 状态：Accepted
- 日期：2026-07-10

## 决策

1. 持久化 repository/domain state 是耐久真相；运行中的共享 run context/stop token 是实时真相；DOM、表单草稿、view 和日志都只是投影。
2. 同一概念只允许一个写入入口。兼容字段必须由主字段确定性派生，不能反向形成第二 active/current 状态。
3. Pipeline 用户可见投影采用白名单：状态、消息、errors/warnings、语义质量和明确设计的步骤专用视图。generic outputs、artifact、manifest、路径和 checkpoint 属于内部数据。
4. UI 可以暂存后端返回的内部字段用于专用能力，但不能把它们递归或通用地渲染到 `textContent`。

## 后果

- `activeProfileId` 只能由 dev active 派生。
- recovery summary 可见，但 checkpoint path 不可见。
- Step07 图片预览必须走受控专用 loader，不能恢复通用 artifact browser。
- 跨层测试需要验证字段名、serde casing、派生规则和 DOM 白名单。

