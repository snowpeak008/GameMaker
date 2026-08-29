# Fixplan 001–008 统筹原子开发总计划

## 记录状态

- 记录日期：2026-07-10。
- 当前状态（2026-07-11）：Phase 0–7 已完成代码实现，Phase 9 的全量 Rust/Web/UI/fixture/术语门禁、两套 portable、bundle 校验和隔离 smoke 已完成，证据见 `NEWrust/testdata/fixplan/baseline/2026-07-11-final.md`。Phase 8（Provider v4、Godot/Unreal 完整扩展）依照本计划的产品确认门槛保持延期，未纳入本轮默认实现。
- 本文件是 `plan/fixplan` 下 001–008 的统一开发顺序。原文件继续保留问题证据和背景，本文件负责消除重复、解决冲突并定义实施先后。
- 额外纳入已经明确、但尚未单独进入 001–008 的需求：在 `NEWrust` 主目录提供一个稳定的双击启动索引文件。它只转交给 `dist` 中的启动器，不承载业务逻辑，也不触发构建。
- 实施纪律：一个原子任务必须形成可独立审查的完整变更，包含相应测试；不得为了追求“一任务一提交”而让主分支持续处于不可构建或不可测试状态。

## 一、统一目标

完成后，产品应同时满足以下结果：

1. `NEWrust` 主目录存在稳定、无需随代码反复重建的双击启动入口。
2. 六个主模态框只保留一个底部退出入口；根页面不再纵向滚动，长内容只在所属区域内部滚动。
3. AI 配置的当前项使用卡片右上角按钮表达，点击后立即刷新；`dev`、`completion`、`image` 三类互不串线。
4. 本地 Codex/Claude CLI、独立 API、中转 API、本地 HTTP 服务可以共存，保存、预检、实际 adapter 与重启恢复使用同一解析结果。
5. 项目目录和编辑器不再主要依赖手工复制路径；支持原生选择、项目识别、Unity 编辑器发现、失效路径重连和后续跨引擎扩展。
6. 流水线界面不显示内部产物列表、文件路径、读取按钮、原始 outputs 或 Base64 正文；Step07 显示可检查的图片。
7. Step07 能区分真实生成、明确降级和失败，不能再把 1×1 占位图冒充有效图片。
8. 流水线接受 `1`、`8`、`12` 等自然步骤输入，统一保存 canonical ID；长步骤能在安全执行单元边界停止，并在显式确认后可靠续跑。
9. 中文界面保留 AI、API、CLI、SDK、URL、ID、JSON、Markdown 等标准技术词，同时避免无差别中英替换。

## 二、审查结论与计划合并方式

| 来源计划 | 根因结论 | 统筹处理 |
| --- | --- | --- |
| 001 项目路径 | 当前把路径字符串直接当作最终配置，缺少“选择、识别、发现、匹配、重连”领域流程，且项目信息与本机工具路径混存 | 先建立通用原生路径边界，再拆项目绑定与机器解析，Unity 优先，其他引擎后置 |
| 002 AI 当前项 | 激活动作耦合在详情 checkbox 中，列表、详情和总览没有统一重绘；未明确草稿与持久化边界 | 并入 AI v3 稳定阶段，先于 Provider Registry 开发 |
| 003 AI Provider | v3 数据基本存在，但 config type、CLI、API、实际 adapter 各自解析；`activeProfileId` 可能成为第二真相 | 先建立唯一 v3 resolution，再把 Provider v4 作为显式迁移阶段；本轮不照搬完整 Launch Profile |
| 004 模态按钮 | 六个 header 与 footer 同时存在退出按钮，四个前端模块又绑定两套 action | 作为首批低风险修复，精确删除六个 header action，不改变 Escape、backdrop 或原生窗口关闭 |
| 005 根滚动 | 根高度链未闭合、路由隐式 grid 行撑高、窄屏主动使用 `overflow: visible` | 在新增卡片和图片预览前完成布局基础，逐路由修复并加入滚动测量门禁 |
| 006 产物与 Step07 | 前端直接展示 artifacts 和 raw outputs；Step07 固定写 1×1 PNG；AI 图片层已有解析工具但没有执行与注入 | 拆为“立即隐藏内部产物”“可见预览与明确 fallback”“真实 Provider 生成”三段 |
| 007 范围与恢复 | 没有注册表驱动的 ID 解析器；执行接口以整步为单位；停止 token 与克隆状态分裂；无 checkpoint 和幂等协议 | 先冻结状态机、run/attempt 身份和提交协议，再逐层接入 Step11/12、恢复命令和 Web |
| 008 术语 | 术语没有场景边界，且把同时含 CLI/API 的顶层能力误称为“API”会制造概念冲突 | 先建立术语规则供后续 UI 使用；所有结构稳定后统一修正文案并启用阻断 lint |

## 三、不可变的统筹决策

