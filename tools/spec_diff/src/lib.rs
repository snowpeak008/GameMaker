//! GameSpec 语义 diff（T-W7-4-0，波 4b 迁移等价自证工具）。
//!
//! 口径（对齐 W7 执行计划 §5 与 golden_diff.ps1 先例）：
//! - 旧侧 GameSpec 先按 id 映射表整树换算（精确映射整串匹配；前缀规则锚定串首，
//!   尾部原样保留——覆盖 `ld.tower_cost * 2` 这类公式引用；`section/id` 形态的
//!   SpecRef 对尾段 id 换算，覆盖 source_map 与 acceptance 的锚定路径）；
//! - 「语义零 diff」= 映射后除 id 本身外全部字段逐字节相等（design_notes 内嵌的
//!   source_decision / source_option 里的 id 同样先换算再比）；
//! - 旧侧元素映射后新侧不存在 → missing 段显式列出（不静默）；新侧多出 → added 段；
//! - 输出确定性：段内按 id（BTreeMap）排序，字段差异按路径排序，同输入同输出。
//!
//! 两侧输入都先 parse 成类型化 [`GameSpec`] 再转 JSON 树比对：serde default 抹平
//! 「旧档缺键 vs 新档空数组」的假差异（如 W7 新增的 `graphs` 段）。

use adm4_spec::GameSpec;
use serde::Deserialize;
use serde_json::Value;
use std::collections::BTreeMap;

/// 前缀规则：`from`/`to` 都必须以 `*` 结尾，如 `"ld.tower_*" → "build_main.tower_*"`。
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PrefixRule {
    pub from: String,
    pub to: String,
}

/// id 映射表（旧 id → 新 id）。三段全部可省：全省 = 恒等映射。
///
/// - `exact`：逐条精确映射，整串匹配，优先级最高；
/// - `prefix`：前缀规则，串首匹配，多规则命中取最长前缀；
/// - `ignore_paths`：比对豁免路径（风格对齐 golden_diff 豁免清单），命中该
///   路径（含其子路径）的差异不计入报告——供 4b 豁免 `identity.frozen_hash`
///   这类迁移必然漂移的字段，豁免决定留痕在映射表文件里。
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct IdMapping {
    #[serde(default)]
    pub exact: BTreeMap<String, String>,
    #[serde(default)]
    pub prefix: Vec<PrefixRule>,
    #[serde(default)]
    pub ignore_paths: Vec<String>,
}

impl IdMapping {
    /// fail-closed 校验：格式错误直接 Err 点名，不静默降级成恒等。
    pub fn validate(&self) -> Result<(), String> {
        for (from, to) in &self.exact {
            if from.is_empty() || to.is_empty() {
                return Err("精确映射的旧 id / 新 id 不得为空串".to_string());
            }
        }
        let mut seen_stems: Vec<&str> = Vec::new();
        for rule in &self.prefix {
            let Some(from_stem) = rule.from.strip_suffix('*') else {
                return Err(format!("前缀规则 from 必须以 * 结尾：{}", rule.from));
            };
            if rule.to.strip_suffix('*').is_none() {
                return Err(format!("前缀规则 to 必须以 * 结尾：{}", rule.to));
            }
            if from_stem.is_empty() {
                return Err(format!("前缀规则 from 的前缀部分不得为空：{}", rule.from));
            }
            if seen_stems.contains(&from_stem) {
                return Err(format!("前缀规则重复：{}", rule.from));
            }
            seen_stems.push(from_stem);
        }
        Ok(())
    }

    /// id 换算：精确映射（整串）优先，其次最长命中前缀规则；无命中原样返回。
    fn map_id(&self, id: &str) -> String {
        if let Some(mapped) = self.exact.get(id) {
            return mapped.clone();
        }
        let mut best: Option<&PrefixRule> = None;
        for rule in &self.prefix {
            let stem = rule.from.trim_end_matches('*');
            if id.starts_with(stem)
                && best.is_none_or(|b| stem.len() > b.from.trim_end_matches('*').len())
            {
                best = Some(rule);
            }
        }
        match best {
            Some(rule) => {
                let from_stem = rule.from.trim_end_matches('*');
                let to_stem = rule.to.trim_end_matches('*');
                format!("{to_stem}{}", &id[from_stem.len()..])
            }
            None => id.to_string(),
        }
    }

