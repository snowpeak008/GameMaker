use serde::{Deserialize, Serialize};

/// Canonical language tag for user-facing pipeline and design artifacts.
///
/// Chinese is the stable default for requests created by older clients. New
/// locales can be added here without changing each command boundary.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum ArtifactLocale {
    #[default]
    #[serde(rename = "zh-CN")]
    ZhCn,
    #[serde(rename = "en-US")]
    EnUs,
}

impl ArtifactLocale {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::ZhCn => "zh-CN",
            Self::EnUs => "en-US",
        }
    }

    /// Normalizes UI/user input to a supported artifact locale.
    ///
    /// Missing and unsupported values deliberately fall back to Chinese so
    /// deserialization and older callers keep the same deterministic default.
    pub fn normalize(value: Option<&str>) -> Self {
        match value.map(str::trim) {
            Some(value)
                if value.eq_ignore_ascii_case("en")
                    || value.eq_ignore_ascii_case("en-US")
                    || value.eq_ignore_ascii_case("en_US")
                    || value.eq_ignore_ascii_case("english") =>
            {
                Self::EnUs
            }
            _ => Self::ZhCn,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn artifact_locale_defaults_to_chinese_and_serializes_as_language_tag() {
        assert_eq!(ArtifactLocale::default(), ArtifactLocale::ZhCn);
        assert_eq!(ArtifactLocale::ZhCn.as_str(), "zh-CN");
        assert_eq!(ArtifactLocale::EnUs.as_str(), "en-US");
        assert_eq!(
            serde_json::to_string(&ArtifactLocale::ZhCn).unwrap(),
            "\"zh-CN\""
        );
        assert_eq!(
            serde_json::to_string(&ArtifactLocale::EnUs).unwrap(),
            "\"en-US\""
        );
    }

    #[test]
    fn artifact_locale_normalizes_supported_values_with_safe_default() {
        assert_eq!(ArtifactLocale::normalize(None), ArtifactLocale::ZhCn);
        assert_eq!(ArtifactLocale::normalize(Some("")), ArtifactLocale::ZhCn);
        assert_eq!(
            ArtifactLocale::normalize(Some("unsupported")),
            ArtifactLocale::ZhCn
        );
        assert_eq!(
            ArtifactLocale::normalize(Some(" zh-CN ")),
            ArtifactLocale::ZhCn
        );
        assert_eq!(
            ArtifactLocale::normalize(Some(" EN_us ")),
            ArtifactLocale::EnUs
        );
    }

    #[test]
    fn artifact_locale_round_trips_canonical_tags() {
        for locale in [ArtifactLocale::ZhCn, ArtifactLocale::EnUs] {
            let json = serde_json::to_string(&locale).unwrap();
            let restored: ArtifactLocale = serde_json::from_str(&json).unwrap();
            assert_eq!(restored, locale);
        }
    }
}
