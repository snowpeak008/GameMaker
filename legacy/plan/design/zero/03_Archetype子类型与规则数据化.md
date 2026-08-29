# 03 Archetype 子类型与规则数据化

## 目标

扩展现有 `core/design/requirements.py`，支持 `tower_defense` 和 `narrative_puzzle` 等更细粒度子类型，避免项目退化为 `generic_playable`。

---

## 修改范围

修改：

```text
core/design/requirements.py
```

新增：

```text
knowledge/design_data/archetypes/archetype_index.json
knowledge/design_data/archetypes/tower_defense.json
knowledge/design_data/archetypes/narrative_puzzle.json
core/tests/unit/test_design_requirements_archetype_detector.py
core/tests/unit/test_design_requirements_archetype_subtypes.py
```

---

## 设计要求

1. `tower_defense` 是 `strategy` 的可玩子类型。
2. `narrative_puzzle` 是 `narrative`/`management` 的可玩子类型。
3. 第一阶段扩展 `requirements.py` 现有 `ARCHETYPE_REQUIREMENTS` 和 `ARCHETYPE_KEYWORDS`。
4. JSON archetype 文件作为数据源引入，`requirements.py` 作为兼容门面读取，不维护两套真相。
5. `detect_archetype()` 增加 gameplay system 权重、关键词、模板元数据、profile 信号。

---

## Archetype 文件最低结构

```json
{
  "archetype_id": "tower_defense",
  "parent_archetypes": ["strategy"],
  "detection_rules": [
    {"signal": "gameplay_system", "key": "build_system", "min_weight": 0.15},
    {"signal": "keyword", "text": "格子", "min_count": 2}
  ],
  "required_systems": [],
  "required_entities": [],
  "required_player_actions": [],
  "required_resources": [],
  "required_objectives": [],
  "minimum_playable_assets": [],
  "style_compatibility": {},
  "open_questions": [],
  "program_task_templates": [],
  "acceptance_scenarios": []
}
```

---

## 验收标准

1. 植物大战僵尸类输入不再 fallback 到 `generic_playable`。
2. 请出示证件类输入不再 fallback 到 `generic_playable`。
3. `build_archetype_requirements()` 保留既有字段：`required_contracts`、`optional_contracts`、`archetype_p0_nodes`、`archetype_p1_nodes`。
4. 新增内容字段不会破坏 D3/D4 现有调用。
5. 测试覆盖父类型、子类型、fallback warning 和 keyword/gameplay system 组合信号。

