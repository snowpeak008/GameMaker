# Web UI Design

状态：第一轮设计完成。

evidence=

- `python_deconstruction/19_ui_reproduction_specs.md`
- `python_deconstruction/06_ui_interaction_matrix.md`

classification=NEWrust authoritative design

confidence=high

open_questions=none

next_read_targets=none

## 1. UI Style

The Web UI must preserve the Python tool feel:

- dense operational application。
- fixed top task switcher。
- fixed bottom status bar。
- muted surfaces and clear table/tree layouts。
- no marketing hero。
- no decorative gradients/orbs。

Use CSS tokens mirroring `core/ui/theme.py`。

## 2. Component Map

```text
AppShell
├── TopTaskBar
├── RouteOutlet
│   ├── DesignWorkbench
│   ├── PipelinePanel
│   ├── PatchPanel
│   ├── PackagePanel
│   ├── LogPanel
│   └── SdkPanel
├── BottomStatusBar
└── AiConfigDialog
```

## 3. Design Workbench Components

```text
DesignWorkbench
├── DesignTopbar
├── WorkspaceSplit
│   ├── DomainSidebar
│   │   ├── ProfilePanel
│   │   └── DomainCardList
│   ├── NodeAndInterviewSplit
│   │   ├── NodePanel
│   │   │   ├── DomainHeader
│   │   │   ├── NodeToolbar
│   │   │   └── NodeCardList
│   │   └── AiInterviewPanel
│   └── ResultTabs
└── DesignStatusBar
```

NodeCard must include:

- progress count。
- L4 badge。
- L5 badge for concrete nodes。
- decision state badge。
- checklist cards。
- option chips。
- note/risk/not-applicable editors。
- design entities JSON editor。

## 4. Pipeline Components

```text
PipelinePanel
├── StepSidebar
├── PipelineMain
│   ├── PipelineConfigBar
│   ├── StepDetail
│   ├── StyleConfirmationGrid
│   └── RuntimeLogPane
```

Step07 grid:

- three columns on desktop。
- image max equivalent 330x225。
- radio selection。
- notes textarea。
- confirm/regenerate。
- fullscreen image preview。

## 5. Utility Panels

Patch:

- request textarea。
- analyze/refresh actions。
- table columns exactly from Python labels。

Package:

- toolbar。
- output JSON text panel。
- blocked issues visible。

Logs:

- level combobox。
- clear button。
- table。

SDK:

- name/url form。
- status update buttons。
- table。
- approved context text area。

## 6. Responsive Rule

Desktop is primary. Target breakpoints:

- `>=1180px`: full parity layout。
- `900-1179px`: keep top/bottom bars, allow horizontal scroll in complex panels。
- `<900px`: stack panels vertically where required; no text overlap。

Pixel parity is judged at desktop viewports defined in `19_ui_reproduction_specs.md`。
