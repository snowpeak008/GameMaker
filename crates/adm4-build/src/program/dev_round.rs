//! 开发轮次记录 + durable docs（册 09 §3）：可停可续的轮次日志与抵抗上下文丢失的四份文档。
//!
//! 为什么每轮都要落 `commands` / `failures`：proof-over-claims——一轮说"成功"必须能指回跑过什么、
//! 失败了什么、修了什么（R7）。日志用 JSON 原子写落到调用方给的目录，缺文件即空日志，字段缺失按
//! serde default，中途断线后 `load` 回来接着 `append_round` 就能续跑。
//!
//! durable docs 初稿只转述切片与清单里的事实；引擎相关内容由后置波次的插件补写。

use super::manifest::{
    DURABLE_ASSETS_FILE, DURABLE_PLAN_FILE, DURABLE_PROOF_FILE, DURABLE_STRUCTURE_FILE,
    RuntimeManifest,
};
use super::slice::PlayableSlice;
use adm4_foundation::{Adm4Error, Adm4Result, atomic_write, read_json_file, write_json_file};
use serde::{Deserialize, Serialize};
use std::path::Path;

/// 开发轮次日志的落盘文件名。
pub const DEV_ROUND_LOG_FILE: &str = "dev_round_log.json";

/// 轮次状态（封闭集合）。`Running` 是默认值：旧档缺键时读成"进行中"，不会把一轮凭空读成成功。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RoundStatus {
    /// 进行中
    #[default]
    Running,
    /// 成功
    Succeeded,
    /// 失败
    Failed,
    /// 中止
    Aborted,
}

impl RoundStatus {
    /// 中文标签（durable docs 与报告用）。
    pub fn label(self) -> &'static str {
        match self {
            Self::Running => "进行中",
            Self::Succeeded => "成功",
            Self::Failed => "失败",
            Self::Aborted => "中止",
        }
    }
}

/// 一轮开发：跑了什么、错了什么、修了什么、结果如何。
///
/// 时间戳是调用方传入的字符串（本模块不读时钟），保证日志内容可在测试里逐字节断言。
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct DevRound {
    /// 轮次序号，由 [`DevRoundLog::append_round`] 分配（从 1 起递增）。
    pub index: u32,
    pub started_at: String,
    pub finished_at: String,
    /// 本轮执行过的命令原文。
    pub commands: Vec<String>,
    /// 本轮失败原文（编译错误 / 运行错误 / proof 缺失），不许摘要成"有错"。
    pub failures: Vec<String>,
    /// 针对上一轮失败做了什么修复。
    pub repair_summary: String,
    pub status: RoundStatus,
}

impl DevRound {
    /// 一轮的自洽检查：宣称失败必须有失败记录（R7：不许口头失败，同样不许口头成功）。
    pub fn validate(&self) -> Adm4Result<()> {
        if self.status == RoundStatus::Failed && self.failures.is_empty() {
            return Err(Adm4Error::validation(format!(
                "第 {} 轮状态为失败但 failures 为空：失败必须落原文",
                self.index
            )));
        }
        if self.status == RoundStatus::Succeeded && self.commands.is_empty() {
            return Err(Adm4Error::validation(format!(
                "第 {} 轮状态为成功但没有执行任何命令：无证据的成功不可记录（R7）",
                self.index
            )));
        }
        Ok(())
    }
}

/// 四份 durable docs 的正文（文件名见 [`super::manifest`] 常量）。
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct DurableDocs {
    pub plan_md: String,
    pub structure_md: String,
    pub assets_md: String,
    pub proof_md: String,
}

impl DurableDocs {
    /// 四份文档按固定文件名落到目录（原子写）。
    pub fn write_to(&self, dir: &Path) -> Adm4Result<()> {
        for (name, body) in self.files() {
            atomic_write(&dir.join(name), body.as_bytes())?;
        }
        Ok(())
    }

