use serde_json::Value;
use tauri::{AppHandle, Emitter, State};

use super::CommandResult;
use crate::app::state::AppState;
use crate::settings::Settings;

pub const SETTINGS_CHANGED: &str = "settings://changed";

#[tauri::command]
pub fn settings_get(state: State<'_, AppState>) -> Settings {
    state.settings.get()
}

#[tauri::command]
pub fn settings_patch(
    app: AppHandle,
    state: State<'_, AppState>,
    patch: Value,
) -> CommandResult<Settings> {
    let settings = state.settings.patch(&patch)?;
    app.emit(SETTINGS_CHANGED, &settings)?;
    Ok(settings)
}
