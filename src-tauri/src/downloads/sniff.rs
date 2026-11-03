//! Content sniffing for auto-open: a file opens on its own only when its
//! bytes and its extension agree on an allowed PDF or image type.

use std::io::Read;
use std::path::Path;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Pdf,
    Image,
}

/// MIME type detected from the first bytes, for the auto-open allowlist.
fn detect(head: &[u8]) -> Option<&'static str> {
    let starts = |magic: &[u8]| head.starts_with(magic);
    Some(if starts(b"%PDF-") {
        "application/pdf"
    } else if starts(&[0xFF, 0xD8, 0xFF]) {
        "image/jpeg"
    } else if starts(b"\x89PNG\r\n\x1a\n") {
        "image/png"
    } else if starts(b"GIF87a") || starts(b"GIF89a") {
        "image/gif"
    } else if starts(b"RIFF") && head.get(8..12) == Some(b"WEBP") {
        "image/webp"
    } else if starts(b"BM") {
        "image/bmp"
    } else if starts(b"II*\0") || starts(b"MM\0*") {
        "image/tiff"
    } else if starts(&[0, 0, 1, 0]) {
        "image/x-icon"
    } else if head.get(4..8) == Some(b"ftyp") && matches!(head.get(8..12), Some(b"avif" | b"avis"))
    {
        "image/avif"
    } else {
        return None;
    })
}

fn extensions(mime: &str) -> &'static [&'static str] {
    match mime {
        "application/pdf" => &["pdf"],
        "image/jpeg" => &["jpg", "jpeg", "jpe"],
        "image/png" => &["png"],
        "image/gif" => &["gif"],
        "image/webp" => &["webp"],
        "image/bmp" => &["bmp"],
        "image/tiff" => &["tif", "tiff"],
        "image/x-icon" => &["ico"],
        "image/avif" => &["avif"],
        _ => &[],
    }
}

/// The auto-open kind of a finished file, if its content matches its name.
pub fn auto_open_kind(path: &Path) -> Option<Kind> {
    let mut head = [0u8; 16];
    let n = std::fs::File::open(path).ok()?.read(&mut head).ok()?;
    let mime = detect(&head[..n])?;
    let ext = path.extension()?.to_str()?.to_ascii_lowercase();
    if !extensions(mime).contains(&ext.as_str()) {
        return None;
    }
    Some(if mime == "application/pdf" {
        Kind::Pdf
    } else {
        Kind::Image
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn kind(name: &str, bytes: &[u8]) -> Option<Kind> {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(name);
        std::fs::write(&path, bytes).unwrap();
        auto_open_kind(&path)
    }

    #[test]
    fn content_and_extension_must_agree() {
        assert_eq!(kind("a.pdf", b"%PDF-1.7 ..."), Some(Kind::Pdf));
        assert_eq!(kind("a.JPG", &[0xFF, 0xD8, 0xFF, 0xE0]), Some(Kind::Image));
        assert_eq!(kind("a.png", b"\x89PNG\r\n\x1a\n...."), Some(Kind::Image));
        assert_eq!(kind("a.pdf", b"\x89PNG\r\n\x1a\n...."), None);
        assert_eq!(kind("a.exe", b"%PDF-1.7"), None);
        assert_eq!(kind("a.pdf", b"MZ\x90\0"), None);
        assert_eq!(kind("a", b"%PDF-1.7"), None);
    }
}
