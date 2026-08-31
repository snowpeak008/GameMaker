# 07 · 补充开发变更流 · SDK 知识库 · 文档集交付（T12）

> 归属：W6 返工尾段 T12「流程版图与占位」的落地设计。依据 `docs/plan/05_V2能力覆盖矩阵与W6返工设计.md`
> §2.2/§3 的能力归宿定义——V2 的「补充开发 / 打包阶段 / SDK 知识库」三项能力在 V4 必须有明确归宿。
> T11 已把顶部六视图导航与三栏工作台做齐，但这三视图当时只落了占位文本；T12 把它们做成
> 有数据模型支撑的可见/可操作视图。落地日期：2026-08-30。

## 0. 分寸（本期做到哪、留到 Phase 2）

| 能力 | 本期（T12） | Phase 2（P0-P5） |
|------|-------------|------------------|
| SDK 知识库 | 数据模型 + 三态审批流（可提交/批准/拒绝，署名必填） | 审批产物驱动构建集成（取用关卡） |
| 补充开发 | 数据模型 + 变更请求生命周期状态机 + 视图 | 「增量重跑受影响段」自动触发（现复用既有 `pipeline_rerun(from,to)`） |
| 打包阶段 | 文档集交付清点（读 C0-C6 产物、算 sha256、标完整性）+ `.adm4proj` 导出导入 | 游戏构建 / Unity 工程导出 / 运行时验证 / 发布包 |

三视图共同红线：署名+意见双必填（R3 评审工作量证明）、终态/跳级 blocked、新 serde 字段全带 `#[serde(default)]`（旧档零字节改动可读）。GUI 无业务规则（D14）：一切校验走 `AppServices`，UI 只渲染+转发，按钮可用性由后端状态推出。

---

## 1. SDK 知识库（`adm4-app/src/sdk.rs`）

**定位**：SDK 资源的登记与审批是 Phase 2 构建集成的前置——未批准的资源不得进入构建。全局共享（跨项目一致），落 `data_root/config/sdk_knowledge.json`（与 V2 `sdk_knowledge_service` 的 data_root 全局语义一致，改用 serde JSON）。

**审批状态机**（分叉终态，非线性链）：

```
Pending ──approve──▶ Approved(终态)
   │
   └────reject───▶ Rejected(终态)
```

- 只有 `Pending` 能被裁决；裁决即终态，重复审批 blocked。
- `SdkRecord` 署名四要素内联（`status`/`reviewer`/`reviewed_at`/`review_note`），不另开并行 map（吸取 F3 `NaJustification` 并行署名留幽灵记录的教训）。
- 服务方法：`sdk_list / sdk_add / sdk_approve / sdk_reject`，尾部写 RunLog 类别 `"sdk"`。

## 2. 补充开发变更流（`adm4-app/src/change.rs`）

**定位**：冻结之后的设计变更走「追加需求 → 影响分析 → 排期 → 应用」，不在冻结产物上原地改。项目内，落 `content/change_requests.json`（authoring_state.json 的兄弟文件，纳入存档指纹）。采用与 `frozen/`、`pipeline/` 产物一致的「事务外补写 + `refresh_fingerprint`」范式（变更清单不属创作态，不经 `AuthoringEngine`）。

**变更状态机**（线性主链 + 任意非终态可拒绝）：

```
Drafted ──impact──▶ ImpactAnalyzed ──advance──▶ Scheduled ──advance──▶ Applied(终态)
   │                      │                          │
   └──────────────────────┴───────── reject ─────────┴──────────▶ Rejected(终态)
```

- `set_impact` 填受影响段（严格校验为 `C0..C6` 子集，去空白/大写/去重/非空），把状态从 `Drafted`/`ImpactAnalyzed`（复评）推到 `ImpactAnalyzed`；其余状态 blocked。
- `advance` 只允许线性下一步或分叉到 `Rejected`（跳级 blocked），署名+结论双必填。
- 「增量重跑受影响段」不新造引擎：`affected_segments` 的 C0..C6 首尾在视图侧映射为对 `AppServices::pipeline_rerun(archive, from, to)` 的调用参数。**必须用 `pipeline_rerun` 而不是 `pipeline_run`**：`run_range` 对已成功段无条件跳过，用 `pipeline_run` 去重跑受影响段一段都不会真的重跑，只会静默返回原状态（F4a 修正）。`pipeline_rerun` 会连带作废该段及其全部下游的产物与人工门署名——这正是变更流需要的语义（下游文档按旧契约渲染，不作废就会出现错版组合）。
- 服务方法：`change_list / change_add / change_set_impact / change_advance`，尾部写 RunLog 类别 `"change"`。

## 3. 文档集交付打包（`adm4-app/src/deliverable.rs`）

**定位**：清点某冻结版本的 C0-C6 流水线产物，汇成带 sha256 与完整性标记的交付清单，落 `content/deliverable/v{N}/manifest.json`。

- `DeliverableManifest::build` 是纯函数（对目录路径操作，可单测）：每段读 `{stage}/document.md` + `{stage}/contract.json`，两者都在才 `present`；目录整体不存在 = 七段全缺、`complete=false`，**不报错**。
- 缺段不静默：manifest 显式列出 `missing_segments` 且 `complete=false`（R2/R6 口径）。
- 服务方法：`deliverable_package`（打包+落盘+指纹+日志）、`deliverable_status`（只读重算，视图刷新用）。
- `.adm4proj` 整包导出导入复用既有 `export_project`/`import_project`。

### 顺带修复：`import_project` 项目名双真相

导入包内 `authoring_state.json` 带导出方项目名，而 `commit_draft` 只把调用方名写进 manifest——两者从不对账。修复：归一化名先于建档，commit 后用一次 `with_project_named` 把创作态项目名归一回写（同 `project_rename` 做法），消除「manifest 名 vs 创作态名」双真相。e2e 断言：导入后 `workbench_overview().summary.project_name == 传入名`。

---

## 4. 桌面 UI 接线（`adm4-desktop`）

严格照「运行日志」范式：slint `export struct` 行模型（`SdkRow`/`ChangeRow`/`DeliverRow`）+ `in property` + `callback` → `main.rs` 的 `hook_lifecycle_callbacks` 注册 + `refresh_sdk`/`refresh_change`/`refresh_deliverable` 刷新 → `view.rs` DTO→行模型确定性装配（可单测）。按钮可用性由后端状态机推出（如变更行的「影响分析/推进/拒绝」按 `ChangeStatus` 门控），`ChangeStatus::as_token/from_token/next` 承担 UI 回调的状态字符串往返，规则仍在 adm4-app。

## 5. 与 Phase 2 的边界

打包视图下方保留 Phase 2 占位说明（不放不可用按钮）：游戏构建 / Unity 工程导出 / 运行时验证 / 发布包属 P0-P5，按设计决定 D16 本期不立项。SDK 审批产物、补充开发的增量重跑，都是 Phase 2 构建产线的输入，届时在 P 段消费。
