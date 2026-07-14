# 代码模式与惯例

> 基于 2026-06-19 代码阅读  
> 来源：实际项目代码观察

以下是项目中实际使用的代码模式。遵守这些模式能保证代码风格统一。

---

## 文件头规则

**所有 .py 文件必须有 `from __future__ import annotations`**

```python
from __future__ import annotations

import json
from pathlib import Path
from typing import Any
```

**文档字符串**（可选，但推荐给复杂模块）：
```python
"""模块一句话描述。

详细说明（可选）。
"""
```

---

## StagePlugin 实现模式

所有步骤插件遵循统一模式：

```python
from core.stage_plugin import StagePlugin
from core.context import StageContext, StageResult
from core.source.groups import SourceGroup
from core.source.importer import run_import_step
from core.engines.generation import apply_development_plan_outputs


class Plugin(StagePlugin):
    stage_id = "NN"  # 两位数字字符串
    _source_groups = [
        SourceGroup("label", ("pattern_*",), "latest", True, ("SourceType",))
    ]

    def execute(self, ctx: StageContext) -> StageResult:
        if ctx.test_mode:
            return StageResult(status="success", outputs={"stage_id": self.stage_id})
        
        report = run_import_step(int(self.stage_id), self._source_groups, context=ctx)
        result = apply_development_plan_outputs(int(self.stage_id), report)
        
        if isinstance(result, dict):
            status = result.get("status", "success")
            return StageResult(status=status, outputs=result)
        return result
```

**关键点**：
- `stage_id` 是类属性（不是实例属性）
- `_source_groups` 定义源包导入规则
- `execute()` 先检查 test_mode，再调用 run_import_step → apply_development_plan_outputs
- 返回 StageResult，不抛异常

---

## 路径管理模式

**规则**：所有路径常量在 `core/paths.py` 定义，其他文件禁止硬编码路径字符串。

```python
# ✓ 正确
from core.paths import ARTIFACTS_DIR, stage_dir
output_path = ARTIFACTS_DIR / f"stage_{step_num:02d}" / "report.json"

# ✓ 也正确（使用辅助函数）
from core.stage import stage_dir
output_path = stage_dir(step_num) / "report.json"

# ✗ 错误
output_path = Path("sandbox/outputs/artifacts/stage_00/report.json")  # 硬编码
```

**路径拼接**：优先用 `pathlib.Path` 的 `/` 运算符，不用 `os.path.join`。

```python
# ✓ 推荐
file_path = PROJECT_ROOT / "knowledge" / "schemas" / "workflow.json"

# ✗ 避免
file_path = os.path.join(PROJECT_ROOT, "knowledge", "schemas", "workflow.json")
```

---

## 错误处理模式

**规则**：不用 try/except 包裹业务逻辑，而是在返回值中表达失败状态。

```python
# ✓ 正确（StagePlugin）
def execute(self, ctx: StageContext) -> StageResult:
    report = run_import_step(...)
    if not report.get("sources"):
        return StageResult(
            status="failed",
            errors=["No source packages found"]
        )
    result = apply_development_plan_outputs(...)
    return StageResult(status=result.get("status", "success"), outputs=result)

# ✓ 正确（引擎层）
def _stage0_outputs(step_number: int, report: dict[str, Any]) -> dict[str, Any]:
    result = {"status": "success", "outputs": {}}
    if not validate_input(report):
        result["status"] = "failed"
        result["errors"] = ["Invalid input"]
        return result
    # ... 业务逻辑
    return result

# ✗ 错误
def execute(self, ctx: StageContext) -> StageResult:
    try:
        result = dangerous_operation()
    except Exception as e:
        return StageResult(status="failed", errors=[str(e)])  # 过于宽泛
```

**例外**：文件 I/O、网络请求等外部操作可以用 try/except，但要捕获具体异常。

```python
# ✓ 可接受
try:
    data = json.loads(path.read_text(encoding="utf-8"))
except (FileNotFoundError, json.JSONDecodeError) as e:
    return {"status": "failed", "errors": [f"Cannot load {path}: {e}"]}
```

---

## JSON 读写模式

**规则**：统一用 `core/io.py` 的 read_json/write_json。

```python
from core.io import read_json, write_json

# ✓ 正确
data = read_json(path, default={})  # 文件不存在时返回 default
write_json(path, data)  # 自动创建父目录，自动 ensure_ascii=False

# ✗ 错误
with open(path, "r", encoding="utf-8") as f:
    data = json.load(f)  # 不处理文件不存在
with open(path, "w", encoding="utf-8") as f:
    json.dump(data, f)  # 不自动创建父目录
```

---

## 类型注解模式

**规则**：函数参数和返回值必须有类型注解。

```python
# ✓ 正确
def stage_dir(step_number: int) -> Path:
    return ARTIFACTS_DIR / f"stage_{step_number:02d}"

def read_json(path: Path, default: Any = None) -> Any:
    if not path.exists():
        return default
    return json.loads(path.read_text(encoding="utf-8"))

# ✗ 错误
def stage_dir(step_number):  # 缺少类型注解
    return ARTIFACTS_DIR / f"stage_{step_number:02d}"
```

**类属性注解**（dataclass 或普通类）：

