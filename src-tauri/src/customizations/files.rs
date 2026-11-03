//! Customization file operations, ported from ZapZap's
//! `customizations_manager.py`. Every path stays inside its scope folder.

use std::path::{Path, PathBuf};

use super::AssetKind;

pub const MAX_URL_BYTES: usize = 2 * 1024 * 1024;
pub const MAX_PREVIEW_BYTES: usize = 5 * 1024 * 1024;
const USER_AGENT: &str = "FastZap/1.0";
const PREVIEW_EXTENSIONS: &[&str] = &["png", "jpg", "jpeg", "webp", "gif", "bmp"];

#[derive(Debug, thiserror::Error)]
pub enum FileError {
    #[error("File name cannot be empty.")]
    EmptyName,
    #[error("invalid file name")]
    InvalidName,
    #[error("file not found")]
    NotFound,
    #[error("Only HTTPS URLs are allowed.")]
    NotHttps,
    #[error("Download is too large.")]
    TooLarge,
    #[error("{0}")]
    Userstyle(#[from] super::UserstyleError),
    #[error("{0}")]
    Io(#[from] std::io::Error),
    #[error("{0}")]
    Http(#[from] reqwest::Error),
}

/// Files of one kind in `dir`, sorted by lowercase name (the injection order).
pub fn list(dir: &Path, kind: AssetKind) -> Vec<String> {
    let Ok(read) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut names: Vec<String> = read
        .filter_map(Result::ok)
        .filter(|e| e.file_type().is_ok_and(|t| t.is_file()))
        .filter_map(|e| e.file_name().into_string().ok())
        .filter(|n| {
            Path::new(n)
                .extension()
                .is_some_and(|e| e.eq_ignore_ascii_case(kind.ext()))
        })
        .collect();
    names.sort_by_key(|n| n.to_lowercase());
    names
}

/// ZapZap's whitelist: `[A-Za-z0-9._-]`, with the kind's extension.
pub fn sanitize(name: &str, ext: &str) -> String {
    let base = name.replace('\\', "/");
    let base = base.rsplit('/').next().unwrap_or_default();
    let mut clean: String = base
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || "._-".contains(c) {
                c
            } else {
                '_'
            }
        })
        .collect();
    if clean.is_empty() {
        clean = format!("custom.{ext}");
    }
    if !clean.to_lowercase().ends_with(&format!(".{ext}")) {
        clean = format!("{clean}.{ext}");
    }
    clean
}

/// `dir/name`, refusing names that leave `dir`.
pub fn safe_join(dir: &Path, name: &str) -> Result<PathBuf, FileError> {
    let invalid = name.is_empty()
        || name == "."
        || name == ".."
        || name.contains(['/', '\\', '\0'])
        || Path::new(name).components().count() != 1;
    if invalid {
        return Err(FileError::InvalidName);
    }
    Ok(dir.join(name))
}

/// A free name in `dir`: `name`, else `stem_1.ext`, `stem_2.ext`, …
pub fn available_name(dir: &Path, name: &str) -> String {
    if !dir.join(name).exists() {
        return name.to_string();
    }
    let path = Path::new(name);
    let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or(name);
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| format!(".{e}"))
        .unwrap_or_default();
    (1..)
        .map(|n| format!("{stem}_{n}{ext}"))
        .find(|candidate| !dir.join(candidate).exists())
        .expect("a free name exists")
}

fn validate(kind: AssetKind, content: &str) -> Result<(), FileError> {
    if kind == AssetKind::Css {
        super::normalize_userstyle(content, true)?;
    }
    Ok(())
}

pub fn read(dir: &Path, name: &str) -> Result<String, FileError> {
    let path = safe_join(dir, name)?;
    if !path.is_file() {
        return Err(FileError::NotFound);
    }
    Ok(std::fs::read_to_string(path)?)
}