1. **Python 只作行为基准。** 不把 Python 代码变成 NEWrust 运行依赖，也不在本计划中修改 Python。
2. **AI v3 先稳定，v4 后迁移。** 在 v3 行为、测试和实际 adapter 对齐前，不引入 Provider Registry 默认迁移。
3. **当前项只有一个真相。** v3 中 `dev.activeEntryId` 是开发类别权威；`activeProfileId` 仅是兼容派生值。`completion` 与 `image` 各自保存独立 `activeEntryId`。
4. **AI 弹窗是草稿事务。** “设为当前配置项”立即更新弹窗内草稿和全部相关视图；只有“保存”写磁盘并替换运行配置；“取消”回到打开弹窗前的持久化状态。
5. **CLI 与 API 语义分开。** `source=cli` 使用本地命令及其本机配置；`source=api` 由应用调用 HTTP；本地 Ollama、局域网服务和中转平台只要走 HTTP，仍属于 API。`source=cli_builtin` 只保留既有图像 CLI 特殊语义。
6. **配置校验分三级。** 结构校验决定能否保存；可用性探测由用户点击触发；实际运行前再次预检。CLI 暂未安装可保存为 warning，但运行前必须成为 blocker。
7. **打开配置页不自动访问网络。** API probe 必须由用户明确点击；preview 只做本地、脱敏解析。
8. **密钥和机器路径不进入项目内容。** API Key、CLI 路径、编辑器可执行路径属于机器级配置；项目/存档只保存非秘密的身份、版本要求或引用。
9. **路径选择能力只实现一次。** Tauri 桌面层提供薄的文件/目录选择适配器；项目识别、编辑器识别、CLI 识别分别在自己的领域服务中完成，不制造“万能发现器”。
10. **手工输入始终是兜底。** 取消原生选择、发现零候选或使用自定义引擎时，不得清空原值或阻断手工路径。
11. **根页面永不纵向滚动。** 通过 grid/flex 高度链和 `min-height: 0` 实现，不使用单纯 `body overflow:hidden` 掩盖内容截断。
12. **内部产物保留但不展示。** artifacts、manifest、checkpoint 和原始输出继续供执行、校验、保存和恢复使用；用户界面不展示其列表、路径或正文。Step07 图片是产品预览，不是通用文件浏览入口。
13. **Step07 状态必须真实。** 图片项至少区分 `generated`、`fallback`、`failed`；fallback 可供人工选择，但不得计入真实生成数量。
14. **步骤 ID 由注册表解释。** 输入文本通过 `StageSpec.number` 或等价注册信息映射到 canonical `stage_id`，不得在各层分别硬编码补零。
15. **运行与尝试分开。** `run_id` 跨暂停/恢复保持不变；每次实际启动使用新的 `attempt_id/attempt_no`，旧 attempt 的停止请求不能影响新 attempt。
16. **恢复必须显式且可证明。** 启动、刷新和 IPC 重连只发现恢复候选；用户点击恢复后才运行。指纹不符、checkpoint 损坏或单元结果不确定时不得盲目重跑。
17. **安全执行单元采用幂等提交协议。** 外部副作用前写入单元开始记录，完成后写结果与提交记录，再推进 checkpoint；崩溃后处于 `running/unknown` 的单元必须先核验。
18. **Step07 人工确认不是暂停恢复。** `waiting_confirmation` 与 `recoverable` 使用不同状态、按钮和后端命令。
19. **技术词按场景治理。** 顶层能力叫“开发配置 / 图像生成配置 / 文本补全配置”，具体类型再显示“Codex CLI / OpenAI API”等；不进行全局字符串替换。
20. **根启动器是稳定索引。** `NEWrust/Start-AutoDesignMaker.cmd` 只使用相对路径转交 `dist/AutoDesignMaker-NEWrust/Start-AutoDesignMaker.cmd`；缺失时给出可操作错误，不编译、不复制数据、不包含业务参数。

## 四、层级边界

| 层 | 职责 | 主要现有位置 |
| --- | --- | --- |
| Web | 表单草稿、呈现、可访问性、局部滚动、显式用户动作；不决定 canonical ID、Provider 或文件有效性 | `NEWrust/web/src`、`NEWrust/web/scripts` |
| Desktop/Tauri | 原生 dialog、后台任务、窗口生命周期、共享 active-run context；不实现领域识别规则 | `NEWrust/apps/desktop-tauri` |
| Command/View | 稳定 IPC DTO、脱敏 view、错误码；不返回密钥，不把内部产物包装成用户功能 | `NEWrust/crates/adm-new-tauri-commands` |
| Contracts | AI、项目绑定、流水线状态、checkpoint、执行单元的可序列化契约和兼容读取 | `NEWrust/crates/adm-new-contracts` |
| Application | 配置 resolution、项目预检、Provider 注入、恢复校验和用例编排 | `NEWrust/crates/adm-new-application` |
| Domain | AI adapter、项目/编辑器发现、注册表与执行单元、Step07 生成流程 | `adm-new-ai`、`adm-new-pipeline`、`adm-new-artifact` |
| Persistence | 原子写入、版本读取、备份、隔离损坏文件；checkpoint 不进入可见 artifact 列表 | `adm-new-storage`、`adm-new-foundation`、`ProjectPaths` |
| Packaging | 生成 `dist`；根启动索引独立存在，不因普通开发重复生成 | `NEWrust/tools`、`NEWrust/dist` |

## 五、全局开发依赖

`基础决策/fixture` → `模态与产物暴露止损` → `根布局` → `AI 当前项 + Step07 可见预览`  
→ `共享路径能力` → `AI v3 resolution` → `项目绑定/Unity 发现`  
→ `流水线状态机/安全单元/恢复` → `Step07 真实图片生成`  
→ `术语收口` → `Provider v4 与其他引擎` → `全量发布验收`

以下编号即默认开发顺序。只有明确标注可并行且不触碰同一文件的任务才允许并行；合并仍按编号进入主分支。

---

## Phase 0：基线、规则与稳定入口（P0）

主要触达：`NEWrust/testdata/fixplan`（拟新增）、相关 ADR/测试说明、`NEWrust/web/scripts`、`NEWrust/Start-AutoDesignMaker.cmd`。

| 顺序 | ID | 依赖 | 原子开发内容 | 完成判定 |
| --- | --- | --- | --- | --- |
| 001 | BASE-01 | 无 | 列出项目配置、AI 配置、流水线状态、Step07 从 Web → IPC → service → persistence 的实际调用链、schema 版本和文件位置；确认哪些字段目前会进入 UI、日志与存档 | 评审记录能定位每个写入者和读取者；没有把猜测当现状 |
| 002 | BASE-02 | 001 | 将本文件第三节的状态真相、草稿事务、CLI/API、项目/机器分层、run/attempt 和恢复规则固化为 ADR 或契约测试说明 | 后续任务不再自行发明第二套语义；未决项只剩具体 adapter 选型 |
| 003 | BASE-03 | 001–002 | 建立不含真实密钥和绝对用户路径的共享 fixture：AI v3、旧 AI 配置、项目路径、Unity 标志、流水线旧状态、Step07 非 1×1 PNG、旧 1×1 PNG、长 UI 数据 | Rust 与 Node 测试可复用；round-trip fixture 不丢未知字段 |
| 004 | BASE-04 | 003 | 记录当前 `cargo`、Web build/test/e2e/UI gate 的基线结果；失败项先登记归属，不顺手修复；测试使用临时数据根，禁止读写真实 `settings` 和用户存档 | 得到可重复的基线命令和失败清单；用户数据时间戳不变化 |
| 005 | I18N-01 | 003 | 建立场景化术语表和非阻断扫描规则：标准 token、适用 key/prefix、允许写法、禁用写法、例外和大小写；新 UI 从此遵守，旧命中暂不阻断 | 规则能区分技术字段与普通“接口/提示词/编号”等自然中文 |
| 006 | BOOT-01 | 004 | 新增 `NEWrust/Start-AutoDesignMaker.cmd`，以 `%~dp0` 计算路径并转交 dist 启动器；目标缺失时显示明确提示并仅在错误路径暂停；不加入 build 命令 | 静态测试确认只含相对转交；dist 存在时双击可启动，普通代码变更无需修改该文件 |

Phase 0 门禁：基线可重复、fixture 无秘密、启动器不触发构建，且本阶段不改变业务行为。

---

## Phase 1：快速 UI 止损（P0）

主要触达：`web/src/index.html`、`features/design.js`、`utility-panels.js`、`settings-style.js`、`ai-config.js`、`pipeline.js`、`locales/pipeline.js`、`web/scripts/test.mjs`、`e2e.mjs`、UI gate。

