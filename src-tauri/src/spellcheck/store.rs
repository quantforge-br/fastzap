//! Installed Hunspell dictionaries: FastZap's own store, which Enchant reads
//! as `$ENCHANT_CONFIG_DIR/hunspell`, plus read-only system directories.

use std::collections::BTreeMap;
use std::io::Write;
use std::path::{Path, PathBuf};

use futures_util::StreamExt;

use super::catalog::{Blob, Catalog, Entry, enchant_tag, git_blob_sha1};

/// Read-only locations Enchant's Hunspell provider also searches.
pub const SYSTEM_DIRS: &[&str] = &[
    "/usr/share/hunspell",
    "/usr/share/myspell",
    "/usr/share/myspell/dicts",
    "/app/share/hunspell",
];

const USER_AGENT: &str = concat!("FastZap/", env!("CARGO_PKG_VERSION"));

#[derive(Debug, thiserror::Error)]
pub enum SpellError {
    #[error("The file or filename is invalid.")]
    InvalidFile,
    #[error("A different dictionary with this name already exists.")]
    Conflict,
    #[error("FastZap could not write to the dictionary directory.")]
    Permission,
    #[error("The dictionary is not installed.")]
    NotInstalled,
    #[error("Check the connection and try again.")]
    Network,
    #[error("The download timed out. Try again.")]
    Timeout,
    #[error("The dictionary server returned an error.")]
    Http,
    #[error("The downloaded size does not match the catalog.")]
    Size,
    #[error("The downloaded file failed integrity verification.")]
    Hash,
    #[error("This dictionary operation is already in progress.")]
    Busy,
}

impl From<reqwest::Error> for SpellError {
    fn from(err: reqwest::Error) -> Self {
        if err.is_timeout() {
            SpellError::Timeout
        } else if err.is_status() {
            SpellError::Http
        } else {
            SpellError::Network
        }
    }
}

/// Where FastZap installs dictionaries (`<root>/hunspell`).
pub fn managed_dir(root: &Path) -> PathBuf {
    root.join("hunspell")
}

/// A dictionary tag a file stem may carry.
pub fn safe_tag(stem: &str) -> bool {
    !stem.is_empty()
        && stem.len() <= 64
        && stem.chars().next().is_some_and(|c| c.is_ascii_alphabetic())
        && stem
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
}

/// Tags with both `.dic` and `.aff` in `dir`.
pub fn list_dir(dir: &Path) -> Vec<String> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut tags: Vec<String> = entries
        .flatten()
        .filter_map(|e| {
            let path = e.path();
            if path.extension()? != "dic" || !e.file_type().ok()?.is_file() {
                return None;
            }
            let stem = path.file_stem()?.to_str()?;
            (safe_tag(stem) && path.with_extension("aff").is_file()).then(|| stem.to_owned())
        })
        .collect();
    tags.sort();
    tags
}

/// Installed dictionaries by tag; `true` marks FastZap's own copies, which
/// win over a system dictionary with the same tag.
pub fn installed(root: &Path) -> BTreeMap<String, bool> {
    let mut all = BTreeMap::new();
    for dir in SYSTEM_DIRS {
        for tag in list_dir(Path::new(dir)) {
            all.insert(tag, false);
        }
    }
    for tag in list_dir(&managed_dir(root)) {
        all.insert(tag, true);
    }
    all
}

async fn download(url: &str, blob: &Blob) -> Result<Vec<u8>, SpellError> {
    let response = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(120))
        .connect_timeout(std::time::Duration::from_secs(15))
        .user_agent(USER_AGENT)
        .redirect(reqwest::redirect::Policy::none())
        .build()?
        .get(url)
        .send()
        .await?
        .error_for_status()?;
    let mut body = Vec::with_capacity(blob.size as usize);
    let mut stream = response.bytes_stream();
    while let Some(chunk) = stream.next().await {
        body.extend_from_slice(&chunk?);
        if body.len() as u64 > blob.size {
            return Err(SpellError::Size);
        }
    }
    verify(&body, blob)?;
    Ok(body)
}

pub fn verify(content: &[u8], blob: &Blob) -> Result<(), SpellError> {
    if content.len() as u64 != blob.size {
        return Err(SpellError::Size);
    }
    if git_blob_sha1(content) != blob.sha {
        return Err(SpellError::Hash);
    }
    Ok(())
}

/// Write through a `.part` file and rename, so a crash never leaves a
/// truncated dictionary behind.
fn write_atomic(path: &Path, content: &[u8]) -> std::io::Result<()> {
    let part = path.with_extension(format!(
        "{}.part",
        path.extension().and_then(|e| e.to_str()).unwrap_or("")
    ));
    let mut file = std::fs::File::create(&part)?;
    file.write_all(content)?;
    file.sync_all()?;
    std::fs::rename(&part, path)
}

/// Store a verified pair; the `.dic` goes last because it is what makes a
/// dictionary visible.
fn store_pair(dir: &Path, tag: &str, aff: &[u8], dic: &[u8]) -> Result<(), SpellError> {
    std::fs::create_dir_all(dir).map_err(|_| SpellError::Permission)?;
    write_atomic(&dir.join(format!("{tag}.aff")), aff).map_err(|_| SpellError::Permission)?;
    write_atomic(&dir.join(format!("{tag}.dic")), dic).map_err(|_| SpellError::Permission)
}

