//! Commands callable from https://web.whatsapp.com (see capabilities/whatsapp-remote.json).
//! The calling account is always derived from the webview label, never from arguments.

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, Webview};
use ts_rs::TS;

use super::CommandResult;
use crate::accounts::AccountPatch;
use crate::app::events::{ACCOUNT_SELECTOR, ACCOUNTS_CHANGED};
use crate::app::{actions, effects, state::AppState, theme};
use crate::webview::manager::account_id_from_label;
use crate::webview::{scripts, unread};

fn caller(webview: &Webview) -> CommandResult<i64> {
    Ok(account_id_from_label(webview.label()).ok_or("not an account webview")?)
}

#[tauri::command]
pub fn wa_title_changed(app: AppHandle, webview: Webview, title: String) -> CommandResult<()> {
    let id = caller(&webview)?;
    let state = app.state::<AppState>();
    if let Some(snapshot) = state.unread.set(id, unread::count_from_title(&title)) {
        effects::on_unread_changed(&app, &snapshot);
    }
    Ok(())
}

#[tauri::command]
pub fn wa_shortcut(app: AppHandle, webview: Webview, action: String) -> CommandResult<()> {
    caller(&webview)?;
    actions::dispatch(&app, &action);
    Ok(())
}

#[tauri::command]
pub fn wa_theme_event(
    app: AppHandle,
    webview: Webview,
    event: String,
    message: Option<String>,
) -> CommandResult<()> {
    caller(&webview)?;
    let state = app.state::<AppState>();
    match event.as_str() {
        // WhatsApp's own theme setting is overridden by FastZap's, as in ZapZap.
        "ready" | "changed" => {
            let dark = theme::is_dark(&app, &state.settings.get());
            webview.eval(scripts::apply_theme(dark))?;
        }
        "selector_ready" => effects::refresh_account_selectors(&app),
        "failed" => {
            log::warn!(
                "WhatsApp Web theme controller failed: {}",
                message.unwrap_or_default()
            );
        }
        _ => {}
    }
    Ok(())
}

#[derive(Debug, Clone, Serialize, TS)]
#[ts(export)]
pub struct AccountSelectorRequest {
    #[ts(type = "number")]
    pub account_id: i64,
    /// Button rectangle in window coordinates (logical pixels).
    pub rect: [f64; 4],
}

#[tauri::command]
pub fn wa_open_account_selector(
    app: AppHandle,
    webview: Webview,
    rect: [f64; 4],
) -> CommandResult<()> {
    let id = caller(&webview)?;
    let scale = webview.window().scale_factor().unwrap_or(1.0);
    let origin = webview
        .position()
        .map(|p| p.to_logical::<f64>(scale))
        .unwrap_or_default();
    let zoom = app
        .state::<AppState>()
        .accounts
        .get(id)
        .map(|a| a.zoom)
        .unwrap_or(1.0);
    let [x, y, w, h] = rect;
    app.emit(
        ACCOUNT_SELECTOR,
        AccountSelectorRequest {
            account_id: id,
            rect: [origin.x + x * zoom, origin.y + y * zoom, w * zoom, h * zoom],
        },
    )?;
    Ok(())
}

#[tauri::command]
pub async fn wa_profile_photo(
    app: AppHandle,
    webview: Webview,
    url: Option<String>,
) -> CommandResult<()> {
    let id = caller(&webview)?;
    let url = url.ok_or("no profile photo available")?;
    let icon = crate::accounts::avatar::download(&url).await?;
    let state = app.state::<AppState>();
    state.accounts.update(
        id,
        AccountPatch {
            icon: Some(Some(icon)),
            ..Default::default()
        },
    )?;
    app.emit(ACCOUNTS_CHANGED, state.accounts.list()?)?;
    Ok(())
}
