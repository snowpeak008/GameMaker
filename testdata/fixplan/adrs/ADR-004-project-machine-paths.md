# ADR-004：项目逻辑配置与机器路径分层

- 状态：Accepted
- 日期：2026-07-10

## 决策

1. 项目/存档文档只保存引擎、required editor version、binding ID 等可迁移逻辑元数据。
2. Unity 项目目录、编辑器程序和未来 Provider/CLI 配置目录属于机器级 binding，不进入可分享项目文档。
3. 有效设置由逻辑文档 + 当前机器 binding 在 application 层合并；Web 不自行识别引擎或拼路径。
4. binding 缺失、路径失效、项目移动或换机时进入明确 relink 状态，不删除逻辑项目数据。
5. 路径可以在表单中明确显示给当前用户；日志、截图、项目导出和共享 fixture 必须脱敏或使用相对合成路径。

## 后果

- `project_config.json` 不得含 `development_path/editor_path`。
- preflight、run context、settings snapshot 与存档同步边界必须另做泄漏审查；它们不能因为是“内部文件”就默认可分享。
- 自定义引擎允许手工兜底，不能套用 Unity 的强校验。

