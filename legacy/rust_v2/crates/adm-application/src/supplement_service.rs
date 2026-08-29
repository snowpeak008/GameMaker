use adm_foundation::{AdmError, AdmResult};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SupplementTask {
    pub task_id: String,
    pub area: String,
    pub source_stage: String,
    pub title: String,
    pub rationale: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SupplementAnalysis {
    pub request: String,
    pub context_summary: String,
    pub tasks: Vec<SupplementTask>,
}

impl SupplementAnalysis {
    pub fn render(&self) -> String {
        let mut document = String::from("# Supplemental Development Analysis\n");
        document.push_str(&format!("request={}\n", sanitize_line(&self.request)));
        document.push_str(&format!(
            "context_summary={}\n",
            sanitize_line(&self.context_summary)
        ));
        document.push_str(&format!("task_count={}\n", self.tasks.len()));
        for task in &self.tasks {
            document.push_str(&format!(
                "- task_id={}; area={}; source_stage={}; title={}; rationale={}\n",
                task.task_id,
                task.area,
                task.source_stage,
                sanitize_line(&task.title),
                sanitize_line(&task.rationale)
            ));
        }
        document
    }
}

pub fn analyze_supplement_request(
    request: &str,
    context_summary: &str,
) -> AdmResult<SupplementAnalysis> {
    let request = request.trim();
    if request.is_empty() {
        return Err(AdmError::invalid_input(
            "supplement request cannot be empty",
        ));
    }
    let mut tasks = Vec::new();
    for (area, source_stage, title, rationale) in classify_request(request) {
        let index = tasks.len() + 1;
        tasks.push(SupplementTask {
            task_id: format!("supplement_{index:02}"),
            area: area.to_string(),
            source_stage: source_stage.to_string(),
            title: title.to_string(),
            rationale: rationale.to_string(),
        });
    }
    if tasks.is_empty() {
        tasks.push(SupplementTask {
            task_id: "supplement_01".to_string(),
            area: "design".to_string(),
            source_stage: "step02".to_string(),
            title: "补充设计评审".to_string(),
            rationale: "需求未命中明确领域，先进入设计冻结前的补充评审。".to_string(),
        });
    }
    Ok(SupplementAnalysis {
        request: request.to_string(),
        context_summary: sanitize_line(context_summary),
        tasks,
    })
}

fn classify_request(
    request: &str,
) -> Vec<(&'static str, &'static str, &'static str, &'static str)> {
    let lower = request.to_ascii_lowercase();
    let mut tasks = Vec::new();
    if contains_any(
        &lower,
        &["玩法", "关卡", "系统", "数值", "loop", "mechanic", "design"],
    ) {
        tasks.push((
            "design",
            "step02",
            "补充设计决策",
            "需求影响玩法或设计结构，需要回写设计冻结输入。",
        ));
    }
    if contains_any(
        &lower,
        &[
            "程序",
            "代码",
            "接口",
            "状态机",
            "存档",
            "backend",
            "api",
            "code",
        ],
    ) {
        tasks.push((
            "development",
            "step08",
            "补充程序开发计划",
            "需求影响实现任务、接口或运行状态，需要生成程序补充任务。",
        ));
    }
    if contains_any(
        &lower,
        &[
            "美术", "资源", "动画", "音效", "ui", "asset", "art", "audio",
        ],
    ) {
        tasks.push((
            "assets",
            "step09",
            "补充美术资源计划",
            "需求影响资源生产、视觉或音频，需要补充资产任务。",
        ));
    }
    if contains_any(
        &lower,
        &["sdk", "插件", "unity", "steam", "支付", "analytics"],
    ) {
        tasks.push((
            "sdk",
            "step10",
            "补充 SDK 集成项",
            "需求影响 SDK、插件或第三方能力，需要进入资源对齐。",
        ));
    }
    if contains_any(
        &lower,
        &[
            "打包", "发布", "构建", "验证", "验收", "build", "release", "package",
        ],
    ) {
        tasks.push((
            "packaging",
            "step14",
            "补充交付验证项",
            "需求影响构建、发布或验收，需要补充交付检查。",
        ));
    }
    tasks
}

fn contains_any(value: &str, needles: &[&str]) -> bool {
    needles.iter().any(|needle| value.contains(needle))
}

fn sanitize_line(value: &str) -> String {
    value.replace(['\r', '\n', ';'], " ").trim().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn supplement_analysis_classifies_development_and_assets() {
        let analysis =
            analyze_supplement_request("增加角色动画和存档接口", "current project").unwrap();

        assert_eq!(analysis.tasks.len(), 2);
        assert!(analysis.render().contains("area=development"));
        assert!(analysis.render().contains("area=assets"));
    }

    #[test]
    fn supplement_analysis_falls_back_to_design_review() {
        let analysis = analyze_supplement_request("补一个新的想法", "").unwrap();

        assert_eq!(analysis.tasks[0].source_stage, "step02");
        assert!(analysis.render().contains("task_count=1"));
    }
}
