use tauri::{AppHandle, Emitter, State};

use super::CommandResult;
use crate::app::effects;
use crate::app::state::AppState;
use crate::downloads::{DOWNLOADS_CHANGED, DownloadDecision, DownloadItem, same_path};

#[tauri::command]
pub fn downloads_list(state: State<'_, AppState>) -> Vec<DownloadItem> {
    state
        .downloads
        .items(&state.settings.get().downloads.recent)
}

#[tauri::command]
pub fn download_answer(state: State<'_, AppState>, id: u64, decision: DownloadDecision) -> bool {
    state.downloads.answer(id, decision)
}

#[tauri::command]
pub fn downloads_clear_history(app: AppHandle, state: State<'_, AppState>) -> CommandResult<()> {
    state.downloads.clear_records();
    effects::change_settings(&app, |s| s.downloads.recent.clear())?;
    app.emit(DOWNLOADS_CHANGED, ())?;
    Ok(())
}

/// Only files FastZap saved may be opened, revealed or deleted from here.
fn known(state: &AppState, path: &str) -> CommandResult<()> {
    if state
        .downloads
        .knows(path, &state.settings.get().downloads.recent)
    {
        Ok(())
    } else {
        Err("not a download".into())
    }
}

#[tauri::command]
pub fn download_remove(
    app: AppHandle,
    state: State<'_, AppState>,
    path: String,
) -> CommandResult<()> {
    known(&state, &path)?;
    state.downloads.remove_record(&path);
    effects::change_settings(&app, |s| {
        s.downloads.recent.retain(|p| !same_path(p, &path))
    })?;
    app.emit(DOWNLOADS_CHANGED, ())?;
    Ok(())
}

/// Delete the file; the history entry stays and shows it missing.
#[tauri::command]
pub fn download_delete_file(
    app: AppHandle,
    state: State<'_, AppState>,
    path: String,
) -> CommandResult<()> {
    known(&state, &path)?;
    std::fs::remove_file(&path)?;
    app.emit(DOWNLOADS_CHANGED, ())?;
    Ok(())
}

#[tauri::command]
pub fn download_open(state: State<'_, AppState>, path: String) -> CommandResult<()> {
    known(&state, &path)?;
    tauri_plugin_opener::open_path(&path, None::<&str>)?;
    Ok(())
}

#[tauri::command]
pub fn download_reveal(state: State<'_, AppState>, path: String) -> CommandResult<()> {
    known(&state, &path)?;
    tauri_plugin_opener::reveal_item_in_dir(&path)?;
    Ok(())
}
