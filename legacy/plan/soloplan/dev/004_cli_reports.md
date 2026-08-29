# 004 CLI 入口与报告输出

## 目标

提供可直接运行的 CLI 入口，使独立性检查和设计同步分析都能被开发者重复执行并生成稳定报告。

## 触达范围

- `NEWrust/apps/adm-new-cli/src/main.rs`

## 原子工作

1. 保持 `doctor` / `standalone-boundary-gate` 作为阻断门禁入口。
2. 新增 `design-sync-audit [--python-root DIR] [--json]`。
3. 默认从 `NEWrust` 父目录识别 Python 根，但报告中标注为审计读取，不作为运行时依赖。
4. 文本模式写入 `NEWrust/gates/design-sync-audit-gate.adm`。
5. JSON 模式输出结构化报告，便于后续工具读取。
6. `design-sync-audit` 是审计报告命令，不把 `attention_required` 当成命令失败；但根身份异常、必需资源组缺失/为空等 `failed` 状态必须返回非零退出码。
7. 默认 Python 根无法识别时命令失败并提示传入 `--python-root DIR`；显式传入的 Python 根若身份异常，则在报告中体现 `failed`/blocker，不进行隐式回退。

## 验收

- `adm-new-cli design-sync-audit` 可从 `NEWrust` 根运行。
- `--python-root` 可覆盖默认 Python 根。
- 缺少 Python 根时命令明确失败，不回退到运行时读取。
- 显式审计根身份错误或必需资源组缺失/为空时，仍输出可诊断报告，但命令返回失败。
- 文本报告实测写入 `NEWrust/gates/design-sync-audit-gate.adm`。
