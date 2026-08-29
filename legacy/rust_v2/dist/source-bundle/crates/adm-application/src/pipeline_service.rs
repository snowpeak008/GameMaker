use crate::{DevFlowStepSpec, devflow_step_specs};
use adm_foundation::{AdmError, AdmResult, UtcTimestamp, fs as foundation_fs};
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

pub const DEVFLOW_REQUESTS_RELATIVE_PATH: &str = "pipeline/devflow_requests.adm";
pub const DEVFLOW_LAST_RANGE_RUN_RELATIVE_PATH: &str = "pipeline/last_range_run.adm";
pub const DEVFLOW_STOP_REQUEST_RELATIVE_PATH: &str = "pipeline/stop_request.adm";
pub const DEVFLOW_STEP07_STYLE_RELATIVE_PATH: &str = "pipeline/step07_style_confirmation.adm";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DevflowRangeRunRequest {
    pub request_id: String,
    pub archive_id: String,
    pub start_step_id: String,
    pub end_step_id: String,
    pub mapped_core_stage_ids: Vec<String>,
    pub created_at_ms: u128,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DevflowStopRequest {
    pub requested_at_ms: u128,
    pub reason: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Step07StyleConfirmation {
    pub archive_id: String,
    pub style_id: String,
    pub prompt: String,
    pub confirmed_at_ms: u128,
}

#[derive(Debug, Clone)]
pub struct PipelineService {
    data_root: PathBuf,
}

impl PipelineService {
    pub fn new(data_root: impl AsRef<Path>) -> Self {
        Self {
            data_root: data_root.as_ref().to_path_buf(),
        }
    }

    pub fn request_path(&self) -> PathBuf {
        self.data_root.join(DEVFLOW_REQUESTS_RELATIVE_PATH)
    }

    pub fn last_range_run_path(&self) -> PathBuf {
        self.data_root.join(DEVFLOW_LAST_RANGE_RUN_RELATIVE_PATH)
    }

    pub fn stop_request_path(&self) -> PathBuf {
        self.data_root.join(DEVFLOW_STOP_REQUEST_RELATIVE_PATH)
    }

    pub fn step07_style_path(&self) -> PathBuf {
        self.data_root.join(DEVFLOW_STEP07_STYLE_RELATIVE_PATH)
    }

    pub fn record_range_run_request(
        &self,
        archive_id: impl AsRef<str>,
        start_step: impl AsRef<str>,
        end_step: impl AsRef<str>,
    ) -> AdmResult<DevflowRangeRunRequest> {
        let start = parse_step_ref(start_step.as_ref())?;
        let end = parse_step_ref(end_step.as_ref())?;
        if start.index > end.index {
            return Err(AdmError::invalid_input(format!(
                "pipeline start step {} cannot be after end step {}",
                start.step_id, end.step_id
            )));
        }
        let mapped_core_stage_ids = core_stage_ids_for_range(start.index, end.index);
        let created_at_ms = UtcTimestamp::now().as_millis();
        let request = DevflowRangeRunRequest {
            request_id: format!("devflow_range_{created_at_ms}"),
            archive_id: sanitize_inline(archive_id.as_ref()),
            start_step_id: start.step_id.to_string(),
            end_step_id: end.step_id.to_string(),
            mapped_core_stage_ids,
            created_at_ms,
        };
        self.append_request(&request)?;
        Ok(request)
    }

    pub fn request_stop(&self, reason: impl AsRef<str>) -> AdmResult<DevflowStopRequest> {
        let request = DevflowStopRequest {
            requested_at_ms: UtcTimestamp::now().as_millis(),
            reason: required_inline("reason", reason.as_ref())?,
        };
        let document = format!(
            "# Pipeline Stop Request\nrequested_at_ms={}\nreason={}\n",
            request.requested_at_ms,
            sanitize_inline(&request.reason)
        );
        foundation_fs::write_string(self.stop_request_path(), &document)?;
        Ok(request)
    }

    pub fn record_range_run_summary(
        &self,
        request: &DevflowRangeRunRequest,
        archive_id: impl AsRef<str>,
        devflow_completed_count: usize,
        status: impl AsRef<str>,
    ) -> AdmResult<()> {
        let document = format!(
            "# DevFlow Last Range Run\narchive_id={}\nstart_step_id={}\nend_step_id={}\nmapped_core_stage_ids={}\ndevflow_completed_count={}\nstatus={}\nupdated_at_ms={}\n",
            sanitize_inline(archive_id.as_ref()),
            sanitize_inline(&request.start_step_id),
            sanitize_inline(&request.end_step_id),
            request.mapped_core_stage_ids.join(","),
            devflow_completed_count,
            required_inline("status", status.as_ref())?,
            UtcTimestamp::now().as_millis()
        );
        foundation_fs::write_string(self.last_range_run_path(), &document)
    }

    pub fn confirm_step07_style(
        &self,
        archive_id: impl AsRef<str>,
        style_id: impl AsRef<str>,
        prompt: impl AsRef<str>,
    ) -> AdmResult<Step07StyleConfirmation> {
        let confirmation = Step07StyleConfirmation {
            archive_id: sanitize_inline(archive_id.as_ref()),
            style_id: required_inline("style_id", style_id.as_ref())?,
            prompt: required_inline("prompt", prompt.as_ref())?,
            confirmed_at_ms: UtcTimestamp::now().as_millis(),
        };
        let document = format!(
            "# Step07 Style Confirmation\narchive_id={}\nstyle_id={}\nprompt={}\nconfirmed_at_ms={}\n",
            sanitize_inline(&confirmation.archive_id),
            sanitize_inline(&confirmation.style_id),
            sanitize_inline(&confirmation.prompt),
            confirmation.confirmed_at_ms
        );
        foundation_fs::write_string(self.step07_style_path(), &document)?;
        Ok(confirmation)
    }

    pub fn render_status(&self) -> AdmResult<String> {
        let request_text = if self.request_path().exists() {
            std::fs::read_to_string(self.request_path())?
        } else {
            "# DevFlow Requests\nrequest_count=0\n".to_string()
        };
        let stop_state = if self.stop_request_path().exists() {
            "stop_requested=true"
        } else {
            "stop_requested=false"
        };
        let style_state = if self.step07_style_path().exists() {
            "step07_style_confirmed=true"
        } else {
            "step07_style_confirmed=false"
        };
        let last_range_run_text = if self.last_range_run_path().exists() {
            std::fs::read_to_string(self.last_range_run_path())?
        } else {
            "# DevFlow Last Range Run\nstatus=none\n".to_string()
        };
        Ok(format!(
            "# Pipeline Service Status\n{}\n{}\n\n{}\n\n{}",
            stop_state, style_state, request_text, last_range_run_text
        ))
    }

    fn append_request(&self, request: &DevflowRangeRunRequest) -> AdmResult<()> {
        let mut document = if self.request_path().exists() {
            std::fs::read_to_string(self.request_path())?
        } else {
            "# DevFlow Requests\n".to_string()
        };
        if !document.ends_with('\n') {
            document.push('\n');
        }
        document.push_str(&format!(
            "- request_id={}; archive_id={}; start_step_id={}; end_step_id={}; mapped_core_stage_ids={}; created_at_ms={}\n",
            sanitize_inline(&request.request_id),
            sanitize_inline(&request.archive_id),
            sanitize_inline(&request.start_step_id),
            sanitize_inline(&request.end_step_id),
            request.mapped_core_stage_ids.join(","),
            request.created_at_ms
        ));
        foundation_fs::write_string(self.request_path(), &document)
    }
}

#[derive(Debug, Clone, Copy)]
struct ParsedStep<'a> {
    step_id: &'a str,
    index: usize,
}

fn parse_step_ref(value: &str) -> AdmResult<ParsedStep<'static>> {
    let normalized = value.trim().to_ascii_lowercase();
    let number_text = normalized
        .strip_prefix("step")
        .or_else(|| normalized.strip_prefix("步骤"))
        .unwrap_or(normalized.as_str());
    let index = number_text.parse::<usize>().map_err(|error| {
        AdmError::invalid_input(format!("invalid pipeline step: {value}; {error}"))
    })?;
    let specs = devflow_step_specs();
    let spec = specs.get(index).ok_or_else(|| {
        AdmError::invalid_input(format!("pipeline step out of range 0..14: {value}"))
    })?;
    Ok(ParsedStep {
        step_id: spec.step_id,
        index,
    })
}

