# Archetype 强推断优化（Step00/01）

## 问题描述

当前两个项目的 `genre_profile.genre_id` 均为 `generic_playable`，丢失了游戏类型的精确信息，导致后续所有步骤都使用通用模板。

**实际期望：**
- 植物大战僵尸 → `tower_defense` 或 `casual_tower_defense`
- 请出示证件 → `narrative_puzzle` 或 `management_simulation`

## 推断机制设计

### 第一层：Gameplay System 决策推断

从用户在设计工作台中勾选的 gameplay system 权重直接推断 Archetype：

```python
ARCHETYPE_RULES = [
    # 规则格式：(必要系统集合, 禁止系统集合, archetype_id, 置信度)
    ({"build_system", "action_rule", "objective"},   {"social_competition"}, "tower_defense",         0.9),
    ({"input_control", "objective", "settlement"},   {"build_system"},       "narrative_puzzle",      0.85),
    ({"randomness_system", "action_rule"},            {},                     "roguelike_action",      0.85),
    ({"progression_system", "resource_economy"},     {},                     "idle_management",       0.8),
    ({"social_competition", "progression_system"},   {},                     "social_casual",         0.8),
    ({"meta_structure", "randomness_system"},        {},                     "roguelike_deckbuilder", 0.85),
]
```

### 第二层：Profile 信号加权

结合项目 profile 进行修正：

| 商业模式 | 平台 | 规模 | 加权调整 |
|---------|------|------|---------|
| free_to_play | mobile | hypercasual | 倾向 casual_ 前缀 |
| buyout | pc | indie | 倾向 narrative_ / simulation |
| free_to_play | pc | mid_core | 倾向 action_ / strategy |

### 第三层：关键词信号

从设计文档的 design_extraction.json 中提取关键词：

```python
KEYWORD_SIGNALS = {
    "tower_defense":    ["格子", "防线", "波次", "tower", "defense", "lane", "plant", "zombie"],
    "narrative_puzzle": ["证件", "审查", "剧情", "选择", "对话", "document", "verify", "story"],
    "roguelike":        ["随机", "重开", "构筑", "roguelike", "permadeath", "run"],
    "management":       ["建造", "经营", "自动化", "工厂", "城市", "manage", "build", "factory"],
}
```

### 推断结果结构

`archetype_requirements.json` 必须包含：

```json
{
  "detected_archetype": "tower_defense",
  "detection_confidence": "high",
  "detection_method": ["gameplay_system_weights", "keyword_signals"],
  "detection_source": [
    "build_system decision (weight=0.25)",
    "action_rule decision (weight=0.20)",
    "keyword: 格子/防线/波次 (count=8)"
  ],
  "archetype_p0_nodes": [
    "build_system_decision",
    "input_control_decision",
    "objective_system_decision",
    "action_rule_decision"
  ],
  "archetype_exclusive_assets": [
    "grid_map_tile",
    "plant_slot_icon",
    "sun_currency_icon",
    "zombie_unit_sprite",
    "wave_progress_bar"
  ],
  "archetype_exclusive_systems": [
    "GridPlacementSystem",
    "SunEconomySystem",
    "WaveSpawnManager",
    "PlantDefenseCalculator"
  ],
  "warnings": []
}
```

## 实现位置

```
core/design/archetype_detector.py       ← 新增
core/design/archetype_rules.json        ← 新增（规则表）
pipeline/step_00_idea_intake/plugin.py  ← 读取决策，调用 archetype_detector
pipeline/step_01_gameplay_framework/    ← 消费 archetype_requirements 生成类型框架
```

## 验收标准

- 植物大战僵尸的 `detected_archetype` ≠ `generic_playable`
- detection_confidence 为 `high` 或 `medium`，不允许 `fallback` 时直接使用
- `archetype_exclusive_assets` 和 `archetype_exclusive_systems` 非空
- fallback 为 `generic_playable` 时必须产生 WARNING，并在 UI 工作台提示用户补充决策
