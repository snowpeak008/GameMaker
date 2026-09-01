//! 资产生产通道（册 08 §4.1，D19 可插拔）：资产是「可执行的生成命令 + 成本 + 路径 +
//! 尺寸 + 使用者记录」，不是一段文字需求。
//!
//! 通道选择按 `can_produce`：都不接 → Blocked，**不产占位资产**（R2）。
//! 提示词以应用契约的 `prompt_prefix` 起头（风格一致性的实际抓手）并过换皮扫描（R5）。

use crate::governance::art_line::ArtAsset;
use crate::governance::asset_registry::AssetRegistryEntry;
use adm4_ai::{ImageProvider, ImageRequest};
use adm4_contracts::SkinScanner;
use adm4_foundation::{Adm4Error, Adm4Result};
use serde::{Deserialize, Serialize};

use super::style_anchor::{StyleApplicationContract, StyleUsage};

/// 一次生产请求：资产规格 + 命名权威登记（文件名与运行时路径由它说了算）。
pub struct ProduceRequest<'a> {
    pub asset: &'a ArtAsset,
    pub registered: &'a AssetRegistryEntry,
    pub contract: &'a StyleApplicationContract,
}

/// 生产结果：字节 + 元数据（落盘与回填由调用方做——通道只管生成，不管产物仓）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProducedAsset {
    pub asset_id: String,
    pub bytes: Vec<u8>,
    /// 字节头嗅探的实际格式（不是配置声称的格式）。
    pub media_type: String,
    /// 生成它的完整提示词（进台账，一致性比对与重生成都要它）。
    pub prompt: String,
    pub provider_id: String,
    pub model: String,
}

/// 资产生产通道（插件接缝，D19/D24）。
pub trait AssetProducer {
    /// 通道标识（`ai` / `external:<tool>`）。
    fn id(&self) -> &str;
    /// 本通道能否生产该资产。
    fn can_produce(&self, asset: &ArtAsset) -> bool;
    /// 生产。失败即 `Err`（R7：无降级、无占位兜底）。
    fn produce(&self, request: &ProduceRequest<'_>) -> Adm4Result<ProducedAsset>;
}

/// 资产大类 → 应用契约的用途约束（tile/icon/ui/background/effect 五类）。
///
/// 映射是保守的：立绘/模型按 icon（近景可读性最严），UI 按 ui。没有「猜不出就跳过约束」
/// 的分支——五类约束在 G2 已被校验为全覆盖，这里必然取得到。
fn usage_for(asset: &ArtAsset) -> StyleUsage {
    use crate::governance::art_line::AssetCategory;
    match asset.category {
        AssetCategory::Ui => StyleUsage::Ui,
        AssetCategory::Vfx => StyleUsage::Effect,
        AssetCategory::Illustration | AssetCategory::Model | AssetCategory::Animation => {
            StyleUsage::Icon
        }
        AssetCategory::Audio | AssetCategory::Unknown => StyleUsage::Icon,
    }
}

/// 拼装该资产的生成提示词：`prompt_prefix`（风格） + 资产语义 + 用途约束。
///
/// 公开成自由函数：缓存键要用同一份提示词（提示词变了缓存必须失效），
/// 两处各拼一遍迟早对不上。
pub fn build_prompt(request: &ProduceRequest<'_>) -> Adm4Result<String> {
    let asset = request.asset;
    // 资产语义出处：真源锚点的 art_rule（C3 的锚定描述）优先，其次 purpose。
    // 两者都空在 ArtContract::validate 已被拦（无出处资产），这里按序取即可。
    let semantic = if asset.art_rule.trim().is_empty() {
        asset.purpose.trim()
    } else {
        asset.art_rule.trim()
    };
    if semantic.is_empty() {
        return Err(Adm4Error::validation(format!(
            "资产 {} 既无描述也无用途：没有语义就没有提示词可拼（R2）",
            asset.asset_id
        )));
    }
    let usage = usage_for(asset);
    let constraint = request.contract.constraint(usage).ok_or_else(|| {
        Adm4Error::validation(format!(
            "应用契约缺 {} 用途的风格约束：G2 契约校验要求五类全覆盖，这份契约不完整",
            usage.label_zh()
        ))
    })?;
    Ok(format!(
        "{prefix} {semantic}。可读性：{readability}；对比度：{contrast}；边距：{margin}。单一主体、无文字、无水印。",
        prefix = request.contract.prompt_prefix.trim(),
        readability = constraint.readability,
        contrast = constraint.contrast,
        margin = constraint.transparent_margin,
    ))
}

/// AI 图像生产通道（本期唯一真通道）。
pub struct AiImageAssetProducer<'a> {
    images: &'a dyn ImageProvider,
    scanner: &'a SkinScanner,
    /// 生成尺寸（来自图像通道配置；三要素的申报尺寸与它不一致时如实进比对报告）。
    pub width: u32,
    pub height: u32,
}

