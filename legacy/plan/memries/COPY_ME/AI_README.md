# AI 项目导读

> 本文件是所有 AI 助手的通用入口。进入项目后先阅读本文件，再阅读记忆索引。

## AI 会话记忆

本项目使用持久化 AI 记忆系统：

1. 会话开始时读取 `knowledge/ai_memory/INDEX.md`。
2. 若索引提示缓存有效，可直接使用 `project_understanding/`、`code_conventions/`、`decisions/` 中的理解。
3. 若缓存过期，运行 `python tools/memory/check_staleness.py` 查看哪些关键文件变化，再只重读变化部分。
4. 会话结束时，新增 `knowledge/ai_memory/session_history/YYYY-MM-DD-NNN.md`，更新 `knowledge/ai_memory/INDEX.md`。
5. 修改记忆或关键文件后运行 `python tools/memory/update_freshness.py` 更新哈希快照。

## 项目说明

请在这里补充目标项目的基本信息：

- 项目名称：
- 项目用途：
- 主要入口：
- 构建/测试命令：
- 重要目录：

## 开发规则

请在这里补充目标项目的开发规则。建议至少包括：

- 代码风格
- 测试要求
- 目录职责
- 禁止提交的文件类型
- 密钥和本地配置处理方式

