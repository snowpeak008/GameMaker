//! T-W7-5-0 标定回归：10 款审计游戏 + EU4 = 11 个样本点的组合校验回归。
//!
//! 双重职责：
//! 1. **预算标定锁定**——`knowledge/calibration/budget.json` 的五档建议值（占位试用态，
//!    待用户对 5-0 标定报告签字后转正）必须让 11 款样本落在标定报告推导的档带内；
//!    B(G) 数字逐款锁定，谁改评分/权重/预算值都会在此撞墙，撞了就该去更新标定报告。
//! 2. **R-C1′ 规则回归**——每款样本的连通性三查（(a) 连通 / (b) 强耦合 / (c) 参考线）
//!    结论如实断言，包括"提示产出但语义正确"的三处（星露谷/极乐迪斯科超直觉档参考线、
//!    旷野之息轮毂拓扑触发双连通守卫）——它们是标定报告的规则验证记录，不是缺陷。
//!
//! 分解依据全部在 `docs/memory/w7_wave5/5-0_标定报告.md` 逐款给出（公开机制事实）；
//! 尖塔/MOBA/EU4 三个锚点直接采用定稿 §6.2/§6.3 与附录 §1 的已验证分解。

use adm4_decision::FindingCode;
use adm4_decision::composition::{
    CompositionBudget, CompositionFinding, CompositionInput, CompositionReport, InterfaceEdge,
    InterfacePort, ProductGrade, SystemInstance, check_composition,
};
use adm4_decision::{CoreLink, FiveAxisRating};
use std::path::PathBuf;

// ---------------------------------------------------------------------------
// 夹具
// ---------------------------------------------------------------------------

fn rating(m: u8, d: u8, c: u8, p: u8, o: u8) -> FiveAxisRating {
    FiveAxisRating { m, d, c, p, o }
}

fn instance(id: &str, module: &str, rating: FiveAxisRating, link: CoreLink) -> SystemInstance {
    SystemInstance {
        instance_id: id.to_string(),
        module_id: module.to_string(),
        declared_tier: "declared".to_string(),
        rating,
        core_link: link,
        is_meta_only: false,
        interface_edges: Vec::new(),
        inductions: Vec::new(),
    }
}

fn edge(from: &str, port: InterfacePort, noun: &str, to: &str) -> InterfaceEdge {
    InterfaceEdge {
        from_instance: from.to_string(),
        port,
        noun: noun.to_string(),
        to_instance: to.to_string(),
    }
}

/// 真预算文件（knowledge/calibration/budget.json）——测试直接消费入库数值，
/// 数据文件与测试断言不一致即门禁红。
fn load_budget() -> CompositionBudget {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("knowledge")
        .join("calibration")
        .join("budget.json");
    let raw =
        std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{} 应可读：{e}", path.display()));
    serde_json::from_str(&raw)
        .unwrap_or_else(|e| panic!("{} 应反序列化为 CompositionBudget：{e}", path.display()))
}

fn input_of(instances: Vec<SystemInstance>, grade: ProductGrade) -> CompositionInput {
    CompositionInput {
        instances,
        core_loop_verbs: Vec::new(),
        product_grade: grade,
        budget: load_budget(),
        form_confirmed: false,
        pack_core_nouns: Vec::new(),
        module_tier_orders: Default::default(),
    }
}

fn codes(findings: &[CompositionFinding]) -> Vec<FindingCode> {
    findings.iter().map(|finding| finding.code).collect()
}

fn assert_budget(report: &CompositionReport, expected: f64, game: &str) {
    assert!(
        (report.budget_total - expected).abs() < 1e-9,
        "{game} 的 B(G) 应为 {expected}，实际 {}（评分或 κ 权重被改动？先更新 5-0 标定报告）",
        report.budget_total
    );
}

// ---------------------------------------------------------------------------
// 预算文件本体：五键齐全、严格递增（占位试用态的结构门禁）
// ---------------------------------------------------------------------------

#[test]
fn budget_file_has_five_strictly_increasing_grades() {
    let budget = load_budget();
    let grades = [
        ProductGrade::HyperCasual,
        ProductGrade::Casual,
        ProductGrade::MidCore,
        ProductGrade::HardCore,
        ProductGrade::Mmo,
    ];
    let mut previous = f64::NEG_INFINITY;
    for grade in grades {
        let value = *budget
            .grade_budgets
            .get(grade.key())
            .unwrap_or_else(|| panic!("budget.json 缺档 {}", grade.key()));
        assert!(
            value > previous,
            "预算值必须随档位严格递增：{} = {value} 不大于前档 {previous}",
            grade.key()
        );
        previous = value;
    }
    assert_eq!(
        budget.grade_budgets.len(),
        5,
        "budget.json 只应有五个档键（大战略档为上呈项，签字前不入表）"
    );
}

// ---------------------------------------------------------------------------
// 样本 1：幸存者（Vampire Survivors）——直觉档：独立/休闲
// ---------------------------------------------------------------------------