/// Download and verify a catalog dictionary into the managed store.
pub async fn install(root: &Path, catalog: &Catalog, entry: &Entry) -> Result<(), SpellError> {
    let aff = download(&catalog.file_url(entry, "aff"), &entry.aff).await?;
    let dic = download(&catalog.file_url(entry, "dic"), &entry.dic).await?;
    store_pair(&managed_dir(root), &entry.tag(), &aff, &dic)
}

/// Remove a managed dictionary; system dictionaries cannot be removed.
pub fn remove(root: &Path, tag: &str) -> Result<(), SpellError> {
    if !safe_tag(tag) {
        return Err(SpellError::InvalidFile);
    }
    let dir = managed_dir(root);
    let dic = dir.join(format!("{tag}.dic"));
    if !dic.is_file() {
        return Err(SpellError::NotInstalled);
    }
    for ext in ["dic", "aff"] {
        match std::fs::remove_file(dir.join(format!("{tag}.{ext}"))) {
            Ok(()) => {}
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(_) => return Err(SpellError::Permission),
        }
    }
    Ok(())
}

/// Copy a local `.dic` and the `.aff` beside it. Returns the tag.
pub fn import(root: &Path, dic: &Path) -> Result<String, SpellError> {
    let stem = dic
        .file_stem()
        .and_then(|s| s.to_str())
        .filter(|s| safe_tag(s) && dic.extension().is_some_and(|e| e == "dic"))
        .ok_or(SpellError::InvalidFile)?;
    let tag = enchant_tag(stem);
    let aff = std::fs::read(dic.with_extension("aff")).map_err(|_| SpellError::InvalidFile)?;
    let content = std::fs::read(dic).map_err(|_| SpellError::InvalidFile)?;
    let dir = managed_dir(root);
    let existing = (
        std::fs::read(dir.join(format!("{tag}.aff"))).ok(),
        std::fs::read(dir.join(format!("{tag}.dic"))).ok(),
    );
    match existing {
        (Some(a), Some(d)) if a == aff && d == content => return Ok(tag),
        (None, None) => {}
        _ => return Err(SpellError::Conflict),
    }
    store_pair(&dir, &tag, &aff, &content)?;
    Ok(tag)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn blob(content: &[u8]) -> Blob {
        Blob {
            sha: git_blob_sha1(content),
            size: content.len() as u64,
        }
    }

    #[test]
    fn verification_checks_size_then_hash() {
        let b = blob(b"SET UTF-8\n");
        assert!(verify(b"SET UTF-8\n", &b).is_ok());
        assert!(matches!(verify(b"SET UTF-8", &b), Err(SpellError::Size)));
        assert!(matches!(verify(b"SET UTF-9\n", &b), Err(SpellError::Hash)));
    }

    #[test]
    fn lists_complete_pairs_with_safe_names() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        let store = managed_dir(root);
        std::fs::create_dir_all(&store).unwrap();
        for (name, body) in [
            ("pt_BR.dic", "1\nola\n"),
            ("pt_BR.aff", "SET UTF-8\n"),
            ("orphan.dic", "1\nx\n"),
            ("_bad.dic", "1\n"),
            ("_bad.aff", ""),
        ] {
            std::fs::write(store.join(name), body).unwrap();
        }
        assert_eq!(list_dir(&store), ["pt_BR"]);
        assert_eq!(installed(root).get("pt_BR"), Some(&true));
    }

    #[test]
    fn import_copies_pairs_and_refuses_conflicts() {
        let dir = tempfile::tempdir().unwrap();
        let src = dir.path().join("src");
        std::fs::create_dir_all(&src).unwrap();
        std::fs::write(src.join("de-AT.dic"), "1\nServus\n").unwrap();
        std::fs::write(src.join("de-AT.aff"), "SET UTF-8\n").unwrap();
        let root = dir.path().join("root");
        assert_eq!(import(&root, &src.join("de-AT.dic")).unwrap(), "de_AT");
        assert!(managed_dir(&root).join("de_AT.aff").is_file());
        // The same files again are fine; different ones conflict.
        assert!(import(&root, &src.join("de-AT.dic")).is_ok());
        std::fs::write(src.join("de-AT.dic"), "1\nGrüß\n").unwrap();
        assert!(matches!(
            import(&root, &src.join("de-AT.dic")),
            Err(SpellError::Conflict)
        ));
        std::fs::write(src.join("x.dic"), "1\n").unwrap();
        assert!(matches!(
            import(&root, &src.join("x.dic")),
            Err(SpellError::InvalidFile)
        ));

        remove(&root, "de_AT").unwrap();
        assert!(list_dir(&managed_dir(&root)).is_empty());
        assert!(matches!(
            remove(&root, "de_AT"),
            Err(SpellError::NotInstalled)
        ));
        assert!(matches!(
            remove(&root, "../x"),
            Err(SpellError::InvalidFile)
        ));
    }
}
