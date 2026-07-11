# Fixplan Phase 0 基线资料

本目录是总计划 Phase 0（BASE-01～BASE-05）的共享评审与测试资料，不参与产品运行时，也不读取真实 `settings/`、`drafts/` 或 `saves/`。

## 内容索引

- `audits/current-call-chain-and-persistence.md`：项目配置、AI 配置、流水线状态与 Step07 的当前真实调用链、schema、读写者和可见面。
- `adrs/`：后续开发必须遵守的六项架构决策。
- `fixtures/`：Rust 与 Node 均可读取的相对路径、空凭证测试数据。
- `baseline/`：2026-07-10 历史节点与 `2026-07-11-final.md` 最终实现/发布证据。
- `terminology/`：场景化术语表和机器可读规则。
- `scripts/`：fixture 校验、PNG fixture 生成、阻断式术语扫描及其测试。

## 安全边界

1. fixture 不得包含真实 API Key、环境变量值、CLI stderr、用户目录或盘符绝对路径。
2. 所有路径以 fixture 根为基准；测试必须把 fixture 复制到临时数据根后运行。
3. `fixtures/ai/ai-config-v3.json` 中的扩展字段放在 `extra_json` 内，这是当前 v3 能稳定 round-trip 的扩展边界；顶层未知字段当前没有保留承诺。
4. PNG 是确定性测试资产，可用生成脚本重建，不代表产品图片质量。
5. 术语旧命中已清零并完成评审；术语扫描现在默认阻断，`--strict` 仍作为显式阻断参数保留。
6. 项目/编辑器路径属于机器绑定；fixture 只使用相对路径，portable 对外候选必须使用空 `user_data`。

## 轻量校验

在 `NEWrust` 下运行：

```powershell
node testdata/fixplan/scripts/generate-png-fixtures.mjs --write
node testdata/fixplan/scripts/verify-fixtures.mjs
node testdata/fixplan/scripts/terminology-scan.test.mjs
node testdata/fixplan/scripts/terminology-scan.mjs
```

最后一个命令默认阻断不合规术语。也可显式运行：

```powershell
node testdata/fixplan/scripts/terminology-scan.mjs --strict
```
