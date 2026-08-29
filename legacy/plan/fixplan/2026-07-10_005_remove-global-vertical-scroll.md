# 全界面垂直滚动移除与区域比例适配计划

## 记录状态

- 已完成（2026-07-11）：根高度链、路由布局和局部滚动已统一，长内容在所属区域内滚动；UI 门禁覆盖目标窗口尺寸。
- 以下内容保留为原始问题、原因、设计方案和验收标准。
- 用户要求：禁止整个界面上下滚动；保留必要内容区域的内部滚动，并做好不同界面和窗口尺寸的占比适配。

## 1. 问题记录

当前 NEWrust 的多个主界面都可以拖动整个页面上下滚动。用户期望的是桌面应用式布局：顶部导航、主内容区域和底部状态栏固定在窗口内；只有列表、日志、长文本、表格等确实需要滚动的内容区域出现内部滚动条。

## 2. 当前实现与根因

### 2.1 主滚动容器设置过宽

`NEWrust/web/src/styles.css` 当前存在：

- `.route-outlet { overflow: auto; padding: 12px; }`
- `.task-panel { min-height: calc(100vh - 102px); }`
- `.app-shell { min-height: 100vh; }`

这会让路由内容容器承担整个页面的纵向滚动；`min-height`、外层 padding 和动态内容叠加后，主内容很容易超过窗口高度。

### 2.2 Grid 子项没有形成稳定的剩余高度链

虽然部分布局已经使用 `minmax(0, 1fr)` 和 `min-height: 0`，但从应用根节点到 `.route-outlet`、`.task-panel`、各个内容 pane 的高度链没有完全闭合：

- 根节点只有 `min-height`，没有明确的 viewport 高度和根溢出策略。
- `.route-outlet` 是滚动容器，导致子布局的高度问题被转移到整页。
- `.task-panel.active` 的显式行定义没有覆盖所有页面中的额外 footer/content 子项。
- 部分 pane 有 `overflow: auto`，但没有统一由固定的剩余高度行提供高度约束。

### 2.3 窄屏规则主动放开了内容溢出

在 `max-width: 899px` 下，`.design-shell-grid`、`.two-pane-shell` 和 `.pipeline-shell` 被设置为：

```css
grid-template-columns: 1fr;
overflow: visible;
```

窄屏三栏改为上下堆叠后，内容高度会自然增长并把滚动交给整个页面，而不是交给设计区域、节点区域、结果区域等内部 pane。

## 3. 目标布局原则

- `html`、`body`、`#app` 和 Tauri WebView 的根壳只占用当前窗口高度，不承担业务内容滚动。
- 顶部任务导航和底部状态栏固定在窗口内。
- `.route-outlet` 和活动中的 `.task-panel` 只负责分配剩余高度，不作为全局纵向滚动条。
- 列表、日志、长文本、表格和详情内容在各自区域内部滚动。
- 窗口变窄或高度不足时，区域按比例缩放；不能通过放开整页滚动来“解决”内容溢出。
- 不通过隐藏内容、截断关键数据或固定一个不可适配的像素高度来达成无整页滚动。

## 4. 计划修改范围

### P0：建立稳定的 viewport 高度链

- [ ] 为 `html`、`body` 和 `#app` 建立 `height/block-size: 100%` 的基础高度链。
- [ ] 为根壳增加 `100vh` fallback 和 `100dvh` 优先值，兼容 Tauri WebView 与窄窗口测试环境。
- [ ] 为根壳和根页面设置 `min-height: 0`，并将根级纵向溢出设为 `hidden`。
- [ ] 将 `.app-shell` 改为明确占满窗口的 grid：`top bar / route content / bottom status`。
- [ ] 将 `.route-outlet` 改为 `min-height: 0`、`overflow: hidden`，移除其作为全局纵向滚动容器的职责。
- [ ] 将活动 `.task-panel` 的高度约束闭合，并显式覆盖 header、主体和可选 footer 的 grid 行。
- [ ] 审查全局 `box-sizing` 和 route padding，避免 padding 再次把固定 viewport 高度撑大。

### P0：主界面改为区域内部滚动

