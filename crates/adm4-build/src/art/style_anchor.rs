//! 设计阶段的美术风格锚点门（册 08 §2-3，选项 A）。
//!
//! # 这道门解决什么
//!
//! 风格是主观口味：只有人看着真图才能定。等到资产批量生产完才发现风格不对，返工代价
//! 最大。所以册 08 把这道门放在**冻结之前**——用户在设计工作台里看图、改词、反复重出图、
//! 署名确认，锁定 `style_anchor_set`；Phase 2 的资产生产只消费它，不重造风格。
//!
//! # 三条边界
//!
//! - **锚点集是风格的唯一真相**（D22）：[`StyleApplicationContract`] 是它的**投影**
//!   （带 `source_anchor_hash` 回指），下游只读契约、不自行解释锚图；
//! - **提示词必须锚定真源**（R4）：提示词由创作态已确认的画像决策点派生，
//!   无锚点直接 `Err`（[`StyleSourceFacts::validate`]），不许凭空编一段风格描述；
//! - **确认必须 attended**（R3）：[`StyleGate::confirm`] 署名与结论双必填，
//!   而且类型层面压根没有 auto_accept 这个取值（[`StyleConfirmMode`] 只有 `Manual`）。
//!
//! # 不可变历史
//!
//! 每次确认写一版 `anchors/v{N}/`，旧版**不改不删**（D4）。重选风格 = 新版本，
//! 因此「这一版游戏当时锁的什么风格」永远查得到；确认之前的旧版仍是下游的权威依据。

use adm4_ai::{ImageProvider, ImageRequest, media_type_extension};
use adm4_contracts::{AnchoredNarrative, SkinScanner, SpecRef};
use adm4_foundation::{
    Adm4Error, Adm4Result, ContentHash, atomic_write, ensure_dir, read_json_file, sha256_hex,
    write_json_file,
};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// 风格契约的 schema 版本（锚点集 / 应用契约 / 适配报告同进同退）。
pub const STYLE_SCHEMA_VERSION: &str = "4.0.0";

/// 风格门产物在存档内容树里的子目录名（存档兼容：不得更名）。
pub const STYLE_SECTION: &str = "style";

/// 风格方向 id 前缀（`STYLE-{NN}-{preset_key}`）。
pub const STYLE_ID_PREFIX: &str = "STYLE-";

/// 锚图 id 前缀（`ANCHOR-{style_id}-{role}`）。
pub const ANCHOR_ID_PREFIX: &str = "ANCHOR-";

/// 图像调用意图（进生成记录与运行日志）。
pub const STYLE_PREVIEW_PURPOSE: &str = "style_anchor_preview";

/// 册 08 §3 的阻断码：风格应用契约未获批准时，下游美术任务一律不得开跑。
pub const STYLE_APPLICATION_CONTRACT_NOT_APPROVED: &str = "STYLE_APPLICATION_CONTRACT_NOT_APPROVED";

/// 方向数下限（册 08 §2.2：N∈[3,5]）。
pub const MIN_DIRECTIONS: usize = 3;
/// 方向数上限。
pub const MAX_DIRECTIONS: usize = 5;

/// 从序号与预设键派生方向 id（册 08 §2.5 的命名：`STYLE-01-readable_production`）。
pub fn style_direction_id(index: usize, preset_key: &str) -> String {
    format!("{STYLE_ID_PREFIX}{:02}-{preset_key}", index + 1)
}

/// 从方向 id 与用途派生锚图 id。
pub fn style_anchor_image_id(style_id: &str, role: &str) -> String {
    format!("{ANCHOR_ID_PREFIX}{style_id}-{role}")
}

// ---------------------------------------------------------------------------
// 风格预设表（册 08 §2.2 的五个方向，各带三色 palette）
// ---------------------------------------------------------------------------

/// 预设的信息密度代价：越高越吃可读性，是[适配报告](StyleFitReport)的判定依据。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DensityCost {
    #[default]
    Low,
    Medium,
    High,
}

/// 一个风格预设：方向的骨架（中文说明 + 英文提示词关键词 + 三色 palette + 约束基调）。
///
/// 预设是**代码里的常量表**而不是数据文件：它不是设计事实（设计事实在真源里），
/// 而是「把真源翻译成图像提示词」的翻译规则。规则变了要过代码评审，不该让人在
/// 清单里随手改一行就改掉全项目的风格候选。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StylePreset {
    pub preset_key: &'static str,
    /// 中文方向名（界面上的卡片标题）。
    pub title: &'static str,
    /// 英文提示词关键词（拼进最终提示词）。
    pub prompt_keywords: &'static str,
    /// 方向意图（中文，进方向说明）。
    pub intent: &'static str,
    pub palette: [&'static str; 3],
    /// 可读性基调（进 [`StyleConstraint::readability`]）。
    pub readability: &'static str,
    /// 对比度基调（进 [`StyleConstraint::contrast`]）。
    pub contrast: &'static str,
    pub density_cost: DensityCost,
}

/// 册 08 §2.2 的五个风格预设。
pub fn style_presets() -> Vec<StylePreset> {
    vec![
        StylePreset {
            preset_key: "readable_production",
            title: "清晰量产",
            prompt_keywords: "flat clean vector-like game art, bold silhouettes, minimal inner detail, even lighting, production-friendly",
            intent: "以可读性与量产效率优先：轮廓清晰、内部细节克制，同一套规则能覆盖大量资产",
            palette: ["#2F4858", "#F6C445", "#EFEFEF"],
            readability: "缩至 25% 仍能靠轮廓认出主体",
            contrast: "主体与背景明度差不低于 40%",
            density_cost: DensityCost::Low,
        },
        StylePreset {
            preset_key: "concept_painting",
            title: "概念绘画",
            prompt_keywords: "painterly concept art, soft brushwork, atmospheric depth, layered value composition",
            intent: "以氛围与质感优先：厚涂笔触与空气感营造沉浸，单张表现力强",
            palette: ["#3B2F4A", "#C07A4B", "#DCD3C3"],
            readability: "近景可读；远景需靠明度分层保证主体不被背景吞掉",
            contrast: "靠明度分层而非描边，需人工核对主体是否跳出",
            density_cost: DensityCost::High,
        },
        StylePreset {
            preset_key: "high_contrast_arcade",
            title: "高对比街机",
            prompt_keywords: "high contrast arcade game art, saturated accent colors, thick outlines, punchy shapes",
            intent: "以瞬时辨识优先：粗描边与高饱和强调色，快节奏下一眼分清敌我",
            palette: ["#101820", "#FF4E50", "#00E0C6"],
            readability: "高速移动中仍可辨识；小尺寸下描边可能糊成一团，需单独核对图标",
            contrast: "描边 + 高饱和强调色，主体与背景明度差不低于 55%",
            density_cost: DensityCost::Medium,
        },
        StylePreset {
            preset_key: "cinematic_realism",
            title: "电影写实",
            prompt_keywords: "cinematic semi-realistic game art, physically plausible materials, dramatic key light, shallow depth of field",
            intent: "以真实感与镜头语言优先：材质与光照可信，适合叙事与特写",
            palette: ["#1C2321", "#7D8CA3", "#D8C3A5"],
            readability: "写实材质与浅景深会压低远景可读性，高信息密度玩法下风险最大",
            contrast: "依赖主光与景深分离主体，需逐场景核对",
            density_cost: DensityCost::High,
        },
        StylePreset {
            preset_key: "stylized_diagram",
            title: "风格化图示",
            prompt_keywords: "stylized diagrammatic game art, geometric shapes, iconographic clarity, flat color blocking",
            intent: "以规则可视化优先：几何化与图示化让机制本身一眼可读",
            palette: ["#22333B", "#5E8B7E", "#F2F4F3"],
            readability: "任意尺寸下都可读；代价是情绪表达弱",
            contrast: "纯色块分区，色相差即区分度",
            density_cost: DensityCost::Low,
        },
    ]
}

// ---------------------------------------------------------------------------
// 真源事实投影（R4 的锚）
// ---------------------------------------------------------------------------

/// 真源里的一条事实：一个已确认的画像决策点及其已选选项。
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct StyleSourceFact {
    pub decision_id: String,
    pub question: String,
    /// 已确认的选项标签（多选点可多条，主选在前）。
    pub option_labels: Vec<String>,
}

/// 风格门的真源事实投影：**由调用方（`adm4-app`）从创作态画像现取**，不落独立存储。
///
/// 为什么是投影而不是一份新状态（D22）：风格门在冻结之前跑，此时还没有 `GameSpec`，
/// 权威真源是创作态里**已确认**的画像决策点（册 08 §2.1 的「L0-L2 画像」）。
/// 本结构只是那份状态的一次只读快照，随生成落盘的是它的**锚点与摘要**（可追溯到
/// 具体决策点），而不是另一棵可编辑的状态树。
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct StyleSourceFacts {
    pub project_name: String,
    pub genre_pack: String,
    /// 创作态 revision：随锚点集落盘，用来判断「锚定的设计后来变了没有」。
    pub source_revision: u64,
    pub entries: Vec<StyleSourceFact>,
}

impl StyleSourceFacts {
    /// R4 的机器化：没有任何已确认的画像事实 → 直接 `Err`。
    ///
    /// 这是本模块最重要的一条拒绝：允许「无锚生成」等于让 AI 凭空编一套风格方向，
    /// 而它随后会被锁成全项目美术的唯一依据。缺就说缺，让人回工作台把画像点确认掉。
    pub fn validate(&self) -> Adm4Result<()> {
        if self.project_name.trim().is_empty() {
            return Err(Adm4Error::validation("风格门真源缺项目名"));
        }
        if self.genre_pack.trim().is_empty() {
            return Err(Adm4Error::validation("风格门真源缺品类包标识"));
        }
        let usable = self
            .entries
            .iter()
            .filter(|entry| {
                !entry.decision_id.trim().is_empty()
                    && entry
                        .option_labels
                        .iter()
                        .any(|label| !label.trim().is_empty())
            })
            .count();
        if usable == 0 {
            return Err(Adm4Error::red_line(
                "R4：没有任何已确认的画像决策点可作为风格提示词的锚点。\
                 风格方向必须派生自真源（品类/平台/体验/美术风格定位等已确认的点），\
                 请先在设计工作台确认这些点，再来生成风格方向",
            ));
        }
        Ok(())
    }

    /// 真源锚点（`profile/<decision_id>`），进 [`AnchoredNarrative`] 与锚点集。
    pub fn anchors(&self) -> Vec<SpecRef> {
        self.entries
            .iter()
            .filter(|entry| !entry.decision_id.trim().is_empty())
            .map(|entry| SpecRef::new(format!("profile/{}", entry.decision_id)))
            .collect()
    }

    /// 人读摘要（每条事实一行）：报告与界面据此说明「这批提示词是从哪派生的」。
    pub fn summary_lines(&self) -> Vec<String> {
        self.entries
            .iter()
            .filter(|entry| !entry.option_labels.is_empty())
            .map(|entry| {
                format!(
                    "{}｜{}：{}",
                    entry.decision_id,
                    entry.question,
                    entry.option_labels.join(" / ")
                )
            })
            .collect()
    }

    /// 提示词里的「设计意图」片段：已确认选项标签的有序拼接。
    ///
    /// 只拼**真源里的原词**，不做任何改写或润色——改写就是在提示词里发明设计。
    pub fn intent_text(&self) -> String {
        let mut labels: Vec<&str> = Vec::new();
        for entry in &self.entries {
            for label in &entry.option_labels {
                let trimmed = label.trim();
                if !trimmed.is_empty() && !labels.contains(&trimmed) {
                    labels.push(trimmed);
                }
            }
        }
        labels.join(", ")
    }
}

// ---------------------------------------------------------------------------
// 生成选项
// ---------------------------------------------------------------------------

/// 一次风格方向生成的参数。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct StyleGenerationOptions {
    /// 方向数（册 08 §2.2：N∈[3,5]）。
    pub direction_count: usize,
    pub preview_width: u32,
    pub preview_height: u32,
    /// true = 推翻现有方向重新派生并清空全部预览（册 08 §2.3「清掉未选图」）；
    /// false = 断点续跑，只给还没出图的方向补图。
    pub force: bool,
}

impl Default for StyleGenerationOptions {
    fn default() -> Self {
        Self {
            direction_count: MAX_DIRECTIONS,
            preview_width: 512,
            preview_height: 512,
            force: false,
        }
    }
}