/// 分解依据：玩家只控移动、攻击全自动（战斗无战中决策 P0）；升级三选一 draft 与
/// 武器+饰品进化配对是唯一构筑深度（P3、进化表 O2）；波次按分钟表推进；经验宝石
/// 掉落被 draft 消费；金币局外解锁走 meta。
fn survivors() -> Vec<SystemInstance> {
    let mut combat = instance(
        "combat",
        "sys.realtime_combat",
        rating(1, 1, 2, 0, 1),
        CoreLink::Core,
    ); // W5
    combat.interface_edges = vec![edge(
        "combat",
        InterfacePort::Provides,
        "kill_event",
        "loot",
    )];
    let mut build = instance(
        "build",
        "sys.in_run_build",
        rating(2, 2, 2, 3, 2),
        CoreLink::Core,
    ); // W11
    build.interface_edges = vec![
        edge(
            "build",
            InterfacePort::Provides,
            "weapon_modifier",
            "combat",
        ),
        edge("build", InterfacePort::Modifies, "attack_rule", "combat"),
    ];
    let mut waves = instance(
        "waves",
        "sys.wave_encounter",
        rating(1, 2, 1, 0, 2),
        CoreLink::Core,
    ); // W6
    waves.interface_edges = vec![edge(
        "waves",
        InterfacePort::Provides,
        "enemy_spawn",
        "combat",
    )];
    let mut loot = instance("loot", "sys.loot", rating(1, 1, 2, 0, 1), CoreLink::Strong); // W5
    loot.interface_edges = vec![edge("loot", InterfacePort::Provides, "xp_gem", "build")];
    let meta = instance(
        "meta_unlock",
        "sys.meta_unlock",
        rating(1, 1, 1, 2, 1),
        CoreLink::Meta,
    ); // W6
    vec![combat, build, waves, loot, meta]
}

#[test]
fn survivors_single_heavy_core_fits_casual() {
    let report = check_composition(&input_of(survivors(), ProductGrade::Casual));
    assert_eq!(report.h_set, vec!["build"], "唯一重核 = 局内构筑（W11）");
    assert!(report.h_connected);
    assert!(report.blocks.is_empty(), "实际：{:?}", report.blocks);
    assert!(
        report.advices.is_empty(),
        "|H|=1 ≤ 休闲参考线 1 且 B=27.25 ≤ 休闲预算 28，应零提示：{:?}",
        report.advices
    );
    // B = core(5+11+6) + strong 5×0.75 + meta 6×0.25 = 22 + 3.75 + 1.5。
    assert_budget(&report, 27.25, "幸存者");
}

// ---------------------------------------------------------------------------
// 样本 2：星露谷——直觉档：独立；设计复杂度 = 重核（定稿 §8 问题 2 裁决确认）
// ---------------------------------------------------------------------------

/// 分解依据：时钟（日期/季节/节日日程状态机，tick 驱动栽培/NPC/商店，W9 采定稿
/// 综合者注 ① 口径）；栽培（生长状态机/季节约束/品质，每日种什么浇什么 P2）；
/// 制造（配方 >100 行，洒水器产物改写浇水规则 → modifies 核心）；商店/货币供给
/// 种子与购买力（核心循环"出售→买种"消费）；体力约束核心行动预算；NPC 好感、
/// 矿洞战斗均可绕开（不送礼/不下矿种田循环仍完整转动 → weak）；收集图鉴局外。
fn stardew() -> Vec<SystemInstance> {
    let mut clock = instance(
        "clock",
        "sys.world_clock",
        rating(2, 2, 3, 0, 2),
        CoreLink::Core,
    ); // W9
    clock.interface_edges = vec![edge("clock", InterfacePort::Provides, "tick", "farming")];
    let mut farming = instance(
        "farming",
        "sys.farming",
        rating(2, 2, 2, 2, 2),
        CoreLink::Core,
    ); // W10
    farming.interface_edges = vec![edge(
        "farming",
        InterfacePort::Provides,
        "harvest_event",
        "clock",
    )];
    let mut crafting = instance(
        "crafting",
        "sys.crafting",
        rating(1, 1, 2, 2, 2),
        CoreLink::Strong,
    ); // W8
    crafting.interface_edges = vec![edge(
        "crafting",
        InterfacePort::Modifies,
        "watering_rule",
        "farming",
    )];
    let mut shop = instance("shop", "sys.shop", rating(1, 1, 2, 1, 2), CoreLink::Strong); // W7
    shop.interface_edges = vec![edge(
        "shop",
        InterfacePort::Provides,
        "seed_entity",
        "farming",
    )];
    let mut currency = instance(
        "currency",
        "sys.currency",
        rating(1, 1, 2, 1, 1),
        CoreLink::Strong,
    ); // W6
    currency.interface_edges = vec![edge(
        "currency",
        InterfacePort::Provides,
        "purchase_power",
        "shop",
    )];
    let mut stamina = instance(
        "stamina",
        "sys.stamina_gate",
        rating(1, 1, 1, 1, 0),
        CoreLink::Strong,
    ); // W4
    stamina.interface_edges = vec![edge(
        "stamina",
        InterfacePort::Modifies,
        "action_budget",
        "farming",
    )];
    let npc = instance(
        "npc_relation",
        "sys.npc_relation",
        rating(1, 2, 1, 2, 2),
        CoreLink::Weak,
    ); // W8
    let mine = instance(
        "mine_combat",
        "sys.realtime_combat",
        rating(1, 1, 1, 0, 1),
        CoreLink::Weak,
    ); // W4
    let gallery = instance(
        "collection",
        "sys.collection_gallery",
        rating(1, 1, 1, 1, 2),
        CoreLink::Meta,
    ); // W6
    vec![
        clock, farming, crafting, shop, currency, stamina, npc, mine, gallery,
    ]
}

