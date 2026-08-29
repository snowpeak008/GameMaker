# 003 设计同步分析清单

## 目标

新增显式设计同步分析，用来回答 Python 与 NEWrust 的设计数据、schema 和 artifact registry 差异，而不是依赖人工猜测或旧 parity 证据。

## 触达范围

- `NEWrust/crates/adm-new-governance/src/lib.rs`
- `NEWrust/apps/adm-new-cli/src/main.rs`

## 原子工作

1. 定义比较范围：`knowledge/design_data`、`knowledge/schemas`、`pipeline/artifact_layer`。
2. 计算每个资源树的文件数、字节数、原始 SHA-256 聚合摘要和语义 SHA-256 聚合摘要。
3. 输出 `byte_identical_files`、`format_only_files`、`identical_files`、`changed_files`、`missing_in_rust`、`rust_only` 数量；`changed_files` 只表示语义内容差异。
4. 记录前若干个差异样本，便于后续拆分精修任务。
5. 报告字段明确 `python_source_mode=explicit_audit_only`。
6. 报告状态区分 `passed`、`attention_required`、`failed`：资源内容差异是 `attention_required`，不是运行时依赖失败。

## 验收

- 当两个临时资源树相同时报告 `status=passed`。
- 当 Rust 缺文件或内容不同，报告差异计数和样本。
- 第一版仓库报告为 `status=attention_required`、`difference_count=144`，复核后确认属于换行格式误报，必须由 `dev/007_semantic_design_sync_audit.md` 修正。
- 修正后的实测语义结果：三个范围均为 0 个 `changed_files`、0 个 `missing_in_rust`、0 个 `rust_only`；139 + 3 + 2 个原始字节差异进入 `format_only_files`，顶层 `difference_count=0`。
