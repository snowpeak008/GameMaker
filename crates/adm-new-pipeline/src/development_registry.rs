use std::collections::BTreeMap;

use adm_new_contracts::ArtifactLocale;
use adm_new_contracts::pipeline::{PipelineRegistry, SourceGroupSpec, StageKind, StageSpec};
use serde_json::json;

use crate::source::SourceGroup;
use crate::stages::step00_02::{
    StagePluginSpec, step00_plugin_spec, step01_plugin_spec, step02_plugin_spec,
};
use crate::stages::step03_06::{
    step03_plugin_spec, step04_plugin_spec, step05_plugin_spec, step06_plugin_spec,
};
use crate::stages::step07::step07_plugin_spec;
use crate::stages::step08_14::{
    step08_plugin_spec, step09_plugin_spec, step10_plugin_spec, step11_plugin_spec,
    step12_plugin_spec, step13_plugin_spec, step14_plugin_spec,
};

const STAGE_TITLES: [&str; 15] = [
    "Idea Intake",
    "Gameplay Framework",
    "Design Review and Freeze",
    "Program Requirements",
    "Art Requirements",
    "Program Review",
    "Art Review",
    "Art Style Generation and Confirmation",
    "Design to Development Plan",
    "Art Production Plan",
    "Asset Alignment",
    "Development Execution",
    "Art Production",
    "Scene Assembly",
    "Integration Validation",
];

const STAGE_TITLES_ZH_CN: [&str; 15] = [
    "创意接收",
    "玩法框架",
    "设计评审与冻结",
    "程序需求",
    "美术需求",
    "程序评审",
    "美术评审",
    "美术风格生成与确认",
    "设计转开发计划",
    "美术生产计划",
    "资源对齐",
    "开发执行",
    "美术生产",
    "场景组装",
    "集成验证",
];

pub fn localized_stage_title(number: u32, locale: ArtifactLocale) -> String {
    let index = number as usize;
    let title = match locale {
        ArtifactLocale::ZhCn => STAGE_TITLES_ZH_CN.get(index).copied(),
        ArtifactLocale::EnUs => STAGE_TITLES.get(index).copied(),
    }
    .unwrap_or("Pipeline Stage");
    if locale == ArtifactLocale::ZhCn {
        format!("步骤 {number:02} {title}")
    } else {
        format!("Step{number:02} {title}")
    }
}

const STAGE_SLUGS: [&str; 15] = [
    "idea_intake",
    "gameplay_framework",
    "design_review_freeze",
    "program_requirements",
    "art_requirements",
    "program_review",
    "art_review",
    "art_style_generation",
    "design_to_plan",
    "art_plan",
    "asset_alignment",
    "dev_execution",
    "art_production",
    "scene_assembly",
    "integration_validation",
];

const STAGE_REQUIRES: [&[&str]; 15] = [
    &[],
    &["00"],
    &["01"],
    &["02"],
    &["02"],
    &["03"],
    &["04"],
    &["06"],
    &["05"],
    &["07"],
    &["08", "09"],
    &["10"],
    &["10"],
    &["11", "12"],
    &["13"],
];

/// The product registry for the serial Step00-14 development pipeline.
pub fn default_development_registry() -> PipelineRegistry {
    let plugins = plugin_specs();
    PipelineRegistry {
        stages: (0..=14)
            .map(|number| {
                let plugin = &plugins[number];
                let stage_id = format!("{number:02}");
                let mut metadata = BTreeMap::from([
                    ("serial_order".to_string(), json!(number)),
                    (
                        "generation_entrypoint".to_string(),
                        json!(plugin.generation_entrypoint),
                    ),
                ]);
                if number == 7 {
                    metadata.insert("manual_gate".to_string(), json!(true));
                    metadata.insert(
                        "confirmation_artifact".to_string(),
                        json!("style_confirmation.json"),
                    );
                }
                StageSpec {
                    stage_id: stage_id.clone(),
                    kind: match number {
                        7 => StageKind::HumanGate,
                        14 => StageKind::Validation,
                        _ => StageKind::Development,
                    },
                    number: Some(number as u32),
                    slug: STAGE_SLUGS[number].to_string(),
                    title: format!("Step{stage_id} {}", STAGE_TITLES[number]),
                    requires: STAGE_REQUIRES[number]
                        .iter()
                        .map(|value| (*value).to_string())
                        .collect(),
                    source_groups: plugin.source_groups.iter().map(source_group_spec).collect(),
                    plugin_ref: format!("pipeline.step_{stage_id}_{}.plugin", STAGE_SLUGS[number]),
                    metadata,
                }
            })
            .collect(),
    }
}

fn plugin_specs() -> Vec<StagePluginSpec> {
    vec![
        step00_plugin_spec(),
        step01_plugin_spec(),
        step02_plugin_spec(),
        step03_plugin_spec(),
        step04_plugin_spec(),
        step05_plugin_spec(),
        step06_plugin_spec(),
        step07_plugin_spec(),
        step08_plugin_spec(),
        step09_plugin_spec(),
        step10_plugin_spec(),
        step11_plugin_spec(),
        step12_plugin_spec(),
        step13_plugin_spec(),
        step14_plugin_spec(),
    ]
}

fn source_group_spec(group: &SourceGroup) -> SourceGroupSpec {
    SourceGroupSpec {
        label: group.label.clone(),
        pattern: group.patterns.join(";"),
        mode: group.mode.clone(),
        source_type: group.source_ids.join(","),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::PipelineService;

    #[test]
    fn default_registry_is_the_ordered_step00_14_product_pipeline() {
        let registry = default_development_registry();
        assert_eq!(registry.stages.len(), 15);
        assert_eq!(registry.stages[0].stage_id, "00");
        assert_eq!(registry.stages[14].stage_id, "14");
        assert_eq!(registry.stages[7].kind, StageKind::HumanGate);
        assert_eq!(registry.stages[10].requires, vec!["08", "09"]);
        assert_eq!(registry.stages[13].requires, vec!["11", "12"]);

        let order = PipelineService::new(registry)
            .unwrap()
            .topological_order()
            .unwrap();
        assert_eq!(
            order,
            (0..=14)
                .map(|number| format!("{number:02}"))
                .collect::<Vec<_>>()
        );
    }
}
