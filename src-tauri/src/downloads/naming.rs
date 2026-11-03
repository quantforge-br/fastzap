//! Download file names, ported from ZapZap's `download_naming_service.py`.

use std::path::{Path, PathBuf};

const MAX_NAME_BYTES: usize = 240;
const MAX_EXTENSION_BYTES: usize = 32;
const FALLBACK: &str = "download";
const RESERVED: &[&str] = &[
    "CON", "PRN", "AUX", "NUL", "COM1", "COM2", "COM3", "COM4", "COM5", "COM6", "COM7", "COM8",
    "COM9", "LPT1", "LPT2", "LPT3", "LPT4", "LPT5", "LPT6", "LPT7", "LPT8", "LPT9",
];

/// Python's `os.path.splitext`: leading dots belong to the stem.
fn split_ext(name: &str) -> (&str, &str) {
    let start = name.len() - name.trim_start_matches('.').len();
    match name[start..].rfind('.') {
        Some(i) => name.split_at(start + i),
        None => (name, ""),
    }
}

fn truncate_bytes(text: &str, max: usize) -> &str {
    if text.len() <= max {
        return text;
    }
    let mut end = max;
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    &text[..end]
}

/// Make a name safe on every platform: no separators, reserved characters,
/// control characters, reserved Windows names or overlong names.
pub fn sanitize_file_name(name: &str) -> String {
    let name = name.replace('\\', "/");
    let base = name.rsplit('/').next().unwrap_or_default();
    let cleaned: String = base
        .chars()
        .map(|c| {
            if "<>:\"/\\|?*".contains(c) || (c as u32) < 0x20 || c as u32 == 0x7f {
                '_'
            } else {
                c
            }
        })
        .collect();
    let mut name = cleaned
        .trim()
        .trim_matches('.')
        .trim_start_matches('.')
        .to_string();
    if name.is_empty() || name == "." || name == ".." {
        name = FALLBACK.into();
    }
    let (stem, ext) = split_ext(&name);
    if RESERVED.contains(&stem.trim_end_matches([' ', '.']).to_uppercase().as_str()) {
        name = format!("_{stem}{ext}");
    }
    if name.len() > MAX_NAME_BYTES {
        let (stem, ext) = split_ext(&name);
        let ext = truncate_bytes(ext, MAX_EXTENSION_BYTES);
        let stem = truncate_bytes(stem, MAX_NAME_BYTES.saturating_sub(ext.len()).max(1));
        name = format!("{stem}{ext}");
    }
    if name.is_empty() {
        name = FALLBACK.into();
    }
    name
}

fn has_extension(name: &str) -> bool {
    let (stem, ext) = split_ext(name);
    !stem.is_empty() && ext.len() > 1
}

fn extension_for_mime(mime: &str) -> Option<&'static str> {
    let mime = mime
        .split(';')
        .next()
        .unwrap_or_default()
        .trim()
        .to_ascii_lowercase();
    Some(match mime.as_str() {
        "application/pdf" | "application/x-pdf" => ".pdf",
        "image/jpeg" | "image/jpg" => ".jpeg",
        "image/png" => ".png",
        "image/gif" => ".gif",
        "image/webp" => ".webp",
        "image/bmp" => ".bmp",
        "image/svg+xml" => ".svg",
        "image/tiff" => ".tiff",
        "image/x-icon" | "image/vnd.microsoft.icon" => ".ico",
        "image/avif" => ".avif",
        "audio/ogg" => ".ogg",
        "audio/mpeg" => ".mp3",
        "video/mp4" => ".mp4",
        "text/plain" => ".txt",
        "application/zip" => ".zip",
        _ => return None,
    })
}

/// Last path segment of a URL, percent-decoded, without query or fragment.
fn url_basename(url: &str) -> String {
    let path = url
        .split(['?', '#'])
        .next()
        .unwrap_or_default()
        .trim_end_matches('/');
    let last = path.rsplit('/').next().unwrap_or_default();
    percent_encoding::percent_decode_str(last)
        .decode_utf8_lossy()
        .trim()
        .to_string()
}

/// The name a download is saved under: the suggested name, else the URL
/// basename, sanitized and given an extension from the MIME type if needed.
pub fn normalized_file_name(suggested: &str, mime: &str, url: &str) -> String {
    let mut name = suggested.trim().to_string();
    if name.is_empty() {
        name = url_basename(url);
    }
    if name.is_empty() {
        name = FALLBACK.into();
    }
    let name = sanitize_file_name(&name);
    if has_extension(&name) {
        return name;
    }
    match extension_for_mime(mime) {
        Some(ext) => {
            let stem = name.trim_start_matches('.');
            let stem = if stem.is_empty() { FALLBACK } else { stem };
            sanitize_file_name(&format!("{stem}{ext}"))
        }
        None => name,
    }
}

/// A free path in `dir` for `name`, adding Chromium's ` (N)` suffix before
/// the extension when the name is taken.
pub fn unique_target(dir: &Path, name: &str) -> PathBuf {
    let candidate = dir.join(name);
    if !candidate.exists() && candidate.symlink_metadata().is_err() {
        return candidate;
    }
    let (stem, ext) = split_ext(name);
    (1..)
        .map(|n| dir.join(format!("{stem} ({n}){ext}")))
        .find(|p| p.symlink_metadata().is_err())
        .expect("an unused name exists")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sanitizes_like_zapzap() {
        assert_eq!(sanitize_file_name("a/b\\c:d?.pdf"), "c_d_.pdf");
        assert_eq!(sanitize_file_name("  ..hidden. "), "hidden");
        assert_eq!(sanitize_file_name(".."), FALLBACK);
        assert_eq!(sanitize_file_name("con.txt"), "_con.txt");
        assert_eq!(sanitize_file_name("LPT1"), "_LPT1");
        assert_eq!(sanitize_file_name("tab\there"), "tab_here");
        let long = format!("{}.{}", "é".repeat(200), "x".repeat(40));
        let cut = sanitize_file_name(&long);
        assert!(cut.len() <= MAX_NAME_BYTES);
        assert!(cut.ends_with(&format!(".{}", "x".repeat(31))));
    }

    #[test]
    fn normalizes_with_fallbacks() {
        assert_eq!(normalized_file_name("doc.pdf", "", ""), "doc.pdf");
        assert_eq!(
            normalized_file_name("", "", "https://x/a/My%20File.pdf?x=1#y"),
            "My File.pdf"
        );
        assert_eq!(
            normalized_file_name("", "image/jpeg", "https://x/"),
            "x.jpeg"
        );
        assert_eq!(normalized_file_name("", "", ""), FALLBACK);
        assert_eq!(
            normalized_file_name("photo", "image/png; q=1", ""),
            "photo.png"
        );
        assert_eq!(
            normalized_file_name("blob", "application/octet-stream", ""),
            "blob"
        );
    }

    #[test]
    fn suffixes_taken_names() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(unique_target(dir.path(), "a.pdf"), dir.path().join("a.pdf"));
        std::fs::write(dir.path().join("a.pdf"), b"").unwrap();
        std::fs::write(dir.path().join("a (1).pdf"), b"").unwrap();
        assert_eq!(
            unique_target(dir.path(), "a.pdf"),
            dir.path().join("a (2).pdf")
        );
        std::fs::write(dir.path().join("noext"), b"").unwrap();
        assert_eq!(
            unique_target(dir.path(), "noext"),
            dir.path().join("noext (1)")
        );
    }
}
