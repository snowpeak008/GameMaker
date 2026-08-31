# V4 · Phase 2 · 08 · 美术线：风格锚点（设计阶段）+ 资产生产

> 立项分册 08/10。总纲见 `06`，治理见 `07`。本册是**美术线**的端到端：设计阶段的风格锚点确认门
> （选项 A，用户已定）→ Phase 2 的资产批量生产。原样吸收 py Step07（风格确认）/Step09（美术计划）
> /Step12（美术生产）与 v3 风格锚点，V4 化重构（每处优化标「【优化】」）。
> 来源：`pipeline/step_07_art_style_generation/`、`core/engines/generation.py`（4212-5144）、
> `core/art_pipeline/`、v3 `plan/newdesignplan/dev/A08a_step07_art_direction.md`。

---

## 1. 为什么风格锚点必须落在设计阶段（选项 A）

用户原则："**美术风格的确认，是在游戏设计阶段就要实际查看图片、UI 风格进行验证的。**"

理由（吸收 py/v3 已验证经验）：
- 风格是**主观口味**，只有人看真图才能定，不能等生产完才发现风格错（返工代价最大）；
- 锁定的风格锚点是后续所有资产生产的**一致性比对基准**（视觉漂移检测靠它，册 07 §3）；
- 冻结前定风格，才能让 GameSpec/冻结产物携带风格约束，两条线派生时美术线有据可依。

**落点**：V4 **设计工作台新增"美术风格锚点"门**（冻结前，≈ py Step07），不是 Phase 2 内的步骤。Phase 2 只消费锁定的 `style_anchor_set`，**绝不重造风格**。

---

## 2. 风格锚点确认门（设计阶段 · 人工门 · attended）

### 2.1 输入（硬前置）
- 品类/体验方向（L0-L2 决策）+ 资产规格草案（美术需求）+（若有）参考锚点；
- **优化【前移】** py 里 Step07 依赖 Step04 资产规格 + Step06 美术评审；V4 里风格门在冻结前，依赖设计工作台已确认的 L0-L2 画像 + 品类包声明的资产类型。

### 2.2 生成（3-5 个方向，带预览图）
- 取 N 个风格预设（清晰量产/概念绘画/高对比街机/电影写实/风格化图示，各带三色 palette），N∈[3,5]；
- 每方向生成图像提示词（项目名 + 风格方向 + 意图 + 代表资产）→ 生成预览 PNG（开图像 API 则调，否则确定性占位图）；
- 打分并标一个 `recommended`；
- **风格-原型适配报告**（`style_fit`）：如"塔防 + 电影写实 = 可读性风险"，提示但不阻断。

### 2.3 交互（用户多次查看/修改——原样保留）
- **风格网格**：3 列渲染每方向预览图 + 单选 + 描述 + 推荐/分数标签；**双击图片全屏放大**；
- **对话式改提示词**：打开提示词编辑器，用户用自然语言描述想要的风格，AI 按格式返回每个 STYLE-id 的精修英文提示词，可反复对话、选生成张数（1-5）→ 写 `prompt_override` 重出图；
- **反复重生成**：不满意就重生成，清掉未选图；
- **确认后仍可改**：已确认显示"✓ 已确认风格"摘要 + "重新选择"按钮，清确认重进选择态。

### 2.4 确认锁定（产物）
- `style_confirmation`：`{status:approved, mode:manual, selected_style_id, selected_title, selected_image_path, notes}`；**禁止 auto_accept**（attended 强制，v3 `confirm_style_anchors_attended` 拒绝自动通过，接红线 R3 署名）；
- **`style_anchor_set`**：确认的方向 + 代表资产锚点（少量，如 4 个），作为后续一致性比对基准；
- **`style_application_contract`（仅确认后才写）**：`selected_style_id` + palette + 分用途 `style_constraints`（tile/icon/ui/background/effect 的可读性/对比度/透明边距策略）——**这是风格向资产生产传递约束的正式接口**，下游据此约束，不得改。

### 2.5 存储/命名/版本
- 方案 id `STYLE-{NN}-{preset_key}`（如 `STYLE-01-readable_production`），预览图 `{style_id}.png`；
- **优化【保留"重跑即覆盖"，补一条锚点历史】** py 是重跑覆盖、无多版本目录；V4 **保留覆盖模型**，但因风格锚点是长期基准，**给已确认的 `style_anchor_set` 留一条不可变历史**（`style/anchors/v{N}/`，同冻结版本号），便于回溯"这版游戏当时锁的什么风格"——这是"真内容优化"，不改交互。

