# A08a Step07 美术规范与风格锚点

状态：已完成（本地，待验收提交）
依赖：A07

## 目标

Step07 只生产美术规范、风格锚点和少量代表资产；建立确定性资产硬门禁。**不做批量生产**（批量在 A08d/Step12）。

## 变更

1. 从 `PresentationSpec` 编译美术规范（风格、色彩方向、构图约束、用途分类）与代表资产任务（主角、核心 UI 等关键项，数量由宪章档位决定）。
2. 风格锚点流程：代表资产生成 → **人工确认**（attended，不可 auto_accept）→ 入锚点参考集，作为后续一致性比对基准（继承旧 Python 版已验证模式）。
3. **确定性硬门禁**（重建，废弃 stage12.rs 的"available 即 passed"）：解码、尺寸、alpha、切片几何、透明边距、重复图检测、OCR 水印、对比度、文件完整性。
4. VLM 评审通道：结构化评分与差异描述，只作证据入审计；按图像内容哈希缓存，重跑不重评。VLM 缓存机制细节在本任务启动时设计（原则已冻结：证据不是裁判、可回放）。
5. 图像生成路由复用现有 adapter；提示词来源可追踪到规格字段。
6. UI 只在 Step07 视图展示本步产物。

## 验收（停止门）

- 通道防守 fixture 产出完整美术规范 + 已确认锚点集 + 代表资产，全部通过硬门禁。
- 硬门禁对构造的坏样本（错尺寸、无 alpha、重复图、含文字水印）全部 fail。
- 同一图像重复评审命中缓存，审计含 VLM 配置 ID 与摘要哈希。

## 回滚

删除新 Step07 编译器与门禁模块；旧 art_pipeline 路径未删除仍可用。

## 实际结果

- 新增 `adm-new-pipeline::stages::step07_v2`，从 GameSpec 的 `PresentationSpec`/实体标签编译美术规范、代表资产任务、风格锚点候选。
- Step07 v2 只生成 4 个代表资产锚点，不做批量资产生产；批量生产仍留给 A08d/Step12。
- 代表 PNG 使用确定性生成，硬门禁覆盖解码、尺寸、alpha、透明边距、重复图、对比度、水印/文字样式启发式检测。
- `confirm_style_anchors_attended` 明确拒绝 `auto_accept`，只有 attended 确认后才写入 `style_anchor_set.json`。
- 新增 `CachedVlmReviewService`，按图像内容哈希缓存 VLM 评审证据，审计记录 `configId` 与 `summaryHash`；评审证据不参与硬裁决。

## 验证

- `cargo fmt --all`
- `cargo test -p adm-new-pipeline step07_v2`
