use crate::provider::{AiCapability, AiProvider, AiRequest, AiResponse};
use adm4_foundation::{Adm4Error, Adm4Result};
use std::collections::BTreeMap;
use std::sync::Mutex;

/// 确定性脚本 Provider：按 purpose 回放固定应答。测试/离线演示用，杜绝测试依赖真实网络。
pub struct ScriptedProvider {
    responses: Mutex<BTreeMap<String, Vec<String>>>,
}

impl ScriptedProvider {
    pub fn new() -> Self {
        Self {
            responses: Mutex::new(BTreeMap::new()),
        }
    }

    /// 为某 purpose 注册应答队列（按序弹出，弹尽复用最后一条）。
    ///
    /// 锁中毒（此前有线程持锁 panic）时取回内部数据继续注册，不再 panic：
    /// 队列本身是纯数据，中毒的锁不代表数据损坏。
    pub fn script(&self, purpose: &str, responses: Vec<String>) {
        let mut guard = match self.responses.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        };
        guard.insert(purpose.to_string(), responses);
    }
}

impl Default for ScriptedProvider {
    fn default() -> Self {
        Self::new()
    }
}

impl AiProvider for ScriptedProvider {
    fn id(&self) -> &str {
        "scripted"
    }

    fn capabilities(&self) -> &[AiCapability] {
        &[
            AiCapability::Text,
            AiCapability::Structured,
            AiCapability::Review,
        ]
    }

    fn invoke(&self, request: &AiRequest) -> Adm4Result<AiResponse> {
        let mut guard = self.responses.lock().map_err(|_| {
            Adm4Error::internal(
                "脚本 Provider 的应答队列锁已中毒（此前有线程 panic），无法回放应答",
            )
        })?;
        let queue = guard.get_mut(&request.purpose).ok_or_else(|| {
            Adm4Error::ai_unavailable(format!(
                "scripted provider has no response for purpose {}",
                request.purpose
            ))
        })?;
        let text = if queue.len() > 1 {
            queue.remove(0)
        } else {
            queue
                .first()
                .cloned()
                .ok_or_else(|| Adm4Error::ai_unavailable("scripted response queue empty"))?
        };
        Ok(AiResponse {
            text,
            provider_id: "scripted".into(),
            model: "scripted".into(),
        })
    }
}
