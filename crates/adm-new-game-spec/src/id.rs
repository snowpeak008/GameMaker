use std::{fmt, str::FromStr};

use serde::{Deserialize, Deserializer, Serialize, de::Error as _};

const MAX_SPEC_ID_BYTES: usize = 96;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
#[serde(transparent)]
pub struct SpecId(String);

impl SpecId {
    pub fn new(value: impl Into<String>) -> Result<Self, SpecIdError> {
        let value = value.into();
        validate_spec_id(&value)?;
        Ok(Self(value))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn into_inner(self) -> String {
        self.0
    }
}

impl fmt::Display for SpecId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl FromStr for SpecId {
    type Err = SpecIdError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Self::new(value)
    }
}

impl TryFrom<String> for SpecId {
    type Error = SpecIdError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}

impl TryFrom<&str> for SpecId {
    type Error = SpecIdError;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}

impl<'de> Deserialize<'de> for SpecId {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        Self::new(value).map_err(D::Error::custom)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpecIdError {
    value: String,
    reason: &'static str,
}

impl SpecIdError {
    pub fn value(&self) -> &str {
        &self.value
    }

    pub fn reason(&self) -> &'static str {
        self.reason
    }
}

impl fmt::Display for SpecIdError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "invalid spec id {:?}: {}",
            self.value, self.reason
        )
    }
}

impl std::error::Error for SpecIdError {}

fn validate_spec_id(value: &str) -> Result<(), SpecIdError> {
    let invalid = |reason| SpecIdError {
        value: value.to_string(),
        reason,
    };

    if value.is_empty() {
        return Err(invalid("must not be empty"));
    }
    if value.len() > MAX_SPEC_ID_BYTES {
        return Err(invalid("must not exceed 96 bytes"));
    }

    let mut chars = value.chars();
    let Some(first) = chars.next() else {
        return Err(invalid("must not be empty"));
    };
    if !first.is_ascii_lowercase() {
        return Err(invalid("must start with an ASCII lowercase letter"));
    }
    if !chars.clone().all(is_spec_id_character) {
        return Err(invalid(
            "may contain only ASCII lowercase letters, digits, '.', '-' and '_'",
        ));
    }
    let mut previous_was_separator = false;
    for character in chars {
        let is_separator = matches!(character, '.' | '-' | '_');
        if is_separator && previous_was_separator {
            return Err(invalid("must not contain adjacent separators"));
        }
        previous_was_separator = is_separator;
    }
    if matches!(value.as_bytes().last(), Some(b'.' | b'-' | b'_')) {
        return Err(invalid("must end with an ASCII lowercase letter or digit"));
    }
    Ok(())
}

fn is_spec_id_character(value: char) -> bool {
    value.is_ascii_lowercase() || value.is_ascii_digit() || matches!(value, '.' | '-' | '_')
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_stable_namespaced_ids() {
        for value in ["project", "core.health", "action_move", "phase-01"] {
            assert_eq!(SpecId::new(value).expect("valid id").as_str(), value);
        }
    }

    #[test]
    fn rejects_ambiguous_or_nonportable_ids() {
        for value in [
            "",
            "Project",
            "two words",
            "ends_",
            "two..parts",
            "含中文",
            "a/b",
        ] {
            assert!(SpecId::new(value).is_err(), "unexpected valid id: {value}");
        }
    }
}