- [ ] 设计工作台桌面布局继续使用三栏比例：领域侧栏约 `2fr`、节点主区约 `5fr`、结果区约 `3fr`。
- [ ] 设计工作台的领域列表、节点列表/访谈内容和结果内容分别成为可滚动区域；三栏容器本身不向外撑高。
- [ ] 开发流水线保持左侧步骤列表与右侧详情区的分栏比例；步骤列表、详情长文本和运行日志分别在自己的区域滚动。
- [ ] 补充开发、打包、运行日志和 SDK 页面固定页面 header，分别让表格、输出文本和 SDK 上下文区域内部滚动。
- [ ] 保留已有的长日志、长文本、表格和详情滚动容器，统一补齐其 `min-height: 0`、`min-width: 0` 和可计算的父级高度。
- [ ] 模态框继续使用 fixed backdrop 和内部 body 滚动；模态框滚动不能传播到主页面。

### P1：窄屏和低高度窗口比例适配

- [ ] 移除或改写窄屏规则中的 `overflow: visible`，避免堆叠布局把内容高度传递到根页面。
- [ ] 设计工作台窄屏改为上下分区，建议使用带 `minmax(0, fr)` 的比例行：领域概览较小、节点工作区最大、结果区保留可读的最小比例。
- [ ] 流水线窄屏采用“步骤列表 / 详情与日志”的比例行；每一行内部滚动，不让整个窗口滚动。
- [ ] 存档、模板和配置等已有内部滚动模态框，在窄屏下继续使用 `calc(100dvh - inset)`，并确保 body 区域而不是 backdrop 或 document 滚动。
- [ ] 顶部导航、页面 header 和底部状态栏允许换行，但它们的动态高度必须由中间 route 行扣除，不能覆盖或撑出 viewport。
- [ ] 以 1280×820、1180×720、900×720 和更低高度窗口作为适配样本；具体最小可用高度应以实际控件可读性为准。

### P1：回归测试与可观测性

- [ ] 在 UI gate 中增加根级滚动断言：`document.documentElement`、`document.body`、`#app` 和活动 route 不得出现超出 viewport 的纵向页面滚动。
- [ ] 增加内部滚动白名单，确认长列表、日志、详情和表格在内容变长时仍可滚动。
- [ ] 测试不能只使用空数据；需要注入长项目名、长日志、多个步骤、长详情和长表格内容，验证滚动发生在目标 pane。
- [ ] 增加 desktop/narrow 两种 viewport 的截图检查，并保留中文与英文模式。
- [ ] 对 modal、design、pipeline、patch、package、logs、sdk 全部界面逐一检查根页面滚动高度和目标滚动容器。

## 5. 推荐实现顺序

1. 先修正 `html/body/#app/app-shell/route-outlet` 的高度和根溢出边界。
2. 再逐个修正 design、pipeline、utility 页面主体的 grid 行和 pane 高度。
3. 然后重写窄屏 stacked layout 的比例行，移除 `overflow: visible` 的逃逸路径。
4. 最后补充根级滚动断言、长数据 fixture、截图 gate 和手动验收记录。

## 6. 验收标准

- [ ] 任意主界面打开后，鼠标在空白区域滚轮不会带动整个页面上下移动。
- [ ] 顶部任务导航和底部状态栏在窗口内保持稳定，不因主体内容变长而被推出窗口。
- [ ] 领域列表、节点区、结果区、步骤列表、日志、表格、长文本等目标区域仍可独立滚动。
- [ ] 长数据不会被截断，且不会造成 document/body 的纵向滚动。
- [ ] 窄屏、低高度窗口下仍保持可操作，区域按比例分配，不出现内容覆盖。
- [ ] 模态框内部滚动正常，打开或关闭模态框不会改变主页面滚动位置。
- [ ] UI unit/e2e、UI gate、baseline gate 和双语截图检查通过。

## 7. 参考资料

- [MDN：`overflow` 属性](https://developer.mozilla.org/en-US/docs/Web/CSS/Reference/Properties/overflow)：垂直 `overflow` 要在有明确高度或最大高度约束的容器上才会形成预期的内部滚动。
- [MDN：`min-height` 属性](https://developer.mozilla.org/en-US/docs/Web/CSS/Reference/Properties/min-height)：Grid/Flex 子项的默认最小尺寸可能阻止其收缩，需要在允许内部滚动的层级明确设置 `min-height: 0`。
- [Tauri GitHub Discussion #3093](https://github.com/tauri-apps/tauri/discussions/3093)：相关 Tauri WebView 布局示例采用根层 `overflow: hidden`、内部应用区域 `overflow: auto` 的分层思路；本项目只借鉴溢出边界原则，不改变原生窗口装饰配置。

## 来源

- 用户反馈：所有界面都可以上下滑动，属于错误设计。
- 用户要求：取消全界面上下滚动，改为相关区域内部滚动，并做好界面占比适配。