impl StyleGenerationOptions {
    pub fn validate(&self) -> Adm4Result<()> {
        if !(MIN_DIRECTIONS..=MAX_DIRECTIONS).contains(&self.direction_count) {
            return Err(Adm4Error::invalid_input(format!(
                "风格方向数 {} 超出 [{MIN_DIRECTIONS}, {MAX_DIRECTIONS}]（册 08 §2.2）",
                self.direction_count
            )));
        }
        if self.preview_width == 0 || self.preview_height == 0 {
            return Err(Adm4Error::invalid_input(format!(
                "预览图尺寸非法（{}x{}）：宽高都必须为正",
                self.preview_width, self.preview_height
            )));
        }
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// 工作态：方向候选 + 生成轮次记录
// ---------------------------------------------------------------------------

/// 一张预览图的在案记录（路径相对 `content/style/`）。
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct StylePreview {
    pub image_path: String,
    pub image_sha256: String,
    pub image_bytes: u64,
    pub media_type: String,
    /// **请求**尺寸（本层不解码图像，因此不声称它是实际尺寸）。
    pub requested_width: u32,
    pub requested_height: u32,
    pub provider_id: String,
    pub model: String,
    pub generated_at: String,
    /// 产出这张图的那一轮生成记录 id。
    pub round_id: String,
}

/// 一个风格方向候选。
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct StyleDirection {
    pub style_id: String,
    pub preset_key: String,
    pub title: String,
    /// 方向说明（中文，锚定真源）。
    pub description: String,
    /// 由真源派生的英文提示词。
    pub derived_prompt: String,
    /// 派生提示词的真源锚点（R4）。
    pub prompt_anchors: Vec<SpecRef>,
    /// 用户提交的改词；非空即**覆盖** `derived_prompt`（册 08 §2.3 对话式改词）。
    pub prompt_override: String,
    pub palette: Vec<String>,
    pub recommended: bool,
    /// 推荐理由（只有 `recommended` 为真时非空）。
    pub recommended_reason: String,
    /// 最近一次预览图；None = 还没出图（或上一轮生成失败）。
    pub preview: Option<StylePreview>,
}

impl StyleDirection {
    /// 实际发给图像通道的提示词：有改词用改词，没有用派生提示词。
    pub fn effective_prompt(&self) -> &str {
        if self.prompt_override.trim().is_empty() {
            &self.derived_prompt
        } else {
            &self.prompt_override
        }
    }

    /// 提示词摘要（界面卡片上的一行）。
    pub fn prompt_summary(&self, limit: usize) -> String {
        let prompt = self.effective_prompt();
        let mut summary: String = prompt.chars().take(limit).collect();
        if prompt.chars().count() > limit {
            summary.push('…');
        }
        summary
    }
}

/// 生成轮次的类型。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StyleRoundKind {
    /// 首次生成/推翻重生成（整批方向）。
    #[default]
    Initial,
    /// 单方向改词重生成。
    Regenerate,
}

/// 一轮生成里针对某个方向的一条记录。
///
/// **成功与失败都记**（R7）：失败留 `failure` 而不是干脆不写这条——不然「为什么这个方向
/// 一直没有图」就查不出来，用户只能看到一个空卡片。
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct StyleGenerationItem {
    pub style_id: String,
    /// 本轮真正发出去的提示词。
    pub prompt: String,
    /// 本轮生效的改词（空 = 用派生提示词）。
    pub prompt_override: String,
    pub requested_width: u32,
    pub requested_height: u32,
    pub provider_id: String,
    pub model: String,
    /// 成功时非空。
    pub image_path: String,
    /// 成功时非空。
    pub image_sha256: String,
    /// 失败时非空（原始错误消息，不改写、不美化）。
    pub failure: String,
}

impl StyleGenerationItem {
    pub fn succeeded(&self) -> bool {
        self.failure.is_empty() && !self.image_sha256.is_empty()
    }
}

/// 一轮生成记录（只追加，不修改）。
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct StyleGenerationRound {
    pub round_id: String,
    pub kind: StyleRoundKind,
    pub at: String,
    pub items: Vec<StyleGenerationItem>,
}

impl StyleGenerationRound {
    /// 本轮失败的条目（非空即本轮不算干净跑完）。
    pub fn failures(&self) -> Vec<&StyleGenerationItem> {
        self.items
            .iter()
            .filter(|item| !item.failure.is_empty())
            .collect()
    }
}

/// 风格门工作态（`content/style/session.json`）：方向候选 + 全部生成轮次。
///
/// 「可停可续」就落在这里：每一轮生成**先落盘再判成败**，因此中途失败/被关窗口之后，
/// 已经出图的方向照旧在案，下一次生成只补缺的那几个（`force=false`）。
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct StyleSession {
    pub schema_version: String,
    pub project_name: String,
    pub genre_pack: String,
    pub source_revision: u64,
    pub source_anchors: Vec<SpecRef>,
    /// 真源事实摘要（人读）。
    pub source_summary: Vec<String>,
    pub preview_width: u32,
    pub preview_height: u32,
    pub directions: Vec<StyleDirection>,
    pub rounds: Vec<StyleGenerationRound>,
    /// 风格-原型适配报告（册 08 §2.2 的 `style_fit`，提示不阻断）。
    pub fit: StyleFitReport,
    /// 本工作态已锁成的锚点版本（0 = 尚未确认，或确认后又开始重新选择）。
    pub confirmed_version: u32,
    pub created_at: String,
    pub updated_at: String,
}

impl StyleSession {
    pub fn direction(&self, style_id: &str) -> Option<&StyleDirection> {
        self.directions
            .iter()
            .find(|item| item.style_id == style_id)
    }

    fn direction_index(&self, style_id: &str) -> Adm4Result<usize> {
        self.directions
            .iter()
            .position(|item| item.style_id == style_id)
            .ok_or_else(|| {
                Adm4Error::not_found(format!(
                    "风格方向 {style_id} 不在当前候选里（可用：{}）",
                    self.directions
                        .iter()
                        .map(|item| item.style_id.as_str())
                        .collect::<Vec<_>>()
                        .join(" / ")
                ))
            })
    }

    /// 被标为推荐的方向数（册 08 §2.2 要求**恰好一个**）。
    pub fn recommended_count(&self) -> usize {
        self.directions
            .iter()
            .filter(|item| item.recommended)
            .count()
    }

    /// 还没出图的方向 id（界面据此显示「待生成」，服务层据此断点续跑）。
    pub fn pending_style_ids(&self) -> Vec<String> {
        self.directions
            .iter()
            .filter(|item| item.preview.is_none())
            .map(|item| item.style_id.clone())
            .collect()
    }
}

// ---------------------------------------------------------------------------
// 风格-原型适配报告（册 08 §2.2）
// ---------------------------------------------------------------------------

/// 适配风险。默认 [`StyleFitRisk::Unknown`]：判不出来就如实说判不出来，不算「没问题」。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StyleFitRisk {
    #[default]
    Unknown,
    Ok,
    Caution,
}

impl StyleFitRisk {
    pub fn label_zh(self) -> &'static str {
        match self {
            Self::Unknown => "未判定",
            Self::Ok => "适配",
            Self::Caution => "需注意",
        }
    }
}

/// 一个方向与当前玩法原型的适配结论。
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct StyleFitEntry {
    pub style_id: String,
    pub preset_key: String,
    pub risk: StyleFitRisk,
    /// 判定依据（真源里的哪条事实 + 预设的哪条属性）。
    pub reason: String,
    /// 结论的真源锚点（结论必须指得出出处）。
    pub anchors: Vec<SpecRef>,
}

/// 风格-原型适配报告。
///
/// `advisory_only` 恒为真：册 08 §2.2 明确「提示但不阻断」——可读性风险要让人看见，
/// 但选不选是人的口味，机器不替人否决。
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct StyleFitReport {
    pub schema_version: String,
    pub generated_at: String,
    pub genre_pack: String,
    pub advisory_only: bool,
    pub entries: Vec<StyleFitEntry>,
}

impl StyleFitReport {
    pub fn entry(&self, style_id: &str) -> Option<&StyleFitEntry> {
        self.entries.iter().find(|entry| entry.style_id == style_id)
    }
}

// ---------------------------------------------------------------------------
// 确认与锁定产物
// ---------------------------------------------------------------------------

/// 确认状态。**只有 `Approved` 一个取值**：一份写着别的状态的确认记录没有意义
/// （没批准就不该有这个文件）。旧档里若出现别的取值，解析即失败（fail-closed）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StyleApprovalStatus {
    #[default]
    Approved,
}

/// 确认方式。**类型层面没有 auto_accept**：册 08 §2.4 与 R3 要求这道门必须由人署名通过，
/// 把「自动通过」做成一个取不到的值，比写一句「禁止自动通过」的注释可靠。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StyleConfirmMode {
    #[default]
    Manual,
}

/// 风格确认记录（册 08 §2.4 的 `style_confirmation`）。
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct StyleConfirmation {
    pub status: StyleApprovalStatus,
    pub mode: StyleConfirmMode,
    pub selected_style_id: String,
    pub selected_title: String,
    /// 选中方向的锚图路径（相对 `content/style/`）。
    pub selected_image_path: String,
    /// 确认结论（必填，R3）。
    pub notes: String,
    /// 署名（必填，R3）。册 08 §2.4 的字段清单里没有它，见模块级【优化】说明。
    pub actor: String,
    pub at: String,
    pub anchor_version: u32,
}

/// 锚点集里的一张锚图。
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct StyleAnchorImage {
    pub anchor_id: String,
    /// 锚图角色：`selected_preview` = 用户选中并确认的那张。
    pub role: String,
    /// 路径相对 `content/style/`（不可变版本目录内，落盘后不再改动）。
    pub image_path: String,
    pub image_sha256: String,
    pub image_bytes: u64,
    pub media_type: String,
    pub requested_width: u32,
    pub requested_height: u32,
    /// 产出这张图的提示词（一致性比对时要知道当时要求的是什么）。
    pub prompt: String,
    pub provider_id: String,
    pub model: String,
}

/// **`style_anchor_set`**：风格的唯一真相（册 08 §2.4）。
///
/// 下游（G3 资产生产）只读它与 [`StyleApplicationContract`]，不重造风格。
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct StyleAnchorSet {
    pub schema_version: String,
    /// 版本号（`anchors/v{N}` 的 N），只增不改。
    pub anchor_version: u32,
    pub generated_at: String,
    pub project_name: String,
    pub genre_pack: String,
    /// 锚定的创作态 revision：设计后来又变了，靠它判定锚点是否落后于设计。
    pub source_revision: u64,
    /// 真源锚点（R4：这套风格派生自哪些已确认决策点）。
    pub source_anchors: Vec<SpecRef>,
    pub selected_style_id: String,
    pub selected_title: String,
    pub preset_key: String,
    /// 最终提示词（改词后的实际提示词；下游拼提示词时以它为准）。
    pub final_prompt: String,
    /// 最终提示词是否由用户改词得来。
    pub prompt_overridden: bool,
    pub palette: Vec<String>,
    /// 代表锚图。当前恒含选中方向那一张；G3 可在新版本里追加代表资产锚图。
    pub anchors: Vec<StyleAnchorImage>,
    pub confirmation: StyleConfirmation,
}

impl StyleAnchorSet {
    /// 内容哈希（规范化 JSON）：应用契约用它回指真源，下游据此判断契约是否过期。
    pub fn content_hash(&self) -> Adm4Result<String> {
        let value = serde_json::to_value(self)
            .map_err(|error| Adm4Error::internal(format!("锚点集序列化失败：{error}")))?;
        Ok(ContentHash::of_canonical_json(&value)?.0)
    }

    /// 确定性自校验：任何一条不成立，这份锚点集就不配当下游的唯一依据。
    pub fn validate(&self) -> Adm4Result<()> {
        if self.anchor_version == 0 {
            return Err(Adm4Error::validation(
                "锚点集缺版本号：不可变历史靠版本号定位（v{N}）",
            ));
        }
        if !self.selected_style_id.starts_with(STYLE_ID_PREFIX) {
            return Err(Adm4Error::validation(format!(
                "锚点集的方向 id「{}」不符命名（应以 {STYLE_ID_PREFIX} 开头，册 08 §2.5）",
                self.selected_style_id
            )));
        }
        if self.final_prompt.trim().is_empty() {
            return Err(Adm4Error::validation(
                "锚点集缺最终提示词：下游拼提示词时无据可依",
            ));
        }
        if self.source_anchors.is_empty() {
            return Err(Adm4Error::red_line(
                "R4：锚点集没有真源锚点，等于一套凭空发明的风格",
            ));
        }
        if self.anchors.is_empty() {
            return Err(Adm4Error::validation(
                "锚点集没有任何锚图：风格锚点的意义就是那张图，缺图即无基准",
            ));
        }
        for anchor in &self.anchors {
            if anchor.image_sha256.trim().is_empty() {
                return Err(Adm4Error::validation(format!(
                    "锚图 {} 缺 sha256：一致性比对拿不到基准指纹",
                    anchor.anchor_id
                )));
            }
            if anchor.image_path.trim().is_empty() {
                return Err(Adm4Error::validation(format!(
                    "锚图 {} 缺落盘路径",
                    anchor.anchor_id
                )));
            }
        }
        if self.confirmation.actor.trim().is_empty() {
            return Err(Adm4Error::red_line(
                "R3：锚点集的确认记录没有署名（风格门是 attended 人工门）",
            ));
        }
        if self.confirmation.notes.trim().is_empty() {
            return Err(Adm4Error::red_line(
                "R3：锚点集的确认记录没有结论（署名而不给结论不构成评审）",
            ));
        }
        if self.confirmation.selected_style_id != self.selected_style_id {
            return Err(Adm4Error::validation(format!(
                "确认记录选的是 {}，锚点集锁的是 {}：两处必须一致",
                self.confirmation.selected_style_id, self.selected_style_id
            )));
        }
        Ok(())
    }

    /// 选中方向的锚图（`role == "selected_preview"`）。
    pub fn selected_anchor(&self) -> Option<&StyleAnchorImage> {
        self.anchors
            .iter()
            .find(|anchor| anchor.role == SELECTED_ANCHOR_ROLE)
    }
}

/// 选中方向锚图的角色标识。
pub const SELECTED_ANCHOR_ROLE: &str = "selected_preview";

/// 代表资产锚图的角色标识（册 08 §2.4，由 G3 在 C3 已在案后以新锚点版本追加）。
pub const REPRESENTATIVE_ANCHOR_ROLE: &str = "representative_asset";

// ---------------------------------------------------------------------------
// 应用契约（风格 → 资产生产的正式接口，册 08 §2.4）
// ---------------------------------------------------------------------------

/// 资产用途（册 08 §2.4 列举的五类）。
///
/// `Unknown` 只是旧档/漏填的落点，校验时报错（与 `AssetCategory` 同款理由）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StyleUsage {
    #[default]
    Unknown,
    Tile,
    Icon,
    Ui,
    Background,
    Effect,
}

impl StyleUsage {
    /// 契约必须覆盖的五类用途（顺序即落盘顺序，确定性）。
    pub fn all() -> [StyleUsage; 5] {
        [
            Self::Tile,
            Self::Icon,
            Self::Ui,
            Self::Background,
            Self::Effect,
        ]
    }

    pub fn label_zh(self) -> &'static str {
        match self {
            Self::Unknown => "未声明",
            Self::Tile => "地块",
            Self::Icon => "图标",
            Self::Ui => "界面",
            Self::Background => "背景",
            Self::Effect => "特效",
        }
    }
}

/// 一类用途上的风格约束。
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct StyleConstraint {
    pub usage: StyleUsage,
    pub readability: String,
    pub contrast: String,
    pub transparent_margin: String,
    pub forbidden: Vec<String>,
}

