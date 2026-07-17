use std::path::Path;

use adm_new_foundation::{AdmError, AdmResult};
use image::{GenericImageView, Rgba, RgbaImage};

use crate::stages::step07_v2::{RepresentativeAssetTask, StyleAnchorCandidate};
use crate::stages::step08_10_v2::AssetManifestItem;

pub(super) fn asset_task_from_manifest_item(item: &AssetManifestItem) -> RepresentativeAssetTask {
    RepresentativeAssetTask {
        asset_id: item.asset_id.clone(),
        title: format!("Produce {}", item.asset_id),
        asset_type: item.slice.clone(),
        expected_width: item.width,
        expected_height: item.height,
        require_alpha: true,
        transparent_margin_px: 8,
        prompt: item.purpose.clone(),
        negative_prompt: "no watermark, no text labels, no unrelated subject".to_string(),
        source_refs: item.source_refs.clone(),
    }
}

pub(super) fn write_generated_asset_png(
    task: &RepresentativeAssetTask,
    index: usize,
    path: &Path,
) -> AdmResult<()> {
    let mut image = RgbaImage::from_pixel(
        task.expected_width,
        task.expected_height,
        Rgba([0, 0, 0, 0]),
    );
    let colors = [
        Rgba([46u8, 126u8, 82u8, 255u8]),
        Rgba([196u8, 82u8, 72u8, 255u8]),
        Rgba([235u8, 184u8, 82u8, 255u8]),
        Rgba([48u8, 94u8, 148u8, 255u8]),
    ];
    let base = colors[index % colors.len()].0;
    let variant = ((index / colors.len()) as u8).saturating_mul(19);
    let fill = Rgba([
        base[0].saturating_add(variant / 2),
        base[1].saturating_sub(variant / 3),
        base[2].saturating_add(variant),
        255u8,
    ]);
    for y in 8..task.expected_height.saturating_sub(8) {
        for x in 8..task.expected_width.saturating_sub(8) {
            let stripe = ((x / 18) + (y / 18) + index as u32).is_multiple_of(5);
            image.put_pixel(
                x,
                y,
                if stripe {
                    Rgba([238u8, 244u8, 218u8, 255u8])
                } else {
                    fill
                },
            );
        }
    }
    image
        .save(path)
        .map_err(|error| AdmError::new(format!("failed to write generated asset PNG: {error}")))
}

pub(super) fn anchor_average_colors(anchors: &[StyleAnchorCandidate]) -> AdmResult<Vec<[f32; 3]>> {
    anchors
        .iter()
        .map(|anchor| average_rgb(Path::new(&anchor.image_path)))
        .collect()
}

pub(super) fn min_style_distance(image_path: &str, anchors: &[[f32; 3]]) -> AdmResult<f32> {
    let image_color = average_rgb(Path::new(image_path))?;
    Ok(anchors
        .iter()
        .map(|anchor| rgb_distance(image_color, *anchor))
        .fold(f32::MAX, f32::min))
}

fn average_rgb(path: &Path) -> AdmResult<[f32; 3]> {
    let image = image::open(path)
        .map_err(|error| AdmError::new(format!("failed to open style image: {error}")))?;
    let mut total = [0f32; 3];
    let mut count = 0f32;
    for (_, _, pixel) in image.pixels().filter(|(_, _, pixel)| pixel.0[3] > 0) {
        total[0] += pixel.0[0] as f32;
        total[1] += pixel.0[1] as f32;
        total[2] += pixel.0[2] as f32;
        count += 1.0;
    }
    if count == 0.0 {
        return Err(AdmError::new("style image has no visible pixels"));
    }
    Ok([total[0] / count, total[1] / count, total[2] / count])
}

fn rgb_distance(left: [f32; 3], right: [f32; 3]) -> f32 {
    ((left[0] - right[0]).powi(2) + (left[1] - right[1]).powi(2) + (left[2] - right[2]).powi(2))
        .sqrt()
}

pub(super) fn safe_asset_file_name(value: &str) -> String {
    value
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || ch == '_' || ch == '-' {
                ch
            } else {
                '_'
            }
        })
        .collect()
}

pub(super) fn path_string(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}
