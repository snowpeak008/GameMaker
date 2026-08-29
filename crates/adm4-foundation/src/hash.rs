use crate::error::{Adm4Error, Adm4Result};
use serde_json::Value;
use sha2::{Digest, Sha256};

/// sha256 十六进制摘要，带 `sha256:` 前缀。
pub fn sha256_hex(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    format!("sha256:{:x}", hasher.finalize())
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContentHash(pub String);

impl ContentHash {
    pub fn of_bytes(bytes: &[u8]) -> Self {
        Self(sha256_hex(bytes))
    }

    pub fn of_canonical_json(value: &Value) -> Adm4Result<Self> {
        Ok(Self(sha256_hex(canonical_json(value)?.as_bytes())))
    }
}

/// 确定性 JSON 序列化：对象键按字典序排序，紧凑输出。
pub fn canonical_json(value: &Value) -> Adm4Result<String> {
    let normalized = normalize(value);
    serde_json::to_string(&normalized)
        .map_err(|error| Adm4Error::internal(format!("canonical json failed: {error}")))
}

fn normalize(value: &Value) -> Value {
    match value {
        Value::Object(map) => {
            let mut sorted = serde_json::Map::new();
            let mut keys = map.keys().collect::<Vec<_>>();
            keys.sort();
            for key in keys {
                sorted.insert(key.clone(), normalize(&map[key]));
            }
            Value::Object(sorted)
        }
        Value::Array(items) => Value::Array(items.iter().map(normalize).collect()),
        other => other.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn canonical_json_sorts_keys_recursively() {
        let a = json!({"b": {"y": 1, "x": 2}, "a": [ {"k2": 1, "k1": 2} ]});
        let b = json!({"a": [ {"k1": 2, "k2": 1} ], "b": {"x": 2, "y": 1}});
        assert_eq!(canonical_json(&a).unwrap(), canonical_json(&b).unwrap());
        assert_eq!(
            ContentHash::of_canonical_json(&a).unwrap(),
            ContentHash::of_canonical_json(&b).unwrap()
        );
    }

    #[test]
    fn sha256_prefix_present() {
        assert!(sha256_hex(b"abc").starts_with("sha256:"));
    }
}
