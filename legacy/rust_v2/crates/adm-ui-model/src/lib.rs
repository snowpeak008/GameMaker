#![forbid(unsafe_code)]

use adm_foundation::SessionId;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShellState {
    pub session_id: SessionId,
    pub current_view: ShellView,
    pub status_text: String,
    pub projects: Vec<ProjectListItem>,
    pub pipeline: PipelineStatusView,
    pub ai: AiStatusView,
    pub ai_diagnostics: AiDiagnosticsView,
    pub package: PackageStatusView,
    pub validation: ValidationStatusView,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShellView {
    Startup,
    ArchiveManager,
    DesignWorkbench,
    Pipeline,
    SdkKnowledge,
    Settings,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectListItem {
    pub archive_id: String,
    pub display_name: String,
    pub locked: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PipelineStatusView {
    pub active_stage: Option<String>,
    pub status: String,
    pub needs_ai_intervention: bool,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AiStatusView {
    pub record_count: usize,
    pub accepted_count: usize,
    pub failed_count: usize,
    pub rejected_count: usize,
    pub intervention: bool,
    pub failure_summary: String,
    pub last_error: String,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AiProviderDiagnosticsItem {
    pub provider_id: String,
    pub readiness: String,
    pub capabilities: String,
    pub notes: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AiDiagnosticsView {
    pub default_budget_units: u32,
    pub retry_max_attempts: u32,
    pub ready_provider_count: usize,
    pub provider_count: usize,
    pub providers: Vec<AiProviderDiagnosticsItem>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackageStatusView {
    pub entry_count: usize,
    pub support_file_count: usize,
    pub artifact_count: usize,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidationStatusView {
    pub status: String,
    pub issue_count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StageProgressItem {
    pub stage_id: String,
    pub label: String,
    pub status: String,
    pub artifact_count: usize,
    pub message: String,
}

impl Default for PipelineStatusView {
    fn default() -> Self {
        Self {
            active_stage: None,
            status: "idle".to_string(),
            needs_ai_intervention: false,
            message: String::new(),
        }
    }
}

impl Default for AiStatusView {
    fn default() -> Self {
        Self {
            record_count: 0,
            accepted_count: 0,
            failed_count: 0,
            rejected_count: 0,
            intervention: false,
            failure_summary: String::new(),
            last_error: String::new(),
            message: "AI idle".to_string(),
        }
    }
}

impl Default for AiDiagnosticsView {
    fn default() -> Self {
        Self {
            default_budget_units: 0,
            retry_max_attempts: 0,
            ready_provider_count: 0,
            provider_count: 0,
            providers: Vec::new(),
        }
    }
}

impl Default for PackageStatusView {
    fn default() -> Self {
        Self {
            entry_count: 0,
            support_file_count: 0,
            artifact_count: 0,
            message: "Package not inspected".to_string(),
        }
    }
}

impl Default for ValidationStatusView {
    fn default() -> Self {
        Self {
            status: "not inspected".to_string(),
            issue_count: 0,
        }
    }
}

impl PipelineStatusView {
    pub fn render(&self) -> String {
        format!("Pipeline: {}\n{}", self.status, self.message)
    }
}

impl AiStatusView {
    pub fn render(&self) -> String {
        let mut lines = vec![
            format!("AI: records={}", self.record_count),
            format!("accepted={}", self.accepted_count),
            format!("failed={}", self.failed_count),
            format!("rejected={}", self.rejected_count),
            format!("intervention={}", self.intervention),
        ];
        if !self.failure_summary.is_empty() {
            lines.push(format!("failures={}", self.failure_summary));
        }
        if !self.last_error.is_empty() {
            lines.push(format!("last_error={}", self.last_error));
        }
        if !self.message.is_empty() {
            lines.push(format!("message={}", self.message));
        }
        lines.join("\n")
    }
}

impl AiDiagnosticsView {
    pub fn render(&self) -> String {
        let mut lines = vec![
            format!(
                "AI Config: ready_provider_count={}",
                self.ready_provider_count
            ),
            format!("provider_count={}", self.provider_count),
            format!("default_budget_units={}", self.default_budget_units),
            format!("retry_max_attempts={}", self.retry_max_attempts),
        ];
        for provider in &self.providers {
            lines.push(format!(
                "{} | {} | {} | {}",
                provider.provider_id, provider.readiness, provider.capabilities, provider.notes
            ));
        }
        lines.join("\n")
    }
}

impl PackageStatusView {
    pub fn render(&self) -> String {
        if self.artifact_count > 0 {
            format!(
                "Package: artifacts={}\nentries={}\nsupport_files={}",
                self.artifact_count, self.entry_count, self.support_file_count
            )
        } else {
            format!(
                "Package: entries={}\nsupport_files={}",
                self.entry_count, self.support_file_count
            )
        }
    }
}

impl ValidationStatusView {
    pub fn render(&self) -> String {
        format!("Validation: {}\nissues={}", self.status, self.issue_count)
    }
}

impl StageProgressItem {
    pub fn render(&self) -> String {
        if self.message.is_empty() {
            format!(
                "{}: {} | artifacts={}",
                self.label, self.status, self.artifact_count
            )
        } else {
            format!(
                "{}: {} | artifacts={} | {}",
                self.label, self.status, self.artifact_count, self.message
            )
        }
    }
}

pub fn render_stage_progress(items: &[StageProgressItem]) -> String {
    if items.is_empty() {
        return "Stages: not run".to_string();
    }
    items
        .iter()
        .map(StageProgressItem::render)
        .collect::<Vec<_>>()
        .join("\n")
}

impl ShellState {
    pub fn blank(session_id: SessionId) -> Self {
        Self {
            session_id,
            current_view: ShellView::Startup,
            status_text: "ready".to_string(),
            projects: Vec::new(),
            pipeline: PipelineStatusView::default(),
            ai: AiStatusView::default(),
            ai_diagnostics: AiDiagnosticsView::default(),
            package: PackageStatusView::default(),
            validation: ValidationStatusView::default(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{AiDiagnosticsView, AiProviderDiagnosticsItem, AiStatusView};

    #[test]
    fn ai_status_render_includes_failure_details_when_present() {
        let view = AiStatusView {
            record_count: 3,
            accepted_count: 1,
            failed_count: 2,
            rejected_count: 0,
            intervention: true,
            failure_summary: "provider_unavailable=2".to_string(),
            last_error: "timeout".to_string(),
            message: String::new(),
        };

        let rendered = view.render();

        assert!(rendered.contains("failed=2"));
        assert!(rendered.contains("failures=provider_unavailable=2"));
        assert!(rendered.contains("last_error=timeout"));
    }

    #[test]
    fn ai_diagnostics_render_includes_provider_readiness() {
        let view = AiDiagnosticsView {
            default_budget_units: 8,
            retry_max_attempts: 2,
            ready_provider_count: 1,
            provider_count: 2,
            providers: vec![
                AiProviderDiagnosticsItem {
                    provider_id: "mock".to_string(),
                    readiness: "Ready".to_string(),
                    capabilities: "text_generation".to_string(),
                    notes: "local".to_string(),
                },
                AiProviderDiagnosticsItem {
                    provider_id: "remote".to_string(),
                    readiness: "MissingSecret".to_string(),
                    capabilities: "text_generation,structured_output".to_string(),
                    notes: "secret missing".to_string(),
                },
            ],
        };

        let rendered = view.render();

        assert!(rendered.contains("ready_provider_count=1"));
        assert!(rendered.contains("default_budget_units=8"));
        assert!(rendered.contains(
            "remote | MissingSecret | text_generation,structured_output | secret missing"
        ));
    }
}