#[test]
fn stardew_design_complexity_is_hardcore_band_as_ruled() {
    // 按用户直觉档（独立 → casual）跑：|H|=2 超休闲档参考线 1 → 提示 + 形态确认；
    // B=45.25 超休闲预算 28 → 预算提示。两条提示都**语义正确**（§8 问题 2：
    // 星露谷判重核档确认为正确结论），是"直觉档 ≠ 设计复杂度档"的机器证据。
    let report = check_composition(&input_of(stardew(), ProductGrade::Casual));
    assert_eq!(report.h_set, vec!["clock", "farming"]);
    assert!(report.h_connected, "时钟↔栽培 tick/收获双边应连通");
    assert!(report.blocks.is_empty(), "实际：{:?}", report.blocks);
    assert_eq!(
        codes(&report.advices),
        vec![FindingCode::V3cCountAdvice, FindingCode::V5BudgetAdvice],
        "休闲档下应恰好两条提示（|H| 超线 + 预算超限），实际：{:?}",
        report.advices
    );
    assert!(report.form_confirmation_required);
    // B = core(9+10) + strong(8+7+6+4)×0.75 + weak(8+4)×0.5 + meta 6×0.25
    //   = 19 + 18.75 + 6 + 1.5。
    assert_budget(&report, 45.25, "星露谷");

    // 按设计复杂度档（重核）跑：全部提示消失——预算 55 容得下 45.25，参考线 3 ≥ 2。
    let hardcore = check_composition(&input_of(stardew(), ProductGrade::HardCore));
    assert!(hardcore.blocks.is_empty());
    assert!(
        hardcore.advices.is_empty(),
        "重核档下星露谷应零提示：{:?}",
        hardcore.advices
    );
}

// ---------------------------------------------------------------------------
// 样本 3：杀戮尖塔（锚点，定稿 §6.2 分解原样）——直觉档：中核偏上
// ---------------------------------------------------------------------------

fn spire() -> Vec<SystemInstance> {
    let mut deck = instance(
        "deck_building",
        "sys.in_run_build",
        rating(3, 3, 2, 3, 3),
        CoreLink::Core,
    ); // W14
    deck.interface_edges = vec![edge(
        "deck_building",
        InterfacePort::Provides,
        "card_modifier",
        "turn_combat",
    )];
    let mut combat = instance(
        "turn_combat",
        "sys.turn_combat",
        rating(2, 2, 2, 2, 2),
        CoreLink::Core,
    ); // W10
    combat.interface_edges = vec![edge(
        "turn_combat",
        InterfacePort::Provides,
        "energy",
        "deck_building",
    )];
    let mut relics = instance(
        "relics",
        "sys.rule_modifier",
        rating(2, 2, 2, 2, 2),
        CoreLink::Strong,
    ); // W10
    relics.interface_edges = vec![
        edge(
            "relics",
            InterfacePort::Modifies,
            "draw_rule",
            "deck_building",
        ),
        edge(
            "relics",
            InterfacePort::Modifies,
            "combat_attribute",
            "turn_combat",
        ),
    ];
    let map = instance(
        "map_route",
        "sys.map_route",
        rating(2, 2, 1, 1, 1),
        CoreLink::Strong,
    ); // W7
    let meta = instance(
        "meta_unlock",
        "sys.meta_unlock",
        rating(1, 1, 1, 1, 0),
        CoreLink::Meta,
    ); // W4
    vec![deck, combat, relics, map, meta]
}

#[test]
fn spire_anchor_fits_midcore_budget_with_count_advice_only() {
    let report = check_composition(&input_of(spire(), ProductGrade::MidCore));
    assert_eq!(report.h_set, vec!["deck_building", "relics", "turn_combat"]);
    assert!(report.h_connected);
    assert!(report.blocks.is_empty(), "实际：{:?}", report.blocks);
    // B=37.75 ≤ 中核预算 42：中核预算以尖塔锚 +10% 余量推导，不产预算提示；
    // |H|=3 > 中核参考线 2 的提示保持（定稿 §6.2 已知语义正确）。
    assert_eq!(codes(&report.advices), vec![FindingCode::V3cCountAdvice]);
    // B = 14 + 10 + 10×0.75 + 7×0.75 + 4×0.25。
    assert_budget(&report, 37.75, "杀戮尖塔");
}

// ---------------------------------------------------------------------------
// 样本 4：Candy Crush——直觉档：休闲
// ---------------------------------------------------------------------------

/// 分解依据：三消棋盘（交换/消除/下落/特糖生成与组合，特糖组合矩阵 5×5、糖果类型
/// 十余种 O1；每步选交换 = 会话级 P2）；关卡章节（目标绑定过关循环，数千关 O3）；
/// 生命限次供给开局许可（被核心消费 → strong）；道具商店供给局内修饰器；星级图鉴局外。
fn candy_crush() -> Vec<SystemInstance> {
    let mut board = instance(
        "board",
        "sys.match3_board",
        rating(2, 2, 2, 2, 1),
        CoreLink::Core,
    ); // W9
    board.interface_edges = vec![edge(
        "board",
        InterfacePort::Provides,
        "match_clear_signal",
        "levels",
    )];
    let mut levels = instance(
        "levels",
        "sys.stage_chapter",
        rating(1, 2, 2, 0, 3),
        CoreLink::Core,
    ); // W8
    levels.interface_edges = vec![edge(
        "levels",
        InterfacePort::Provides,
        "level_objective",
        "board",
    )];
    let mut lives = instance(
        "lives",
        "sys.energy_gate",
        rating(1, 1, 1, 1, 0),
        CoreLink::Strong,
    ); // W4
    lives.interface_edges = vec![edge(
        "lives",
        InterfacePort::Provides,
        "play_permit",
        "board",
    )];
    let mut boosters = instance(
        "boosters",
        "sys.shop",
        rating(1, 1, 2, 1, 1),
        CoreLink::Strong,
    ); // W6
    boosters.interface_edges = vec![edge(
        "boosters",
        InterfacePort::Provides,
        "booster_modifier",
        "board",
    )];
    let stars = instance(
        "stars",
        "sys.collection_gallery",
        rating(1, 1, 1, 1, 1),
        CoreLink::Meta,
    ); // W5
    vec![board, levels, lives, boosters, stars]
}