/// Create a file, or save an edit of `old_name` (renaming when the name
/// changed). Returns the final name.
pub fn write(
    dir: &Path,
    kind: AssetKind,
    old_name: Option<&str>,
    name: &str,
    content: &str,
) -> Result<String, FileError> {
    if name.trim().is_empty() {
        return Err(FileError::EmptyName);
    }
    validate(kind, content)?;
    std::fs::create_dir_all(dir)?;
    let wanted = sanitize(name.trim(), kind.ext());
    let final_name = match old_name {
        Some(old) if old == wanted => wanted,
        Some(old) => {
            let old_path = safe_join(dir, old)?;
            if !old_path.is_file() {
                return Err(FileError::NotFound);
            }
            let new_name = available_name(dir, &wanted);
            std::fs::rename(old_path, safe_join(dir, &new_name)?)?;
            new_name
        }
        None => available_name(dir, &wanted),
    };
    std::fs::write(safe_join(dir, &final_name)?, content)?;
    Ok(final_name)
}

/// Copy a file chosen by the user, keeping its original base name.
pub fn import_file(dir: &Path, kind: AssetKind, source: &Path) -> Result<String, FileError> {
    let matches = source
        .extension()
        .is_some_and(|e| e.eq_ignore_ascii_case(kind.ext()));
    let name = source
        .file_name()
        .and_then(|n| n.to_str())
        .filter(|_| matches)
        .ok_or(FileError::InvalidName)?;
    safe_join(dir, name)?;
    let content = std::fs::read_to_string(source)?;
    validate(kind, &content)?;
    std::fs::create_dir_all(dir)?;
    let name = available_name(dir, name);
    std::fs::copy(source, dir.join(&name))?;
    Ok(name)
}

pub fn duplicate(dir: &Path, kind: AssetKind, name: &str) -> Result<String, FileError> {
    let source = safe_join(dir, name)?;
    if !source.is_file() {
        return Err(FileError::NotFound);
    }
    let stem = Path::new(name)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("custom");
    let copy = available_name(
        dir,
        &sanitize(&format!("{stem}_copy.{}", kind.ext()), kind.ext()),
    );
    std::fs::copy(source, dir.join(&copy))?;
    Ok(copy)
}

pub fn delete(dir: &Path, name: &str) -> Result<(), FileError> {
    Ok(std::fs::remove_file(safe_join(dir, name)?)?)
}

/// GET with ZapZap's limits: HTTPS only, 15 s, at most `max` bytes.
async fn fetch(url: &str, max: usize) -> Result<Vec<u8>, FileError> {
    use futures_util::StreamExt;
    let parsed = url::Url::parse(url.trim()).map_err(|_| FileError::NotHttps)?;
    if parsed.scheme() != "https" {
        return Err(FileError::NotHttps);
    }
    let response = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(15))
        .user_agent(USER_AGENT)
        // Redirects must stay on HTTPS.
        .redirect(reqwest::redirect::Policy::custom(|attempt| {
            if attempt.url().scheme() != "https" || attempt.previous().len() > 5 {
                attempt.stop()
            } else {
                attempt.follow()
            }
        }))
        .build()?
        .get(parsed)
        .send()
        .await?
        .error_for_status()?;
    let mut body = Vec::new();
    let mut stream = response.bytes_stream();
    while let Some(chunk) = stream.next().await {
        body.extend_from_slice(&chunk?);
        if body.len() > max {
            return Err(FileError::TooLarge);
        }
    }
    Ok(body)
}

/// Download a stylesheet or script. Returns the saved name.
pub async fn import_url(
    dir: &Path,
    kind: AssetKind,
    url: &str,
    name: Option<&str>,
) -> Result<String, FileError> {
    let body = fetch(url, MAX_URL_BYTES).await?;
    let content = String::from_utf8_lossy(&body).into_owned();
    validate(kind, &content)?;
    let fallback = match kind {
        AssetKind::Css => "style.css",
        AssetKind::Js => "script.js",
    };
    let from_url = url
        .split(['?', '#'])
        .next()
        .unwrap_or_default()
        .trim_end_matches('/')
        .rsplit('/')
        .next()
        .filter(|s| !s.is_empty() && !s.contains(':'));
    let chosen = name
        .map(str::trim)
        .filter(|n| !n.is_empty())
        .or(from_url)
        .unwrap_or(fallback);
    std::fs::create_dir_all(dir)?;
    let final_name = available_name(dir, &sanitize(chosen, kind.ext()));
    std::fs::write(dir.join(&final_name), content)?;
    Ok(final_name)
}