impl<'a> AiImageAssetProducer<'a> {
    pub fn new(
        images: &'a dyn ImageProvider,
        scanner: &'a SkinScanner,
        width: u32,
        height: u32,
    ) -> Self {
        Self {
            images,
            scanner,
            width,
            height,
        }
    }
}

impl AssetProducer for AiImageAssetProducer<'_> {
    fn id(&self) -> &str {
        "ai"
    }

    /// 视觉白名单（R2）：只接**图像可表达**的资产。音频没有图像通道可走，
    /// Unknown 是坏数据——都不接，让通道选择落到「无通道 → Blocked」。
    fn can_produce(&self, asset: &ArtAsset) -> bool {
        use crate::governance::art_line::AssetCategory;
        matches!(
            asset.category,
            AssetCategory::Illustration
                | AssetCategory::Ui
                | AssetCategory::Vfx
                | AssetCategory::Animation
                | AssetCategory::Model
        )
    }

    fn produce(&self, request: &ProduceRequest<'_>) -> Adm4Result<ProducedAsset> {
        let prompt = build_prompt(request)?;
        // R5：提示词与资产名都不得携带外部参考名（豁免口径由调用方装配扫描器时定）。
        let mut hits = self
            .scanner
            .scan(&format!("produce/{}", request.asset.asset_id), &prompt);
        hits.extend(self.scanner.scan(
            &format!("produce/{}/asset_id", request.asset.asset_id),
            &request.asset.asset_id,
        ));
        if !hits.is_empty() {
            let detail: Vec<String> = hits
                .iter()
                .map(|hit| format!("{} 命中 {}", hit.location, hit.matched_word))
                .collect();
            return Err(Adm4Error::red_line(format!(
                "R5：资产 {} 的生成输入命中参考名（{} 处）：{}",
                request.asset.asset_id,
                hits.len(),
                detail.join("; ")
            )));
        }
        let artifact = self.images.generate(&ImageRequest {
            purpose: format!("asset_production/{}", request.asset.asset_id),
            prompt: prompt.clone(),
            width: self.width,
            height: self.height,
        })?;
        Ok(ProducedAsset {
            asset_id: request.asset.asset_id.clone(),
            bytes: artifact.bytes,
            media_type: artifact.media_type,
            prompt,
            provider_id: artifact.provider_id,
            model: artifact.model,
        })
    }
}

/// 外部工具通道（D19 接缝，本期占位）：调用即诚实报未配置（R7，不装作能产）。
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct ExternalToolProducer {
    /// 工具名（配置了才有；空 = 未配置）。
    pub tool: String,
}

impl AssetProducer for ExternalToolProducer {
    fn id(&self) -> &str {
        "external"
    }

    /// 未配置的通道什么都不接——接了也产不出来，不如让选择器落到真通道或如实 Blocked。
    fn can_produce(&self, _asset: &ArtAsset) -> bool {
        false
    }

    fn produce(&self, request: &ProduceRequest<'_>) -> Adm4Result<ProducedAsset> {
        Err(Adm4Error::blocked(format!(
            "外部工具通道未配置（D19 接缝，本期只留 trait）：资产 {} 请走 AI 通道或等外部通道接入",
            request.asset.asset_id
        )))
    }
}

