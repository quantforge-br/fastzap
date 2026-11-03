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

/// Async because settings changes may recreate webviews (see `accounts`).
#[tauri::command]
pub async fn settings_patch(
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

/// Resolved interface locale (`ll_CC`); `"system"` follows the OS.
#[tauri::command]
pub fn app_locale(state: State<'_, AppState>) -> String {
    crate::i18n::resolve_locale(&state.settings.get().system.interface_language)
}