#[test]
fn candy_crush_fits_casual_cleanly() {
    let report = check_composition(&input_of(candy_crush(), ProductGrade::Casual));
    assert_eq!(report.h_set, vec!["board"]);
    assert!(report.blocks.is_empty(), "实际：{:?}", report.blocks);
    assert!(report.advices.is_empty(), "实际：{:?}", report.advices);
    // B = core(9+8) + strong(4+6)×0.75 + meta 5×0.25 = 17 + 7.5 + 1.25。
    assert_budget(&report, 25.75, "Candy Crush");
}

// ---------------------------------------------------------------------------
// 样本 5：斗地主——直觉档：休闲
// ---------------------------------------------------------------------------

/// 分解依据：牌型组合（约 13 种牌型模式 + 压制偏序 + 计分倍数，手牌拆分规划 P2；
/// 回合出牌时序按 T2 与牌型系统同体——摘除回合序则压制判定必须改写）；叫地主竞价
/// 绑定开局循环；欢乐豆赌注被局内倍数结算消费（strong）；段位纯局外。
fn doudizhu() -> Vec<SystemInstance> {
    let mut patterns = instance(
        "hand_patterns",
        "sys.card_pattern",
        rating(2, 2, 2, 2, 1),
        CoreLink::Core,
    ); // W9
    patterns.interface_edges = vec![edge(
        "hand_patterns",
        InterfacePort::Provides,
        "round_result",
        "bidding",
    )];
    let mut bidding = instance(
        "bidding",
        "sys.turn_bidding",
        rating(1, 1, 1, 2, 0),
        CoreLink::Core,
    ); // W5
    bidding.interface_edges = vec![edge(
        "bidding",
        InterfacePort::Provides,
        "landlord_role",
        "hand_patterns",
    )];
    let mut stakes = instance(
        "stakes",
        "sys.currency",
        rating(1, 1, 2, 1, 1),
        CoreLink::Strong,
    ); // W6
    stakes.interface_edges = vec![edge(
        "stakes",
        InterfacePort::Provides,
        "wager_pool",
        "hand_patterns",
    )];
    let ranking = instance(
        "ranking",
        "sys.ranked_ladder",
        rating(1, 1, 1, 0, 1),
        CoreLink::Meta,
    ); // W4
    vec![patterns, bidding, stakes, ranking]
}

#[test]
fn doudizhu_is_lightest_sample_and_fits_casual() {
    let report = check_composition(&input_of(doudizhu(), ProductGrade::Casual));
    assert_eq!(report.h_set, vec!["hand_patterns"]);
    assert!(report.blocks.is_empty(), "实际：{:?}", report.blocks);
    assert!(report.advices.is_empty(), "实际：{:?}", report.advices);
    // B = core(9+5) + strong 6×0.75 + meta 4×0.25 = 14 + 4.5 + 1.0。
    assert_budget(&report, 19.5, "斗地主");
}

// ---------------------------------------------------------------------------
// 样本 6：CS2——直觉档：重核竞技；设计复杂度 = 中核带（射程边界效应，见标定报告）
// ---------------------------------------------------------------------------

/// 分解依据：射击战斗（命中部位/穿透/移动精度/后坐力模式 ≥7 规则 M3；每武器后坐力
/// 序列表嵌套 D3；购买-对局内决策 P2）；回合经济（起枪局/eco 局、连败补偿、价格表，
/// W9 与战斗互锁双边）；炸弹目标绑定核心循环；段位与开箱皮肤纯局外。
/// 手感/netcode 射程外（定稿 §6.5）——设计复杂度只计射程内部分。
fn cs2() -> Vec<SystemInstance> {
    let mut gunplay = instance(
        "gunplay",
        "sys.realtime_combat",
        rating(3, 3, 2, 2, 2),
        CoreLink::Core,
    ); // W12
    gunplay.interface_edges = vec![
        edge(
            "gunplay",
            InterfacePort::Provides,
            "kill_reward",
            "round_economy",
        ),
        edge(
            "gunplay",
            InterfacePort::Provides,
            "kill_event",
            "objective",
        ),
    ];
    let mut economy = instance(
        "round_economy",
        "sys.round_economy",
        rating(2, 2, 2, 2, 1),
        CoreLink::Strong,
    ); // W9
    economy.interface_edges = vec![edge(
        "round_economy",
        InterfacePort::Provides,
        "buy_power",
        "gunplay",
    )];
    let mut objective = instance(
        "objective",
        "sys.objective",
        rating(2, 1, 2, 1, 1),
        CoreLink::Core,
    ); // W7
    objective.interface_edges = vec![edge(
        "objective",
        InterfacePort::Provides,
        "round_end",
        "round_economy",
    )];
    let ranking = instance(
        "ranking",
        "sys.ranked_ladder",
        rating(2, 2, 1, 0, 1),
        CoreLink::Meta,
    ); // W6
    let cases = instance(
        "cases",
        "sys.gacha_box",
        rating(1, 1, 1, 1, 2),
        CoreLink::Meta,
    ); // W6
    vec![gunplay, economy, objective, ranking, cases]
}

#[test]
fn cs2_in_scope_complexity_is_midcore_scale() {
    let report = check_composition(&input_of(cs2(), ProductGrade::HardCore));
    assert_eq!(report.h_set, vec!["gunplay", "round_economy"]);
    assert!(report.h_connected, "战斗↔经济 买枪/击杀奖励双边");
    assert!(report.blocks.is_empty(), "实际：{:?}", report.blocks);
    assert!(
        report.advices.is_empty(),
        "|H|=2 ≤ 重核参考线 3 且 B=28.75 ≤ 55：{:?}",
        report.advices
    );
    // B = core(12+7) + strong 9×0.75 + meta(6+6)×0.25 = 19 + 6.75 + 3.0。
    // 低于尖塔 37.75——重核竞技直觉反映的是品质/运营成本，射程内设计复杂度
    // 是中核量级（手感/netcode 出射程），预算刻度=设计复杂度语义下不算倒挂。
    assert_budget(&report, 28.75, "CS2");
}

