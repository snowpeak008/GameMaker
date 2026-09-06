# T-W7-5c 断点申报 — 音游微样板（G11 计分连击 + #29 谱面时间轴）

- 开工时间：2026-09-05
- 领取人：W7 波 5 子 agent（前任断线于调研期零落盘，本次从头做）
- 基线：`cargo test -p adm4-decision` 全绿（已确认）
- **状态：已完成**（2026-09-05 同日交付，全门禁绿）

## 任务范围
1. `knowledge\systems\sys.beatmap_timeline\module.json` 入库（#29 谱面时间轴）✅
2. `knowledge\design_space\rhythm_micro\pack.json` 微样板（谱面中档 + 计分中档）✅
3. `crates\adm4-app\tests\rhythm_sample_e2e.rs` 全链 e2e（装配线 + 全链线，2 测试）✅
4. `docs\memory\w7_wave5\5c_样板验收单.md` ✅

## 关键咬合点（调研期确认，最终裁量见验收单 §一）
- `sys.scoring_combo` consumes `sys.rhythm_judgement.judgement_signal`——本模块 provides 裸名词 `judgement_signal` 逐字节对齐。
- dotted 前缀 `sys.rhythm_judgement` 是 4a2 历史命名（当时 #29 未入库）；dotted 门禁只核库内提供方，前缀模块不在库内 → 咬合走 pack noun_bindings（V6 按名词键匹配，不要求前缀=提供方模块 id）。禁改 scoring_combo，改我方侧闭合。
- scoring_combo 三事件源（rhythm/combat/match3）在 V6 下须逐名词绑定——微样板全部绑到 `chart_main.judgement_signal`（同源多绑申报，见验收单）。

## 中途撞墙记录
- **R5 换皮门**：首跑 C0 被拦——模块/pack 机制文案含「osu!/Muse Dash/Arcaea」，与 reference_games 撞词（换皮扫描器扫产物全文）。修复 = 机制口径文案中性化，商业游戏名只留 reference_games。教训已写入验收单 §六。
- **RollCheck 未交付臂**：combo_window 的 unbroken_chain 选项含 RollCheck（C4 未交付渲染臂），全链选 timed_window 绕行并申报（验收单 §四）。

## 纪律确认（全部遵守）
- 不碰 `cli_smoke.ps1`（并行卡 5b）✅
- 不改 `src\**`、既有模块、既有 pack、`docs\plan` ✅
- 不用 PowerShell 管道改源文件 ✅（源文件全部走 Write/StrReplace 工具）
- 无新增依赖、无 git 操作 ✅

## 断点记录
- [x] 基线测试全绿
- [x] 调研完成（门禁解析口径、V6 成边口径、Curve 编译通路、spire 基准形态）
- [x] 模块入库（7 决策点 / 3 档谱系 / 过 knowledge_modules 门禁）
- [x] pack 落盘（两实例 + 2 pack 点 + 反例咬合用于 e2e）
- [x] e2e 全绿（装配零悬空 + 反例 V6 点名 + 全链 C0-C6 + Curve/判定窗/供需链断言）
- [x] 全门禁：`cargo test --workspace` 全绿只增；fmt 无 diff；clippy 干净
- [x] 验收单落盘（`5c_样板验收单.md`）
