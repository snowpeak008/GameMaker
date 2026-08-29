# Workspace and Crate Design

状态：第一轮设计完成。

evidence=

- `NEWrust/Cargo.toml`
- `NEWrust/README.md`
- `python_deconstruction/20_parity_gate_test_matrix.md`

classification=NEWrust authoritative design

confidence=high

open_questions=

- 是否在开发阶段保留 `adm-new-*` 前缀到 release；当前设计保留。

next_read_targets=

- 原子计划阶段读取每个现有 crate 源码。

## 1. 目标目录

```text
NEWrust/
├── apps/
│   ├── adm-new-cli/
│   └── desktop-tauri/
├── web/
├── crates/
│   ├── adm-new-foundation/
│   ├── adm-new-contracts/
│   ├── adm-new-governance/
│   ├── adm-new-storage/
│   ├── adm-new-design/
│   ├── adm-new-save/
│   ├── adm-new-ai/
│   ├── adm-new-pipeline/
│   ├── adm-new-artifact/
│   ├── adm-new-packaging/
│   ├── adm-new-patch/
│   ├── adm-new-sdk/
│   ├── adm-new-application/
│   └── adm-new-tauri-commands/
└── gates/
```

现有 crate：

- `adm-new-foundation`
- `adm-new-contracts`
- `adm-new-governance`
- `adm-new-cli`

这些作为 contract-first 骨架保留，后续逐步扩展。

## 2. crate 职责

| crate | 职责 | 禁止事项 |
| --- | --- | --- |
| `adm-new-foundation` | error、time、ids、stable hash、path safety、atomic file、gate report | 不依赖业务 crate |
| `adm-new-contracts` | serde models、schema version、view model DTO、validation result | 不做文件 IO |
| `adm-new-storage` | project root、repositories、atomic write、file manifests、migration helpers | 不做业务决策 |
| `adm-new-design` | DesignEngine、project_state normalize、coverage、quality、export/handoff pure logic | 不调用 Tauri |
| `adm-new-save` | save index、manifest、draft/formal sync、lock、snapshot、timeline | 不直接渲染 UI |
| `adm-new-ai` | AI config、schema-mode prompt service、completion JSON、high-confidence writeback、memory events | 不让 Web UI 合并状态 |
| `adm-new-pipeline` | stage registry、dependency order、run_range、runtime control、step state | 不硬编码 UI |
| `adm-new-artifact` | artifact registry、preflight、reviewer、validator、schema refs | 不读 UI state |
| `adm-new-packaging` | Step14 packaging validation、build report、manifest、notes | 不信任 UI button state |
| `adm-new-patch` | patch request analyzer、patch manifest、validation route | 不写 pipeline state |
| `adm-new-sdk` | SDK spec store、review status、approved context、optional extraction | 不自动批准 AI 抽取 |
| `adm-new-application` | orchestrates services and transactions | 不包含 UI component code |
| `adm-new-tauri-commands` | command handlers and DTO mapping | 不承载 domain rules |
| `adm-new-governance` | plan gates、score gates、release checks | 不作为产品 service |

## 3. dependency 方向

```text
foundation
  -> contracts
    -> storage
      -> domain crates
        -> application
          -> tauri-commands
```

Allowed reverse dependency：无。

`adm-new-governance` 可以依赖 contracts/foundation/storage 做 gate，但不得被 product crates 依赖。

## 4. Web workspace

`web/` 负责：

- shell layout。
- route views。
- components。
- state/query layer。
- Playwright tests。

Web UI 数据来源只能是：

- Tauri command responses。
- frontend-local transient UI state。

禁止：

- Web 直接读文件系统。
- Web 直接生成 final business JSON。
- Web 复制 Rust domain logic。

## 5. CI/local gates

开发阶段最低命令：

```powershell
cargo fmt --check
cargo check --workspace
cargo test --workspace
npm run build
npm run test
npm run e2e
cargo run -p adm-new-cli -- plan-gate
```

具体 npm 命令在初始化 web 时落地。
