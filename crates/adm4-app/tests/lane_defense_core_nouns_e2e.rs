//! T-W7-6pre 缺口 B：lane_defense 补 `core_nouns` 的 app 级验收。
//!
//! 背景（5a_真AI访谈记录 §2 场景 2/3）：ld 是六包中唯一没有 `core_nouns` 键的包，
//! `sys.turn_combat` 这类模块 consumes `sys.player_input.command_intent`（玩家输入 =
//! 平台事实），pack 不声明核心名词 → 概念访谈提案的输入类名词绑定必悬空、V6 必拒。
//! 本测试锁定：
//! - 真实 lane_defense pack 装配后携带核心名词 `player_command_intent`；
//! - scripted 复现记录场景 2：turn_combat 实例显式绑定
//!   `sys.player_input.command_intent → player_command_intent` → 提案通过、
//!   确认落盘（装配层 V6 同口径放行）——真 AI 版结果人读记录进 5a 追加节。

use adm4_ai::ScriptedProvider;
use adm4_app::{AppConfig, AppServices, save_config};
use adm4_archive::DataRoot;
use adm4_decision::DesignLevel;
use std::path::{Path, PathBuf};

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
}

fn services_at(temp: &Path) -> AppServices {
    std::fs::remove_dir_all(temp).ok();
    let data_root = DataRoot::new(temp).unwrap();
    save_config(
        &data_root,
        &AppConfig {
            design_space_root: repo_root()
                .join("knowledge")
                .join("design_space")
                .to_string_lossy()
                .into_owned(),
            system_modules_root: repo_root()
                .join("knowledge")
                .join("systems")
                .to_string_lossy()
                .into_owned(),
            ai_provider: None,
            image_provider: None,
            engine_backend: None,
        },
    )
    .unwrap();
    AppServices::open(Some(temp.to_path_buf())).unwrap()
}

/// 真实 lane_defense pack 装配后携带核心名词（缺口 B 数据层落地的直接断言）。
#[test]
fn lane_defense_pack_declares_player_command_intent_core_noun() {
    let root = adm4_space::DesignSpaceRoot::new(repo_root().join("knowledge").join("design_space"));
    let space = adm4_space::load_design_space(&root, "lane_defense").unwrap();
    assert_eq!(
        space.pack.core_nouns,
        vec!["player_command_intent".to_string()],
        "ld 应与 spire_like 同款声明玩家输入核心名词（宁少勿滥：只加平台事实名词）"
    );
}

/// 记录场景 2 的 scripted 复现：ld 项目里提案 turn_combat 实例，
/// 输入名词显式绑定核心名词 → 提案通过 + 确认落盘（修复前该场景必拒）。
#[test]
fn ld_concept_proposal_binds_player_input_to_core_noun() {
    let temp = std::env::temp_dir().join(format!("adm4_ld_core_nouns_{}", std::process::id()));
    let services = services_at(&temp);
    let archive_id = services
        .project_new("ld 输入名词复验", "lane_defense", DesignLevel::L6, None)
        .unwrap();

    let provider = ScriptedProvider::new();
    provider.script(
        "interview_concept",
        vec![
            r#"{
              "systems": [
                { "instance_id": "turn_combat", "module_id": "sys.turn_combat",
                  "suggested_tier": "tc0_turn_sequence", "core_link": "core",
                  "rationale": "爬塔卡牌肉鸽的回合战斗骨架",
                  "noun_bindings": { "sys.player_input.command_intent": "player_command_intent" } }
              ],
              "core_loop": [ { "verb": "出牌", "instance_id": "turn_combat" } ],
              "notes": "记录场景 2 复现：输入名词落在 pack 核心名词上"
            }"#
            .into(),
        ],
    );
    let proposal = services
        .interview_concept_with(&archive_id, &provider, "爬塔卡牌肉鸽")
        .unwrap();
    assert_eq!(proposal.systems.len(), 1);
    assert_eq!(
        proposal.systems[0].noun_bindings["sys.player_input.command_intent"],
        "player_command_intent",
        "输入名词应绑到 pack 核心名词"
    );
    // 提示词层可断言：核心名词与已占用清单都进了上下文（缺口 A+B 的接缝可见性）。
    let calls = provider.calls();
    assert!(
        calls[0].user_prompt.contains("player_command_intent"),
        "pack 核心名词应进提示词：{}",
        calls[0].user_prompt
    );
    assert!(
        calls[0].user_prompt.contains("已占用实例 id 清单"),
        "已占用清单段应存在：{}",
        calls[0].user_prompt
    );

    // 确认落盘：装配层 V6 同口径放行（绑定目标 = pack 核心名词）。
    let report = services
        .interview_concept_confirm(&archive_id, &proposal)
        .unwrap();
    assert_eq!(report.instances, vec!["turn_combat"]);
    let state = services.load_authoring_state(&archive_id).unwrap();
    let tier = &state.selections["turn_combat.tier"];
    assert_eq!(tier.option_id, "tc0_turn_sequence");
    assert!(tier.confirmed_by_user);

    std::fs::remove_dir_all(&temp).ok();
}
