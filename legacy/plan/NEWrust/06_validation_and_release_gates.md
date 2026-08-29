# NEWrust 测试、验收与发布门禁

## 1. 验收分层

| 层级 | 名称 | 说明 |
| --- | --- | --- |
| L0 | Static | 格式、目录、文件体量、schema 存在 |
| L1 | Unit | 单 crate 纯逻辑 |
| L2 | Contract | render/parse/hash/validate 往返 |
| L3 | Service | application command 持久化和事务 |
| L4 | UI | view model、交互、截图、真实数据态 |
| L5 | Local Release | 本地发布包和同 hash 报告 |
| L6 | External | 真实 AI provider 和 Unity PlayMode |

每个需求必须声明它需要证明到哪一层。

## 2. 初始命令

Phase 0 至 Phase 2 最低命令：

```powershell
cargo fmt --check
cargo check --workspace
cargo test --workspace
cargo run -p adm-new-cli -- plan-gate
```

后续新增：

```powershell
cargo run -p adm-new-cli -- content-gate
cargo run -p adm-new-cli -- ui-audit
cargo run -p adm-new-cli -- release-gate
cargo run -p adm-new-cli -- external-acceptance
```

## 3. Release gate

release gate 必须校验：

- exe exists。
- release manifest exists。
- manifest hash == actual exe hash。
- local smoke report build_hash == manifest hash。
- ui audit build_hash == manifest hash。
- handoff status build_hash == manifest hash。
- source bundle hash present。
- stale report count == 0。

任一项失败，release 不可交付。

## 4. External acceptance gate

真实外部验收必须区分：

- no credentials：blocked，不是 passed。
- no Unity：blocked，不是 passed。
- mock provider：local only，不是 real。
- fake Unity：local only，不是 real。
- dry-run：planning evidence，不是 execution evidence。

真实 AI provider 通过条件：

- provider_is_mock=false。
- configured_ready=true。
- invoke_attempted=true，如果 strict 模式要求。
- invoke_succeeded=true，如果 strict 模式要求。
- output validation passed。

真实 Unity 通过条件：

- Unity executable discovered。
- runner=unity_playmode。
- runtime report ready=true。
- imported into archive。
- Step14 acceptance links to runtime evidence。

## 5. UI gate

UI gate 必须检查：

- 技术选型 ADR 存在。
- UI 文件体量合规。
- 每个控件映射到 command。
- 每个 command 有 service 测试。
- 六任务区真实数据态截图。
- 六任务区错误态截图。
- 长文本、长列表、窄宽窗口布局。

## 6. Content gate

Content gate 必须检查：

- Step00-14 全部生成。
- 每个 Step 有 required sections。
- 每个 Step 有 typed contract。
- 每个 Step 有 downstream consumers。
- placeholder rejection pass。
- semantic consistency pass。
- evidence level 明确。

## 7. Completion audit

每次声称完成前，必须逐项列出：

- 需求。
- 证据。
- 证据层级。
- 是否足够。
- 剩余风险。

缺少证据时，默认未完成。