| 顺序 | ID | 依赖 | 原子开发内容 | 完成判定 |
| --- | --- | --- | --- | --- |
| 007 | MODAL-01 | 004 | 只删除六个外层 modal header action：`close-template-browser`、`close-save-template`、`close-save-manager`、`close-project-config`、`close-ai-config`、`close-style-prompt-editor`；保留 footer 和嵌套确认按钮 | 六个 header 退出按钮不存在，每个外层 modal 仍有一个底部退出按钮 |
| 008 | MODAL-02 | 007 | 从四个 JS 模块移除已删除 action 的查询、绑定和禁用选择器；不改变 Escape、backdrop、未保存提示、原生窗口关闭 | 无失效选择器；底部取消不会触发保存、应用、删除或加载 |
| 009 | MODAL-03 | 008 | 增加六个 modal 的静态和逐框交互回归，覆盖打开、footer 取消、再次打开与焦点恢复 | Web test/e2e 通过；中英文、窄屏 modal 无多余退出入口 |
| 010 | PVIEW-01 | 004 | 冻结流水线“用户可见投影”：只展示状态、message、errors/warnings、语义质量和专用步骤视图；generic artifacts、manifest、路径和 raw outputs 定义为内部字段 | 形成显式白名单，不使用递归删除所有 `path` 字段的脆弱黑名单 |
| 011 | PVIEW-02 | 010 | 从步骤详情移除 artifact 列表、读取按钮和通用 outputs JSON；后端 artifact 记录与 `read_pipeline_artifact` 保留供内部能力使用 | Step00 等普通步骤页面不出现产物数量、文件名、路径、Base64 或 `[编码=base64]` |
| 012 | PVIEW-03 | 011 | 反转原有“必须显示 artifact list”的测试；增加 DOM 可见文本泄漏断言，同时保留后端目录逃逸和读取大小限制测试 | 前端无法回归到通用文件浏览；后端运行、保存和校验产物不受影响 |

Phase 1 门禁：六个 modal footer 可正常退出；所有普通流水线步骤的可见文本中没有内部文件信息。

---

## Phase 2：关闭根滚动并建立区域布局（P0）

主要触达：`web/src/styles.css`、`index.html`、`main.js`、`scripts/fixtures.mjs`、`ui-gate.mjs`、`ui-baseline-gate.mjs`。

| 顺序 | ID | 依赖 | 原子开发内容 | 完成判定 |
| --- | --- | --- | --- | --- |
| 013 | LAY-01 | 009、012 | 用长项目名、长日志、长表格、多步骤和长访谈 fixture 记录 `documentElement/body/#app/route-outlet/active-panel` 的 clientHeight、scrollHeight、scrollTop 基线 | 每个路由都能复现或证明根滚动问题，不用空页面截图代替 |
| 014 | LAY-02 | 013 | 闭合根高度链：`html/body/#app` 100%，`app-shell` 使用 100vh fallback 与 100dvh，根级和 route-outlet `min-height:0; overflow:hidden`；不用 fixed 定位覆盖内容 | 根容器高度不超过 viewport，header/footer 保持在正常 grid 行内 |
| 015 | LAY-03 | 014 | 为 design、pipeline、patch、package、logs、sdk 分别声明真实显式 grid 行，清除统一两行规则产生的隐式第三行；所有可收缩直接子项补 `min-height:0` | 六个路由都不会由隐式行撑高根页面 |
| 016 | LAY-04 | 015 | 将设计工作台左/中/右栏拆成固定控制区与内部滚动区；保持搜索、访谈输入、tab 和操作按钮可见 | 长领域、长节点、长访谈和长结果各自在正确 pane 滚动 |
| 017 | LAY-05 | 015 | 将流水线步骤列表、详情、Step07 卡片区和日志建立有限高度与独立滚动；隐藏的 style grid 不占空间 | 任一区域增长都不把其他操作区或 footer 推出窗口 |
| 018 | LAY-06 | 015 | 逐个处理 patch、package、logs、sdk：移除撑高根页的固定最小高度，为表格、输出、上下文建立内部滚动 | 四个 utility 路由在长数据下根滚动仍为 0 |
| 019 | LAY-07 | 016–018 | 修复 900–1179 中宽布局和 899 以下窄屏规则，删除 `overflow:visible`；用比例行分配 design 与 pipeline 的堆叠区域 | 横向溢出留在 shell/pane 内，纵向溢出不传播到 document |
| 020 | LAY-08 | 019 | 对齐 modal 的 100vh/100dvh 与 body 内滚动；明确最小可用窗口宽高并与 Tauri 最小尺寸一致 | 低高度下标题和 footer 可见，只有 modal body 滚动 |
| 021 | LAY-09 | 020 | UI gate 增加根滚动和白名单 pane 滚动断言，覆盖 1280×820、1180×720、900×720、窄屏和最低支持高度，覆盖中英文 | 根层 `scrollHeight <= clientHeight + 容差`；指定 pane 可真实改变 scrollTop；无内容被隐藏冒充通过 |

Phase 2 门禁：所有路由、modal、语言和目标 viewport 都无 document 纵向滚动，且长内容仍可访问。

---

## Phase 3：AI 当前项与 Step07 可见预览（P0）

主要触达：`features/ai-config.js`、`pipeline.js`、`index.html`、`styles.css`、`locales/settings.js`、`locales/pipeline.js`、`adm-new-pipeline/src/stages/step07.rs`、`adm-new-artifact`、Web 测试与 UI gate。

