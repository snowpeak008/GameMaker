# UI Reproduction Specs

状态：第一轮 UI 优化完成。

证据文件：

- `core/ui/theme.py`
- `core/ui/main_window.py`
- `core/ui/app_window.py`
- `core/ui/pipeline_panel.py`
- `core/ui/embedded_interview.py`
- `core/ui/patch_panel.py`
- `core/ui/package_panel.py`
- `core/ui/log_panel.py`
- `core/ui/sdk_panel.py`
- `core/ui/ai_config_unified_dialog.py`

## 1. 全局视觉基线

| token | value |
| --- | --- |
| `bg` | `#F3F6F8` |
| `surface` | `#FFFFFF` |
| `surface_alt` | `#F8FAFC` |
| `border` | `#D7E0E8` |
| `border_strong` | `#A8B7C5` |
| `text` | `#15202B` |
| `muted` | `#657486` |
| `primary` | `#2563EB` |
| `primary_soft` | `#EAF1FF` |
| `success` | `#0F8A5F` |
| `success_soft` | `#E7F7EF` |
| `warning` | `#B45309` |
| `warning_soft` | `#FFF4DE` |
| `danger` | `#B42318` |
| `danger_soft` | `#FDEBEA` |
| `dark` | `#17212B` |
| `user_message_bg` | `#EFF6FF` |
| `user_message_border` | `#2563EB` |
| `ai_message_bg` | `#ECFDF5` |
| `ai_message_border` | `#0F8A5F` |
| `system_message_bg` | `#FFF7ED` |
| `system_message_border` | `#B45309` |

字体基线：

| token | Python value | Web UI 等价 |
| --- | --- | --- |
| body | Microsoft YaHei UI 10 | system CJK sans, 14px equivalent |
| small | Microsoft YaHei UI 9 | 12px equivalent |
| title | Microsoft YaHei UI 16 bold | 22px bold equivalent |
| section | Microsoft YaHei UI 12 bold | 16px bold equivalent |
| card | Microsoft YaHei UI 10 bold | 14px semibold equivalent |
| badge | Microsoft YaHei UI 8 bold | 11px semibold equivalent |

Web UI 不要求逐像素复制 Tk 字体渲染，但必须保持：

- 浅灰工作背景、白色工作面、蓝色主导航、深色状态栏。
- 业务状态色不替换：success/warning/danger/primary 的语义必须一致。
- 不使用营销化 hero、装饰性渐变和大面积卡片堆叠。

## 2. MainWindow 骨架

窗口事实：

- 标题：`AutoDesignMaker`
- 最小尺寸：`1180x720`
- 默认恢复 `settings/window_geometry.json`，否则最大化。
- 顶部导航固定，底部状态栏固定，中间内容区全尺寸叠放各面板。

顶部导航：

| 顺序 | 文本 | 激活色 |
| --- | --- | --- |
| 1 | `设计工作台` | primary bg + white text |
| 2 | `开发流水线` | primary bg + white text |
| 3 | `补充开发` | primary bg + white text |
| 4 | `打包阶段` | primary bg + white text |
| 5 | `运行日志` | primary bg + white text |
| 6 | `SDK 知识库` | primary bg + white text |

未激活：surface bg + muted text。点击切换只 lift 对应 panel，不销毁已创建 panel。

底部状态栏：

| 区域 | 行为 |
| --- | --- |
| AI status | 点击打开 AI 配置；显示 active profile 和 adapter；有效为绿色，异常为红色。 |
| progress | 点击切换 PipelinePanel；显示 `进度: passed/total`。 |
| system status | 右侧显示 `系统: 就绪` 或 `系统: 流水线运行中`。 |

刷新节奏：每 2000ms 更新一次状态栏。

关闭行为：

- pipeline running 时必须先请求停止，不直接关闭。
- design panel 存在时先 flush autosave。
- 有未保存更改时根据 save 状态弹出确认/保存/取消逻辑。
- 最终释放当前 save lock。

## 3. Design Workbench

主布局：

