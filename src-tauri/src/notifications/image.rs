//! Notification images: the sender photo drawn in a circle (as ZapZap does),
//! or the app icon.

use base64::Engine;
use image::{ImageFormat, ImageReader, Limits, Rgba, RgbaImage};

use super::policy::MAX_ICON_URL;
use crate::settings::TrayTheme;

const SIZE: u32 = 128;
/// Decoding limits: page-supplied photos are small, and a highly compressed
/// image must not expand into a huge allocation.
const MAX_DECODED_SIDE: u32 = 1024;
const MAX_DECODED_BYTES: u64 = 16 * 1024 * 1024;

fn decode_data_url(url: &str) -> Option<Vec<u8>> {
    if url.len() > MAX_ICON_URL {
        return None;
    }
    let (meta, data) = url.strip_prefix("data:")?.split_once(',')?;
    if !meta.starts_with("image/") || !meta.ends_with(";base64") {
        return None;
    }
    base64::engine::general_purpose::STANDARD.decode(data).ok()
}

/// Circular PNG of the photo carried by a `data:image/...;base64` URL.
pub fn circular_photo(data_url: &str) -> Option<Vec<u8>> {
    let bytes = decode_data_url(data_url)?;
    let mut limits = Limits::default();
    limits.max_image_width = Some(MAX_DECODED_SIDE);
    limits.max_image_height = Some(MAX_DECODED_SIDE);
    limits.max_alloc = Some(MAX_DECODED_BYTES);
    let mut reader = ImageReader::new(std::io::Cursor::new(bytes))
        .with_guessed_format()
        .ok()?;
    reader.limits(limits);
    let img = reader.decode().ok()?;
    let img = img
        .resize_to_fill(SIZE, SIZE, image::imageops::FilterType::Lanczos3)
        .to_rgba8();
    let r = SIZE as f32 / 2.0;
    let out = RgbaImage::from_fn(SIZE, SIZE, |x, y| {
        let (dx, dy) = (x as f32 + 0.5 - r, y as f32 + 0.5 - r);
        let dist = (dx * dx + dy * dy).sqrt();
        let p = img.get_pixel(x, y);
        // Anti-aliased edge one pixel wide.
        let coverage = (r - dist).clamp(0.0, 1.0);
        Rgba([p[0], p[1], p[2], (p[3] as f32 * coverage) as u8])
    });
    encode(&out)
}

/// The default tray artwork, used when there is no photo.
pub fn app_icon() -> Option<Vec<u8>> {
    let rgba = crate::tray::icon::render(TrayTheme::Default, 0).ok()?;
    encode(&RgbaImage::from_raw(
        crate::tray::icon::SIZE,
        crate::tray::icon::SIZE,
        rgba,
    )?)
}

fn encode(img: &RgbaImage) -> Option<Vec<u8>> {
    let mut out = std::io::Cursor::new(Vec::new());
    img.write_to(&mut out, ImageFormat::Png).ok()?;
    Some(out.into_inner())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn photo_corners_are_transparent() {
        let src = RgbaImage::from_pixel(64, 64, Rgba([10, 20, 30, 255]));
        let png = encode(&src).unwrap();
        let url = format!(
            "data:image/png;base64,{}",
            base64::engine::general_purpose::STANDARD.encode(png)
        );
        let out = image::load_from_memory(&circular_photo(&url).unwrap())
            .unwrap()
            .to_rgba8();
        assert_eq!(out.get_pixel(0, 0)[3], 0);
        assert_eq!(out.get_pixel(64, 64)[3], 255);
    }

    #[test]
    fn rejects_non_image_urls() {
        assert!(circular_photo("data:text/html;base64,PGI+").is_none());
        assert!(circular_photo("https://example.com/a.png").is_none());
    }
}
