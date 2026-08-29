use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Adm4ErrorKind {
    InvalidInput,
    Validation,
    NotFound,
    Conflict,
    AlreadyLocked,
    Io,
    PathEscape,
    Blocked,
    AiUnavailable,
    RedLine,
    Internal,
}

#[derive(Debug, Clone)]
pub struct Adm4Error {
    pub kind: Adm4ErrorKind,
    pub message: String,
}

impl Adm4Error {
    pub fn new(kind: Adm4ErrorKind, message: impl Into<String>) -> Self {
        Self {
            kind,
            message: message.into(),
        }
    }

    pub fn invalid_input(message: impl Into<String>) -> Self {
        Self::new(Adm4ErrorKind::InvalidInput, message)
    }

    pub fn validation(message: impl Into<String>) -> Self {
        Self::new(Adm4ErrorKind::Validation, message)
    }

    pub fn not_found(message: impl Into<String>) -> Self {
        Self::new(Adm4ErrorKind::NotFound, message)
    }

    pub fn conflict(message: impl Into<String>) -> Self {
        Self::new(Adm4ErrorKind::Conflict, message)
    }

    pub fn already_locked(message: impl Into<String>) -> Self {
        Self::new(Adm4ErrorKind::AlreadyLocked, message)
    }

    pub fn io(message: impl Into<String>) -> Self {
        Self::new(Adm4ErrorKind::Io, message)
    }

    pub fn path_escape(message: impl Into<String>) -> Self {
        Self::new(Adm4ErrorKind::PathEscape, message)
    }

    pub fn blocked(message: impl Into<String>) -> Self {
        Self::new(Adm4ErrorKind::Blocked, message)
    }

    pub fn ai_unavailable(message: impl Into<String>) -> Self {
        Self::new(Adm4ErrorKind::AiUnavailable, message)
    }

    pub fn red_line(message: impl Into<String>) -> Self {
        Self::new(Adm4ErrorKind::RedLine, message)
    }

    pub fn internal(message: impl Into<String>) -> Self {
        Self::new(Adm4ErrorKind::Internal, message)
    }
}

impl fmt::Display for Adm4Error {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{:?}: {}", self.kind, self.message)
    }
}

impl std::error::Error for Adm4Error {}

pub type Adm4Result<T> = Result<T, Adm4Error>;