// ---------------------------------------------------------------------------
// 样本 7：旷野之息——直觉档：大制作（重核）
// ---------------------------------------------------------------------------

/// 分解依据：战斗（武器类型/属性/耐久联动，接口边全口径 ≥4 → C3）；装备（武器
/// 耐久强制轮换 = E1-E2 级谱系，战斗消费武器 + 战斗改写耐久双边）；料理（材料×
/// 效果配方，供给增益并改写战斗属性双边）；化学元素单跳规则（火/电/冰 modifies
/// 战斗规则——多跳涌现射程外）；神庙供给试炼之证、成长改写核心属性；任务与商店
/// 可绕开（自由路线是该作定义性特征 → weak）。
fn botw() -> Vec<SystemInstance> {
    let mut combat = instance(
        "combat",
        "sys.realtime_combat",
        rating(2, 2, 3, 2, 2),
        CoreLink::Core,
    ); // W11
    combat.interface_edges = vec![edge(
        "combat",
        InterfacePort::Modifies,
        "weapon_durability",
        "gear",
    )];
    let mut gear = instance(
        "gear",
        "sys.equipment",
        rating(2, 2, 2, 2, 2),
        CoreLink::Strong,
    ); // W10
    gear.interface_edges = vec![edge(
        "gear",
        InterfacePort::Provides,
        "weapon_entity",
        "combat",
    )];
    let mut cooking = instance(
        "cooking",
        "sys.crafting",
        rating(1, 2, 2, 2, 2),
        CoreLink::Strong,
    ); // W9
    cooking.interface_edges = vec![
        edge("cooking", InterfacePort::Provides, "meal_buff", "combat"),
        edge(
            "cooking",
            InterfacePort::Modifies,
            "hearts_attribute",
            "combat",
        ),
    ];
    let mut chemistry = instance(
        "chemistry",
        "sys.status_effect",
        rating(2, 2, 3, 0, 1),
        CoreLink::Strong,
    ); // W8
    chemistry.interface_edges = vec![edge(
        "chemistry",
        InterfacePort::Modifies,
        "combat_rule",
        "combat",
    )];
    let mut shrines = instance(
        "shrines",
        "sys.stage_chapter",
        rating(1, 1, 1, 1, 2),
        CoreLink::Strong,
    ); // W6
    shrines.interface_edges = vec![edge(
        "shrines",
        InterfacePort::Provides,
        "spirit_orb",
        "progression",
    )];
    let mut progression = instance(
        "progression",
        "sys.char_level",
        rating(1, 1, 1, 1, 0),
        CoreLink::Strong,
    ); // W4
    progression.interface_edges = vec![edge(
        "progression",
        InterfacePort::Modifies,
        "max_hearts",
        "combat",
    )];
    let quests = instance(
        "quests",
        "sys.quest_board",
        rating(1, 2, 1, 1, 2),
        CoreLink::Weak,
    ); // W7
    let shops = instance("shops", "sys.shop", rating(1, 1, 1, 1, 1), CoreLink::Weak); // W5
    vec![
        combat,
        gear,
        cooking,
        chemistry,
        shrines,
        progression,
        quests,
        shops,
    ]
}

#[test]
fn botw_hub_topology_triggers_biconnectivity_advice_not_block() {
    let report = check_composition(&input_of(botw(), ProductGrade::HardCore));
    assert_eq!(report.h_set, vec!["combat", "cooking", "gear"]);
    assert!(report.h_connected);
    assert!(report.blocks.is_empty(), "实际：{:?}", report.blocks);
    // 规则回归发现（5-0 第二产出，如实记录）：H 为"战斗轮毂"拓扑——装备与料理
    // 各自只与战斗双边耦合，删战斗后两者分家 → 双连通守卫点名战斗为割点。
    // 提示级不拦；对 ARPG"单主核+多强辅"常见形态偏敏感，上报措辞复核、不改规则。
    assert_eq!(
        codes(&report.advices),
        vec![FindingCode::BiconnectivityAdvice],
        "|H|=3 ≤ 重核参考线 3、B=44.75 ≤ 55，应只有割点提示：{:?}",
        report.advices
    );
    assert_eq!(report.advices[0].subject, "combat");
    // B = core 11 + strong(10+9+8+6+4)×0.75 + weak(7+5)×0.5 = 11 + 27.75 + 6。
    assert_budget(&report, 44.75, "旷野之息");
}

// ---------------------------------------------------------------------------
// 样本 8：MOBA（锚点，定稿 §6.3 分解原样 + 中档件补全）——直觉档：重核竞技/MMO
// ---------------------------------------------------------------------------

fn moba() -> Vec<SystemInstance> {
    let mut skill = instance("skill", "sys.skill", rating(3, 3, 3, 3, 3), CoreLink::Core); // W15
    skill.interface_edges = vec![edge(
        "skill",
        InterfacePort::Modifies,
        "combat_attribute",
        "combat",
    )];
    let mut combat = instance(
        "combat",
        "sys.realtime_combat",
        rating(2, 2, 2, 2, 2),
        CoreLink::Core,
    ); // W10
    combat.interface_edges = vec![edge(
        "combat",
        InterfacePort::Provides,
        "kill_event",
        "objective",
    )];
    let mut objective = instance(
        "objective",
        "sys.objective",
        rating(2, 2, 2, 2, 1),
        CoreLink::Strong,
    ); // W9
    objective.interface_edges = vec![edge(
        "objective",
        InterfacePort::Provides,
        "tower_gold",
        "gear",
    )];
    let mut gear = instance(
        "gear",
        "sys.equipment",
        rating(2, 2, 2, 2, 1),
        CoreLink::Strong,
    ); // W9
    gear.interface_edges = vec![edge(
        "gear",
        InterfacePort::Provides,
        "stat_modifier",
        "skill",
    )];
    let mut economy = instance(
        "economy",
        "sys.round_economy",
        rating(2, 2, 2, 1, 1),
        CoreLink::Strong,
    ); // W8
    economy.interface_edges = vec![edge(
        "economy",
        InterfacePort::Provides,
        "gold_income",
        "gear",
    )];
    let mut minions = instance(
        "minions",
        "sys.wave_encounter",
        rating(1, 1, 1, 0, 2),
        CoreLink::Strong,
    ); // W5
    minions.interface_edges = vec![edge(
        "minions",
        InterfacePort::Provides,
        "lasthit_gold",
        "economy",
    )];
    let ranked = instance(
        "ranked",
        "sys.ranked_ladder",
        rating(2, 2, 2, 1, 1),
        CoreLink::Meta,
    ); // W8
    vec![skill, combat, objective, gear, economy, minions, ranked]
}

