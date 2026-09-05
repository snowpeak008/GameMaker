# T-W7-4c PromptLibrary 打捞对账单

**总账**：v2 通用层 2575 点（104 节点：103 个满编节点 × 25 点 + `gameplay_system_scope_decision` 1 点占位）
→ 提示词库全库 **211 条**（含 3f 种子 30 条），上限 300，余量 89。打捞率 211/2575 ≈ **8.2%**。

## 打捞方法（沿用 3f 报告方法论）

v2 的 25 点/节点 = 5 子项 × 5 维度冲压：同一节点下 5 个子项的 5 个维度问句**逐字相同**、
选项也相同（全库仅约 130 套选项模板）。原句几乎全是「『X』的 Y 怎么定？」的事实填空，
不含任何取舍。真取舍必须从「子项 × 维度」的**组合含义**重构：
如 `zhuang_bei_cheng_zhang × gameplay_rule_lever`（装备成长×规则杠杆）重构为
「词条上下限差距拉多大——惊喜 vs 方差毁平衡」。source_ref 标注语义锚点
（该问句的取舍语义落在哪个子项×维度交叉点上），不是逐字翻译。

**放弃大类**（下表「放弃理由」列的缩写）：
- **纯填空**：问句只要求归类/列举（「服务哪种核心感受？」「用什么机制？」），选项是分类学不是取舍。
- **重复冲压**：该节点的取舍语义已被同族其他节点更好的锚点覆盖（同维度族 5-8 个节点问句全同）。
- **皮肤层**：措辞/文案/素材口径类，取舍双方是「写法 A vs 写法 B」，不触设计代价。
- **流程管理**：文档、协作、验收流程类，是项目管理最佳实践不是设计取舍。
- **执行清单**：合规/上架/复核类，正确答案由法条或平台规则决定，无自由裁量的取舍。

## 104 节点逐行对账（103 满编节点 + 1 占位点）