```text
topbar
workspace horizontal paned: left(weight=2) | middle(weight=5) | right(weight=3)
bottom statusbar
```

Topbar：

- 左侧标题区：
  - 小标题：`Commercial Game Design Decision Tool`
  - 主标题：`完整商业游戏设计决策工具`
  - 描述：16 领域、二级节点、三级 checklist、文本导出。
- 右侧 action grid：
  - 项目名称 entry width 26
  - 导出格式 combobox：markdown/json/txt/text/prompt
  - `导出`
  - `存档管理`
  - `模板查看`
  - `另存为模板`
  - `重置`

Left panel：

- 标题 `领域总览`
- 说明 `16 个领域全部保留；项目画像只影响排序和风险提示。`
- `项目画像` LabelFrame，按 profile fields 渲染 readonly combobox。
- 领域卡片滚动区。
- 领域卡片显示：
  - domain name
  - `节点 {nodePercent}% / 子项 {checklistPercent}%`
  - L4 gap/progress line
  - progressbar
  - 当前领域使用 `primary_soft`，focus domain 使用 `warning_soft`。

Middle panel：

```text
vertical paned:
  top: node list and filters
  bottom: EmbeddedInterviewPanel, minsize=200
```

Top node area：

- domain title: title font
- domain description: body muted, wraplength 760
- search entry + `搜索` + `清空`
- filter combobox：全部/已决策/未完成/有风险/不适用/L4 未完整
- scrollable node cards

Node card state palette：

| effective state | bg | border | marker |
| --- | --- | --- | --- |
| completed | success_soft | success | success |
| risk | warning_soft | warning | warning |
| not_applicable | surface_alt | border_strong | border_strong |
| selected | primary_soft | primary | primary |
| default/not_started | surface | border | surface_alt |

Node card content：

- header: node name, progress `done/total`, optional `L4 done/total`, optional `L5 entity_count`, decision state badge。
- description line wraplength 760。
- checklist item cards。
- concrete role nodes include `L5 设计实体` JSON editor with Consolas 9 equivalent, height 7。
- action row:
  - `查看描述/补充描述/隐藏描述`
  - `标记风险`
  - `此节点不适用`
- note/risk/not-applicable text boxes save on focus out。

Right panel：

- notebook tabs：`摘要`、`缺失项`、`风险`、`校验`
- text views are read-only style information panes。

Bottom design statusbar：

- dark bg, left status text。

## 4. Embedded Interview Panel

布局：

```text
current question block (top, fixed)
chat transcript (middle, expands)
input block (bottom, fixed)
```

Current question block：

- label `当前 AI 提问`
- readonly Text height 3
- bg `ai_message_bg`, border `ai_message_border`

Input block：

- dynamic hint label
- Text height 3
- Ctrl+Enter submits
- status row
- buttons：`发送回答`、`生成输出`、`标记不准`、`保存访谈存档`

Chat transcript：

- Text + vertical scrollbar。
- User header uses blue border color，AI header uses green，system header uses warning。
- Message body backgrounds：user `#EFF6FF`，AI `#ECFDF5`，system `#FFF7ED`。

Running state：

- send/output/correction buttons disabled。
- status text becomes AI 正在生成。
- render refresh every 500ms while running。

## 5. Pipeline Panel

主布局：

```text
horizontal paned:
  left sidebar width=200
  right area weight=4
right area:
  config bar fixed top
  vertical paned:
    detail
    runtime log minsize=80
```

Left sidebar：

- scrollable cards grouped by `_GROUPS`。
- group label muted small。
- StepCard per stage, status mapped from pipeline state。

Config bar：

- `项目配置`
- `AI 配置`
- `导出到流水线`
- spinbox from step
- spinbox to step
- checkbox `跳过人工确认`
- `▶ 运行`
- `⏹ 停止`

Detail panel：

- default: `点击左侧步骤查看详情`
- selected step card:
  - `步骤 NN：title`
  - `状态：status`
  - `当前引擎：engine_label`
  - `AI 适配器：adapter_label`
  - semantic quality panel
  - `▶ 运行此步骤` or `🔁 重新运行`

