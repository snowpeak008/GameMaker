use std::io::Cursor;

use adm_new_foundation::{AdmError, AdmResult};
use image::{DynamicImage, GenericImageView, ImageFormat};

use crate::image::png_metadata_from_bytes;

use super::{MAX_IMAGE_BYTES, MAX_IMAGE_EDGE, MAX_IMAGE_PIXELS};

pub(super) fn sanitize_png(bytes: &[u8]) -> AdmResult<(Vec<u8>, u32, u32)> {
    if bytes.len() > MAX_IMAGE_BYTES {
        return Err(AdmError::new(format!(
            "PNG image exceeded {MAX_IMAGE_BYTES} bytes"
        )));
    }
    let metadata = png_metadata_from_bytes(bytes)?;
    validate_dimensions(metadata.width, metadata.height, "generated")?;
    let decoded = image::load_from_memory_with_format(bytes, ImageFormat::Png)
        .map_err(|_| AdmError::new("image API returned an invalid or truncated PNG"))?;
    let (width, height) = decoded.dimensions();
    validate_dimensions(width, height, "decoded")?;
    if (width, height) != (metadata.width, metadata.height) {
        return Err(AdmError::new(
            "PNG decoded dimensions did not match the IHDR metadata",
        ));
    }
    let sanitized = DynamicImage::ImageRgba8(decoded.to_rgba8());
    let mut writer = Cursor::new(Vec::new());
    sanitized
        .write_to(&mut writer, ImageFormat::Png)
        .map_err(|_| AdmError::new("failed to sanitize generated PNG"))?;
    let output = writer.into_inner();
    if output.len() > MAX_IMAGE_BYTES {
        return Err(AdmError::new(format!(
            "sanitized PNG exceeded {MAX_IMAGE_BYTES} bytes"
        )));
    }
    Ok((output, width, height))
}

pub(super) fn validate_dimensions(width: u32, height: u32, context: &str) -> AdmResult<()> {
    if width <= 1 || height <= 1 {
        return Err(AdmError::new(format!(
            "{context} image dimensions must be greater than 1x1"
        )));
    }
    if width > MAX_IMAGE_EDGE || height > MAX_IMAGE_EDGE {
        return Err(AdmError::new(format!(
            "{context} image edge exceeded {MAX_IMAGE_EDGE} pixels"
        )));
    }
    let pixels = u64::from(width)
        .checked_mul(u64::from(height))
        .ok_or_else(|| AdmError::new(format!("{context} image dimensions overflowed")))?;
    if pixels > MAX_IMAGE_PIXELS {
        return Err(AdmError::new(format!(
            "{context} image exceeded {MAX_IMAGE_PIXELS} pixels"
        )));
    }
    Ok(())
}
