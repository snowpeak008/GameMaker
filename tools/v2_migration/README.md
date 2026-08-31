# W6 T10 二版知识库迁移工具（一次性，不进构建）

`migrate_v2_knowledge.py` 把二版 `knowledge/design_data/` 全量迁成四版 `V4/knowledge/design_space/`
静态 JSON。**交付物是 JSON 产物，脚本只是可复现的生成器**：不在 cargo workspace 内，不参与构建与测试。

```powershell
python V4\tools\v2_migration\migrate_v2_knowledge.py          # 生成/刷新产物（需要二版源数据）
python V4\tools\v2_migration\migrate_v2_knowledge.py --stamp-origin   # 只补/刷模板的迁移登记
powershell -ExecutionPolicy Bypass -File V4\tools\v2_migration\verify_migration.ps1   # 机器校验
```

脚本幂等：重复执行会按同样规则重写 `domains.json` / `v2_checklist.json` / `references/*.json`；
对 `core.json` 与两个 `pack.json` 的就地补丁在检测到已有 `"node_id"` 时跳过（不会重复插入）。

## F4d 增补：批量迁移登记（认证证据）

25 份内置模板是本工具**直接写死 `certification.status=certified`** 落盘的，不经
`TemplateLibrary::certify`，因此不受认证证据关卡约束。四版把**取用侧**（预填/对照）也
改成查证据后，它们必须自带可核对的登记，否则整批失效；反之，手工塞进 `references/` 的
`certified` JSON 因为没有登记而被拒——这才堵住了「认证流程形同虚设」的旁路。

每份模板的 `origin` 因此写成 `bulk_migration` 变体：

| 字段 | 含义 | 怎么核对 |
|------|------|---------|
| `batch_id` | 迁移批次（同批共享） | 人读，对账「哪一批、什么时候」 |
| `tool_version` | 迁移工具版本 | 人读，产物归因到具体代码 |
| `source_ref` | 二版源文件路径 | 人读，回到 `knowledge/design_data/project_templates/` 逐条核对 |
| `answers_digest` | 答卷结构指纹（sha256） | **机器重算**：Rust 侧 `Template::answers_digest()` 现算后逐字比对 |
| `migrated_at` | 迁移时刻 | 人读 |

`answers_digest` 的 canonical 形态由 `answers_digest()`（Python）与 `Template::answers_digest()`
（Rust）两侧共同实现，必须逐字一致：每条答卷一行、制表符分隔
`decision_id \t option_id \t 附加选项id(逗号连接) \t 主选id(无则空)`、行尾 `\n`，
整段 UTF-8 取 sha256 并加 `sha256:` 前缀。**只覆盖答卷结构**（答了哪些点、选了哪些选项、
谁是主选），不覆盖 `parameters` / `evidence`——那两者的 canonical 形态依赖 serde 的枚举表示，
跨语言逐字节复现会变成隐式契约，一改序列化就静默失配。因此它叫 digest（结构指纹）而不是
内容哈希，能核对的范围到此为止。

`--stamp-origin` 模式只给已落盘的 `universal/references/*.json` 补/刷这个 `origin` 块
（登记块由与生成路径同一个 `build_origin()` 产出，指纹按文件里的答卷现算），
不读二版源数据、不动其它任何文件；已是最新则字节不变（幂等）。
为什么单独一个模式：全量重跑会重写 4.7MB 清单并就地改 `pack.json`，只为补一个键去动那些
文件，风险与收益不成比例。

## 产出

| 路径 | 内容 |
|------|------|
| `universal/domains.json` | 16 领域（order 1..16 取自 `domain_order.json`）+ 104 节点 |
| `universal/v2_checklist.json` | 2575 个决策点（515 检查单项 × L4 选项组 + 1 个玩法系统范围点） |
| `universal/references/*.json` | 25 份二版内置模板（Certified，批量迁移通道 + 迁移登记，见下节） |
| `skin_wordlist.json` | 50 个换皮词（25 模板游戏名 + 中文别名） |
| `universal/core.json` | 只读（不由脚本生成）：脚本读它核对画像映射表指向的点/选项是否存在 |
| `lane_defense/pack.json`、`grid_strategy/pack.json` | 就地补 `nodes`（4 / 7 个品类专属节点）与每个决策点的 `node_id` |
| `unmigrated_report.json` | 未迁移答案清单 + 未迁移规则清单（机器可读） |

## 映射规则（终稿见任务报告）

- 决策点 id：`v2.<二版节点id>.<检查单项id>.<选项组id>`。
- level：`meta_planning` / `system_concrete` 节点的首组 L3、其余组 L4；`content_concrete` 全部 L4。
- unlocks：每个领域内按「节点拓扑序 × 检查单项序 × 组序」拉一条顺序链，前一点的**每个**选项都
  unlock 下一点；每个领域的入口点是 `requirement=baseline` 的根点（恒适用、可用理由码豁免）。
- L4 选项统一声明 `compiler_tags.spec_role=profile`：二版检查单没有效果语义，按 R2 不发明
  `effects_template`，答案在 C0 落进 GameSpec 的设计意图档案而不是机制。

## F3 增补（模型缺口修复后的迁移语义）

- **非必做（`requirement=optional`）**：二版选项组的 `required=false` 现在如实迁成
  `PointRequirement::Optional`（此前该字段被整体丢弃）。领域入口点只从**必做**点里挑
  （非必做点当入口会让整域默认不进分母）。报告里新增 `requirements` 分布。
  当前二版数据里 2574 个选项组全是 `required=true`，因此实测 optional 计数为 0——
  语义通道已打通，数据侧暂无非必做项。
- **多选答卷**：`TemplateAnswer` 已支持 `additional_options` + `primary_option`，
  检查单多选组与玩法系统范围点的附加选项如实落盘（此前只留主选、其余进未迁移清单）。
  单选组收到多个已选时仍只保留主选，并逐条进未迁移清单。
- **画像字段**：`PROFILE_ANSWER_MAP` 覆盖二版全部可无损映射的 `profile.*` 取值
  （对应 `universal/core.json` 里 F3 新补的选项与 8 个 `optional` 画像点）。
  `referenceGame` / `referenceArchetype` **故意不迁**：把参考游戏名做成选项或答卷内容
  等于把换皮词写进设计空间，违反 R5。
- 未迁移条目：330 → 75（50 条参考游戏名 + 25 条节点文本）。
