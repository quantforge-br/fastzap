use serde_json::Value;
use tauri::{AppHandle, Emitter, State};

use super::CommandResult;
use crate::app::effects;
use crate::app::events::SETTINGS_CHANGED;
use crate::app::state::AppState;
use crate::settings::Settings;

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
    let old = state.settings.get();
    let settings = state.settings.patch(&patch)?;
    app.emit(SETTINGS_CHANGED, &settings)?;
    effects::on_settings_changed(&app, &old, &settings);
    Ok(settings)
}