| 顺序 | ID | 依赖 | 原子开发内容 | 完成判定 |
| --- | --- | --- | --- | --- |
| 022 | AI-ACT-01 | 021 | 为 `dev/completion/image` 三类别建立独立激活模型测试，包含无效 active ID、空类别和兼容 `activeProfileId` | 改一类不会改变另外两类；dev 兼容字段只能由 dev 派生 |
| 023 | AI-ACT-02 | 022 | 明确并实现弹窗草稿事务：打开时深拷贝持久化配置，保存提交，取消丢弃；选择卡和详情字段都写草稿 | 点击当前项不会提前写磁盘；取消后重新打开恢复原值 |
| 024 | AI-ACT-03 | 023 | 增加唯一 `setActive(categoryId, entryId)` 动作；执行前同步当前详情表单，校验目标属于当前类别 | 未保存的详情字段不丢失，错误类别/ID 被明确拒绝 |
| 025 | AI-ACT-04 | 024 | 在每张配置卡右上角渲染动作按钮：非当前显示“设为当前配置项”，当前显示高亮“当前配置项”并设置 `aria-pressed`；不使用对号 | 按钮尺寸、命中区、对比度和窄屏换行满足 UI gate |
| 026 | AI-ACT-05 | 025 | 点击后明确选中目标卡，并一次性重绘列表、详情、类别总览和状态提示；阻止依赖卡片冒泡，恢复合理焦点 | 无需点击其他控件即可看到左侧当前项和所有关联视图刷新 |
| 027 | AI-ACT-06 | 026 | 删除右侧“设为当前”checkbox、漂浮“当前”标签和重复视觉表达，清理失效事件与 locale | 每个配置只保留卡片内一个当前状态表达 |
| 028 | AI-ACT-07 | 027 | 固化删除规则：最后一个条目不可删；删除当前项优先选择下一项，否则上一项；删除非当前项不改变 active ID | 边界行为有模型测试，三类别一致 |
| 029 | AI-ACT-08 | 028 | 覆盖保存、取消、关闭重开、保存失败、删除后保存和应用重启；验证实际 adapter 读取的 active 与 UI 一致 | 重启后 active ID 稳定，保存失败不污染运行配置 |
| 030 | AI-ACT-09 | 029 | 完成中英文、键盘、焦点、窄屏、长名称和截图门禁 | AI 配置页无漂浮标签、按钮遮挡或当前状态延迟 |
| 031 | PREVIEW-01 | 021、012 | 在前端建立仅供 Step07 使用的 preview loader，内部调用已有 artifact read；参数固定 stage 07、受控 imagePath 和最大字节，状态为 idle/loading/loaded/unavailable | 通用 artifact 读取入口不重新出现在界面 |
| 032 | PREVIEW-02 | 031 | 验证 MIME、Base64 encoding、非截断和非空后，只把内容拼入 `img.src`；渲染固定比例、`object-fit` 和本地化 alt，不显示 image_path | Base64 不进入 textContent、日志或错误；图片卡真实显示图像 |
| 033 | PREVIEW-03 | 032 | 增加非图片、截断、失败、空路径和步骤切换后的过期异步响应保护；失败只显示可读状态 | 旧请求不能覆盖新卡片，错误时不会退回 Base64 文本 |
| 034 | FALLBACK-01 | 003、033 | 在可复用图片资产层生成至少 640×384 的确定性 palette/reference PNG，替换 Step07 固定 1×1 字节常量；同一输入得到稳定结果 | 新运行的 fallback naturalWidth/naturalHeight 均大于 1，且视觉可检查 |
| 035 | FALLBACK-02 | 034 | 为当前 fallback 写最小结构化状态和生成统计，明确 `fallback` 不是 `generated`；保留 style option 与 confirmation 引用 | UI、manifest 和日志都不会把 fallback 计作真实 Provider 成功 |
| 036 | PREVIEW-04 | 034–035 | 加载旧存档时通过 PNG 元数据识别 1×1 占位图，显示“旧版占位图，需要重新运行 Step07”，不自动伪造新图 | 旧 1×1 不被当作有效预览或成功图片 |
| 037 | PREVIEW-05 | 031–036 | 为正常图、fallback、旧 1×1、读取失败和竞态补 unit/e2e/UI gate；断言图片可见且尺寸大于 1×1 | Step07 有可检查预览，普通步骤仍完全隐藏内部产物 |

Phase 3 门禁：AI 当前项立即刷新且三类独立；Step07 不显示路径/Base64，并至少提供明确标记的可检查 fallback。

---

## Phase 4：共享路径能力、AI v3 运行语义和 Unity 项目发现（P0/P1）

主要触达：`apps/desktop-tauri/Cargo.toml`、`commands/config.rs`、`lib.rs`、`adm-new-tauri-commands/src/config.rs`、`adm-new-contracts/src/ai.rs`、`adm-new-config`、`adm-new-ai`、`adm-new-application/src/runtime.rs`、`settings-style.js`、`ai-config.js`。

