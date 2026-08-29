# 流水线产物隐藏与 Step07 图片预览修复

## 记录状态

- 已完成（2026-07-11）：通用产物列表、路径、正文、raw outputs 与 Base64 文本不再进入可见界面；Step07 使用 Blob URL 显示有效 PNG，并区分真实生成、明确 fallback、失败和旧 1×1 占位图。
- Step07/API/CLI 图片执行已接入安全工作单元、私有缓存和隔离工作目录；真实外部 Provider 的账号级质量仍需用户环境验收。

## 问题

用户运行项目导出和 Step00 后，界面展示了多个内部产物文件。用户不需要在流水线详情中浏览这些文件，也不希望点击后看到二进制内容的 `[编码=base64]`。

运行存档并到达 Step07 时，风格参考图没有以图片形式展示。NEWrust 生成的是 1×1 PNG 占位字节，前端没有图片预览组件，产物读取器把 PNG 当作二进制文本显示为 Base64。

## 根因

- `NEWrust/web/src/features/pipeline.js` 在步骤详情中渲染 `pipeline-artifacts` 列表，并提供“查看内容”按钮。
- `stage_result.rs` 仍然把 `artifact_records` 放入步骤输出，这是流水线校验和持久化所需的数据；前端的“步骤输出”也可能把这些内部字段直接序列化显示。
- `NEWrust/crates/adm-new-pipeline/src/stages/step07.rs` 原先始终写入固定 1×1 `PNG_PLACEHOLDER`，没有调用 active image provider。
- Step07 风格卡片只显示 `image_path` 文本，没有通过 `read_pipeline_artifact` 转换成图片数据 URI。

## 后续拟议方案（未实施）

### 界面

- 保留后端产物记录和文件，不再在流水线步骤详情中渲染产物列表、产物读取按钮或二进制正文。
- 步骤输出显示时过滤 `artifact_records`、`artifacts`、产物索引/清单、图片路径、相对路径和内容预览等内部文件字段。
- Step07 风格卡片增加图片预览：通过现有 `read_pipeline_artifact` 读取 PNG，转换为 `data:image/png;base64,...` 后交给 `<img>`，不把 Base64 文本展示给用户。
- 图片加载失败只显示“图片预览不可用/加载失败”，不回退到 Base64 文本。

### 流水线与图片生成

- `adm-new-ai` 增加 active image entry 的实际生成入口：
  - `codex_cli_image` 尝试调用本地 Codex CLI image_gen。
  - API 图片配置尝试调用已有的 Responses/Image Generations 请求形状并解码 PNG。
  - 外部 provider 不可用时返回安全错误，由 Step07 使用可检查的回退图。
- Step07 生产运行时把项目根目录传给生成器，从 `settings/ai_config.json` 解析当前图片配置；测试模式仍可离线运行。
- 固定 1×1 字节占位图替换为基于风格 palette 生成的 640×384 PNG 回退图，确保即使没有外部 provider 也能看到可检查的风格参考。
- generation manifest 增加真实生成数和回退失败数，保留每个选项的状态与原因。

## 验证

- 本轮未验证上述拟议方案，因为未进行开发。
- 回退后仅确认原有 `cargo fmt --all -- --check`、`cargo check -p adm-new-ai -p adm-new-pipeline` 和 `npm test` 仍可通过。

## 后续事项

- 真实图片质量仍取决于当前 active image provider、模型和提示词；provider 不可用时的 640×384 图明确属于回退参考图，不应冒充 AI 生成结果。
- 后续可根据实际 provider 返回协议补充更多图片格式和 API 模式，但不能重新把二进制内容作为文本展示。

## 来源

- `NEWrust/web/src/features/pipeline.js`
- `NEWrust/web/src/styles.css`
- `NEWrust/web/src/locales/pipeline.js`
- `NEWrust/crates/adm-new-ai/src/image.rs`
- `NEWrust/crates/adm-new-pipeline/src/stages/step07.rs`
- `NEWrust/crates/adm-new-pipeline/src/product_executor.rs`
- `NEWrust/crates/adm-new-pipeline/src/stage_result.rs`
