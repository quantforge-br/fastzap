//! Account avatars: images are normalized to a PNG data URL of at most 256 px,
//! like ZapZap's embedded PNG icons.

use base64::Engine;
use image::ImageFormat;
use image::imageops::FilterType;

pub const MAX_AVATAR_SIDE: u32 = 256;
const MAX_SOURCE_BYTES: usize = 5 * 1024 * 1024;

#[derive(Debug, thiserror::Error)]
pub enum AvatarError {
    #[error("image is too large")]
    TooLarge,
    #[error("invalid image: {0}")]
    Invalid(#[from] image::ImageError),
}

pub fn normalize_to_data_url(bytes: &[u8]) -> Result<String, AvatarError> {
    if bytes.len() > MAX_SOURCE_BYTES {
        return Err(AvatarError::TooLarge);
    }
    let mut img = image::load_from_memory(bytes)?;
    if img.width() > MAX_AVATAR_SIDE || img.height() > MAX_AVATAR_SIDE {
        img = img.resize(MAX_AVATAR_SIDE, MAX_AVATAR_SIDE, FilterType::Lanczos3);
    }
    let mut out = std::io::Cursor::new(Vec::new());
    img.write_to(&mut out, ImageFormat::Png)?;
    Ok(format!(
        "data:image/png;base64,{}",
        base64::engine::general_purpose::STANDARD.encode(out.into_inner())
    ))
}

/// Download an avatar from WhatsApp's CDN. Only HTTPS URLs are accepted.
pub async fn download(url: &str) -> anyhow::Result<String> {
    let parsed = url::Url::parse(url)?;
    anyhow::ensure!(parsed.scheme() == "https", "avatar URL must use HTTPS");
    let response = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(20))
        .build()?
        .get(parsed)
        .send()
        .await?
        .error_for_status()?;
    let bytes = response.bytes().await?;
    Ok(normalize_to_data_url(&bytes)?)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn large_images_are_downscaled() {
        let img = image::RgbaImage::from_pixel(600, 300, image::Rgba([1, 2, 3, 255]));
        let mut buf = std::io::Cursor::new(Vec::new());
        image::DynamicImage::ImageRgba8(img)
            .write_to(&mut buf, ImageFormat::Png)
            .unwrap();
        let url = normalize_to_data_url(&buf.into_inner()).unwrap();
        let data = base64::engine::general_purpose::STANDARD
            .decode(url.trim_start_matches("data:image/png;base64,"))
            .unwrap();
        let out = image::load_from_memory(&data).unwrap();
        assert_eq!((out.width(), out.height()), (256, 128));
    }

    #[test]
    fn garbage_is_rejected() {
        assert!(normalize_to_data_url(b"not an image").is_err());
    }
}