#[test]
fn moba_anchor_triggers_budget_advice_at_midcore_not_at_mmo() {
    // B = core(15+10) + strong(9+9+8+5)×0.75 + meta 8×0.25 = 25 + 23.25 + 2 = 50.25。
    // MMO 档（预算 68、参考线 4）：环形四重核零提示——锚点语义与定稿 §6.3 一致。
    let mmo = check_composition(&input_of(moba(), ProductGrade::Mmo));
    assert_eq!(mmo.h_set.len(), 4);
    assert!(mmo.h_connected);
    assert!(mmo.blocks.is_empty(), "实际：{:?}", mmo.blocks);
    assert!(mmo.advices.is_empty(), "实际：{:?}", mmo.advices);
    assert_budget(&mmo, 50.25, "MOBA");

    // 中核档：B=50.25 > 42 → 预算提示（附贡献降序表）；|H|=4 > 2 → 数量提示。
    let midcore = check_composition(&input_of(moba(), ProductGrade::MidCore));
    assert!(midcore.blocks.is_empty(), "预算是提示不是 block");
    assert_eq!(
        codes(&midcore.advices),
        vec![FindingCode::V3cCountAdvice, FindingCode::V5BudgetAdvice]
    );
    let v5 = midcore
        .advices
        .iter()
        .find(|finding| finding.code == FindingCode::V5BudgetAdvice)
        .expect("应有预算提示");
    assert_eq!(
        v5.related[0], "skill",
        "分值表按贡献降序，最贵的是技能（15.0）：{:?}",
        v5.related
    );
}

// ---------------------------------------------------------------------------
// 样本 9：音游（节奏光廊/OSU 型）——直觉档：休闲-中核
// ---------------------------------------------------------------------------

/// 分解依据：谱面时间轴（音符调度/判定窗分档/长按持续；谱面音符行数以千计 → O3，
/// 内容量诚实入 O 维——见标定报告 O 维口径观察）；计分连击在核心循环
/// "读谱→输入→判定→计分"内（G11 模块对位）；曲目列表供给谱面内容；解锁与
/// 表现分（pp）纯局外。延迟补偿算法射程外（定稿 §6.4）。
fn rhythm_game() -> Vec<SystemInstance> {
    let mut chart = instance(
        "chart",
        "sys.rhythm_chart",
        rating(2, 2, 2, 0, 3),
        CoreLink::Core,
    ); // W9
    chart.interface_edges = vec![edge(
        "chart",
        InterfacePort::Provides,
        "judgement_signal",
        "scoring",
    )];
    let mut scoring = instance(
        "scoring",
        "sys.scoring_combo",
        rating(2, 2, 2, 0, 1),
        CoreLink::Core,
    ); // W7
    scoring.interface_edges = vec![edge(
        "scoring",
        InterfacePort::Provides,
        "rating_grade",
        "unlocks",
    )];
    let mut songlist = instance(
        "songlist",
        "sys.stage_chapter",
        rating(1, 1, 1, 1, 2),
        CoreLink::Strong,
    ); // W6
    songlist.interface_edges = vec![edge(
        "songlist",
        InterfacePort::Provides,
        "chart_content",
        "chart",
    )];
    let unlocks = instance(
        "unlocks",
        "sys.meta_unlock",
        rating(1, 1, 1, 1, 1),
        CoreLink::Meta,
    ); // W5
    let pp = instance(
        "performance_rating",
        "sys.ranked_ladder",
        rating(1, 2, 1, 0, 1),
        CoreLink::Meta,
    ); // W5
    vec![chart, scoring, songlist, unlocks, pp]
}

#[test]
fn rhythm_game_fits_casual_upper_range() {
    let report = check_composition(&input_of(rhythm_game(), ProductGrade::Casual));
    assert_eq!(report.h_set, vec!["chart"]);
    assert!(report.blocks.is_empty(), "实际：{:?}", report.blocks);
    assert!(report.advices.is_empty(), "实际：{:?}", report.advices);
    // B = core(9+7) + strong 6×0.75 + meta(5+5)×0.25 = 16 + 4.5 + 2.5——
    // 休闲带上段，与"休闲-中核"直觉一致。
    assert_budget(&report, 23.0, "音游");
}

// ---------------------------------------------------------------------------
// 样本 10：极乐迪斯科——直觉档：中核叙事；设计复杂度 = 重核带（四重核互锁）
// ---------------------------------------------------------------------------