/// **`style_application_contract`**：风格向资产生产传递约束的正式接口（册 08 §2.4）。
///
/// 它是 [`StyleAnchorSet`] 的**投影**（`source_anchor_hash` 回指），不是第二真源：
/// 锚点集换版本，本契约必须重新派生，旧契约不得跨版本复用（与 `ContractEnvelope`
/// 用 `source_frozen_hash` 锚定 `GameSpec` 是同一个套路）。
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct StyleApplicationContract {
    pub schema_version: String,
    pub anchor_version: u32,
    pub generated_at: String,
    /// 所派生锚点集的内容哈希（D22 的机器保证）。
    pub source_anchor_hash: String,
    pub selected_style_id: String,
    pub preset_key: String,
    pub palette: Vec<String>,
    /// 下游资产生产必须拼在自己提示词前的风格前缀（保风格一致的实际抓手）。
    pub prompt_prefix: String,
    /// 分用途约束（恒覆盖 [`StyleUsage::all`] 五类）。
    pub style_constraints: Vec<StyleConstraint>,
    /// 下游必须遵守、不得改的通用规则。
    pub application_rules: Vec<String>,
}

impl StyleApplicationContract {
    /// 从锚点集确定性派生（同一份锚点集永远得到逐字相同的契约）。
    pub fn derive(anchor_set: &StyleAnchorSet, now: &str) -> Adm4Result<Self> {
        anchor_set.validate()?;
        let preset = style_presets()
            .into_iter()
            .find(|preset| preset.preset_key == anchor_set.preset_key)
            .ok_or_else(|| {
                Adm4Error::not_found(format!(
                    "锚点集引用的风格预设 {} 不在预设表里：无法派生应用契约",
                    anchor_set.preset_key
                ))
            })?;
        let constraints = StyleUsage::all()
            .into_iter()
            .map(|usage| usage_constraint(usage, &preset))
            .collect();
        Ok(Self {
            schema_version: STYLE_SCHEMA_VERSION.to_string(),
            anchor_version: anchor_set.anchor_version,
            generated_at: now.to_string(),
            source_anchor_hash: anchor_set.content_hash()?,
            selected_style_id: anchor_set.selected_style_id.clone(),
            preset_key: anchor_set.preset_key.clone(),
            palette: anchor_set.palette.clone(),
            prompt_prefix: anchor_set.final_prompt.clone(),
            style_constraints: constraints,
            application_rules: vec![
                format!(
                    "一切资产提示词必须以本契约的 prompt_prefix 起头，风格词不得替换（锚点 v{}）",
                    anchor_set.anchor_version
                ),
                format!(
                    "配色只许取 palette 里的 {} 个色值及其明度变体",
                    anchor_set.palette.len()
                ),
                "本契约由风格锚点集派生：要改风格请回设计工作台的风格门另立新版，不得就地改契约"
                    .to_string(),
                format!(
                    "一致性比对以锚图 {} 为基准",
                    anchor_set
                        .selected_anchor()
                        .map(|anchor| anchor.image_path.as_str())
                        .unwrap_or("（缺锚图）")
                ),
            ],
        })
    }

    /// 确定性自校验。
    pub fn validate(&self) -> Adm4Result<()> {
        if self.source_anchor_hash.trim().is_empty() {
            return Err(Adm4Error::validation(
                "风格应用契约未记录锚点集哈希：无法判断它派生自哪一版风格（D22）",
            ));
        }
        if self.prompt_prefix.trim().is_empty() {
            return Err(Adm4Error::validation(
                "风格应用契约缺 prompt_prefix：下游拼不出带风格的提示词",
            ));
        }
        let mut seen: Vec<StyleUsage> = Vec::new();
        for constraint in &self.style_constraints {
            if constraint.usage == StyleUsage::Unknown {
                return Err(Adm4Error::validation(
                    "风格应用契约里有未声明用途的约束条目（tile/icon/ui/background/effect）",
                ));
            }
            if seen.contains(&constraint.usage) {
                return Err(Adm4Error::conflict(format!(
                    "用途 {} 有两条约束：同一用途只能有一条（否则下游不知道听谁的）",
                    constraint.usage.label_zh()
                )));
            }
            seen.push(constraint.usage);
        }
        for usage in StyleUsage::all() {
            if !seen.contains(&usage) {
                return Err(Adm4Error::validation(format!(
                    "风格应用契约缺 {} 用途的约束：册 08 §2.4 要求五类用途全覆盖",
                    usage.label_zh()
                )));
            }
        }
        Ok(())
    }

    pub fn constraint(&self, usage: StyleUsage) -> Option<&StyleConstraint> {
        self.style_constraints
            .iter()
            .find(|item| item.usage == usage)
    }

    /// 校验一份契约是否确实派生自给定锚点集（下游开跑前的对账）。
    pub fn matches(&self, anchor_set: &StyleAnchorSet) -> Adm4Result<()> {
        let expected = anchor_set.content_hash()?;
        if self.source_anchor_hash != expected {
            return Err(Adm4Error::conflict(format!(
                "风格应用契约锚定的锚点集哈希与实际锚点集不符（契约 {} vs 实际 {}）：\
                 锚点集改过而契约没重新派生",
                self.source_anchor_hash, expected
            )));
        }
        Ok(())
    }
}

/// 某一类用途的约束：预设的基调 + 该用途固有的硬要求。
fn usage_constraint(usage: StyleUsage, preset: &StylePreset) -> StyleConstraint {
    let (readability, margin, forbidden): (&str, &str, Vec<&str>) = match usage {
        StyleUsage::Tile => (
            "地块在最小缩放下仍需靠轮廓区分类型",
            "无（贴满格，不留透明边）",
            vec!["图形溢出格边", "相邻格拼接处出现缝隙或错位"],
        ),
        StyleUsage::Icon => (
            "缩至 32x32 仍能认出主体",
            "四周留 8% 透明边距",
            vec!["细描边", "图标内嵌文字"],
        ),
        StyleUsage::Ui => (
            "文字与底色对比度不低于 4.5:1",
            "九宫格拉伸区四周留 12px 安全边",
            vec!["高频渐变噪点", "与内容争焦点的大面积高饱和色块"],
        ),
        StyleUsage::Background => (
            "背景不得与前景单位争焦点",
            "无",
            vec!["与前景同明度", "高频细节干扰单位轮廓"],
        ),
        StyleUsage::Effect => (
            "半透明叠加不得遮蔽单位轮廓",
            "四周留 15% 透明边距",
            vec!["纯白全屏爆闪", "覆盖 UI 区域"],
        ),
        StyleUsage::Unknown => ("未声明", "未声明", Vec::new()),
    };
    StyleConstraint {
        usage,
        readability: format!("{readability}；风格基调：{}", preset.readability),
        contrast: preset.contrast.to_string(),
        transparent_margin: margin.to_string(),
        forbidden: forbidden.into_iter().map(str::to_string).collect(),
    }
}

// ---------------------------------------------------------------------------
// 就绪查询（给 Phase 2 runner / 门面用）
// ---------------------------------------------------------------------------

/// 「风格锚点是否就绪」的可判定结论。
///
/// 为什么是结构而不是 `Result`：未确认**不是错误**（设计阶段本来就有没定风格的时刻），
/// 它是一条要显示给人看的结论。真正要阻断下游时调 [`StyleReadiness::require_ready`]。
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct StyleReadiness {
    pub ready: bool,
    /// 就绪时为当前锚点版本；未就绪为 0。
    pub anchor_version: u32,
    pub selected_style_id: String,
    /// 就绪时为锚点集内容哈希。
    pub anchor_hash: String,
    /// 结论说明（未就绪时带册 08 §3 的阻断码）。
    pub detail: String,
}

impl StyleReadiness {
    /// 未就绪的结论（带册 08 §3 的阻断码，便于脚本与日志定位）。
    pub fn not_ready(reason: impl std::fmt::Display) -> Self {
        Self {
            ready: false,
            anchor_version: 0,
            selected_style_id: String::new(),
            anchor_hash: String::new(),
            detail: format!("{STYLE_APPLICATION_CONTRACT_NOT_APPROVED}：{reason}"),
        }
    }

    /// 下游开跑前的硬门：未就绪即 `Blocked`（册 08 §3）。
    pub fn require_ready(&self) -> Adm4Result<()> {
        if self.ready {
            return Ok(());
        }
        Err(Adm4Error::blocked(format!(
            "{} 下游美术任务不得开跑：请先在设计工作台的风格门确认风格锚点",
            self.detail
        )))
    }
}

// ---------------------------------------------------------------------------
// 存储层
// ---------------------------------------------------------------------------

/// 风格门的产物仓（根 = 存档内容树的 `style/`）。
///
/// 落盘路径一律**相对本根**记录（`previews/...` / `anchors/v1/...`），因此存档整体
/// 搬家或导出导入之后路径照旧有效——记绝对路径的话，导入别人的包立刻满屏坏图。
pub struct StyleAnchorStore {
    root: PathBuf,
}

impl StyleAnchorStore {
    pub fn new(root: PathBuf) -> Self {
        Self { root }
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    /// 相对路径 → 绝对路径（呈现层加载图片用）。
    pub fn absolute(&self, relative: &str) -> PathBuf {
        self.root.join(relative)
    }

    fn session_path(&self) -> PathBuf {
        self.root.join("session.json")
    }

    fn anchors_root(&self) -> PathBuf {
        self.root.join("anchors")
    }

    fn version_dir(&self, version: u32) -> PathBuf {
        self.anchors_root().join(format!("v{version}"))
    }

    /// 工作态（尚未生成过 → `Ok(None)`）。
    pub fn load_session(&self) -> Adm4Result<Option<StyleSession>> {
        let path = self.session_path();
        if !path.is_file() {
            return Ok(None);
        }
        Ok(Some(read_json_file(&path)?))
    }

    /// 工作态必须在（没有就报错，指路怎么生成）。
    pub fn require_session(&self) -> Adm4Result<StyleSession> {
        self.load_session()?.ok_or_else(|| {
            Adm4Error::not_found(
                "本项目还没有风格工作态：请先生成风格方向（CLI：style generate；界面：设计工作台「风格」页签）",
            )
        })
    }

    pub fn save_session(&self, session: &StyleSession) -> Adm4Result<()> {
        write_json_file(&self.session_path(), session)
    }

    /// 写一张图（相对路径），返回 sha256 与字节数。
    pub fn write_image(&self, relative: &str, bytes: &[u8]) -> Adm4Result<(String, u64)> {
        let safe = adm4_foundation::ensure_within_root(Path::new(relative))?;
        let path = self.root.join(safe);
        atomic_write(&path, bytes)?;
        Ok((sha256_hex(bytes), bytes.len() as u64))
    }

    pub fn read_image(&self, relative: &str) -> Adm4Result<Vec<u8>> {
        let safe = adm4_foundation::ensure_within_root(Path::new(relative))?;
        let path = self.root.join(safe);
        std::fs::read(&path)
            .map_err(|error| Adm4Error::io(format!("读图像 {} 失败：{error}", path.display())))
    }

    /// 已锁定的全部锚点版本（升序）。
    pub fn anchor_versions(&self) -> Adm4Result<Vec<u32>> {
        let root = self.anchors_root();
        if !root.is_dir() {
            return Ok(Vec::new());
        }
        let entries = std::fs::read_dir(&root)
            .map_err(|error| Adm4Error::io(format!("读锚点历史目录失败：{error}")))?;
        let mut versions = Vec::new();
        for entry in entries {
            let entry =
                entry.map_err(|error| Adm4Error::io(format!("遍历锚点历史失败：{error}")))?;
            let name = entry.file_name();
            let Some(name) = name.to_str() else {
                continue;
            };
            let Some(digits) = name.strip_prefix('v') else {
                continue;
            };
            let Ok(version) = digits.parse::<u32>() else {
                continue;
            };
            // 只认真正落成的版本：有目录没 anchor_set.json 说明上次写盘半途而废，
            // 把它算进历史会让 readiness 去读一个不存在的文件。
            if entry.path().join(ANCHOR_SET_FILE).is_file() {
                versions.push(version);
            }
        }
        versions.sort_unstable();
        Ok(versions)
    }

    /// 最近一版锚点版本号（0 = 从未确认过）。
    pub fn latest_anchor_version(&self) -> Adm4Result<u32> {
        Ok(self.anchor_versions()?.last().copied().unwrap_or(0))
    }

    pub fn load_anchor_set(&self, version: u32) -> Adm4Result<StyleAnchorSet> {
        read_json_file(&self.version_dir(version).join(ANCHOR_SET_FILE))
    }

    pub fn load_application_contract(&self, version: u32) -> Adm4Result<StyleApplicationContract> {
        read_json_file(&self.version_dir(version).join(APPLICATION_CONTRACT_FILE))
    }

    pub fn load_confirmation(&self, version: u32) -> Adm4Result<StyleConfirmation> {
        read_json_file(&self.version_dir(version).join(CONFIRMATION_FILE))
    }

    pub fn load_fit_report(&self, version: u32) -> Adm4Result<StyleFitReport> {
        read_json_file(&self.version_dir(version).join(FIT_REPORT_FILE))
    }

    /// **风格锚点是否就绪**：G1 的制品注册表把「风格锚点集」声明为 P2 的外部输入，
    /// 这就是那个「外部输入到位了没有」的查询。
    ///
    /// 就绪的判定是**实读实校**（不看一个状态位）：锚点集与应用契约都能读出来、
    /// 各自自校验通过、且契约确实派生自这份锚点集。任何一条不成立即未就绪并说清原因。
    pub fn readiness(&self) -> Adm4Result<StyleReadiness> {
        let version = self.latest_anchor_version()?;
        if version == 0 {
            return Ok(StyleReadiness::not_ready(
                "本项目从未确认过风格锚点（style/anchors/ 下没有任何已锁版本）",
            ));
        }
        let anchor_set = match self.load_anchor_set(version) {
            Ok(anchor_set) => anchor_set,
            Err(error) => {
                return Ok(StyleReadiness::not_ready(format!(
                    "锚点 v{version} 读不出来：{}",
                    error.message
                )));
            }
        };
        if let Err(error) = anchor_set.validate() {
            return Ok(StyleReadiness::not_ready(format!(
                "锚点 v{version} 自校验不通过：{}",
                error.message
            )));
        }
        let contract = match self.load_application_contract(version) {
            Ok(contract) => contract,
            Err(error) => {
                return Ok(StyleReadiness::not_ready(format!(
                    "锚点 v{version} 的风格应用契约读不出来：{}",
                    error.message
                )));
            }
        };
        if let Err(error) = contract.validate() {
            return Ok(StyleReadiness::not_ready(format!(
                "锚点 v{version} 的风格应用契约自校验不通过：{}",
                error.message
            )));
        }
        if let Err(error) = contract.matches(&anchor_set) {
            return Ok(StyleReadiness::not_ready(error.message));
        }
        Ok(StyleReadiness {
            ready: true,
            anchor_version: version,
            selected_style_id: anchor_set.selected_style_id.clone(),
            anchor_hash: contract.source_anchor_hash.clone(),
            detail: format!(
                "风格锚点 v{version} 已确认（方向 {}，署名 {} 于 {}）",
                anchor_set.selected_title,
                anchor_set.confirmation.actor,
                anchor_set.confirmation.at
            ),
        })
    }
}

/// 锚点集文件名（存档兼容：不得更名）。
pub const ANCHOR_SET_FILE: &str = "anchor_set.json";
/// 应用契约文件名。
pub const APPLICATION_CONTRACT_FILE: &str = "application_contract.json";
/// 确认记录文件名。
pub const CONFIRMATION_FILE: &str = "style_confirmation.json";
/// 适配报告文件名。
pub const FIT_REPORT_FILE: &str = "style_fit.json";

// ---------------------------------------------------------------------------
// 门（生成 / 改词重生成 / 确认锁定 / 状态）
// ---------------------------------------------------------------------------

/// 一次确认锁定的产物集合。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StyleLockOutcome {
    pub anchor_set: StyleAnchorSet,
    pub application_contract: StyleApplicationContract,
    pub fit_report: StyleFitReport,
    /// 确认后的工作态（`confirmed_version` 已更新）。
    pub session: StyleSession,
    /// 被取代的上一版（None = 这是第一版）。
    pub superseded_version: Option<u32>,
}

