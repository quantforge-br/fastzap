//! Unclean-exit detection (`.session-active`), panic capture and a buffer of
//! this session's warnings for manual reports.

use std::collections::VecDeque;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, OnceLock};

use serde_json::{Value, json};

use super::document;
use super::store::{ReportStatus, ReportStore};

pub const MARKER: &str = ".session-active";
const LOG_CAPACITY: usize = 16_000;

static SESSION_ID: OnceLock<String> = OnceLock::new();
static REPORTS_DIR: OnceLock<PathBuf> = OnceLock::new();
/// Mirrors `reporting.crash_prompts` for the panic hook.
pub static CRASH_PROMPTS: AtomicBool = AtomicBool::new(false);
static LAST_PANIC: Mutex<Option<Value>> = Mutex::new(None);
static LOG: Mutex<VecDeque<String>> = Mutex::new(VecDeque::new());

fn marker_path(dir: &Path) -> PathBuf {
    dir.join(MARKER)
}

fn write_marker(dir: &Path, phase: &str) {
    let Some(session_id) = SESSION_ID.get() else {
        return;
    };
    let path = marker_path(dir);
    // Only our own marker is updated.
    if phase != "startup"
        && read_marker(dir)
            .and_then(|m| m.get("session_id")?.as_str().map(str::to_owned))
            .as_deref()
            != Some(session_id)
    {
        return;
    }
    let marker = json!({
        "schema_version": 1,
        "session_id": session_id,
        "pid": std::process::id(),
        "started_at": document::now(),
        "phase": phase,
        "system_information": document::system_information(),
    });
    let tmp = dir.join(format!("{MARKER}.tmp"));
    let ok = std::fs::create_dir_all(dir).is_ok()
        && std::fs::write(&tmp, marker.to_string()).is_ok()
        && std::fs::rename(&tmp, &path).is_ok();
    if !ok {
        log::warn!("could not write the session marker");
    }
}

fn read_marker(dir: &Path) -> Option<Value> {
    let bytes = std::fs::read(marker_path(dir)).ok()?;
    if bytes.len() > 64 * 1024 {
        return None;
    }
    serde_json::from_slice(&bytes).ok()
}

/// Start the session: a marker left by the previous run becomes a report
/// (when crash prompts are on), then ours replaces it.
pub fn start(store: &ReportStore, crash_prompts: bool) {
    CRASH_PROMPTS.store(crash_prompts, Ordering::Relaxed);
    let dir = store.dir().to_path_buf();
    if let Some(previous) = read_marker(&dir)
        && crash_prompts
        // A panic already produced its own report.
        && previous.get("phase").and_then(Value::as_str) != Some("crashed")
        && let Err(e) = store.save(
            document::unexpected_shutdown(&previous),
            ReportStatus::PendingReview,
        )
    {
        log::warn!("could not save the unexpected shutdown report: {e}");
    }
    let _ = SESSION_ID.set(super::store::new_id());
    let _ = REPORTS_DIR.set(dir.clone());
    write_marker(&dir, "startup");
}

pub fn mark(phase: &str) {
    if let Some(dir) = REPORTS_DIR.get() {
        write_marker(dir, phase);
    }
}

/// A clean exit removes our marker.
pub fn close() {
    let (Some(dir), Some(id)) = (REPORTS_DIR.get(), SESSION_ID.get()) else {
        return;
    };
    if read_marker(dir)
        .and_then(|m| m.get("session_id")?.as_str().map(str::to_owned))
        .as_deref()
        == Some(id.as_str())
    {
        let _ = std::fs::remove_file(marker_path(dir));
    }
}

pub fn last_panic() -> Option<Value> {
    LAST_PANIC.lock().ok()?.clone()
}

/// Record panics; with crash prompts on they become a report to review on
/// the next start (release builds abort right after).
pub fn install_panic_hook() {
    let previous = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let message = info
            .payload()
            .downcast_ref::<&str>()
            .map(|s| (*s).to_owned())
            .or_else(|| info.payload().downcast_ref::<String>().cloned())
            .unwrap_or_else(|| "panic".into());
        let location = info
            .location()
            .map(|l| format!("{}:{}", l.file(), l.line()))
            .unwrap_or_default();
        let backtrace = std::backtrace::Backtrace::force_capture().to_string();
        let error = document::error_information(
            "panic",
            &message,
            &format!("at {location}\n\n{backtrace}"),
        );
        if let Ok(mut last) = LAST_PANIC.lock() {
            *last = Some(error.clone());
        }
        if CRASH_PROMPTS.load(Ordering::Relaxed)
            && let Some(dir) = REPORTS_DIR.get()
        {
            let store = ReportStore::new(dir.clone());
            let fp = document::fingerprint(&["panic", &location]);
            let _ = store.save(document::crash(error, fp), ReportStatus::PendingReview);
            write_marker(dir, "crashed");
        }
        previous(info);
    }));
}

/// The buffered warnings and errors of this session.
pub fn recent_logs() -> String {
    LOG.lock()
        .map(|l| l.iter().cloned().collect::<Vec<_>>().join("\n"))
        .unwrap_or_default()
}

/// Wraps the stderr logger and keeps warnings and errors in memory.
pub struct TeeLogger {
    inner: env_logger::Logger,
}

impl TeeLogger {
    pub fn init(inner: env_logger::Logger) {
        let max = inner.filter();
        if log::set_boxed_logger(Box::new(Self { inner })).is_ok() {
            log::set_max_level(max.max(log::LevelFilter::Warn));
        }
    }
}

impl log::Log for TeeLogger {
    fn enabled(&self, metadata: &log::Metadata) -> bool {
        metadata.level() <= log::Level::Warn || self.inner.enabled(metadata)
    }

    fn log(&self, record: &log::Record) {
        if record.level() <= log::Level::Warn
            && let Ok(mut buffer) = LOG.lock()
        {
            buffer.push_back(format!(
                "{} {} {} {}",
                document::now(),
                record.level(),
                record.target(),
                record.args()
            ));
            let mut size: usize = buffer.iter().map(String::len).sum();
            while size > LOG_CAPACITY {
                size -= buffer.pop_front().map_or(0, |l| l.len());
            }
        }
        if self.inner.matches(record) {
            self.inner.log(record);
        }
    }

    fn flush(&self) {
        self.inner.flush();
    }
}
