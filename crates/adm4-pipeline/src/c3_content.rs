use crate::framework::StageStatus;
use crate::runner::RunnerContext;
use adm4_ai::AiRequest;
use adm4_contracts::{
    AnchoredNarrative, CardinalityDeclaration, CardinalityRange, SpecRef, UnclassifiedItem,
};
use adm4_foundation::{Adm4Error, Adm4Result};
use adm4_spec::{GameSpec, VisualForm};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// C3 基数申报对账所依据的品类包期望键。
const ASSET_CARDINALITY_KEY: &str = "asset_specs";

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AssetSpec {
    pub id: String,
    pub entity_id: String,
    pub asset_kind: String, // "sprite2d" | "model3d"
    /// 画面描述（AI 生成，锚定实体；不允许空占位）。
    pub description: AnchoredNarrative,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct UiSpecEntry {
    pub id: String,
    pub entity_id: String,
    pub purpose: String,
}

/// C3 契约：内容盘点 + 资产需求（视觉白名单 R2 + 基数申报 R6）。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ContentInventoryContract {
    pub assets: Vec<AssetSpec>,
    pub ui_entries: Vec<UiSpecEntry>,
    pub non_visual_entities: Vec<String>,
    pub cardinality: CardinalityDeclaration,
}

pub fn execute(ctx: &RunnerContext<'_>) -> Adm4Result<StageStatus> {
    let spec: GameSpec = ctx.store.read_contract("C0")?;

    // 视觉形态白名单：只有显式声明 visual_form 的实体产美术资产；未声明 → R2 阻塞。
    let mut unknown: Vec<UnclassifiedItem> = Vec::new();
    let mut assets = Vec::new();
    let mut ui_entries = Vec::new();
    let mut non_visual = Vec::new();
    for entity in &spec.entities {
        match &entity.visual_form {
            None => unknown.push(UnclassifiedItem {
                item: entity.id.clone(),
                reason: "实体未声明视觉形态，不能猜测是否产美术资产".into(),
            }),
            Some(VisualForm::Invisible) => non_visual.push(entity.id.clone()),
            Some(VisualForm::UiOnly) => ui_entries.push(UiSpecEntry {
                id: format!("ui_{}", entity.id),
                entity_id: entity.id.clone(),
                purpose: format!("{} 的界面呈现", entity.name),
            }),
            Some(form @ (VisualForm::Sprite2d | VisualForm::Model3d)) => {
                let anchor = SpecRef::new(format!("entities/{}", entity.id));
                let description = describe_asset(ctx, &spec, entity, anchor)?;
                assets.push(AssetSpec {
                    id: format!("asset_{}", entity.id),
                    entity_id: entity.id.clone(),
                    asset_kind: match form {
                        VisualForm::Sprite2d => "sprite2d".into(),
                        _ => "model3d".into(),
                    },
                    description,
                });
            }
        }
    }
    if !unknown.is_empty() {
        let detail: Vec<String> = unknown
            .iter()
            .map(|item| format!("{}（{}）", item.item, item.reason))
            .collect();
        return Err(Adm4Error::blocked(format!(
            "R2: C3 视觉白名单阻塞 {} 项：{}",
            unknown.len(),
            detail.join("; ")
        )));
    }

    // 基数申报（R6）：对照品类包期望；超界 → 人工确认门。
    let expected = asset_cardinality_expectation(
        &ctx.space.pack.pack_id,
        &ctx.space.pack.cardinality_expectations,
    )?;
    let cardinality = CardinalityDeclaration {
        rule: "每个可见实体（sprite2d/model3d）派生 1 条美术资产需求；ui_only 进 UI 清单；invisible 不产资产".into(),
        produced: assets.len(),
        expected,
        dropped: non_visual
            .iter()
            .map(|entity_id| adm4_contracts::DroppedItem {
                item: entity_id.clone(),
                reason: "visual_form=invisible".into(),
            })
            .collect(),
    };

    let contract = ContentInventoryContract {
        assets: assets.clone(),
        ui_entries,
        non_visual_entities: non_visual,
        cardinality: cardinality.clone(),
    };
    let mut document = format!(
        "# C3 内容与资产需求\n\n- 美术资产需求：{} 条\n- UI 清单：{} 条\n- 非视觉实体：{} 个\n- 基数申报：{}（期望 {}..{}）\n\n",
        contract.assets.len(),
        contract.ui_entries.len(),
        contract.non_visual_entities.len(),
        cardinality.produced,
        cardinality.expected.min,
        cardinality.expected.max
    );
    for asset in &assets {
        document.push_str(&format!(
            "## {}\n\n- 实体：`{}`\n- 形态：{}\n- 画面描述：{}\n\n",
            asset.id, asset.entity_id, asset.asset_kind, asset.description.text
        ));
    }
    document.push_str("> 本文档由 contract.json 渲染，请勿手改。\n");
    ctx.store.write_stage("C3", &contract, &document)?;

    if !cardinality.within_expectation() {
        // 超界不 fail：等待人工确认（block 到确认为止）。
        return Ok(StageStatus::WaitingHuman {
            gate: format!(
                "cardinality_confirm（产出 {} 超出期望 {}..{}）",
                cardinality.produced, cardinality.expected.min, cardinality.expected.max
            ),
        });
    }
    Ok(StageStatus::Succeeded)
}