/// 方向的状态摘要行（呈现层用）。
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct StyleDirectionStatus {
    pub style_id: String,
    pub title: String,
    pub description: String,
    pub prompt_summary: String,
    pub prompt_overridden: bool,
    pub recommended: bool,
    pub recommended_reason: String,
    pub palette: Vec<String>,
    pub fit_risk: StyleFitRisk,
    pub fit_reason: String,
    /// 预览图相对路径（空 = 还没出图）。
    pub image_path: String,
    pub image_sha256: String,
    /// 最近一次失败原因（空 = 没失败过或已成功）。
    pub last_failure: String,
    pub is_selected: bool,
}

/// 风格门状态（唯一的只读投影：CLI 与桌面共用一份口径）。
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct StyleGateStatus {
    pub project_name: String,
    pub genre_pack: String,
    /// 工作态是否存在。
    pub session_present: bool,
    /// 工作态锚定的创作态 revision。
    pub session_revision: u64,
    /// 当前创作态 revision。
    pub current_revision: u64,
    /// 工作态锚定的设计已变（提示不阻断：改词重生成或推翻重生成都能解决）。
    pub session_stale: bool,
    pub directions: Vec<StyleDirectionStatus>,
    pub round_count: usize,
    pub latest_round_id: String,
    /// 已锁定的全部版本（升序）。
    pub anchor_versions: Vec<u32>,
    pub readiness: StyleReadiness,
    /// 已确认版本锚定的 revision 与当前不一致（风格落后于设计，提示不阻断）。
    pub anchor_stale: bool,
    pub confirmed_actor: String,
    pub confirmed_at: String,
    pub confirmed_notes: String,
}

/// 设计阶段风格门。
///
/// 所有状态迁移都在这里（GUI 无业务规则，D14）：界面只把「哪个方向 / 什么改词 /
/// 谁署名 / 什么结论」转发过来，能不能确认、下一步是什么，一律由本类型判定。
pub struct StyleGate<'a> {
    store: &'a StyleAnchorStore,
}

impl<'a> StyleGate<'a> {
    pub fn new(store: &'a StyleAnchorStore) -> Self {
        Self { store }
    }

    /// 生成风格方向 + 预览图（册 08 §2.2）。
    ///
    /// 三种情形一个入口：
    /// - 没有工作态 → 派生方向并全量出图；
    /// - 有工作态且真源未变、`force=false` → **断点续跑**，只给还没出图的方向补图
    ///   （不重复调用图像通道，也就不重复花钱）；
    /// - `force=true` 或真源 revision 变了 → 推翻重来（重新派生方向、清掉全部旧预览，
    ///   册 08 §2.3「清掉未选图」）。已锁定的历史版本一概不动。
    ///
    /// 图像生成失败时：本轮记录**先落盘**（可停可续），然后把原始失败原因原样上抛
    /// （R7：不产占位图、不静默跳过那个方向）。
    pub fn generate(
        &self,
        facts: &StyleSourceFacts,
        scanner: &SkinScanner,
        images: &dyn ImageProvider,
        options: &StyleGenerationOptions,
        now: &str,
    ) -> Adm4Result<StyleSession> {
        facts.validate()?;
        options.validate()?;
        let existing = self.store.load_session()?;
        let reusable = existing.filter(|previous| {
            !options.force
                && previous.source_revision == facts.source_revision
                && previous.directions.len() == options.direction_count
                && previous.preview_width == options.preview_width
                && previous.preview_height == options.preview_height
        });
        let mut session = match reusable {
            Some(previous) => previous,
            None => new_session(facts, scanner, options, now)?,
        };
        let pending = session.pending_style_ids();
        if pending.is_empty() {
            // 全都有图了：不重复调用图像通道。要重出图请 force，或对单个方向重生成。
            session.updated_at = now.to_string();
            self.store.save_session(&session)?;
            return Ok(session);
        }
        let round_id = next_round_id(&session);
        let mut round = StyleGenerationRound {
            round_id: round_id.clone(),
            kind: StyleRoundKind::Initial,
            at: now.to_string(),
            items: Vec::with_capacity(pending.len()),
        };
        for style_id in &pending {
            let index = session.direction_index(style_id)?;
            let item = self.render_direction(&mut session, index, images, &round_id, now)?;
            round.items.push(item);
        }
        self.finish_round(&mut session, round, now)
    }

    /// 对某个方向提交改词并重生成预览（册 08 §2.3，次数不限）。
    ///
    /// `prompt_override` 为空串 = **清掉改词**回到派生提示词（这是唯一的清除通道；
    /// 不给这个语义的话，改错一次就再也回不到锚定真源的那版提示词了）。
    pub fn regenerate(
        &self,
        style_id: &str,
        prompt_override: &str,
        scanner: &SkinScanner,
        images: &dyn ImageProvider,
        now: &str,
    ) -> Adm4Result<StyleSession> {
        let mut session = self.store.require_session()?;
        let index = session.direction_index(style_id)?;
        let trimmed = prompt_override.trim();
        if !trimmed.is_empty() {
            scan_prompt(scanner, style_id, trimmed)?;
        }
        session.directions[index].prompt_override = trimmed.to_string();
        let round_id = next_round_id(&session);
        let mut round = StyleGenerationRound {
            round_id: round_id.clone(),
            kind: StyleRoundKind::Regenerate,
            at: now.to_string(),
            items: Vec::with_capacity(1),
        };
        let item = self.render_direction(&mut session, index, images, &round_id, now)?;
        round.items.push(item);
        self.finish_round(&mut session, round, now)
    }

    /// attended 确认并锁定（册 08 §2.4，R3）。
    ///
    /// 拒绝条件全在这里，界面不重复判：署名或结论为空 → 红线错误；选中的方向没有
    /// 预览图 → `Blocked`（风格门的意义就是看真图，没图的方向不许确认）。
    ///
    /// 落盘写**新版本** `anchors/v{N+1}/`；旧版本一个字节都不动（D4 不可变历史）。
    pub fn confirm(
        &self,
        style_id: &str,
        actor: &str,
        note: &str,
        now: &str,
    ) -> Adm4Result<StyleLockOutcome> {
        let actor = actor.trim();
        let note = note.trim();
        if actor.is_empty() {
            return Err(Adm4Error::red_line(
                "R3：风格锚点确认必须署名（这道门是 attended 人工门，禁止自动通过）",
            ));
        }
        if note.is_empty() {
            return Err(Adm4Error::red_line(
                "R3：风格锚点确认必须写结论（署名而不给结论不构成评审）",
            ));
        }
        let mut session = self.store.require_session()?;
        let index = session.direction_index(style_id)?;
        let direction = session.directions[index].clone();
        let Some(preview) = direction.preview.clone() else {
            return Err(Adm4Error::blocked(format!(
                "风格方向 {style_id} 还没有预览图：风格必须看真图才能确认，请先生成或重生成它"
            )));
        };

        let superseded = self.store.latest_anchor_version()?;
        let version = superseded + 1;
        let version_dir = self.store.version_dir(version);
        if version_dir.exists() {
            return Err(Adm4Error::conflict(format!(
                "锚点版本目录 {} 已存在：不可变历史绝不覆盖，请检查 style/anchors/ 的现状",
                version_dir.display()
            )));
        }
        ensure_dir(&version_dir)?;

        // 锚图落进版本目录：预览图会被后续重生成覆盖，锚图必须是这一版自己的副本。
        let extension = media_type_extension(&preview.media_type)?;
        let anchor_relative = format!("anchors/v{version}/{style_id}.{extension}");
        let bytes = self.store.read_image(&preview.image_path)?;
        let (image_sha256, image_bytes) = self.store.write_image(&anchor_relative, &bytes)?;
        if image_sha256 != preview.image_sha256 {
            return Err(Adm4Error::validation(format!(
                "锚图落盘后的指纹与预览记录不符（{image_sha256} vs {}）：预览图在确认前被改过",
                preview.image_sha256
            )));
        }

        let confirmation = StyleConfirmation {
            status: StyleApprovalStatus::Approved,
            mode: StyleConfirmMode::Manual,
            selected_style_id: direction.style_id.clone(),
            selected_title: direction.title.clone(),
            selected_image_path: anchor_relative.clone(),
            notes: note.to_string(),
            actor: actor.to_string(),
            at: now.to_string(),
            anchor_version: version,
        };
        let anchor_set = StyleAnchorSet {
            schema_version: STYLE_SCHEMA_VERSION.to_string(),
            anchor_version: version,
            generated_at: now.to_string(),
            project_name: session.project_name.clone(),
            genre_pack: session.genre_pack.clone(),
            source_revision: session.source_revision,
            source_anchors: session.source_anchors.clone(),
            selected_style_id: direction.style_id.clone(),
            selected_title: direction.title.clone(),
            preset_key: direction.preset_key.clone(),
            final_prompt: direction.effective_prompt().to_string(),
            prompt_overridden: !direction.prompt_override.trim().is_empty(),
            palette: direction.palette.clone(),
            anchors: vec![StyleAnchorImage {
                anchor_id: style_anchor_image_id(&direction.style_id, SELECTED_ANCHOR_ROLE),
                role: SELECTED_ANCHOR_ROLE.to_string(),
                image_path: anchor_relative,
                image_sha256,
                image_bytes,
                media_type: preview.media_type.clone(),
                requested_width: preview.requested_width,
                requested_height: preview.requested_height,
                prompt: direction.effective_prompt().to_string(),
                provider_id: preview.provider_id.clone(),
                model: preview.model.clone(),
            }],
            confirmation: confirmation.clone(),
        };
        anchor_set.validate()?;
        let contract = StyleApplicationContract::derive(&anchor_set, now)?;
        contract.validate()?;
        let mut fit_report = session.fit.clone();
        fit_report.generated_at = now.to_string();

        write_json_file(&version_dir.join(ANCHOR_SET_FILE), &anchor_set)?;
        write_json_file(&version_dir.join(APPLICATION_CONTRACT_FILE), &contract)?;
        write_json_file(&version_dir.join(CONFIRMATION_FILE), &confirmation)?;
        write_json_file(&version_dir.join(FIT_REPORT_FILE), &fit_report)?;

        session.confirmed_version = version;
        session.updated_at = now.to_string();
        self.store.save_session(&session)?;

        Ok(StyleLockOutcome {
            anchor_set,
            application_contract: contract,
            fit_report,
            session,
            superseded_version: (superseded > 0).then_some(superseded),
        })
    }

