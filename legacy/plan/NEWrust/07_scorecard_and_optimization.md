# NEWrust v2 评分与优化规则

## 1. 合格标准

每个阶段的评分必须满足：

- 单项评分 `>=90`。
- 综合加权评分 `>=95`。
- 无硬门禁失败。
- `confidence != low`。

这替代旧规则“每项都必须高于 95”。

## 2. 通用评分字段

每个 scorecard 条目必须包含：

```text
role=
area=
score=
weight=
confidence=
evidence=
issues=
required_action=
```

## 3. 多角色

- Python Archaeologist
- Product Parity Reviewer
- Data Contract Architect
- UI Reproduction Reviewer
- Rust Architecture Reviewer
- QA Release Reviewer
- Red Team Reviewer

## 4. 硬门禁

以下任一项出现，阶段不合格：

- 没有入口证据却列为必须复刻。
- 把 quarantine 内容纳入核心设计。
- UI 互动没有追到后端行为。
- 数据写入路径不清楚。
- mock/fake/static 证据被当成 real。
- Web UI 设计绕过 Rust service。
- release manifest hash 与实际构建物不一致。
- `confidence=low`。

## 5. 循环规则

不合格时必须执行：

1. 标记低分原因。
2. 回到对应 Python 代码或设计文档重读。
3. 修改拆解或设计文档。
4. 写入本轮修改记录。
5. 重新评分。

评分不能只修改数字，必须有文档或证据变化。

## 6. 阶段评分状态

当前状态：

- Python 解构：未开始。
- NEWrust 详细设计：未开始。
- 原子开发计划：未开始。

下一步：执行 `python_deconstruction/01_source_authority_index.md`。

