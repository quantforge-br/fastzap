//! Account avatars: images are normalized to a PNG data URL of at most 256 px,
//! like ZapZap's embedded PNG icons.

use base64::Engine;
use futures_util::StreamExt;
use image::imageops::FilterType;
use image::{ImageFormat, ImageReader, Limits};

pub const MAX_AVATAR_SIDE: u32 = 256;
const MAX_SOURCE_BYTES: usize = 5 * 1024 * 1024;
const MAX_DECODED_SIDE: u32 = 4096;
const MAX_DECODED_BYTES: u64 = 64 * 1024 * 1024;
/// WhatsApp serves profile photos from `pps.whatsapp.net`.
const CDN_SUFFIX: &str = ".whatsapp.net";

static DOWNLOADS: tokio::sync::Semaphore = tokio::sync::Semaphore::const_new(2);

#[derive(Debug, thiserror::Error)]
pub enum AvatarError {
    #[error("image is too large")]
    TooLarge,
    #[error("invalid image: {0}")]
    Invalid(#[from] image::ImageError),
    #[error("invalid image: {0}")]
    Io(#[from] std::io::Error),
}

pub fn normalize_to_data_url(bytes: &[u8]) -> Result<String, AvatarError> {
    if bytes.len() > MAX_SOURCE_BYTES {
        return Err(AvatarError::TooLarge);
    }
    let mut limits = Limits::default();
    limits.max_image_width = Some(MAX_DECODED_SIDE);
    limits.max_image_height = Some(MAX_DECODED_SIDE);
    limits.max_alloc = Some(MAX_DECODED_BYTES);
    let mut reader = ImageReader::new(std::io::Cursor::new(bytes)).with_guessed_format()?;
    reader.limits(limits);
    let mut img = reader.decode()?;
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

/// Only HTTPS URLs on WhatsApp's CDN on the default port may be fetched.
pub fn is_cdn_url(url: &url::Url) -> bool {
    url.scheme() == "https"
        && url.port().is_none()
        && url.username().is_empty()
        && url.password().is_none()
        && matches!(url.host(), Some(url::Host::Domain(host))
            if host.to_ascii_lowercase().ends_with(CDN_SUFFIX))
}

/// Download an avatar from WhatsApp's CDN. Redirects are not followed and
/// the body is read with a size cap, so a page script cannot make the app
/// fetch other hosts or exhaust memory.
pub async fn download(url: &str) -> anyhow::Result<String> {
    let parsed = url::Url::parse(url)?;
    anyhow::ensure!(is_cdn_url(&parsed), "avatar URL is not on WhatsApp's CDN");
    let _permit = DOWNLOADS.acquire().await?;
    let response = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(20))
        .redirect(reqwest::redirect::Policy::none())
        .build()?
        .get(parsed)
        .send()
        .await?
        .error_for_status()?;
    anyhow::ensure!(
        response.status().is_success(),
        "unexpected avatar response {}",
        response.status()
    );
    if response
        .content_length()
        .is_some_and(|len| len > MAX_SOURCE_BYTES as u64)
    {
        return Err(AvatarError::TooLarge.into());
    }
    let mut body = Vec::new();
    let mut stream = response.bytes_stream();
    while let Some(chunk) = stream.next().await {
        let chunk = chunk?;
        if body.len() + chunk.len() > MAX_SOURCE_BYTES {
            return Err(AvatarError::TooLarge.into());
        }
        body.extend_from_slice(&chunk);
    }
    Ok(normalize_to_data_url(&body)?)
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
    fn only_whatsapp_cdn_urls_are_fetched() {
        let ok = |s: &str| is_cdn_url(&url::Url::parse(s).unwrap());
        assert!(ok("https://pps.whatsapp.net/v/t61/photo.jpg?x=1"));
        assert!(!ok("http://pps.whatsapp.net/a.jpg"));
        assert!(!ok("https://pps.whatsapp.net:8443/a.jpg"));
        assert!(!ok("https://evil.com/?pps.whatsapp.net"));
        assert!(!ok("https://whatsapp.net.evil.com/a.jpg"));
        assert!(!ok("https://127.0.0.1/a.jpg"));
        assert!(!ok("https://user@pps.whatsapp.net/a.jpg"));
    }

    #[test]
    fn oversized_dimensions_are_rejected() {
        let img = image::GrayImage::new(MAX_DECODED_SIDE + 1, 1);
        let mut buf = std::io::Cursor::new(Vec::new());
        image::DynamicImage::ImageLuma8(img)
            .write_to(&mut buf, ImageFormat::Png)
            .unwrap();
        assert!(normalize_to_data_url(&buf.into_inner()).is_err());
    }

    #[test]
    fn garbage_is_rejected() {
        assert!(normalize_to_data_url(b"not an image").is_err());
    }
}