    /// 追加**代表资产锚图**（册 08 §2.4 的 G3 裁决项）：C3 已在案后，按应用契约的
    /// `prompt_prefix` 为少量代表资产生成锚图，以**新锚点版本**落盘。
    ///
    /// 不可变历史一条不破：旧版本目录不改不删；新版本复制上一版的选中锚图 + 追加
    /// 代表资产锚图（`role = "representative_asset"`），确认记录沿用原署名——追加锚图
    /// 不是重新选风格，方向、提示词、palette 逐字继承。
    ///
    /// `representatives`：(资产 id, 语义描述)。选法由调用方定（P2 门面按确定性规则选），
    /// 这里只负责生成与落盘。空清单直接报错——没有代表资产就没有可追加的东西。
    pub fn append_representative_anchors(
        &self,
        images: &dyn ImageProvider,
        representatives: &[(String, String)],
        width: u32,
        height: u32,
        now: &str,
    ) -> Adm4Result<StyleAnchorSet> {
        if representatives.is_empty() {
            return Err(Adm4Error::invalid_input(
                "代表资产清单为空：没有可追加的锚图（C3 白名单一个可见实体都没有？）",
            ));
        }
        let base_version = self.store.latest_anchor_version()?;
        if base_version == 0 {
            return Err(Adm4Error::blocked(
                "本项目还没有已确认的风格锚点：请先在风格门确认方向，再追加代表资产锚图",
            ));
        }
        let base = self.store.load_anchor_set(base_version)?;
        base.validate()?;
        let contract = self.store.load_application_contract(base_version)?;
        contract.matches(&base)?;

        let version = base_version + 1;
        let version_dir = self.store.version_dir(version);
        if version_dir.exists() {
            return Err(Adm4Error::conflict(format!(
                "锚点版本目录 {} 已存在：不可变历史绝不覆盖",
                version_dir.display()
            )));
        }
        ensure_dir(&version_dir)?;

        let mut anchor_set = base.clone();
        anchor_set.anchor_version = version;
        anchor_set.generated_at = now.to_string();
        anchor_set.confirmation.anchor_version = version;
        // 上一版的锚图复制进新版本目录（版本目录自足：删掉旧版不影响新版可用）。
        let mut carried = Vec::new();
        for anchor in &base.anchors {
            let bytes = self.store.read_image(&anchor.image_path)?;
            let file_name = anchor
                .image_path
                .rsplit('/')
                .next()
                .unwrap_or(anchor.image_path.as_str());
            let relative = format!("anchors/v{version}/{file_name}");
            let (sha256, size) = self.store.write_image(&relative, &bytes)?;
            if sha256 != anchor.image_sha256 {
                return Err(Adm4Error::validation(format!(
                    "锚图 {} 复制后指纹不符：上一版锚图被外部改动过",
                    anchor.anchor_id
                )));
            }
            let mut copied = anchor.clone();
            copied.image_path = relative;
            copied.image_bytes = size;
            carried.push(copied);
        }
        anchor_set.anchors = carried;

        // 逐代表资产生成锚图：提示词 = 契约前缀 + 资产语义（同 P2 生产口径的前缀纪律）。
        for (asset_id, semantic) in representatives {
            if semantic.trim().is_empty() {
                return Err(Adm4Error::validation(format!(
                    "代表资产 {asset_id} 没有语义描述：没有语义就没有提示词可拼（R2）"
                )));
            }
            let prompt = format!(
                "{} {}。单一主体、无文字、无水印。",
                contract.prompt_prefix.trim(),
                semantic.trim()
            );
            let artifact = images.generate(&ImageRequest {
                purpose: format!("style_representative/{asset_id}"),
                prompt: prompt.clone(),
                width,
                height,
            })?;
            let extension = media_type_extension(&artifact.media_type)?;
            let relative = format!(
                "anchors/v{version}/rep_{}.{extension}",
                asset_id.to_ascii_lowercase()
            );
            let (sha256, size) = self.store.write_image(&relative, &artifact.bytes)?;
            anchor_set.anchors.push(StyleAnchorImage {
                anchor_id: format!(
                    "{ANCHOR_ID_PREFIX}{}-representative-{asset_id}",
                    base.selected_style_id
                ),
                role: REPRESENTATIVE_ANCHOR_ROLE.to_string(),
                image_path: relative,
                image_sha256: sha256,
                image_bytes: size,
                media_type: artifact.media_type,
                requested_width: width,
                requested_height: height,
                prompt,
                provider_id: artifact.provider_id,
                model: artifact.model,
            });
        }
        anchor_set.validate()?;
        let new_contract = StyleApplicationContract::derive(&anchor_set, now)?;
        new_contract.validate()?;
        let fit_report = self.store.load_fit_report(base_version)?;

        write_json_file(&version_dir.join(ANCHOR_SET_FILE), &anchor_set)?;
        write_json_file(&version_dir.join(APPLICATION_CONTRACT_FILE), &new_contract)?;
        write_json_file(
            &version_dir.join(CONFIRMATION_FILE),
            &anchor_set.confirmation,
        )?;
        write_json_file(&version_dir.join(FIT_REPORT_FILE), &fit_report)?;

        // 工作态跟进确认版本（就绪查询按最新版本实读实校）。
        if let Some(mut session) = self.store.load_session()? {
            session.confirmed_version = version;
            session.updated_at = now.to_string();
            self.store.save_session(&session)?;
        }
        Ok(anchor_set)
    }

    /// 风格门状态（只读投影）。
    pub fn status(&self, current_revision: u64) -> Adm4Result<StyleGateStatus> {
        let readiness = self.store.readiness()?;
        let anchor_versions = self.store.anchor_versions()?;
        let session = self.store.load_session()?;
        let mut status = StyleGateStatus {
            anchor_versions,
            current_revision,
            ..StyleGateStatus::default()
        };
        if readiness.ready {
            let anchor_set = self.store.load_anchor_set(readiness.anchor_version)?;
            status.project_name = anchor_set.project_name.clone();
            status.genre_pack = anchor_set.genre_pack.clone();
            status.anchor_stale = anchor_set.source_revision != current_revision;
            status.confirmed_actor = anchor_set.confirmation.actor.clone();
            status.confirmed_at = anchor_set.confirmation.at.clone();
            status.confirmed_notes = anchor_set.confirmation.notes.clone();
        }
        status.readiness = readiness;
        if let Some(session) = session {
            status.session_present = true;
            status.project_name = session.project_name.clone();
            status.genre_pack = session.genre_pack.clone();
            status.session_revision = session.source_revision;
            status.session_stale = session.source_revision != current_revision;
            status.round_count = session.rounds.len();
            status.latest_round_id = session
                .rounds
                .last()
                .map(|round| round.round_id.clone())
                .unwrap_or_default();
            let selected = if status.readiness.ready {
                status.readiness.selected_style_id.clone()
            } else {
                String::new()
            };
            status.directions = session
                .directions
                .iter()
                .map(|direction| direction_status(&session, direction, &selected))
                .collect();
        }
        Ok(status)
    }

    /// 生成一个方向的预览图并就地更新工作态里的 `preview`。
    ///
    /// 返回本轮该方向的记录条目（成功或失败都有条目）；失败不在这里上抛——
    /// 由 [`Self::finish_round`] 在**落盘之后**统一上抛，保证「记录先在案」。
    fn render_direction(
        &self,
        session: &mut StyleSession,
        index: usize,
        images: &dyn ImageProvider,
        round_id: &str,
        now: &str,
    ) -> Adm4Result<StyleGenerationItem> {
        let direction = &session.directions[index];
        let prompt = direction.effective_prompt().to_string();
        let request = ImageRequest {
            purpose: STYLE_PREVIEW_PURPOSE.to_string(),
            prompt: prompt.clone(),
            width: session.preview_width,
            height: session.preview_height,
        };
        let mut item = StyleGenerationItem {
            style_id: direction.style_id.clone(),
            prompt,
            prompt_override: direction.prompt_override.clone(),
            requested_width: request.width,
            requested_height: request.height,
            provider_id: images.id().to_string(),
            ..StyleGenerationItem::default()
        };
        match images.generate(&request) {
            Ok(artifact) => {
                let extension = media_type_extension(&artifact.media_type)?;
                let relative = format!(
                    "previews/{round_id}/{}.{extension}",
                    session.directions[index].style_id
                );
                let (sha256, bytes) = self.store.write_image(&relative, &artifact.bytes)?;
                item.model = artifact.model.clone();
                item.provider_id = artifact.provider_id.clone();
                item.image_path = relative.clone();
                item.image_sha256 = sha256.clone();
                session.directions[index].preview = Some(StylePreview {
                    image_path: relative,
                    image_sha256: sha256,
                    image_bytes: bytes,
                    media_type: artifact.media_type,
                    requested_width: request.width,
                    requested_height: request.height,
                    provider_id: artifact.provider_id,
                    model: artifact.model,
                    generated_at: now.to_string(),
                    round_id: round_id.to_string(),
                });
            }
            Err(error) => {
                // R7：失败原因原样记录，且**清掉**这个方向的旧预览——留着旧图会让人
                // 以为这轮改词生效了（看到的其实是上一轮的图）。
                item.failure = error.message.clone();
                session.directions[index].preview = None;
            }
        }
        Ok(item)
    }

    /// 收尾一轮：追加记录 → 落盘 → 有失败则上抛（顺序不可换）。
    fn finish_round(
        &self,
        session: &mut StyleSession,
        round: StyleGenerationRound,
        now: &str,
    ) -> Adm4Result<StyleSession> {
        let failures: Vec<String> = round
            .failures()
            .into_iter()
            .map(|item| format!("{}：{}", item.style_id, item.failure))
            .collect();
        session.rounds.push(round);
        session.updated_at = now.to_string();
        // 先落盘再判成败：这样中途失败之后已出图的方向照旧在案（可停可续）。
        self.store.save_session(session)?;
        if !failures.is_empty() {
            return Err(Adm4Error::ai_unavailable(format!(
                "图像生成失败 {} 个方向（已生成的方向与本轮记录均已落盘，可续跑）：{}",
                failures.len(),
                failures.join("；")
            )));
        }
        Ok(session.clone())
    }
}

/// 派生一份新的工作态（方向候选 + 提示词 + 适配报告），不含任何图像调用。
fn new_session(
    facts: &StyleSourceFacts,
    scanner: &SkinScanner,
    options: &StyleGenerationOptions,
    now: &str,
) -> Adm4Result<StyleSession> {
    let presets = style_presets();
    if presets.len() < options.direction_count {
        return Err(Adm4Error::internal(format!(
            "风格预设表只有 {} 条，取不出 {} 个方向",
            presets.len(),
            options.direction_count
        )));
    }
    let anchors = facts.anchors();
    let intent = facts.intent_text();
    let mut directions = Vec::with_capacity(options.direction_count);
    for (index, preset) in presets.iter().take(options.direction_count).enumerate() {
        let style_id = style_direction_id(index, preset.preset_key);
        let prompt = format!(
            "{}, game art style board for \"{}\" ({}), design intent: {}, palette {} / {} / {}, {}",
            preset.prompt_keywords,
            facts.project_name.trim(),
            facts.genre_pack.trim(),
            intent,
            preset.palette[0],
            preset.palette[1],
            preset.palette[2],
            preset.readability
        );
        // R4：提示词必须锚定真源。锚点为空时 `AnchoredNarrative::new` 直接拒——
        // 上面的 `facts.validate()` 已经拦过一次，这里是类型层面的第二道保险。
        let anchored = AnchoredNarrative::new(prompt, anchors.clone())?;
        scan_prompt(scanner, &style_id, &anchored.text)?;
        let description = format!(
            "{}。派生依据：{}",
            preset.intent,
            if intent.is_empty() {
                "（真源未声明）".to_string()
            } else {
                intent.clone()
            }
        );
        directions.push(StyleDirection {
            style_id,
            preset_key: preset.preset_key.to_string(),
            title: preset.title.to_string(),
            description,
            derived_prompt: anchored.text,
            prompt_anchors: anchored.anchors,
            prompt_override: String::new(),
            palette: preset
                .palette
                .iter()
                .map(|color| color.to_string())
                .collect(),
            recommended: false,
            recommended_reason: String::new(),
            preview: None,
        });
    }
    let fit = derive_fit_report(facts, &directions, &presets, now);
    mark_recommended(&mut directions, &fit);
    Ok(StyleSession {
        schema_version: STYLE_SCHEMA_VERSION.to_string(),
        project_name: facts.project_name.trim().to_string(),
        genre_pack: facts.genre_pack.trim().to_string(),
        source_revision: facts.source_revision,
        source_anchors: anchors,
        source_summary: facts.summary_lines(),
        preview_width: options.preview_width,
        preview_height: options.preview_height,
        directions,
        rounds: Vec::new(),
        fit,
        confirmed_version: 0,
        created_at: now.to_string(),
        updated_at: now.to_string(),
    })
}

/// 风格-原型适配报告（册 08 §2.2 的 `style_fit`）：确定性判定，不经 AI。
///
/// 判定规则只有一条，写在这里比写在文档里更难跑偏：**预设的信息密度代价高 = 需注意**。
/// 「电影写实 + 高信息密度玩法 = 可读性风险」正是册 08 举的那个例子。
/// 结论一律 advisory（提示不阻断）——选哪个方向是人的口味。
fn derive_fit_report(
    facts: &StyleSourceFacts,
    directions: &[StyleDirection],
    presets: &[StylePreset],
    now: &str,
) -> StyleFitReport {
    let anchors = facts.anchors();
    let intent = facts.intent_text();
    let entries = directions
        .iter()
        .map(|direction| {
            let preset = presets
                .iter()
                .find(|preset| preset.preset_key == direction.preset_key);
            let (risk, reason) = match preset {
                Some(preset) => match preset.density_cost {
                    DensityCost::High => (
                        StyleFitRisk::Caution,
                        format!(
                            "「{}」的信息密度代价高：{}。本作已确认的方向是「{}」，\
                             若玩法信息密度高，可读性风险最大（提示不阻断）",
                            preset.title, preset.readability, intent
                        ),
                    ),
                    DensityCost::Medium => (
                        StyleFitRisk::Ok,
                        format!(
                            "「{}」信息密度代价中等：{}。与已确认方向「{}」可配，小尺寸图标需单独核对",
                            preset.title, preset.readability, intent
                        ),
                    ),
                    DensityCost::Low => (
                        StyleFitRisk::Ok,
                        format!(
                            "「{}」信息密度代价低：{}。与已确认方向「{}」适配",
                            preset.title, preset.readability, intent
                        ),
                    ),
                },
                // 预设表里找不到 = 工作态与代码不同版本，如实标未判定而不猜一个结论。
                None => (
                    StyleFitRisk::Unknown,
                    format!(
                        "方向 {} 引用的预设 {} 不在当前预设表里，无法判定适配性",
                        direction.style_id, direction.preset_key
                    ),
                ),
            };
            StyleFitEntry {
                style_id: direction.style_id.clone(),
                preset_key: direction.preset_key.clone(),
                risk,
                reason,
                anchors: anchors.clone(),
            }
        })
        .collect();
    StyleFitReport {
        schema_version: STYLE_SCHEMA_VERSION.to_string(),
        generated_at: now.to_string(),
        genre_pack: facts.genre_pack.trim().to_string(),
        advisory_only: true,
        entries,
    }
}