    /// 字符串换算：先按 id 换算；不命中且含 `/` 时按 SpecRef（`section/id`）对尾段换算。
    fn map_string(&self, text: &str) -> String {
        let mapped = self.map_id(text);
        if mapped != text {
            return mapped;
        }
        if let Some((section, id)) = text.split_once('/') {
            let mapped_id = self.map_id(id);
            if mapped_id != id {
                return format!("{section}/{mapped_id}");
            }
        }
        text.to_string()
    }
}

/// 字段级差异（路径 + 旧值 + 新值，值已做展示截断）。
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct FieldDiff {
    pub path: String,
    pub old: String,
    pub new: String,
}

/// 旧侧元素映射后新侧缺失（含悬空映射：旧 id 无映射且新侧无同 id）。
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct MissingEntry {
    pub section: String,
    pub old_id: String,
    pub mapped_id: String,
}

/// 新侧多出的元素。
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct AddedEntry {
    pub section: String,
    pub id: String,
}

/// 语义 diff 报告：三段全空 = 语义零 diff。
#[derive(Debug, Default)]
pub struct DiffReport {
    pub changed: Vec<FieldDiff>,
    pub missing: Vec<MissingEntry>,
    pub added: Vec<AddedEntry>,
}

impl DiffReport {
    pub fn is_clean(&self) -> bool {
        self.changed.is_empty() && self.missing.is_empty() && self.added.is_empty()
    }

    /// 确定性中文报告正文（同输入同输出；调用方自行加上下文头）。
    pub fn render(&self) -> String {
        let mut out = String::new();
        if self.is_clean() {
            out.push_str("[结论] 语义零 diff：映射后除 id 本身外全部字段逐字节相等\n");
            return out;
        }
        if !self.changed.is_empty() {
            out.push_str(&format!("== 字段级差异（{} 条）==\n", self.changed.len()));
            for diff in &self.changed {
                out.push_str(&format!(
                    "  {}\n    旧值：{}\n    新值：{}\n",
                    diff.path, diff.old, diff.new
                ));
            }
        }
        if !self.missing.is_empty() {
            out.push_str(&format!(
                "== missing（旧侧元素映射后新侧缺失，{} 条）==\n",
                self.missing.len()
            ));
            for entry in &self.missing {
                out.push_str(&format!(
                    "  {}：旧 id {}（映射后 {}）新侧不存在\n",
                    entry.section, entry.old_id, entry.mapped_id
                ));
            }
        }
        if !self.added.is_empty() {
            out.push_str(&format!(
                "== added（新侧多出的元素，{} 条）==\n",
                self.added.len()
            ));
            for entry in &self.added {
                out.push_str(&format!("  {}：新侧多出 {}\n", entry.section, entry.id));
            }
        }
        out.push_str(&format!(
            "[结论] 发现差异：字段级 {} 条，missing {} 条，added {} 条\n",
            self.changed.len(),
            self.missing.len(),
            self.added.len()
        ));
        out
    }
}

/// GameSpec 里带 id 的元素段（intent/identity 单独比，source_map 按集合比）。
const ID_SECTIONS: [&str; 7] = [
    "systems",
    "mechanics",
    "entities",
    "tables",
    "content",
    "graphs",
    "acceptance",
];

