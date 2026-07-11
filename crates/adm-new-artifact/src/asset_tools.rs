use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use adm_new_foundation::io::{write_json_serializable, write_text};
use adm_new_foundation::{AdmError, AdmResult, sha256_hex, unix_timestamp};
use image::GenericImageView;
use image::ImageReader;
use image::imageops::{FilterType, overlay, resize};
use serde::{Deserialize, Serialize};

pub const DEFAULT_AUDIO_SAMPLE_RATE: u32 = 44_100;
pub const DEFAULT_AUDIO_CHANNELS: u16 = 1;
pub const DEFAULT_AUDIO_BITS_PER_SAMPLE: u16 = 16;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AudioPlaceholderReport {
    pub status: String,
    pub placeholder: bool,
    pub path: PathBuf,
    pub duration_seconds: f32,
    pub sample_rate: u32,
    pub channels: u16,
    pub bits_per_sample: u16,
    pub sample_count: u32,
    pub byte_count: u64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SfxPlaceholderReport {
    pub prompt: String,
    pub audio: AudioPlaceholderReport,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SpriteSliceReport {
    pub status: String,
    pub sheet_path: PathBuf,
    pub output_dir: PathBuf,
    pub grid: [u32; 2],
    pub cell_size: [u32; 2],
    pub gap: u32,
    pub saved_files: Vec<PathBuf>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SpriteAtlasFrame {
    pub index: usize,
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
    pub source: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SpriteAtlasReport {
    pub status: String,
    pub atlas_path: PathBuf,
    pub metadata_path: PathBuf,
    pub cell_size: [u32; 2],
    pub grid: [u32; 2],
    pub frames: Vec<SpriteAtlasFrame>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LocalizationReplacement {
    pub file_path: PathBuf,
    pub key: String,
    pub text: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LocalizationInjectionReport {
    pub manager_path: PathBuf,
    pub injected_files: Vec<PathBuf>,
    pub replacements: Vec<LocalizationReplacement>,
}

pub fn generate_audio_placeholder(
    output_dir: impl AsRef<Path>,
    filename: Option<&str>,
) -> AdmResult<AudioPlaceholderReport> {
    write_silent_wav(output_dir, filename, 0.5, "placeholder")
}

pub fn generate_sfx_placeholder(
    prompt: &str,
    output_dir: impl AsRef<Path>,
    duration_seconds: f32,
    filename: Option<&str>,
) -> AdmResult<SfxPlaceholderReport> {
    Ok(SfxPlaceholderReport {
        prompt: prompt.to_string(),
        audio: write_silent_wav(output_dir, filename, duration_seconds, "sfx")?,
    })
}

pub fn write_silent_wav(
    output_dir: impl AsRef<Path>,
    filename: Option<&str>,
    duration_seconds: f32,
    default_prefix: &str,
) -> AdmResult<AudioPlaceholderReport> {
    if !duration_seconds.is_finite() || duration_seconds <= 0.0 {
        return Err(AdmError::new("duration_seconds must be positive"));
    }
    let output_dir = output_dir.as_ref();
    fs::create_dir_all(output_dir)?;
    let filename = filename
        .map(ToString::to_string)
        .unwrap_or_else(|| format!("{default_prefix}_{}.wav", unix_timestamp()));
    if Path::new(&filename).components().count() != 1 {
        return Err(AdmError::new("filename must not contain path separators"));
    }
    let path = output_dir.join(filename);
    let sample_count = (duration_seconds * DEFAULT_AUDIO_SAMPLE_RATE as f32).round() as u32;
    let bytes_per_sample = u32::from(DEFAULT_AUDIO_BITS_PER_SAMPLE / 8);
    let data_bytes = sample_count * u32::from(DEFAULT_AUDIO_CHANNELS) * bytes_per_sample;
    let byte_rate =
        DEFAULT_AUDIO_SAMPLE_RATE * u32::from(DEFAULT_AUDIO_CHANNELS) * bytes_per_sample;
    let block_align = DEFAULT_AUDIO_CHANNELS * (DEFAULT_AUDIO_BITS_PER_SAMPLE / 8);

    let mut file = fs::File::create(&path)?;
    file.write_all(b"RIFF")?;
    file.write_all(&(36 + data_bytes).to_le_bytes())?;
    file.write_all(b"WAVE")?;
    file.write_all(b"fmt ")?;
    file.write_all(&16_u32.to_le_bytes())?;
    file.write_all(&1_u16.to_le_bytes())?;
    file.write_all(&DEFAULT_AUDIO_CHANNELS.to_le_bytes())?;
    file.write_all(&DEFAULT_AUDIO_SAMPLE_RATE.to_le_bytes())?;
    file.write_all(&byte_rate.to_le_bytes())?;
    file.write_all(&block_align.to_le_bytes())?;
    file.write_all(&DEFAULT_AUDIO_BITS_PER_SAMPLE.to_le_bytes())?;
    file.write_all(b"data")?;
    file.write_all(&data_bytes.to_le_bytes())?;
    file.write_all(&vec![0_u8; data_bytes as usize])?;
    let byte_count = fs::metadata(&path)?.len();

    Ok(AudioPlaceholderReport {
        status: "placeholder".to_string(),
        placeholder: true,
        path,
        duration_seconds,
        sample_rate: DEFAULT_AUDIO_SAMPLE_RATE,
        channels: DEFAULT_AUDIO_CHANNELS,
        bits_per_sample: DEFAULT_AUDIO_BITS_PER_SAMPLE,
        sample_count,
        byte_count,
    })
}

pub fn slice_sprite_sheet(
    sheet_path: impl AsRef<Path>,
    output_dir: impl AsRef<Path>,
    grid: &str,
    cell_size: &str,
    gap: u32,
) -> AdmResult<SpriteSliceReport> {
    let sheet_path = sheet_path.as_ref();
    let output_dir = output_dir.as_ref();
    let [cols, rows] = parse_pair(grid, "grid")?;
    let [cell_width, cell_height] = parse_pair(cell_size, "cell_size")?;
    let image = ImageReader::open(sheet_path)
        .map_err(|error| AdmError::new(format!("failed to open sprite sheet: {error}")))?
        .decode()
        .map_err(|error| AdmError::new(format!("failed to decode sprite sheet: {error}")))?;
    let (width, height) = image.dimensions();
    let required_width = cols * cell_width + cols.saturating_sub(1) * gap;
    let required_height = rows * cell_height + rows.saturating_sub(1) * gap;
    if width < required_width || height < required_height {
        return Err(AdmError::new(format!(
            "sprite sheet is too small: required {required_width}x{required_height}, actual {width}x{height}"
        )));
    }
    fs::create_dir_all(output_dir)?;
    let mut saved_files = Vec::new();
    for row in 0..rows {
        for col in 0..cols {
            let x = col * (cell_width + gap);
            let y = row * (cell_height + gap);
            let crop = image.crop_imm(x, y, cell_width, cell_height);
            let path = output_dir.join(format!("icon_{row:02}_{col:02}.png"));
            crop.save(&path)
                .map_err(|error| AdmError::new(format!("failed to save sprite slice: {error}")))?;
            saved_files.push(path);
        }
    }
    Ok(SpriteSliceReport {
        status: "success".to_string(),
        sheet_path: sheet_path.to_path_buf(),
        output_dir: output_dir.to_path_buf(),
        grid: [cols, rows],
        cell_size: [cell_width, cell_height],
        gap,
        saved_files,
    })
}

pub fn pack_sprite_atlas(
    frame_paths: &[PathBuf],
    output_dir: impl AsRef<Path>,
    atlas_name: &str,
    cell_size: Option<&str>,
) -> AdmResult<SpriteAtlasReport> {
    if frame_paths.is_empty() {
        return Err(AdmError::new("sprite atlas requires at least one frame"));
    }
    let output_dir = output_dir.as_ref();
    fs::create_dir_all(output_dir)?;
    let first = load_rgba(&frame_paths[0])?;
    let [cell_width, cell_height] = match cell_size {
        Some(value) => parse_pair(value, "cell_size")?,
        None => [first.width(), first.height()],
    };
    let frame_count = frame_paths.len() as u32;
    let cols = if frame_count > 1 {
        (frame_count as f64).sqrt() as u32
    } else {
        1
    }
    .max(1);
    let rows = frame_count.div_ceil(cols);
    let mut atlas = image::RgbaImage::from_pixel(
        cols * cell_width,
        rows * cell_height,
        image::Rgba([0, 0, 0, 0]),
    );
    let mut frames = Vec::new();
    for (index, frame_path) in frame_paths.iter().enumerate() {
        let source = load_rgba(frame_path)?;
        let resized = resize(&source, cell_width, cell_height, FilterType::Lanczos3);
        let col = index as u32 % cols;
        let row = index as u32 / cols;
        let x = col * cell_width;
        let y = row * cell_height;
        overlay(&mut atlas, &resized, i64::from(x), i64::from(y));
        frames.push(SpriteAtlasFrame {
            index,
            x,
            y,
            width: cell_width,
            height: cell_height,
            source: frame_path
                .file_name()
                .and_then(|value| value.to_str())
                .unwrap_or_default()
                .to_string(),
        });
    }
    let atlas_filename = format!("{atlas_name}.png");
    let atlas_path = output_dir.join(&atlas_filename);
    atlas
        .save(&atlas_path)
        .map_err(|error| AdmError::new(format!("failed to save sprite atlas: {error}")))?;
    let metadata_path = output_dir.join(format!("{atlas_name}.json"));
    write_json_serializable(
        &metadata_path,
        &serde_json::json!({
            "atlas": atlas_filename,
            "cell_size": [cell_width, cell_height],
            "frames": frames,
        }),
    )?;

    Ok(SpriteAtlasReport {
        status: "success".to_string(),
        atlas_path,
        metadata_path,
        cell_size: [cell_width, cell_height],
        grid: [cols, rows],
        frames,
    })
}

pub fn generate_localization_manager(output_dir: impl AsRef<Path>) -> AdmResult<PathBuf> {
    let path = output_dir.as_ref().join("LocalizationManager.cs");
    write_text(&path, localization_manager_source())?;
    Ok(path)
}

pub fn run_localization_injector(
    source_dir: impl AsRef<Path>,
    output_dir: impl AsRef<Path>,
) -> AdmResult<LocalizationInjectionReport> {
    let manager_path = generate_localization_manager(output_dir)?;
    let mut injected_files = Vec::new();
    let mut replacements = Vec::new();
    let mut files = Vec::new();
    collect_cs_files(source_dir.as_ref(), &mut files)?;
    files.sort();
    for path in files {
        if path
            .file_name()
            .and_then(|value| value.to_str())
            .is_some_and(|name| name == "LocalizationManager.cs")
        {
            continue;
        }
        let content = fs::read_to_string(&path)?;
        if content.contains("Loc.Get(") {
            continue;
        }
        let literals = chinese_string_literals(&content);
        if literals.is_empty() {
            continue;
        }
        let mut new_content = String::new();
        let mut cursor = 0usize;
        for literal in literals {
            new_content.push_str(&content[cursor..literal.start]);
            let key = localization_key(&literal.text);
            new_content.push_str(&format!("Loc.Get(\"{key}\")"));
            cursor = literal.end;
            replacements.push(LocalizationReplacement {
                file_path: path.clone(),
                key,
                text: literal.text,
            });
        }
        new_content.push_str(&content[cursor..]);
        fs::write(&path, new_content)?;
        injected_files.push(path);
    }
    Ok(LocalizationInjectionReport {
        manager_path,
        injected_files,
        replacements,
    })
}

fn load_rgba(path: &Path) -> AdmResult<image::RgbaImage> {
    Ok(ImageReader::open(path)
        .map_err(|error| {
            AdmError::new(format!("failed to open image {}: {error}", path.display()))
        })?
        .decode()
        .map_err(|error| {
            AdmError::new(format!(
                "failed to decode image {}: {error}",
                path.display()
            ))
        })?
        .to_rgba8())
}

fn parse_pair(value: &str, label: &str) -> AdmResult<[u32; 2]> {
    let normalized = value.trim().to_ascii_lowercase();
    let Some((left, right)) = normalized.split_once('x') else {
        return Err(AdmError::new(format!(
            "{label} must use WxH or COLSxROWS format"
        )));
    };
    let left = left
        .trim()
        .parse::<u32>()
        .map_err(|_| AdmError::new(format!("{label} left value must be an integer")))?;
    let right = right
        .trim()
        .parse::<u32>()
        .map_err(|_| AdmError::new(format!("{label} right value must be an integer")))?;
    if left == 0 || right == 0 {
        return Err(AdmError::new(format!("{label} values must be positive")));
    }
    Ok([left, right])
}

fn collect_cs_files(root: &Path, files: &mut Vec<PathBuf>) -> AdmResult<()> {
    if !root.exists() {
        return Ok(());
    }
    for entry in fs::read_dir(root)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            collect_cs_files(&path, files)?;
        } else if path
            .extension()
            .and_then(|value| value.to_str())
            .is_some_and(|extension| extension.eq_ignore_ascii_case("cs"))
        {
            files.push(path);
        }
    }
    Ok(())
}

#[derive(Debug, Clone)]
struct StringLiteral {
    start: usize,
    end: usize,
    text: String,
}

fn chinese_string_literals(content: &str) -> Vec<StringLiteral> {
    let mut literals = Vec::new();
    let mut in_string = false;
    let mut escaped = false;
    let mut start = 0usize;
    let mut content_start = 0usize;
    for (index, ch) in content.char_indices() {
        if !in_string {
            if ch == '"' {
                in_string = true;
                escaped = false;
                start = index;
                content_start = index + ch.len_utf8();
            }
            continue;
        }
        if escaped {
            escaped = false;
            continue;
        }
        if ch == '\\' {
            escaped = true;
            continue;
        }
        if ch == '"' {
            let text = &content[content_start..index];
            if contains_cjk(text) {
                literals.push(StringLiteral {
                    start,
                    end: index + ch.len_utf8(),
                    text: text.to_string(),
                });
            }
            in_string = false;
        }
    }
    literals
}

fn contains_cjk(text: &str) -> bool {
    text.chars()
        .any(|ch| ('\u{4e00}'..='\u{9fff}').contains(&ch))
}

fn localization_key(text: &str) -> String {
    let hash = sha256_hex(text.as_bytes());
    format!("text_{}", &hash[..12])
}

fn localization_manager_source() -> &'static str {
    r#"// Auto-generated localization manager.
using System.Collections.Generic;
using System.IO;
using UnityEngine;

public static class Loc
{
    private static Dictionary<string, string> _strings = new Dictionary<string, string>();

    public static void LoadLanguage(string lang)
    {
        _strings.Clear();
        string path = Path.Combine(Application.streamingAssetsPath, "Localization", lang + ".md");
        if (!File.Exists(path))
        {
            Debug.LogWarning("Loc: language file not found: " + path);
            return;
        }
        foreach (string line in File.ReadAllLines(path))
        {
            int sep = line.IndexOf(": ");
            if (sep <= 0)
                continue;
            string key = line.Substring(0, sep).Trim();
            string value = line.Substring(sep + 2).Trim().Trim('"');
            _strings[key] = value;
        }
        Debug.Log("Loc: loaded language " + lang + ", count=" + _strings.Count);
    }

    public static string Get(string key)
    {
        if (_strings.TryGetValue(key, out string value))
            return value;
        return "[" + key + "]";
    }
}
"#
}

#[cfg(test)]
mod tests {
    use super::*;
    use adm_new_foundation::new_stable_id;

    #[test]
    fn audio_and_sfx_placeholders_write_pcm_wav_files() {
        let root = temp_root("audio");
        let audio = generate_audio_placeholder(&root, Some("placeholder.wav")).unwrap();
        assert!(audio.placeholder);
        assert_eq!(audio.sample_rate, 44_100);
        assert_eq!(audio.sample_count, 22_050);
        assert_eq!(fs::read(&audio.path).unwrap()[0..4], *b"RIFF");

        let sfx = generate_sfx_placeholder("jump", &root, 0.1, Some("jump.wav")).unwrap();
        assert_eq!(sfx.prompt, "jump");
        assert!(sfx.audio.path.exists());
        assert!(sfx.audio.byte_count > 44);
        cleanup(root);
    }

    #[test]
    fn sprite_sheet_slicer_and_atlas_writer_create_png_outputs_and_metadata() {
        let root = temp_root("sprites");
        let sheet = root.join("sheet.png");
        let mut image = image::RgbaImage::new(4, 2);
        for (x, y, pixel) in image.enumerate_pixels_mut() {
            *pixel = image::Rgba([(x * 60) as u8, (y * 120) as u8, 200, 255]);
        }
        image.save(&sheet).unwrap();

        let slices = slice_sprite_sheet(&sheet, root.join("slices"), "2x1", "2x2", 0).unwrap();
        assert_eq!(slices.saved_files.len(), 2);
        assert!(slices.saved_files.iter().all(|path| path.exists()));

        let atlas = pack_sprite_atlas(&slices.saved_files, root.join("atlas"), "walk", Some("2x2"))
            .unwrap();
        assert_eq!(atlas.grid, [1, 2]);
        assert!(atlas.atlas_path.exists());
        let metadata = fs::read_to_string(atlas.metadata_path).unwrap();
        assert!(metadata.contains("\"atlas\": \"walk.png\""));
        cleanup(root);
    }

    #[test]
    fn localization_injector_generates_manager_and_replaces_chinese_literals() {
        let root = temp_root("loc");
        let source = root.join("Assets/Scripts");
        fs::create_dir_all(&source).unwrap();
        let script = source.join("Title.cs");
        fs::write(
            &script,
            "public class Title { string text = \"开始游戏\"; }",
        )
        .unwrap();

        let report = run_localization_injector(&source, root.join("Generated")).unwrap();

        assert!(report.manager_path.exists());
        assert_eq!(report.injected_files, vec![script.clone()]);
        assert_eq!(report.replacements.len(), 1);
        let content = fs::read_to_string(script).unwrap();
        assert!(content.contains("Loc.Get(\"text_"));
        cleanup(root);
    }

    fn temp_root(label: &str) -> PathBuf {
        let root = std::env::temp_dir().join(format!(
            "adm_new_asset_tools_{label}_{}",
            new_stable_id("root").unwrap()
        ));
        fs::create_dir_all(&root).unwrap();
        root
    }

    fn cleanup(root: PathBuf) {
        let _ = fs::remove_dir_all(root);
    }
}