| # | 节点 | 点数 | 打捞 | 放弃理由（其余点为何不要） |
|---|------|-----|-----|--------------------------|
| 1 | product_vision_decision | 25 | 2 | 核心承诺浓度、非目标纪律两条真金；其余为愿景归类纯填空 |
| 2 | target_player_decision | 24 | 2 | 受众冲突取舍、时间结构假设两条；画像枚举（年龄/设备/付费档）纯填空 |
| 3 | market_category_positioning_decision | 25 | 1 | 对标品抄改配比一条；卖点表述/口径一致类是皮肤层 |
| 4 | business_goal_decision | 25 | 2 | 商业模式锁定期、生命周期赌注两条；KPI 数字假设是填表不是取舍 |
| 5 | project_scope_decision | 25 | 1 | MVP 砍系统一条真金；范围清单枚举是流程管理 |
| 6 | platform_play_context_decision | 25 | 1 | 首发平台押注一条；设备场景枚举纯填空 |
| 7 | brand_ip_asset_decision | 25 | 1 | 品牌资产押角色还是玩法一条；命名/视觉资产登记是皮肤层 |
| 8 | core_fun_decision | 25 | 0 | 「主要乐趣来源选哪类」全节点是感受分类学，无代价结构，纯填空 |
| 9 | core_loop_decision | 25 | 0 | 循环动词归类（行动入口/反馈/再投入）纯填空；其取舍实质已被 meta_structure、session_rhythm 锚点覆盖 |
| 10 | player_behavior_decision | 25 | 0 | 行为频次归类纯填空；「禁止行为」的真取舍已由社区安全节点覆盖，重复冲压 |
| 11 | pressure_source_decision | 25 | 6 | 时间/资源/对抗/空间/风险五种压力各有真取舍，是通用层最厚的真金节点之一 |
| 12 | reward_experience_decision | 25 | 3 | 稀有奖励出货率、延迟奖励周期、成就绑定三条；奖励类型枚举纯填空 |
| 13 | learning_curve_decision | 25 | 5 | 首次理解可失败性、平台期介入、技巧窗口、策略深化槽位、精通外化五条 |
| 14 | session_rhythm_decision | 25 | 5 | 会话原子大小、进场摩擦、中断恢复、退出收尾均真取舍；再进入动机与留存节点重复 |
| 15 | gameplay_system_scope_decision | 1 | 0 | 单点占位（系统范围勾选），无内容 |
| 16 | input_control_decision | 25 | 4 | 响应规则前摇、容错缓冲、操作节奏、意图粒度四条；输入映射枚举纯填空 |
| 17 | action_rule_decision | 25 | 6 | 成本/收益/限制/条件/失败后果全是真取舍富矿，seed 已 1 条、本卡 5 条 |
| 18 | objective_system_decision | 25 | 2 | 隐藏目标提示度、可选目标奖励差两条；主/子目标枚举纯填空 |
| 19 | settlement_system_decision | 25 | 6 | 胜负平局、评分、奖惩结算、重试摩擦、掉线判定全真金（seed 2 + 本卡 4） |
| 20 | progression_system_decision | 25 | 7 | 升级节拍、垂直水平、装备成长、收藏钩子、账号角色账本——成长域最厚节点 |
| 21 | build_system_decision | 25 | 4 | 约束带宽、组合上限、洗点自由、套装绑定（seed 1 + 本卡 3）；组件枚举纯填空 |
| 22 | randomness_system_decision | 25 | 5 | 保底公开性、定向口子、揭晓演出、锁词条、鉴定时机全真金（seed 3 + 本卡 2） |
| 23 | meta_structure_decision | 25 | 4 | 局内外兑换率、局外准备窗口、共享仓库、地块扩张四条；局外枚举纯填空 |
| 24 | content_type_decision | 25 | 2 | 首小时浓度勾兑、职业专属内容产能两条；内容类型归类纯填空 |
| 25 | level_space_decision | 25 | 6 | 线性开放、节奏低谷、风险区定价、奖励区布局、网格自由、预览透明度 |
| 26 | quest_event_decision | 25 | 2 | 任务可失败性、分支预算两条；任务目标/条件/奖励枚举纯填空 |
| 27 | character_unit_decision | 25 | 5 | 职业定位软硬、机制差异深度、转职成本、AI 行为规则、编制上限五条 |
| 28 | item_resource_content_decision | 25 | 6 | 获取来源绑定、使用门槛、消耗规则、价值层级、道具定位（seed 3 + 本卡 3） |
| 29 | narrative_content_decision | 25 | 2 | 叙事剂量与跳过权、世界观投放方式两条；剧情结构/文本风格是皮肤层 |
| 30 | content_consumption_decision | 25 | 3 | 解锁闸门类型、重复价值引擎、见底预警三条；首次消耗路径枚举纯填空 |
| 31 | content_supply_structure_decision | 25 | 5 | 模板复用边界、淘汰回收、质量分层、PCG 配比、新内容操作强度（seed 2 + 本卡 3） |
| 32 | economy_loop_decision | 25 | 5 | 转换损耗、通胀回收、储备、交易税、拆除返还（seed 3 + 本卡 2） |
| 33 | currency_system_decision | 25 | 3 | 兑换开口、稀有货币节奏、付费币可肝性（seed 2 + 本卡 1）；货币枚举纯填空 |
| 34 | reward_distribution_decision | 25 | 1 | 投放形状（集中大奖 vs 细水长流）一条；奖励场合枚举与 reward_experience 重复冲压 |
| 35 | payment_point_decision | 25 | 2 | 通行证满级线、外观掺数值红线两条；付费点类型枚举纯填空 |
| 36 | product_structure_decision | 25 | 2 | 直购 vs 概率、新手包弹窗时机两条；商品类型枚举纯填空 |
| 37 | pricing_value_decision | 25 | 2 | 首充让利深度、锚点折扣战术两条；价格梯度填表纯填空 |
| 38 | payment_fairness_decision | 25 | 2 | 免费可达上限、未成年保护深度两条；公平边界宣言与 balance_payment 重复冲压 |
| 39 | economy_security_dispute_decision | 25 | 4 | 交易开放度、拍卖行结构、退款口子、稀缺真演四条 |
| 40 | ux_information_architecture_decision | 25 | 3 | 入口层级深度、异常兜底、信息优先级（战术板）三条；界面状态枚举纯填空 |
| 41 | ux_flow_decision | 25 | 2 | 确认摩擦道数、领奖动线两条；流程步骤枚举纯填空 |
| 42 | hud_feedback_decision | 25 | 5 | HUD 密度、危险预警透明度、死亡归因、成功反馈显眼度、指挥视角信息五条 |
| 43 | onboarding_guidance_decision | 25 | 3 | 强制度、教学时机、复习系统三条；引导触发枚举纯填空 |
| 44 | readability_decision | 25 | 2 | 数字精确 vs 模糊条、分数量级通胀两条；图标语义/文案清晰是皮肤层 |
| 45 | accessibility_decision | 25 | 1 | 无障碍预算与竞技边界一条；具体选项枚举是执行清单 |
| 46 | help_support_experience_decision | 25 | 2 | 求助入口位置、规则说明书深度两条；客服流程是流程管理 |
| 47 | art_direction_decision | 25 | 2 | 风格押注、特效华丽可读分界两条；色彩/造型语义是皮肤层 |
| 48 | animation_feel_decision | 25 | 0 | 动效感受归类纯填空；打击顿帧/取消/重量的真取舍全在 juice 节点，重复冲压 |
| 49 | audio_experience_decision | 25 | 1 | 听觉带宽分配一条；音效类型枚举纯填空 |
| 50 | juice_control_feel_decision | 25 | 3 | 顿帧重量、手感基调、后摇取消三条真金 |
| 51 | cinematic_presentation_decision | 25 | 2 | 演出夺权、失败演出重量两条；宣传可用演出是皮肤层 |
| 52 | balance_model_decision | 25 | 4 | 公式透明度、属性维度宽度、软上限拐点、装备操作配比四条 |
| 53 | balance_difficulty_decision | 25 | 6 | 开局立杆、中期剪刀差、后期墙型、峰值分位、失败恢复坡度、难度选项（seed 3 + 本卡 3） |
| 54 | balance_economy_decision | 25 | 5 | 产出热调杠杆、储备上限、消耗闸门、通胀账本（seed 3 + 本卡 2） |
| 55 | balance_content_decision | 25 | 2 | 敌人压强配方、时长注水两条；内容收益归类与 content 节点重复冲压 |
| 56 | balance_competition_decision | 25 | 4 | 角色轮换、匹配精度换等待、先后手补偿、滚雪球刹车四条 |
| 57 | balance_payment_decision | 25 | 2 | 付费深度封顶、效率差倍率两条；免费付费进度归类与 payment_fairness 重复 |
| 58 | social_relationship_decision | 25 | 3 | 赠礼通道、公会权重、师徒奖励三条；关系类型枚举纯填空 |
| 59 | social_collaboration_decision | 25 | 3 | 组队强制度、失败归因暴露、组队掉落分配三条；协作分工枚举纯填空 |
| 60 | social_competition_decision | 25 | 3 | 重置深度、排名可见域、赛制容错三条；竞争目标枚举纯填空 |
| 61 | social_expression_decision | 25 | 3 | 战绩曝光、嘲讽锋利度、装饰数值钩子三条；展示类型枚举纯填空 |
| 62 | community_behavior_decision | 25 | 3 | UGC 审发顺序、新老隔离、举报回执三条；正向激励枚举纯填空 |
| 63 | retention_onboarding_decision | 25 | 2 | 新手保护摘除坡道、新手资源肥度两条；首日/首周目标填表纯填空 |
| 64 | retention_daily_loop_decision | 25 | 1 | 日课剂量一条；每日/每周任务枚举与其重复冲压 |
| 65 | retention_mid_long_goal_decision | 25 | 2 | 锚点密度、终极目标形态两条；目标类型枚举纯填空 |
| 66 | retention_returning_player_decision | 25 | 1 | 追赶坡度一条真金；回流触发/引导枚举纯填空 |
| 67 | retention_loss_control_decision | 25 | 0 | 流失场景归类纯填空；各流失点的真取舍已散落在难度/社交/付费锚点，重复冲压 |
| 68 | liveops_launch_content_decision | 25 | 1 | 首发备货分仓一条；首发内容类型枚举纯填空 |
| 69 | liveops_activity_structure_decision | 25 | 1 | 限时错过成本（FOMO 旋钮）一条；活动主题/任务枚举纯填空 |
| 70 | liveops_version_rhythm_decision | 25 | 1 | 版本间隔鼓点一条；版本主题归类纯填空 |
| 71 | liveops_season_decision | 25 | 1 | 赛季资产重置深度一条；赛季目标/荣誉枚举纯填空 |
| 72 | liveops_update_communication_decision | 25 | 2 | 公告坦白度、事故补偿价码两条；预告节奏是流程管理 |
| 73 | data_goal_metric_decision | 25 | 1 | 北极星指标押注一条；指标清单填表纯填空 |
| 74 | data_path_metric_decision | 25 | 0 | 路径埋点清单是执行清单，无取舍 |
| 75 | data_test_design_decision | 25 | 0 | 测试类型枚举是流程管理；听嘴看手的真取舍归 feedback 节点 |
| 76 | data_feedback_collection_decision | 25 | 1 | 行为数据 vs 访谈证词的裁决权一条；问卷题型是执行清单 |
| 77 | data_iteration_decision | 25 | 1 | 再修一版还是砍的判死标准一条；保留/调整条件枚举是流程管理 |
| 78 | data_experiment_decision | 25 | 1 | 对照组伦理红线一条；灰度窗口/样本分层是执行清单 |
| 79 | compliance_age_rating_decision | 25 | 1 | 尺度对分级的表达代价一条；分级材料填报是执行清单 |
| 80 | compliance_player_protection_decision | 25 | 1 | 保底概率表达（软硬保底体感）一条；时长/消费提示是法条执行清单 |
| 81 | compliance_community_safety_decision | 25 | 2 | 聊天默认态、处罚梯子两条；违规定义/申诉流程是执行清单 |
| 82 | compliance_fairness_risk_decision | 25 | 0 | 公平风险归类纯填空；实质取舍已由 payment_fairness、balance_competition 锚点覆盖 |
| 83 | compliance_opinion_risk_decision | 25 | 0 | 舆情争议归类纯填空；补偿/沟通的真取舍已由 liveops 沟通节点覆盖 |
| 84 | compliance_localization_culture_decision | 25 | 0 | 本地化适配是执行清单（文化禁忌规避无自由裁量） |
| 85 | compliance_abuse_fairness_decision | 25 | 1 | 反作弊侵入深度一条；作弊定义/触发层级是执行清单 |
| 86 | compliance_privacy_data_decision | 25 | 1 | 个性化画像深度一条；授权/删除请求流程是法条执行清单 |
| 87 | documentation_core_doc_decision | 25 | 0 | 文档结构选择是流程管理，非玩家可感设计取舍 |
| 88 | documentation_table_schema_decision | 25 | 0 | 表结构选择是流程管理 |
| 89 | documentation_acceptance_decision | 25 | 0 | 验收标准是流程管理 |
| 90 | documentation_change_management_decision | 25 | 0 | 变更管理是流程管理 |
| 91 | documentation_cross_function_decision | 25 | 0 | 跨职能表达是流程管理 |
| 92 | release_store_entry_decision | 25 | 1 | 商店页美颜落差一条；分类/文案口径是皮肤层 |
| 93 | release_promotion_content_decision | 25 | 0 | 宣发素材顺序是皮肤层；实机落差的真取舍已由 store_entry 锚点覆盖 |
| 94 | release_preregistration_decision | 25 | 1 | 预约奖励透支一条；预约阶段目标是执行清单 |
| 95 | release_community_warmup_decision | 25 | 0 | 社群预热话题是运营执行清单 |
| 96 | release_regional_experience_decision | 25 | 0 | 区域适配是执行清单 |
| 97 | release_platform_material_decision | 25 | 0 | 上架材料是执行清单（平台规则决定，无裁量） |
| 98 | launch_version_decision | 25 | 1 | 带病上线闸口一条；首发完整度清单是执行清单 |
| 99 | launch_rhythm_decision | 25 | 1 | 开服容量赌注一条；开服阶段划分是流程管理 |
| 100 | launch_activity_decision | 25 | 0 | 开服活动类型枚举纯填空；活动取舍已由 liveops/econ 锚点覆盖 |
| 101 | launch_feedback_decision | 25 | 1 | 热修快稳边界一条；问题分级/公告口径是流程管理 |
| 102 | launch_player_support_decision | 25 | 0 | 客服流程是执行清单；求助入口的真取舍已由 help_support 锚点覆盖 |
| 103 | launch_post_launch_followup_decision | 25 | 1 | 停服退场姿势（远期账）一条；第一版本/活动承接是流程管理 |
| 104 | launch_readiness_review_decision | 25 | 0 | 准入复核是流程管理（勾选清单，无设计裁量） |

## 收敛过程审计摘要

- **打捞密度与素材真金成正比，不硬凑均匀**：玩法系统域（gameplay_system_design 相关 8 节点）打捞 38 条最厚，
  流程管理类（documentation 5 节点 + launch_readiness）打捞 0 条——它们 150 点全是项目管理清单。
- **打捞为 0 的 22 个节点**分四类：纯填空分类学（core_fun/core_loop/player_behavior/animation_feel 等 6 节点）、
  重复冲压被更好锚点覆盖（retention_loss_control/compliance_fairness_risk 等 5 节点）、
  流程管理（documentation 全族 5 节点 + 3 launch 节点）、执行清单（localization/platform_material 等 3 节点）。
- **总量 211 < 300**：素材里真取舍就这么多。继续往上凑只能同题微调或把填空句伪装成取舍句，
  两者都被测试断言（前 12 字符去重、可溯源）挡住，也被"宁缺勿滥"红线禁止。
