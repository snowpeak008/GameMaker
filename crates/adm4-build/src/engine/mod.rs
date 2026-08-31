//! 引擎后端接缝（D17）：加一个引擎 = 加一个 [`EngineBackend`] 实现，运行骨架零改。
//!
//! ## 本波只声明能诚实定型的部分
//!
//! 册 09 给出的完整后端接口还包含「现场开发一轮」「跑起来」「捕获运行证据」三个方法，
//! 它们的入参/返回类型（切片任务、开发轮次、证据包）都是 G4/G5 的制品契约。本波先编一组
//! 空壳类型摆在这里，只会让 G4 拿到一份必须推翻的接口——那比留白更糟。
//! 所以这里只定型两件现在就说得清的事：**后端身份**与**环境预检**。
//!
//! 预检不是可有可无的礼貌接口：册 09 明确要求「环境缺失时诚实降级，不伪装成功」（R7）。
//! [`EnginePreflight`] 就是那句话的机器形态——`ready=false` 带原因，调用方据此落 Blocked。
//!
//! 具体引擎的实现（含其 MCP 驱动）属 G4，放在本模块的子目录里；治理与运行骨架不认得任何
//! 一个具体引擎的名字。

use adm4_foundation::Adm4Result;
use serde::{Deserialize, Serialize};

/// 引擎环境预检结论。
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct EnginePreflight {
    pub backend_id: String,
    /// 环境是否就绪。就绪与否都必须给出 `detail`：说不清理由的「就绪」没有价值。
    pub ready: bool,
    pub detail: String,
}

/// 引擎后端接缝。
pub trait EngineBackend {
    /// 后端标识（进日志与制品，便于追溯「这一版是谁跑的」）。
    fn id(&self) -> &str;

    /// 环境预检：能不能在这台机器上真的驱动这个引擎。
    ///
    /// 返回 `Err` 只用于「预检本身没跑成」（如读配置失败）；
    /// 「环境不满足」是一条**正常结论**，走 `Ok(EnginePreflight { ready: false, .. })`。
    fn preflight(&self) -> Adm4Result<EnginePreflight>;
}

/// 未配置的后端：任何时候都如实报「没配」。
///
/// 它存在的意义是让「没接引擎」这件事有一个可注入、可断言的对象，
/// 而不是在骨架里写 `if backend.is_none()` 到处兜底。
pub struct NotConfiguredBackend {
    id: String,
    reason: String,
}

impl NotConfiguredBackend {
    pub fn new(id: &str, reason: &str) -> Self {
        Self {
            id: id.to_string(),
            reason: reason.to_string(),
        }
    }
}

impl EngineBackend for NotConfiguredBackend {
    fn id(&self) -> &str {
        &self.id
    }

    fn preflight(&self) -> Adm4Result<EnginePreflight> {
        Ok(EnginePreflight {
            backend_id: self.id.clone(),
            ready: false,
            detail: self.reason.clone(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn not_configured_backend_reports_honestly() {
        let backend = NotConfiguredBackend::new("none", "本波未接任何引擎后端（G4 补）");
        let preflight = backend.preflight().expect("预检本身应成功");
        assert_eq!(preflight.backend_id, "none");
        assert!(!preflight.ready);
        assert!(!preflight.detail.is_empty(), "不就绪必须说清原因");
        assert_eq!(backend.id(), "none");
    }

    #[test]
    fn preflight_round_trips_and_tolerates_legacy_records() {
        let preflight = EnginePreflight {
            backend_id: "none".into(),
            ready: false,
            detail: "未配置".into(),
        };
        let json = serde_json::to_string(&preflight).expect("序列化");
        assert_eq!(
            serde_json::from_str::<EnginePreflight>(&json).expect("反序列化"),
            preflight
        );
        let legacy: EnginePreflight =
            serde_json::from_str(r#"{"backend_id":"none"}"#).expect("旧档应可解析");
        assert!(!legacy.ready, "缺字段时按未就绪读（fail-closed）");
    }
}
