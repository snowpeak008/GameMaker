#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""W6 T10 一次性迁移工具：二版 knowledge/design_data -> 四版 knowledge/design_space。

只读二版数据，写四版静态 JSON。本脚本不进入 cargo 构建，产物才是交付物。
用法（仓库根目录或任意目录均可）：

    python V4\\tools\\v2_migration\\migrate_v2_knowledge.py

写出：
  V4/knowledge/design_space/universal/domains.json        16 领域 + 104 节点（组织维度）
  V4/knowledge/design_space/universal/v2_checklist.json   2575 个决策点（二版检查单 x L4 选项组）
  V4/knowledge/design_space/universal/references/*.json   25 份二版内置模板（Certified）
  V4/knowledge/design_space/skin_wordlist.json            换皮词表（模板游戏名 + 别名）
  就地补 node_id：universal/core.json、lane_defense/pack.json、grid_strategy/pack.json
  就地补 nodes：lane_defense/pack.json、grid_strategy/pack.json（品类专属节点）
"""

from __future__ import annotations

import json
import os
import re
import sys
from collections import OrderedDict

SPACE_VERSION = "0.1.0"
MIGRATION_TAG = "W6-T10 二版知识库批量迁移"

HERE = os.path.dirname(os.path.abspath(__file__))
REPO = os.path.abspath(os.path.join(HERE, "..", "..", ".."))
V2 = os.path.join(REPO, "knowledge", "design_data")
SPACE = os.path.join(REPO, "V4", "knowledge", "design_space")
UNIVERSAL = os.path.join(SPACE, "universal")

# 既有通用层决策点 -> 二版节点（决策点 id 与语义不改，只补 node_id）。
LEGACY_POINT_NODES = {
    "u.business_model": "business_goal_decision",
    "u.platform": "platform_play_context_decision",
    "u.experience": "core_fun_decision",
    "u.genre": "market_category_positioning_decision",
}

# 品类包专属节点：按包内 `domain` 标签分组，挂到通用层领域。
PACK_NODES = {
    "lane_defense": {
        "combat": ("ld_combat", "通道塔防·战斗与克制", "gameplay_system_design", "system_concrete",
                   "通道塔防的战斗结算、克制关系与伤害规则。"),
        "deploy": ("ld_deploy", "通道塔防·部署与守卫", "gameplay_system_design", "system_concrete",
                   "守卫的部署方式、成本规则与守卫名单数值。"),
        "wave": ("ld_wave", "通道塔防·波次与关卡内容", "content_design", "content_concrete",
                 "敌人名单、关卡名单与各关波次编排数据。"),
        "economy": ("ld_economy", "通道塔防·资源经济", "economy_monetization_design", "system_concrete",
                    "资源产出与消耗结构、经济条目数值。"),
    },
    "grid_strategy": {
        "battlefield": ("grid_battlefield", "网格战棋·战场与移动", "gameplay_system_design",
                        "system_concrete", "网格战场结构与单位移动规则。"),
        "turn": ("grid_turn", "网格战棋·回合与行动点", "gameplay_system_design", "system_concrete",
                 "回合结构与行动点消耗规则。"),
        "combat": ("grid_combat", "网格战棋·战斗结算", "gameplay_system_design", "system_concrete",
                   "伤害公式、命中暴击判定、反击与兵种克制。"),
        "unit": ("grid_unit", "网格战棋·单位与兵种内容", "content_design", "content_concrete",
                 "单位名单、兵种表等内容实体。"),
        "progression": ("grid_progression", "网格战棋·养成与平衡", "balance_design", "system_concrete",
                        "经验获取、强化成本与养成数值曲线。"),
        "stage": ("grid_stage", "网格战棋·关卡战役内容", "content_design", "content_concrete",
                  "关卡名单、胜负条件与敌方配置数据。"),
        "terrain": ("grid_terrain", "网格战棋·地形交互", "gameplay_system_design", "system_concrete",
                    "地形效果规则与地形表。"),
    },
}

# 玩法系统选项库承载节点（二版 gameplay_system_options.json 的四版归宿）。
GAMEPLAY_SCOPE_NODE = "gameplay_system_scope_decision"
GAMEPLAY_SCOPE_POINT = "v2.gameplay_system_scope"

# 二版画像字段 -> 四版既有 L0 决策点的可无损映射（不可无损的进未迁移清单）。
PROFILE_ANSWER_MAP = {
    ("businessModel", "buyout"): ("u.business_model", "premium"),
    ("primaryPlatform", "mobile"): ("u.platform", "mobile"),
}


def read_json(path):
    with open(path, "r", encoding="utf-8") as handle:
        return json.load(handle, object_pairs_hook=OrderedDict)


def write_text(path, text):
    os.makedirs(os.path.dirname(path), exist_ok=True)
    with open(path, "w", encoding="utf-8", newline="\n") as handle:
        handle.write(text)


def compact(value):
    return json.dumps(value, ensure_ascii=False, separators=(",", ":"))


def write_rows(path, header_keys, rows_key, rows):
    """写「一行一条记录」的大 JSON：可读可 diff，又不被缩进撑爆体积。"""
    lines = ["{"]
    for key, value in header_keys.items():
        lines.append('  %s: %s,' % (compact(key), compact(value)))
    lines.append('  %s: [' % compact(rows_key))
    for index, row in enumerate(rows):
        tail = "," if index + 1 < len(rows) else ""
        lines.append("    " + compact(row) + tail)
    lines.append("  ]")
    lines.append("}")
    write_text(path, "\n".join(lines) + "\n")


# ---------------------------------------------------------------------------
# 1. 读二版：领域 / 节点 / 检查单 / 共享元模板的 L4 选项组
# ---------------------------------------------------------------------------

def load_v2():
    order = read_json(os.path.join(V2, "domain_order.json"))["domainOrder"]
    shared = {}
    templates_dir = os.path.join(V2, "templates")
    for name in sorted(os.listdir(templates_dir)):
        if name.endswith(".json"):
            data = read_json(os.path.join(templates_dir, name))
            shared[data["id"]] = data
    domains = []
    for domain_id in order:
        domains.append(read_json(os.path.join(V2, "domains", "%s.json" % domain_id)))
    return order, domains, shared


def groups_of(item, shared):
    """检查单项的 L4 选项组：内联优先，否则取 templateRef 指向的共享元模板。"""
    inline = item.get("optionGroups")
    if inline:
        return inline, None
    ref = item.get("templateRef")
    if not ref:
        raise SystemExit("检查单项 %s 既无 optionGroups 也无 templateRef" % item["id"])
    return shared[ref]["optionGroups"], ref


def topological_nodes(nodes):
    """按 requires 拓扑排序（二版文件序通常已合法，此处兜底并保持稳定）。"""
    remaining = list(nodes)
    emitted = []
    seen = set()
    while remaining:
        progressed = False
        for node in list(remaining):
            if all(dep in seen or dep not in {n["id"] for n in nodes} for dep in node.get("requires", [])):
                emitted.append(node)
                seen.add(node["id"])
                remaining.remove(node)
                progressed = True
        if not progressed:
            raise SystemExit("节点 requires 存在环：%s" % [n["id"] for n in remaining])
    return emitted


# ---------------------------------------------------------------------------
# 2. 组织维度 + 决策点
# ---------------------------------------------------------------------------

def level_of(role_class, is_first_group_of_node):
    if role_class == "content_concrete":
        return "L4"
    return "L3" if is_first_group_of_node else "L4"


def build_space(order, domains, shared, gameplay_options):
    out_domains = []
    out_nodes = []
    points = []
    stats = {
        "items": 0,
        "groups": 0,
        "options": 0,
        "levels": {},
        "entry_points": [],
        "option_relations_skipped": 0,
    }

    for index, domain_file in enumerate(domains):
        meta = domain_file["domain"]
        domain_id = meta["id"]
        out_domains.append(OrderedDict([
            ("id", domain_id),
            ("name", meta["name"]),
            ("description", meta.get("description", "")),
            ("order", index + 1),
        ]))

        nodes = topological_nodes(domain_file["nodes"])
        domain_points = []

        if domain_id == "gameplay_system_design":
            out_nodes.append(OrderedDict([
                ("id", GAMEPLAY_SCOPE_NODE),
                ("domain_id", domain_id),
                ("name", "玩法系统范围决策"),
                ("description", "先确认本项目包含哪些玩法系统，再逐系统细化（二版玩法系统选项库的归宿）。"),
                ("role_class", "meta_planning"),
            ]))
            domain_points.append(gameplay_scope_point(gameplay_options))

        for node in nodes:
            out_nodes.append(OrderedDict([
                ("id", node["id"]),
                ("domain_id", domain_id),
                ("name", node["name"]),
                ("description", node.get("description", "")),
                ("role_class", node.get("roleClass", "")),
            ]))
            first_group_seen = False
            for item in node.get("checklist", []):
                stats["items"] += 1
                stats["option_relations_skipped"] += len(item.get("optionRelations", []) or [])
                groups, ref = groups_of(item, shared)
                if ref is not None:
                    stats["option_relations_skipped"] += len(shared[ref].get("optionRelations", []) or [])
                for group in groups:
                    point = build_point(node, item, group, not first_group_seen)
                    first_group_seen = True
                    stats["groups"] += 1
                    stats["options"] += len(point["options"])
                    stats["levels"][point["level"]] = stats["levels"].get(point["level"], 0) + 1
                    domain_points.append(point)

        chain(domain_points)
        domain_points[0]["requirement"] = "baseline"
        stats["entry_points"].append(domain_points[0]["id"])
        points.extend(domain_points)

    return out_domains, out_nodes, points, stats


def build_point(node, item, group, is_first_group_of_node):
    point_id = "v2.%s.%s.%s" % (node["id"], item["id"], group["id"])
    allow_primary = bool(group.get("allowPrimary"))
    mode = "multi" if group.get("selectionMode") == "multi" else "single"
    selection_mode = OrderedDict([("mode", mode)])
    if mode == "multi":
        selection_mode["allow_primary"] = allow_primary

    level = level_of(node.get("roleClass", ""), is_first_group_of_node)
    options = []
    for option in group["options"]:
        entry = OrderedDict([
            ("id", option["id"]),
            ("label", option["label"]),
            ("summary", option.get("description", "")),
        ])
        # L4 在 C0 的默认 spec_role 是 mechanic，而二版检查单没有效果语义；
        # 按 R2「不发明效果」显式声明为 profile：答案落进 GameSpec 的设计意图档案。
        if level == "L4":
            entry["compiler_tags"] = OrderedDict([("spec_role", "profile")])
        options.append(entry)

    point = OrderedDict([
        ("id", point_id),
        # C0 编译期分组标签（不是 16 领域）：用二版节点 id，使同节点的 L3 系统与 L4 规则同组。
        ("domain", node["id"]),
        ("level", level),
        ("genre_scope", "universal"),
        ("node_id", node["id"]),
        ("question", "「%s」的%s怎么定？" % (item["label"], group["label"])),
    ])
    design_question = (group.get("designQuestion") or "").strip()
    if design_question:
        point["design_question"] = design_question
    mda = group.get("mdaLayer")
    if mda:
        point["mda_layer"] = mda
    point["selection_mode"] = selection_mode
    point["options"] = options
    return point


def gameplay_scope_point(gameplay_options):
    options = []
    for option in gameplay_options["options"]:
        options.append(OrderedDict([
            ("id", option["id"]),
            ("label", option["name"]),
            ("summary", option.get("mapping_desc", "")),
        ]))
    return OrderedDict([
        ("id", GAMEPLAY_SCOPE_POINT),
        ("domain", GAMEPLAY_SCOPE_NODE),
        ("level", "L3"),
        ("genre_scope", "universal"),
        ("node_id", GAMEPLAY_SCOPE_NODE),
        ("question", "本项目包含哪些玩法系统？"),
        ("design_question", "哪些系统是本作必须自己实现的，哪些只是借用外部结构？"),
        ("mda_layer", "mechanics"),
        ("selection_mode", OrderedDict([("mode", "multi"), ("allow_primary", True)])),
        ("options", options),
    ])


def chain(domain_points):
    """域内顺序链：前一个点的每个选项都 unlock 下一个点（选任何选项都能推进）。"""
    for current, following in zip(domain_points, domain_points[1:]):
        for option in current["options"]:
            option["unlocks"] = [following["id"]]


# ---------------------------------------------------------------------------
# 3. 二版内置模板 -> 四版认证模板
# ---------------------------------------------------------------------------

def build_templates(points_by_id, order, domains, shared):
    template_dir = os.path.join(V2, "project_templates")
    files = sorted(
        name for name in os.listdir(template_dir)
        if name.startswith("builtin_") and name.endswith(".json")
    )
    unmigrated = []
    skin_words = []
    templates = []
    for name in files:
        data = read_json(os.path.join(template_dir, name))
        meta = data["template"]
        state = data["projectState"]
        game_name = meta["gameName"]
        aliases = []
        match = re.search(r"[（(]([^）)]+)[）)]", meta.get("name", ""))
        if match and match.group(1) != game_name:
            aliases.append(match.group(1))
        skin_words.append(game_name)
        skin_words.extend(aliases)

        source_url = "adm4://v2-builtin/%s" % name
        answers = []
        levels = set()

        def evidence():
            return [OrderedDict([
                ("source_url", source_url),
                ("source_type", "inference"),
                ("confidence", "low"),
            ])]

        for node_id, node_state in state.get("nodes", {}).items():
            for item_id, groups in (node_state.get("checklistOptions") or {}).items():
                for group_id, selection in groups.items():
                    selected = selection.get("selected") or []
                    if not selected:
                        continue
                    decision_id = "v2.%s.%s.%s" % (node_id, item_id, group_id)
                    point = points_by_id.get(decision_id)
                    if point is None:
                        unmigrated.append((name, decision_id, "决策点不存在（二版清单已变更）"))
                        continue
                    option_ids = {option["id"] for option in point["options"]}
                    primary = selection.get("primary") or selected[0]
                    if primary not in option_ids:
                        unmigrated.append((name, decision_id, "选项 %s 不在迁移后的选项集" % primary))
                        continue
                    extras = [item for item in selected if item != primary and item in option_ids]
                    answer = OrderedDict([
                        ("decision_id", decision_id),
                        ("option_id", primary),
                        ("evidence", evidence()),
                    ])
                    if extras:
                        answer["notes"] = "二版多选：%s（主选 %s）；TemplateAnswer 暂无多选字段，仅主选可预填" % (
                            "、".join(selected), primary)
                        unmigrated.append((name, decision_id, "多选附加选项 %s 无 TemplateAnswer 承载" % "、".join(extras)))
                    answers.append(answer)
                    levels.add(point["level"])

        # 画像字段：只迁可无损映射的取值，其余进未迁移清单。
        for field, value in (state.get("profile") or {}).items():
            mapped = PROFILE_ANSWER_MAP.get((field, value))
            if mapped is None:
                unmigrated.append((name, "profile.%s=%s" % (field, value),
                                   "四版无对应决策点/选项（既有 u.* 选项集不含该取值，禁止改既有选项）"))
                continue
            decision_id, option_id = mapped
            answers.append(OrderedDict([
                ("decision_id", decision_id),
                ("option_id", option_id),
                ("evidence", evidence()),
                ("notes", "二版项目画像 %s=%s" % (field, value)),
            ]))
            levels.add("L0")

        # 玩法系统选择 -> v2.gameplay_system_scope（主选取权重最高者）。
        systems = state.get("gameplaySystems") or {}
        selected_systems = [item for item in (systems.get("selected") or [])]
        if selected_systems:
            weights = systems.get("weights") or {}
            scope_point = points_by_id[GAMEPLAY_SCOPE_POINT]
            known = {option["id"] for option in scope_point["options"]}
            valid = [item for item in selected_systems if item in known]
            missing = [item for item in selected_systems if item not in known]
            for item in missing:
                unmigrated.append((name, "gameplaySystems.%s" % item, "不在玩法系统选项库内"))
            if valid:
                def weight_of(system_id):
                    entry = weights.get(system_id)
                    if isinstance(entry, dict):
                        return entry.get("weight", 0)
                    return entry or 0

                primary = max(valid, key=lambda item: (weight_of(item), item))
                answer = OrderedDict([
                    ("decision_id", GAMEPLAY_SCOPE_POINT),
                    ("option_id", primary),
                    ("evidence", evidence()),
                    ("notes", "二版已选玩法系统：%s（按权重取主选 %s）" % ("、".join(valid), primary)),
                ])
                answers.append(answer)
                levels.add("L3")
                extras = [item for item in valid if item != primary]
                if extras:
                    unmigrated.append((name, GAMEPLAY_SCOPE_POINT,
                                       "多选附加系统 %s 无 TemplateAnswer 承载" % "、".join(extras)))
        for custom in (systems.get("custom") or []):
            unmigrated.append((name, "gameplaySystems.custom", "自定义系统 %s 无对应选项" % custom))

        # 节点文本：Template 结构只有 answers，节点级说明无承载字段。
        notes_count = sum(
            1 for node_state in state.get("nodes", {}).values()
            if (node_state.get("designNote") or "").strip()
        )
        risk_count = sum(
            1 for node_state in state.get("nodes", {}).values()
            if (node_state.get("riskNote") or "").strip()
        )
        if notes_count:
            unmigrated.append((name, "nodes[].designNote x%d" % notes_count,
                               "Template 无节点文本字段（项目态才有 node_design_notes）"))
        if risk_count:
            unmigrated.append((name, "nodes[].riskNote x%d" % risk_count,
                               "Template 无节点风险字段"))

        depth = max(levels) if levels else "L4"
        template = OrderedDict([
            ("template_id", meta["id"]),
            ("game_name", game_name),
            ("aliases", aliases),
            ("genre_pack", "universal"),
            ("pack_version", SPACE_VERSION),
            ("depth_reached", depth),
            ("certification", OrderedDict([
                ("status", "certified"),
                ("reviewed_by", MIGRATION_TAG),
                ("reviewed_at", "2026-08-29T00:00:00Z"),
                ("review_note",
                 "v2 内置模板批量迁移：来源 knowledge/design_data/project_templates/%s"
                 "（走批量导入通道，未跑逆向五步 AI 双会话，故 mapping_hash / crosscheck_proof 留空）" % name),
            ])),
            ("mapping_hash", ""),
            ("crosscheck_proof", None),
        ])
        templates.append((meta["id"], template, answers))
    return templates, unmigrated, skin_words


def write_templates(templates):
    out_dir = os.path.join(UNIVERSAL, "references")
    os.makedirs(out_dir, exist_ok=True)
    for template_id, template, answers in templates:
        header = OrderedDict()
        for key, value in template.items():
            header[key] = value
        write_rows(os.path.join(out_dir, "%s.json" % template_id), header, "answers", answers)


# ---------------------------------------------------------------------------
# 4. 就地补 node_id / pack nodes
# ---------------------------------------------------------------------------

def patch_core_node_ids():
    path = os.path.join(UNIVERSAL, "core.json")
    text = open(path, "r", encoding="utf-8").read()
    if '"node_id"' in text:
        return 0
    patched = 0
    for point_id, node_id in LEGACY_POINT_NODES.items():
        needle = '      "id": "%s",\n' % point_id
        if needle not in text:
            raise SystemExit("core.json 未找到决策点 %s 的 id 行" % point_id)
        text = text.replace(needle, needle + '      "node_id": "%s",\n' % node_id, 1)
        patched += 1
    write_text(path, text)
    return patched


def patch_pack(pack_id):
    path = os.path.join(SPACE, pack_id, "pack.json")
    text = open(path, "r", encoding="utf-8").read()
    if '"node_id"' in text:
        return 0, 0
    data = read_json(path)
    by_domain = PACK_NODES[pack_id]
    node_lines = []
    for domain_tag in sorted({point["domain"] for point in data["decision_points"]}):
        if domain_tag not in by_domain:
            raise SystemExit("%s 的 domain 标签 %s 未配置节点" % (pack_id, domain_tag))
        node_id, name, domain_id, role_class, description = by_domain[domain_tag]
        node_lines.append(OrderedDict([
            ("id", node_id),
            ("domain_id", domain_id),
            ("name", name),
            ("description", description),
            ("role_class", role_class),
        ]))

    point_nodes = {}
    for point in data["decision_points"]:
        point_nodes[point["id"]] = by_domain[point["domain"]][0]

    patched = 0
    for point_id, node_id in point_nodes.items():
        needle = '      "id": "%s",\n' % point_id
        if needle not in text:
            raise SystemExit("%s 未找到决策点 %s 的 id 行" % (pack_id, point_id))
        text = text.replace(needle, needle + '      "node_id": "%s",\n' % node_id, 1)
        patched += 1

    anchor = '  "decision_points": ['
    if anchor not in text:
        raise SystemExit("%s 未找到 decision_points 锚点" % pack_id)
    block = '  "nodes": [\n'
    block += ",\n".join("    " + compact(node) for node in node_lines)
    block += "\n  ],\n"
    text = text.replace(anchor, block + anchor, 1)
    write_text(path, text)
    return patched, len(node_lines)


# ---------------------------------------------------------------------------
# main
# ---------------------------------------------------------------------------

def main():
    order, domains, shared = load_v2()
    gameplay_options = read_json(os.path.join(V2, "gameplay_system_options.json"))

    out_domains, out_nodes, points, stats = build_space(order, domains, shared, gameplay_options)

    seen = set()
    for point in points:
        if point["id"] in seen:
            raise SystemExit("决策点 id 冲突：%s" % point["id"])
        seen.add(point["id"])
    node_ids = {node["id"] for node in out_nodes}
    for point in points:
        if point["node_id"] not in node_ids:
            raise SystemExit("决策点 %s 的 node_id 未声明" % point["id"])
    for point in points:
        for option in point["options"]:
            for target in option.get("unlocks", []):
                if target not in seen:
                    raise SystemExit("悬空 unlock：%s -> %s" % (point["id"], target))

    write_text(
        os.path.join(UNIVERSAL, "domains.json"),
        json.dumps(
            OrderedDict([
                ("space_version", SPACE_VERSION),
                ("domains", out_domains),
                ("nodes", out_nodes),
            ]),
            ensure_ascii=False,
            indent=2,
        ) + "\n",
    )
    write_rows(
        os.path.join(UNIVERSAL, "v2_checklist.json"),
        OrderedDict([("space_version", SPACE_VERSION)]),
        "decision_points",
        points,
    )

    points_by_id = {point["id"]: point for point in points}
    templates, unmigrated, skin_words = build_templates(points_by_id, order, domains, shared)
    write_templates(templates)

    words = sorted({word.strip() for word in skin_words if word.strip()})
    write_text(
        os.path.join(SPACE, "skin_wordlist.json"),
        json.dumps(OrderedDict([("words", words)]), ensure_ascii=False, indent=2) + "\n",
    )

    core_patched = patch_core_node_ids()
    pack_report = {pack_id: patch_pack(pack_id) for pack_id in sorted(PACK_NODES)}

    report = OrderedDict([
        ("domains", len(out_domains)),
        ("nodes", len(out_nodes)),
        ("checklist_items", stats["items"]),
        ("decision_points", len(points)),
        ("options", stats["options"]),
        ("levels", stats["levels"]),
        ("entry_points", stats["entry_points"]),
        ("templates", len(templates)),
        ("template_answers", sum(len(answers) for _, _, answers in templates)),
        ("skin_words", len(words)),
        ("core_points_patched", core_patched),
        ("pack_patched", pack_report),
        ("option_relations_skipped", stats["option_relations_skipped"]),
        ("unmigrated_entries", len(unmigrated)),
    ])
    print(json.dumps(report, ensure_ascii=False, indent=2))

    summary = OrderedDict()
    for _, _, reason in unmigrated:
        key = re.sub(r"[0-9]+", "N", reason)
        summary[key] = summary.get(key, 0) + 1
    print(json.dumps(OrderedDict([("unmigrated_by_reason", summary)]), ensure_ascii=False, indent=2))

    rules = collect_unmigrated_rules(order, domains, shared)
    print(json.dumps(OrderedDict([
        ("cross_layer_rules", len(rules["cross_layer_rules"])),
        ("option_relation_definitions", rules["option_relation_definitions"]),
        ("option_relation_instances", stats["option_relations_skipped"]),
    ]), ensure_ascii=False, indent=2))

    write_text(
        os.path.join(HERE, "unmigrated_report.json"),
        json.dumps(
            OrderedDict([
                ("generated_by", MIGRATION_TAG),
                ("answers_by_reason", summary),
                ("answer_entries", [
                    OrderedDict([("source", source), ("item", item), ("reason", reason)])
                    for source, item, reason in unmigrated
                ]),
                ("rules", rules),
            ]),
            ensure_ascii=False,
            indent=1,
        ) + "\n",
    )
    return 0


def collect_unmigrated_rules(order, domains, shared):
    """未迁移规则清单：跨层规则 + 选项软冲突关系（四版无对应承载）。"""
    cross = read_json(os.path.join(V2, "cross_layer_rules.json"))["rules"]
    cross_out = []
    for rule in cross:
        cross_out.append(OrderedDict([
            ("id", rule["id"]),
            ("severity", rule.get("severity", "")),
            ("if", rule.get("if", {})),
            ("forbids_option_id", rule.get("forbidsOptionId", [])),
            ("reason", rule.get("reason", "")),
            ("not_migrated_because",
             "四版的等价物是选项级 conflicts（硬冲突、需成对声明、必须引用具体决策点 id）；"
             "本规则的条件端是二版项目画像字段（profile.*），禁止端只给 option id 不给决策点，"
             "且 WARNING 级软冲突在四版无承载（ConsistencyRule 只有 answered_together / "
             "matrix_axis_matches_table_rows / row_reference 三种，且只能声明在品类包里，通用层无容器）"),
        ]))

    relations = []
    definitions = 0
    for name, template in shared.items():
        for relation in template.get("optionRelations", []) or []:
            definitions += 1
            relations.append(OrderedDict([
                ("source", "templates/%s" % name),
                ("id", relation["id"]),
                ("type", relation.get("type", "")),
                ("severity", relation.get("severity", "")),
            ]))
    for domain_file in domains:
        for node in domain_file["nodes"]:
            for item in node.get("checklist", []):
                for relation in item.get("optionRelations", []) or []:
                    definitions += 1
                    relations.append(OrderedDict([
                        ("source", "domains/%s#%s.%s" % (domain_file["domain"]["id"], node["id"], item["id"])),
                        ("id", relation["id"]),
                        ("type", relation.get("type", "")),
                        ("severity", relation.get("severity", "")),
                    ]))
    return OrderedDict([
        ("cross_layer_rules", cross_out),
        ("option_relation_definitions", definitions),
        ("option_relations_not_migrated_because",
         "全部是 soft_conflict/severity=warning 的软冲突提示；四版 DecisionOption.conflicts 是硬冲突"
         "（选中即被拦截、且要求双向对称声明），把警告级关系迁成硬冲突会改变语义并锁死合法组合，"
         "故整体不迁移，等四版补软冲突（提示级）承载后再迁"),
        ("option_relations", relations),
    ])


if __name__ == "__main__":
    sys.exit(main())
