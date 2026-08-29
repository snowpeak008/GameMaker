# 007 设计同步语义审计

## 目标

修复第一版 `design-sync-audit` 将 CRLF/LF 和文件末尾换行差异误报为设计内容差异的问题，使报告能准确回答 Rust 与 Python 设计资源是否在语义上同步。

## 触达范围

- `NEWrust/crates/adm-new-governance/src/lib.rs`
- `NEWrust/apps/adm-new-cli/src/main.rs`（仅在字段输出需要时调整）
- 相关 governance/CLI 测试

## 原子工作

1. 每个文件同时计算原始字节摘要和规范化内容摘要。
2. UTF-8 文本统一 CRLF/CR 为 LF，并忽略文件末尾换行数量差异。
3. `.json` 文件解析为 `serde_json::Value` 后序列化为稳定结构再摘要，使缩进、换行和对象键顺序不产生语义差异。
4. 报告增加 `byte_identical_files`、`format_only_files`、`rust_semantic_digest`、`python_semantic_digest`。
5. `identical_files` 表示语义相同文件总数；`changed_files` 和顶层 `difference_count` 只统计语义差异。
6. 差异样本将仅格式差异标记为 `format_only:<path>`，真实内容差异保留 `changed:<path>`。
7. 任一必需资源组缺失或为空均属于审计 `failed`，不得因双方文件数同为 0 而误判为同步通过。

## 验收

- CRLF/LF、JSON 缩进、JSON 对象键顺序和末尾换行不同均归入 `format_only_files`。
- JSON 值、数组顺序或普通文本内容变化计入 `changed_files`。
- 当前仓库报告为 `status=passed`、`difference_count=0`、`format_difference_count=144`。
- 不复制 Python 资源，不引入任何 Python 运行时依赖。
