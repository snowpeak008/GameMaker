# A08d Step12 批量资产生产与运行时绑定

状态：已完成（v2 内核，本地，待验收提交）
依赖：A08a、A08b

## 目标

按冻结 AssetManifest 批量生成、导入和绑定资产；集成的定义是真实引擎加载，不是文件到位。

## 变更

1. 按 Step09 冻结的 AssetManifest 批量生成图像（复用 A08a 的生成路由、硬门禁与 VLM 缓存评审）；与风格锚点做一致性比对。
2. 确认策略：批量资产默认 `sample(n)` 抽样人工 + 硬门禁全检；关键资产逐张 attended。
3. **运行时绑定三条硬规则**：
   - 每个资产至少被一个场景/预制体引用，孤儿资产 fail；
   - headless/smoke 运行逐个实例化加载，失败 fail；
   - 引用了但不存在的资产在本步阻塞，不漏到 Step13。
4. 元数据、引用、许可、格式检查；失败资产进修正队列（复用 A08c 队列机制），支持断点续跑。

## 验收（停止门）

- 通道防守 fixture 全量资产生成、导入、绑定通过；引擎加载验证零失败或残留全部人工解决。
- 孤儿资产、缺失引用、锚点偏离的负样例全部 fail。
- 审计含每资产的门禁结果与确认记录。

## 实际结果

- 新增 `adm-new-pipeline::stages::step12_v2`，拆分为 facade、`types.rs`、`production.rs`、`image_support.rs`。
- 直接消费 A08b `FrozenAssetManifest` 与 A08a `StyleAnchorCandidate`，生成批量 PNG，复用 A08a `validate_anchor_images` 做尺寸、alpha、透明边距、重复图像、对比度和水印式标记硬门禁。
- 增加锚点平均色距离检查，资产超过 `style_distance_threshold` 时进入修正队列。
- 增加 `AssetProductionPolicy`：支持 attended 全量批准和 sample(n) 策略；关键资产可逐张 attended，批量资产保留抽样策略字段。
- 增加运行时绑定图与 `EngineAssetLoader` trait；当前默认实现为 `DeterministicHeadlessAssetLoader`，用于无 Unity 绑定环境下验证“每个资产被场景/预制体引用并可被加载探针实例化”。真实 Unity 加载器应在 A08e/A08f 或验收环境接入同一 trait。
- 输出 `raw_generated_asset_manifest.json`、`processed_asset_manifest.json`、`asset_import_report.json`、`asset_binding_graph.json`、`engine_load_binding_report.json`、`asset_confirmation_report.json`、`asset_correction_queue.json`、`step12_asset_production_output.json`。
- 修复批量生成器重复图像风险：资产序号参与颜色和纹理变化，避免不同资产产生相同内容哈希。

## 验证

- `cargo fmt --all`：通过。
- `cargo test -p adm-new-pipeline --test step12_v2`：3 passed。
- `cargo test -p adm-new-pipeline`：168 passed；集成测试 1 + 4 + 3 passed；doc tests 0。
- 负样例覆盖：孤儿资产 fail、引用不存在资产 fail、风格锚点偏离进入修正队列。

## 回滚

删除新 Step12 执行器；生成的资产按清单可整体清除。
