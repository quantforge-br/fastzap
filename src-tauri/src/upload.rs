//! Local files attached to outgoing messages: size cap, reading and MIME type.

use std::path::Path;

use crate::i18n::tr;

/// Upload limit; WhatsApp itself caps documents at 2 GB but each file is held
/// in memory whole (the Evolution Go server keeps the multipart body, and
/// WhatsApp Web drops are copied into the page).
pub const MAX_UPLOAD: u64 = 100 * 1024 * 1024;

#[derive(Debug, thiserror::Error)]
pub enum UploadError {
    #[error("{}", tr("Only regular files can be attached."))]
    NotAFile,
    #[error("{}", tr("The file is larger than 100 MB."))]
    TooLarge,
    #[error(transparent)]
    Io(#[from] std::io::Error),
}

/// MIME type from the file extension, `application/octet-stream` otherwise.
pub fn mime_for(path: &Path) -> &'static str {
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .map(str::to_ascii_lowercase)
        .unwrap_or_default();
    match ext.as_str() {
        "jpg" | "jpeg" => "image/jpeg",
        "png" => "image/png",
        "webp" => "image/webp",
        "gif" => "image/gif",
        "mp4" | "m4v" => "video/mp4",
        "3gp" => "video/3gpp",
        "mov" => "video/quicktime",
        "mp3" => "audio/mpeg",
        "ogg" | "opus" => "audio/ogg",
        "m4a" => "audio/mp4",
        "aac" => "audio/aac",
        "wav" => "audio/wav",
        "pdf" => "application/pdf",
        "txt" => "text/plain",
        "zip" => "application/zip",
        "doc" => "application/msword",
        "docx" => "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
        "xls" => "application/vnd.ms-excel",
        "xlsx" => "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
        "ppt" => "application/vnd.ms-powerpoint",
        "pptx" => "application/vnd.openxmlformats-officedocument.presentationml.presentation",
        _ => "application/octet-stream",
    }
}

/// Reads a regular file of at most `limit` bytes, probing one extra byte to
/// detect growth. Check both the path and opened handle before reading.
pub fn read_upload(path: &Path, limit: u64) -> Result<Vec<u8>, UploadError> {
    use std::io::Read;
    // A FIFO would block `open`, so the path is checked first.
    if !std::fs::metadata(path)?.is_file() {
        return Err(UploadError::NotAFile);
    }
    let file = std::fs::File::open(path)?;
    let meta = file.metadata()?;
    if !meta.is_file() {
        return Err(UploadError::NotAFile);
    }
    if meta.len() > limit {
        return Err(UploadError::TooLarge);
    }
    let mut bytes = Vec::with_capacity(usize::try_from(meta.len()).unwrap_or(0));
    file.take(limit.saturating_add(1)).read_to_end(&mut bytes)?;
    if bytes.len() as u64 > limit {
        return Err(UploadError::TooLarge);
    }
    Ok(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn uploads_reject_directories() {
        let dir = tempfile::tempdir().unwrap();
        assert!(matches!(
            read_upload(dir.path(), MAX_UPLOAD),
            Err(UploadError::NotAFile)
        ));
    }

    #[test]
    fn uploads_reject_missing_files() {
        let dir = tempfile::tempdir().unwrap();
        assert!(read_upload(&dir.path().join("gone.txt"), MAX_UPLOAD).is_err());
    }

    #[test]
    fn uploads_are_capped_at_the_limit() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("a.bin");
        std::fs::write(&path, [7u8; 16]).unwrap();
        assert_eq!(read_upload(&path, 16).unwrap(), vec![7u8; 16]);
        let err = read_upload(&path, 15).unwrap_err();
        assert_eq!(err.to_string(), tr("The file is larger than 100 MB."));
    }

    #[test]
    fn empty_files_are_accepted() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("empty");
        std::fs::write(&path, []).unwrap();
        assert!(read_upload(&path, MAX_UPLOAD).unwrap().is_empty());
    }

    #[test]
    fn mime_types_follow_the_extension() {
        assert_eq!(mime_for(Path::new("a.JPG")), "image/jpeg");
        assert_eq!(mime_for(Path::new("a.pdf")), "application/pdf");
        assert_eq!(mime_for(Path::new("a")), "application/octet-stream");
    }
}
