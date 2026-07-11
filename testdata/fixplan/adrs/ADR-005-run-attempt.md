# ADR-005：run/attempt 身份与并发

- 状态：Accepted
- 日期：2026-07-10

## 决策

1. `run_id` 标识一次逻辑流水线运行；显式恢复沿用 run ID。
2. `attempt_id` 标识一次具体执行尝试；每次恢复生成新 attempt，并记录 parent attempt。
3. 同一项目同一时间只允许一个 active run；旧 attempt 的 worker、stop token 或结果不得覆盖新 attempt。
4. state/checkpoint revision 单调递增；状态落盘与 worker 合并使用版本/CAS 或等价规则。
5. canonical range 在任何状态变更前完成解析；state、log、checkpoint 只保存 canonical stage ID。

## 后果

- “停止请求”与“最后单元完成”的竞态必须有故障测试。
- 重复 stop 幂等；旧 attempt stop 被隔离。
- 双重恢复、已完成 run、项目锁占用和 revision 冲突必须拒绝。

