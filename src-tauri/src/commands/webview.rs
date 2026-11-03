//! Commands from the local UI that drive account webviews.

use tauri::{AppHandle, Emitter, State};

use super::CommandResult;
use crate::app::actions;
use crate::app::events::LAYOUT_CHANGED;
use crate::app::state::AppState;
use crate::webview::deeplink::{self, ChatTarget, RegionCallingCode};
use crate::webview::manager::{LayoutState, Rect};
use crate::webview::scripts;
use crate::webview::unread::UnreadSnapshot;

#[tauri::command]
pub fn layout_get(state: State<'_, AppState>) -> LayoutState {
    state.webviews.state()
}

#[tauri::command]
pub fn layout_set_bounds(app: AppHandle, state: State<'_, AppState>, bounds: Rect) {
    state.webviews.set_bounds(&app, bounds);
}

#[tauri::command]
pub fn layout_set_overlay(app: AppHandle, state: State<'_, AppState>, overlay: bool) {
    state.webviews.set_overlay(&app, overlay);
    let _ = app.emit(LAYOUT_CHANGED, state.webviews.state());
}

#[tauri::command]
pub fn account_activate(app: AppHandle, id: i64) {
    actions::activate(&app, id);
}

#[tauri::command]
pub fn app_action(app: AppHandle, action: String) {
    actions::dispatch(&app, &action);
}

#[tauri::command]
pub fn unread_get(state: State<'_, AppState>) -> UnreadSnapshot {
    state.unread.snapshot()
}

#[tauri::command]
pub fn account_sync_photo(app: AppHandle, state: State<'_, AppState>, id: i64) {
    state.webviews.eval(&app, id, scripts::read_profile_photo());
}

#[tauri::command]
pub fn calling_codes() -> Vec<RegionCallingCode> {
    deeplink::region_calling_codes()
}

/// Validate a number and open the conversation in the given (or active) account.
#[tauri::command]
pub fn open_chat_by_number(
    app: AppHandle,
    state: State<'_, AppState>,
    target: ChatTarget,
    account_id: Option<i64>,
) -> CommandResult<()> {
    let target = deeplink::validate_chat_target(&target)?;
    let id = account_id
        .or_else(|| state.webviews.active())
        .ok_or("no active account")?;
    state
        .webviews
        .navigate(&app, id, deeplink::open_chat_url(&target));
    actions::activate(&app, id);
    Ok(())
}

/// Open a `whatsapp:`/wa.me link in the active WhatsApp Web account.
pub fn open_deep_link(app: &AppHandle, state: &AppState, url: &str) -> bool {
    let Some(script) = deeplink::open_deep_link_script(url) else {
        return false;
    };
    let Some(id) = state.webviews.active() else {
        return false;
    };
    state.webviews.eval(app, id, &script);
    crate::app::window::show(app);
    true
}