/// userstyles.world style id of a URL, for its preview image.
pub fn userstyles_world_id(url: &str) -> Option<&str> {
    let rest = url.split("userstyles.world/api/style/").nth(1)?;
    let id = rest.strip_suffix(".user.css")?;
    (!id.is_empty() && id.bytes().all(|b| b.is_ascii_digit())).then_some(id)
}

/// Try userstyles.world's preview images; returns the saved image name.
pub async fn fetch_userstyles_preview(previews: &Path, css_name: &str, id: &str) -> Option<String> {
    for n in 0..2 {
        let url = format!("https://userstyles.world/preview/{id}/{n}.webp");
        if let Ok(bytes) = fetch(&url, MAX_PREVIEW_BYTES).await {
            let stem = Path::new(css_name).file_stem()?.to_str()?;
            std::fs::create_dir_all(previews).ok()?;
            let name = available_name(previews, &sanitize(&format!("{stem}-preview.webp"), "webp"));
            std::fs::write(previews.join(&name), bytes).ok()?;
            return Some(name);
        }
    }
    None
}

/// Copy a chosen image as the preview of `css_name`.
pub fn set_preview(previews: &Path, css_name: &str, image: &Path) -> Result<String, FileError> {
    let ext = image
        .extension()
        .and_then(|e| e.to_str())
        .map(str::to_lowercase)
        .filter(|e| PREVIEW_EXTENSIONS.contains(&e.as_str()))
        .unwrap_or_else(|| "png".into());
    if std::fs::metadata(image)?.len() > MAX_PREVIEW_BYTES as u64 {
        return Err(FileError::TooLarge);
    }
    let stem = Path::new(css_name)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("preview");
    std::fs::create_dir_all(previews)?;
    let name = available_name(previews, &sanitize(&format!("{stem}.{ext}"), &ext));
    std::fs::copy(image, previews.join(&name))?;
    Ok(name)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sanitizes_with_the_whitelist() {
        assert_eq!(sanitize("../my theme!.css", "css"), "my_theme_.css");
        assert_eq!(sanitize("", "js"), "custom.js");
        assert_eq!(sanitize("x.CSS", "css"), "x.CSS");
        assert_eq!(sanitize("x", "css"), "x.css");
    }

    #[test]
    fn rejects_escaping_names() {
        let dir = Path::new("/tmp/x");
        assert!(safe_join(dir, "../a.css").is_err());
        assert!(safe_join(dir, "..").is_err());
        assert!(safe_join(dir, "a/b.css").is_err());
        assert!(safe_join(dir, "a.css").is_ok());
    }

    #[test]
    fn create_rename_duplicate_delete() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path().join("css");
        let a = write(&dir, AssetKind::Css, None, "a", "b{}").unwrap();
        assert_eq!(a, "a.css");
        assert_eq!(
            write(&dir, AssetKind::Css, None, "a.css", "c{}").unwrap(),
            "a_1.css"
        );
        let renamed = write(&dir, AssetKind::Css, Some("a_1.css"), "z", "d{}").unwrap();
        assert_eq!(renamed, "z.css");
        assert_eq!(read(&dir, "z.css").unwrap(), "d{}");
        assert_eq!(
            duplicate(&dir, AssetKind::Css, "z.css").unwrap(),
            "z_copy.css"
        );
        assert_eq!(list(&dir, AssetKind::Css), ["a.css", "z.css", "z_copy.css"]);
        delete(&dir, "z_copy.css").unwrap();
        assert!(matches!(
            write(
                &dir,
                AssetKind::Css,
                None,
                "bad",
                "@-moz-document domain(\"x.org\") { a{} }"
            ),
            Err(FileError::Userstyle(_))
        ));
        assert!(matches!(
            write(&dir, AssetKind::Js, None, " ", ""),
            Err(FileError::EmptyName)
        ));
    }

    #[test]
    fn userstyles_world_ids() {
        assert_eq!(
            userstyles_world_id("https://userstyles.world/api/style/123.user.css"),
            Some("123")
        );
        assert_eq!(
            userstyles_world_id("https://userstyles.world/api/style/x.user.css"),
            None
        );
        assert_eq!(userstyles_world_id("https://example.com/a.css"), None);
    }
}
