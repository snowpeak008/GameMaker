# NEWrust Memory Tools

这些工具维护 `knowledge/ai_memory/project_understanding/freshness.json`，只依赖 Windows PowerShell 和当前仓库文件。

```powershell
powershell -ExecutionPolicy Bypass -File .\tools\memory\Update-MemoryFreshness.ps1
powershell -ExecutionPolicy Bypass -File .\tools\memory\Test-MemoryFreshness.ps1
```

`knowledge/ai_memory/project_understanding/legacy_python_freshness.json` 是旧 Python 项目复制时的历史快照，不参与当前 NEWrust 新鲜度判定。历史记忆不会进入便携产品，也不能作为父目录运行依赖。