/// 标一个 `recommended`（册 08 §2.2）。
///
/// **【优化】不打分**：册 08 说「打分并标一个 recommended」，但一个没有证据的分数正是
/// R1 禁止的裸数字（`MeasuredMetric` 才是合法的指标形态）。这里改成「可判定的推荐 +
/// 写得出来的推荐理由」：取第一个适配结论为「适配」的方向；全都需注意时取第一个，
/// 并在理由里如实说明「本批没有低风险方向」。
fn mark_recommended(directions: &mut [StyleDirection], fit: &StyleFitReport) {
    let pick = directions
        .iter()
        .position(|direction| {
            fit.entry(&direction.style_id)
                .is_some_and(|entry| entry.risk == StyleFitRisk::Ok)
        })
        .unwrap_or(0);
    if let Some(direction) = directions.get_mut(pick) {
        let risk = fit
            .entry(&direction.style_id)
            .map(|entry| entry.risk)
            .unwrap_or_default();
        direction.recommended = true;
        direction.recommended_reason = match risk {
            StyleFitRisk::Ok => {
                "本批中第一个适配结论为「适配」的方向（推荐仅为起点，选哪个由你定）".to_string()
            }
            _ => format!(
                "本批没有适配结论为「适配」的方向，按顺序取第一个作为起点（当前结论：{}）",
                risk.label_zh()
            ),
        };
    }
}

/// R5：提示词过换皮扫描（册 08 §5 把提示词明确列进 R5 的强制点）。
///
/// 命中即拒而不是改写：静默替换会让用户以为自己写的提示词生效了，而实际发出去的是
/// 另一段话。改写是人的活儿。
fn scan_prompt(scanner: &SkinScanner, style_id: &str, prompt: &str) -> Adm4Result<()> {
    let hits = scanner.scan(&format!("style/{style_id}/prompt"), prompt);
    if hits.is_empty() {
        return Ok(());
    }
    let words: Vec<&str> = hits.iter().map(|hit| hit.matched_word.as_str()).collect();
    Err(Adm4Error::red_line(format!(
        "R5：风格提示词命中换皮词 {}（方向 {style_id}）。\
         参考游戏名不得进提示词——请改写成你自己的风格描述",
        words.join("、")
    )))
}

/// 下一轮的 round id（零填充四位：字典序 = 时间序，报告与目录都好排）。
fn next_round_id(session: &StyleSession) -> String {
    format!("r{:04}", session.rounds.len() + 1)
}

/// 方向 → 状态摘要行。
fn direction_status(
    session: &StyleSession,
    direction: &StyleDirection,
    selected_style_id: &str,
) -> StyleDirectionStatus {
    let fit = session.fit.entry(&direction.style_id);
    // 最近一次失败：从后往前找该方向的第一条记录，只有它才代表「现在的状况」。
    let last_failure = session
        .rounds
        .iter()
        .rev()
        .flat_map(|round| round.items.iter().rev())
        .find(|item| item.style_id == direction.style_id)
        .map(|item| item.failure.clone())
        .unwrap_or_default();
    let preview = direction.preview.as_ref();
    StyleDirectionStatus {
        style_id: direction.style_id.clone(),
        title: direction.title.clone(),
        description: direction.description.clone(),
        prompt_summary: direction.prompt_summary(PROMPT_SUMMARY_CHARS),
        prompt_overridden: !direction.prompt_override.trim().is_empty(),
        recommended: direction.recommended,
        recommended_reason: direction.recommended_reason.clone(),
        palette: direction.palette.clone(),
        fit_risk: fit.map(|entry| entry.risk).unwrap_or_default(),
        fit_reason: fit.map(|entry| entry.reason.clone()).unwrap_or_default(),
        image_path: preview
            .map(|item| item.image_path.clone())
            .unwrap_or_default(),
        image_sha256: preview
            .map(|item| item.image_sha256.clone())
            .unwrap_or_default(),
        last_failure,
        is_selected: !selected_style_id.is_empty() && selected_style_id == direction.style_id,
    }
}

/// 卡片上提示词摘要的字符上限。
pub const PROMPT_SUMMARY_CHARS: usize = 96;

#[cfg(test)]
mod tests {
    use super::*;
    use adm4_ai::ScriptedImageProvider;
    use std::path::PathBuf;

    struct TempRoot {
        path: PathBuf,
    }

    impl TempRoot {
        fn new(tag: &str) -> Self {
            let path = std::env::temp_dir().join(format!(
                "adm4_style_{tag}_{}_{}",
                std::process::id(),
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map(|value| value.as_nanos())
                    .unwrap_or_default()
            ));
            ensure_dir(&path).expect("建临时目录");
            Self { path }
        }
    }

