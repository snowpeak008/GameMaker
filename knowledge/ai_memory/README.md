# NEWrust AI Memory

此目录是 NEWrust 仓库内的开发期跨会话记忆副本。

## 内容来源

- `session_history/` 保留 AutoDesignMaker 旧 Python 项目和两代 Rust 实现的历史记录。
- `project_understanding/legacy_python_freshness.json` 是复制时保留的旧 Python 关键文件哈希快照，仅用于历史审计。
- `project_understanding/freshness.json` 由 NEWrust 自己的 PowerShell 工具维护，只跟踪当前 Rust/Tauri 项目的关键文件。

## 安全边界

- 本目录不在 `knowledge/resource-manifest.json` 的产品资源组内。
- 便携构建不会复制本目录，桌面运行时也不得读取本目录。
- 历史中的 Python 路径和模块名只是记录，不是当前运行依赖。
- 不在记忆中保存密钥、令牌、个人凭据或未脱敏配置。

## 维护

```powershell
powershell -ExecutionPolicy Bypass -File .\tools\memory\Update-MemoryFreshness.ps1
powershell -ExecutionPolicy Bypass -File .\tools\memory\Test-MemoryFreshness.ps1
```

关键文件清单位于 `memory_config.json`。
