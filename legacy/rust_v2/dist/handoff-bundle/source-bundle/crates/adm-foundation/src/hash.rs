use std::fmt::{Display, Formatter};

#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ContentHash(String);

impl ContentHash {
    pub fn from_bytes(bytes: &[u8]) -> Self {
        let mut hash = 0xcbf29ce484222325u64;
        for byte in bytes {
            hash ^= u64::from(*byte);
            hash = hash.wrapping_mul(0x100000001b3);
        }
        Self(format!("fnv64:{hash:016x}"))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl Display for ContentHash {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hash_is_stable_for_same_bytes() {
        assert_eq!(
            ContentHash::from_bytes(b"same"),
            ContentHash::from_bytes(b"same")
        );
    }
}
