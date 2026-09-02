//! 引擎后端接缝（D17）：加一个引擎 = 加一个 [`EngineBackend`] 实现，运行骨架零改。
//!
//! ## 身份与预检（G1 定型）
//!
//! G1 只定型了两件当时就说得清的事：**后端身份**与**环境预检**。预检不是可有可无的礼貌接口：
//! 册 09 明确要求「环境缺失时诚实降级，不伪装成功」（R7）。[`EnginePreflight`] 就是那句话的
//! 机器形态——`ready=false` 带原因，调用方据此落 Blocked。
//!
//! ## 四个「干活」方法（G4a 补齐）
//!
//! 切片任务、开发轮次、运行结果、证据包四个契约在 [`types`] 里定型后，本 trait 补上册 09 §4
//! 的 `open_or_create_project` / `agent_develop` / `run_playmode` / `capture_proof`。
//! 两条边界不变：**只增不改**既有签名；接口本身零引擎语义。
//!
//! [`mcp`] 是通用的 MCP stdio 客户端协议层，任何走 MCP 的引擎后端都复用它；
//! [`mock::MockEngineBackend`] 是确定性回放后端，供接线与端到端测试在没有真机时跑通全链。
//! 具体引擎的后端实现属后置波次，放在本模块的子目录里；治理与运行骨架不认得任何一个具体
//! 引擎的名字。

pub mod mcp;
pub mod mock;
pub mod types;

use adm4_foundation::{Adm4Error, Adm4Result};
pub use mock::{MockCall, MockEngineBackend, MockEngineScript, SEED_FILE_NAME};
use serde::{Deserialize, Serialize};
use std::path::Path;
pub use types::{
    DevContext, DevRound, DevRoundStatus, EngineProjectSeed, ProofBundle, RunResult, SliceTask,
};

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

    /// 按种子在 `dir` 下打开或创建引擎工程（工程目录名取 `seed.project_dir_name`）。
    ///
    /// 幂等：工程已存在则打开，不得清空重建——续跑不能毁掉上一轮的现场。
    fn open_or_create_project(&self, seed: &EngineProjectSeed, dir: &Path) -> Adm4Result<()>;

    /// 围绕切片任务现场开发一轮，返回这一轮的真实记录（命令/失败/修复原文）。
    ///
    /// 一轮失败是 `Ok(DevRound { status: Failed, .. })`；`Err` 只用于「这一轮没法开始」。
    fn agent_develop(&self, task: &SliceTask, ctx: &DevContext) -> Adm4Result<DevRound>;

    /// 把工程跑起来（进游戏模式/启动运行时），返回启动事实与日志位置。
    fn run_playmode(&self, project: &Path) -> Adm4Result<RunResult>;

    /// 捕获运行证据（日志/截图/视频）。捕获不到要说清原因，不得伪装成功（R7）。
    fn capture_proof(&self, project: &Path) -> Adm4Result<ProofBundle>;
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

    fn open_or_create_project(&self, seed: &EngineProjectSeed, dir: &Path) -> Adm4Result<()> {
        Err(self.blocked(
            "open_or_create_project",
            format!(
                "种子 engine_id={} project_dir_name={} 目标目录={}",
                seed.engine_id,
                seed.project_dir_name,
                dir.display()
            ),
        ))
    }

    fn agent_develop(&self, task: &SliceTask, ctx: &DevContext) -> Adm4Result<DevRound> {
        Err(self.blocked(
            "agent_develop",
            format!(
                "轮次 {} 切片 {} 工程 {}",
                task.round_index,
                task.slice_ref,
                ctx.project_dir.display()
            ),
        ))
    }

    fn run_playmode(&self, project: &Path) -> Adm4Result<RunResult> {
        Err(self.blocked("run_playmode", format!("工程 {}", project.display())))
    }

    fn capture_proof(&self, project: &Path) -> Adm4Result<ProofBundle> {
        Err(self.blocked("capture_proof", format!("工程 {}", project.display())))
    }
}

impl NotConfiguredBackend {
    /// 统一的「没配」错误：带后端 id、被拒的操作、调用现场与配置时给的原因，方便定位是谁在没配引擎时试图干活。
    fn blocked(&self, operation: &str, context: String) -> Adm4Error {
        Adm4Error::blocked(format!(
            "引擎后端 {} 未配置，无法执行 {operation}（{context}）：{}",
            self.id, self.reason
        ))
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

    #[test]
    fn not_configured_backend_blocks_all_four_work_methods_with_reason() {
        let backend = NotConfiguredBackend::new("none", "配置里没有引擎");
        let seed = EngineProjectSeed {
            engine_id: "none".into(),
            project_dir_name: "proj".into(),
            ..EngineProjectSeed::default()
        };
        let dir = Path::new("some/dir");
        let task = SliceTask {
            slice_ref: "slice".into(),
            round_index: 0,
            ..SliceTask::default()
        };
        let ctx = DevContext::default();

        let errors = [
            backend
                .open_or_create_project(&seed, dir)
                .expect_err("open_or_create_project 应 Err"),
            backend
                .agent_develop(&task, &ctx)
                .expect_err("agent_develop 应 Err"),
            backend.run_playmode(dir).expect_err("run_playmode 应 Err"),
            backend
                .capture_proof(dir)
                .expect_err("capture_proof 应 Err"),
        ];
        for (error, operation) in errors.iter().zip([
            "open_or_create_project",
            "agent_develop",
            "run_playmode",
            "capture_proof",
        ]) {
            assert_eq!(error.kind, adm4_foundation::Adm4ErrorKind::Blocked);
            assert!(!error.message.is_empty());
            assert!(
                error.message.contains(operation),
                "消息应点名被拒操作：{}",
                error.message
            );
            assert!(
                error.message.contains("配置里没有引擎"),
                "消息应带配置时的原因：{}",
                error.message
            );
        }
    }

    #[test]
    fn backends_are_injectable_as_boxed_trait_objects() {
        // 接线层按 `Box<dyn EngineBackend>` 注入；这里锁定 trait 保持对象安全且两个内置后端都能装进去。
        let backends: Vec<Box<dyn EngineBackend>> = vec![
            Box::new(NotConfiguredBackend::new("none", "未配置")),
            Box::new(MockEngineBackend::new("mock", MockEngineScript::default())),
        ];
        let ids: Vec<&str> = backends.iter().map(|backend| backend.id()).collect();
        assert_eq!(ids, vec!["none", "mock"]);
        for backend in &backends {
            let preflight = backend.preflight().expect("预检本身应成功");
            assert!(!preflight.ready, "两者按构造参数都未就绪");
            assert!(!preflight.detail.is_empty());
        }
    }
}
