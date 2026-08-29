# Full Project Scope Matrix

状态：v3 第一版，目录级矩阵已建立，文件级矩阵待 P2 深读完成。

## 1. 顶层范围

| 范围 | Python/资产数量事实 | v3 处理策略 | 目标 |
| --- | ---: | --- | --- |
| `gui_app.py` | 1 py | `implemented` | Tauri desktop 启动入口 |
| `sitecustomize.py` | 1 py | `external_dev_only` 或 `absorbed` 待确认 | Rust 启动环境不应依赖 Python path hack |
| `conftest.py` | 1 py | `test_port` | Cargo/Web/gate 测试环境替代 |
| `core/` | 245 py | runtime/test 全拆 | Rust crates + Tauri command + Web view models |
| `pipeline/` | 50 py | stage/plugin 全拆 | Rust typed stage registry 和 stage services |
| `tools/` | 55 py | CLI/xtask/gate/drop 裁决 | Rust CLI 工具链或明确废弃 |
| `knowledge/ucos/` | 26 py | 产品/外部/资产裁决 | Rust AI memory bridge 或外部开发工具裁决 |
| `knowledge/design_data/` | JSON assets | `asset_or_schema` | Rust typed loader + validation |
| `knowledge/schemas/` | JSON schema assets | `asset_or_schema` | Rust schema refs / validators |
| `settings/` | JSON/TOML config | `asset_or_schema` | Rust config schema + migration |
| runtime dirs | drafts/saves/sandbox/logs/locks | sample only | 格式样本，不复制实例数据 |

## 2. `core/` 二级范围

| 范围 | Python 文件数 | v3 目标 |
| --- | ---: | --- |
| `core/tests` | 73 | Rust/Web/gate 测试迁移矩阵 |
| `core/design` | 40 | `adm-new-design`, `adm-new-ai`, data loaders, structured handoff |
| `core/ui` | 22 | Web UI + Tauri commands + screenshot baseline |
| `core/adapters` | 17 | Rust AI adapter abstraction, provider validation, CLI subprocess policy |
| `core/engines` | 14 | Rust execution object and generation services |
| `core/runtime` | 10 | Rust runtime control, lock, run context, preflight |
| `core/art_pipeline` | 7 | Rust art pipeline stage helpers |
| `core/ai_design` | 7 | Rust AI design contracts, gates, completion service |
| `core/artifact` | 7 | Rust artifact graph/preflight/reviewer/validator |
| `core/utils` | 7 | Rust foundation utilities or absorbed helpers |
| `core/patch` | 6 | Rust patch service, manifest, validator |
| `core/config` | 6 | Rust config loader/validator/migration |
| `core/source` | 6 | Rust source package discovery/import/snapshot |
| `core/packaging` | 4 | Rust package service/validation/manifest |
| `core/iteration` | 4 | Rust iteration/delta scheduler or drop-with-reason |
| `core/sdk` | 3 | Rust SDK knowledge service |
| `core/save` | 2 | Rust save archive service |
| root modules | 10 | Rust foundation/application boundaries |

## 3. `pipeline/` 范围

所有 D1-D4 与 Step00-14 插件都进入 v3 文件级迁移。

目标不是只保留 stage 名称，而是：

- 每个 `plugin.py` 的 source groups、输入、输出、fallback、test mode、artifact 写入都要拆解。
- 每个 `helpers.py` / `contract_builder.py` / `binding.py` 都要映射到 Rust stage service 或 typed contract builder。
- prompts 和 README 作为资产或文档输入，不允许丢失。

## 4. `tools/` 范围

`tools/` 不再统一标为 reference。每个脚本必须裁决：

- `tools/validators/*`：优先迁移为 `adm-new-cli *-gate` 或 Rust validator crate。
- `tools/build/*`：迁移为 release/dist gate 或明确废弃。
- `tools/memory/*`：迁移为 Rust dev CLI，或标为外部 AI 开发流程。
- `tools/asset_production/*`：迁移为 Rust asset tooling 或明确不属于 NEWrust MVP。
- `tools/dev/*`：迁移为 xtask/scaffold，或明确 drop。
- `tools/scripts/*`：迁移为 migration CLI 或 drop-with-reason。

2026-07-09 范围修正：`tools/build/*.py` 被 `.gitignore` 的通用 `build/` 规则漏扫，但属于 `tools/` 源码命名空间，已纳入 v3 基线；`_trash/`、`sandbox/`、`build/skill/` 等垃圾或构建产物目录继续排除。

## 5. 旧 v2 完成物处理

v2 已完成代码不删除，但其状态降级为：

```text
useful_existing_implementation
```

它必须被 v3 文件级矩阵重新审计后，才能计入全项目复刻完成。
