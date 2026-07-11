use serde::{Deserialize, Serialize};

use crate::{
    CATEGORY_COMPLETION, CATEGORY_DEV, CATEGORY_IMAGE, CONFIG_TYPE_CODEX_CLI_IMAGE,
    CONFIG_TYPE_CUSTOM_COMPLETION_API, CONFIG_TYPE_CUSTOM_DEV_API, CONFIG_TYPE_CUSTOM_IMAGE_API,
    CONFIG_TYPE_LOCAL_CLAUDE_CLI, CONFIG_TYPE_LOCAL_CLAUDE_COMPLETION_CLI,
    CONFIG_TYPE_LOCAL_CODEX_CLI, CONFIG_TYPE_LOCAL_CODEX_COMPLETION_CLI,
    CONFIG_TYPE_OPENAI_COMPLETION_API, CONFIG_TYPE_OPENAI_DEV_API, CONFIG_TYPE_OPENAI_IMAGE_API,
    CONFIG_TYPE_SD_WEBUI_API,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AiConfigCategory {
    Dev,
    Image,
    Completion,
}

impl AiConfigCategory {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Dev => CATEGORY_DEV,
            Self::Image => CATEGORY_IMAGE,
            Self::Completion => CATEGORY_COMPLETION,
        }
    }

    pub fn from_id(value: &str) -> Option<Self> {
        match value {
            CATEGORY_DEV => Some(Self::Dev),
            CATEGORY_IMAGE => Some(Self::Image),
            CATEGORY_COMPLETION => Some(Self::Completion),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AiConfigSource {
    Cli,
    Api,
    CliBuiltin,
}

impl AiConfigSource {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Cli => "cli",
            Self::Api => "api",
            Self::CliBuiltin => "cli_builtin",
        }
    }

    pub const fn is_cli(self) -> bool {
        matches!(self, Self::Cli | Self::CliBuiltin)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AiAdapterKind {
    Codex,
    Claude,
    OpenAiCompatible,
    OpenAiImage,
    SdWebUi,
    CustomImage,
}

impl AiAdapterKind {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Codex => "codex",
            Self::Claude => "claude",
            Self::OpenAiCompatible => "openai_compatible",
            Self::OpenAiImage => "openai_image",
            Self::SdWebUi => "sd_webui",
            Self::CustomImage => "custom_image",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AiRequiredField {
    ApiUrl,
    ApiKey,
    Model,
}

impl AiRequiredField {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::ApiUrl => "api_url",
            Self::ApiKey => "api_key",
            Self::Model => "extra_json.model",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AiConfigDescriptor {
    pub config_type: &'static str,
    pub category: AiConfigCategory,
    pub source: AiConfigSource,
    pub adapter: AiAdapterKind,
    pub capabilities: &'static [&'static str],
    pub required_fields: &'static [AiRequiredField],
    pub default_program: Option<&'static str>,
}

/// Owned, serializable projection of an AI configuration descriptor.
///
/// The static descriptor remains the domain source of truth, while this view is
/// safe to expose across IPC because it contains neither configuration values
/// nor secret material.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AiConfigDescriptorView {
    pub config_type: String,
    pub category: AiConfigCategory,
    pub source: AiConfigSource,
    pub adapter: AiAdapterKind,
    pub capabilities: Vec<String>,
    pub required_fields: Vec<String>,
    pub default_program: Option<String>,
}

impl From<&AiConfigDescriptor> for AiConfigDescriptorView {
    fn from(descriptor: &AiConfigDescriptor) -> Self {
        Self {
            config_type: descriptor.config_type.to_string(),
            category: descriptor.category,
            source: descriptor.source,
            adapter: descriptor.adapter,
            capabilities: descriptor
                .capabilities
                .iter()
                .map(|capability| (*capability).to_string())
                .collect(),
            required_fields: descriptor
                .required_fields
                .iter()
                .map(|field| field.as_str().to_string())
                .collect(),
            default_program: descriptor.default_program.map(str::to_string),
        }
    }
}

const TEXT_DEV_CAPABILITIES: &[&str] = &["text_generation", "code_editing", "tool_use"];
const IMAGE_CAPABILITIES: &[&str] = &["image_generation"];
const COMPLETION_CAPABILITIES: &[&str] = &["text_generation", "structured_output"];
const API_CREDENTIAL_FIELDS: &[AiRequiredField] =
    &[AiRequiredField::ApiUrl, AiRequiredField::ApiKey];
const IMAGE_API_FIELDS: &[AiRequiredField] = &[
    AiRequiredField::ApiUrl,
    AiRequiredField::ApiKey,
    AiRequiredField::Model,
];
const COMPLETION_API_FIELDS: &[AiRequiredField] = &[
    AiRequiredField::ApiUrl,
    AiRequiredField::ApiKey,
    AiRequiredField::Model,
];
const URL_FIELD: &[AiRequiredField] = &[AiRequiredField::ApiUrl];

pub const AI_CONFIG_DESCRIPTORS: &[AiConfigDescriptor] = &[
    AiConfigDescriptor {
        config_type: CONFIG_TYPE_LOCAL_CODEX_CLI,
        category: AiConfigCategory::Dev,
        source: AiConfigSource::Cli,
        adapter: AiAdapterKind::Codex,
        capabilities: TEXT_DEV_CAPABILITIES,
        required_fields: &[],
        default_program: Some("codex"),
    },
    AiConfigDescriptor {
        config_type: CONFIG_TYPE_LOCAL_CLAUDE_CLI,
        category: AiConfigCategory::Dev,
        source: AiConfigSource::Cli,
        adapter: AiAdapterKind::Claude,
        capabilities: TEXT_DEV_CAPABILITIES,
        required_fields: &[],
        default_program: Some("claude"),
    },
    AiConfigDescriptor {
        config_type: CONFIG_TYPE_OPENAI_DEV_API,
        category: AiConfigCategory::Dev,
        source: AiConfigSource::Api,
        adapter: AiAdapterKind::OpenAiCompatible,
        capabilities: TEXT_DEV_CAPABILITIES,
        required_fields: API_CREDENTIAL_FIELDS,
        default_program: None,
    },
    AiConfigDescriptor {
        config_type: CONFIG_TYPE_CUSTOM_DEV_API,
        category: AiConfigCategory::Dev,
        source: AiConfigSource::Api,
        adapter: AiAdapterKind::OpenAiCompatible,
        capabilities: TEXT_DEV_CAPABILITIES,
        required_fields: API_CREDENTIAL_FIELDS,
        default_program: None,
    },
    AiConfigDescriptor {
        config_type: CONFIG_TYPE_CODEX_CLI_IMAGE,
        category: AiConfigCategory::Image,
        source: AiConfigSource::CliBuiltin,
        adapter: AiAdapterKind::Codex,
        capabilities: IMAGE_CAPABILITIES,
        required_fields: &[],
        default_program: Some("codex"),
    },
    AiConfigDescriptor {
        config_type: CONFIG_TYPE_OPENAI_IMAGE_API,
        category: AiConfigCategory::Image,
        source: AiConfigSource::Api,
        adapter: AiAdapterKind::OpenAiImage,
        capabilities: IMAGE_CAPABILITIES,
        required_fields: IMAGE_API_FIELDS,
        default_program: None,
    },
    AiConfigDescriptor {
        config_type: CONFIG_TYPE_SD_WEBUI_API,
        category: AiConfigCategory::Image,
        source: AiConfigSource::Api,
        adapter: AiAdapterKind::SdWebUi,
        capabilities: IMAGE_CAPABILITIES,
        required_fields: URL_FIELD,
        default_program: None,
    },
    AiConfigDescriptor {
        config_type: CONFIG_TYPE_CUSTOM_IMAGE_API,
        category: AiConfigCategory::Image,
        source: AiConfigSource::Api,
        adapter: AiAdapterKind::CustomImage,
        capabilities: IMAGE_CAPABILITIES,
        required_fields: IMAGE_API_FIELDS,
        default_program: None,
    },
    AiConfigDescriptor {
        config_type: CONFIG_TYPE_LOCAL_CODEX_COMPLETION_CLI,
        category: AiConfigCategory::Completion,
        source: AiConfigSource::Cli,
        adapter: AiAdapterKind::Codex,
        capabilities: COMPLETION_CAPABILITIES,
        required_fields: &[],
        default_program: Some("codex"),
    },
    AiConfigDescriptor {
        config_type: CONFIG_TYPE_LOCAL_CLAUDE_COMPLETION_CLI,
        category: AiConfigCategory::Completion,
        source: AiConfigSource::Cli,
        adapter: AiAdapterKind::Claude,
        capabilities: COMPLETION_CAPABILITIES,
        required_fields: &[],
        default_program: Some("claude"),
    },
    AiConfigDescriptor {
        config_type: CONFIG_TYPE_OPENAI_COMPLETION_API,
        category: AiConfigCategory::Completion,
        source: AiConfigSource::Api,
        adapter: AiAdapterKind::OpenAiCompatible,
        capabilities: COMPLETION_CAPABILITIES,
        required_fields: COMPLETION_API_FIELDS,
        default_program: None,
    },
    AiConfigDescriptor {
        config_type: CONFIG_TYPE_CUSTOM_COMPLETION_API,
        category: AiConfigCategory::Completion,
        source: AiConfigSource::Api,
        adapter: AiAdapterKind::OpenAiCompatible,
        capabilities: COMPLETION_CAPABILITIES,
        required_fields: COMPLETION_API_FIELDS,
        default_program: None,
    },
];

pub fn ai_config_descriptors() -> &'static [AiConfigDescriptor] {
    AI_CONFIG_DESCRIPTORS
}

pub fn ai_config_descriptor_views() -> Vec<AiConfigDescriptorView> {
    AI_CONFIG_DESCRIPTORS
        .iter()
        .map(AiConfigDescriptorView::from)
        .collect()
}

pub fn descriptor_for_config_type(config_type: &str) -> Option<&'static AiConfigDescriptor> {
    AI_CONFIG_DESCRIPTORS
        .iter()
        .find(|descriptor| descriptor.config_type == config_type)
}

