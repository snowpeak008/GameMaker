use adm4_contracts::ReviewProof;
use adm4_decision::{DecisionId, DepthProfile, GenrePackId, NaJustification, Selection};
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

/// 设计期唯一权威状态。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AuthoringState {
    pub project_name: String,
    pub genre_pack: GenrePackId,
    pub pack_version: String,
    pub depth_profile: DepthProfile,
    pub selections: BTreeMap<DecisionId, Selection>,
    #[serde(default)]
    pub not_applicable: BTreeMap<DecisionId, NaJustification>,
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
}
