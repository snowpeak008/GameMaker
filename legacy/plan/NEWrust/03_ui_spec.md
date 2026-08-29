# NEWrust UI 规范 v2

## 1. 固定技术路线

UI 路线固定为：

```text
Tauri + Web UI + Rust 后端
```

推荐 Web 技术栈：

- React + TypeScript。
- CSS Modules 或受约束的 Tailwind。
- Playwright 做截图和交互验收。

Web UI 只显示 Rust 后端提供的 view model snapshot，并通过 Tauri command 发起用户操作。

## 2. 禁止事项

- Web UI 直接读写业务文件。
- Web UI 直接拼接 pipeline 制品作为业务事实。
- Tauri command 内堆业务逻辑。
- UI 控件没有对应 service command。
- UI 先于 Python 解构和 service design 开发。
- 用截图非空证明 UI 功能完成。

## 3. 顶层任务区

必须复刻 Python 项目的六任务区：

- 设计工作台
- 开发流水线
- 补充开发
- 打包阶段
- 运行日志
- SDK 知识库

每个任务区必须有：

- Python 入口证据。
- UI 信息架构拆解。
- UI 互动矩阵。
- Rust service command。
- Web view model。
- Playwright 验收。

## 4. 高保真复刻标准

高保真不是单像素 hash 相等，而是：

- 信息结构一致。
- 主流程路径一致。
- 控件行为一致。
- 数据状态变化一致。
- 指定分辨率下布局区域一致。
- 长文本、表格、滚动区、错误态可用。

指定基准分辨率：

- 1280x860
- 1600x900
- 1920x1080

## 5. UI 验收分层

- `DOM contract`：关键元素、状态、按钮、列表存在。
- `Interaction contract`：点击、输入、选择会调用 Tauri command。
- `State contract`：Rust service 后状态变化反映到 UI。
- `Screenshot contract`：真实数据态、空态、错误态截图通过。
- `Manual review`：人工点击复核，不以自动截图替代。

## 6. UI 开发门禁

开始 UI 开发前必须满足：

- Python UI 信息架构评分合格。
- Python UI interaction matrix 评分合格。
- NEWrust service command 设计评分合格。
- 原子任务中已有 UI 依赖链。
- Playwright 验收方案已写入 atomic backlog。