| 顺序 | ID | 依赖 | 原子开发内容 | 完成判定 |
| --- | --- | --- | --- | --- |
| 038 | PATH-01 | 004 | 定义薄的原生选择契约：file/folder、标题、可选过滤器、selected/cancelled/error；后端负责规范化，领域层另行校验 | Web 不直接依赖 OS 路径规则，取消不是异常 |
| 039 | PATH-02 | 038 | 在 desktop 层接入一个 Tauri 2 原生 dialog adapter，并通过稳定命令/view 暴露；补齐所需 capability/permission，禁止把 dialog 依赖扩散到 domain crate | 文件和目录选择均返回结构化结果，测试可注入 fake adapter |
| 040 | PATH-03 | 039 | 覆盖 Windows 盘符、UNC、空格、中文、程序扩展名、取消和不存在路径；取消保持原值，非法选择不静默覆盖 | 路径规范化与错误码测试通过，没有字符串解析错误信息的前端逻辑 |
| 041 | AI-RES-01 | 003、030 | 建立 Python v3/旧格式共享 fixture 和 serde 扩展字段保存机制，覆盖三类别、CLI、API、未知字段、旧 profiles、无效 active ID | 读取再保存不丢未知字段；fixture 不含真实 Key |
| 042 | AI-RES-02 | 041 | 集中实现 active 规范化和确定性回退；保存时由 dev 派生 `activeProfileId`，读取旧文件时可兼容但不产生双状态 | UI、保存文件和运行 adapter 对同一 active 得出相同结果 |
| 043 | AI-RES-03 | 042 | 建立唯一 `configType descriptor` 表，描述 category、`cli/api/cli_builtin` source、adapter kind、能力、必填字段；Web 选项与后端表通过契约测试对齐 | 各层不再用 startsWith/字符串散落推断同一类型 |
| 044 | AI-RES-04 | 043 | 定义内部 `ResolvedAiTarget` 与可序列化的脱敏 `AiResolutionView`；包含类别、entry、source、adapter、命令/地址、模型、能力和诊断，不允许 secret-bearing 类型序列化 | preview 只返回 masked/has-secret 信息，adapter 才能取得秘密材料 |
| 045 | AI-RES-05 | 044 | 统一结构错误、availability warning、run blocker 的错误码和严重度，覆盖 active 缺失、类型错类、URL/模型不完整、CLI 不存在 | 保存、检测和运行前预检不再混用同一布尔校验 |
| 046 | AI-RES-06 | 045 | 实现 Codex/Claude PATH 发现和可选显式命令路径覆盖，覆盖 Windows `.exe/.cmd/.bat` 与命令优先级 | 未安装、路径无效、不可执行、权限失败可区分；未安装不阻止保存 |
| 047 | AI-RES-07 | 046 | 实现安全版本探测：显式 program/args、超时、隐藏窗口、stdout/stderr 上限和脱敏，不使用 shell 字符串拼接 | 版本成功、超时、非零退出、乱码/大输出均有确定结果 |
| 048 | AI-RES-08 | 047 | 增加用户触发的“检测 CLI” IPC 和 UI，显示路径、版本、来源与错误；打开弹窗不自动运行 | CLI 检测可重复，检测结果不改写用户配置 |
| 049 | AI-RES-09 | 045 | 集中 API URL/endpoint 规范化、model、timeout、temperature、reasoning、headers/body 与能力校验；认证支持直接 Key、环境变量引用、无认证 | `/v1` 和 endpoint 不重复拼接，本地 HTTP 仍按 API 处理 |
| 050 | AI-RES-10 | 046–049 | 实现纯本地 resolution preview，展示最终 CLI program/args 摘要或脱敏 API 地址、模型、来源、能力与 blockers | preview 不访问网络、不展开 Key、不返回完整敏感 header |
| 051 | AI-RES-11 | 049–050 | 实现用户点击触发的 API probe，使用可注入 transport、明确超时和状态码分类；测试只用 mock server/fake transport | 成功、鉴权失败、超时、无认证服务均可判别，打开页面不发请求 |
| 052 | AI-RES-12 | 050–051 | 让 dev/completion/image 的实际 adapter 只消费统一 resolution；移除重复裸 entry 解析，明确 generic local HTTP 使用 OpenAI-compatible/custom API adapter | 保存预览与实际调用目标一致，本地 CLI 无 URL/Key 可运行 |
| 053 | AI-RES-13 | 039、047 | 为 Codex TOML/JSON 和可选 CLI 路径接入原生文件选择与只读诊断；只记录引用/存在性，不复制配置文件中的密钥 | 取消不清空，诊断输出无秘密，路径可重新选择 |
| 054 | AI-RES-14 | 041–053 | 完成三类别、CLI/API 共存、重启、旧配置、URL、probe、超时和秘密泄漏测试；扫描 DOM、日志、错误和 snapshot | v3 行为闭环，任何 Key/Base64 均不进入可见输出 |
| 055 | PROJ-01 | 040 | 定义项目绑定与机器解析 v2：项目侧保存 engine、要求版本、逻辑 binding/reference；外部绝对项目路径和 editor executable 保存到机器级 `settings/project_bindings.json` 或等价仓库；兼容读取旧 `development_path/editor_path` | 移动或换机只使机器解析失效，不丢项目身份；迁移不静默删除旧字段 |
| 056 | PROJ-02 | 055 | 项目配置 UI 接入“选择项目文件夹”“选择编辑器程序”，保留手输；选择后先检测/校验，取消保持原值 | 中文、空格、UNC、失效路径的交互有明确状态 |
| 057 | PROJ-03 | 055 | 定义可扩展 `ProjectDetector`、`EditorLocator`、候选、诊断和 fixAction 接口；项目识别器与 CLI 识别器不合并 | Unity 实现可替换测试 fake，未来引擎无需改 Web/IPC 基本契约 |
| 058 | PROJ-04 | 057 | 实现 Unity 目录标志检测和 `ProjectSettings/ProjectVersion.txt` 解析，保留完整版本及可比较部分 | 临时目录 fixture 可稳定识别有效、部分和非 Unity 项目 |
| 059 | PROJ-05 | 058 | 只扫描 Unity Hub/已知安装位置和已有手工路径，不做全盘扫描；验证候选必须为 Unity Editor，排除 Unity Hub | 扫描有界、可取消、Hub 不会被误认作 Editor |
| 060 | PROJ-06 | 059 | 候选按精确版本、兼容版本、手工指定排序并去重；零候选回退手工，多候选交给用户选择 | 同一 fixture 排序确定，不能静默选不匹配版本 |
| 061 | PROJ-07 | 056、060 | UI 展示检测到的 engine/version/editor 候选；检测结果与用户已选 engine 冲突时要求确认，不自动覆盖 | 选择、冲突确认、零/多候选、重新扫描都有 e2e |
| 062 | PROJ-08 | 061 | 将 preflight 改为结构化 diagnostics：severity、errorCode、field、message、fixAction；动作至少支持重新扫描、重新选择、打开目录 | Web 不解析英文错误文本决定按钮；blocker 与 warning 清晰 |
| 063 | PROJ-09 | 062 | 实现项目移动、editor 升级、机器解析丢失的 relink 流程；验证项目仍有效时只重建机器解析，不要求重填全部元数据 | 换目录/换机 fixture 能恢复，旧失效路径不会被悄悄继续使用 |
| 064 | PROJ-10 | 055–063 | 完成配置兼容、路径选择、Unity 检测、候选排序、preflight、relink、保存重启及流水线运行前预检回归 | 项目选择流程可用，旧项目配置可读，真实用户配置不参与测试 |

Phase 4 门禁：v3 AI 的 UI、preview 与实际 adapter 一致；项目路径可选择、Unity 可识别、机器工具路径可失效重连。

---

## Phase 5：流水线 canonical 输入、安全停止与显式恢复（P0/P1，高风险）

主要触达：`adm-new-contracts/src/pipeline.rs`、`adm-new-pipeline`、`adm-new-application`、`adm-new-tauri-commands/src/pipeline.rs`、`apps/desktop-tauri/src/runtime.rs`、`commands/pipeline.rs`、`web/src/features/pipeline.js`、`index.html`、pipeline locale 与测试。

