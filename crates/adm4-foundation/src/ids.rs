use crate::time::UtcTimestamp;
use std::process;
use std::sync::atomic::{AtomicU64, Ordering};

static COUNTER: AtomicU64 = AtomicU64::new(0);

/// 生成形如 `prefix_millis_pid_counter` 的 ID。
pub fn new_id(prefix: &str) -> String {
    let counter = COUNTER.fetch_add(1, Ordering::Relaxed);
    format!(
        "{prefix}_{}_{}_{counter}",
        UtcTimestamp::now().as_millis(),
        process::id()
    )
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionId(pub String);

impl SessionId {
    pub fn new() -> Self {
        Self(new_id("session"))
    }
}

impl Default for SessionId {
    fn default() -> Self {
        Self::new()
    }
}
