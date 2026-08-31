# V4 设计 · 06 · 存档、AI 层与 UI

> 落点 crate：`adm4-archive` / `adm4-ai` / `adm4-app` / `apps/adm4-cli` / `apps/adm4-desktop`
> 依据：第二版存档/锁/导出包设计契约（CONTEXT.md 词汇表）+ NEWrust 存档事务经验 + redesign_v3 GUI 边界

---

## 1. 数据根与存档（adm4-archive）

### 1.1 数据根布局（全新格式，不兼容旧版）

```
data_root/（默认 {cwd}/.adm4_data，可配置）
├── config/
│   ├── app.json                应用配置
│   └── secrets.json            named secrets（named: 引用的落点；env: 引用不落盘）
├── archives/<archive_id>/      正式存档
│   ├── manifest.json           { format_version:1, archive_id, project_name, created/updated_at,
│   │                             content_fingerprint }
│   ├── .lock                   存档锁（session_id/pid/created_at；同档单编辑）
│   └── content/                项目内容树（见 1.3）
├── drafts/<session_id>/        草稿工作区（自动保存；结构同 content/）
│   └── draft_meta.json         { session_id, pid, linked_archive: Option<archive_id>, updated_at }
└── logs/run_log.jsonl          结构化运行日志
```

### 1.2 存档语义（继承第二版词汇表）

- **正式存档**：显式保存动作创建/更新的权威状态；不在其上直接编辑；
- **草稿工作区**：每会话一个，所有编辑发生在此，防抖自动保存；`linked_archive` 为空 = 新项目/模板项目；
- **存档锁**：打开正式存档编辑时写锁文件；第二会话打开报错；会话结束释放；支持清理外部陈旧锁；
- **原子保存**：写临时目录 → 校验内容指纹 → 替换正式目录（同盘 rename 原子）；
- **内容指纹**：content/ 全部文件按相对路径排序，逐文件 sha256 连接再 sha256；用于退出对话框与冲突检查；
- **脏标记**：内存布尔量，任何变更置 true，保存成功清除；退出判定以内容指纹为准。

### 1.3 项目内容树

```
content/
├── project.json                项目元信息（名称/品类包/深度档/创建时间）
├── authoring_state.json        设计期权威状态（AuthoringState）
├── frozen/v{N}/                每个冻结版本（只读）
│   ├── frozen_design.json
│   └── gate_report.json
├── pipeline/v{N}/              对应冻结版本的流水线运行
│   ├── run_state.json
│   └── {C0..C6}/
│       ├── contract.json       机器契约（真相源）
│       └── document.md         渲染文档
└── deliverable/v{N}/           Phase 1 文档集交付包
```

### 1.4 导出包（.adm4proj）

单文件包：头（magic `ADM4PROJ_V1` + format_version + payload sha256 + 文件数）+ manifest + 逐文件（path + sha256 + 字节）。`package-doctor` 可离线校验。

---

## 2. AI 层（adm4-ai）

```rust
pub trait AiProvider {
    fn id(&self) -> &str;
    fn capabilities(&self) -> &[AiCapability];   // Text | Structured | Review | Image
    fn invoke(&self, req: &AiRequest) -> Adm4Result<AiResponse>;  // 失败=Err，无降级参数
}
```

- **OpenAI 兼容 HTTP Provider**（chat completions）+ preset（openai/openrouter/deepseek/local_openai）；
- **SecretRef**：`env:NAME` / `named:NAME`；named 值存 `config/secrets.json`；日志与 Debug 全脱敏；
- **预算与重试**：调用计数/token 预算；重试仅限网络错误，语义失败不重试不兜底（R7）;
- **journal**：每次调用记录（无原始密钥、可选不存原始输出）；
- **测试 Provider**：确定性回放（fixtures），单测/集成测试用，杜绝测试依赖真实网络。

AI 介入点（继承第二版「AI 介入」语义：决策点触发，非常驻）：访谈提案、冻结门红队、C1 红队、C2/C3/C4 叙述与命名、C5 生图、逆向 S2/S3。

---

## 3. 应用编排（adm4-app）