pub fn descriptors_for_category(
    category: AiConfigCategory,
) -> impl Iterator<Item = &'static AiConfigDescriptor> {
    AI_CONFIG_DESCRIPTORS
        .iter()
        .filter(move |descriptor| descriptor.category == category)
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use super::*;

    #[test]
    fn catalog_contains_one_descriptor_for_each_of_the_twelve_config_types() {
        let config_types = ai_config_descriptors()
            .iter()
            .map(|descriptor| descriptor.config_type)
            .collect::<BTreeSet<_>>();

        assert_eq!(ai_config_descriptors().len(), 12);
        assert_eq!(config_types.len(), 12);
        assert_eq!(descriptors_for_category(AiConfigCategory::Dev).count(), 4);
        assert_eq!(descriptors_for_category(AiConfigCategory::Image).count(), 4);
        assert_eq!(
            descriptors_for_category(AiConfigCategory::Completion).count(),
            4
        );
    }

    #[test]
    fn descriptor_is_the_single_source_for_adapter_and_requirements() {
        let completion = descriptor_for_config_type(CONFIG_TYPE_OPENAI_COMPLETION_API).unwrap();
        assert_eq!(completion.category, AiConfigCategory::Completion);
        assert_eq!(completion.source, AiConfigSource::Api);
        assert_eq!(completion.adapter, AiAdapterKind::OpenAiCompatible);
        assert_eq!(completion.required_fields, COMPLETION_API_FIELDS);

        let builtin_image = descriptor_for_config_type(CONFIG_TYPE_CODEX_CLI_IMAGE).unwrap();
        assert_eq!(builtin_image.source, AiConfigSource::CliBuiltin);
        assert_eq!(builtin_image.default_program, Some("codex"));
        assert_eq!(builtin_image.capabilities, IMAGE_CAPABILITIES);
    }

    #[test]
    fn descriptor_views_are_complete_serializable_and_secret_free() {
        let views = ai_config_descriptor_views();
        assert_eq!(views.len(), 12);
        let completion = views
            .iter()
            .find(|view| view.config_type == CONFIG_TYPE_OPENAI_COMPLETION_API)
            .unwrap();
        assert_eq!(completion.category, AiConfigCategory::Completion);
        assert_eq!(completion.source, AiConfigSource::Api);
        assert_eq!(completion.adapter, AiAdapterKind::OpenAiCompatible);
        assert_eq!(
            completion.required_fields,
            vec!["api_url", "api_key", "extra_json.model"]
        );

        let serialized = serde_json::to_string(&views).unwrap();
        assert!(!serialized.contains("secret"));
        assert!(!serialized.contains("api_key\":"));
    }
}