| 顺序 | ID | 依赖 | 原子开发内容 | 完成判定 |
| --- | --- | --- | --- | --- |
| 065 | RUN-01 | 064 | 冻结运行状态机、run/attempt 身份、单项目互斥、checkpoint schema 和安全单元提交协议；状态至少区分 running、stop_requested、stopping、recoverable、resuming、waiting_confirmation、completed、failed/recovery_blocked | 状态转换表明确合法来源、目标和持久化时机；Step07 门禁不与恢复混用 |
| 066 | RUN-02 | 065 | 为 `PipelineRunState` 增加 schema/version、范围、attempt、状态版本和恢复摘要；兼容加载当前无版本/旧字段状态 | 旧 `pipeline_state.json` 可加载并确定性升级，未知/损坏状态安全隔离 |
| 067 | RUN-03 | 065 | 在注册表领域实现步骤输入解析：trim、十进制整数文本、按 `StageSpec.number` 找 canonical ID、按拓扑校验范围；拒绝空、负数、小数、字母、歧义和反向范围 | `0/00→00`、`1→01`、`8→08`、`12→12`；执行顺序不用字符串排序 |
| 068 | RUN-04 | 067 | Web、Tauri request、command service 和 CLI 入口统一调用该解析器；日志、响应、状态和 checkpoint 只保存 canonical ID | 任一入口对同一输入返回同一 ID 和错误码 |
| 069 | RUN-05 | 068 | 将两个 select 改为可编辑 input+datalist 或等价控件；提交成功回显 canonical 值，失败显示字段错误 | 用户可直接输入 1、8、12；运行中范围编辑受控 |
| 070 | RUN-06 | 068 | 为现有异步运行补回归：长任务 pending 时 Web 事件循环和 stop IPC 仍可响应；不为此另造无必要的任务队列 | 点击运行立即显示已受理/运行中，界面不被同步阻塞 |
| 071 | RUN-07 | 066 | 建立共享 `ActiveRunContext`：run/attempt ID、实时 stop token、状态版本、项目锁和持久化协调器；后台 worker 不再以克隆 state 作为实时真相 | 同项目只能有一个 active run，旧 attempt token 被隔离 |
| 072 | RUN-08 | 071 | 停止命令更新同一个 context 并持久化 stop 事件：请求时间、原因、模式、目标边界、处理状态；在步骤/单元前后读取 | 当前 worker 能实时看到停止请求，重复 stop 幂等 |
| 073 | RUN-09 | 072 | 使用状态版本/CAS 或等价规则解决“最后单元完成”和 stop 同时到达的覆盖竞态 | 故障测试中最终状态与已提交 checkpoint 一致，不由旧克隆覆盖 |
| 074 | RUN-10 | 066、071 | 实现独立 checkpoint repository：runtime 控制目录、原子写入、读取校验、版本兼容、损坏隔离和活动/归档边界；排除 artifact UI/index | 重启可找到有效 checkpoint；半写文件不能被当成可恢复 |
| 075 | RUN-11 | 071、074 | 扩展执行接口，注入 run context、stop token、稳定 unit ID、幂等键、unit started/result/commit 回调和 reconcile 入口 | domain executor 不读取 Tauri 全局状态，单元生命周期可测试 |
| 076 | RUN-12 | 075 | 为不可拆分短步骤提供“整步骤即一个安全单元”的默认适配器，明确 stop 为本步骤提交后生效 | 00–10、13–14 等未细分步骤保持现有行为且进入统一 checkpoint |
| 077 | RUN-13 | 075–076 | 用 fake 多单元 executor 注入副作用前、结果后、checkpoint 前后崩溃，以及重复 stop/resume、旧 attempt stop、双重恢复 | 已提交单元不重做；unknown 单元先 reconcile，不能盲跑 |
| 078 | RUN-14 | 075 | 审计 Step11/12 的任务、分组、文件写入、CLI/API 调用和副作用，形成安全边界表、稳定 unit ID 和核验方式 | 每种副作用都有 commit/reconcile 策略，无“以后再看”的未知边界 |
| 079 | RUN-15 | 077–078 | Step11 接入任务/分组级执行单元、结果复用、幂等键和 checkpoint；停止后不启动下一单元 | 真实 Step11 在 N 单元停止，恢复只执行 N+1 以后 |
| 080 | RUN-16 | 079 | Step12 复用同一协议接入分组/任务边界，不复制另一套恢复框架 | Step12 通过同样的 stop/resume 与幂等门禁 |
| 081 | RUN-17 | 074、079–080 | 实现恢复校验器：项目 binding、草稿/存档、范围/拓扑、输入、选中配置引用、stage 实现版本、计划与必要结果引用指纹；不把秘密值写入指纹 | 任一不匹配返回结构化不可恢复原因 |
| 082 | RUN-18 | 073、081 | 增加显式 resume command：锁定原 run、验证 checkpoint、递增 attempt、记录父 checkpoint，从 next_unit 开始；普通 run 不隐式恢复 | 双重恢复、已完成 run、被占用 run 和错误 checkpoint 均安全拒绝 |
| 083 | RUN-19 | 082 | 启动、刷新、IPC 重连只加载恢复候选；旧 running 有有效 checkpoint 才转 recoverable，否则进入 recovery_blocked，而非含糊 stopped | 应用不会自动续跑；损坏/缺失 checkpoint 有明确下一步 |
| 084 | RUN-20 | 069、083 | Web 展示 running、停止处理中、recoverable、恢复失败、waiting_confirmation；显示当前步骤/安全单元/时间和“继续/恢复”，不显示 checkpoint 路径 | 状态和按钮互斥正确，Step07 确认不会出现恢复按钮 |
| 085 | RUN-21 | 065–084 | 全链路执行 canonical 范围、并发拒绝、stop 竞态、崩溃、重启、重连、Step11/12、Step07 人工门禁和旧状态回归 | Rust workspace、Web、e2e、UI gate 与发布 smoke 全通过 |

Phase 5 门禁：恢复能力只有在 checkpoint、幂等、校验、显式命令和 UI 全部接通后才可标记为“可恢复”。

---

## Phase 6：Step07 真实图片生成（P1）

主要触达：`adm-new-ai/src/image.rs`、AI adapter/transport、`adm-new-pipeline/src/stages/step07.rs`、`product_executor.rs`、`generation.rs`、application 注入、pipeline command/view、Step07 Web 视图。

| 顺序 | ID | 依赖 | 原子开发内容 | 完成判定 |
| --- | --- | --- | --- | --- |
| 086 | IMG-01 | 054、085 | 定义可注入 `StyleImageGenerator`：request 含 prompt、期望尺寸/格式、项目上下文和 unit ID；result 含 bytes/临时路径、provider/model、实际尺寸、status 和脱敏错误 | Step07 不自行打开 AI 配置或拼 HTTP/CLI |
| 087 | IMG-02 | 086 | application/ProductPipelineExecutor 通过统一 AI resolution 解析当前 image entry 并注入 generator；未配置时返回明确 unavailable | 配置页、preview 和 Step07 使用同一 active image entry |
| 088 | IMG-03 | 087 | 补齐 API 图片执行器，复用现有 request builder、Responses/Image API Base64 提取和 PNG 校验；增加 transport、超时、状态码与脱敏 | fake transport 覆盖流式、JSON、鉴权失败、超时和无效图片；不访问真实网络 |
| 089 | IMG-04 | 087 | 补齐 Codex CLI 图片执行器，复用 command spec、输出路径/session 解析；限制本次启动时间窗口和新文件集合，校验后复制到临时目录 | 不会把 Codex Home 历史 PNG 误认本次结果；超时和 stderr 均脱敏 |
| 090 | IMG-05 | 088–089 | 固化生成策略：active entry 能力匹配则真实生成；显式 test/offline 使用 fake/fallback；provider 失败记录 degraded/failed，不能静默 success | 不支持图像能力的 CLI 被明确拒绝或降级 |
| 091 | IMG-06 | 085、090 | 将每个 style option 建模为稳定执行单元，逐项生成、校验和提交；stop 在当前图片单元完成后生效，恢复复用已成功项 | Step07 真实生成遵循 Phase 5 的 checkpoint/幂等协议 |
| 092 | IMG-07 | 091 | 在临时目录完成本轮候选并校验后原子替换正式目录；provider 中途失败时保留上一轮有效预览或明确记录本轮未提交 | 重跑不会先删空全部旧图，失败后目录状态可解释 |
| 093 | IMG-08 | 092 | 升级 generation manifest：requested、provider_generated、fallback、failed 数量；逐项 provider/model/status/reason/尺寸/格式/prompt override；总状态 success/partial/degraded/failed | 统计与实际文件、UI 状态一致，fallback 不计真实生成 |
| 094 | IMG-09 | 093 | Step07 UI 展示真实生成/降级/失败标签和可操作提示；confirmation 引用当前轮 option；重生成与清理不删除被选有效图 | 用户能判断图片来源和质量，不看到内部路径 |
| 095 | IMG-10 | 086–094 | 覆盖全部成功、部分 fallback、全失败、CLI 超时、API 错误、stop/resume、旧 1×1、重跑保留/替换、confirmation 和秘密/Base64 泄漏 | adm-new-ai、pipeline、commands、desktop、Web 和 UI 门禁通过 |

