use tauri::{AppHandle, Emitter, State};

use super::CommandResult;
use crate::accounts::{Account, AccountPatch, NewAccount, user_agent};
use crate::app::effects;
use crate::app::events::ACCOUNTS_CHANGED;
use crate::app::state::AppState;

fn changed(app: &AppHandle, state: &AppState) -> CommandResult<Vec<Account>> {
    let list = state.accounts.list()?;
    app.emit(ACCOUNTS_CHANGED, &list)?;
    effects::on_accounts_changed(app);
    Ok(list)
}

#[tauri::command]
pub fn accounts_list(state: State<'_, AppState>) -> CommandResult<Vec<Account>> {
    Ok(state.accounts.list()?)
}

// Commands that may create or recreate webviews are async: creating a
// WebView2 instance inside a synchronous command deadlocks on Windows.

#[tauri::command]
pub async fn accounts_create(
    app: AppHandle,
    state: State<'_, AppState>,
    account: NewAccount,
) -> CommandResult<Account> {
    let limit = state.settings.get().system.max_accounts;
    let created = state.accounts.create(account, limit)?;
    changed(&app, &state)?;
    Ok(created)
}

#[tauri::command]
pub async fn accounts_update(
    app: AppHandle,
    state: State<'_, AppState>,
    id: i64,
    patch: AccountPatch,
) -> CommandResult<Account> {
    let before = state.accounts.get(id)?;
    let updated = state.accounts.update(id, patch)?;
    if before.user_agent != updated.user_agent {
        state.webviews.recreate(&app, id)?;
    }
    if before.zoom != updated.zoom {
        state.webviews.set_zoom(&app, id, updated.zoom);
    }
    changed(&app, &state)?;
    Ok(updated)
}

#[tauri::command]
pub async fn accounts_delete(
    app: AppHandle,
    state: State<'_, AppState>,
    id: i64,
) -> CommandResult<()> {
    state.accounts.delete(id)?;
    // Close the webview before deleting its data directory.
    if let Some(webview) = tauri::Manager::get_webview(&app, &crate::webview::manager::label(id)) {
        let _ = webview.close();
    }
    let _ = std::fs::remove_dir_all(state.paths.profile_dir(id));
    let _ = std::fs::remove_dir_all(state.paths.evolution_dir(id));
    for key in [
        crate::secrets::evolution_admin_key(id),
        crate::secrets::evolution_instance_token(id),
    ] {
        let _ = state.secrets.delete(&key);
    }
    let snapshot = state.unread.remove(id);
    effects::on_unread_changed(&app, &snapshot);
    changed(&app, &state)?;
    Ok(())
}

#[tauri::command]
pub async fn accounts_reorder(
    app: AppHandle,
    state: State<'_, AppState>,
    ids: Vec<i64>,
) -> CommandResult<()> {
    state.accounts.reorder(&ids)?;
    changed(&app, &state)?;
    Ok(())
}

#[tauri::command]
pub fn user_agent_presets() -> Vec<&'static str> {
    user_agent::keys()
}
