# 可复制 AI 记忆系统

这个目录是从当前项目的 `knowledge/ai_memory/` 与 `tools/memory/` 机制抽出来的通用版本。

## 复制方式

把 `COPY_ME/` 目录里的所有内容复制到目标项目根目录：

```text
目标项目/
  AGENTS.md
  AI_README.md
  knowledge/ai_memory/
  tools/memory/
```

如果目标项目已经有 `AGENTS.md` 或 `AI_README.md`，不要直接覆盖。把本包对应文件里的“AI 会话记忆”段落合并进去即可。

## 启用后怎么用

1. 会话开始时，AI 先读 `AI_README.md`，再读 `knowledge/ai_memory/INDEX.md`。
2. 第一次接入项目后，编辑 `knowledge/ai_memory/project_understanding/memory_config.json`，填入真正关键文件。
3. 运行：

```bash
python tools/memory/update_freshness.py
```

4. 后续会话开始时可运行：

```bash
python tools/memory/check_staleness.py
```

5. 会话结束时新增一条 `knowledge/ai_memory/session_history/YYYY-MM-DD-NNN.md`，同步更新 `INDEX.md`，再运行 `update_freshness.py`。

## 本包不包含什么

- 不包含当前 AutoDesignMaker 的历史会话记录。
- 不包含任何密钥、私有配置、运行产物或本地路径。
- 不强制目标项目使用某种语言或框架。