---

## 3. 美术任务计划（≈ py Step09 · 消费风格契约）

- 硬门：`style_application_contract` 与 `style_confirmation` **都必须 approved 且有 selected_style_id**，否则阻断（`STYLE_APPLICATION_CONTRACT_NOT_APPROVED`）；
- 每条美术任务的 `source_refs` 追加 `style_application_contract`，并把 `selected_style_id` 注入任务提示上下文；
- 美术任务从 GameSpec 美术线（asset_spec_set + asset_registry）派生，带稳定 `asset_id`（册 07 铁律②）。

---

## 4. 资产批量生产（Phase 2 · ≈ py Step12 · AI 全自动 + skill）

**优化【采纳 godogen"资产即可执行 skill"】**：资产不是文字需求，而是**可执行的生成命令 + 成本 + 路径 + 尺寸 + 使用者记录**。

### 4.1 AssetProducer skill（可插拔通道，D19）
```rust
pub trait AssetProducer {
    fn id(&self) -> &str;                               // "ai" | "external:<tool>"
    fn can_produce(&self, spec: &AssetSpec) -> bool;
    fn produce(&self, spec, anchors: &StyleAnchorSet, out) -> Adm4Result<AssetResult>;
}
```
- `AiAssetProducer`（本期）：图/音/UI 走 AI 生成，**锚定 `style_anchor_set`** 保风格一致；
- `ExternalToolProducer`（接缝，后置）：D19 的"本地软件"（SD/ComfyUI/DCC）——本期只留 trait + `NotConfigured` 占位（诚实 Blocked）；
- 通道选择：按 `spec.kind` + `can_produce`；都不接 → Blocked（不产占位资产，R2）。

### 4.2 资产预算门（人工门 · 首次付费确认）
- **生产前清单人工门**（署名放行要生产哪些资产，R3）；
- `asset_budget`：最大成本、**首个付费调用需确认**（避免自动烧钱）、路径规则；
- 超预算 → 人工确认（R6 基数申报同源）。

### 4.3 资产表（防漂移的核心记录 · 原样保留）
每个资产必须记录（godogen 硬要求）：
```
Name / Purpose / Runtime path / In-game size / Generation cost / Fallback / Used by
```
- **内容哈希缓存**：同 asset_spec + style 哈希命中则复用，不重生；
- 生产完回填 **AssetGenome**（册 07 §5）：设计 asset_id → 实际文件路径，且 **path = 运行时加载 path**；
- 换皮扫描（R5）+ 视觉白名单（无 `visual_form` 不产，R2）+ 基数申报（R6）。

### 4.4 一致性比对（vs 风格锚点）
- 生产出的资产与 `style_anchor_set` 做 `drift_checks`（可选 VLM 评审，证据缓存）；
- BLOCK 级漂移 → 回风格门/美术评审修正（不进后续装配）。

---

## 5. 红线映射（美术线）

| 红线 | 强制点 |
|------|--------|
| R2 未知即停 | 视觉白名单（无 visual_form 不产）；无法生产即 Blocked，不占位 |
| R3 评审工作量证明 | 风格锚点门 attended 署名；资产预算门署名 |
| R5 参考名扫描 | 资产名/元数据/提示词过 SkinScanner + 禁止词根 |
| R6 基数申报 | 资产数对照期望区间，超界人工确认 |

---

## 6. 本册落点（给册 10）

- 设计阶段风格门：`adm4-authoring` / 设计工作台（`adm4-desktop`）新增风格锚点面板（看图/改词/确认）+ `adm4-app` 风格服务；
- Phase 2 资产线：`adm4-build/art/`（`asset_producer.rs` + `budget.rs` + `cache.rs` + `genome.rs`）；
- 全部做成插件（AssetProducer 通道即插件，D24）；契约全 serde + `#[serde(default)]`。

> 优化诚实边界：**【优化】** py 的风格门在冻结后的开发流水线里（Step07）；V4 按用户选项 A **前移到设计阶段**（冻结前）。这是位置优化，交互与产物结构原样保留；代价是 V4 设计工作台要加一个门（Phase 1.x，见册 10 的 G2 波次）。
