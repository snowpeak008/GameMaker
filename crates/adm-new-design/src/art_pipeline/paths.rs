use serde_json::Value;

use super::first_str;

pub const UNITY_GENERATED_ROOT: &str = "Assets/AutoDesign/";
pub const UNITY_ART_SOURCE_ROOT: &str = "Assets/AutoDesign/Art/Source/";
pub const UNITY_ART_PROCESSED_ROOT: &str = "Assets/AutoDesign/Art/Processed/";
pub const UNITY_ART_ATLAS_ROOT: &str = "Assets/AutoDesign/Art/Atlas/";
pub const UNITY_UI_PREFAB_ROOT: &str = "Assets/AutoDesign/Prefabs/UI/";
pub const UNITY_RUNTIME_GENERATED_ROOT: &str = "Assets/AutoDesign/Runtime/Generated/";
pub const UNITY_AUDIO_PLACEHOLDER_ROOT: &str = "Assets/AutoDesign/Audio/Placeholders/";
pub const UNITY_EDITOR_ROOT: &str = "Assets/AutoDesign/Editor/";
pub const LEGACY_GENERATED_ROOTS: &[&str] = &[
    "Assets/Art/",
    "Assets/UI/",
    "Assets/VFX/",
    "Assets/Audio/",
    "Assets/Textures/",
];

pub fn normalize_unity_path(path: impl AsRef<str>) -> String {
    let mut text = path.as_ref().trim().replace('\\', "/");
    while text.contains("//") {
        text = text.replace("//", "/");
    }
    text
}

pub fn slug(value: impl AsRef<str>, fallback: &str) -> String {
    let mut out = String::new();
    let mut last_was_sep = false;
    for ch in value.as_ref().trim().chars() {
        if ch.is_ascii_alphanumeric() || ch == '_' {
            out.push(ch);
            last_was_sep = false;
        } else if !last_was_sep {
            out.push('_');
            last_was_sep = true;
        }
    }
    let trimmed = out.trim_matches('_').to_string();
    if trimmed.is_empty() {
        fallback.to_string()
    } else {
        trimmed
    }
}

pub fn unity_file_extension(asset_type: impl AsRef<str>) -> &'static str {
    match asset_type.as_ref().trim().to_ascii_lowercase().as_str() {
        "config" | "data" | "runtime_data" => ".json",
        "audio_placeholder" => ".placeholder",
        "prefab" | "ui_prefab" => ".prefab",
        _ => ".png",
    }
}

pub fn canonical_unity_target_path(asset: &Value) -> String {
    let asset_id = slug(
        first_str(asset, &["asset_id", "name"]).unwrap_or_default(),
        "asset",
    );
    let asset_type = first_str(asset, &["asset_type"])
        .unwrap_or_else(|| "art_asset".to_string())
        .to_ascii_lowercase();
    let ext = unity_file_extension(&asset_type);
    match asset_type.as_str() {
        "audio_placeholder" => format!("{UNITY_AUDIO_PLACEHOLDER_ROOT}{asset_id}{ext}"),
        "config" | "data" | "runtime_data" => {
            format!("{UNITY_RUNTIME_GENERATED_ROOT}{asset_id}{ext}")
        }
        "ui_prefab" | "prefab" => format!("{UNITY_UI_PREFAB_ROOT}{asset_id}{ext}"),
        _ => format!("{UNITY_ART_SOURCE_ROOT}{asset_id}{ext}"),
    }
}

pub fn canonical_processed_path(asset: &Value) -> String {
    let asset_id = slug(
        first_str(asset, &["asset_id", "name"]).unwrap_or_default(),
        "asset",
    );
    let asset_type = first_str(asset, &["asset_type"])
        .unwrap_or_else(|| "art_asset".to_string())
        .to_ascii_lowercase();
    match asset_type.as_str() {
        "audio_placeholder" => canonical_unity_target_path(asset),
        "ui_prefab" | "prefab" => format!("{UNITY_UI_PREFAB_ROOT}{asset_id}.prefab"),
        "config" | "data" | "runtime_data" => {
            format!("{UNITY_RUNTIME_GENERATED_ROOT}{asset_id}.json")
        }
        _ => format!("{UNITY_ART_PROCESSED_ROOT}{asset_id}.png"),
    }
}

pub fn atlas_path(name: &str) -> String {
    format!(
        "{UNITY_ART_ATLAS_ROOT}{}.spriteatlasv2",
        slug(name, "asset")
    )
}

pub fn prefab_path(screen_id: impl AsRef<str>) -> String {
    format!(
        "{UNITY_UI_PREFAB_ROOT}Screen_{}.prefab",
        slug(screen_id, "screen")
    )
}

pub fn is_autodesign_path(path: impl AsRef<str>) -> bool {
    normalize_unity_path(path).starts_with(UNITY_GENERATED_ROOT)
}

pub fn is_legacy_generated_path(path: impl AsRef<str>) -> bool {
    let normalized = normalize_unity_path(path);
    LEGACY_GENERATED_ROOTS
        .iter()
        .any(|root| normalized.starts_with(root))
}

pub fn allowed_parent_path(path: impl AsRef<str>) -> String {
    let normalized = normalize_unity_path(path);
    if !normalized.contains('/') {
        return UNITY_GENERATED_ROOT.to_string();
    }
    let parent = normalized
        .rsplit_once('/')
        .map(|(parent, _)| parent.trim_end_matches('/'))
        .unwrap_or_default();
    if parent.is_empty() {
        UNITY_GENERATED_ROOT.to_string()
    } else {
        format!("{parent}/")
    }
}
