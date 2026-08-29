use crate::framework::StageStatus;
use crate::runner::RunnerContext;
use adm4_foundation::Adm4Result;
use adm4_spec::GameSpec;
use serde::{Deserialize, Serialize};

/// C5 契约：美术方向简报（从规格确定性汇总；风格锚点需人工确认）。
/// 生图锚点（anchor_images）在 Provider 支持 Image 能力时生成，本契约先固化文字简报。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StyleBriefContract {
    pub experience_promise: String,
    pub genre_structure: String,
    pub visual_subjects: Vec<String>,
    pub tone_keywords: Vec<String>,
}

pub fn execute(ctx: &RunnerContext<'_>) -> Adm4Result<StageStatus> {
    let spec: GameSpec = ctx.store.read_contract("C0")?;
    let contract = StyleBriefContract {
        experience_promise: spec.intent.experience_promise.clone(),
        genre_structure: spec.intent.genre_structure.clone(),
        visual_subjects: spec
            .entities
            .iter()
            .filter(|entity| entity.visual_form.is_some())
            .map(|entity| entity.name.clone())
            .collect(),
        tone_keywords: spec.intent.profile.values().cloned().collect(),
    };
    let document = format!(
        "# C5 美术方向简报\n\n- 体验承诺：{}\n- 品类结构：{}\n- 视觉主体：{}\n\n**等待人工确认风格方向后方可进入 C6。**\n\n> 本文档由 contract.json 渲染，请勿手改。\n",
        contract.experience_promise,
        contract.genre_structure,
        contract.visual_subjects.join("、")
    );
    ctx.store.write_stage("C5", &contract, &document)?;
    Ok(StageStatus::WaitingHuman {
        gate: "style_confirmation".into(),
    })
}
