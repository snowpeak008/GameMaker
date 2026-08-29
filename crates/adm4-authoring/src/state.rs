use adm4_contracts::ReviewProof;
use adm4_decision::{DecisionId, DepthProfile, GenrePackId, NaJustification, NodeId, Selection};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(tag = "mode", rename_all = "snake_case")]
pub enum TemplateMode {
    #[default]
    None,
    /// 预填：答卷整卷进入项目，provenance=Template，冻结前必须过换皮门。
    Prefilled { template_id: String },
    /// 对照：模板不进项目，仅侧栏展示。
    Compare { template_id: String },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct InterviewEntry {
    pub decision_id: DecisionId,
    pub role: String, // "ai_proposal" | "user_confirm" | "user_reject" | "user_skip"
    pub content: String,
    pub at: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct InterviewState {
    /// 拓扑序游标：下一个待访谈的决策点。
    pub cursor: Option<DecisionId>,
    pub transcript: Vec<InterviewEntry>,
    /// 本轮被用户拒绝的决策点：propose_next 不立刻重提，
    /// 直到同层其余待办处理完只剩它（D11：拒绝的提案留在待办）。
    #[serde(default)]
    pub skipped_this_round: BTreeSet<DecisionId>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FindingDisposition {
    /// 已修改设计。
    Fixed,
    /// 接受风险（记录在案）。
    RiskAccepted,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Finding {
    pub id: String,
    pub severity: String, // "blocker" | "warning"
    /// 发现指向的设计位置（决策点 id / 表名等）；无法定位的发现不可处置，解析时必填。
    /// 旧存档没有该字段（`serde(default)` → 空串），只影响展示不影响门禁判定。
    #[serde(default)]
    pub target: String,
    pub text: String,
    #[serde(default)]
    pub disposition: Option<FindingDisposition>,
    #[serde(default)]
    pub disposition_note: String,
}

/// 冻结门第 4 道的红队记录（AI 产出 + 用户逐条处置）。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RedTeamRecord {
    pub findings: Vec<Finding>,
    pub proof: ReviewProof,
    /// 红队评审针对的创作状态 revision；设计再变更即失效。
    pub reviewed_revision: u64,
}

/// 【遗留结构，只读兼容】人工豁免的署名。
///
/// F3 起署名合并进 `NaJustification::actor`/`at`（见该结构的说明）：并行 map 要求两处键
/// 始终同步，漏删就会留下「豁免已解除但署名还在」的幽灵记录。本结构只用于反序列化 F3
/// 之前的存档，`AuthoringState::adopt_legacy_na_signoffs` 把它合并进 `not_applicable`
/// 之后即清空，新代码不再写入。
///
/// 单条豁免不走 `ReviewProof`——那是批量评审的工作量证明（需要 reviewed/upstream 计数
/// 与逐类证据）；一条豁免的可追责性由「非空理由 + 非空说明 + 署名 + 时间」构成。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NaSignoff {
    /// 署名（人名/账号），不得为空。
    pub actor: String,
    pub at: String,
}

/// 设计期唯一权威状态。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AuthoringState {
    pub project_name: String,
    pub genre_pack: GenrePackId,
    pub pack_version: String,
    pub depth_profile: DepthProfile,
    pub selections: BTreeMap<DecisionId, Selection>,
    /// 显式 N/A（含理由码、说明与人工豁免署名，署名自 F3 起并入 `NaJustification`）。
    #[serde(default)]
    pub not_applicable: BTreeMap<DecisionId, NaJustification>,
    /// 【遗留字段，只读兼容】F3 之前的人工豁免署名并行 map。
    ///
    /// 反序列化后由 [`AuthoringState::adopt_legacy_na_signoffs`] 合并进 `not_applicable`
    /// 并清空；为空时不再序列化，因此新存档里不会出现这个键。
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub na_signoffs: BTreeMap<DecisionId, NaSignoff>,
    /// 按节点的设计说明（二版节点文本「设计说明」的归宿）。
    /// 决策点级的设计说明仍是 `Selection::rationale`，二者层级不同、互不替代。
    #[serde(default)]
    pub node_design_notes: BTreeMap<NodeId, String>,
    /// 按节点的风险说明（右栏「风险」页签的数据源之一）。
    #[serde(default)]
    pub node_risk_notes: BTreeMap<NodeId, String>,
    #[serde(default)]
    pub interview: InterviewState,
    #[serde(default)]
    pub template_mode: TemplateMode,
    #[serde(default)]
    pub red_team: Option<RedTeamRecord>,
    /// 每次变更 +1；红队记录/冻结版本以此判定是否过期。
    #[serde(default)]
    pub revision: u64,
    /// 已冻结的版本数（下一个冻结版本号 = frozen_versions + 1）。
    #[serde(default)]
    pub frozen_versions: u32,
}

impl AuthoringState {
    pub fn new(
        project_name: impl Into<String>,
        genre_pack: impl Into<String>,
        pack_version: impl Into<String>,
        depth_profile: DepthProfile,
    ) -> Self {
        Self {
            project_name: project_name.into(),
            genre_pack: genre_pack.into(),
            pack_version: pack_version.into(),
            depth_profile,
            selections: BTreeMap::new(),
            not_applicable: BTreeMap::new(),
            na_signoffs: BTreeMap::new(),
            node_design_notes: BTreeMap::new(),
            node_risk_notes: BTreeMap::new(),
            interview: InterviewState::default(),
            template_mode: TemplateMode::None,
            red_team: None,
            revision: 0,
            frozen_versions: 0,
        }
    }

    pub fn bump_revision(&mut self) {
        self.revision += 1;
    }

    /// 把 F3 之前的 `na_signoffs` 并行 map 合并进 `not_applicable` 的署名字段并清空它。
    ///
    /// 读入存档后立刻调用（`AuthoringEngine::new` 与 `AppServices::load_authoring_state`
    /// 各调一次），使「署名只有一个真相源」这件事对上层完全透明。
    /// 已有署名的条目不覆盖；`not_applicable` 里已没有的键直接丢弃（豁免已解除的幽灵署名）。
    /// 返回合并条数，便于调用方在日志里说明。
    pub fn adopt_legacy_na_signoffs(&mut self) -> usize {
        if self.na_signoffs.is_empty() {
            return 0;
        }
        let mut adopted = 0;
        for (decision_id, signoff) in std::mem::take(&mut self.na_signoffs) {
            if let Some(justification) = self.not_applicable.get_mut(&decision_id)
                && !justification.is_signed()
            {
                justification.actor = signoff.actor;
                justification.at = signoff.at;
                adopted += 1;
            }
        }
        adopted
    }
}
