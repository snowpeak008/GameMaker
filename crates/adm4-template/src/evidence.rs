use crate::model::SourceType;
use adm4_foundation::Adm4Result;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EvidenceQuery {
    pub game_name: String,
    pub decision_question: String,
    pub keywords: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EvidenceCandidate {
    pub source_url: String,
    pub title: String,
    pub snippet: String,
    pub source_type: SourceType,
    /// 抓取内容哈希（缓存与复核用）。
    pub fetched_hash: String,
}

/// 联网证据检索通道（接口先行；自动实现落地时二选一：CLI 包装 / 搜索 API）。
pub trait EvidenceSearchChannel: Send + Sync {
    fn channel_id(&self) -> &str;
    fn search(&self, query: &EvidenceQuery) -> Adm4Result<Vec<EvidenceCandidate>>;
}

/// 人工供证通道：审核界面「贴来源」入口积累的候选（低置信与 L6 数值层强制走此通道）。
#[derive(Debug, Default)]
pub struct ManualEvidenceChannel {
    candidates: Vec<(String, EvidenceCandidate)>,
}

impl ManualEvidenceChannel {
    pub fn submit(&mut self, game_name: &str, candidate: EvidenceCandidate) {
        self.candidates.push((game_name.to_string(), candidate));
    }
}

impl EvidenceSearchChannel for ManualEvidenceChannel {
    fn channel_id(&self) -> &str {
        "manual"
    }

    fn search(&self, query: &EvidenceQuery) -> Adm4Result<Vec<EvidenceCandidate>> {
        Ok(self
            .candidates
            .iter()
            .filter(|(game, _)| game == &query.game_name)
            .map(|(_, candidate)| candidate.clone())
            .collect())
    }
}
