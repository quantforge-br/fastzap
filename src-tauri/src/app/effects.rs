//! Side effects of settings, account and unread changes.

use tauri::{AppHandle, Emitter, Manager, Runtime};

use super::events::{LAYOUT_CHANGED, UNREAD_CHANGED};
use super::state::AppState;
use crate::settings::Settings;
use crate::webview::manager::page_labels;
use crate::webview::scripts;
use crate::webview::unread::UnreadSnapshot;

pub fn on_settings_changed<R: Runtime>(app: &AppHandle<R>, old: &Settings, new: &Settings) {
    let state = app.state::<AppState>();
    let webviews = &state.webviews;

    if old.system.interface_language != new.system.interface_language {
        crate::i18n::set_language(&new.system.interface_language);
    }
    if old.appearance.theme != new.appearance.theme {
        super::actions::apply_theme(app);
    }
    if old.downloads.pdf_click_behavior != new.downloads.pdf_click_behavior {
        webviews.eval_all(
            app,
            &scripts::set_pdf_click_mode(new.downloads.pdf_click_behavior),
        );
    }
    if old.appearance.resizable_chat_list_enabled != new.appearance.resizable_chat_list_enabled {
        let script = if new.appearance.resizable_chat_list_enabled {
            scripts::resizable_chat_list(&page_labels())
        } else {
            scripts::disable_resizable_chat_list().to_owned()
        };
        webviews.eval_all(app, &script);
    }
    if old.appearance.integrated_account_selector_enabled
        != new.appearance.integrated_account_selector_enabled
    {
        refresh_account_selectors(app);
    }
    if old.appearance.grid_cols != new.appearance.grid_cols {
        webviews.apply_layout(app);
    }
    if old.appearance.csr != new.appearance.csr
        && let Some(w) = super::window::main_window(app)
    {
        let _ = w.set_decorations(!new.appearance.csr);
    }
    if old.customizations != new.customizations {
        crate::customizations::reapply_all(app);
    }
}

pub fn on_accounts_changed<R: Runtime>(app: &AppHandle<R>) {
    let state = app.state::<AppState>();
    if let Err(e) = state.webviews.sync(app) {
        log::warn!("failed to sync account webviews: {e}");
    }
    let _ = app.emit(LAYOUT_CHANGED, state.webviews.state());
    refresh_account_selectors(app);
}

pub fn on_unread_changed<R: Runtime>(app: &AppHandle<R>, snapshot: &UnreadSnapshot) {
    let _ = app.emit(UNREAD_CHANGED, snapshot);
    refresh_account_selectors(app);
}

/// Update the integrated selector button of every page: it shows an activity
/// dot when another account has unread messages.
pub fn refresh_account_selectors<R: Runtime>(app: &AppHandle<R>) {
    let state = app.state::<AppState>();
    let enabled = state
        .settings
        .get()
        .appearance
        .integrated_account_selector_enabled;
    let snapshot = state.unread.snapshot();
    for (label, webview) in app.webviews() {
        if let Some(id) = crate::webview::manager::account_id_from_label(&label) {
            let other_unread = snapshot
                .per_account
                .iter()
                .any(|(other, count)| *other != id && *count > 0);
            let _ = webview.eval(scripts::set_account_selector_state(enabled, other_unread));
        }
    }
}
