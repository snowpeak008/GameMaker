# 014 UI 一致性与前端公共工具

## 目标

在不调整页面内容和业务流程的前提下，修复未样式化控件、增强窄屏导航可发现性，并移除无业务价值的前端重复代码。

## 原子工作

1. 为 `.secondary-button` 定义与现有次级命令一致的尺寸、边框、禁用、hover 和 focus-visible 样式。
2. 为按钮、输入框、选择框和文本域增加统一键盘焦点环。
3. 窄屏任务导航保留横向滚动，增加稳定细滚动条和 scroll-snap，确保隐藏入口可发现。
4. 不改变任何按钮文案、任务顺序、面板结构和命令绑定。
5. 新增 `web/src/shared/value.js` 与 `web/src/shared/dom.js`，提取相同的 `read/asArray/clear/el`。
6. 各 feature 仅删除完全等价的本地实现；带业务语义的错误映射、content-origin 和命令包装继续留在所属模块。
7. 增加公共工具单元测试及 CSS 静态断言。

## 验收

- AI 配置的浏览、预览和检测按钮不再显示浏览器原生样式。
- 390px 窄屏可明确看到任务导航可横向滚动，且没有文字裁切或布局跳动。
- 公共工具迁移前后 Web unit/e2e/UI 截图结果一致。

## 实施结果（2026-07-14）

状态：**已完成**。新增 `web/src/shared/value.js` 和 `web/src/shared/dom.js`，相关 feature 已迁移到共享实现；补齐 `.secondary-button`、全局 `focus-visible`、窄屏滚动条与 scroll-snap。桌面和 390px 窄屏截图复核无重叠，重试按钮保持单行。