/// 语义 diff 主入口。Err = 输入/映射表不合法（映射碰撞等），与「有差异」区分。
pub fn diff_specs(
    old: &GameSpec,
    new: &GameSpec,
    mapping: &IdMapping,
) -> Result<DiffReport, String> {
    mapping.validate()?;

    let old_value = serde_json::to_value(old).map_err(|e| format!("旧侧 spec 序列化失败：{e}"))?;
    let new_value = serde_json::to_value(new).map_err(|e| format!("新侧 spec 序列化失败：{e}"))?;

    // 映射碰撞检测：同段内两个旧 id 换算到同一新 id → 映射表错误，直接 Err。
    let mut id_pairs: BTreeMap<&str, Vec<(String, String)>> = BTreeMap::new();
    for section in ID_SECTIONS {
        let pairs = collect_id_pairs(&old_value, section, mapping)?;
        let mut by_mapped: BTreeMap<&str, Vec<&str>> = BTreeMap::new();
        for (old_id, mapped_id) in &pairs {
            by_mapped.entry(mapped_id).or_default().push(old_id);
        }
        for (mapped_id, old_ids) in &by_mapped {
            if old_ids.len() > 1 {
                return Err(format!(
                    "映射碰撞：{section} 段的旧 id [{}] 都换算到 {mapped_id}",
                    old_ids.join("、")
                ));
            }
        }
        id_pairs.insert(section, pairs);
    }

    // 旧侧整树字符串换算（source_map 的原始形态先留底，missing 报告要显示旧貌）。
    let old_source_pairs = collect_source_map_pairs(&old_value, mapping)?;
    let mut old_mapped = old_value;
    rewrite_strings(&mut old_mapped, mapping);

    let mut report = DiffReport::default();
    let ignore = &mapping.ignore_paths;

    diff_value(
        "identity",
        &old_mapped["identity"],
        &new_value["identity"],
        ignore,
        &mut report.changed,
    );
    diff_value(
        "intent",
        &old_mapped["intent"],
        &new_value["intent"],
        ignore,
        &mut report.changed,
    );

    for section in ID_SECTIONS {
        compare_id_section(
            section,
            &old_mapped,
            &new_value,
            &id_pairs[section],
            ignore,
            &mut report,
        )?;
    }

    compare_source_map(&old_source_pairs, &new_value, ignore, &mut report)?;

    report.changed.sort();
    report.missing.sort();
    report.added.sort();
    Ok(report)
}

/// 取某段全部 (旧 id, 映射后 id)。段结构不对（非数组 / 元素缺 id）→ Err。
fn collect_id_pairs(
    spec: &Value,
    section: &str,
    mapping: &IdMapping,
) -> Result<Vec<(String, String)>, String> {
    let items = spec[section]
        .as_array()
        .ok_or_else(|| format!("spec 的 {section} 段不是数组"))?;
    items
        .iter()
        .map(|item| {
            let id = item["id"]
                .as_str()
                .ok_or_else(|| format!("{section} 段有元素缺 id 字段"))?;
            Ok((id.to_string(), mapping.map_id(id)))
        })
        .collect()
}

/// source_map 条目：((映射后 spec_path, 映射后 decision_id), (原 spec_path, 原 decision_id))。
type SourceMapPairs = BTreeMap<(String, String), (String, String)>;

fn collect_source_map_pairs(spec: &Value, mapping: &IdMapping) -> Result<SourceMapPairs, String> {
    let items = spec["source_map"]
        .as_array()
        .ok_or("spec 的 source_map 段不是数组")?;
    let mut pairs = SourceMapPairs::new();
    for item in items {
        let spec_path = item["spec_path"]
            .as_str()
            .ok_or("source_map 有条目缺 spec_path")?;
        let decision_id = item["decision_id"]
            .as_str()
            .ok_or("source_map 有条目缺 decision_id")?;
        pairs.insert(
            (
                mapping.map_string(spec_path),
                mapping.map_string(decision_id),
            ),
            (spec_path.to_string(), decision_id.to_string()),
        );
    }
    Ok(pairs)
}

/// 整树字符串换算：只改字符串值，不改对象键（GameSpec 里对象键是字段名/列名，非 id）。
fn rewrite_strings(value: &mut Value, mapping: &IdMapping) {
    match value {
        Value::String(text) => {
            let mapped = mapping.map_string(text);
            if mapped != *text {
                *text = mapped;
            }
        }
        Value::Array(items) => {
            for item in items {
                rewrite_strings(item, mapping);
            }
        }
        Value::Object(fields) => {
            for item in fields.values_mut() {
                rewrite_strings(item, mapping);
            }
        }
        _ => {}
    }
}

/// 路径是否命中豁免（豁免项本身或其子路径）。
fn is_ignored(path: &str, ignore_paths: &[String]) -> bool {
    ignore_paths.iter().any(|prefix| {
        path == prefix
            || (path.starts_with(prefix)
                && matches!(path.as_bytes().get(prefix.len()), Some(b'.') | Some(b'[')))
    })
}

