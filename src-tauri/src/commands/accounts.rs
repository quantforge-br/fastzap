use tauri::{AppHandle, Emitter, State};

use super::CommandResult;
use crate::accounts::{Account, AccountPatch, NewAccount, user_agent};
use crate::app::state::AppState;

pub const ACCOUNTS_CHANGED: &str = "accounts://changed";

fn changed(app: &AppHandle, state: &AppState) -> CommandResult<Vec<Account>> {
    let list = state.accounts.list()?;
    app.emit(ACCOUNTS_CHANGED, &list)?;
    Ok(list)
}

#[tauri::command]
pub fn accounts_list(state: State<'_, AppState>) -> CommandResult<Vec<Account>> {
    Ok(state.accounts.list()?)
}

#[tauri::command]
pub fn accounts_create(
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
pub fn accounts_update(
    app: AppHandle,
    state: State<'_, AppState>,
    id: i64,
    patch: AccountPatch,
) -> CommandResult<Account> {
    let updated = state.accounts.update(id, patch)?;
    changed(&app, &state)?;
    Ok(updated)
}

#[tauri::command]
pub fn accounts_delete(app: AppHandle, state: State<'_, AppState>, id: i64) -> CommandResult<()> {
    state.accounts.delete(id)?;
    let _ = std::fs::remove_dir_all(state.paths.profile_dir(id));
    let _ = std::fs::remove_dir_all(state.paths.evolution_dir(id));
    changed(&app, &state)?;
    Ok(())
}

#[tauri::command]
pub fn accounts_reorder(
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
