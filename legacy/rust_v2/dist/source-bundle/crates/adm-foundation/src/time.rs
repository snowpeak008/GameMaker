use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct UtcTimestamp {
    millis_since_unix_epoch: u128,
}

impl UtcTimestamp {
    pub fn now() -> Self {
        let millis_since_unix_epoch = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|duration| duration.as_millis())
            .unwrap_or_default();
        Self {
            millis_since_unix_epoch,
        }
    }

    pub fn from_millis(millis_since_unix_epoch: u128) -> Self {
        Self {
            millis_since_unix_epoch,
        }
    }

    pub fn as_millis(&self) -> u128 {
        self.millis_since_unix_epoch
    }
}