| 服务 | 职责 |
|------|------|
| `ProjectService` | 创建（选包+深度档+模板模式）/打开/保存/另存/导出/导入 |
| `AuthoringService` | 决策操作转发 + 完成度/冲突查询 + 访谈回合 |
| `FreezeService` | 五道门评估 + 执行冻结（产 FrozenDesign 新版本） |
| `PipelineService` | C0-C6 运行/区间执行/断点续跑/人工门确认/产物读取 |
| `ReverseService` | 逆向产线 S1-S5 + 审核操作 + 认证入库 |
| `RunLogService` | 结构化事件写 `run_log.jsonl` |
| `SdkKnowledgeService` | SDK 知识条目收集/审批/呈现 |
| `DeliverableService` | Phase 1 文档集打包 |

GUI/CLI 只调用这些服务，不含业务规则。

---

## 4. 桌面 UI（adm4-desktop，Slint）

顶栏任务区切换（延续第二版契约）：

```
设计工作台 | 冻结门 | 开发流水线 | 模板逆向 | 打包 | 运行日志 | SDK 知识库

> **实现对齐说明（2026-08-29 / T12 更新 2026-08-30）**：当前桌面顶部六视图为 设计工作台 |
> 开发流水线 | 补充开发 | 打包阶段 | 运行日志 | SDK 知识库（存档管理/模板逆向为设计工作台内的覆盖面板，
> AI 访谈并入设计工作台中栏）。**T12 已把「补充开发 / 打包阶段 / SDK 知识库」三视图从占位做成
> 有数据模型支撑的可操作视图**（详见 `07_补充开发与SDK知识库.md`）：SDK 三态审批流、补充开发变更流
> 状态机、文档集交付清点全部落地；游戏构建 / Unity / 运行时验证仍属 Phase 2（P0-P5）占位。
```

- **设计工作台**：左=领域列表（含各域完成度）；中=当前域决策点卡片流（L 层标、选项+implications、参数表单；L5/L6 渲染表格/矩阵编辑器；冲突高亮）；右=摘要/待填清单/冲突/访谈 Tab；
- **冻结门**：五道门状态灯 + 逐门 block 清单 + 红队发现处置列表 + 冻结按钮（全绿激活）；
- **流水线**：C0-C6 阶段卡（数据驱动 registry）；每卡状态/耗时/产物入口；C5 风格确认、C6 签收对话框；
- **模板逆向**：维护工具面板；模板列表（认证状态）、逐领域答卷过卷、证据链接、改/退回/认证；
- **打包**：文档集交付打包与校验结果；
- **日志 / SDK**：结构化列表 + 过滤 / 审批队列。日志**只读不可清空**（裁决，2026-08-31）：RunLog 是红线审计流（R3 人工门与破坏性操作署名、R5 换皮命中、冻结与重跑记录都落在这里），提供"清空"按钮等于给用户一键抹除审计证据，与 R3 直接冲突；界面明示不可清空，导出与过滤照常提供。

底栏：当前项目/存档状态（脏标记）、AI Provider 状态、后台任务进度。

## 5. CLI（adm4-cli）

```
adm4 space validate [pack]            清单加载与校验（含 <3 参考 blocked 负例）
adm4 project new <name> --pack --depth [--template <id> --prefill|--compare]
adm4 project list|open|save|export|import|doctor
adm4 authoring status|select|set-param|confirm|na   （脚本化创作，测试/自动化用）
adm4 freeze check|run                 五道门评估 / 执行冻结
adm4 pipeline run [--from C0 --to C6] | status | confirm <gate>
adm4 reverse ingest|map|crosscheck|review|certify
adm4 ai doctor|secret-set|provider-set|invoke-check
adm4 deliver package|doctor
```

> **实现对齐说明（2026-08-29）**：命令面以 `adm4-cli --help` 为准。实际实现：逆向产线为
> `template new-draft|search-corpus|map|cross-check|review|certify|compare`；预填为
> `project new --template <id>`，对照为 `template compare`；另有 `interview next|confirm|reject|progress`
> 与 `authoring set-rationale`（本节定稿后由创作/访谈设计落地）。`project open|save`、
> `ai secret-set|provider-set|invoke-check` 细分本期未实现。
>
> **T12 更新（2026-08-30）**：SDK 审批 / 补充开发变更流 / 文档集交付的服务层（`AppServices` 的
> `sdk_* / change_* / deliverable_*`）已落地并接入桌面三视图（详见 `07_补充开发与SDK知识库.md`）；
> 对应 CLI 子命令（`deliver package`、`sdk`、`change`）本期不在任务范围内，留后续补齐。