/// 分解依据：检定（2d6+技能对抗难度/白检可重试红检一次性/装备嗑药改修正，
/// 检定门控对话-任务-属性全口径边 C3）；对话叙事（巨型对话图回环 hub → D3，
/// 节点行数以万计 → O3——百万字文本本体射程外，容器行数如实入 O）；24 技能
/// （点数分配构筑 P3，modifies 检定修正 → strong）；思维阁（装备思想改写技能
/// 上限与检定规则 = 规则修饰器收集，构筑级 P3）；任务/金钱压力/昼夜时段为中轻件。
fn disco_elysium() -> Vec<SystemInstance> {
    let mut checks = instance(
        "checks",
        "sys.skill_check",
        rating(2, 2, 3, 2, 1),
        CoreLink::Core,
    ); // W10
    checks.interface_edges = vec![edge(
        "checks",
        InterfacePort::Provides,
        "check_result",
        "dialogue",
    )];
    let mut dialogue = instance(
        "dialogue",
        "sys.dialogue_graph",
        rating(2, 3, 2, 2, 3),
        CoreLink::Core,
    ); // W12
    dialogue.interface_edges = vec![edge(
        "dialogue",
        InterfacePort::Provides,
        "check_trigger",
        "checks",
    )];
    let mut skills = instance(
        "skills",
        "sys.char_level",
        rating(2, 2, 2, 3, 1),
        CoreLink::Strong,
    ); // W10
    skills.interface_edges = vec![
        edge(
            "skills",
            InterfacePort::Modifies,
            "check_modifier",
            "checks",
        ),
        edge(
            "skills",
            InterfacePort::Provides,
            "passive_interjection",
            "dialogue",
        ),
    ];
    let mut cabinet = instance(
        "thought_cabinet",
        "sys.rule_modifier",
        rating(2, 2, 2, 3, 1),
        CoreLink::Strong,
    ); // W10
    cabinet.interface_edges = vec![
        edge(
            "thought_cabinet",
            InterfacePort::Modifies,
            "skill_cap",
            "skills",
        ),
        edge(
            "thought_cabinet",
            InterfacePort::Modifies,
            "check_rule",
            "checks",
        ),
    ];
    let quests = instance(
        "quests",
        "sys.quest_board",
        rating(1, 2, 2, 1, 2),
        CoreLink::Strong,
    ); // W8
    let money = instance(
        "money",
        "sys.currency",
        rating(1, 1, 1, 1, 0),
        CoreLink::Strong,
    ); // W4
    let clock = instance(
        "day_clock",
        "sys.world_clock",
        rating(1, 1, 2, 0, 1),
        CoreLink::Strong,
    ); // W5
    vec![checks, dialogue, skills, cabinet, quests, money, clock]
}

#[test]
fn disco_elysium_four_interlocked_heavy_cores_are_hardcore_scale() {
    // 按直觉档（中核）跑：|H|=4 > 2 → 提示 + 形态确认；B=49.75 > 42 → 预算提示。
    // 与星露谷同型的"直觉档 ≠ 设计复杂度档"证据：检定×对话×技能×思维阁是
    // 真互锁四重核（无割点），设计负担重核级。
    let report = check_composition(&input_of(disco_elysium(), ProductGrade::MidCore));
    assert_eq!(
        report.h_set,
        vec!["checks", "dialogue", "skills", "thought_cabinet"]
    );
    assert!(report.h_connected);
    assert!(report.blocks.is_empty(), "实际：{:?}", report.blocks);
    assert_eq!(
        codes(&report.advices),
        vec![FindingCode::V3cCountAdvice, FindingCode::V5BudgetAdvice],
        "四重核互锁网无割点，不应有双连通提示：{:?}",
        report.advices
    );
    assert!(report.form_confirmation_required);
    // B = core(10+12) + strong(10+10+8+4+5)×0.75 = 22 + 27.75。
    assert_budget(&report, 49.75, "极乐迪斯科");

    // 重核档：预算 55 容下 49.75、参考线 3 < 4 仍有数量提示（|H|=4 确实超重核线）。
    let hardcore = check_composition(&input_of(disco_elysium(), ProductGrade::HardCore));
    assert!(hardcore.blocks.is_empty());
    assert_eq!(codes(&hardcore.advices), vec![FindingCode::V3cCountAdvice]);
}

// ---------------------------------------------------------------------------
// 样本 11：EU4（锚点，附录 §1 分解原样含中轻件）——大战略超线样本
// ---------------------------------------------------------------------------

