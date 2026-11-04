//! Downloads started by WhatsApp Web pages.
//!
//! The engine saves every download into a private staging folder. Once it
//! finishes, ZapZap's rules decide where it goes: the multiple-download gate,
//! then the configured behavior (save dialog, automatic or ask every time).
//! Only then does the file reach the downloads folder, the history and the
//! auto-open check.

pub mod naming;
pub mod sniff;

use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use serde::{Deserialize, Serialize};
use tauri::webview::DownloadEvent;
use tauri::{AppHandle, Emitter, Manager, Runtime};
use tokio::sync::oneshot;
use ts_rs::TS;

use crate::app::effects;
use crate::app::state::AppState;
use crate::i18n::tr;
use crate::settings::{DownloadBehavior, TriState};

pub const MAX_RECENT: usize = 100;
const MAX_SESSION_RECORDS: usize = 100;

pub const DOWNLOADS_CHANGED: &str = "downloads://changed";
pub const DOWNLOAD_PROMPT: &str = "downloads://prompt";
pub const DOWNLOAD_SAVED: &str = "downloads://saved";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub enum DownloadStatus {
    /// Being received by the engine.
    Active,
    /// Received, waiting for the user's decision.
    Pending,
    Completed,
    Cancelled,
    Blocked,
    Failed,
}

#[derive(Debug, Clone, Serialize, TS)]
#[ts(export)]
pub struct DownloadItem {
    pub key: String,
    pub name: String,
    /// Final path; empty while live or when the file never arrived.
    pub path: String,
    pub status: DownloadStatus,
    pub live: bool,
    pub file_exists: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub enum PromptKind {
    /// "WhatsApp wants to download multiple files."
    Multiple,
    /// The Download dialog (Save / Open / Save as / Cancel).
    Save,
}

#[derive(Debug, Clone, Serialize, TS)]
#[ts(export)]
pub struct DownloadPrompt {
    #[ts(type = "number")]
    pub id: u64,
    pub kind: PromptKind,
    pub name: String,
    pub folder: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub enum DownloadDecision {
    Save,
    Open,
    SaveAs,
    Cancel,
    AllowOnce,
    AlwaysAllow,
    Block,
}

struct Live {
    account_id: i64,
    /// Matches completions that carry no path (failed WebKitGTK downloads).
    url: String,
    name: String,
    staged: PathBuf,
    status: DownloadStatus,
    needs_gate: bool,
    answer: Option<oneshot::Sender<DownloadDecision>>,
}

struct Record {
    seq: u64,
    name: String,
    path: Option<PathBuf>,
    status: DownloadStatus,
}

#[derive(Default)]
struct Inner {
    seq: u64,
    /// The first request of the session always passes the gate.
    first_seen: bool,
    live: BTreeMap<u64, Live>,
    records: Vec<Record>,
    /// Per-account "Save as" folder, kept in memory only.
    last_dirs: HashMap<i64, PathBuf>,
}

impl Inner {
    /// The live download an engine completion belongs to, if exactly one
    /// matches. Only downloads still being received match: WebKitGTK reports
    /// a completion once per handler registration, and an account's web
    /// context outlives its webview on Linux, so every recreated webview
    /// registers again and repeats the completion. An ambiguous pathless
    /// completion is left alone rather than failing another transfer.
    fn completed_seq(&self, account_id: i64, url: &str, path: Option<&Path>) -> Option<u64> {
        let mut found = self.live.iter().filter_map(|(seq, live)| {
            let matches = live.status == DownloadStatus::Active
                && match path {
                    Some(p) => p == live.staged,
                    None => live.account_id == account_id && live.url == url,
                };
            matches.then_some(*seq)
        });
        match (found.next(), found.next()) {
            (Some(seq), None) => Some(seq),
            _ => None,
        }
    }
}

pub struct Downloads {
    staging: PathBuf,
    inner: Mutex<Inner>,
}

impl Downloads {
    /// `staging` is emptied: leftovers of a previous run are never resumed.
    pub fn new(staging: PathBuf) -> Self {
        let _ = std::fs::remove_dir_all(&staging);
        Self {
            staging,
            inner: Mutex::default(),
        }
    }

