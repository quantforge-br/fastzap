//! Application actions triggered by shortcuts, the tray or the UI.

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, Runtime};
use ts_rs::TS;

use super::events::{LAYOUT_CHANGED, SETTINGS_CHANGED, UI_ACTION};
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
    "donate",
    "report_problem",
];

pub fn dispatch<R: Runtime>(app: &AppHandle<R>, action: &str) {
    let state = app.state::<AppState>();
    let active = state.webviews.active();
    match action {
        "quit" => app.exit(0),
        "hide" => window::hide(app),
        "show" => window::show(app),
        "toggle_window" => window::toggle_visibility(app),
        "fullscreen" => {
            if let Some(w) = window::main_window(app) {
                let _ = w.set_fullscreen(!w.is_fullscreen().unwrap_or(false));
            }
        }
        "toggle_grid" => {
            state.webviews.toggle_grid(app);
            let _ = app.emit(LAYOUT_CHANGED, state.webviews.state());
        }
        "reload" => {
            if let Some(id) = active {
                state.webviews.reload(app, id);
            }
        }
        "zoom_in" | "zoom_out" | "zoom_reset" => {
            if let Some(id) = active {
                change_zoom(app, id, action);
            }
        }
        "theme_auto" | "theme_light" | "theme_dark" => {
            let theme = match action {
                "theme_light" => ThemeMode::Light,
                "theme_dark" => ThemeMode::Dark,
                _ => ThemeMode::Auto,
            };
            if let Ok(settings) = state.settings.update(|s| s.appearance.theme = theme) {
                let _ = app.emit(SETTINGS_CHANGED, &settings);
                apply_theme(app);
            }
        }
        "new_chat" => eval_active(app, "window.__FASTZAP__ && window.__FASTZAP__.newChat();"),
        "close_conversation" => eval_active(
            app,
            "window.__FASTZAP__ && window.__FASTZAP__.closeConversation();",
        ),
        "lock" => eval_active(app, "window.__FASTZAP__ && window.__FASTZAP__.lockApp();"),
        "paste_plain_text" => {
            if let Some(text) = clipboard::read_plain_text() {
                eval_active(app, &scripts::paste_plain_text(&text));
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
                    account_id: active,
                },
            );
        }
        other => log::debug!("unknown action {other}"),
    }
}

fn eval_active<R: Runtime>(app: &AppHandle<R>, script: &str) {
    let state = app.state::<AppState>();
    if let Some(id) = state.webviews.active() {
        state.webviews.eval(app, id, script);
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
    if let Some(w) = window::main_window(app) {
        let _ = w.set_theme(super::theme::window_theme(&settings));
    }
    let dark = super::theme::is_dark(app, &settings);
    state.webviews.eval_all(app, &scripts::apply_theme(dark));
}
