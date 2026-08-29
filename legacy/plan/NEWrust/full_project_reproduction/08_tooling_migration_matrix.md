# Tooling Migration Matrix

状态：已完成第一轮工具层逐文件裁决；后续评分如发现缺口再回读源码修正。

## 1. 范围

`tools/` 下 55 个 Python 文件不再默认视为 reference。每个脚本必须迁移或裁决。

## 2. 迁移分类

| tools area | 默认目标 |
| --- | --- |
| `tools/validators` | `adm-new-cli *-gate` 或 governance validator |
| `tools/build` | release/dist gate 或 drop |
| `tools/memory` | Rust dev CLI 或 external dev-only |
| `tools/asset_production` | Rust asset tooling 或 drop-with-reason |
| `tools/dev` | xtask/scaffold 或 drop-with-reason |
| `tools/scripts` | migration CLI 或 drop-with-reason |
| `tools/save` | save repair/audit CLI 或 drop-with-reason |
| `tools/sdk` / `tools/patch` / `tools/config` / `tools/design` | 对应 Rust service/CLI |

## 3. 硬门禁

每个 tools 脚本都必须有：

- 使用场景。
- 是否仍属于全项目。
- Rust 目标或 drop 理由。
- 测试/gate 替代。

## 4. 当前裁决摘要

| category | count | decision |
| --- | ---: | --- |
| package markers | 8 | `drop_with_reason` |
| build/dist tools | 2 | `adm-new-cli dist ...` / `xtask` |
| asset/image/audio tools | 10 | `adm-new-cli asset ...` / `adm-new-cli image ...` plus `adm-new-ai` / `adm-new-artifact` |
| config/design/template tools | 3 | `adm-new-cli config/design ...` plus `adm-new-config` / `adm-new-design` |
| dev/codegen/scaffold tools | 8 | `adm-new-cli dev/project/pipeline/unity ...` |
| memory tools | 2 | `adm-new-cli memory ...` plus `adm-new-knowledge` |
| patch/sdk managers | 2 | `adm-new-cli patch ...` / `adm-new-cli sdk ...` |
| save/maintenance repair tools | 5 | `adm-new-cli save ...` / `adm-new-cli maintenance ...` |
| migration/scripts | 6 | `adm-new-cli migrate/schema/governance/pipeline/design ...` |
| validators | 9 | `adm-new-cli validate ...` plus governance/contracts/foundation/application services |

Source of truth:

- Disposition: `03_file_disposition_matrix.md`
- Rust mapping: `04_rust_target_mapping.md`
