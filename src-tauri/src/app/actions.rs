//! Application actions triggered by shortcuts, the tray or the UI.

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, Runtime};
use ts_rs::TS;

use super::events::{LAYOUT_CHANGED, UI_ACTION};
use super::state::AppState;
use super::window;
use crate::accounts::{AccountKind, AccountPatch};
use crate::settings::ThemeMode;
use crate::webview::{clipboard, scripts};

const ZOOM_STEP: f64 = 0.1;

#[derive(Debug, Clone, Serialize, TS)]
#[ts(export)]
pub struct UiAction {
    pub action: String,
    #[ts(type = "number | null")]
    pub account_id: Option<i64>,
}

/// Actions handled by the UI itself; the window is shown first.
const UI_OWNED: &[&str] = &[
    "settings",
    "downloads",
    "add_account",
    "open_chat_by_number",
    "show_shortcuts",
    "report_problem",
    "spellcheck_languages",
];

pub fn dispatch<R: Runtime>(app: &AppHandle<R>, action: &str) {
    dispatch_for(app, action, None);
}

/// Run `action`; per-account actions target `account`, or the active account
/// when the action did not come from an account page.
pub fn dispatch_for<R: Runtime>(app: &AppHandle<R>, action: &str, account: Option<i64>) {
    let state = app.state::<AppState>();
    let target = account.or_else(|| state.webviews.active());
    match action {
        "quit" => super::lifecycle::request_close(app, true),
        "hide" => window::hide(app),
        "show" => window::show(app),
        "toggle_mute" => {
            let muted = !state.settings.get().system.audio_muted;
            let _ = super::effects::change_settings(app, |s| s.system.audio_muted = muted);
        }
        "toggle_window" => window::toggle_visibility(app),
        "show_panel" => super::panel::show(app),
        "restart" => app.restart(),
        "devtools" => {
            if let Some(webview) =
                target.and_then(|id| app.get_webview(&crate::webview::manager::label(id)))
            {
                webview.open_devtools();
            }
        }
        "compact_chat_list" => eval_on(
            app,
            target,
            "window._zapZapResizableChatList && window._zapZapResizableChatList.compact();",
        ),
        "fullscreen" => {
            if let Some(w) = window::main_window(app) {
                let _ = w.set_fullscreen(!w.is_fullscreen().unwrap_or(false));
            }
        }
        // ZapZap's tray "Accounts" opens the account overview (grid view).
        "accounts" => {
            state.webviews.set_grid(app, true);
            let _ = app.emit(LAYOUT_CHANGED, state.webviews.state());
            window::show(app);
            let _ = app.emit(
                UI_ACTION,
                UiAction {
                    action: action.to_owned(),
                    account_id: target,
                },
            );
        }
        "toggle_grid" => {
            state.webviews.toggle_grid(app);
            let _ = app.emit(LAYOUT_CHANGED, state.webviews.state());
        }
        "reload" => {
            if let Some(id) = target {
                state.webviews.reload(app, id);
            }
        }
        "zoom_in" | "zoom_out" | "zoom_reset" => {
            if let Some(id) = target {
                change_zoom(app, id, action);
            }
        }
        "theme_auto" | "theme_light" | "theme_dark" => {
            let theme = match action {
                "theme_light" => ThemeMode::Light,
                "theme_dark" => ThemeMode::Dark,
                _ => ThemeMode::Auto,
            };
            let _ = super::effects::change_settings(app, |s| s.appearance.theme = theme);
        }
        "new_chat" => eval_on(
            app,
            target,
            "window.__FASTZAP__ && window.__FASTZAP__.newChat();",
        ),
        "close_conversation" => eval_on(
            app,
            target,
            "window.__FASTZAP__ && window.__FASTZAP__.closeConversation();",
        ),
        "paste_plain_text" => {
            if let Some(id) = target {
                paste_plain_text(app, id);
            }
        }
        _ if action.starts_with("account_") => {
            if let Some(n) = action
                .strip_prefix("account_")
                .and_then(|n| n.parse::<usize>().ok())
            {
                activate_nth(app, n);
            }
        }
        _ if UI_OWNED.contains(&action) => {
            window::show(app);
            let _ = app.emit(
                UI_ACTION,
                UiAction {
                    action: action.to_owned(),
                    account_id: target,
                },
            );
        }
        other => log::debug!("unknown action {other}"),
    }
}

