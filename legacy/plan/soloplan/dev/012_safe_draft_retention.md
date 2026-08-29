# 012 安全空白草稿保留策略

## 目标

使已有 `pruneDraftsKeepCount` 契约具备真实但保守的执行语义，控制默认空白启动产生的无内容草稿增长，同时不破坏恢复能力。

## 原子工作

1. 新增 `apps/desktop-tauri/src/draft_retention.rs`，从显式环境变量读取保留数量；缺省和 0 均禁用。
2. 只扫描 `drafts/desktop_<pid>_<timestamp>_<attempt>`；排除当前会话、`desktop_current`、`.transactions`、`.session_locks` 和未知目录。
3. 候选必须同时满足：未绑定存档、workspace 为 unsaved、autosave 等于规范空项目、流水线为空闲、文件布局仅含已知空白基线文件。
4. 删除前必须获取对应 session lock 的独占锁；锁竞争、损坏、未知字段或任何检查失败均跳过并报告，不视为可删。
5. 按最后修改时间保留最近 N 个安全候选，只删除更旧候选；非空和未知草稿不参与 N 的计算。
6. 运行时记录实际 keep count、删除数量和警告；Shell state 返回真实 `pruneDraftsKeepCount`。
7. 文档说明环境变量、默认关闭及安全边界。
8. 增加测试：禁用、保留数量、活动锁、非空设计、非空流水线、存档绑定、额外文件和损坏 JSON。

## 验收

- 默认启动不删除任何历史数据。
- 显式启用后仅删除已解锁、可证明为空白且超过保留数量的旧会话。
- 正式存档和所有不确定草稿保持字节不变。

## 实施结果（2026-07-14）

状态：**已完成**。环境变量 `ADM_NEWRUST_PRUNE_BLANK_DRAFTS_KEEP_COUNT` 缺省、无效或为 0 时禁用；正整数仅清理超过保留数的严格空白旧会话。测试覆盖禁用、保留数、非空设计/流水线、未知文件、绑定存档、活动锁和损坏 JSON，所有不确定候选均保留。
