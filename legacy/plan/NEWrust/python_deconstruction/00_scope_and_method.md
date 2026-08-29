# Python 解构范围与方法

## 1. 解构范围

优先入口：

- `gui_app.py`
- `core/main.py`
- `core/ui/main_window.py`
- `pipeline/_registry.json`
- `core/registry.py`
- `core/paths.py`
- `core/save/manager.py`
- `core/runtime/`

扩展范围：

- `core/ui/`
- `core/design/`
- `core/engines/`
- `core/artifact/`
- `core/config/`
- `core/adapters/`
- `core/packaging/`
- `pipeline/step_*/`
- `pipeline/artifact_layer/`
- `knowledge/schemas/`
- 关键 `knowledge/design_data/`

## 2. 分类规则

- `authoritative`：真实入口可达。
- `reference`：历史或辅助实现，有参考价值。
- `quarantine`：垃圾、废弃、重复、临时、历史残留。

## 3. 证据规则

每个必须复刻功能都需要：

```text
entry_point -> callback/command -> service/function -> data read/write -> output/effect
```

没有入口证据，不进入 `must replicate`。

