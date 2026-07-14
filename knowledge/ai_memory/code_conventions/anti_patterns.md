# 反模式与禁止事项

> 基于 CLAUDE.md 规则 + 实际踩坑经验  
> 这些是项目中**绝对不能做**的事情。

---

## 目录结构禁令

### ❌ 禁止在根目录新增任何目录

根目录结构已固定，不允许扩展：

```
AutoDesignMaker/
├── core/               ✓ 允许在此目录内新增子目录
├── pipeline/           ✓ 允许在此目录内新增步骤目录
├── knowledge/          ✓ 允许在此目录内新增知识子类别
├── settings/           ✓ 仅修改文件内容，不新增目录
├── tools/              ✓ 允许在此目录内新增工具类别目录
├── ucos/               ✓ 仅在 knowledge/episodic/ 下操作
├── artifact_layer/     ✓ 仅修改 registry.json
├── _archive/           ✓ 只读参考，不新增内容
├── sandbox/            ✓ 程序自动管理
├── saves/              ✓ 程序自动管理
├── logs/               ✓ 程序自动管理
└── plan/               ✓ 允许放置开发计划文档
```

**错误示例**：
```
❌ AutoDesignMaker/new_module/    # 禁止在根目录新建模块目录
❌ AutoDesignMaker/tests/         # tests/ 应放在 core/tests/
❌ AutoDesignMaker/scripts/       # scripts/ 应放在 tools/scripts/
```

**正确做法**：
```
✓ core/new_module/               # 新核心模块放 core/ 下
✓ tools/scripts/new_script.py    # 新脚本放 tools/scripts/ 下
✓ knowledge/new_category/        # 新知识类别放 knowledge/ 下
```

---

### ❌ 禁止在 core/ 根层级新增顶级模块

`core/` 的顶级结构已定义，新模块必须归入现有子目录：

```
core/
├── adapters/       ✓ AI 适配器放这里
├── artifact/       ✓ 制品审查相关放这里
├── config/         ✓ 配置加载相关放这里
├── design/         ✓ 设计引擎相关放这里
├── engines/        ✓ 业务引擎放这里
├── runtime/        ✓ 运行时控制放这里
├── save/           ✓ 存档相关放这里
├── source/         ✓ 源包系统放这里
├── ui/             ✓ GUI 组件放这里
├── utils/          ✓ 通用工具放这里
└── tests/          ✓ 测试放这里
```

**错误示例**：
```
❌ core/new_engine.py           # 应放 core/engines/new_engine.py
❌ core/helper.py               # 应放 core/utils/helper.py
```

---

### ❌ 禁止在 tools/ 根目录放任何 .py 文件

`tools/` 必须按类别分子目录：

```
tools/
├── validators/       ✓ 验证工具
├── asset_production/ ✓ 媒体制作工具
├── dev/              ✓ 开发辅助
├── scripts/          ✓ 维护脚本
├── build/            ✓ 构建工具
└── memory/           ✓ 记忆系统工具（新增）
```

**错误示例**：
```
❌ tools/update_freshness.py    # 应放 tools/memory/update_freshness.py
❌ tools/helper.py              # 应放 tools/scripts/ 或 tools/dev/
```

---

## 文件大小禁令

### ❌ 禁止创建超过 400 行的新文件

**规则**：新创建的文件不得超过 400 行（不含注释和空行）。超过则必须拆分。

**例外**：现有巨文件（如 `core/engines/generation.py` 3795行）可以保留，但不能继续膨胀，新逻辑应拆到新文件。

**拆分原则**：
- 按职责拆分（不同功能到不同文件）
- 按阶段拆分（generation.py 应拆为 stage_00.py ~ stage_15.py）
- 按层级拆分（UI 层、业务层、数据层分离）

---

## 路径硬编码禁令

### ❌ 禁止在非 core/paths.py 的文件中硬编码路径

**错误示例**：
```python
# ❌ 绝对禁止
output_dir = Path("sandbox/outputs/artifacts/stage_00")
config_file = PROJECT_ROOT / "settings/app.toml"  # PROJECT_ROOT 可以，但 "settings/app.toml" 应用常量
```

