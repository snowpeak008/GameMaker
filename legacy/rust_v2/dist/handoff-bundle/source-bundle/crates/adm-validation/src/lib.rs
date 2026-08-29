#![forbid(unsafe_code)]

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ValidationStatus {
    Passed,
    Warning,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidationIssue {
    pub status: ValidationStatus,
    pub code: String,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidationReport {
    pub status: ValidationStatus,
    pub issues: Vec<ValidationIssue>,
}

impl ValidationReport {
    pub fn from_issues(issues: Vec<ValidationIssue>) -> Self {
        Self {
            status: status_for_issues(&issues),
            issues,
        }
    }

    pub fn passed() -> Self {
        Self {
            status: ValidationStatus::Passed,
            issues: Vec::new(),
        }
    }

    pub fn render(&self) -> String {
        let mut document = String::from("# Validation Report\n");
        document.push_str(&format!("status={:?}\n", self.status));
        for issue in &self.issues {
            document.push_str(&format!(
                "- status={:?}; code={}; message={}\n",
                issue.status, issue.code, issue.message
            ));
        }
        document
    }
}

pub fn status_for_issues(issues: &[ValidationIssue]) -> ValidationStatus {
    if issues
        .iter()
        .any(|issue| issue.status == ValidationStatus::Failed)
    {
        ValidationStatus::Failed
    } else if issues
        .iter()
        .any(|issue| issue.status == ValidationStatus::Warning)
    {
        ValidationStatus::Warning
    } else {
        ValidationStatus::Passed
    }
}
