use crate::model::{Confidence, Template};
use adm4_decision::{DecisionGraph, DesignLevel};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LevelCoverage {
    pub answered: usize,
    pub total: usize,
}

/// 逆向答卷覆盖率报告：各层覆盖 + 低置信清单。宁缺勿造——缺口如实呈现。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CoverageReport {
    pub by_level: BTreeMap<String, LevelCoverage>,
    pub low_confidence: Vec<String>,
    pub unanswered: Vec<String>,
    pub conflicted: Vec<String>,
}

pub fn compute_coverage(template: &Template, graph: &DecisionGraph) -> CoverageReport {
    let mut by_level: BTreeMap<String, LevelCoverage> = BTreeMap::new();
    let mut unanswered = Vec::new();
    for level in DesignLevel::all() {
        if level > template.depth_reached {
            continue;
        }
        let key = format!("{level:?}");
        let mut coverage = LevelCoverage {
            answered: 0,
            total: 0,
        };
        for point in graph.points().iter().filter(|point| point.level == level) {
            coverage.total += 1;
            if template
                .answers
                .iter()
                .any(|answer| answer.decision_id == point.id)
            {
                coverage.answered += 1;
            } else {
                unanswered.push(point.id.clone());
            }
        }
        by_level.insert(key, coverage);
    }
    let low_confidence = template
        .answers
        .iter()
        .filter(|answer| {
            answer
                .evidence
                .iter()
                .all(|evidence| evidence.confidence == Confidence::Low)
        })
        .map(|answer| answer.decision_id.clone())
        .collect();
    let conflicted = template
        .answers
        .iter()
        .filter(|answer| answer.crosscheck_agreed == Some(false))
        .map(|answer| answer.decision_id.clone())
        .collect();
    CoverageReport {
        by_level,
        low_confidence,
        unanswered,
        conflicted,
    }
}