    impl Drop for TempRoot {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.path);
        }
    }

    const NOW: &str = "2026-09-01T00:00:00Z";

    fn facts() -> StyleSourceFacts {
        StyleSourceFacts {
            project_name: "霜落峡谷防卫计划".into(),
            genre_pack: "lane_defense".into(),
            source_revision: 12,
            entries: vec![
                StyleSourceFact {
                    decision_id: "u.genre".into(),
                    question: "主品类是什么？".into(),
                    option_labels: vec!["通道防守".into()],
                },
                StyleSourceFact {
                    decision_id:
                        "v2.art_direction_decision.feng_ge_ding_wei.presentation_feeling_target"
                            .into(),
                    question: "呈现要给玩家什么感受？".into(),
                    option_labels: vec!["清晰可读".into(), "沉浸氛围".into()],
                },
            ],
        }
    }

    fn options(count: usize) -> StyleGenerationOptions {
        StyleGenerationOptions {
            direction_count: count,
            preview_width: 32,
            preview_height: 24,
            force: false,
        }
    }

    /// 提示词派生必须锚定真源：一条已确认事实都没有 → Err（R4）。
    #[test]
    fn prompt_derivation_requires_source_anchors() {
        let store_root = TempRoot::new("noanchor");
        let store = StyleAnchorStore::new(store_root.path.clone());
        let gate = StyleGate::new(&store);
        let scanner = SkinScanner::default();
        let images = ScriptedImageProvider::new();

        let mut empty = facts();
        empty.entries.clear();
        let error = gate
            .generate(&empty, &scanner, &images, &options(3), NOW)
            .expect_err("无锚必须报错");
        assert_eq!(error.kind, adm4_foundation::Adm4ErrorKind::RedLine);
        assert!(error.message.contains("R4"), "{}", error.message);

        // 有决策点但选项标签全空同样算无锚（不拿空标签冒充事实）。
        let mut blank = facts();
        for entry in &mut blank.entries {
            entry.option_labels = vec!["  ".into()];
        }
        assert!(
            gate.generate(&blank, &scanner, &images, &options(3), NOW)
                .is_err()
        );
        // 一张图都不该生成。
        assert!(images.calls().is_empty());
    }

    /// 生成：3-5 方向、提示词含真源意图、锚点齐备、恰好一个 recommended、预览图落盘。
    #[test]
    fn generate_produces_anchored_directions_with_previews() {
        let store_root = TempRoot::new("generate");
        let store = StyleAnchorStore::new(store_root.path.clone());
        let gate = StyleGate::new(&store);
        let images = ScriptedImageProvider::new();
        let session = gate
            .generate(&facts(), &SkinScanner::default(), &images, &options(4), NOW)
            .expect("生成");

        assert_eq!(session.directions.len(), 4);
        assert_eq!(session.rounds.len(), 1);
        assert_eq!(session.rounds[0].round_id, "r0001");
        assert_eq!(session.rounds[0].kind, StyleRoundKind::Initial);
        assert_eq!(session.source_revision, 12);
        assert_eq!(session.source_anchors.len(), 2);
        assert_eq!(
            session.recommended_count(),
            1,
            "必须恰好标一个推荐方向（册 08 §2.2）"
        );
        for direction in &session.directions {
            assert!(direction.style_id.starts_with(STYLE_ID_PREFIX));
            assert!(
                direction.derived_prompt.contains("清晰可读"),
                "提示词必须带真源意图：{}",
                direction.derived_prompt
            );
            assert!(direction.derived_prompt.contains("霜落峡谷防卫计划"));
            assert_eq!(direction.prompt_anchors.len(), 2);
            assert_eq!(direction.palette.len(), 3);
            let preview = direction.preview.as_ref().expect("每个方向都该有预览图");
            assert!(preview.image_path.starts_with("previews/r0001/"));
            assert!(preview.image_sha256.starts_with("sha256:"));
            assert!(store.absolute(&preview.image_path).is_file());
            assert_eq!(preview.provider_id, "scripted_image");
            assert_eq!(
                (preview.requested_width, preview.requested_height),
                (32, 24)
            );
        }
        // 每个方向一次图像调用，提示词就是实际发出去的那一段。
        let calls = images.calls();
        assert_eq!(calls.len(), 4);
        assert_eq!(calls[0].purpose, STYLE_PREVIEW_PURPOSE);
        assert_eq!(calls[0].prompt, session.directions[0].derived_prompt);
        // 工作态可回读，且逐字相同。
        let reloaded = store.require_session().expect("回读工作态");
        assert_eq!(reloaded, session);
        // 未确认 → 下游必须可判定被阻断。
        let readiness = store.readiness().expect("就绪查询");
        assert!(!readiness.ready);
        assert!(
            readiness
                .detail
                .contains(STYLE_APPLICATION_CONTRACT_NOT_APPROVED)
        );
        assert!(readiness.require_ready().is_err());
    }

    #[test]
    fn direction_count_out_of_range_is_rejected() {
        let store_root = TempRoot::new("range");
        let store = StyleAnchorStore::new(store_root.path.clone());
        let gate = StyleGate::new(&store);
        let images = ScriptedImageProvider::new();
        for count in [0usize, 2, 6, 9] {
            assert!(
                gate.generate(
                    &facts(),
                    &SkinScanner::default(),
                    &images,
                    &options(count),
                    NOW
                )
                .is_err(),
                "方向数 {count} 应被拒"
            );
        }
        let mut zero_size = options(3);
        zero_size.preview_width = 0;
        assert!(
            gate.generate(&facts(), &SkinScanner::default(), &images, &zero_size, NOW)
                .is_err()
        );
    }

    /// 断点续跑：上一轮部分失败 → 记录已落盘，下一次只补缺的那个方向。
    #[test]
    fn failed_round_is_recorded_and_resumes_only_missing_directions() {
        let store_root = TempRoot::new("resume");
        let store = StyleAnchorStore::new(store_root.path.clone());
        let gate = StyleGate::new(&store);

        // 第一轮：图像通道整体不可用 → 生成失败，但记录必须在案。
        let broken = ScriptedImageProvider::new();
        broken.fail_with("图像 API 返回 500：上游故障");
        let error = gate
            .generate(&facts(), &SkinScanner::default(), &broken, &options(3), NOW)
            .expect_err("图像失败必须上抛（R7）");
        assert_eq!(error.kind, adm4_foundation::Adm4ErrorKind::AiUnavailable);
        assert!(error.message.contains("上游故障"), "{}", error.message);

        let session = store.require_session().expect("失败后工作态仍应在案");
        assert_eq!(session.rounds.len(), 1);
        assert_eq!(session.rounds[0].failures().len(), 3);
        assert_eq!(session.pending_style_ids().len(), 3);
        for item in &session.rounds[0].items {
            assert!(!item.succeeded());
            assert!(item.image_sha256.is_empty(), "失败不许留假指纹");
        }

        // 第二轮：通道恢复 → 续跑补齐三个方向，方向候选与提示词一字未改。
        let good = ScriptedImageProvider::new();
        let resumed = gate
            .generate(&facts(), &SkinScanner::default(), &good, &options(3), NOW)
            .expect("续跑");
        assert_eq!(resumed.rounds.len(), 2, "轮次记录只追加不改写");
        assert_eq!(resumed.rounds[1].round_id, "r0002");
        assert!(resumed.pending_style_ids().is_empty());
        assert_eq!(
            resumed.directions[0].derived_prompt,
            session.directions[0].derived_prompt
        );
        assert_eq!(good.calls().len(), 3);

        // 第三轮：全都有图了 → 不重复调用图像通道（不重复花钱）。
        let idle = ScriptedImageProvider::new();
        let again = gate
            .generate(&facts(), &SkinScanner::default(), &idle, &options(3), NOW)
            .expect("已齐备时应直接返回");
        assert!(idle.calls().is_empty(), "不该重复出图");
        assert_eq!(again.rounds.len(), 2, "没跑就不该多一轮记录");
    }

    /// 真源 revision 变了 → 方向重新派生（旧提示词锚的是旧设计）。
    #[test]
    fn changed_source_revision_rederives_directions() {
        let store_root = TempRoot::new("revision");
        let store = StyleAnchorStore::new(store_root.path.clone());
        let gate = StyleGate::new(&store);
        let images = ScriptedImageProvider::new();
        gate.generate(&facts(), &SkinScanner::default(), &images, &options(3), NOW)
            .expect("首轮");

        let mut moved = facts();
        moved.source_revision = 13;
        moved.entries[0].option_labels = vec!["网格防守".into()];
        let session = gate
            .generate(&moved, &SkinScanner::default(), &images, &options(3), NOW)
            .expect("真源变了应重新派生");
        assert_eq!(session.source_revision, 13);
        assert_eq!(session.rounds.len(), 1, "重新派生 = 新工作态，轮次从头计");
        assert!(session.directions[0].derived_prompt.contains("网格防守"));
        assert_eq!(images.calls().len(), 6, "两批各三张");
    }

    /// force 重生成：推翻方向、清掉全部旧预览（册 08 §2.3）。
    #[test]
    fn force_regeneration_clears_previous_previews() {
        let store_root = TempRoot::new("force");
        let store = StyleAnchorStore::new(store_root.path.clone());
        let gate = StyleGate::new(&store);
        let images = ScriptedImageProvider::new();
        let first = gate
            .generate(&facts(), &SkinScanner::default(), &images, &options(3), NOW)
            .expect("首轮");
        let mut forced = options(3);
        forced.force = true;
        let second = gate
            .generate(&facts(), &SkinScanner::default(), &images, &forced, NOW)
            .expect("推翻重生成");
        assert_eq!(second.rounds.len(), 1);
        for (before, after) in first.directions.iter().zip(second.directions.iter()) {
            assert_eq!(before.style_id, after.style_id);
            let before_round = before.preview.as_ref().map(|item| item.round_id.clone());
            let after_round = after.preview.as_ref().map(|item| item.round_id.clone());
            assert_eq!(before_round.as_deref(), Some("r0001"));
            assert_eq!(after_round.as_deref(), Some("r0001"));
        }
        assert_eq!(images.calls().len(), 6);
    }

    /// 对话式改词重生成：override 生效、可清空、次数不限、每轮留痕。
    #[test]
    fn regenerate_applies_and_clears_prompt_override() {
        let store_root = TempRoot::new("override");
        let store = StyleAnchorStore::new(store_root.path.clone());
        let gate = StyleGate::new(&store);
        let images = ScriptedImageProvider::new();
        let session = gate
            .generate(&facts(), &SkinScanner::default(), &images, &options(3), NOW)
            .expect("首轮");
        let style_id = session.directions[1].style_id.clone();
        let derived = session.directions[1].derived_prompt.clone();

        let updated = gate
            .regenerate(
                &style_id,
                "  colder palette, thicker outlines, night scene  ",
                &SkinScanner::default(),
                &images,
                NOW,
            )
            .expect("改词重生成");
        let direction = updated.direction(&style_id).expect("方向仍在");
        assert_eq!(
            direction.prompt_override, "colder palette, thicker outlines, night scene",
            "改词应 trim 后存下"
        );
        assert_eq!(direction.effective_prompt(), direction.prompt_override);
        assert_eq!(direction.derived_prompt, derived, "派生提示词不被改词覆盖");
        assert_eq!(updated.rounds.len(), 2);
        assert_eq!(updated.rounds[1].kind, StyleRoundKind::Regenerate);
        assert_eq!(updated.rounds[1].items.len(), 1);
        assert_eq!(
            updated.rounds[1].items[0].prompt,
            "colder palette, thicker outlines, night scene"
        );
        assert_eq!(
            direction
                .preview
                .as_ref()
                .map(|item| item.round_id.as_str()),
            Some("r0002")
        );

        // 次数不限：再改一次。
        let again = gate
            .regenerate(
                &style_id,
                "even colder",
                &SkinScanner::default(),
                &images,
                NOW,
            )
            .expect("再改一次");
        assert_eq!(again.rounds.len(), 3);
        assert_eq!(
            again.direction(&style_id).expect("方向").prompt_override,
            "even colder"
        );

        // 空串 = 清掉改词，回到锚定真源的派生提示词。
        let cleared = gate
            .regenerate(&style_id, "   ", &SkinScanner::default(), &images, NOW)
            .expect("清掉改词");
        let direction = cleared.direction(&style_id).expect("方向");
        assert!(direction.prompt_override.is_empty());
        assert_eq!(direction.effective_prompt(), derived);
        assert_eq!(cleared.rounds.len(), 4);

        // 未知方向 id / 无工作态时改词必须显式报错。
        assert!(
            gate.regenerate("STYLE-99-nope", "x", &SkinScanner::default(), &images, NOW)
                .is_err()
        );
        let other_root = TempRoot::new("override_empty");
        let other = StyleAnchorStore::new(other_root.path.clone());
        assert!(
            StyleGate::new(&other)
                .regenerate(&style_id, "x", &SkinScanner::default(), &images, NOW)
                .is_err()
        );
    }

    /// 改词重生成失败时：旧预览必须清掉（不让上一轮的图冒充这一轮的结果）。
    #[test]
    fn failed_regeneration_drops_the_stale_preview() {
        let store_root = TempRoot::new("stalepreview");
        let store = StyleAnchorStore::new(store_root.path.clone());
        let gate = StyleGate::new(&store);
        let images = ScriptedImageProvider::new();
        let session = gate
            .generate(&facts(), &SkinScanner::default(), &images, &options(3), NOW)
            .expect("首轮");
        let style_id = session.directions[0].style_id.clone();

        let broken = ScriptedImageProvider::new();
        broken.fail_with("图像 API 返回 429：额度耗尽");
        let error = gate
            .regenerate(&style_id, "warmer", &SkinScanner::default(), &broken, NOW)
            .expect_err("失败必须上抛");
        assert!(error.message.contains("额度耗尽"), "{}", error.message);
        let after = store.require_session().expect("工作态");
        let direction = after.direction(&style_id).expect("方向");
        assert!(direction.preview.is_none(), "失败后不许留旧图冒充新结果");
        assert_eq!(direction.prompt_override, "warmer", "改词照旧留痕");
        assert_eq!(after.rounds.len(), 2);
        // 未确认之下，其它方向的图不受影响（可停可续）。
        assert!(after.directions[1].preview.is_some());
    }

    /// R5：提示词命中换皮词一律拒（册 08 §5），派生与改词两条路都拦。
    #[test]
    fn prompts_hitting_skin_wordlist_are_rejected() {
        let store_root = TempRoot::new("skin");
        let store = StyleAnchorStore::new(store_root.path.clone());
        let gate = StyleGate::new(&store);
        let images = ScriptedImageProvider::new();

        // 派生路径：真源里的选项标签恰好是词表里的外部游戏名。
        let scanner = SkinScanner::new(vec!["晨昏防线".into()]);
        let mut tainted = facts();
        tainted.entries[0].option_labels = vec!["晨昏防线".into()];
        let error = gate
            .generate(&tainted, &scanner, &images, &options(3), NOW)
            .expect_err("命中换皮词必须拒");
        assert_eq!(error.kind, adm4_foundation::Adm4ErrorKind::RedLine);
        assert!(error.message.contains("晨昏防线"), "{}", error.message);
        assert!(images.calls().is_empty(), "拦下之后不该发生任何图像调用");

        // 改词路径：用户手打了参考游戏名。
        gate.generate(&facts(), &scanner, &images, &options(3), NOW)
            .expect("干净的真源应放行");
        let style_id = style_direction_id(0, "readable_production");
        let error = gate
            .regenerate(
                &style_id,
                "look exactly like 晨昏防线",
                &scanner,
                &images,
                NOW,
            )
            .expect_err("改词命中换皮词必须拒");
        assert_eq!(error.kind, adm4_foundation::Adm4ErrorKind::RedLine);
        // 被拒之后工作态不该被污染。
        let after = store.require_session().expect("工作态");
        assert!(
            after
                .direction(&style_id)
                .expect("方向")
                .prompt_override
                .is_empty()
        );
    }

    /// attended 确认：署名与结论双必填，无图不许确认（R3）。
    #[test]
    fn confirm_requires_signature_conclusion_and_a_real_image() {
        let store_root = TempRoot::new("confirm_guard");
        let store = StyleAnchorStore::new(store_root.path.clone());
        let gate = StyleGate::new(&store);

        // 无工作态时确认必须报错（不许凭空锁一版风格）。
        assert!(
            gate.confirm("STYLE-01-readable_production", "甲", "结论", NOW)
                .is_err()
        );

        let broken = ScriptedImageProvider::new();
        broken.fail_with("图像 API 不可用");
        let _ = gate.generate(&facts(), &SkinScanner::default(), &broken, &options(3), NOW);
        let style_id = style_direction_id(0, "readable_production");
        // 没出图的方向不许确认：风格门的意义就是看真图。
        let error = gate
            .confirm(&style_id, "主美甲", "就它了", NOW)
            .expect_err("无图不许确认");
        assert_eq!(error.kind, adm4_foundation::Adm4ErrorKind::Blocked);
        assert!(error.message.contains("看真图"), "{}", error.message);

        let good = ScriptedImageProvider::new();
        gate.generate(&facts(), &SkinScanner::default(), &good, &options(3), NOW)
            .expect("补图");
        for (actor, note) in [("  ", "结论"), ("主美甲", "  "), ("", "")] {
            let error = gate
                .confirm(&style_id, actor, note, NOW)
                .expect_err("署名/结论缺一不可");
            assert_eq!(error.kind, adm4_foundation::Adm4ErrorKind::RedLine);
            assert!(error.message.contains("R3"), "{}", error.message);
        }
        // 未知方向 id 同样报错。
        assert!(
            gate.confirm("STYLE-99-nope", "主美甲", "就它了", NOW)
                .is_err()
        );
        // 一路被拒之后仍旧未确认。
        assert!(!store.readiness().expect("就绪查询").ready);
    }

    /// 确认锁定：锚点集/应用契约/确认记录/适配报告四件产物齐备且互相对得上。
    #[test]
    fn confirm_locks_anchor_set_and_derives_application_contract() {
        let store_root = TempRoot::new("lock");
        let store = StyleAnchorStore::new(store_root.path.clone());
        let gate = StyleGate::new(&store);
        let images = ScriptedImageProvider::new();
        let session = gate
            .generate(&facts(), &SkinScanner::default(), &images, &options(3), NOW)
            .expect("生成");
        let style_id = session.directions[2].style_id.clone();
        gate.regenerate(
            &style_id,
            "moody dusk lighting",
            &SkinScanner::default(),
            &images,
            NOW,
        )
        .expect("改词");

        let outcome = gate
            .confirm(
                &style_id,
                "主美甲",
                "三个方向都看过大图，选它兼顾可读与氛围",
                NOW,
            )
            .expect("确认");
        assert_eq!(outcome.anchor_set.anchor_version, 1);
        assert_eq!(outcome.superseded_version, None);
        assert_eq!(outcome.anchor_set.selected_style_id, style_id);
        assert_eq!(outcome.anchor_set.final_prompt, "moody dusk lighting");
        assert!(outcome.anchor_set.prompt_overridden);
        assert_eq!(outcome.anchor_set.source_revision, 12);
        assert_eq!(outcome.anchor_set.source_anchors.len(), 2);
        assert_eq!(outcome.anchor_set.confirmation.actor, "主美甲");
        assert_eq!(
            outcome.anchor_set.confirmation.status,
            StyleApprovalStatus::Approved
        );
        assert_eq!(
            outcome.anchor_set.confirmation.mode,
            StyleConfirmMode::Manual
        );
        assert!(outcome.anchor_set.validate().is_ok());

        // 锚图落进版本目录，且指纹与预览一致。
        let anchor = outcome.anchor_set.selected_anchor().expect("选中锚图");
        assert_eq!(anchor.image_path, format!("anchors/v1/{style_id}.png"));
        // 锚图是**落盘字节的真指纹**，且与改词那一轮的预览图逐字节相同。
        let anchor_bytes = store.read_image(&anchor.image_path).expect("读锚图");
        assert_eq!(anchor.image_sha256, sha256_hex(&anchor_bytes));
        assert_eq!(anchor.image_bytes, anchor_bytes.len() as u64);
        let latest_preview = store
            .require_session()
            .expect("工作态")
            .direction(&style_id)
            .and_then(|item| item.preview.clone())
            .expect("改词后的预览");
        assert_eq!(latest_preview.round_id, "r0002");
        assert_eq!(
            store
                .read_image(&latest_preview.image_path)
                .expect("读预览"),
            anchor_bytes
        );
        assert_eq!(anchor.prompt, "moody dusk lighting");

        // 应用契约派生自锚点集，五类用途全覆盖。
        let contract = &outcome.application_contract;
        assert!(contract.validate().is_ok());
        assert!(contract.matches(&outcome.anchor_set).is_ok());
        assert_eq!(contract.anchor_version, 1);
        assert_eq!(contract.selected_style_id, style_id);
        assert_eq!(contract.prompt_prefix, "moody dusk lighting");
        assert_eq!(contract.palette, outcome.anchor_set.palette);
        assert_eq!(contract.style_constraints.len(), 5);
        for usage in StyleUsage::all() {
            let constraint = contract.constraint(usage).expect("五类用途全覆盖");
            assert!(!constraint.readability.is_empty());
            assert!(!constraint.contrast.is_empty());
        }
        assert!(!contract.application_rules.is_empty());

        // 派生是确定性的：同一份锚点集再派生一次，逐字段相同（除生成时刻）。
        let again = StyleApplicationContract::derive(&outcome.anchor_set, NOW).expect("再派生");
        assert_eq!(again, *contract);

        // 四件产物都能回读。
        assert_eq!(
            store.load_anchor_set(1).expect("锚点集"),
            outcome.anchor_set
        );
        assert_eq!(
            store.load_application_contract(1).expect("应用契约"),
            *contract
        );
        assert_eq!(
            store.load_confirmation(1).expect("确认记录"),
            outcome.anchor_set.confirmation
        );
        assert_eq!(store.load_fit_report(1).expect("适配报告").entries.len(), 3);

        // 就绪查询转为 Ready，下游不再被阻断。
        let readiness = store.readiness().expect("就绪查询");
        assert!(readiness.ready);
        assert_eq!(readiness.anchor_version, 1);
        assert_eq!(readiness.selected_style_id, style_id);
        assert_eq!(readiness.anchor_hash, contract.source_anchor_hash);
        assert!(readiness.require_ready().is_ok());
    }

    /// 锚点历史不可变：重选风格写 v2，v1 一个字节都不动（D4）。
    #[test]
    fn reselecting_style_writes_a_new_immutable_version() {
        let store_root = TempRoot::new("history");
        let store = StyleAnchorStore::new(store_root.path.clone());
        let gate = StyleGate::new(&store);
        let images = ScriptedImageProvider::new();
        let session = gate
            .generate(&facts(), &SkinScanner::default(), &images, &options(3), NOW)
            .expect("生成");
        let first_id = session.directions[0].style_id.clone();
        let second_id = session.directions[1].style_id.clone();

        gate.confirm(&first_id, "主美甲", "先定清晰量产", NOW)
            .expect("确认 v1");
        let v1_bytes = std::fs::read(store.absolute("anchors/v1/anchor_set.json")).expect("读 v1");
        let v1_anchor = std::fs::read(store.absolute(&format!("anchors/v1/{first_id}.png")))
            .expect("读 v1 锚图");

        // 重新选择：另立新版（旧版仍是历史事实）。
        let later = "2026-09-02T00:00:00Z";
        let outcome = gate
            .confirm(&second_id, "主美乙", "试玩后改走概念绘画", later)
            .expect("确认 v2");
        assert_eq!(outcome.anchor_set.anchor_version, 2);
        assert_eq!(outcome.superseded_version, Some(1));
        assert_eq!(outcome.anchor_set.selected_style_id, second_id);

        // v1 逐字节未变。
        assert_eq!(
            std::fs::read(store.absolute("anchors/v1/anchor_set.json")).expect("重读 v1"),
            v1_bytes
        );
        assert_eq!(
            std::fs::read(store.absolute(&format!("anchors/v1/{first_id}.png")))
                .expect("重读 v1 锚图"),
            v1_anchor
        );
        assert_eq!(
            store.load_anchor_set(1).expect("v1").selected_style_id,
            first_id
        );
        assert_eq!(store.anchor_versions().expect("版本清单"), vec![1, 2]);
        // 就绪查询指向最新一版。
        let readiness = store.readiness().expect("就绪查询");
        assert_eq!(readiness.anchor_version, 2);
        assert_eq!(readiness.selected_style_id, second_id);
    }

    /// 状态投影：未生成 / 已生成未确认 / 已确认三态，以及 stale 提醒。
    #[test]
    fn status_projects_all_three_phases() {
        let store_root = TempRoot::new("status");
        let store = StyleAnchorStore::new(store_root.path.clone());
        let gate = StyleGate::new(&store);
        let images = ScriptedImageProvider::new();

        let empty = gate.status(12).expect("未生成时也要能查");
        assert!(!empty.session_present);
        assert!(empty.directions.is_empty());
        assert!(!empty.readiness.ready);
        assert!(empty.anchor_versions.is_empty());

        let session = gate
            .generate(&facts(), &SkinScanner::default(), &images, &options(3), NOW)
            .expect("生成");
        let generated = gate.status(12).expect("状态");
        assert!(generated.session_present);
        assert_eq!(generated.directions.len(), 3);
        assert_eq!(generated.round_count, 1);
        assert_eq!(generated.latest_round_id, "r0001");
        assert!(!generated.session_stale);
        assert_eq!(
            generated
                .directions
                .iter()
                .filter(|row| row.recommended)
                .count(),
            1
        );
        for row in &generated.directions {
            assert!(!row.image_path.is_empty());
            assert!(row.last_failure.is_empty());
            assert!(!row.prompt_summary.is_empty());
            assert!(!row.fit_reason.is_empty());
            assert!(!row.is_selected, "未确认时没有选中项");
        }
        // 电影写实（信息密度代价高）必须标「需注意」。
        let cinematic = generated
            .directions
            .iter()
            .find(|row| row.style_id.ends_with("cinematic_realism"));
        assert!(cinematic.is_none(), "只取三个方向时还没到电影写实");
        let full = derive_fit_report(&facts(), &session.directions, &style_presets(), NOW);
        assert!(full.advisory_only);

        // 设计后来又变了 → stale 提醒（不阻断）。
        let stale = gate.status(13).expect("状态");
        assert!(stale.session_stale);

        let style_id = session.directions[0].style_id.clone();
        gate.confirm(&style_id, "主美甲", "定它", NOW)
            .expect("确认");
        let confirmed = gate.status(12).expect("状态");
        assert!(confirmed.readiness.ready);
        assert_eq!(confirmed.anchor_versions, vec![1]);
        assert_eq!(confirmed.confirmed_actor, "主美甲");
        assert_eq!(confirmed.confirmed_notes, "定它");
        assert!(!confirmed.anchor_stale);
        assert_eq!(
            confirmed
                .directions
                .iter()
                .filter(|row| row.is_selected)
                .count(),
            1
        );
        // 锚点锚的是 revision 12；设计推进到 14 之后锚点落后于设计（提示不阻断）。
        let drifted = gate.status(14).expect("状态");
        assert!(drifted.anchor_stale);
        assert!(drifted.readiness.ready, "落后不等于失效");
    }

    /// 五个预设全取时，适配报告必须把两个高密度代价方向标成「需注意」。
    #[test]
    fn fit_report_flags_high_density_presets_as_caution() {
        let report = derive_fit_report(&facts(), &[], &style_presets(), NOW);
        assert!(report.entries.is_empty(), "没有方向就没有条目");

        let store_root = TempRoot::new("fit");
        let store = StyleAnchorStore::new(store_root.path.clone());
        let gate = StyleGate::new(&store);
        let images = ScriptedImageProvider::new();
        let session = gate
            .generate(&facts(), &SkinScanner::default(), &images, &options(5), NOW)
            .expect("五方向");
        let caution: Vec<&str> = session
            .fit
            .entries
            .iter()
            .filter(|entry| entry.risk == StyleFitRisk::Caution)
            .map(|entry| entry.preset_key.as_str())
            .collect();
        assert_eq!(caution, vec!["concept_painting", "cinematic_realism"]);
        for entry in &session.fit.entries {
            assert!(!entry.reason.is_empty());
            assert_eq!(entry.anchors.len(), 2, "结论必须指得出真源出处");
        }
        // 推荐落在第一个「适配」方向上（清晰量产）。
        assert!(session.directions[0].recommended);
        assert!(session.directions[0].recommended_reason.contains("适配"));
    }

    /// 契约 serde 往返 + 旧档兼容 + auto_accept 一律解析失败（fail-closed）。
    #[test]
    fn contracts_round_trip_and_reject_auto_accept() {
        let store_root = TempRoot::new("serde");
        let store = StyleAnchorStore::new(store_root.path.clone());
        let gate = StyleGate::new(&store);
        let images = ScriptedImageProvider::new();
        let session = gate
            .generate(&facts(), &SkinScanner::default(), &images, &options(3), NOW)
            .expect("生成");
        let outcome = gate
            .confirm(&session.directions[0].style_id, "主美甲", "定它", NOW)
            .expect("确认");

        for (label, json) in [
            (
                "锚点集",
                serde_json::to_string_pretty(&outcome.anchor_set).expect("序列化"),
            ),
            (
                "应用契约",
                serde_json::to_string_pretty(&outcome.application_contract).expect("序列化"),
            ),
        ] {
            assert!(!json.is_empty(), "{label} 序列化不应为空");
        }
        let back: StyleAnchorSet =
            serde_json::from_str(&serde_json::to_string(&outcome.anchor_set).expect("序列化"))
                .expect("反序列化");
        assert_eq!(back, outcome.anchor_set);
        let back: StyleApplicationContract = serde_json::from_str(
            &serde_json::to_string(&outcome.application_contract).expect("序列化"),
        )
        .expect("反序列化");
        assert_eq!(back, outcome.application_contract);

        // 旧档：只有几个老字段的确认记录照旧可读（新字段落空）。
        let legacy: StyleConfirmation = serde_json::from_str(
            r#"{"selected_style_id":"STYLE-01-readable_production","notes":"定它","actor":"甲"}"#,
        )
        .expect("旧档确认记录应可解析");
        assert_eq!(legacy.status, StyleApprovalStatus::Approved);
        assert_eq!(legacy.mode, StyleConfirmMode::Manual);
        assert_eq!(legacy.anchor_version, 0);

        // auto_accept 在类型层面不存在 → 解析即失败（禁止自动通过，R3）。
        assert!(
            serde_json::from_str::<StyleConfirmation>(
                r#"{"mode":"auto_accept","selected_style_id":"STYLE-01-x","notes":"n","actor":"a"}"#
            )
            .is_err()
        );
        assert!(
            serde_json::from_str::<StyleConfirmation>(
                r#"{"status":"auto_approved","selected_style_id":"STYLE-01-x","notes":"n","actor":"a"}"#
            )
            .is_err()
        );
    }

    /// 锚点集/应用契约的自校验负例：每条拒绝都得真拦得住。
    #[test]
    fn anchor_set_and_contract_validation_negatives() {
        let store_root = TempRoot::new("validate");
        let store = StyleAnchorStore::new(store_root.path.clone());
        let gate = StyleGate::new(&store);
        let images = ScriptedImageProvider::new();
        let session = gate
            .generate(&facts(), &SkinScanner::default(), &images, &options(3), NOW)
            .expect("生成");
        let outcome = gate
            .confirm(&session.directions[0].style_id, "主美甲", "定它", NOW)
            .expect("确认");
        let good = outcome.anchor_set.clone();
        assert!(good.validate().is_ok());

        let mut broken = good.clone();
        broken.anchor_version = 0;
        assert!(broken.validate().is_err());

        let mut broken = good.clone();
        broken.selected_style_id = "readable_production".into();
        assert!(broken.validate().is_err());

        let mut broken = good.clone();
        broken.final_prompt = "  ".into();
        assert!(broken.validate().is_err());

        let mut broken = good.clone();
        broken.source_anchors.clear();
        assert_eq!(
            broken.validate().unwrap_err().kind,
            adm4_foundation::Adm4ErrorKind::RedLine
        );

        let mut broken = good.clone();
        broken.anchors.clear();
        assert!(broken.validate().is_err());

        let mut broken = good.clone();
        broken.anchors[0].image_sha256 = String::new();
        assert!(broken.validate().is_err());

        let mut broken = good.clone();
        broken.confirmation.actor = "  ".into();
        assert_eq!(
            broken.validate().unwrap_err().kind,
            adm4_foundation::Adm4ErrorKind::RedLine
        );

        let mut broken = good.clone();
        broken.confirmation.notes = String::new();
        assert!(broken.validate().is_err());

        let mut broken = good.clone();
        broken.confirmation.selected_style_id = "STYLE-02-other".into();
        assert!(broken.validate().is_err());

        // 预设表里没有的 preset_key 无法派生契约（不猜一套约束）。
        let mut unknown_preset = good.clone();
        unknown_preset.preset_key = "no_such_preset".into();
        assert_eq!(
            StyleApplicationContract::derive(&unknown_preset, NOW)
                .unwrap_err()
                .kind,
            adm4_foundation::Adm4ErrorKind::NotFound
        );

        // 契约侧：缺一类用途 / 用途重复 / 缺哈希 / 缺前缀。
        let mut contract = outcome.application_contract.clone();
        contract.style_constraints.pop();
        assert!(contract.validate().is_err());

        let mut contract = outcome.application_contract.clone();
        let duplicate = contract.style_constraints[0].clone();
        contract.style_constraints.push(duplicate);
        assert_eq!(
            contract.validate().unwrap_err().kind,
            adm4_foundation::Adm4ErrorKind::Conflict
        );

        let mut contract = outcome.application_contract.clone();
        contract.style_constraints[0].usage = StyleUsage::Unknown;
        assert!(contract.validate().is_err());

        let mut contract = outcome.application_contract.clone();
        contract.source_anchor_hash = String::new();
        assert!(contract.validate().is_err());

        let mut contract = outcome.application_contract.clone();
        contract.prompt_prefix = "   ".into();
        assert!(contract.validate().is_err());

        // 契约与锚点集对不上（锚点集改过而契约没重派生）。
        let mut drifted = good.clone();
        drifted.final_prompt = "someone edited this".into();
        assert_eq!(
            outcome
                .application_contract
                .matches(&drifted)
                .unwrap_err()
                .kind,
            adm4_foundation::Adm4ErrorKind::Conflict
        );
    }

    /// 就绪查询是**实读实校**：产物被人手改坏 → 未就绪并说清原因，而不是看状态位放行。
    #[test]
    fn readiness_rechecks_products_instead_of_trusting_a_flag() {
        let store_root = TempRoot::new("readiness");
        let store = StyleAnchorStore::new(store_root.path.clone());
        let gate = StyleGate::new(&store);
        let images = ScriptedImageProvider::new();
        let session = gate
            .generate(&facts(), &SkinScanner::default(), &images, &options(3), NOW)
            .expect("生成");
        gate.confirm(&session.directions[0].style_id, "主美甲", "定它", NOW)
            .expect("确认");
        assert!(store.readiness().expect("就绪").ready);

        // ① 锚点集被改坏（署名被抹掉）→ 未就绪。
        let mut anchor_set = store.load_anchor_set(1).expect("锚点集");
        anchor_set.confirmation.actor = String::new();
        write_json_file(&store.absolute("anchors/v1/anchor_set.json"), &anchor_set).expect("写回");
        let readiness = store.readiness().expect("就绪");
        assert!(!readiness.ready);
        assert!(readiness.detail.contains("R3"), "{}", readiness.detail);

        // ② 锚点集被改（哈希变了）而契约没重派生 → 未就绪。
        let mut anchor_set = store.load_anchor_set(1).expect("锚点集");
        anchor_set.confirmation.actor = "主美甲".into();
        anchor_set.final_prompt = "someone edited this".into();
        write_json_file(&store.absolute("anchors/v1/anchor_set.json"), &anchor_set).expect("写回");
        let readiness = store.readiness().expect("就绪");
        assert!(!readiness.ready);
        assert!(readiness.detail.contains("哈希"), "{}", readiness.detail);

        // ③ 只有目录没有 anchor_set.json 的半成品版本不算历史。
        ensure_dir(&store.absolute("anchors/v9")).expect("造半成品目录");
        assert_eq!(store.anchor_versions().expect("版本清单"), vec![1]);
    }

    #[test]
    fn ids_and_helpers_follow_the_documented_naming() {
        assert_eq!(
            style_direction_id(0, "readable_production"),
            "STYLE-01-readable_production"
        );
        assert_eq!(style_direction_id(9, "x"), "STYLE-10-x");
        assert_eq!(
            style_anchor_image_id("STYLE-01-x", SELECTED_ANCHOR_ROLE),
            "ANCHOR-STYLE-01-x-selected_preview"
        );
        assert_eq!(StyleUsage::all().len(), 5);
        assert_eq!(StyleUsage::Icon.label_zh(), "图标");
        assert_eq!(StyleUsage::Unknown.label_zh(), "未声明");
        assert_eq!(StyleFitRisk::Caution.label_zh(), "需注意");
        assert_eq!(style_presets().len(), MAX_DIRECTIONS);
        // 预设键唯一（否则方向 id 会撞）。
        let mut keys: Vec<&str> = style_presets()
            .iter()
            .map(|preset| preset.preset_key)
            .collect();
        keys.sort_unstable();
        let count = keys.len();
        keys.dedup();
        assert_eq!(keys.len(), count);

        let direction = StyleDirection {
            derived_prompt: "abc".into(),
            prompt_override: "  ".into(),
            ..StyleDirection::default()
        };
        assert_eq!(direction.effective_prompt(), "abc");
        assert_eq!(direction.prompt_summary(2), "ab…");
        assert_eq!(direction.prompt_summary(9), "abc");
    }
}
