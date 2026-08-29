# W6 T10 二版知识库迁移工具（一次性，不进构建）

`migrate_v2_knowledge.py` 把二版 `knowledge/design_data/` 全量迁成四版 `V4/knowledge/design_space/`
静态 JSON。**交付物是 JSON 产物，脚本只是可复现的生成器**：不在 cargo workspace 内，不参与构建与测试。

```powershell
python V4\tools\v2_migration\migrate_v2_knowledge.py          # 生成/刷新产物
powershell -ExecutionPolicy Bypass -File V4\tools\v2_migration\verify_migration.ps1   # 机器校验
```

脚本幂等：重复执行会按同样规则重写 `domains.json` / `v2_checklist.json` / `references/*.json`；
对 `core.json` 与两个 `pack.json` 的就地补丁在检测到已有 `"node_id"` 时跳过（不会重复插入）。

## 产出

| 路径 | 内容 |
|------|------|
| `universal/domains.json` | 16 领域（order 1..16 取自 `domain_order.json`）+ 104 节点 |
| `universal/v2_checklist.json` | 2575 个决策点（515 检查单项 × L4 选项组 + 1 个玩法系统范围点） |
| `universal/references/*.json` | 25 份二版内置模板（Certified，批量导入通道） |
| `skin_wordlist.json` | 50 个换皮词（25 模板游戏名 + 中文别名） |
| `universal/core.json` | 就地补 4 个既有 `u.*` 点的 `node_id`（id 与选项一字未改） |
| `lane_defense/pack.json`、`grid_strategy/pack.json` | 就地补 `nodes`（4 / 7 个品类专属节点）与每个决策点的 `node_id` |
| `unmigrated_report.json` | 未迁移答案清单 + 未迁移规则清单（机器可读） |

## 映射规则（终稿见任务报告）

- 决策点 id：`v2.<二版节点id>.<检查单项id>.<选项组id>`。
- level：`meta_planning` / `system_concrete` 节点的首组 L3、其余组 L4；`content_concrete` 全部 L4。
- unlocks：每个领域内按「节点拓扑序 × 检查单项序 × 组序」拉一条顺序链，前一点的**每个**选项都
  unlock 下一点；每个领域的入口点是 `requirement=baseline` 的根点（恒适用、可用理由码豁免）。
- L4 选项统一声明 `compiler_tags.spec_role=profile`：二版检查单没有效果语义，按 R2 不发明
  `effects_template`，答案在 C0 落进 GameSpec 的设计意图档案而不是机制。