    /// （文件名, 正文）对，顺序固定。
    pub fn files(&self) -> [(&'static str, &str); 4] {
        [
            (DURABLE_PLAN_FILE, self.plan_md.as_str()),
            (DURABLE_STRUCTURE_FILE, self.structure_md.as_str()),
            (DURABLE_ASSETS_FILE, self.assets_md.as_str()),
            (DURABLE_PROOF_FILE, self.proof_md.as_str()),
        ]
    }
}

/// 开发轮次日志（`dev_round_log.json`）。
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct DevRoundLog {
    pub rounds: Vec<DevRound>,
    pub durable: DurableDocs,
}

impl DevRoundLog {
    /// 追加一轮并分配序号（上一轮序号 + 1；空日志从 1 起）。传入的 `index` 被忽略并覆盖，
    /// 避免调用方手填序号造成重复或跳号。返回分配到的序号。
    pub fn append_round(&mut self, round: DevRound) -> Adm4Result<u32> {
        let index = self
            .rounds
            .last()
            .map(|last| last.index)
            .map_or(1, |last| last + 1);
        let round = DevRound { index, ..round };
        round.validate()?;
        self.rounds.push(round);
        Ok(index)
    }

    /// 最近一轮（续跑时看上轮状态与失败原文）。
    pub fn last_round(&self) -> Option<&DevRound> {
        self.rounds.last()
    }

    /// 从目录加载：缺文件 = 空日志（首轮），文件存在但解析失败 = Err（不许静默丢日志）。
    pub fn load(dir: &Path) -> Adm4Result<Self> {
        let path = dir.join(DEV_ROUND_LOG_FILE);
        if !path.exists() {
            return Ok(Self::default());
        }
        read_json_file(&path).map_err(|error| {
            Adm4Error::validation(format!(
                "开发轮次日志 {} 无法读取：{}",
                path.display(),
                error.message
            ))
        })
    }