fn eu4() -> Vec<SystemInstance> {
    let mut clock = instance(
        "clock",
        "sys.world_clock",
        rating(2, 2, 3, 0, 2),
        CoreLink::Core,
    ); // W9
    clock.interface_edges = [
        "diplomacy",
        "warfare",
        "territory",
        "trade",
        "currency",
        "tech",
        "religion",
    ]
    .iter()
    .map(|target| edge("clock", InterfacePort::Provides, "tick", target))
    .collect();
    let mut diplomacy = instance(
        "diplomacy",
        "sys.diplomacy",
        rating(3, 3, 3, 3, 2),
        CoreLink::Core,
    ); // W14
    diplomacy.interface_edges = vec![edge(
        "diplomacy",
        InterfacePort::Provides,
        "war_declaration",
        "warfare",
    )];
    let mut warfare = instance(
        "warfare",
        "sys.warfare",
        rating(3, 2, 3, 2, 1),
        CoreLink::Core,
    ); // W11
    warfare.interface_edges = vec![edge(
        "warfare",
        InterfacePort::Modifies,
        "province_control",
        "territory",
    )];
    let mut territory = instance(
        "territory",
        "sys.territory",
        rating(3, 3, 3, 2, 1),
        CoreLink::Core,
    ); // W12
    territory.interface_edges = vec![edge("territory", InterfacePort::Provides, "goods", "trade")];
    let mut trade = instance(
        "trade",
        "sys.trade_network",
        rating(2, 3, 2, 3, 1),
        CoreLink::Strong,
    ); // W11
    trade.interface_edges = vec![edge(
        "trade",
        InterfacePort::Provides,
        "trade_income",
        "currency",
    )];
    let mut currency = instance(
        "currency",
        "sys.currency",
        rating(2, 2, 3, 2, 1),
        CoreLink::Strong,
    ); // W10
    currency.interface_edges = vec![
        edge("currency", InterfacePort::Provides, "war_funds", "warfare"),
        edge(
            "currency",
            InterfacePort::Provides,
            "monarch_points",
            "tech",
        ),
    ];
    let mut tech = instance(
        "tech",
        "sys.tech_ideas",
        rating(2, 2, 2, 2, 1),
        CoreLink::Strong,
    ); // W9
    tech.interface_edges = vec![
        edge("tech", InterfacePort::Modifies, "combat_rule", "warfare"),
        edge(
            "tech",
            InterfacePort::Modifies,
            "development_rule",
            "territory",
        ),
        edge("tech", InterfacePort::Modifies, "trade_rule", "trade"),
    ];
    let mut religion = instance(
        "religion",
        "sys.religion",
        rating(2, 2, 2, 2, 1),
        CoreLink::Strong,
    ); // W9
    religion.interface_edges = vec![
        edge("religion", InterfacePort::Modifies, "unrest", "territory"),
        edge(
            "religion",
            InterfacePort::Modifies,
            "relations",
            "diplomacy",
        ),
    ];
    let missions = instance(
        "missions",
        "sys.quest_board",
        rating(1, 2, 1, 1, 1),
        CoreLink::Strong,
    ); // W6
    let rebels = instance(
        "rebels",
        "sys.wave_encounter",
        rating(1, 1, 2, 0, 1),
        CoreLink::Strong,
    ); // W5
    let achievements = instance(
        "achievements",
        "sys.collection_gallery",
        rating(1, 0, 0, 1, 0),
        CoreLink::Meta,
    ); // W2
    vec![
        clock,
        diplomacy,
        warfare,
        territory,
        trade,
        currency,
        tech,
        religion,
        missions,
        rebels,
        achievements,
    ]
}

#[test]
fn eu4_exceeds_even_mmo_budget_as_grand_strategy_evidence() {
    let mut input = input_of(eu4(), ProductGrade::Mmo);
    input.form_confirmed = true;
    let report = check_composition(&input);
    assert_eq!(report.h_set.len(), 8, "附录 §1.2：|H|=8");
    assert!(report.h_connected);
    assert!(report.blocks.is_empty(), "实际：{:?}", report.blocks);
    // B = core(9+14+11+12) + strong(11+10+9+9+6+5)×0.75 + meta 2×0.25
    //   = 46 + 37.5 + 0.5 = 84（附录 §1.5 原数）。
    assert_budget(&report, 84.0, "EU4");
    // 超线样本的机器证据：MMO 档预算 68 也装不下 84 → 预算提示如实产出
    // （提示制不拦，指向减重度档或未来的大战略档——附录修正案 1/3 为上呈项）。
    assert_eq!(
        codes(&report.advices),
        vec![FindingCode::V3cCountAdvice, FindingCode::V5BudgetAdvice],
        "|H|=8 > 4 数量提示 + 84 > 68 预算提示；真互锁网无割点：{:?}",
        report.advices
    );
    assert!(
        !report.form_confirmation_required,
        "已署名确认后不再要求确认"
    );
}

// ---------------------------------------------------------------------------
// 散点一致性：11 款 B(G) 与五档建议值的档带对应（标定报告 §3 散点表的机器版）
// ---------------------------------------------------------------------------

#[test]
fn scatter_of_eleven_samples_matches_grade_bands() {
    let budget = load_budget();
    let ceiling = |grade: ProductGrade| -> f64 { budget.grade_budgets[grade.key()] };
    let samples: [(&str, Vec<SystemInstance>, ProductGrade); 11] = [
        // （游戏, 分解, 标定报告推导的设计复杂度档——预算刻度=设计复杂度，§8 问题 2）
        ("斗地主", doudizhu(), ProductGrade::Casual),
        ("音游", rhythm_game(), ProductGrade::Casual),
        ("Candy Crush", candy_crush(), ProductGrade::Casual),
        ("幸存者", survivors(), ProductGrade::Casual),
        ("CS2", cs2(), ProductGrade::MidCore),
        ("杀戮尖塔", spire(), ProductGrade::MidCore),
        ("旷野之息", botw(), ProductGrade::HardCore),
        ("星露谷", stardew(), ProductGrade::HardCore),
        ("极乐迪斯科", disco_elysium(), ProductGrade::HardCore),
        ("MOBA", moba(), ProductGrade::HardCore),
        ("EU4", eu4(), ProductGrade::Mmo),
    ];
    let mut previous_b = f64::NEG_INFINITY;
    for (game, instances, grade) in samples {
        let report = check_composition(&input_of(instances, grade));
        // 散点排序：上表按 B(G) 升序排列，等权五维在 11 款上未产生排序倒挂
        // （若有倒挂此断言先红——权重调整是上呈项，本轮不改公式）。
        assert!(
            report.budget_total > previous_b,
            "{game} 的 B(G)={} 未超过前一样本 {previous_b}——散点排序出现倒挂",
            report.budget_total
        );
        previous_b = report.budget_total;
        // 除 EU4（大战略超线样本，档表外）外，各样本 B(G) ≤ 其设计复杂度档预算。
        if game != "EU4" {
            assert!(
                report.budget_total <= ceiling(grade),
                "{game} 的 B(G)={} 超出其档 {} 预算 {}",
                report.budget_total,
                grade.key(),
                ceiling(grade)
            );
        } else {
            assert!(
                report.budget_total > ceiling(ProductGrade::Mmo),
                "EU4 应超出 MMO 档预算（大战略档缺位的机器证据）"
            );
        }
    }
}
