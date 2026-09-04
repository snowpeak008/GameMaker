# T-W7-3e 断点申报：桌面端 W7 适配（系统模块/档位/组合提示/署名确认流）

## 状态申报
- 2026-09-05 05:10 开工。领取任务，开始调研（必读：desktop 现状、3a/3b 契约尾部、W7 定稿 §4.2(c)、services.rs 签名）。
- 基线核验：进行中（cargo build -p adm4-desktop + cargo test --workspace）。
- 2026-09-05 05:25 基线确认：cargo build -p adm4-desktop 成功；cargo test --workspace 全绿（各套件加和 = 559，与任务卡一致）。调研完毕，设计定案（接力者以此为准）：
  1. **「系统组合」落位 = design 视图的全窗覆盖面板**（archive-panel/template-panel 同款先例，`system-panel-open`），顶部工具条加「系统组合」按钮。右栏 348px 放不下 ①+② 两块内容，页签再加一枚会挤爆五钮行。
  2. **档位选择 = 跳转既有决策点通道**：实例行点击 → 关面板 + `select-decision("<instance>.tier")`——tier 合成点是普通 L3 单选点，中栏既有选项渲染/确认按钮全部复用，零新机制。
  3. **模块档位数据（W 分/档带）来源**：services 不暴露模块表（engine.system_modules 私有），crates 禁改。裁决：desktop 直接按 `services.system_modules_root()` + `space.system_instances[].module_id` 读 `<root>/<module_id>/module.json`（adm4_foundation::read_json_file + adm4_decision::SystemModule，两者均为既有依赖，零新增依赖）；私有模块数据走 `system_module_list`（自带完整 SystemModule）。读不到时降级显示"（模块数据不可读）"不拦渲染——权威校验在装配层已跑过，这里纯展示。
  4. **κ 来源**：pack 实例取 `load_space_shared(pack).pack.system_refs[].core_link`；私有实例取 `ProjectSystemModule.instance.core_link`。
  5. **组合提示渲染**：`composition_report` → TextRow 复用（kind=bad 红/新增 warn 黄/info），missing_tiers 行 target=`<instance>.tier` 可点跳转；确认卡按 `form_confirmation_required` 显示（h_set+各重核 W+B(G)+免责句+署名框+按钮→`compose_confirm_form`）；已确认显示 signer/at 留痕；`confirmation_stale` 显示"组合已变化，须重新确认"。确认仅按钮手势触发，无自动签。
  6. **③ 真机核验为主**：tier 合成点已是普通决策点（3a），中栏检查单/完成度理论零改动；gate2 组合 finding 走既有 freeze_check 渲染面（generic finding 行），核验列入走查清单。
- 下一步：view.rs 纯函数（行构造）→ main.slint（面板+属性+回调）→ main.rs（接线+refresh_system）→ 门禁。

## 里程碑
- 2026-09-05 06:05 【里程碑 1】三块代码落盘，desktop 构建/clippy/fmt 全干净：
  - view.rs：`SystemInstanceFacts`（实例展示原料）+ 6 个纯装配函数（`system_instance_facts`/`system_instance_rows`/`system_module_rows`/`system_report_rows`/`system_confirm_text`/`system_confirmed_text`/`system_panel_summary`）+ 7 个单元测试（55 全绿）。行模型复用 TextRow；advices 用新 kind=warn（黄）。
  - main.slint：顶部工具条加「系统组合」按钮（三面板互斥）；全窗覆盖面板（左 46% 实例清单+私有模块只读清单 / 右 组合校验报告+确认卡+留痕卡）；新属性 7 个 + 回调 2 个（system-refresh / system-confirm-form）。档位跳转零新回调：行点击关面板走既有 select-decision。
  - main.rs：`hook_system_callbacks`（刷新 + 署名确认转发，拒绝原因原样回显）+ `refresh_system`/`load_system_data`/`clear_system`；open_project 时关面板清数据。模块标定读取按设计决定 3（system_modules_root 逐模块按需读，坏文件降级「模块数据不可读」）。
- 2026-09-05 06:40 【里程碑 2】真机自查通过（沙箱 .tmp_3e_sandbox，已清理）：临时数据根 + 双装备实例包（composition_gate_e2e.rs 的 stapled_pack 同款）+ knowledge/systems 四模块真实副本。CLI 建项后：① compose report 初始 = 2 条 tier_unselected；② select+confirm 双 e3_socket 后 = v3a/v3b/v1×8 硬违例 + v3c 提示 + CONFIRM-REQUIRED（tier 合成点走的就是 desktop 同一条 select/confirm 通道，authoring status 里 equip_alpha.* 模块点全部在清单）；③ compose confirm-form 署名后 = [CONFIRMED] 走查员甲 留痕、不再 REQUIRED。desktop.exe 以沙箱数据根冒烟启动 6 秒无崩溃退出。UI 交互级核验（点击/渲染）列入真机走查清单，交主开发/用户。
- 2026-09-05 06:45 【里程碑 3·完工】全门禁通过：cargo fmt --check 无 diff；cargo clippy -p adm4-desktop --all-targets 零警告；cargo build -p adm4-desktop 成功；cargo test --workspace 全绿（总 589 = 基线 559 + 本卡 view 单测 8 + 并行卡 3d 期间落盘的新增测试，0 failed 0 ignored；本卡零回退零弱化）。改动面：仅 apps/adm4-desktop/{src/view.rs, src/main.rs, ui/main.slint} + 本文件，crates/knowledge/docs/plan 零接触。T-W7-3e 完成，待主开发验收。
- 【遗留申报】① 私有模块实例的档位显示依赖 system_module_list 记录自带的 SystemModule（无需读库），库内实例读 system_modules_root/<module_id>/module.json——若运行期换库目录（进程期不变量被绕过）展示层会滞后一拍，刷新即恢复；权威数据始终在后端。② 面板不提供私有模块登记表单（system_module_add 需要完整 SystemModule JSON 草案，桌面表单形态归后续波裁决）；只读清单已到位。③ advices 用了新 kind="warn"（黄）——TextRow 行模型自带 kind 字段，其它页签不受影响。
