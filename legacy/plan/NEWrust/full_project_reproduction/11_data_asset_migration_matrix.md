# Data Asset Migration Matrix

状态：v3 数据/配置/schema 第一轮分组矩阵。该文件补足 Python 文件矩阵之外的资产迁移规则；后续如发现新增资产目录，必须补表后再评分。

## 1. Scope Counts

当前已纳入的非 Python 资产范围：`knowledge/` 非 Python 资产 715 个，`settings/` 配置资产 12 个，合计 727 个。

| area | file count | migration target | status |
| --- | ---: | --- | --- |
| `knowledge` top-level governance docs | 6 | project documentation/governance reference; copied as docs, not runtime loader input | decided |
| `knowledge/ai_memory` | 277 | `adm-new-knowledge` import/export store; treated as project memory seed/dev knowledge, not immutable runtime fixture | decided |
| `knowledge/decisions` | 16 | design/architecture decision reference docs and JSON; docs bundle or migration evidence | decided |
| `knowledge/design_data` root files | 6 | `adm-new-design::data_loader` typed assets | decided |
| `knowledge/design_data/archetypes` | 3 | archetype requirement detector assets | decided |
| `knowledge/design_data/domains` | 16 | 16-domain design workbench/domain loader | decided |
| `knowledge/design_data/entity_schemas` | 7 | entity schema registry and validator | decided |
| `knowledge/design_data/framework_memory` | 3 | framework memory import/export store | decided |
| `knowledge/design_data/project_templates` | 80 | builtin/custom template loader; archived templates are migration evidence, not default UI list | decided |
| `knowledge/design_data/prompt_evaluation` | 15 | prompt evaluation samples/reports; reports are historical evidence, policy/sample sets are active assets | decided |
| `knowledge/design_data/prompt_framework` | 10 | prompt framework manifest/module loader | decided |
| `knowledge/design_data/templates` | 16 | shared meta-template loader | decided |
| `knowledge/governance` | 51 | governance documentation and policy reference; copied docs plus validator references where explicitly loaded | decided |
| `knowledge/market_data` | 1 | optional reference data asset; excluded from default runtime unless loader references it | decided |
| `knowledge/schemas` root files | 10 | `adm-new-contracts::schema_registry` | decided |
| `knowledge/schemas/ai_design` | 73 | AI design/art pipeline/schema validators | decided |
| `knowledge/schemas/playable_contracts` | 10 | playable contract validator registry | decided |
| `knowledge/sdks` | 2 | `adm-new-sdk` seed knowledge/import fixture | decided |
| `knowledge/skills` | 16 | `adm-new-knowledge::skill_engine` seed specs and docs | decided |
| `knowledge/ucos` non-Python JSON/.gitkeep | 97 | `adm-new-knowledge` UCOS data store/schema seed; `.gitkeep` retained only as directory placeholder intent | decided |
| `settings` active/example files | 8 | `adm-new-config` + `adm-new-ai` config loader/migrator | decided |
| `settings/.backup_20260627_123402` | 4 | migration fixture only; not runtime default | decided |

Total in this matrix: 727 files.

## 2. Runtime vs Historical Asset Rules

| rule | meaning | required Rust behavior |
| --- | --- | --- |
| active design data | domains, archetypes, options, mappings, templates used by UI/engine | embedded or copied asset with typed loader, validation and version checks |
| schema asset | JSON schema under `knowledge/schemas` | loaded by `adm-new-contracts` validators; schema path compatibility preserved |
| prompt framework asset | prompt module manifest and modules | `adm-new-design` prompt framework loader with version and missing-module errors |
| project template archive | `_archived_*` project templates | excluded from default template picker but available to migration/rebuild commands |
| prompt evaluation report | generated historical report | not part of runtime default; can be used as fixture/evidence only |
| AI memory / decisions / governance docs | project knowledge state | migrated as workspace knowledge/docs; not hardcoded into product defaults |
| UCOS non-Python state | memory/identity/skill/world model JSON and directory placeholders | loaded through `adm-new-knowledge` data store with version/default handling |
| settings active config | `ai_config.json`, `app.toml`, `project_settings.json`, examples | migrated to `adm-new-config` typed models; secrets masked in reports |
| settings backup | `.backup_*` files | migration fixture only; never loaded as active config unless explicitly requested |
| runtime sample dirs | drafts/saves/sandbox/logs/locks | format samples only; user instance data is not bundled into NEWrust |

## 3. Required Gates

- `data-asset-inventory-v3`: verifies every active asset group above has a Rust loader or explicit exclusion reason.
- `schema-registry-v3`: loads all active schema files and validates known representative artifacts.
- `template-registry-v3`: loads builtin templates, excludes archive folders from default picker, verifies `template_index.json`.
- `settings-migration-v3`: migrates legacy active configs and masks secrets in reports.
- `prompt-framework-v3`: validates prompt framework manifest/module references and fallback behavior.
- `knowledge-store-v3`: validates UCOS/AI memory/skills seed loading and explicit non-runtime documentation treatment.

## 4. Development Notes

- Do not hardcode JSON paths in Web UI. Web code must call Tauri commands or consume typed assets emitted by Rust.
- Do not bundle user saves/drafts as default data.
- Do not silently drop archived templates; mark them as archive/migration evidence.
- Do not treat generated prompt evaluation reports as active runtime policy unless a loader explicitly references them.