**正确做法**：
```python
from core.paths import ARTIFACTS_DIR, APP_CONFIG_FILE
from core.stage import stage_dir

output_dir = stage_dir(0)  # 或 ARTIFACTS_DIR / "stage_00"
config_file = APP_CONFIG_FILE
```

---

## 依赖禁令

### ❌ 禁止 import 已删除的旧模块

以下 import 语句会被 `forbidden_runtime_matches()` 检测并阻止执行：

```python
# ❌ 绝对禁止
import steps.common
from steps.common import STEP_SPECS
from design_tool.engine import DesignEngine
import design_tool

# ❌ 禁止 CrewAI 残留
from crewai import Agent, Task, Crew
import crewai
from crewai_tools import *
```

**正确做法**：
```python
# ✓ 使用迁移后的模块
from core.registry import STEP_SPECS
from core.design.engine import DesignEngine
```

---

## GUI 反模式

### ❌ 禁止在状态变更后调用全量 render()

**问题**：`CommercialDesignApp.render()` 销毁重建所有 widget，导致卡顿。

**错误示例**：
```python
def on_checklist_change(self, node_id, item_id, checked):
    self.engine.set_checklist_item(self.project_state, node_id, item_id, checked)
    self.render()  # ❌ 全量重建，卡顿
```

**正确做法**（待重构）：
```python
def on_checklist_change(self, node_id, item_id, checked):
    self.engine.set_checklist_item(self.project_state, node_id, item_id, checked)
    self._update_node_card(node_id)  # ✓ 只更新受影响的卡片
    self._update_domain_progress(domain_id)  # ✓ 只更新领域进度条
```

---

### ❌ 禁止在子线程直接操作 tkinter widget

**错误示例**：
```python
def worker():
    result = do_work()
    self.log_text.insert(tk.END, result)  # ❌ 子线程直接操作 widget
```

**正确做法**：
```python
def worker():
    result = do_work()
    self.log_queue.put(result)  # ✓ 通过队列传递

def _poll_log_queue(self):
    try:
        line = self.log_queue.get_nowait()
        self.log_text.insert(tk.END, line)  # ✓ 主线程操作
    except queue.Empty:
        pass
    self.after(100, self._poll_log_queue)
```

---

### ❌ 禁止硬编码颜色和字体

**错误示例**：
```python
frame = tk.Frame(parent, bg="#FFFFFF")  # ❌ 硬编码颜色
label = tk.Label(frame, font=("Microsoft YaHei UI", 14))  # ❌ 硬编码字体
```

**正确做法**：
```python
from core.ui.theme import COLORS, FONT_TITLE

frame = tk.Frame(parent, bg=COLORS["surface"])
label = tk.Label(frame, font=FONT_TITLE)
```

---

## 错误处理反模式

### ❌ 禁止按进程名批量终止 Codex / sandbox 进程

**问题**：`Stop-Process` / `taskkill` 按 `codex.exe`、`node.exe`、`sandbox` 等进程名或模糊 PID 清理，可能会杀掉当前 AI 会话自身，导致运行中断。

**错误示例**：
```powershell
Get-Process | Where-Object { $_.ProcessName -like "*codex*" } | Stop-Process -Force
Stop-Process -Id 1234,5678 -Force  # 未证明这些 PID 不属于当前会话进程树
```

**正确做法**：
- 默认只做只读诊断，不自动杀进程。
- 如确需终止进程，必须先证明目标 PID 不在当前 Codex/Claude 会话自身进程树内。
- 终止外部 CLI 残留前，列出 PID、命令行、父进程、启动时间和判断依据，并让用户确认。
- 不为破坏性进程清理申请持久 approval rule。

### ❌ 禁止用宽泛的 except 捕获业务逻辑错误

**错误示例**：
```python
def execute(self, ctx):
    try:
        result = complex_business_logic()
        return StageResult(status="success", outputs=result)
    except Exception as e:  # ❌ 过于宽泛，会掩盖真正的 bug
        return StageResult(status="failed", errors=[str(e)])
```

