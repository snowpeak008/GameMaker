use adm_new_application::RunLogService;
use adm_new_contracts::log::{LogEntry, LogLevel};
use adm_new_foundation::AdmResult;
use serde::{Deserialize, Serialize};

use crate::{CommandAdapterResult, handle_command};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ListLatestLogsRequest {
    pub limit: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReadLogEntriesRequest {
    #[serde(default)]
    pub level: Option<LogLevel>,
    #[serde(default)]
    pub limit: Option<usize>,
}

pub trait LogsCommandService {
    fn list_latest_logs(&self, request: &ListLatestLogsRequest) -> AdmResult<Vec<LogEntry>>;
    fn read_log_entries(&self, request: &ReadLogEntriesRequest) -> AdmResult<Vec<LogEntry>>;
    fn clear_logs(&mut self) -> AdmResult<Vec<LogEntry>>;
    fn export_log_jsonl(&self) -> AdmResult<String>;
}

impl LogsCommandService for RunLogService {
    fn list_latest_logs(&self, request: &ListLatestLogsRequest) -> AdmResult<Vec<LogEntry>> {
        Ok(self.latest(request.limit))
    }

    fn read_log_entries(&self, request: &ReadLogEntriesRequest) -> AdmResult<Vec<LogEntry>> {
        let mut entries = match &request.level {
            Some(level) => self.filter_level(level.clone()),
            None => self.latest(usize::MAX),
        };
        if let Some(limit) = request.limit {
            entries.truncate(limit);
        }
        Ok(entries)
    }

    fn clear_logs(&mut self) -> AdmResult<Vec<LogEntry>> {
        self.clear();
        Ok(Vec::new())
    }

    fn export_log_jsonl(&self) -> AdmResult<String> {
        Ok(self.export_jsonl())
    }
}

pub fn list_latest_logs<S>(
    service: &S,
    request: ListLatestLogsRequest,
) -> CommandAdapterResult<Vec<LogEntry>>
where
    S: LogsCommandService,
{
    handle_command(|| service.list_latest_logs(&request))
}

pub fn read_log_entries<S>(
    service: &S,
    request: ReadLogEntriesRequest,
) -> CommandAdapterResult<Vec<LogEntry>>
where
    S: LogsCommandService,
{
    handle_command(|| service.read_log_entries(&request))
}

pub fn export_log_jsonl<S>(service: &S) -> CommandAdapterResult<String>
where
    S: LogsCommandService,
{
    handle_command(|| service.export_log_jsonl())
}

pub fn clear_logs<S>(service: &mut S) -> CommandAdapterResult<Vec<LogEntry>>
where
    S: LogsCommandService,
{
    handle_command(|| service.clear_logs())
}

#[cfg(test)]
mod tests {
    use std::cell::Cell;
    use std::collections::BTreeMap;

    use super::*;

    #[test]
    fn log_commands_list_filter_and_export_jsonl() {
        let mut service = RunLogService::new();
        for index in 0..4 {
            service.write(LogEntry {
                timestamp: format!("unix:{index}"),
                level: if index % 2 == 0 {
                    LogLevel::Info
                } else {
                    LogLevel::Error
                },
                context: "pipeline".to_string(),
                message: format!("entry {index}"),
                source: "test".to_string(),
                metadata: BTreeMap::new(),
            });
        }

        let latest = list_latest_logs(&service, ListLatestLogsRequest { limit: 2 });
        assert_eq!(latest.data.unwrap()[0].message, "entry 3");

        let errors = read_log_entries(
            &service,
            ReadLogEntriesRequest {
                level: Some(LogLevel::Error),
                limit: None,
            },
        );
        assert_eq!(errors.data.unwrap().len(), 2);

        let jsonl = export_log_jsonl(&service);
        assert!(jsonl.data.unwrap().contains("\"level\":\"ERROR\""));

        let cleared = clear_logs(&mut service);
        assert!(cleared.ok);
        assert!(
            list_latest_logs(&service, ListLatestLogsRequest { limit: 5 })
                .data
                .unwrap()
                .is_empty()
        );
    }

    #[test]
    fn logs_command_wrapper_calls_service_trait_mock() {
        let service = MockLogsService {
            latest_calls: Cell::new(0),
        };
        let response = list_latest_logs(&service, ListLatestLogsRequest { limit: 1 });
        assert!(response.ok);
        assert_eq!(service.latest_calls.get(), 1);
    }

    struct MockLogsService {
        latest_calls: Cell<usize>,
    }

    impl LogsCommandService for MockLogsService {
        fn list_latest_logs(&self, _: &ListLatestLogsRequest) -> AdmResult<Vec<LogEntry>> {
            self.latest_calls.set(self.latest_calls.get() + 1);
            Ok(Vec::new())
        }

        fn read_log_entries(&self, _: &ReadLogEntriesRequest) -> AdmResult<Vec<LogEntry>> {
            Ok(Vec::new())
        }

        fn clear_logs(&mut self) -> AdmResult<Vec<LogEntry>> {
            Ok(Vec::new())
        }

        fn export_log_jsonl(&self) -> AdmResult<String> {
            Ok(String::new())
        }
    }
}
