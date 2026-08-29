use std::fmt::{Display, Formatter};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AdmErrorKind {
    InvalidInput,
    Io,
    PathEscape,
    AlreadyLocked,
    NotFound,
    Conflict,
    Validation,
    Unsupported,
    Internal,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AdmError {
    kind: AdmErrorKind,
    message: String,
    context: Vec<String>,
}

impl AdmError {
    pub fn new(kind: AdmErrorKind, message: impl Into<String>) -> Self {
        Self {
            kind,
            message: message.into(),
            context: Vec::new(),
        }
    }

    pub fn invalid_input(message: impl Into<String>) -> Self {
        Self::new(AdmErrorKind::InvalidInput, message)
    }

    pub fn conflict(message: impl Into<String>) -> Self {
        Self::new(AdmErrorKind::Conflict, message)
    }

    pub fn validation(message: impl Into<String>) -> Self {
        Self::new(AdmErrorKind::Validation, message)
    }

    pub fn unsupported(message: impl Into<String>) -> Self {
        Self::new(AdmErrorKind::Unsupported, message)
    }

    pub fn with_context(mut self, context: impl Into<String>) -> Self {
        self.context.push(context.into());
        self
    }

    pub fn kind(&self) -> &AdmErrorKind {
        &self.kind
    }

    pub fn message(&self) -> &str {
        &self.message
    }

    pub fn context(&self) -> &[String] {
        &self.context
    }
}

impl Display for AdmError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}: {}", self.kind, self.message)?;
        for context in &self.context {
            write!(f, " [{context}]")?;
        }
        Ok(())
    }
}

impl std::error::Error for AdmError {}

impl From<std::io::Error> for AdmError {
    fn from(value: std::io::Error) -> Self {
        Self::new(AdmErrorKind::Io, value.to_string())
    }
}

pub type AdmResult<T> = Result<T, AdmError>;