Phase 6 门禁：Step07 默认尝试当前有效 image provider；无法真实生成时明确降级，永不再用 1×1 假成功。

---

## Phase 7：技术术语统一收口（P1）

主要触达：`web/src/locales/shell.js`、`settings.js`、`utility.js`、`pipeline.js`、`design.js`、`index.html`、设计内容源数据/生成器、`i18n-test.mjs`、language/UI gate。

| 顺序 | ID | 依赖 | 原子开发内容 | 完成判定 |
| --- | --- | --- | --- | --- |
| 096 | I18N-02 | 005、095 | 生成当前命中报告并为术语规则写正例/反例测试，区分技术字段、导航、普通说明和 generated 内容 | 不使用全局替换，规则不会误报合法自然中文 |
| 097 | I18N-03 | 096 | 修正 `shell.js` 中 AI、SDK、ID、Markdown、JSON、URL，并同步 `index.html` fallback、placeholder、aria-label；加入 catalog/fallback 一致性检查 | 首屏加载前后术语一致，key/placeholder 不变化 |
| 098 | I18N-04 | 054、096 | 修正 `settings.js`：顶层采用“开发配置/图像生成配置/文本补全配置”，具体类型使用“本地 Codex CLI/OpenAI API/自定义 API”；中文标签使用 `API Key` | 不把包含 CLI 的能力类别整体叫 API |
| 099 | I18N-05 | 096 | 修正 `utility.js` 中明确技术场景的 SDK/ID 等；保留“存档、编号、接口”等自然产品词 | utility 行为和 key 不变 |
| 100 | I18N-06 | 085、095–096 | 修正 `pipeline.js`、`design.js` 中明确技术场景的 AI、Adapter、Prompt、Token；恢复和图片状态沿用术语表 | 新增运行状态文案和旧页面一致，不机械英文化普通说明 |
| 101 | I18N-07 | 096 | 扫描设计内容源数据；只有真实命中才修改源数据或生成规则并重新生成，禁止直接编辑 `design-content.generated.js` | generated stale check 通过，生成文件可追溯 |
| 102 | I18N-08 | 097–101 | 审核英文 catalog 的缩写大小写和自然搭配，不要求与中文逐字映射 | 中英文 key、变量、换行和 placeholder 完全对齐 |
| 103 | I18N-09 | 096–102 | 在现有命中清零后才把 terminology lint 切为阻断；规则按 key/prefix 和例外作用 | 指定技术 key 的禁用写法清零，普通句子不被阻断 |
| 104 | I18N-10 | 103 | 执行 i18n、generated check、language gate、Web unit/e2e、双语窄屏 UI/screenshot 全量门禁 | 文案变化不影响 AI 保存、API Key 脱敏、SDK 或流水线行为 |

Phase 7 门禁：标准技术名可直接对应代码与配置，同时中文产品说明仍然自然。

---

## Phase 8：Provider v4 与跨引擎扩展（P2，显式开启）

此阶段不阻塞 Phase 0–7 的核心交付。只有 v3 全部门禁通过并由产品确认需要跨类别复用 Provider 时才启动。

主要触达：AI contracts/config/repository/resolution/UI/migration；项目 detector/locator 与 project binding repository。

| 顺序 | ID | 依赖 | 原子开发内容 | 完成判定 |
| --- | --- | --- | --- | --- |
| 105 | PRV-01 | 054、104 | 提交 Provider v4 ADR，确认复用场景、schema、机器/项目边界和迁移触发；明确本阶段只做 `ProviderDefinition + 三类别独立选择`，不引入完整 Launch Profile | 未获得确认则阶段停在 ADR，不修改用户配置 |
| 106 | PRV-02 | 105 | 定义 ProviderDefinition：kind/source、地址、认证引用、模型、参数、能力、配置目录和扩展字段；类别 selection 只保存 provider ID | profile/selection 不复制密钥和完整连接参数 |
| 107 | PRV-03 | 106 | 实现机器级 provider repository CRUD、唯一 ID、引用完整性、删除保护和确定性排序 | 被引用 Provider 不能静默删除；未知字段保留 |
| 108 | PRV-04 | 107 | 让 dev/completion/image 分别解析 provider 引用；优先级固定为显式类别选择 → 可选任务选择 → 经确认的默认 → 明确错误；不允许全局默认覆盖 active | 三类别继续独立，可复用同一 Provider 而不复制配置 |
| 109 | PRV-05 | 107–108 | 将 Key、CLI 路径、配置目录保留在机器级；项目/存档只保存 provider ID；统一 preview/probe/日志脱敏 | 保存和导出项目均不包含秘密 |
| 110 | PRV-06 | 108–109 | 实现 v3→v4 显式迁移 preview、备份、原子切换、回滚和幂等；旧 profile 作为兼容投影，迁移前后 resolution 必须相同 | 失败自动恢复备份，多次迁移不重复创建 Provider |
| 111 | PRV-07 | 107–110 | 增加 Provider CRUD、引用选择、检测、preview 和配置目录操作 UI；沿用 Phase 3 当前项按钮与 Phase 7 术语 | 不重新引入 checkbox、第二 active 状态或明文 Key |
| 112 | PRV-08 | 105–111 | 完成 v3/v4 共存、迁移/回滚、引用删除、三类别、CLI/API、重启和秘密扫描 | 默认不迁移旧用户；显式迁移可恢复 |
| 113 | ENG-X-01 | 064 | 实现 Godot `project.godot` 标志、版本需求和有界编辑器发现，复用 ProjectDetector/EditorLocator | Godot 不复制 Unity 分支 UI/IPC |
| 114 | ENG-X-02 | 064 | 实现 Unreal `.uproject`、版本关联和有界编辑器发现 | 多 uproject、缺版本和多编辑器候选有确定行为 |
| 115 | ENG-X-03 | 064 | 自定义引擎使用宽松规则：允许名称、项目目录和可选程序；缺少标准结构只 warning，不错误套用 Unity/Godot/Unreal | 自定义工作流始终保留手工兜底 |
| 116 | ENG-X-04 | 113–115 | 存档恢复和项目重开时重新验证 binding；支持项目移动、换机和编辑器升级后的 relink | 项目元数据不因机器路径失效而丢失 |
| 117 | ENG-X-05 | 113–116 | 完成三引擎/自定义的检测、候选、preflight、relink、重启、路径字符集和 UI matrix | 跨引擎共用契约，错误码与修复动作一致 |