/// 从注册的通道里为资产选一条能产的（按注册顺序，先到先得）。
///
/// 都不接 → Blocked 并点名资产与其大类：**不产占位资产**（R2），也不静默跳过——
/// 跳过的资产会让基数对账（R6）如实把账算破，那正是要人看见的。
pub fn select_producer<'a>(
    producers: &'a [&'a dyn AssetProducer],
    asset: &ArtAsset,
) -> Adm4Result<&'a dyn AssetProducer> {
    producers
        .iter()
        .find(|producer| producer.can_produce(asset))
        .copied()
        .ok_or_else(|| {
            Adm4Error::blocked(format!(
                "资产 {}（{:?}）没有任何生产通道能接：不产占位资产（R2），请配置对应通道",
                asset.asset_id, asset.category
            ))
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::art::style_anchor::{
        STYLE_SCHEMA_VERSION, StyleAnchorImage, StyleAnchorSet, StyleApprovalStatus,
        StyleConfirmMode, StyleConfirmation,
    };
    use crate::governance::SpecTriple;
    use crate::governance::art_line::AssetCategory;
    use crate::governance::asset_registry::{AssetLifecycleState, StabilityLevel};
    use adm4_ai::ScriptedImageProvider;
    use adm4_contracts::SpecRef;

    fn anchor_set() -> StyleAnchorSet {
        StyleAnchorSet {
            schema_version: STYLE_SCHEMA_VERSION.into(),
            anchor_version: 1,
            generated_at: "2026-08-31T00:00:00Z".into(),
            project_name: "演示项目".into(),
            genre_pack: "lane_defense".into(),
            source_revision: 7,
            source_anchors: vec![SpecRef::new("profile/u.genre")],
            selected_style_id: "STYLE-01-readable_production".into(),
            selected_title: "清晰量产".into(),
            preset_key: "readable_production".into(),
            final_prompt: "clean readable production style".into(),
            prompt_overridden: false,
            palette: vec!["#223344".into(), "#aabbcc".into(), "#ffeedd".into()],
            anchors: vec![StyleAnchorImage {
                anchor_id: "ANCHOR-STYLE-01-readable_production-selected_preview".into(),
                role: "selected_preview".into(),
                image_path: "anchors/v1/STYLE-01-readable_production.png".into(),
                image_sha256: "deadbeef".into(),
                image_bytes: 128,
                media_type: "image/png".into(),
                requested_width: 1024,
                requested_height: 1024,
                prompt: "clean readable production style".into(),
                provider_id: "scripted_image".into(),
                model: "scripted".into(),
            }],
            confirmation: StyleConfirmation {
                status: StyleApprovalStatus::Approved,
                mode: StyleConfirmMode::Manual,
                selected_style_id: "STYLE-01-readable_production".into(),
                selected_title: "清晰量产".into(),
                selected_image_path: "anchors/v1/STYLE-01-readable_production.png".into(),
                notes: "结论".into(),
                actor: "主美甲".into(),
                at: "2026-08-31T00:00:00Z".into(),
                anchor_version: 1,
            },
        }
    }

    fn contract() -> StyleApplicationContract {
        StyleApplicationContract::derive(&anchor_set(), "2026-08-31T00:00:00Z").expect("派生契约")
    }

    fn asset(asset_id: &str, category: AssetCategory) -> ArtAsset {
        ArtAsset {
            asset_id: asset_id.into(),
            name: "守卫".into(),
            category,
            purpose: "守卫的游戏内呈现".into(),
            production_spec: SpecTriple::full(
                1,
                crate::governance::AssetSize::new(1024, 1024),
                "png",
            ),
            naming_pattern: format!("{}.png", asset_id.to_ascii_lowercase()),
            required_readability: "轮廓可辨".into(),
            forbidden_visuals: Vec::new(),
            acceptance_checks: Vec::new(),
            source_refs: vec![SpecRef::new("entities/guard")],
            art_rule: "扁平卡通守卫立绘".into(),
        }
    }

    fn registered(asset_id: &str) -> AssetRegistryEntry {
        AssetRegistryEntry {
            asset_id: asset_id.into(),
            naming_pattern: format!("{}.png", asset_id.to_ascii_lowercase()),
            runtime_path: format!("GameAssets/{}.png", asset_id.to_ascii_lowercase()),
            state: AssetLifecycleState::Draft,
            stability: StabilityLevel::Experimental,
            source_refs: Vec::new(),
        }
    }

    /// 提示词以契约前缀起头 + 带用途约束；AI 通道真出图且元数据齐全。
    #[test]
    fn ai_producer_prefixes_prompt_and_produces_bytes() {
        let images = ScriptedImageProvider::new();
        let scanner = SkinScanner::default();
        let producer = AiImageAssetProducer::new(&images, &scanner, 1024, 1024);
        let asset = asset("T_Guard", AssetCategory::Illustration);
        let entry = registered("T_Guard");
        let contract = contract();
        let request = ProduceRequest {
            asset: &asset,
            registered: &entry,
            contract: &contract,
        };
        let prompt = build_prompt(&request).expect("拼提示词");
        assert!(
            prompt.starts_with(contract.prompt_prefix.trim()),
            "提示词必须以应用契约前缀起头：{prompt}"
        );
        assert!(prompt.contains("扁平卡通守卫立绘"), "{prompt}");

        let produced = producer.produce(&request).expect("生产");
        assert_eq!(produced.asset_id, "T_Guard");
        assert!(!produced.bytes.is_empty());
        assert_eq!(produced.media_type, "image/png");
        assert_eq!(produced.prompt, prompt, "台账记录的必须是实际用的提示词");
        assert_eq!(images.calls().len(), 1);
    }

    /// R5：提示词命中外部参考名即拒，一次图像调用都不发。
    #[test]
    fn skin_hit_in_prompt_blocks_before_any_image_call() {
        let images = ScriptedImageProvider::new();
        let scanner = SkinScanner::new(vec!["Kingdom Rush".into()]);
        let producer = AiImageAssetProducer::new(&images, &scanner, 1024, 1024);
        let mut tainted = asset("T_Guard", AssetCategory::Illustration);
        tainted.art_rule = "照 Kingdom Rush 的守卫画".into();
        let entry = registered("T_Guard");
        let contract = contract();
        let error = producer
            .produce(&ProduceRequest {
                asset: &tainted,
                registered: &entry,
                contract: &contract,
            })
            .expect_err("换皮命中必须拒");
        assert_eq!(error.kind, adm4_foundation::Adm4ErrorKind::RedLine);
        assert!(images.calls().is_empty(), "被拦的生产不许花一次调用");
    }

    /// 通道选择：音频类没人接 → Blocked 点名；外部通道占位诚实不接。
    #[test]
    fn audio_has_no_channel_and_external_is_honestly_unconfigured() {
        let images = ScriptedImageProvider::new();
        let scanner = SkinScanner::default();
        let ai = AiImageAssetProducer::new(&images, &scanner, 1024, 1024);
        let external = ExternalToolProducer::default();
        let producers: Vec<&dyn AssetProducer> = vec![&ai, &external];

        let audio = asset("VFX_Boom", AssetCategory::Audio);
        let error = match select_producer(&producers, &audio) {
            Ok(_) => panic!("音频不该有通道接"),
            Err(error) => error,
        };
        assert_eq!(error.kind, adm4_foundation::Adm4ErrorKind::Blocked);
        assert!(error.message.contains("VFX_Boom"), "{}", error.message);

        let visual = asset("T_Guard", AssetCategory::Illustration);
        let chosen = match select_producer(&producers, &visual) {
            Ok(producer) => producer,
            Err(error) => panic!("视觉资产该走 AI 通道：{}", error.message),
        };
        assert_eq!(chosen.id(), "ai");

        // 外部通道被直接调用时诚实报未配置。
        let entry = registered("T_Guard");
        let contract = contract();
        let error = external
            .produce(&ProduceRequest {
                asset: &visual,
                registered: &entry,
                contract: &contract,
            })
            .expect_err("未配置的外部通道必须 Blocked");
        assert_eq!(error.kind, adm4_foundation::Adm4ErrorKind::Blocked);
    }

    /// 生成失败原样上抛（R7）：不重试、不占位。
    #[test]
    fn image_failure_surfaces_as_is() {
        let images = ScriptedImageProvider::new();
        images.fail_with("图像 API 欠费");
        let scanner = SkinScanner::default();
        let producer = AiImageAssetProducer::new(&images, &scanner, 1024, 1024);
        let asset = asset("T_Guard", AssetCategory::Illustration);
        let entry = registered("T_Guard");
        let contract = contract();
        let error = producer
            .produce(&ProduceRequest {
                asset: &asset,
                registered: &entry,
                contract: &contract,
            })
            .expect_err("失败就是失败");
        assert!(error.message.contains("欠费"), "{}", error.message);
    }
}