/// Apply the interface scale to the local UI, the panel and every account page.
pub fn apply_scale<R: Runtime>(app: &AppHandle<R>) {
    let state = app.state::<AppState>();
    let scale = crate::webview::manager::interface_scale(&state.settings.get());
    for label in [crate::app::window::UI_WEBVIEW, super::panel::PANEL_WINDOW] {
        if let Some(webview) = app.get_webview(label) {
            let _ = webview.set_zoom(scale);
        }
    }
    if let Ok(accounts) = state.accounts.list() {
        for account in accounts {
            state.webviews.set_zoom(app, account.id, account.zoom);
        }
    }
}

fn eval_on<R: Runtime>(app: &AppHandle<R>, account: Option<i64>, script: &str) {
    if let Some(id) = account {
        app.state::<AppState>().webviews.eval(app, id, script);
    }
}

/// Paste the clipboard as plain text into one account page.
pub fn paste_plain_text<R: Runtime>(app: &AppHandle<R>, account_id: i64) {
    if let Some(text) = clipboard::read_plain_text() {
        app.state::<AppState>()
            .webviews
            .eval(app, account_id, &scripts::paste_plain_text(&text));
    }
}

/// Activate the n-th (1-based) enabled account, as `Ctrl+1..9` in ZapZap.
pub fn activate_nth<R: Runtime>(app: &AppHandle<R>, n: usize) {
    let state = app.state::<AppState>();
    let Ok(accounts) = state.accounts.list() else {
        return;
    };
    if let Some(account) = accounts
        .iter()
        .filter(|a| a.enabled)
        .nth(n.saturating_sub(1))
    {
        activate(app, account.id);
    }
}

pub fn activate<R: Runtime>(app: &AppHandle<R>, account_id: i64) {
    let state = app.state::<AppState>();
    if !state.accounts.get(account_id).is_ok_and(|a| a.enabled) {
        return;
    }
    state.webviews.set_active(app, Some(account_id));
    let _ = app.emit(LAYOUT_CHANGED, state.webviews.state());
}

fn change_zoom<R: Runtime>(app: &AppHandle<R>, account_id: i64, action: &str) {
    let state = app.state::<AppState>();
    let Ok(account) = state.accounts.get(account_id) else {
        return;
    };
    if account.kind != AccountKind::WhatsappWeb {
        return;
    }
    let zoom = match action {
        "zoom_in" => account.zoom + ZOOM_STEP,
        "zoom_out" => account.zoom - ZOOM_STEP,
        _ => 1.0,
    };
    let zoom = (zoom * 100.0).round() / 100.0;
    if let Ok(updated) = state.accounts.update(
        account_id,
        AccountPatch {
            zoom: Some(zoom),
            ..Default::default()
        },
    ) {
        state.webviews.set_zoom(app, account_id, updated.zoom);
        if let Ok(list) = state.accounts.list() {
            let _ = app.emit(super::events::ACCOUNTS_CHANGED, list);
        }
    }
}

/// Push the effective theme to native decorations and every WhatsApp page.
pub fn apply_theme<R: Runtime>(app: &AppHandle<R>) {
    let state = app.state::<AppState>();
    let settings = state.settings.get();
    let theme = super::theme::window_theme(&settings);
    if let Some(w) = window::main_window(app) {
        let _ = w.set_theme(theme);
    }
    if let Some(w) = app.get_webview_window(super::panel::PANEL_WINDOW) {
        let _ = w.set_theme(theme);
    }
    apply_page_theme(app);
}

/// Push the effective theme to every WhatsApp page only.
pub fn apply_page_theme<R: Runtime>(app: &AppHandle<R>) {
    let state = app.state::<AppState>();
    let dark = super::theme::is_dark(app, &state.settings.get());
    state.webviews.eval_all(app, &scripts::apply_theme(dark));
}
