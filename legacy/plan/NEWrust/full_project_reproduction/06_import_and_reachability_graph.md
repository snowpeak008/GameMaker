# Import and Reachability Graph Plan

状态：第二轮索引已刷新。静态、动态和孤儿裁决文档已按 v3 直接源码范围同步。

## 1. 目标

建立 Python 全项目可达性图，不再只依赖人工阅读主入口。

必须覆盖：

- 静态 `import` / `from import`。
- `importlib` 动态加载。
- `pipeline/_registry.json` 动态 stage plugin。
- GUI callback 入口。
- pytest 入口。
- tools CLI 入口。
- scripts/migration/build 工具入口。
- knowledge/ucos scripts 入口。

## 2. 输出

后续必须生成：

- `06_import_graph_static.md`
- `06_dynamic_reachability_index.md`
- `entrypoint_index.md`
- `dynamic_loading_index.md`
- `unreachable_but_tested_index.md`
- `06_orphan_file_decision.md`

## 2.1 第一轮静态索引结果

已生成 `06_import_graph_static.md`。

| 指标 | 数量 |
| --- | ---: |
| Python file count | 379 |
| Parse error count | 0 |
| Entry candidate count | 161 |
| Static orphan candidate count | 71 |

结论：

- 入口候选明显多于 v2 计划覆盖的主 GUI/pipeline 入口。
- 2026-07-09 第二轮刷新：索引使用直接源码范围扫描，已纳入 `tools/build/*.py`。
- 71 个静态孤立候选不能直接 drop，已在 `06_orphan_file_decision.md` 逐项结合 disposition matrix 复核。
- 可达性矩阵当前无 pending，但最终仍需通过多角色评分和原子开发计划门禁。

## 2.2 第一轮动态索引结果

已生成 `06_dynamic_reachability_index.md`。

| 指标 | 数量 |
| --- | ---: |
| `pipeline/_registry.json` module entries | 19 |
| `__main__` entry files | 42 |
| `importlib` hits | 7 |
| `subprocess` / `Popen` hits | 98 |

结论：

- `tools/` 和 `knowledge/ucos/scripts` 不能再被整体视为 reference；其中大量文件是命令入口。
- `core/plugin_manager.py` 的动态 stage loading 是全项目复刻硬边界。
- subprocess 行为必须迁移到 Rust service/CLI/gate，不允许悄悄丢弃。

## 2.3 第一轮孤立候选矩阵

已生成 `06_orphan_file_decision.md`。

| 指标 | 数量 |
| --- | ---: |
| Static orphan candidates | 71 |
| Final decisions completed | 71 |

结论：

- 没有孤立候选因“静态不可达”被自动 drop。
- 每个候选已结合动态引用、测试、CLI、工具用途和用户工作流复核。
- 该矩阵当前无 pending；下一硬门禁是多角色评分和 v3 原子开发计划。

## 3. 硬门禁

任何 Python 文件如果既没有入口证据，也没有 drop 理由，则不能被判定完成。
