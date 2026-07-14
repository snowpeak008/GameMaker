# AI 配置 v2 架构

最后更新：2026-06-27

## 当前结论

AI 配置统一迁移到 `settings/ai_config.json`。该文件本地敏感、gitignored，负责同时管理 Profile、Adapter、LLM 和图片生成配置。

## 关键文件

- `core/config/ai_config.py`：v2 数据类、加载保存、默认 Profile、激活 Profile
- `core/config/validator.py`：Profile 验证、CLI 可用性检测
- `tools/config/migrate_ai_config.py`：从 `ai_profiles.json`、`api_config.toml`、旧 `app.toml [model]` 和 `project_settings.pipeline_adapter` 迁移
- `core/config/loader.py`：提供 `get_active_ai_profile()`、`get_pipeline_adapter()`，并保留废弃的 `get_api_config()` 兼容层
- `core/ui/ai_config_unified_dialog.py`：统一 GUI 配置入口

## 行为规则

- 新代码选择 AI 适配器时优先使用 active AI Profile。
- `project_settings.json::pipeline_adapter` 仅作为无 `ai_config.json` 时的旧版回退。
- 图片生成唯一开关是 active Profile 的 `image.enabled`。
- `settings/api_config.toml` 仅作为旧版迁移和 `get_api_config()` 兼容回退来源。
- 真实密钥只允许存在于 `settings/ai_config.json` 或旧版本地 ignored 配置中，不提交。
