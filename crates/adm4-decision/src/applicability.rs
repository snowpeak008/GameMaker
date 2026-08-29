use crate::graph::DecisionGraph;
use crate::types::{
    DecisionId, DepthProfile, DesignLevel, NaJustification, PointRequirement, Selection,
};
use std::collections::{BTreeMap, BTreeSet};

/// 决策点对某项目的适用性（设计 01 号文档 §2.4）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PointApplicability {
    /// 激活：必须 confirmed 才算完成。
    Active,
    /// 未被任何已选选项 unlock：不进分母。
    Inactive,
    /// 显式 N/A：不进分母，理由码进报告。
    /// 两条来路——baseline 点的结构化理由码跳过，以及适用点的人工豁免（带署名，R3）。
    NotApplicable(NaJustification),
    /// 超出深度档：不进分母。
    BeyondDepth,
}

pub type ApplicabilityMap = BTreeMap<DecisionId, PointApplicability>;

/// 计算全图适用性。
///
/// 规则：
/// 1. L0-L2 的通用根点恒 Active；
/// 2. 深层点（L3+）只有被已选选项 `unlocks` 才 Active，否则 Inactive；
///    没有任何父点声明 unlock 它的点视为根点（恒 Active）；
///    多选点按**全部已选选项**取 unlocks 并集（少算一个选项就会漏激活下游）；
/// 3. `requirement=Baseline` 的点无论激活与否都 Active，除非显式 N/A；
/// 4. `level > depth_profile.target` 一律 BeyondDepth；
/// 5. 显式 N/A（`not_applicable`）对任何未超深度档的点生效——人工豁免适用点是
///    二版「不适用」能力的归宿；理由/署名的合法性由 `AuthoringEngine` 入口把关，
///    本函数只负责归属，不复核（免得同一规则两处实现出现分歧）。
pub fn compute_applicability(
    graph: &DecisionGraph,
    selections: &BTreeMap<DecisionId, Selection>,
    not_applicable: &BTreeMap<DecisionId, NaJustification>,
    depth: DepthProfile,
) -> ApplicabilityMap {
    // 全图被声明为 unlock 目标的点集合（无论选没选）。
    let mut unlock_targets: BTreeSet<&str> = BTreeSet::new();
    // 被「已选选项」实际激活的点集合。
    let mut unlocked_now: BTreeSet<&str> = BTreeSet::new();

    for point in graph.points() {
        for option in &point.options {
            for target in &option.unlocks {
                unlock_targets.insert(target.as_str());
            }
        }
        if let Some(selection) = selections.get(&point.id) {
            for selected in selection.selected_options() {
                let Some(option) = point.option(selected.option_id) else {
                    continue;
                };
                for target in &option.unlocks {
                    unlocked_now.insert(target.as_str());
                }
            }
        }
    }

    let mut map = ApplicabilityMap::new();
    for point in graph.points() {
        if point.level > depth.target {
            map.insert(point.id.clone(), PointApplicability::BeyondDepth);
            continue;
        }
        if let Some(justification) = not_applicable.get(&point.id) {
            map.insert(
                point.id.clone(),
                PointApplicability::NotApplicable(justification.clone()),
            );
            continue;
        }
        let is_root = point.level <= DesignLevel::L2 || !unlock_targets.contains(point.id.as_str());
        let active = is_root
            || unlocked_now.contains(point.id.as_str())
            || point.requirement == PointRequirement::Baseline;
        map.insert(
            point.id.clone(),
            if active {
                PointApplicability::Active
            } else {
                PointApplicability::Inactive
            },
        );
    }
    map
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{
        DecisionOption, DecisionPoint, GenreScope, ParameterValues, Provenance, SelectionMode,
    };

    fn option(id: &str, unlocks: Vec<&str>) -> DecisionOption {
        DecisionOption {
            id: id.into(),
            label: id.into(),
            unlocks: unlocks.into_iter().map(Into::into).collect(),
            ..Default::default()
        }
    }

    fn point(id: &str, level: DesignLevel, options: Vec<DecisionOption>) -> DecisionPoint {
        DecisionPoint {
            id: id.into(),
            domain: "d".into(),
            level,
            genre_scope: GenreScope::Universal,
            question: "q".into(),
            mda_layer: None,
            design_question: None,
            node_id: None,
            selection_mode: SelectionMode::Single,
            requirement: PointRequirement::Unlocked,
            options,
            skin_fields: Vec::new(),
            evidence_slots: false,
        }
    }

    fn select(decision: &str, option: &str) -> (DecisionId, Selection) {
        (
            decision.into(),
            Selection {
                decision_id: decision.into(),
                option_id: option.into(),
                parameters: ParameterValues::None,
                rationale: String::new(),
                provenance: Provenance::UserManual,
                confirmed_by_user: true,
                template_original: None,
                additional_options: Vec::new(),
                primary_option: None,
            },
        )
    }

    #[test]
    fn unlock_drives_activation_and_depth_caps() {
        let graph = DecisionGraph::new(vec![
            point(
                "genre",
                DesignLevel::L2,
                vec![
                    option("tower", vec!["deep_system"]),
                    option("puzzle", vec![]),
                ],
            ),
            point("deep_system", DesignLevel::L4, vec![option("x", vec![])]),
            point("numbers", DesignLevel::L6, vec![option("y", vec![])]),
        ])
        .unwrap();
        let depth = DepthProfile::new(DesignLevel::L4).unwrap();

        // 未选 genre：deep_system 未激活。
        let map = compute_applicability(&graph, &BTreeMap::new(), &BTreeMap::new(), depth);
        assert_eq!(map["deep_system"], PointApplicability::Inactive);
        assert_eq!(map["genre"], PointApplicability::Active);
        assert_eq!(map["numbers"], PointApplicability::BeyondDepth);

        // 选 tower：deep_system 激活。
        let selections: BTreeMap<_, _> = [select("genre", "tower")].into_iter().collect();
        let map = compute_applicability(&graph, &selections, &BTreeMap::new(), depth);
        assert_eq!(map["deep_system"], PointApplicability::Active);

        // 选 puzzle：deep_system 不激活（简单玩法分母天然小）。
        let selections: BTreeMap<_, _> = [select("genre", "puzzle")].into_iter().collect();
        let map = compute_applicability(&graph, &selections, &BTreeMap::new(), depth);
        assert_eq!(map["deep_system"], PointApplicability::Inactive);
    }

    #[test]
    fn baseline_point_supports_explicit_na() {
        let mut baseline = point(
            "save_system",
            DesignLevel::L3,
            vec![option("cloud", vec![])],
        );
        baseline.requirement = PointRequirement::Baseline;
        let graph = DecisionGraph::new(vec![baseline]).unwrap();
        let depth = DepthProfile::new(DesignLevel::L4).unwrap();

        let map = compute_applicability(&graph, &BTreeMap::new(), &BTreeMap::new(), depth);
        assert_eq!(map["save_system"], PointApplicability::Active);

        let na: BTreeMap<_, _> = [(
            "save_system".to_string(),
            NaJustification::reason_code_only("no_persistence", "超休闲无存档"),
        )]
        .into_iter()
        .collect();
        let map = compute_applicability(&graph, &BTreeMap::new(), &na, depth);
        assert!(matches!(
            map["save_system"],
            PointApplicability::NotApplicable(_)
        ));
    }

    /// 人工豁免：非 baseline 的适用点也能被显式 N/A 移出分母（二版「不适用」的归宿）。
    #[test]
    fn human_exemption_applies_to_any_active_point() {
        let graph = DecisionGraph::new(vec![point(
            "genre",
            DesignLevel::L2,
            vec![option("tower", vec![]), option("puzzle", vec![])],
        )])
        .unwrap();
        let depth = DepthProfile::new(DesignLevel::L4).unwrap();
        assert_eq!(
            compute_applicability(&graph, &BTreeMap::new(), &BTreeMap::new(), depth)["genre"],
            PointApplicability::Active
        );

        let na: BTreeMap<_, _> = [(
            "genre".to_string(),
            NaJustification {
                reason_code: "out_of_scope".into(),
                note: "本项目不做品类分化".into(),
                actor: "主策划".into(),
                at: "2026-08-29T00:00:00Z".into(),
            },
        )]
        .into_iter()
        .collect();
        assert!(matches!(
            compute_applicability(&graph, &BTreeMap::new(), &na, depth)["genre"],
            PointApplicability::NotApplicable(_)
        ));
    }

    /// 多选点的 unlocks 取全部已选选项的并集：只看首选项会漏激活下游。
    #[test]
    fn multi_selection_unlocks_union_of_all_selected_options() {
        let graph = DecisionGraph::new(vec![
            point(
                "systems",
                DesignLevel::L2,
                vec![
                    option("combat", vec!["combat_rule"]),
                    option("economy", vec!["economy_rule"]),
                ],
            ),
            point("combat_rule", DesignLevel::L4, vec![option("x", vec![])]),
            point("economy_rule", DesignLevel::L4, vec![option("y", vec![])]),
        ])
        .unwrap();
        let depth = DepthProfile::new(DesignLevel::L4).unwrap();

        let (id, mut selection) = select("systems", "combat");
        selection.additional_options = vec![crate::types::SelectedOption {
            option_id: "economy".into(),
            ..Default::default()
        }];
        selection.primary_option = Some("economy".into());
        let selections: BTreeMap<_, _> = [(id, selection)].into_iter().collect();

        let map = compute_applicability(&graph, &selections, &BTreeMap::new(), depth);
        assert_eq!(map["combat_rule"], PointApplicability::Active);
        assert_eq!(map["economy_rule"], PointApplicability::Active);
    }
}