/// 值的短文本呈现（报告用，对齐 golden_diff 截断口径）。
fn short(value: &Value) -> String {
    let text = value.to_string();
    if text.chars().count() > 120 {
        let head: String = text.chars().take(117).collect();
        format!("{head}...")
    } else {
        text
    }
}

/// 递归字段级比对：类型不同/标量不等/键增删/数组长度漂移都逐条进 changed。
fn diff_value(path: &str, old: &Value, new: &Value, ignore: &[String], out: &mut Vec<FieldDiff>) {
    if is_ignored(path, ignore) {
        return;
    }
    match (old, new) {
        (Value::Object(old_fields), Value::Object(new_fields)) => {
            for (key, old_child) in old_fields {
                let child_path = format!("{path}.{key}");
                match new_fields.get(key) {
                    Some(new_child) => diff_value(&child_path, old_child, new_child, ignore, out),
                    None => {
                        if !is_ignored(&child_path, ignore) {
                            out.push(FieldDiff {
                                path: child_path,
                                old: short(old_child),
                                new: "(键不存在)".to_string(),
                            });
                        }
                    }
                }
            }
            for (key, new_child) in new_fields {
                if old_fields.contains_key(key) {
                    continue;
                }
                let child_path = format!("{path}.{key}");
                if !is_ignored(&child_path, ignore) {
                    out.push(FieldDiff {
                        path: child_path,
                        old: "(键不存在)".to_string(),
                        new: short(new_child),
                    });
                }
            }
        }
        (Value::Array(old_items), Value::Array(new_items)) => {
            if old_items.len() != new_items.len() {
                out.push(FieldDiff {
                    path: path.to_string(),
                    old: format!("数组长度 {}", old_items.len()),
                    new: format!("数组长度 {}", new_items.len()),
                });
                return;
            }
            for (index, (old_item, new_item)) in old_items.iter().zip(new_items).enumerate() {
                diff_value(&format!("{path}[{index}]"), old_item, new_item, ignore, out);
            }
        }
        _ => {
            if old != new {
                out.push(FieldDiff {
                    path: path.to_string(),
                    old: short(old),
                    new: short(new),
                });
            }
        }
    }
}

/// 带 id 段比对：两侧按 id 建索引，交集逐字段比，旧独有进 missing，新独有进 added。
fn compare_id_section(
    section: &str,
    old_mapped: &Value,
    new_value: &Value,
    id_pairs: &[(String, String)],
    ignore: &[String],
    report: &mut DiffReport,
) -> Result<(), String> {
    let old_items = old_mapped[section]
        .as_array()
        .ok_or_else(|| format!("旧侧 {section} 段不是数组"))?;
    let new_items = new_value[section]
        .as_array()
        .ok_or_else(|| format!("新侧 {section} 段不是数组"))?;

    let mut old_by_id: BTreeMap<&str, &Value> = BTreeMap::new();
    for item in old_items {
        let id = item["id"]
            .as_str()
            .ok_or_else(|| format!("旧侧 {section} 段有元素缺 id"))?;
        old_by_id.insert(id, item);
    }
    let mut new_by_id: BTreeMap<&str, &Value> = BTreeMap::new();
    for item in new_items {
        let id = item["id"]
            .as_str()
            .ok_or_else(|| format!("新侧 {section} 段有元素缺 id"))?;
        new_by_id.insert(id, item);
    }
    let mapped_to_old: BTreeMap<&str, &str> = id_pairs
        .iter()
        .map(|(old_id, mapped_id)| (mapped_id.as_str(), old_id.as_str()))
        .collect();

    for (mapped_id, old_item) in &old_by_id {
        let element_path = format!("{section}[{mapped_id}]");
        if is_ignored(&element_path, ignore) {
            continue;
        }
        match new_by_id.get(mapped_id) {
            Some(new_item) => {
                diff_value(
                    &element_path,
                    old_item,
                    new_item,
                    ignore,
                    &mut report.changed,
                );
            }
            None => report.missing.push(MissingEntry {
                section: section.to_string(),
                old_id: mapped_to_old
                    .get(mapped_id)
                    .unwrap_or(mapped_id)
                    .to_string(),
                mapped_id: (*mapped_id).to_string(),
            }),
        }
    }
    for id in new_by_id.keys() {
        if old_by_id.contains_key(id) {
            continue;
        }
        let element_path = format!("{section}[{id}]");
        if is_ignored(&element_path, ignore) {
            continue;
        }
        report.added.push(AddedEntry {
            section: section.to_string(),
            id: (*id).to_string(),
        });
    }
    Ok(())
}

