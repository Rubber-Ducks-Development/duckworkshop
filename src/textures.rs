use crate::Result;
use image::RgbaImage;
use std::path::Path;

/// File types the texture preview can open.
pub const EXTENSIONS: [&str; 11] = [
    "png", "jpg", "jpeg", "psd", "dds", "tga", "bmp", "tif", "tiff", "webp", "gif",
];
/// Layered PSDs are often far larger than the 100 MB model limit.
const MAX_FILE: u64 = 1024 * 1024 * 1024;
const MAX_SIDE: u32 = 16384;

/// Size and modification time; a change means the file was saved again.
pub type Stamp = (u64, Option<std::time::SystemTime>);

pub fn stamp(path: &Path) -> Option<Stamp> {
    let meta = std::fs::metadata(path).ok()?;
    Some((meta.len(), meta.modified().ok()))
}

pub fn load(path: &Path) -> Result<RgbaImage> {
    let size = std::fs::metadata(path).map_err(|e| e.to_string())?.len();
    if size > MAX_FILE {
        return Err("Texture exceeds the 1 GB input limit.".into());
    }
    let bytes = std::fs::read(path).map_err(|e| e.to_string())?;
    let ext = path
        .extension()
        .unwrap_or_default()
        .to_string_lossy()
        .to_lowercase();
    if ext == "psd" || bytes.starts_with(b"8BPS") {
        return decode_psd(&bytes);
    }
    decode(&bytes)
}

pub fn decode(bytes: &[u8]) -> Result<RgbaImage> {
    let reader = image::ImageReader::new(std::io::Cursor::new(bytes))
        .with_guessed_format()
        .map_err(|e| e.to_string())?;
    let (width, height) = reader
        .into_dimensions()
        .map_err(|e| format!("Unsupported image: {e}"))?;
    check_size(width, height)?;
    let image = image::load_from_memory(bytes).map_err(|e| format!("Unsupported image: {e}"))?;
    Ok(image.to_rgba8())
}

/// Uses the merged image Photoshop stores in the file. When that is blank (saved
/// without Maximize Compatibility), visible layers are combined instead.
pub fn decode_psd(bytes: &[u8]) -> Result<RgbaImage> {
    if bytes.get(4..6) == Some(&[0, 2]) {
        return Err("PSB (large document) files are unsupported. Save as PSD.".into());
    }
    let psd = std::panic::catch_unwind(|| psd::Psd::from_bytes(bytes))
        .map_err(|_| "This PSD could not be read.".to_string())?
        .map_err(|e| format!("Invalid PSD: {e}"))?;
    let (width, height) = (psd.width(), psd.height());
    check_size(width, height)?;
    if !matches!(
        psd.color_mode(),
        psd::ColorMode::Rgb | psd::ColorMode::Grayscale
    ) {
        return Err("Only RGB and grayscale PSDs are supported.".into());
    }
    let mut pixels = std::panic::catch_unwind(|| psd.rgba())
        .map_err(|_| "This PSD's merged image could not be read.".to_string())?;
    let blank = pixels
        .as_chunks::<4>()
        .0
        .iter()
        .all(|p| p[..] == pixels[..4]);
    if blank && !psd.layers().is_empty() {
        pixels = std::panic::catch_unwind(|| psd.flatten_layers_rgba(&|_| true))
            .map_err(|_| "This PSD's layers could not be combined.".to_string())?
            .map_err(|e| e.to_string())?;
    }
    RgbaImage::from_raw(width, height, pixels).ok_or_else(|| "Incomplete PSD image data.".into())
}

fn check_size(width: u32, height: u32) -> Result<()> {
    if width == 0 || height == 0 || width > MAX_SIDE || height > MAX_SIDE {
        return Err(format!(
            "Textures must be 1–{MAX_SIDE} pixels per side ({width} x {height})."
        ));
    }
    Ok(())
}