    fn record(
        inner: &mut Inner,
        seq: u64,
        name: String,
        path: Option<PathBuf>,
        status: DownloadStatus,
    ) {
        inner.records.push(Record {
            seq,
            name,
            path,
            status,
        });
        if inner.records.len() > MAX_SESSION_RECORDS {
            inner.records.remove(0);
        }
    }

    /// Live downloads and this session's records (newest first), then the
    /// persisted history not already listed.
    pub fn items(&self, recent: &[String]) -> Vec<DownloadItem> {
        let inner = self.inner.lock().unwrap();
        let mut items: Vec<(u64, DownloadItem)> = inner
            .live
            .iter()
            .map(|(seq, live)| {
                let item = DownloadItem {
                    key: format!("live-{seq}"),
                    name: live.name.clone(),
                    path: String::new(),
                    status: live.status,
                    live: true,
                    file_exists: false,
                };
                (*seq, item)
            })
            .chain(inner.records.iter().map(|r| {
                let path = r.path.as_deref().map(path_string).unwrap_or_default();
                let item = DownloadItem {
                    key: format!("record-{}", r.seq),
                    name: r.name.clone(),
                    file_exists: r.path.as_deref().is_some_and(Path::exists),
                    path,
                    status: r.status,
                    live: false,
                };
                (r.seq, item)
            }))
            .collect();
        items.sort_by_key(|a| std::cmp::Reverse(a.0));
        let mut items: Vec<DownloadItem> = items.into_iter().map(|(_, i)| i).collect();
        for (i, path) in recent.iter().enumerate() {
            if items.iter().any(|item| same_path(&item.path, path)) {
                continue;
            }
            items.push(DownloadItem {
                key: format!("completed-{i}-{path}"),
                name: Path::new(path)
                    .file_name()
                    .map(|n| n.to_string_lossy().into_owned())
                    .unwrap_or_default(),
                file_exists: Path::new(path).exists(),
                path: path.clone(),
                status: DownloadStatus::Completed,
                live: false,
            });
        }
        items
    }

    /// Forget session records (the persisted list is cleared by the caller).
    pub fn clear_records(&self) {
        self.inner.lock().unwrap().records.clear();
    }

    pub fn remove_record(&self, path: &str) {
        let mut inner = self.inner.lock().unwrap();
        inner.records.retain(|r| {
            !r.path
                .as_deref()
                .is_some_and(|p| same_path(&path_string(p), path))
        });
    }

    /// Whether `path` is a file this app saved (session or history).
    pub fn knows(&self, path: &str, recent: &[String]) -> bool {
        recent.iter().any(|p| same_path(p, path))
            || self.inner.lock().unwrap().records.iter().any(|r| {
                r.path
                    .as_deref()
                    .is_some_and(|p| same_path(&path_string(p), path))
            })
    }

    pub fn forget_last_dirs(&self) {
        self.inner.lock().unwrap().last_dirs.clear();
    }