```python
from dataclasses import dataclass

@dataclass
class StageContext:
    stage_id: str
    inputs: dict[str, Any]
    outputs: dict[str, Any]
    test_mode: bool = False
```

---

## 命名规则

| 类型 | 规则 | 示例 |
|------|------|------|
| 模块 | snake_case | `generation.py`, `plugin_manager.py` |
| 类 | PascalCase | `StagePlugin`, `ModelAdapter` |
| 函数/方法 | snake_case | `run_import_step()`, `apply_development_plan_outputs()` |
| 常量 | UPPER_SNAKE_CASE | `PROJECT_ROOT`, `STEP_SPECS` |
| 私有函数 | _snake_case | `_stage0_outputs()`, `_parse_design_text()` |
| 临时变量 | snake_case | `report`, `result`, `data` |

**文件命名**：
- 步骤插件目录：`step_{NN}_{slug}/`（例如 `step_03_program_requirements/`）
- 适配器文件：`{model}_adapter.py`（例如 `openai_adapter.py`）
- 引擎文件：功能名词（`generation.py`, `delta_patch.py`）

---

## 函数长度规则

**规则**：单个函数不超过 50 行（不含空行和注释）。超过则拆分。

**例外**：`core/engines/generation.py` 中的 `_stageN_outputs()` 函数可以超过 50 行，因为每个都是独立逻辑块，拆分后反而难以维护。

---

## 注释规则

**默认不写注释**。只在以下情况写注释：

1. **非显而易见的"为什么"**（不是"做什么"）
2. **隐藏的约束或不变量**
3. **绕过某个 bug 的 workaround**
4. **会让读者困惑的代码**

```python
# ✓ 好注释
# Codex CLI 需要绝对路径，相对路径会导致 CWD 混乱
task.input_files = [str(Path(f).resolve()) for f in input_files]

# ✗ 坏注释
# 创建目录
path.mkdir(parents=True, exist_ok=True)  # 代码已经说明了做什么
```

**文档字符串**（docstring）：
- 模块级：可选，描述整个模块的职责
- 类级：可选，描述类的作用和用法
- 函数级：**不写**，除非是公开 API 或逻辑极其复杂

---

## dataclass 模式

**优先用 dataclass 定义数据结构**。

```python
from dataclasses import dataclass, field

@dataclass(frozen=True)  # 不可变
class StepSpec:
    number: int
    slug: str
    title: str
    requires: tuple[int, ...] = field(default_factory=tuple)

@dataclass  # 可变
class StageContext:
    stage_id: str
    inputs: dict[str, Any] = field(default_factory=dict)
    outputs: dict[str, Any] = field(default_factory=dict)
```

---

## 导入顺序

```python
# 1. __future__
from __future__ import annotations

# 2. 标准库（按字母序）
import json
import re
from datetime import datetime
from pathlib import Path
from typing import Any

# 3. 第三方库（按字母序）
import toml

# 4. 本项目模块（按层级）
from core.paths import PROJECT_ROOT, ARTIFACTS_DIR
from core.io import read_json, write_json
from core.stage_plugin import StagePlugin
```

---

## GUI 组件模式

**颜色和字体必须从 theme.py 引用**。

```python
from core.ui.theme import COLORS, FONT_BODY, FONT_TITLE

# ✓ 正确
frame = tk.Frame(parent, bg=COLORS["surface"])
label = tk.Label(frame, text="标题", font=FONT_TITLE, fg=COLORS["text"])

# ✗ 错误
frame = tk.Frame(parent, bg="#FFFFFF")  # 硬编码颜色
label = tk.Label(frame, text="标题", font=("Microsoft YaHei UI", 14))  # 硬编码字体
```

---

## 禁止的 import 语句

```python
# ✗ 绝对禁止（已删除的旧模块）
import steps.common
from design_tool.engine import DesignEngine

# ✗ 禁止（CrewAI 残留）
from crewai import Agent, Task
import crewai
```

`core/source/importer.py::forbidden_runtime_matches()` 会检查这些 import，发现后阻止执行。

---

## 线程安全模式（GUI）

**规则**：子线程不直接操作 tkinter widget，通过 `after()` 调度到主线程。

```python
import queue
import threading

class MyPanel(tk.Frame):
    def __init__(self, parent):
        super().__init__(parent)
        self.log_queue = queue.Queue()
        self._start_polling()
    
    def _start_polling(self):
        """主线程定时消费队列"""
        try:
            while True:
                line = self.log_queue.get_nowait()
                self.log_text.insert(tk.END, line + "\n")
        except queue.Empty:
            pass
        self.after(100, self._start_polling)  # 100ms 后再次调度
    
    def run_background_task(self):
        """启动后台线程"""
        def worker():
            # 后台逻辑
            self.log_queue.put("Task started")
            # ... do work
            self.log_queue.put("Task finished")
        threading.Thread(target=worker, daemon=True).start()
```

---

## 总结：5条铁律

1. **路径统一**：core/paths.py 是唯一来源
2. **StagePlugin 统一**：stage_id + _source_groups + execute() 三件套
3. **错误返回**：status="failed" 而非抛异常
4. **JSON 统一**：read_json/write_json 而非 open()
5. **主题统一**：COLORS/FONT_* 而非硬编码
