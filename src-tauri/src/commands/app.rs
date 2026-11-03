//! Application-level queries and actions for the local UI.

use tauri::{AppHandle, Manager, State};

use super::CommandResult;
use crate::app::state::AppState;
use crate::build_info::{self, AppInfo};

#[tauri::command]
pub fn app_info() -> AppInfo {
    build_info::app_info()
}

#[tauri::command]
pub fn panel_supports_positioning() -> bool {
    crate::app::panel::supports_positioning()
}

/// Store the proxy password in the keyring; an empty value removes it.
#[tauri::command]
pub fn proxy_password_set(state: State<'_, AppState>, password: String) -> CommandResult<()> {
    let key = crate::secrets::PROXY_PASSWORD;
    if password.is_empty() {
        state.secrets.delete(key)?;
    } else {
        state
            .secrets
            .set(key, &crate::secrets::Secret::new(password))?;
    }
    Ok(())
}

/// Whether a proxy password is stored; the value never leaves the backend.
#[tauri::command]
pub fn proxy_password_present(state: State<'_, AppState>) -> bool {
    matches!(
        state.secrets.get(crate::secrets::PROXY_PASSWORD),
        Ok(Some(_))
    )
}

/// Reset every setting to its default, keeping the import marker so ZapZap
/// data is not imported again.
#[tauri::command]
pub async fn settings_reset(app: AppHandle) -> CommandResult<()> {
    let state = app.state::<AppState>();
    let migration = state.settings.get().migration;
    let defaults = crate::settings::Settings {
        migration,
        ..Default::default()
    };
    let old = state.settings.get();
    let settings = state.settings.replace(defaults)?;
    tauri::Emitter::emit(&app, crate::app::events::SETTINGS_CHANGED, &settings)?;
    crate::app::effects::on_settings_changed(&app, &old, &settings);
    Ok(())
}

/// Folder chosen in the system dialog, starting at `start`.
#[tauri::command]
pub async fn dialog_pick_folder(app: AppHandle, start: Option<String>) -> Option<String> {
    use tauri_plugin_dialog::DialogExt;
    let mut dialog = app.dialog().file();
    if let Some(start) = start.filter(|s| !s.is_empty()) {
        dialog = dialog.set_directory(start);
    }
    dialog
        .blocking_pick_folder()
        .and_then(|p| p.into_path().ok())
        .map(|p| p.to_string_lossy().into_owned())
}

/// Files chosen in the system dialog, filtered by extension.
#[tauri::command]
pub async fn dialog_pick_files(
    app: AppHandle,
    title: String,
    filter_name: String,
    extensions: Vec<String>,
    multiple: bool,
) -> Vec<String> {
    use tauri_plugin_dialog::DialogExt;
    let extensions: Vec<&str> = extensions.iter().map(String::as_str).collect();
    let dialog = app
        .dialog()
        .file()
        .set_title(title)
        .add_filter(filter_name, &extensions);
    let picked = if multiple {
        dialog.blocking_pick_files().unwrap_or_default()
    } else {
        dialog.blocking_pick_file().into_iter().collect()
    };
    picked
        .into_iter()
        .filter_map(|p| p.into_path().ok())
        .map(|p| p.to_string_lossy().into_owned())
        .collect()
}

/// Open a file or folder with the system default application.
#[tauri::command]
pub fn open_path(path: String) -> CommandResult<()> {
    tauri_plugin_opener::open_path(path, None::<&str>)?;
    Ok(())
}

/// Reveal a file in the system file manager.
#[tauri::command]
pub fn reveal_path(path: String) -> CommandResult<()> {
    tauri_plugin_opener::reveal_item_in_dir(path)?;
    Ok(())
}

/// The system Downloads folder, used when no custom folder is set.
#[tauri::command]
pub fn downloads_default_dir() -> String {
    crate::downloads::default_dir()
        .to_string_lossy()
        .into_owned()
}

/// The user's answer to a permission request of a WhatsApp Web page.
#[tauri::command]
pub fn permission_answer(app: AppHandle, id: u64, allow: bool) {
    crate::permissions::answer(&app, id, allow);
}

#[tauri::command]
pub fn update_info(
    state: tauri::State<'_, crate::update::UpdateState>,
) -> Option<crate::update::UpdateInfo> {
    state.get()
}

/// Open a donation method's official page (ids from `donation::url`).
#[tauri::command]
pub fn donation_open(method: String) -> CommandResult<()> {
    let url = crate::donation::url(&method)
        .ok_or_else(|| super::CommandError::from(format!("unknown donation method {method}")))?;
    tauri_plugin_opener::open_url(url, None::<&str>)?;
    Ok(())
}

#[tauri::command]
pub fn donation_open_website() -> CommandResult<()> {
    tauri_plugin_opener::open_url(crate::donation::WEBSITE, None::<&str>)?;
    Ok(())
}