**正确做法**：
```python
def execute(self, ctx):
    result = complex_business_logic()  # 让真正的 bug 直接暴露
    if not result.get("valid"):
        return StageResult(status="failed", errors=result.get("errors", []))
    return StageResult(status="success", outputs=result)
```

**例外**（允许的 try/except）：
```python
# ✓ 捕获具体的外部操作异常
try:
    data = json.loads(path.read_text(encoding="utf-8"))
except FileNotFoundError:
    return {"status": "failed", "errors": [f"File not found: {path}"]}
except json.JSONDecodeError as e:
    return {"status": "failed", "errors": [f"Invalid JSON: {e}"]}
```

---

## 提交禁令

### ❌ 禁止提交以下文件到 git

```
settings/api_config.toml          # 含 API 密钥
settings/project_settings.json    # 含本地路径
sandbox/                          # 运行时输出
saves/                            # 存档
logs/                             # 日志
bug收集文档*.md                   # 用户本地 bug 检查输入，不入库
bug优化文档*.md                   # 用户本地 bug 优化输入，不入库
plan/l5_entity_ai_supplement/      # 临时开发执行计划，不入库
*.pyc                             # 编译缓存
__pycache__/                      # Python 缓存
.venv/                            # 虚拟环境
```

这些文件已在 `.gitignore` 中，但要确保不要用 `git add -f` 强制添加。

---

## 性能反模式

### ❌ 禁止在循环内重复读取同一文件

**错误示例**：
```python
for step_num in range(16):
    registry = read_json(PROJECT_ROOT / "artifact_layer" / "registry.json")  # ❌ 重复读取
    artifacts = registry["artifacts"]
    # ...
```

**正确做法**：
```python
registry = read_json(PROJECT_ROOT / "artifact_layer" / "registry.json")  # ✓ 循环外读一次
for step_num in range(16):
    artifacts = registry["artifacts"]
    # ...
```

---

### ❌ 禁止在每次调用时重新构建昂贵数据结构

**错误示例**（DesignEngine）：
```python
def domain_coverage(self, project_state, domain_id):
    # ❌ 每次调用都重建 node_by_id
    node_by_id = {node["id"]: node for node in self.nodes}
    # ...
```

**正确做法**：
```python
def __init__(self, ...):
    self.node_by_id = {node["id"]: node for node in self.nodes}  # ✓ 构造时建一次

def domain_coverage(self, project_state, domain_id):
    # ✓ 直接使用
    # ...
```

---

## 命名反模式

### ❌ 禁止用缩写或单字母变量（除循环索引外）

**错误示例**：
```python
cfg = load_config()  # ❌ 缩写
pth = PROJECT_ROOT / "data"  # ❌ 缩写
r = run_import_step()  # ❌ 单字母
```

**正确做法**：
```python
config = load_config()
data_path = PROJECT_ROOT / "data"
report = run_import_step()
```

**例外**（允许的缩写）：
```python
for i, item in enumerate(items):  # ✓ i 作为索引
ctx = StageContext(...)  # ✓ ctx 是约定俗成的 context 缩写
```

---

## 架构反模式

### ❌ 禁止在步骤插件中直接调用其他步骤的 _stageN_outputs()

**错误示例**：
```python
# 在 step_03 的 plugin.py 中
from core.engines.generation import _stage02_outputs

def execute(self, ctx):
    stage02_result = _stage02_outputs(2, {})  # ❌ 跨步骤直接调用
```

**正确做法**：
```python
# ✓ 通过制品依赖声明，让编排器按拓扑序执行
# artifact_layer/registry.json
{
  "id": "stage_03.program_requirements",
  "depends_on": ["stage_02.design_freeze_bundle"]  # ✓ 声明依赖
}
```

---

## 总结：10大禁令

1. **不在根目录新建目录**
2. **不在 core/ 根层级新建模块**
3. **不在 tools/ 根目录放 .py 文件**
4. **不创建超过 400 行的新文件**
5. **不硬编码路径**（除 core/paths.py）
6. **不 import 旧模块**（steps.*, design_tool.*, crewai.*）
7. **不在 GUI 状态变更后全量 render()**
8. **不在子线程直接操作 widget**
9. **不硬编码颜色/字体**
10. **不提交密钥和本地配置**
