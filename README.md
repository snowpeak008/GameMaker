# AutoDesignMaker V4

第四版：决策驱动的游戏设计与生产工作台。全新工程，与历代版本代码零关联。

## 架构一句话

用户在决策工具中完成创作（手动选择 ⇄ AI 访谈分层确认，AI 只提案、永不代提交），经**冻结门五道**（完备度→一致性→换皮→AI 红队→哈希冻结）固化为唯一内容真相源 `FrozenDesign`，再由 **C0-C6 文档编译流水线**（C5 风格、C6 签收两道人工门）产出双格式文档集；**模板生产线**独立于项目流程，把成熟游戏逆向为带证据链的认证答卷，用于新项目预填与对照；Phase 2（P0-P5 EXE 生产，仅 L6 深度档）本版只保留架构接缝，另行立项。

## 文档导航

- **设计真相源**：`docs/design/00_第四版总体设计.md` 起（00-06 七份，开发争议以此为准）
- **计划四件套**：`docs/plan/`（01 设计计划 / 02 开发计划 / 03 工程规范 / 04 子任务排序与派发）
- **跨会话记忆**：`docs/memory/00_续接开发入口.md`（新会话从这里接管）
- **设计空间清单**：`knowledge/design_space/`（universal + lane_defense + grid_strategy）

## 构建与测试

```powershell
cargo check --workspace
cargo test --workspace
cargo run -p adm4-cli -- space validate
cargo build -p adm4-desktop --release

# CLI 全链冒烟（零网络、临时目录隔离，覆盖逆向五步→预填→访谈→冻结→C0-C6）
powershell -ExecutionPolicy Bypass -File scripts\cli_smoke.ps1
```

## 工作流（CLI）

任意子命令加 `--help` 查看中文详情。AI 相关命令默认使用 `config/app.json` 配置的真实 Provider（`ai doctor` 可诊断）；`--scripted-file <应答文件>` 为确定性离线测试开关。

### 项目主线：创建 → 创作 → 冻结 → 编译

```powershell
# 校验设计空间清单
cargo run -p adm4-cli -- space validate lane_defense

# 创建项目（选品类包 + 深度档；--template 用认证模板预填）
cargo run -p adm4-cli -- project new "我的塔防" --pack lane_defense --depth L5

# 脚本化创作（GUI 之外的自动化通道）
cargo run -p adm4-cli -- authoring status <archive_id>
cargo run -p adm4-cli -- authoring select <archive_id> <决策点> <选项>
cargo run -p adm4-cli -- authoring confirm <archive_id> <决策点>

# 冻结门五道评估与冻结
cargo run -p adm4-cli -- freeze red-team <archive_id>
cargo run -p adm4-cli -- freeze check <archive_id>
cargo run -p adm4-cli -- freeze run <archive_id>

# C0-C6 文档编译流水线（停在 C5/C6 人工门时用 confirm 放行）
cargo run -p adm4-cli -- pipeline run <archive_id>
cargo run -p adm4-cli -- pipeline confirm <archive_id> C5 <确认人> "风格方向确认"
cargo run -p adm4-cli -- pipeline status <archive_id>
```

### 模板生产线（逆向五步，状态机只进不跳）

```powershell
# S0 新建草稿（--game 必填，游戏名与别名认证时自动登记进换皮词表 R5）
cargo run -p adm4-cli -- template new-draft lane_defense tpl_demo --game "逆向目标游戏名" --alias "别名" --depth L4

# S1 本地语料检索（零网络；可换关键词多轮调用，候选池按来源去重累积）
cargo run -p adm4-cli -- template search-corpus lane_defense tpl_demo --corpus <语料目录> --question "战斗与部署结构" --keywords "克制,网格"

# S2 AI 映射：证据候选 → 逆向答卷（无证据整卷拒收 R1）
cargo run -p adm4-cli -- template map lane_defense tpl_demo

# S3 交叉核验：独立二次 AI 会话逐条对照，冲突降级待人工
cargo run -p adm4-cli -- template cross-check lane_defense tpl_demo

# S4 人工审核（署名与结论必填 R3）
cargo run -p adm4-cli -- template review lane_defense tpl_demo --reviewer "评审人" --note "审核结论"

# S5 认证入库（只有 Certified 模板可预填/对照）
cargo run -p adm4-cli -- template certify lane_defense tpl_demo

# 认证模板预填新项目 + 只读对照（模板不进项目）
cargo run -p adm4-cli -- project new "新项目" --pack lane_defense --depth L6 --template tpl_demo
cargo run -p adm4-cli -- template compare <archive_id> tpl_demo
```

预填条目需逐条 `authoring confirm`，并用 `authoring set-rationale` 改写理由完成换皮——预填理由含模板游戏名会被冻结换皮门拦截（属预期）。

### AI 访谈（分层逐条确认）

L 层升序推进、同层拓扑序、被拒点排同层末尾；L5/L6 整表为一个确认单元。

```powershell
# 生成下一个提案（stdout 单行回合 JSON，保存后 confirm 原样传回）
cargo run -p adm4-cli -- interview next <archive_id> > turn.json

# 确认提案（用户手势；--overrides-file 例外下钻，整表确认时改若干行/格）
cargo run -p adm4-cli -- interview confirm <archive_id> --proposal-file turn.json
cargo run -p adm4-cli -- interview confirm <archive_id> --proposal-file turn.json --overrides-file overrides.json

# 拒绝提案（该点排同层末尾，同层其余处理完后重提）
cargo run -p adm4-cli -- interview reject <archive_id> <决策点id> "拒绝理由"

# 查询分层进度（current_level 为 null 即全部完成）
cargo run -p adm4-cli -- interview progress <archive_id>
```

## 桌面应用（adm4-desktop）

Slint 桌面壳，围绕四大面板：**设计工作台**（L5/L6 结构化表格/矩阵编辑器）、**AI 访谈**（提案展示→确认/拒绝/例外下钻）、**冻结门**（五道门逐条 finding 明细）、**模板逆向**（产线五步状态 + 人工审核动作），另有项目、开发流水线、运行日志页签；GUI 无业务规则，一切校验与状态迁移在 `adm4-app` 服务层。