fn core_stage_ids_for_range(start: usize, end: usize) -> Vec<String> {
    let mut seen = BTreeSet::new();
    let mut stages = Vec::new();
    for spec in devflow_step_specs()
        .iter()
        .skip(start)
        .take(end - start + 1)
    {
        push_unique_core_stage(spec, &mut seen, &mut stages);
    }
    stages
}

fn push_unique_core_stage(
    spec: &DevFlowStepSpec,
    seen: &mut BTreeSet<String>,
    stages: &mut Vec<String>,
) {
    if seen.insert(spec.core_stage_id.to_string()) {
        stages.push(spec.core_stage_id.to_string());
    }
}

fn required_inline(name: &str, value: &str) -> AdmResult<String> {
    let value = sanitize_inline(value);
    if value.is_empty() {
        return Err(AdmError::invalid_input(format!("{name} cannot be empty")));
    }
    Ok(value)
}

fn sanitize_inline(value: &str) -> String {
    value.replace(['\r', '\n', ';'], " ").trim().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use adm_foundation::SessionId;

    #[test]
    fn pipeline_service_records_range_stop_and_step07_style() {
        let root = std::env::temp_dir().join(format!(
            "adm_pipeline_service_{}_{}",
            std::process::id(),
            SessionId::generate()
        ));
        let service = PipelineService::new(&root);

        let request = service
            .record_range_run_request("archive_demo", "03", "10")
            .expect("range request");
        service
            .record_range_run_summary(&request, "archive_demo", 8, "completed")
            .expect("range summary");
        service.request_stop("user clicked stop").expect("stop");
        service
            .confirm_step07_style(
                "archive_demo",
                "style_clean_2d",
                "high contrast readable art",
            )
            .expect("style");
        let status = service.render_status().expect("status");

        assert_eq!(request.start_step_id, "step03");
        assert_eq!(request.end_step_id, "step10");
        assert_eq!(
            request.mapped_core_stage_ids,
            vec![
                "development".to_string(),
                "assets".to_string(),
                "sdk".to_string()
            ]
        );
        assert!(status.contains("stop_requested=true"));
        assert!(status.contains("step07_style_confirmed=true"));
        assert!(status.contains("devflow_completed_count=8"));
        assert!(status.contains("status=completed"));
        assert!(service.request_path().exists());
        assert!(service.last_range_run_path().exists());
        assert!(service.stop_request_path().exists());
        assert!(service.step07_style_path().exists());
        let _ = std::fs::remove_dir_all(root);
    }
}