    /// Deliver the user's answer to a pending prompt.
    pub fn answer(&self, id: u64, decision: DownloadDecision) -> bool {
        let sender = self
            .inner
            .lock()
            .unwrap()
            .live
            .get_mut(&id)
            .and_then(|l| l.answer.take());
        sender.is_some_and(|s| s.send(decision).is_ok())
    }
}

fn path_string(path: &Path) -> String {
    path.to_string_lossy().into_owned()
}

pub fn same_path(a: &str, b: &str) -> bool {
    !a.is_empty() && Path::new(a).components().eq(Path::new(b).components())
}

fn emit_changed<R: Runtime>(app: &AppHandle<R>) {
    let _ = app.emit(DOWNLOADS_CHANGED, ());
}

/// `on_download` hook of the account webviews.
pub fn handle_event<R: Runtime>(
    app: &AppHandle<R>,
    account_id: i64,
    event: DownloadEvent<'_>,
) -> bool {
    let state = app.state::<AppState>();
    let downloads = &state.downloads;
    match event {
        DownloadEvent::Requested { url, destination } => {
            let suggested = destination
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default();
            let name = naming::normalized_file_name(&suggested, "", url.as_str());
            let permission = state.settings.get().downloads.multiple_download_permission;
            let mut inner = downloads.inner.lock().unwrap();
            inner.seq += 1;
            let seq = inner.seq;
            let needs_gate = if !inner.first_seen {
                inner.first_seen = true;
                false
            } else {
                match permission {
                    TriState::Allow => false,
                    TriState::Ask => true,
                    TriState::Block => {
                        Downloads::record(&mut inner, seq, name, None, DownloadStatus::Blocked);
                        drop(inner);
                        emit_changed(app);
                        return false;
                    }
                }
            };
            let staged = downloads.staging.join(seq.to_string()).join(&name);
            if let Err(err) = std::fs::create_dir_all(staged.parent().expect("staged has a parent"))
            {
                log::warn!("download staging failed: {err}");
                return false;
            }
            *destination = staged.clone();
            inner.live.insert(
                seq,
                Live {
                    account_id,
                    url: url.to_string(),
                    name,
                    staged,
                    status: DownloadStatus::Active,
                    needs_gate,
                    answer: None,
                },
            );
            drop(inner);
            emit_changed(app);
            true
        }
        DownloadEvent::Finished { url, path, success } => {
            let mut inner = downloads.inner.lock().unwrap();
            let Some(seq) = inner.completed_seq(account_id, url.as_str(), path.as_deref()) else {
                return true;
            };
            if !success {
                let live = inner.live.remove(&seq).expect("found above");
                Downloads::record(&mut inner, seq, live.name, None, DownloadStatus::Failed);
                drop(inner);
                discard_staging(&live.staged);
                emit_changed(app);
                return true;
            }
            inner.live.get_mut(&seq).expect("found above").status = DownloadStatus::Pending;
            drop(inner);
            emit_changed(app);
            let app = app.clone();
            tauri::async_runtime::spawn(async move { resolve(app, seq).await });
            true
        }
        _ => true,
    }
}

/// Download a file the app already holds (an Evolution Go media message):
/// it is staged like a finished page download and then follows the same
/// rules, folder, history and auto-open. The user started it, so the
/// multiple-download gate does not apply. Returns once it is decided.
pub async fn save_native<R: Runtime>(
    app: &AppHandle<R>,
    account_id: i64,
    suggested: &str,
    mime: &str,
    bytes: Vec<u8>,
) -> std::io::Result<()> {
    let state = app.state::<AppState>();
    let downloads = &state.downloads;
    let name = naming::normalized_file_name(suggested, mime, "");
    let seq = {
        let mut inner = downloads.inner.lock().unwrap();
        inner.seq += 1;
        inner.seq
    };
    let staged = downloads.staging.join(seq.to_string()).join(&name);
    let written = async {
        tokio::fs::create_dir_all(staged.parent().expect("staged has a parent")).await?;
        tokio::fs::write(&staged, bytes).await
    };
    if let Err(err) = written.await {
        discard_staging(&staged);
        return Err(err);
    }
    downloads.inner.lock().unwrap().live.insert(
        seq,
        Live {
            account_id,
            // Received already: no engine completion has to find it.
            url: String::new(),
            name,
            staged,
            status: DownloadStatus::Pending,
            needs_gate: false,
            answer: None,
        },
    );
    emit_changed(app);
    resolve(app.clone(), seq).await;
    Ok(())
}

fn discard_staging(staged: &Path) {
    if let Some(dir) = staged.parent() {
        let _ = std::fs::remove_dir_all(dir);
    }
}

/// Ask the UI and wait for its answer; a dropped prompt counts as Cancel.
async fn prompt<R: Runtime>(
    app: &AppHandle<R>,
    seq: u64,
    kind: PromptKind,
    folder: &Path,
) -> DownloadDecision {
    let state = app.state::<AppState>();
    let (tx, rx) = oneshot::channel();
    let name = {
        let mut inner = state.downloads.inner.lock().unwrap();
        let Some(live) = inner.live.get_mut(&seq) else {
            return DownloadDecision::Cancel;
        };
        live.answer = Some(tx);
        live.name.clone()
    };
    let _ = app.emit(
        DOWNLOAD_PROMPT,
        DownloadPrompt {
            id: seq,
            kind,
            name,
            folder: path_string(folder),
        },
    );
    rx.await.unwrap_or(DownloadDecision::Cancel)
}

/// The configured folder, falling back to (and resetting to) the default
/// when it cannot be used.
fn target_dir<R: Runtime>(app: &AppHandle<R>) -> PathBuf {
    let configured = app
        .state::<AppState>()
        .settings
        .get()
        .downloads
        .download_path;
    if !configured.is_empty() {
        let dir = PathBuf::from(&configured);
        if std::fs::create_dir_all(&dir).is_ok() && dir.is_dir() {
            return dir;
        }
        log::warn!("download folder is not usable; using the default");
        let _ = effects::change_settings(app, |s| s.downloads.download_path.clear());
    }
    let dir = default_dir();
    let _ = std::fs::create_dir_all(&dir);
    dir
}

async fn save_dialog<R: Runtime>(app: &AppHandle<R>, dir: &Path, name: &str) -> Option<PathBuf> {
    use tauri_plugin_dialog::DialogExt;
    let mut dialog = app
        .dialog()
        .file()
        .set_title(tr("Save file"))
        .set_directory(dir)
        .set_file_name(name);
    if let Some(ext) = Path::new(name).extension().and_then(|e| e.to_str()) {
        dialog = dialog.add_filter(format!("*.{ext}"), &[ext]);
    }
    let (tx, rx) = oneshot::channel();
    dialog.save_file(move |path| {
        let _ = tx.send(path.and_then(|p| p.into_path().ok()));
    });
    rx.await.ok().flatten()
}

async fn resolve<R: Runtime>(app: AppHandle<R>, seq: u64) {
    let state = app.state::<AppState>();
    let Some((account_id, name, needs_gate)) = state
        .downloads
        .inner
        .lock()
        .unwrap()
        .live
        .get(&seq)
        .map(|l| (l.account_id, l.name.clone(), l.needs_gate))
    else {
        return;
    };
    let dir = target_dir(&app);

    if needs_gate {
        match prompt(&app, seq, PromptKind::Multiple, &dir).await {
            DownloadDecision::AllowOnce => {}
            DownloadDecision::AlwaysAllow => {
                let _ = effects::change_settings(&app, |s| {
                    s.downloads.multiple_download_permission = TriState::Allow
                });
            }
            // Block, closing the dialog or Esc block for good, as in ZapZap.
            _ => {
                let _ = effects::change_settings(&app, |s| {
                    s.downloads.multiple_download_permission = TriState::Block
                });
                return finish(&app, seq, None, DownloadStatus::Blocked, false);
            }
        }
    }

    let settings = state.settings.get().downloads;
    let save_as_dir = || {
        let inner = state.downloads.inner.lock().unwrap();
        settings
            .remember_last_save_as_directory
            .then(|| inner.last_dirs.get(&account_id).cloned())
            .flatten()
            .unwrap_or_else(|| dir.clone())
    };
    let (target, open) = match settings.behavior {
        DownloadBehavior::Automatic => (Some(Target::Free(dir.join(&name))), false),
        DownloadBehavior::AskEveryTime => (
            save_dialog(&app, &save_as_dir(), &name)
                .await
                .map(Target::Chosen),
            false,
        ),
        DownloadBehavior::Dialog => loop {
            match prompt(&app, seq, PromptKind::Save, &dir).await {
                DownloadDecision::Save => break (Some(Target::Free(dir.join(&name))), false),
                DownloadDecision::Open => break (Some(Target::Free(dir.join(&name))), true),
                DownloadDecision::SaveAs => {
                    // Cancelling "Save as" returns to the Download dialog.
                    if let Some(path) = save_dialog(&app, &save_as_dir(), &name).await {
                        break (Some(Target::Chosen(path)), false);
                    }
                }
                _ => break (None, false),
            }
        },
    };
    let Some(target) = target else {
        // Cancelled before saving: nothing is recorded, as in ZapZap.
        let live = state.downloads.inner.lock().unwrap().live.remove(&seq);
        if let Some(live) = live {
            discard_staging(&live.staged);
        }
        emit_changed(&app);
        return;
    };
    if settings.remember_last_save_as_directory
        && settings.behavior != DownloadBehavior::Automatic
        && let Some(parent) = target.path().parent()
    {
        state
            .downloads
            .inner
            .lock()
            .unwrap()
            .last_dirs
            .insert(account_id, parent.to_path_buf());
    }
    finish(&app, seq, Some(target), DownloadStatus::Completed, open);
}

/// Where a finished download goes.
enum Target {
    /// Picked in the save dialog, which already confirmed any overwrite.
    Chosen(PathBuf),
    /// The download folder's name, suffixed with ` (N)` when taken.
    Free(PathBuf),
}

impl Target {
    fn path(&self) -> &Path {
        match self {
            Target::Chosen(p) | Target::Free(p) => p,
        }
    }
}

fn move_file(from: &Path, to: &Path) -> std::io::Result<()> {
    if std::fs::rename(from, to).is_ok() {
        return Ok(());
    }
    // Across filesystems: copy, then drop the staged file.
    std::fs::copy(from, to)?;
    std::fs::remove_file(from)
}

/// Move without replacing an existing file; `AlreadyExists` when taken.
fn move_new(from: &Path, to: &Path) -> std::io::Result<()> {
    match std::fs::hard_link(from, to) {
        Ok(()) => return std::fs::remove_file(from),
        Err(err) if err.kind() == std::io::ErrorKind::AlreadyExists => return Err(err),
        // Across filesystems or without hard links: exclusive create + copy.
        Err(_) => {}
    }
    let mut out = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(to)?;
    let copied = std::io::copy(&mut std::fs::File::open(from)?, &mut out);
    if let Err(err) = copied {
        drop(out);
        let _ = std::fs::remove_file(to);
        return Err(err);
    }
    std::fs::remove_file(from)
}

/// Claim a free name next to `wanted`; concurrent downloads of the same name
/// get distinct suffixes instead of replacing each other.
fn move_to_free(from: &Path, wanted: &Path) -> std::io::Result<PathBuf> {
    let dir = wanted.parent().unwrap_or(Path::new("."));
    let name = wanted
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    loop {
        let target = naming::unique_target(dir, &name);
        match move_new(from, &target) {
            Err(err) if err.kind() == std::io::ErrorKind::AlreadyExists => continue,
            other => return other.map(|()| target),
        }
    }
}

fn finish<R: Runtime>(
    app: &AppHandle<R>,
    seq: u64,
    target: Option<Target>,
    status: DownloadStatus,
    open: bool,
) {
    let state = app.state::<AppState>();
    let Some(live) = state.downloads.inner.lock().unwrap().live.remove(&seq) else {
        return;
    };
    let mut status = status;
    let mut saved = None;
    if let Some(target) = target {
        let moved = match target {
            Target::Chosen(path) => move_file(&live.staged, &path).map(|()| path),
            Target::Free(wanted) => move_to_free(&live.staged, &wanted),
        };
        match moved {
            Ok(path) => saved = Some(path),
            Err(err) => {
                log::warn!("could not save download: {err}");
                status = DownloadStatus::Failed;
            }
        }
    }
    discard_staging(&live.staged);
    {
        let mut inner = state.downloads.inner.lock().unwrap();
        Downloads::record(&mut inner, seq, live.name.clone(), saved.clone(), status);
    }
    if let Some(path) = saved {
        let path_str = path_string(&path);
        let _ = effects::change_settings(app, |s| {
            let recent = &mut s.downloads.recent;
            recent.retain(|p| !same_path(p, &path_str));
            recent.insert(0, path_str.clone());
            recent.truncate(MAX_RECENT);
        });
        let settings = state.settings.get().downloads;
        let auto = match sniff::auto_open_kind(&path) {
            Some(sniff::Kind::Pdf) => settings.auto_open_pdf,
            Some(sniff::Kind::Image) => settings.auto_open_images,
            None => false,
        };
        if open || auto {
            let _ = tauri_plugin_opener::open_path(&path, None::<&str>);
        }
        let _ = app.emit(DOWNLOAD_SAVED, &path_str);
    }
    emit_changed(app);
}

/// The system Downloads folder, falling back to the home directory.
pub fn default_dir() -> PathBuf {
    dirs::download_dir()
        .or_else(dirs::home_dir)
        .unwrap_or_else(std::env::temp_dir)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn free_targets_never_replace_files() {
        let dir = tempfile::tempdir().unwrap();
        let wanted = dir.path().join("photo.jpg");
        std::fs::write(&wanted, b"first").unwrap();
        let staged = dir.path().join("staged");
        std::fs::write(&staged, b"second").unwrap();
        let saved = move_to_free(&staged, &wanted).unwrap();
        assert_eq!(saved, dir.path().join("photo (1).jpg"));
        assert_eq!(std::fs::read(&wanted).unwrap(), b"first");
        assert_eq!(std::fs::read(&saved).unwrap(), b"second");
        assert!(!staged.exists());
    }

    #[test]
    fn items_merge_session_and_history() {
        let downloads = Downloads::new(tempfile::tempdir().unwrap().path().join("staging"));
        {
            let mut inner = downloads.inner.lock().unwrap();
            Downloads::record(
                &mut inner,
                1,
                "a.pdf".into(),
                Some("/d/a.pdf".into()),
                DownloadStatus::Completed,
            );
            Downloads::record(&mut inner, 2, "b.pdf".into(), None, DownloadStatus::Blocked);
        }
        let items = downloads.items(&["/d/a.pdf".into(), "/d/old.txt".into()]);
        let keys: Vec<_> = items.iter().map(|i| i.key.as_str()).collect();
        assert_eq!(keys, ["record-2", "record-1", "completed-1-/d/old.txt"]);
        assert_eq!(items[2].name, "old.txt");
        assert!(downloads.knows("/d/old.txt", &["/d/old.txt".into()]));
        assert!(!downloads.knows("/etc/passwd", &[]));
        downloads.remove_record("/d/a.pdf");
        assert_eq!(downloads.items(&[]).len(), 1);
    }

    #[test]
    fn session_records_are_capped() {
        let downloads = Downloads::new(tempfile::tempdir().unwrap().path().join("staging"));
        let mut inner = downloads.inner.lock().unwrap();
        for seq in 0..150 {
            Downloads::record(&mut inner, seq, "x".into(), None, DownloadStatus::Cancelled);
        }
        assert_eq!(inner.records.len(), MAX_SESSION_RECORDS);
        assert_eq!(inner.records[0].seq, 50);
    }

    fn live(account_id: i64, url: &str, staged: &str) -> Live {
        Live {
            account_id,
            url: url.into(),
            name: "f.pdf".into(),
            staged: staged.into(),
            status: DownloadStatus::Active,
            needs_gate: false,
            answer: None,
        }
    }

    #[test]
    fn repeated_completions_are_ignored() {
        let mut inner = Inner::default();
        inner.live.insert(1, live(7, "blob:a", "/s/1/f.pdf"));
        inner.live.insert(2, live(7, "blob:b", "/s/2/f.pdf"));
        let path = Path::new("/s/2/f.pdf");
        assert_eq!(inner.completed_seq(7, "blob:b", Some(path)), Some(2));
        inner.live.get_mut(&2).unwrap().status = DownloadStatus::Pending;
        // A second registration reports the same completion again.
        assert_eq!(inner.completed_seq(7, "blob:b", Some(path)), None);
        assert_eq!(inner.completed_seq(7, "blob:b", None), None);
        assert_eq!(inner.completed_seq(7, "blob:a", None), Some(1));
        assert_eq!(inner.completed_seq(8, "blob:a", None), None);
    }
}