Step07 special UI：

- if approved style confirmation exists, show green confirmed summary and `重新选择风格`。
- otherwise show 3-column style option grid。
- image preview max 330x225 via subsample。
- double-click image opens fullscreen black background preview。
- footer includes notes Text height 3, `确认选择`, `重新生成`。

Runtime log：

- title `运行日志`
- dark text area bg `#17212B`
- text color `#D0E8C0`
- Consolas 9 equivalent。

## 6. Patch Panel

布局：

- request Text height 5。
- action row：
  - `分析补充开发（Analyze）`
  - `刷新列表（Refresh）`
  - right status label。
- Treeview height 10，columns:
  - `patch_id` width 220
  - `status` width 100
  - `tasks` width 80
  - `updated_at` width 160

行为：

- request empty -> status `请求内容为空（Request is empty）`。
- analyze running -> button disabled，status `正在分析（Analyzing...）`。
- worker calls `PatchAnalyzer.analyze()`。
- finish -> status `已分析（Analyzed）{patch_id}` and refresh。

## 7. Package Panel

布局：

- toolbar top:
  - `生成打包资料`
  - `刷新状态`
  - right status label
- output Text height 22，wrap word。

行为：

- Step14 status != success: package button disabled，status Step14 未通过。
- run package: buttons disabled，status generating。
- finish displays JSON result or error。

## 8. Log Panel

布局：

- toolbar:
  - label `级别（Level）`
  - readonly combobox width 16：ALL/DEBUG/INFO/WARNING/ERROR
  - right button `清空日志（Clear）`
- Treeview height 12，columns:
  - timestamp width 150
  - level width 80
  - source width 140
  - context width 180
  - message width 520 stretch

行为：

- filter applies immediately on combobox selected。
- clear removes in-memory entries and tree rows。
- export_jsonl writes visible stored entries to selected path when called by host logic。
- MainWindow initially loads latest 5 JSONL run logs by mtime。

## 9. SDK Panel

布局：

- top form:
  - name entry width 24
  - source URL entry width 48
  - `新增 SDK（Add）`
  - `批准入库（Approve）`
  - `标记待复核（Pending）`
  - `拒绝入库（Reject）`
  - right status label
- Treeview height 10，columns:
  - sdk_id width 140
  - name width 180
  - review_status width 130
  - source_url width 360
  - updated_at width 160
- bottom context Text height 8。

行为：

- add with empty name -> status `名称为空（Name is empty）`。
- update status without selection -> status `请先选择一个 SDK（Select an SDK first）`。
- refresh initializes knowledge base, reloads index, and renders `approved_prompt_context()`。

## 10. AI Config Dialog

窗口事实：

- Toplevel title `AI 配置管理`
- modal `grab_set()`
- fixed size centered `820x650`
- three tabs：开发API、生图API、补全API

布局：

- root padding 16/14。
- left panel width 250 with listbox height 22 and `+ 新建` / `- 删除`。
- right panel surface with border，renders type selector and conditional fields。
- footer：status label + `应用` + `保存` + `取消`。

迁移要求：

- Web UI 需要保留三分类配置模型和 active entry 标记。
- 本地 CLI 类型显示 PATH 检测结果，不要求输入 API URL/key。
- API 类型显示 API URL/API Key secret input。
- Codex file config types显示 `.toml` 和 `.json` 配置路径。

## 11. UI 验收方式

NEWrust UI 阶段必须执行：

- Playwright desktop viewport：`1366x768`、`1440x900`、`1920x1080`。
- Mobile/narrow viewport 只要求无文本溢出，不要求复刻 Tk 三栏布局。
- 截图检查六任务区、设计工作台、pipeline、Step07 风格确认、patch、package、logs、sdk、AI config。
- DOM 状态检查按钮启禁、tab active state、status text、tree column labels、form validation text。
- 关键颜色 token 检查，禁止随意换主题。
