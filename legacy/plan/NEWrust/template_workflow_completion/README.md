# NEWrust 项目模板链路完成报告

日期：2026-07-10

## 1. 根因

“查看模板”按钮并非被 CSS 遮挡或禁用。旧实现只在点击后写入“暂无可选模板”状态，没有弹窗、模板列表或后端命令；“另存为模板”也只计算文件名和哈希，没有落盘。此前 A32 的完成结论覆盖了请求构造器和静态标记，但没有覆盖真实按钮点击和文件生命周期，因此遗漏被门禁掩盖。

## 2. Python 与旧 NEWrust 差异

| 范围 | Python | 修复前 NEWrust | 当前 NEWrust |
| --- | --- | --- | --- |
| 查看界面 | 模态列表与详情 | 点击占位提示 | 双栏浏览器、窄屏上下布局、键盘选择 |
| 列表数据 | 扫描完整 JSON | 无命令 | 只返回元数据与告警，不传完整状态 |
| 套用模板 | 完整替换、确认、去除访谈、标准化 | 接收客户端整份状态 | 仅传 ID，服务端读取、推断、标准化、自动保存 |
| 自定义保存 | 写入 draft workspace | 只算 ID/哈希 | 原子写入当前 session draft |
| 覆盖保护 | 内置不可覆盖，自定义需确认 | 无 | 后端强制保护，前端二次确认 |
| 删除 | 只删自定义 | 无 | 只删自定义，内置按钮禁用且后端拒绝 |
| 损坏 JSON | 单文件可阻断列表 | 不适用 | 跳过单个文件并返回 warnings/日志诊断 |
| 存档恢复 | workspace 随存档 | 未接模板目录 | 正式存档归档、恢复、重启后可继续 list/apply |

## 3. 数据与存储契约

模板文件继续使用 Python 兼容结构：

```text
schemaVersion
template      # id/source/name/gameName/targetScale/summary/analysis/verification...
projectState  # 完整项目设计状态
```

浏览命令只输出 `template` 的轻量摘要。套用时 Rust 根据 `template_id` 重新读取权威文件，忽略兼容请求中可能出现的旧 `project_state`。自定义模板写入：

```text
drafts/<desktop-session>/workspace/projects/templates/custom_<scale>_<name>.json
```

保存时移除 `aiInterview`，目标规模写回 `projectState.profile.targetScale`。五种规模、模板名/ID/前缀长度、控制字符和目录边界均由后端验证。

## 4. 存档语义

- 当前草稿直接持有自定义模板。
- “另存为副本”和正式存档同步会包含完整 `workspace`，因此保留模板。
- 加载正式存档会恢复其中的模板；集成测试覆盖删除草稿副本后从正式存档恢复并重启继续套用。
- “新建项目存档”按既定存档语义清空 workspace，因此不会继承旧项目的自定义模板。
- 套用模板只更新草稿自动保存，不隐式创建正式存档；用户仍可从已有存档恢复套用前设计。

## 5. 错误与语言

模板命令使用稳定错误码：`TEMPLATE_NOT_FOUND`、`TEMPLATE_BUILTIN_CONFLICT`、`TEMPLATE_ALREADY_EXISTS`、`TEMPLATE_DELETE_FORBIDDEN`。Web 按当前语言显示这些错误。内置模板浏览时，中文模式采用中文名/摘要和中文分析说明，英文模式采用 `gameName` 与英文分析；原始 JSON、用户自定义名称和项目正文不被改写。

## 6. 自动验证

- Rust：`cargo fmt --all -- --check`、`cargo check --workspace --locked`、`cargo test --workspace --locked`。
- Python 模板基线：7/7。
- Web：unit、e2e、2489 对称语言键、两语言各 12 个界面纯度检查。
- UI：56 个桌面/窄屏截图；覆盖查看、键盘选择、取消套用、确认套用、取消删除、确认删除、取消覆盖、确认覆盖、空名称和忙碌态。
- UI baseline：93/93。

## 7. 恢复后的最终收尾

- 最新便携版位于 `NEWrust/dist/AutoDesignMaker-NEWrust/`。
- `AutoDesignMaker.exe` 为 21,350,912 bytes，SHA-256 为 `0e691d579d47783c85def909378d77c38d52295730daaed89b1325e5c8ba3d75`，与 `build-manifest.json` 一致。
- 便携目录的 156 个设计数据文件与源数据按相对路径、长度和 SHA-256 全部一致。
- 真实 Tauri 记录覆盖模板浏览、另存模板、输入确认和重启后恢复；四次启动均正常关闭，自定义模板在重启后仍可列出和套用。
- 当前无 NEWrust 进程，流水线状态为 `idle`，没有 pending journal、corrupt、quarantine 或 tombstone 残留。
- 恢复现场后重新执行 `release-gate` 与 `final-handoff-v3-gate`，两者均为 `passed` 且 blocker 为 0。

## 8. 非阻断后续

- 真实 Unity、live AI provider 输出质量和 Step00-14 生成制品质量仍按约定由目标环境验收。
- 长期备份轮换和专用恢复 UI 仍属于未来加固，不是模板链路交付阻断项。
- 严格 `cargo clippy --workspace --all-targets --locked -- -D warnings` 仍会命中旧 foundation 代码的一项 `trim_split_whitespace` 提示；本轮格式、check、workspace test、release gate 与模板相关门禁均已通过。
