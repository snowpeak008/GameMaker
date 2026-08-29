# AI 会话记忆索引

> 最后更新：初始化
> 缓存状态：未生成。首次使用请运行 `python tools/memory/update_freshness.py`。

---

## 使用顺序

1. 读取本文件。
2. 读取 `project_understanding/key_files.md`，确认当前项目的关键文件清单。
3. 如需检查缓存是否过期，运行 `python tools/memory/check_staleness.py`。
4. 缓存有效时优先使用已有记忆；缓存过期时只重读变化文件，并更新相应记忆。
5. 会话结束时新增 `session_history/YYYY-MM-DD-NNN.md`，更新本索引与 freshness。

---

## 上次会话摘要

暂无。第一次接入项目后，请把本次项目摸底结果写入 `session_history/`，并在这里放最新摘要。

---

## L1 项目理解缓存状态

| 文件 | 缓存状态 | 上次读取 |
|---|---|---|
| project_understanding/architecture.md | 待填写 | 待填写 |
| project_understanding/key_files.md | 待填写 | 待填写 |
| project_understanding/freshness.json | 未生成 | 初始化 |

---

## L2 代码惯例速查

详见：

- `code_conventions/patterns.md`
- `code_conventions/anti_patterns.md`

请在完成项目摸底后，把最重要的 5-10 条规则同步摘到这里。

---

## L3 决策记录

详见：

- `decisions/architecture.md`
- `decisions/open_questions.md`

---

## L4 待办决策

暂无。把跨会话需要继续讨论的问题写入 `decisions/open_questions.md`。

