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
    /// baseline 点被显式跳过：不进分母，理由码进报告。
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
/// 3. `requirement=Baseline` 的点无论激活与否都 Active，除非显式 N/A；
/// 4. `level > depth_profile.target` 一律 BeyondDepth。
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
        let selected_option = selections
            .get(&point.id)
            .and_then(|selection| point.option(&selection.option_id));
        for option in &point.options {
            for target in &option.unlocks {
                unlock_targets.insert(target.as_str());
            }
        }
        if let Some(option) = selected_option {
            for target in &option.unlocks {
                unlocked_now.insert(target.as_str());
            }
        }
    }

    let mut map = ApplicabilityMap::new();
    for point in graph.points() {
        if point.level > depth.target {
            map.insert(point.id.clone(), PointApplicability::BeyondDepth);
            continue;
        }
        if let Some(justification) = not_applicable.get(&point.id)
            && point.requirement == PointRequirement::Baseline
        {
            map.insert(
                point.id.clone(),
                PointApplicability::NotApplicable(justification.clone()),
            );
            continue;
        }
        // 非 baseline 点的 N/A 声明无效——按激活规则处理。
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
    use crate::types::{DecisionOption, DecisionPoint, GenreScope, ParameterValues, Provenance};

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
            NaJustification {
                reason_code: "no_persistence".into(),
                note: "超休闲无存档".into(),
            },
        )]
        .into_iter()
        .collect();
        let map = compute_applicability(&graph, &BTreeMap::new(), &na, depth);
        assert!(matches!(
            map["save_system"],
            PointApplicability::NotApplicable(_)
        ));
    }
}
