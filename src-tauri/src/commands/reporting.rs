use serde::Serialize;
use serde_json::Value;
use tauri::State;
use ts_rs::TS;

use super::CommandResult;
use crate::app::state::AppState;
use crate::reporting::document::{self, ProblemForm};
use crate::reporting::sanitize;
use crate::reporting::session;
use crate::reporting::store::{ReportRecord, ReportStatus, ReportStore};

#[derive(Debug, Clone, Serialize, TS)]
#[ts(export)]
pub struct ReportView {
    /// Absent for a report not saved yet.
    pub id: Option<String>,
    pub status: Option<ReportStatus>,
    pub created_at: Option<String>,
    #[ts(type = "Record<string, unknown>")]
    pub document: Value,
    pub markdown: String,
}

impl From<ReportRecord> for ReportView {
    fn from(r: ReportRecord) -> Self {
        Self {
            markdown: document::markdown(&r.document),
            id: Some(r.id),
            status: Some(r.status),
            created_at: Some(r.created_at),
            document: r.document,
        }
    }
}

fn store(state: &AppState) -> ReportStore {
    ReportStore::new(state.paths.reports_dir())
}

/// Build the sanitized document of the report form for review.
#[tauri::command]
pub fn report_preview(form: ProblemForm) -> ReportView {
    let doc = document::manual(&form, session::last_panic(), &session::recent_logs());
    ReportView {
        id: None,
        status: None,
        created_at: None,
        markdown: document::markdown(&doc),
        document: doc,
    }
}

#[derive(Debug, Clone, Serialize, TS)]
#[ts(export)]
pub struct ReportHandoff {
    pub id: String,
    pub markdown: String,
}

/// Keep the reviewed report as `copied` before the UI copies it.
#[tauri::command]
pub fn report_save(
    state: State<'_, AppState>,
    id: Option<String>,
    document: Value,
) -> CommandResult<ReportHandoff> {
    let store = store(&state);
    // Marked copied by `report_copied` once the UI wrote the clipboard.
    let record = match id.and_then(|id| store.get(&id)) {
        Some(record) => record,
        None => store.save(sanitize::value(&document), ReportStatus::PendingReview)?,
    };
    Ok(ReportHandoff {
        markdown: document::markdown(&record.document),
        id: record.id,
    })
}

#[tauri::command]
pub fn report_copied(state: State<'_, AppState>, id: String) -> CommandResult<()> {
    store(&state).set_status(&id, ReportStatus::Copied)?;
    Ok(())
}

/// Open the new-issue page; returns whether a browser was launched.
#[tauri::command]
pub fn report_open_github(state: State<'_, AppState>, id: String) -> CommandResult<bool> {
    let store = store(&state);
    let record = store.get(&id).ok_or("report not found")?;
    let opened =
        tauri_plugin_opener::open_url(document::issue_url(&record.document), None::<&str>).is_ok();
    if opened {
        store.set_status(&id, ReportStatus::OpenedOnGithub)?;
    }
    Ok(opened)
}

#[tauri::command]
pub fn report_list(state: State<'_, AppState>) -> Vec<ReportView> {
    store(&state).list().into_iter().map(Into::into).collect()
}

/// The newest automatic report waiting for review, for the startup prompt.
#[tauri::command]
pub fn report_pending(state: State<'_, AppState>) -> Option<ReportView> {
    let settings = state.settings.get();
    // Not during first-run setup, as in ZapZap.
    if !settings.reporting.crash_prompts || !settings.onboarding.completed {
        return None;
    }
    store(&state)
        .list()
        .into_iter()
        .find(|r| {
            r.status == ReportStatus::PendingReview
                && r.document.get("report_type").and_then(Value::as_str) != Some(document::MANUAL)
        })
        .map(Into::into)
}

/// Keep a pending report locally without asking again.
#[tauri::command]
pub fn report_keep(state: State<'_, AppState>, id: String) -> CommandResult<()> {
    store(&state).set_status(&id, ReportStatus::Pending)?;
    Ok(())
}

#[tauri::command]
pub fn report_delete(state: State<'_, AppState>, id: String) -> CommandResult<()> {
    store(&state).delete(&id)?;
    Ok(())
}

#[tauri::command]
pub fn report_clear(state: State<'_, AppState>) {
    store(&state).clear();
}

#[tauri::command]
pub fn report_open_folder(state: State<'_, AppState>) -> CommandResult<()> {
    let dir = state.paths.reports_dir();
    std::fs::create_dir_all(&dir)?;
    tauri_plugin_opener::open_path(&dir, None::<&str>)?;
    Ok(())
}