Phase 8 门禁：Provider v4 只在显式批准后迁移；跨引擎不破坏 Unity 与手工路径兜底。

---

## Phase 9：集成收口与发布验收（P0 收口）

| 顺序 | ID | 依赖 | 原子开发内容 | 完成判定 |
| --- | --- | --- | --- | --- |
| 118 | FIN-01 | 104；若启动 Phase 8 则再依赖 117 | 做 UI → store/service → IPC → application → domain → persistence 的跨层契约审查，重点比对命名、serde casing、错误码、active ID、run/attempt 和 status | 没有只改一层导致的静默 fallback 或第二真相 |
| 119 | FIN-02 | 118 | 做安全与隐私审计：API Key、环境变量值、CLI stderr、Base64、绝对机器路径、artifact/checkpoint 路径不得进入 DOM、普通日志、项目存档和截图 | 自动扫描与人工抽查均无泄漏 |
| 120 | FIN-03 | 119 | 执行 Rust fmt/check/workspace tests、目标 crate tests、Web build/test/e2e、i18n/language/UI/baseline gate；记录耗时和失败归属 | 所有必需门禁为绿，无跳过项冒充通过 |
| 121 | FIN-04 | 120 | 生成一次 portable dist 并执行发布 smoke；随后验证根启动器只转交现有 dist，普通源码改动后无需重建根启动器 | dist 启动、数据目录、语言和根双击入口均正确 |
| 122 | FIN-05 | 121 | 按真实用户路径手工验收：AI 当前项、仅 CLI、API 中转、路径选择/Unity 发现、Step00 无产物、Step07 图片、1→8、停止/重启/恢复、双语窄屏 | 每个场景留下可复核结果和失败截图/日志摘要 |
| 123 | FIN-06 | 122 | 更新 001–008 状态和实现链接，记录已完成、延期与显式未启动的 Phase 8；写最终交接清单和回滚点 | 原问题与实际实现一一可追踪，不把计划项误标为已完成 |

## 六、分阶段统一门禁

每个 Phase 完成后必须执行与变更相称的门禁，不能把所有测试推迟到 Phase 9：

- Rust：`cargo fmt --check`、目标 crate 测试、`cargo check --workspace`；契约或共享执行层变化时运行 `cargo test --workspace`。
- Web：`npm.cmd test`、`npm.cmd run build`；交互变化运行 `npm.cmd run e2e`。
- UI：`npm.cmd run ui-gate`、`npm.cmd run ui-baseline-gate`，覆盖中文、英文、desktop、narrow 和本阶段新增状态。
- 持久化：旧格式读取、round-trip、未知字段保留、原子写入、损坏隔离、迁移幂等和失败恢复。
- 进程/网络：超时、隐藏窗口、显式 args、输出上限、mock transport；自动测试不访问真实外网。
- 秘密：DOM、日志、错误、preview、snapshot、fixture、项目存档和 screenshot 中均不得出现明文 Key。
- 路径：Windows 盘符、UNC、空格、中文、取消、失效、移动、换机和重启。

## 七、来源计划验收映射

| 原计划 | 主要实现任务 | 最终验收 |
| --- | --- | --- |
| 001 | 038–040、055–064、113–117 | 原生选择、Unity 发现、项目/机器分层、relink、跨引擎 |
| 002 | 022–030 | 卡片右上按钮、立即刷新、无对号、三类别独立、保存/取消正确 |
| 003 | 041–054、105–112 | v3 resolution、CLI/API 共存、probe/preview、显式 Provider v4 迁移 |
| 004 | 007–009 | 六个 header 退出按钮删除，footer 和其他关闭语义不变 |
| 005 | 013–021 | 根滚动为 0，所有长内容在白名单区域内部滚动 |
| 006 | 010–012、031–037、086–095 | 不展示 artifacts/Base64；Step07 图片、真实/降级状态和旧 1×1 兼容 |
| 007 | 065–085 | canonical 输入、安全停止、checkpoint、Step11/12、显式恢复和重连 |
| 008 | 005、096–104 | 场景化术语、generated 来源修复、阻断 lint 和双语 UI gate |
| 根启动入口补充 | 006、121 | `NEWrust` 主目录可双击，文件只负责相对索引，不参与构建 |

## 八、明确不做

1. 不删除后端产物、manifest、checkpoint 或现有路径安全校验。
2. 不让 Web 自己判断项目类型、拼接编辑器路径、规范化 Provider URL 或补零步骤 ID。
3. 不在打开 AI 配置时自动检测 CLI、访问 API 或读取配置文件秘密。
4. 不自动恢复流水线，不在指纹不符时提供“强制继续”。
5. 不把 fallback 图片标记为 AI/provider generated。
6. 不直接修改 `design-content.generated.js`。
7. 不复制 `cc-pane` 的完整运行时、技能、环境和 Launch Profile 模型；只有明确复用需求后另立计划。
8. 不让根启动器执行 `cargo`、`npm`、portable build 或业务初始化。

## 九、最终 Definition of Done

只有同时满足下列条件，整个总计划才可标记完成：

- 123 个原子任务中，所有已批准范围都有实现链接、测试证据和回滚点；未启动的 P2 项明确标为延期，而不是已完成。
- UI 无重复退出按钮、无根纵向滚动、无内部产物和 Base64 文本泄漏。
- AI 当前项、保存文件、resolution、实际 adapter 和重启状态一致。
- 路径选择、Unity 识别和 relink 不依赖用户复制粘贴作为唯一入口。
- 流水线 canonical 输入、停止、checkpoint、恢复和人工门禁状态经过故障注入与重启验证。
- Step07 展示可检查图片，并准确区分真实生成、fallback、失败与旧 1×1。
- 技术术语、双语、窄屏、持久化、安全、portable 和根启动入口的全量门禁通过。