    /// 原子写到目录（覆盖）。
    pub fn save(&self, dir: &Path) -> Adm4Result<()> {
        write_json_file(&dir.join(DEV_ROUND_LOG_FILE), self)
    }
}

fn section(out: &mut String, title: &str, lines: &[String]) {
    out.push_str("## ");
    out.push_str(title);
    out.push('\n');
    for line in lines {
        if !line.trim().is_empty() {
            out.push_str("- ");
            out.push_str(line);
            out.push('\n');
        }
    }
    out.push('\n');
}

fn single(value: &str) -> Vec<String> {
    vec![value.to_string()]
}

/// 从切片与清单渲染四份 durable docs 初稿。每条 `- ` 行都是切片或清单字段的原文。
///
/// `PLAYABLE_ASSETS.md` 初稿只列画面/资产两类 proof 要求与锚点：资产条目的真源是资产表，
/// 本函数拿不到它就不编（由 T-G4a-3 落盘时决定是否附资产表）。
pub fn render_durable_docs(slice: &PlayableSlice, manifest: &RuntimeManifest) -> DurableDocs {
    let anchors: Vec<String> = slice
        .anchors
        .iter()
        .map(|anchor| anchor.0.clone())
        .collect();
    let manifest_anchors: Vec<String> = manifest
        .anchors
        .iter()
        .map(|anchor| anchor.0.clone())
        .collect();

    let mut plan = String::from("# PLAYABLE_PLAN\n\n");
    section(&mut plan, "目标", &single(&manifest.goal));
    section(&mut plan, "核心循环", &single(&slice.core_loop));
    section(&mut plan, "主操作", &slice.primary_input);
    section(&mut plan, "玩家反馈", &slice.player_feedback);
    section(&mut plan, "主场景", &single(&slice.scene));
    section(&mut plan, "成败状态", &single(&slice.success_or_fail_state));
    section(&mut plan, "排除范围", &slice.excluded_scope);
    section(&mut plan, "失败修复循环", &manifest.repair_loop);
    section(&mut plan, "真源锚点", &anchors);

    let mut structure = String::from("# PLAYABLE_STRUCTURE\n\n");
    section(&mut structure, "可玩切片", &single(&manifest.slice_ref));
    section(
        &mut structure,
        "引擎指南",
        &single(&manifest.engine_guide_ref),
    );
    section(&mut structure, "durable 状态文件", &manifest.durable_files);
    section(&mut structure, "主场景", &single(&slice.scene));
    section(&mut structure, "主操作", &slice.primary_input);
    section(&mut structure, "排除范围", &slice.excluded_scope);

    let asset_requirements: Vec<String> = manifest
        .proof_requirements
        .iter()
        .filter(|item| item.starts_with("资产风险：") || item.starts_with("画面风险："))
        .cloned()
        .collect();
    let mut assets = String::from("# PLAYABLE_ASSETS\n\n");
    section(&mut assets, "画面与资产 proof 要求", &asset_requirements);
    section(&mut assets, "真源锚点", &manifest_anchors);

    let mut proof = String::from("# PLAYABLE_PROOF\n\n");
    section(&mut proof, "proof 要求", &manifest.proof_requirements);
    section(
        &mut proof,
        "成败状态",
        &single(&slice.success_or_fail_state),
    );
    section(&mut proof, "失败修复循环", &manifest.repair_loop);

    DurableDocs {
        plan_md: plan,
        structure_md: structure,
        assets_md: assets,
        proof_md: proof,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::program::manifest::render_runtime_manifest;
    use crate::program::slice::extract_playable_slice;
    use crate::program::slice::test_fixtures::{program, spec};
    use std::path::PathBuf;

    struct TempDir(PathBuf);

    impl TempDir {
        fn new(tag: &str) -> Self {
            let dir = std::env::temp_dir().join(format!(
                "adm4_dev_round_{tag}_{}_{}",
                std::process::id(),
                adm4_foundation::new_id("t")
            ));
            std::fs::create_dir_all(&dir).expect("建临时目录");
            Self(dir)
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    fn round(status: RoundStatus) -> DevRound {
        DevRound {
            index: 99,
            started_at: "2026-09-02T00:00:00Z".into(),
            finished_at: "2026-09-02T00:01:00Z".into(),
            commands: vec!["build".into()],
            failures: if status == RoundStatus::Failed {
                vec!["error: link failed".into()]
            } else {
                Vec::new()
            },
            repair_summary: String::new(),
            status,
        }
    }

    /// 验收 f：append_round 索引从 1 起递增，传入的 index 被覆盖。
    #[test]
    fn append_round_assigns_increasing_indices() {
        let mut log = DevRoundLog::default();
        assert_eq!(
            log.append_round(round(RoundStatus::Failed)).expect("追加"),
            1
        );
        assert_eq!(
            log.append_round(round(RoundStatus::Running)).expect("追加"),
            2
        );
        assert_eq!(
            log.append_round(round(RoundStatus::Succeeded))
                .expect("追加"),
            3
        );
        let indices: Vec<u32> = log.rounds.iter().map(|r| r.index).collect();
        assert_eq!(indices, vec![1, 2, 3]);
        assert_eq!(
            log.last_round().map(|r| r.status),
            Some(RoundStatus::Succeeded)
        );
    }

    /// R7：宣称失败必须有失败原文；宣称成功必须跑过命令。
    #[test]
    fn append_round_rejects_unproven_claims() {
        let mut log = DevRoundLog::default();
        let mut fake_fail = round(RoundStatus::Failed);
        fake_fail.failures.clear();
        assert!(
            log.append_round(fake_fail)
                .unwrap_err()
                .message
                .contains("failures")
        );
        let mut fake_success = round(RoundStatus::Succeeded);
        fake_success.commands.clear();
        assert!(
            log.append_round(fake_success)
                .unwrap_err()
                .message
                .contains("命令")
        );
        assert!(log.rounds.is_empty(), "被拒的轮次不得入库");
    }

    /// 验收 f：落盘/加载往返；缺文件加载为空；坏文件报错不吞。
    #[test]
    fn log_round_trips_through_disk_and_missing_file_is_empty() {
        let temp = TempDir::new("roundtrip");
        let empty = DevRoundLog::load(&temp.0).expect("缺文件应为空日志");
        assert_eq!(empty, DevRoundLog::default());

        let mut log = empty;
        log.append_round(round(RoundStatus::Failed)).expect("追加");
        log.durable.plan_md = "# PLAYABLE_PLAN\n".into();
        log.save(&temp.0).expect("保存");
        assert!(temp.0.join(DEV_ROUND_LOG_FILE).exists());

        let mut loaded = DevRoundLog::load(&temp.0).expect("加载");
        assert_eq!(loaded, log);
        // 续跑：加载后追加，序号接着上轮。
        assert_eq!(
            loaded
                .append_round(round(RoundStatus::Succeeded))
                .expect("续跑"),
            2
        );
        loaded.save(&temp.0).expect("再保存");
        let again = DevRoundLog::load(&temp.0).expect("再加载");
        assert_eq!(again.rounds.len(), 2);

        std::fs::write(temp.0.join(DEV_ROUND_LOG_FILE), "{not json").expect("写坏文件");
        let error = DevRoundLog::load(&temp.0).expect_err("坏文件必须报错");
        assert!(error.message.contains("无法读取"), "{}", error.message);
    }

    /// 旧档兼容：字段缺失按 default；status 缺键读成进行中而不是成功。
    #[test]
    fn legacy_log_reads_with_defaults() {
        let legacy: DevRoundLog =
            serde_json::from_str(r#"{"rounds":[{"index":1,"commands":["build"]}]}"#)
                .expect("旧档可读");
        assert_eq!(legacy.rounds[0].status, RoundStatus::Running);
        assert!(legacy.rounds[0].failures.is_empty());
        assert_eq!(legacy.durable, DurableDocs::default());
        let json = serde_json::to_string(&RoundStatus::Aborted).expect("序列化");
        assert_eq!(json, "\"aborted\"");
    }

    /// durable docs：四份初稿每条 `- ` 事实都能在切片或清单字段里找到；落盘文件名固定。
    #[test]
    fn durable_docs_only_contain_contract_facts_and_write_to_disk() {
        let extraction = extract_playable_slice(&spec(), &program()).expect("抽取");
        let manifest = render_runtime_manifest(&extraction.slice, &extraction.risk_plan);
        let docs = render_durable_docs(&extraction.slice, &manifest);

        let mut facts = manifest.facts();
        let slice = &extraction.slice;
        facts.push(slice.core_loop.clone());
        facts.push(slice.scene.clone());
        facts.push(slice.success_or_fail_state.clone());
        facts.extend(slice.primary_input.iter().cloned());
        facts.extend(slice.player_feedback.iter().cloned());
        facts.extend(slice.excluded_scope.iter().cloned());
        facts.extend(slice.anchors.iter().map(|a| a.0.clone()));

        let mut fact_lines = 0;
        for (name, body) in docs.files() {
            assert!(!body.is_empty(), "{name} 不得为空");
            for line in body.lines() {
                if let Some(fact) = line.strip_prefix("- ") {
                    fact_lines += 1;
                    assert!(
                        facts.iter().any(|known| known == fact),
                        "{name} 的事实「{fact}」在契约里找不到"
                    );
                } else {
                    assert!(line.is_empty() || line.starts_with('#'), "{name}: {line:?}");
                }
            }
        }
        assert!(fact_lines > 10);
        assert!(docs.assets_md.contains("资产风险："));
        assert!(docs.assets_md.contains("画面风险："));
        assert!(!docs.assets_md.contains("手感风险："));

        let temp = TempDir::new("durable");
        docs.write_to(&temp.0).expect("落盘");
        for (name, body) in docs.files() {
            let on_disk = std::fs::read_to_string(temp.0.join(name)).expect("读回");
            assert_eq!(on_disk, body);
        }
    }

    /// 确定性：同一切片/清单两次渲染逐字节相同。
    #[test]
    fn durable_docs_are_deterministic() {
        let extraction = extract_playable_slice(&spec(), &program()).expect("抽取");
        let manifest = render_runtime_manifest(&extraction.slice, &extraction.risk_plan);
        let first = render_durable_docs(&extraction.slice, &manifest);
        let second = render_durable_docs(&extraction.slice, &manifest);
        assert_eq!(first, second);
    }
}