/// 取品类包的美术资产基数期望。
///
/// 缺键即阻塞：没有期望区间就无法对账，若退回 `0..usize::MAX` 兜底，R6 的基数门
/// 会在整条品类包上永久失效（路径级红线失效），故按 R2「未知即停」显式报错。
fn asset_cardinality_expectation(
    pack_id: &str,
    expectations: &BTreeMap<String, CardinalityRange>,
) -> Adm4Result<CardinalityRange> {
    expectations
        .get(ASSET_CARDINALITY_KEY)
        .copied()
        .ok_or_else(|| {
            Adm4Error::blocked(format!(
                "R6: 品类包 {pack_id} 的 cardinality_expectations 缺少 {ASSET_CARDINALITY_KEY} 键，\
                 C3 无法对账美术资产基数（不允许用 0..无上限 默认区间放行，请在 pack.json 补齐该键）"
            ))
        })
}

fn describe_asset(
    ctx: &RunnerContext<'_>,
    _spec: &GameSpec,
    entity: &adm4_spec::EntitySpec,
    anchor: SpecRef,
) -> Adm4Result<AnchoredNarrative> {
    let request = AiRequest {
        purpose: "c3_asset_description".into(),
        system_prompt: "你是美术需求撰写者。基于实体信息写一段供画师使用的画面描述，\
                        不得提及任何真实游戏名。输出 JSON：{\"description\": ...}。"
            .into(),
        user_prompt: format!(
            "实体：{}（{}），属性：{}",
            entity.name,
            entity.id,
            entity
                .properties
                .iter()
                .map(|property| property.key.clone())
                .collect::<Vec<_>>()
                .join(", ")
        ),
        expect_json: true,
    };
    let response = ctx.ai.invoke(&request)?;
    let value: serde_json::Value = serde_json::from_str(response.text.trim())
        .map_err(|error| Adm4Error::validation(format!("C3 画面描述不是合法 JSON：{error}")))?;
    let text = value
        .get("description")
        .and_then(|description| description.as_str())
        .ok_or_else(|| Adm4Error::validation("C3 画面描述缺少 description 字段"))?;
    AnchoredNarrative::new(text, vec![anchor])
}

#[cfg(test)]
mod tests {
    use super::*;
    use adm4_foundation::Adm4ErrorKind;

    #[test]
    fn missing_asset_expectation_blocks_instead_of_defaulting() {
        let error = asset_cardinality_expectation("grid_strategy", &BTreeMap::new()).unwrap_err();
        assert_eq!(error.kind, Adm4ErrorKind::Blocked);
        assert!(error.message.contains("asset_specs"), "{}", error.message);
    }

    #[test]
    fn present_asset_expectation_is_returned() {
        let expectations = BTreeMap::from([(
            ASSET_CARDINALITY_KEY.to_string(),
            CardinalityRange { min: 5, max: 14 },
        )]);
        let range = asset_cardinality_expectation("lane_defense", &expectations).unwrap();
        assert_eq!(range, CardinalityRange { min: 5, max: 14 });
    }
}
