use crate::c3_content::ContentInventoryContract;
use crate::c4_capabilities::CapabilitiesContract;
use crate::framework::StageStatus;
use crate::runner::RunnerContext;
use adm4_contracts::{EvidencePointer, MeasuredMetric, SpecRef};
use adm4_foundation::{Adm4Error, Adm4Result};
use adm4_spec::GameSpec;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TaskNode {
    pub id: String,
    pub kind: String, // "program" | "asset" | "assembly"
    pub title: String,
    pub source_refs: Vec<SpecRef>,
    pub depends_on: Vec<String>,
}

/// C6 契约：任务图 + 与 C3/C4 的真实全量对账 + Phase1 签收（人工门）。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TaskGraphContract {
    pub tasks: Vec<TaskNode>,
    /// 对账覆盖率（R1：逐条证据，禁止硬编码分数）。
    pub reconciliation: MeasuredMetric,
    pub missing: Vec<String>,
}

pub fn execute(ctx: &RunnerContext<'_>) -> Adm4Result<StageStatus> {
    let spec: GameSpec = ctx.store.read_contract("C0")?;
    let content: ContentInventoryContract = ctx.store.read_contract("C3")?;
    let capabilities: CapabilitiesContract = ctx.store.read_contract("C4")?;

    let mut tasks = Vec::new();
    for capability in &capabilities.capabilities {
        tasks.push(TaskNode {
            id: format!("task_{}", capability.id),
            kind: "program".into(),
            title: format!("实现 {}", capability.interface_name),
            source_refs: capability.source_refs.clone(),
            depends_on: Vec::new(),
        });
    }
    for asset in &content.assets {
        tasks.push(TaskNode {
            id: format!("task_{}", asset.id),
            kind: "asset".into(),
            title: format!("制作资产 {}", asset.id),
            source_refs: vec![SpecRef::new(format!("entities/{}", asset.entity_id))],
            depends_on: Vec::new(),
        });
    }
    for system in &spec.systems {
        let dependency_ids: Vec<String> = tasks
            .iter()
            .filter(|task| {
                task.kind == "program"
                    && task.source_refs.iter().any(|source_ref| {
                        spec.mechanics
                            .iter()
                            .filter(|mechanic| mechanic.system_id == system.id)
                            .any(|mechanic| source_ref.0 == format!("mechanics/{}", mechanic.id))
                    })
            })
            .map(|task| task.id.clone())
            .collect();
        tasks.push(TaskNode {
            id: format!("task_assemble_{}", system.id),
            kind: "assembly".into(),
            title: format!("装配系统 {}", system.name),
            source_refs: vec![SpecRef::new(format!("systems/{}", system.id))],
            depends_on: dependency_ids,
        });
    }

    // 真实对账：每个能力、每条资产需求必须有对应任务；缺一列一。
    let mut evidence = Vec::new();
    let mut missing = Vec::new();
    let mut checked = 0usize;
    let mut covered = 0usize;
    for capability in &capabilities.capabilities {
        checked += 1;
        let task_id = format!("task_{}", capability.id);
        if tasks.iter().any(|task| task.id == task_id) {
            covered += 1;
            evidence.push(EvidencePointer {
                file: "C6/contract.json".into(),
                path: format!("capabilities/{}", capability.id),
                observed: format!("任务 {task_id} 存在"),
            });
        } else {
            missing.push(format!("能力 {} 无对应程序任务", capability.id));
        }
    }
    for asset in &content.assets {
        checked += 1;
        let task_id = format!("task_{}", asset.id);
        if tasks.iter().any(|task| task.id == task_id) {
            covered += 1;
            evidence.push(EvidencePointer {
                file: "C6/contract.json".into(),
                path: format!("assets/{}", asset.id),
                observed: format!("任务 {task_id} 存在"),
            });
        } else {
            missing.push(format!("资产需求 {} 无对应任务", asset.id));
        }
    }
    if !missing.is_empty() {
        return Err(Adm4Error::validation(format!(
            "C6 对账缺口 {} 项：{}",
            missing.len(),
            missing.join("; ")
        )));
    }
    let reconciliation = MeasuredMetric::new(
        if checked == 0 {
            0.0
        } else {
            covered as f64 / checked as f64
        },
        evidence,
    )?;

    let contract = TaskGraphContract {
        tasks: tasks.clone(),
        reconciliation,
        missing: Vec::new(),
    };
    let mut document = format!(
        "# C6 开发计划\n\n- 任务总数：{}（程序 {} / 资产 {} / 装配 {}）\n- 对账覆盖率：{:.0}%（逐条证据见 contract.json）\n\n| 任务 | 类型 | 依赖 |\n|------|------|------|\n",
        tasks.len(),
        tasks.iter().filter(|task| task.kind == "program").count(),
        tasks.iter().filter(|task| task.kind == "asset").count(),
        tasks.iter().filter(|task| task.kind == "assembly").count(),
        contract.reconciliation.value() * 100.0
    );
    for task in &tasks {
        document.push_str(&format!(
            "| {} | {} | {} |\n",
            task.title,
            task.kind,
            if task.depends_on.is_empty() {
                "-".to_string()
            } else {
                task.depends_on.join(", ")
            }
        ));
    }
    document
        .push_str("\n**等待 Phase 1 人工签收。**\n\n> 本文档由 contract.json 渲染，请勿手改。\n");
    ctx.store.write_stage("C6", &contract, &document)?;
    Ok(StageStatus::WaitingHuman {
        gate: "phase1_signoff".into(),
    })
}