/// source_map 按 (spec_path, decision_id) 集合比：旧独有进 missing（显示原貌与映射后），
/// 新独有进 added。条目无字段内层，不产字段级差异。
fn compare_source_map(
    old_pairs: &SourceMapPairs,
    new_value: &Value,
    ignore: &[String],
    report: &mut DiffReport,
) -> Result<(), String> {
    let new_items = new_value["source_map"]
        .as_array()
        .ok_or("新侧 source_map 段不是数组")?;
    let mut new_set: BTreeMap<(String, String), ()> = BTreeMap::new();
    for item in new_items {
        let spec_path = item["spec_path"]
            .as_str()
            .ok_or("新侧 source_map 有条目缺 spec_path")?;
        let decision_id = item["decision_id"]
            .as_str()
            .ok_or("新侧 source_map 有条目缺 decision_id")?;
        new_set.insert((spec_path.to_string(), decision_id.to_string()), ());
    }

    for (mapped, original) in old_pairs {
        let element_path = format!("source_map[{}|{}]", mapped.0, mapped.1);
        if is_ignored(&element_path, ignore) || new_set.contains_key(mapped) {
            continue;
        }
        report.missing.push(MissingEntry {
            section: "source_map".to_string(),
            old_id: format!("spec_path={} decision_id={}", original.0, original.1),
            mapped_id: format!("spec_path={} decision_id={}", mapped.0, mapped.1),
        });
    }
    for key in new_set.keys() {
        let element_path = format!("source_map[{}|{}]", key.0, key.1);
        if is_ignored(&element_path, ignore) || old_pairs.contains_key(key) {
            continue;
        }
        report.added.push(AddedEntry {
            section: "source_map".to_string(),
            id: format!("spec_path={} decision_id={}", key.0, key.1),
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    /// 最小合法 GameSpec（各段一个元素，id 引用/design_notes/source_map 全覆盖）。
    fn spec_value(p: &str) -> Value {
        json!({
            "identity": {
                "schema_version": "4.0.0",
                "project_id": "示例项目",
                "frozen_hash": "sha256:abc"
            },
            "intent": { "title": "示例", "experience_promise": "承诺" },
            "systems": [{
                "id": format!("{p}.combat"),
                "name": "战斗",
                "purpose": "克制式伤害",
                "interfaces": ["需要克制矩阵"],
                "design_notes": [{
                    "source_decision": format!("{p}.combat_style"),
                    "source_option": "counter",
                    "role": "rationale",
                    "text": "克制关系带来配队深度"
                }]
            }],
            "mechanics": [{
                "id": format!("{p}.damage"),
                "system_id": format!("{p}.combat"),
                "rule_text": "伤害 = 攻击 × 克制系数",
                "effects": [{
                    "effect": "modify_property",
                    "entity": format!("{p}.enemy"),
                    "property": "hp",
                    "formula": format!("{p}.damage_base * 2")
                }]
            }],
            "entities": [{ "id": format!("{p}.enemy"), "name": "敌人" }],
            "tables": [{
                "id": format!("{p}.stats"),
                "columns": [{ "key": "hp", "kind": { "kind": "int" } }],
                "row_key": "hp"
            }],
            "content": [{
                "id": format!("{p}.level1"),
                "content_kind": "level",
                "data": { "waves": 3 }
            }],
            "source_map": [{
                "spec_path": format!("systems/{p}.combat"),
                "decision_id": format!("{p}.combat_style")
            }]
        })
    }

    fn spec(p: &str) -> GameSpec {
        serde_json::from_value(spec_value(p)).expect("测试 spec 应能解析")
    }

    fn prefix_mapping(from: &str, to: &str) -> IdMapping {
        IdMapping {
            prefix: vec![PrefixRule {
                from: format!("{from}*"),
                to: format!("{to}*"),
            }],
            ..IdMapping::default()
        }
    }

    #[test]
    fn 恒等映射零diff() {
        let s = spec("ld");
        let report = diff_specs(&s, &s, &IdMapping::default()).expect("diff 应成功");
        assert!(report.is_clean(), "自比对应零 diff：\n{}", report.render());
        assert!(report.render().contains("语义零 diff"));
    }

    #[test]
    fn 改名映射零diff() {
        // 旧 ld.* → 新 build_main.*：元素 id、system_id 引用、effects 实体引用、
        // 公式内前缀引用、design_notes 决策 id、source_map SpecRef 全部要跟着换算。
        let old = spec("ld");
        let new = spec("build_main");
        let report =
            diff_specs(&old, &new, &prefix_mapping("ld.", "build_main.")).expect("diff 应成功");
        assert!(
            report.is_clean(),
            "改名映射后应零 diff：\n{}",
            report.render()
        );
    }

    #[test]
    fn 字段漂移被抓且点名路径() {
        let old = spec("ld");
        let mut new_value = spec_value("ld");
        new_value["systems"][0]["purpose"] = json!("完全不同的目的");
        let new: GameSpec = serde_json::from_value(new_value).expect("应能解析");
        let report = diff_specs(&old, &new, &IdMapping::default()).expect("diff 应成功");
        assert_eq!(
            report.changed.len(),
            1,
            "只该抓到一条漂移：\n{}",
            report.render()
        );
        assert_eq!(report.changed[0].path, "systems[ld.combat].purpose");
        assert!(report.changed[0].old.contains("克制式伤害"));
        assert!(report.changed[0].new.contains("完全不同的目的"));
        assert!(report.missing.is_empty() && report.added.is_empty());
    }

    #[test]
    fn 悬空映射进missing() {
        // 旧侧 ld.combat 无映射规则、新侧也没有同 id 元素 → 必须显式进 missing。
        let old = spec("ld");
        let new = spec("build_main");
        let report = diff_specs(&old, &new, &IdMapping::default()).expect("diff 应成功");
        assert!(
            report.missing.iter().any(|m| m.section == "systems"
                && m.old_id == "ld.combat"
                && m.mapped_id == "ld.combat"),
            "悬空的 ld.combat 应进 missing：\n{}",
            report.render()
        );
        assert!(
            report.missing.iter().any(|m| m.section == "source_map"),
            "source_map 悬空条目也应进 missing"
        );
    }

    #[test]
    fn 新增元素进added() {
        let old = spec("ld");
        let mut new_value = spec_value("ld");
        new_value["entities"]
            .as_array_mut()
            .expect("entities 是数组")
            .push(json!({ "id": "ld.tower", "name": "新塔" }));
        let new: GameSpec = serde_json::from_value(new_value).expect("应能解析");
        let report = diff_specs(&old, &new, &IdMapping::default()).expect("diff 应成功");
        assert_eq!(report.added.len(), 1);
        assert_eq!(report.added[0].section, "entities");
        assert_eq!(report.added[0].id, "ld.tower");
        assert!(report.changed.is_empty() && report.missing.is_empty());
    }

    #[test]
    fn 前缀规则与精确映射并用() {
        // 精确映射优先改写 ld.damage（含其 design/引用），其余 ld.* 走前缀规则。
        let old = spec("ld");
        let mut new_value = spec_value("build_main");
        new_value["mechanics"][0]["id"] = json!("core.damage_rule");
        // 精确映射整串匹配：公式里的 "ld.damage_base * 2" 走前缀规则换算，
        // 新侧对应 "build_main.damage_base * 2"（spec_value 已生成）。
        let new: GameSpec = serde_json::from_value(new_value).expect("应能解析");
        let mapping = IdMapping {
            exact: BTreeMap::from([("ld.damage".to_string(), "core.damage_rule".to_string())]),
            prefix: vec![PrefixRule {
                from: "ld.*".to_string(),
                to: "build_main.*".to_string(),
            }],
            ignore_paths: Vec::new(),
        };
        let report = diff_specs(&old, &new, &mapping).expect("diff 应成功");
        assert!(
            report.is_clean(),
            "精确+前缀并用应零 diff：\n{}",
            report.render()
        );
    }

    #[test]
    fn 最长前缀优先() {
        let mapping = IdMapping {
            prefix: vec![
                PrefixRule {
                    from: "ld.*".to_string(),
                    to: "a.*".to_string(),
                },
                PrefixRule {
                    from: "ld.tower_*".to_string(),
                    to: "b.tower_*".to_string(),
                },
            ],
            ..IdMapping::default()
        };
        assert_eq!(mapping.map_id("ld.tower_types"), "b.tower_types");
        assert_eq!(mapping.map_id("ld.wave"), "a.wave");
        assert_eq!(mapping.map_id("gs.unit"), "gs.unit");
    }

    #[test]
    fn 映射碰撞报错不静默() {
        // ld.combat 精确映射到 ld.enemy 已占的 id 形态：两个旧 id 落到同一新 id。
        let old = spec("ld");
        let mapping = IdMapping {
            exact: BTreeMap::from([
                ("ld.combat".to_string(), "x.same".to_string()),
                // systems 段只有一个元素，碰撞要构造在同段：给 entities 段造两条。
            ]),
            ..IdMapping::default()
        };
        // 构造 entities 两元素映射到同一 id。
        let mut old_value = spec_value("ld");
        old_value["entities"]
            .as_array_mut()
            .expect("entities 是数组")
            .push(json!({ "id": "ld.enemy2", "name": "敌人二" }));
        let old2: GameSpec = serde_json::from_value(old_value).expect("应能解析");
        let mapping2 = IdMapping {
            exact: BTreeMap::from([
                ("ld.enemy".to_string(), "x.foe".to_string()),
                ("ld.enemy2".to_string(), "x.foe".to_string()),
            ]),
            ..IdMapping::default()
        };
        let err = diff_specs(&old2, &old2, &mapping2).expect_err("碰撞必须 Err");
        assert!(err.contains("映射碰撞"), "错误要点名碰撞：{err}");
        // 顺带：不碰撞的映射照常可跑（映射后 missing/added 而非 Err）。
        assert!(diff_specs(&old, &old, &mapping).is_ok());
    }

    #[test]
    fn 前缀规则格式错误fail_closed() {
        let bad = IdMapping {
            prefix: vec![PrefixRule {
                from: "ld.".to_string(),
                to: "a.*".to_string(),
            }],
            ..IdMapping::default()
        };
        let s = spec("ld");
        let err = diff_specs(&s, &s, &bad).expect_err("from 缺 * 必须 Err");
        assert!(err.contains("必须以 * 结尾"), "错误要点名格式：{err}");
    }

    #[test]
    fn 豁免路径不计差异() {
        let old = spec("ld");
        let mut new_value = spec_value("ld");
        new_value["identity"]["frozen_hash"] = json!("sha256:changed");
        let new: GameSpec = serde_json::from_value(new_value).expect("应能解析");
        let mapping = IdMapping {
            ignore_paths: vec!["identity.frozen_hash".to_string()],
            ..IdMapping::default()
        };
        let report = diff_specs(&old, &new, &mapping).expect("diff 应成功");
        assert!(report.is_clean(), "豁免后应零 diff：\n{}", report.render());
        // 无豁免时同一漂移必须被抓——豁免不是默认行为。
        let caught = diff_specs(&old, &new, &IdMapping::default()).expect("diff 应成功");
        assert_eq!(caught.changed.len(), 1);
        assert_eq!(caught.changed[0].path, "identity.frozen_hash");
    }

    #[test]
    fn 报告输出确定性() {
        let old = spec("ld");
        let new = spec("build_main");
        let render = |r: &DiffReport| r.render();
        let a = render(&diff_specs(&old, &new, &IdMapping::default()).expect("diff 应成功"));
        let b = render(&diff_specs(&old, &new, &IdMapping::default()).expect("diff 应成功"));
        assert_eq!(a, b, "同输入必须同输出");
    }
}
