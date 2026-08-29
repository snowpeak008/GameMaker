# 002 首次运行与 user_data 策略检查

## 目标

把“正式发布必须空 user_data”和“本地试用 user_data 是受保护用户状态”写入自动检查，避免再次把本地运行状态误认为 Rust 初始项目状态。

## 触达范围

- `NEWrust/crates/adm-new-governance/src/lib.rs`
- 现有 `NEWrust/tools/build-portable.ps1` 与 `verify-standalone.ps1` 作为行为基准，不优先改动。

## 原子工作

1. 在 standalone boundary gate 中读取 `dist/AutoDesignMaker-NEWrust-release/build-manifest.json`。
2. 若正式发布存在，检查 `user_data_mode=clean_release`、`user_data_files=0`、`user_data_bytes=0`。
3. 同时统计正式发布目录下实际 `user_data` 文件数和字节数，防止 manifest 与文件系统不一致。
4. 若正式发布目录尚未构建，报告 `formal_release_clean_first_run=not_built`，不阻断当前开发门禁。
5. 统计 `dist/AutoDesignMaker-NEWrust/user_data` 文件数和字节数，仅作为 local trial protected data 行输出，不阻断。
6. 明确报告 `formal_release_clean_first_run=true|false|not_built`。

## 验收

- 正式发布夹带 user_data 时 gate 失败。
- 本地试用目录存在用户数据时 gate 只报告，不失败。
- 当前报告实测：正式发布 `formal_release_clean_first_run=true`；本地试用 `local_trial_user_data:files=382`、`local_trial_user_data:bytes=18136250`。
