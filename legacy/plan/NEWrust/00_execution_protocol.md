# NEWrust v2 执行协议

## 1. 当前阶段顺序

```text
Python 解构 -> Python 解构评分 -> NEWrust 详细设计 -> 设计评分 -> 原子计划 -> 原子计划评分 -> 开发
```

任何工作开始前都必须确认自己处于哪个阶段。

## 2. 每小阶段结束动作

每完成一个小阶段，必须：

1. 回读 `plan/NEWrust/README.md`。
2. 回读当前阶段目录下的 `scorecard.md`。
3. 检查是否发生任务偏移。
4. 更新当前阶段文档。
5. 记录 `plan_reread`。

## 3. 任务偏移判断

以下情况视为偏移：

- 在 Python 解构未合格前开始 NEWrust 详细设计。
- 在 NEWrust 详细设计未合格前拆原子任务。
- 在原子计划未合格前继续开发功能。
- 在 service command 前实现 UI。
- 把旧 `RUST/` 当成继续修补对象。
- 把未引用 Python 垃圾内容纳入核心复刻。

## 4. 输出格式

每个阶段文档必须写明：

```text
evidence=
classification=
confidence=
open_questions=
next_read_targets=
```

## 5. 修改纪律

- `plan/NEWrust` 是设计和计划事实源。
- `NEWrust` 是新开发事实源。
- 旧 Python 项目只读，除非用户明确批准修改。
- 旧 `RUST/` 不修改。

