use std::fmt;

use serde::de::{Deserialize, Deserializer, Error as _, MapAccess, SeqAccess, Visitor};
use serde_json::{Map, Value};
use serde_path_to_error::{Segment, deserialize};

use crate::{GameSpec, ValidationSeverity};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GameSpecParseError {
    pub code: &'static str,
    pub severity: ValidationSeverity,
    pub path: String,
    pub related_ids: Vec<String>,
    pub message: String,
    pub suggestion: String,
}

pub fn parse_game_spec(input: &str) -> Result<GameSpec, GameSpecParseError> {
    let mut deserializer = serde_json::Deserializer::from_str(input);
    let unique = deserialize::<_, UniqueValue>(&mut deserializer)
        .map_err(|error| parse_error("SPEC_JSON_INVALID", &error))?;
    deserializer.end().map_err(|error| GameSpecParseError {
        code: "SPEC_JSON_INVALID",
        severity: ValidationSeverity::Error,
        path: "/".to_string(),
        related_ids: Vec::new(),
        message: error.to_string(),
        suggestion: "Remove trailing data and provide exactly one JSON object.".to_string(),
    })?;

    deserialize::<_, GameSpec>(unique.0).map_err(|error| parse_error("SPEC_SCHEMA_INVALID", &error))
}

fn parse_error<E>(
    default_code: &'static str,
    error: &serde_path_to_error::Error<E>,
) -> GameSpecParseError
where
    E: fmt::Display,
{
    let message = error.inner().to_string();
    let duplicate = message.contains("duplicate object key");
    GameSpecParseError {
        code: if duplicate {
            "SPEC_DUPLICATE_ID"
        } else {
            default_code
        },
        severity: ValidationSeverity::Error,
        path: display_path(error.path()),
        related_ids: duplicate_key(&message).into_iter().collect(),
        message,
        suggestion: if duplicate {
            "Remove the duplicate key; identifiers must be unique within their collection."
                .to_string()
        } else if default_code == "SPEC_SCHEMA_INVALID" {
            "Make the document conform to the strict GameSpec schema.".to_string()
        } else {
            "Provide syntactically valid JSON with no duplicate object keys.".to_string()
        },
    }
}

fn display_path(path: &serde_path_to_error::Path) -> String {
    let mut pointer = String::new();
    for segment in path {
        pointer.push('/');
        match segment {
            Segment::Seq { index } => pointer.push_str(&index.to_string()),
            Segment::Map { key } | Segment::Enum { variant: key } => {
                pointer.push_str(&key.replace('~', "~0").replace('/', "~1"));
            }
            Segment::Unknown => pointer.push('?'),
        }
    }
    if pointer.is_empty() {
        "/".to_string()
    } else {
        pointer
    }
}

fn duplicate_key(message: &str) -> Option<String> {
    message.split('`').nth(1).map(str::to_string)
}

impl fmt::Display for GameSpecParseError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "{} at {}: {}",
            self.code, self.path, self.message
        )
    }
}

impl std::error::Error for GameSpecParseError {}

struct UniqueValue(Value);

impl<'de> Deserialize<'de> for UniqueValue {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_any(UniqueValueVisitor)
    }
}

struct UniqueValueVisitor;

impl<'de> Visitor<'de> for UniqueValueVisitor {
    type Value = UniqueValue;

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("a JSON value without duplicate object keys")
    }

    fn visit_bool<E>(self, value: bool) -> Result<Self::Value, E> {
        Ok(UniqueValue(Value::Bool(value)))
    }

    fn visit_i64<E>(self, value: i64) -> Result<Self::Value, E> {
        Ok(UniqueValue(Value::Number(value.into())))
    }

    fn visit_u64<E>(self, value: u64) -> Result<Self::Value, E> {
        Ok(UniqueValue(Value::Number(value.into())))
    }

    fn visit_f64<E>(self, value: f64) -> Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        serde_json::Number::from_f64(value)
            .map(Value::Number)
            .map(UniqueValue)
            .ok_or_else(|| E::custom("non-finite JSON number"))
    }

    fn visit_str<E>(self, value: &str) -> Result<Self::Value, E> {
        Ok(UniqueValue(Value::String(value.to_string())))
    }

    fn visit_string<E>(self, value: String) -> Result<Self::Value, E> {
        Ok(UniqueValue(Value::String(value)))
    }

    fn visit_none<E>(self) -> Result<Self::Value, E> {
        Ok(UniqueValue(Value::Null))
    }

    fn visit_unit<E>(self) -> Result<Self::Value, E> {
        Ok(UniqueValue(Value::Null))
    }

    fn visit_some<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
    where
        D: Deserializer<'de>,
    {
        UniqueValue::deserialize(deserializer)
    }

    fn visit_seq<A>(self, mut sequence: A) -> Result<Self::Value, A::Error>
    where
        A: SeqAccess<'de>,
    {
        let mut values = Vec::new();
        while let Some(UniqueValue(value)) = sequence.next_element::<UniqueValue>()? {
            values.push(value);
        }
        Ok(UniqueValue(Value::Array(values)))
    }

    fn visit_map<A>(self, mut object: A) -> Result<Self::Value, A::Error>
    where
        A: MapAccess<'de>,
    {
        let mut values = Map::new();
        while let Some(key) = object.next_key::<String>()? {
            if values.contains_key(&key) {
                return Err(A::Error::custom(format!("duplicate object key `{key}`")));
            }
            let UniqueValue(value) = object.next_value::<UniqueValue>()?;
            values.insert(key, value);
        }
        Ok(UniqueValue(Value::Object(values)))
    }
}
