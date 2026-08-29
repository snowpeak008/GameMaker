# Garbage Isolation Draft

状态：第一轮初稿。此文档只做标记，不移动、不删除项目文件。

目标：在 Python 解构期间防止历史工程、运行时产物、临时计划、旧架构和缓存污染 NEWrust 设计。全部设计完成后，需要把本文件的临时标记收敛成最终 `drop/defer/reference` 清单；清理是文档清理，不代表删除源码。删除任何 Python 文件都必须另行确认。

## 1. 分类标准

| 分类 | 含义 | 是否可进入 NEWrust 设计 |
| --- | --- | --- |
| `authoritative` | 当前 Python 运行入口可达，或被当前权威代码直接调用 | 必须复刻 |
| `reference` | 对理解有帮助，但不是当前产品功能事实源 | 可参考，不直接复制 |
| `defer` | 可能有价值，但当前证据不足 | 后续二次审计 |
| `drop-candidate` | 缓存、构建产物、历史废弃内容、运行时输出 | 不进入设计；最终只做清单归档 |
| `generated-runtime-data` | drafts/saves/logs/sandbox/locks 等运行数据 | 可作格式样本，不作源码 |
| `target-workspace` | NEWrust 开发目标 | 不作为 Python 解构证据 |

## 2. 根目录隔离表

| 路径 | 初始分类 | 依据 | 当前处理 |
| --- | --- | --- | --- |
| `core/` | authoritative | `AI_README.md` 标注为全部运行时 Python 代码，入口链直接引用 | 继续文件级拆解 |
| `pipeline/` | authoritative | `pipeline/_registry.json` 和 `core/main.py` 动态加载 | 继续 stage/plugin 级拆解 |
| `knowledge/design_data/` | authoritative data | `data_loader.py` 读取设计域、模板、选项、schema | 保留为数据事实源 |
| `knowledge/schemas/` | authoritative data | artifact validators 和 schema refs 使用 | 保留为合同事实源 |
| `settings/` | authoritative config/reference sample | AI config、应用配置来源；敏感本地配置不入库 | 复刻 schema，不复制本机 secret |
| `tools/` | reference/defer | 维护脚本、validator/build helper；部分被流程间接依赖 | 按调用证据二次分类 |
| `RUST/` | reference | 旧 Rust/Slint 工程和验收经验，不是本轮 `Tauri + Web UI + Rust` 目标 | 只抽取失败模式和验收经验，不延续旧结构 |
| `NEWrust/` | target-workspace | 新开发目标目录，已有 Rust workspace 骨架 | 不作为 Python 功能证据；开发阶段再审计 |
| `NEWrust/target/` | drop-candidate/generated | Cargo build output | 不进入设计 |
| `plan/NEWrust/` | process-authoritative | 本轮计划、解构、评分事实源 | 每小阶段回读 |
| `plan/` 其他历史计划 | reference/drop-candidate | 被 `.gitignore` 标为临时实现计划 | 不作为产品功能 |
| `bug/` | reference | 问题材料/过程计划/评分报告 | 只作为风险背景 |
| `_archive/` | reference/quarantine | 历史档案，`.gitignore` 归档目录 | 不进入核心复刻 |
| `_trash/` | drop-candidate/reference sample | 已归档清理内容，包含旧 Step15-17、runtime 输出和 pycache | 不进入设计；必要时只抽样验证历史决策 |
| `build/` | drop-candidate | 构建产物 | 不进入设计 |
| `.cache/`, `__pycache__/` | drop-candidate | 生成缓存 | 不进入设计 |
| `drafts/` | generated-runtime-data | 当前会话 draft workspace | 仅作 save/draft 格式样本 |
| `saves/` | generated-runtime-data | 正式存档数据 | 仅作 manifest/index/workspace 样本 |
| `sandbox/` | generated-runtime-data | 运行/测试沙盒 | 仅作输出格式样本 |
| `logs/` | generated-runtime-data | JSONL 和运行日志 | 仅作日志格式样本 |
| `locks/` | generated-runtime-data | save/unity runtime locks | 复刻锁语义，不复制实例文件 |
| `AutoDesignMaker.exe` | reference binary | 用户启动器，不含依赖；Python 源入口是 `gui_app.py` | 迁移为 Tauri desktop 启动体验，不反编译 |

