# 001 独立性边界门禁增强

## 目标

扩展 `standalone_boundary_gate_report`，从少量硬编码文件检查升级为多文件源树扫描，阻断 NEWrust 源码、脚本和配置中的隐式 Python 父项目路径依赖。

## 触达范围

- `NEWrust/crates/adm-new-governance/src/lib.rs`
- `NEWrust/crates/adm-new-foundation/src/source_root.rs`
- `NEWrust/apps/desktop-tauri/src/design_specs.rs`（边界复核，不需要改动）
- `NEWrust/README.md`

## 原子工作

1. 定义可扫描根、扩展名和跳过目录。
2. 扫描 `apps`、`crates`、`tools`、`web/src`、`web/scripts` 以及关键根配置文件。
3. 扫描扩展名覆盖 `*.cmd`、`*.css`、`*.html`、`*.js`、`*.json`、`*.md`、`*.ps1`、`*.rs`、`*.toml`、`*.ts`、`*.tsx`、`*.yaml`、`*.yml`。
4. 对运行时代码和脚本阻断父级 Python 根、`../knowledge`、`../pipeline`、`../saves`、`../drafts`、`../sandbox`、`AutoDesignMaker\\knowledge` 等依赖模式。
5. 跳过 `.git`、`.vite`、`dist`、`gates`、`node_modules`、`target`、`test-results`，避免历史报告、发布产物和依赖目录误报。
6. 在 gate report 中输出扫描根数量、扩展名数量、扫描文件数、命中数、跳过目录。
7. 复核 debug-only `ADM_NEWRUST_SOURCE_ROOT`：必须通过 Rust v2 完整项目身份校验；增加非 Rust 项目标识拒绝测试，并明确 release 不使用该覆盖路径。
8. 复核旧格式存档读取：只允许用户显式加载 Rust 数据根内的导入归档，不允许访问 Python 项目目录。

## 验收

- 人为构造的父级路径 fixture 会触发 blocker。
- 当前 NEWrust 源树运行门禁不因历史文档文本误报。
- 当前报告实测：`boundary_scan:file_count=213`、`boundary_scan:forbidden_hit_count=0`。
- 非 Rust `projectId` 的外部源码根在读取资源前失败；父级 Python 根不满足 Rust v2 身份契约。