## 3. 文件级特殊标记

| 路径 | 初始分类 | 依据 | 迁移要求 |
| --- | --- | --- | --- |
| `core/ui/workbench.py` | defer/reference | `AI_README.md` 标注为旧桌面工作台辅助工具，当前无主入口引用 | 删除前二次审计；当前不纳入 UI 高保真复刻 |
| `core/ui/ai_interview_window.py` | authoritative/reference | 独立窗口含 UCOS bridge 调用，内嵌面板缺失该调用 | 以能力补全为准，不按 UI 外壳重复实现 |
| `core/ui/embedded_interview.py` | authoritative | 设计工作台当前内嵌体验 | 复刻内嵌交互，并补齐 UCOS bridge 行为 |
| `tools/scripts/migrate_legacy.py` | defer | 名称显示迁移用途，需按调用证据判定 | 不进入产品 UI |
| `tools/dev/scaffold*.py` | reference | 开发辅助脚手架 | 可参考工程治理，不进入产品运行 |
| `knowledge/design_data/project_templates/_archived_*` | reference | 模板历史版本 | 不作为默认模板事实源，除非 UI 可选项引用 |

## 3.1 tools/ 二级处理

| 路径模式 | 分类 | 说明 |
| --- | --- | --- |
| `tools/dev/scaffold*.py` | reference | 开发时生成步骤/脚手架，迁移为工程治理参考，不作为产品功能。 |
| `tools/scripts/migrate_legacy.py` | defer/reference | 仅在迁移或维护时可能使用；NEWrust 不把 legacy migration 做成默认产品入口。 |
| `tools/memory/*` | reference | 维护 `knowledge/ai_memory` 的会话记忆，不属于用户产品功能。 |
| `tools/build/*` | defer | 若 NEWrust release gate 需要，可参考打包流程；否则不复刻 Python 构建脚本。 |
| validator/helper scripts called by tests | reference/authoritative-test | 若被 tests 或 gate 调用，可转为 NEWrust gate 设计证据，但不进入主 UI。 |

## 4. 旧 Step15-17 处理

`_trash/20260704_pipeline_stage_restructure/` 中存在：

- `pipeline_step_15_build_package`
- `pipeline_step_16_delta_patch`
- `pipeline_step_17_migration_audit`

当前权威事实是：

- 打包阶段已经移到 `core/packaging/` + `core/ui/package_panel.py`，与开发流水线平级。
- pipeline registry 只启用 D1-D4 和 Step00-14。
- `AI_README.md` 明确“版本历史、增量补丁、项目审查/迁移审计功能当前已取消，不作为运行时入口保留”。

因此旧 Step15-17 不进入 NEWrust 主 pipeline 复刻，只能作为历史参考。

## 5. 设计污染防线

NEWrust 设计阶段必须满足：

- 每个功能都能回指到 authoritative 代码、authoritative data 或明确的用户需求。
- 来自 `RUST/`、`_archive/`、`_trash/`、`bug/` 的内容必须标注为 reference，不得伪装为 Python 现状。
- 来自 `drafts/`、`saves/`、`sandbox/`、`logs/` 的内容只能作为格式样本，不能扩展产品功能范围。
- 对 `core/ui/workbench.py`、`tools/` 等 defer 项，除非找到主入口调用或测试依赖，否则不作为复刻范围。

## 6. 后续清理动作

完成全部设计后执行文档层清理：

1. 把本文件所有 `defer` 项复核为 `reference` 或 `drop-candidate`。
2. 把 `drop-candidate` 从功能清单、UI 清单、原子开发计划中剔除。
3. 保留一份最终隔离清单，删除临时“待确认”措辞。
4. 不删除仓库文件，除非用户明确批准删除动作。
