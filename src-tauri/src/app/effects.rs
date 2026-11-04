//! Side effects of settings, account and unread changes.

use tauri::{AppHandle, Emitter, Manager, Runtime};

use super::events::{LAYOUT_CHANGED, SETTINGS_CHANGED, UNREAD_CHANGED};
use super::state::AppState;
use crate::settings::Settings;
use crate::webview::manager::page_labels;
use crate::webview::scripts;
use crate::webview::unread::UnreadSnapshot;

/// Persist a settings change, notify the UI and apply its side effects.
pub fn change_settings<R: Runtime>(
    app: &AppHandle<R>,
    f: impl FnOnce(&mut Settings),
) -> Result<Settings, crate::settings::store::StoreError> {
    let state = app.state::<AppState>();
    let crate::settings::store::Changed { old, new } = state.settings.update(f)?;
    let _ = app.emit(SETTINGS_CHANGED, &new);
    on_settings_changed(app, &old, &new);
    Ok(new)
}

pub fn on_settings_changed<R: Runtime>(app: &AppHandle<R>, old: &Settings, new: &Settings) {
    let state = app.state::<AppState>();
    let webviews = &state.webviews;

    if old.downloads.remember_last_save_as_directory
        && !new.downloads.remember_last_save_as_directory
    {
        state.downloads.forget_last_dirs();
    }
    if old.system.interface_language != new.system.interface_language {
        crate::i18n::set_language(&new.system.interface_language);
    }
    if crate::webview::manager::creation_config(old)
        != crate::webview::manager::creation_config(new)
    {
        // WhatsApp sessions live in the profile directory and survive this.
        if let Err(e) = webviews.recreate_all(app) {
            log::warn!("failed to recreate account webviews: {e}");
        }
    }
    if old.reporting.crash_prompts != new.reporting.crash_prompts {
        crate::reporting::session::CRASH_PROMPTS.store(
            new.reporting.crash_prompts,
            std::sync::atomic::Ordering::Relaxed,
        );
    }
    if old.spellcheck != new.spellcheck {
        crate::spellcheck::apply_all(app);
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
    if old.appearance.scale != new.appearance.scale {
        super::actions::apply_scale(app);
    }
    if old.appearance.grid_cols != new.appearance.grid_cols {
        webviews.apply_layout(app);
    }
    let decorations = super::window::native_decorations(&new.appearance);
    if super::window::native_decorations(&old.appearance) != decorations
        && let Some(w) = super::window::main_window(app)
    {
        let _ = w.set_decorations(decorations);
    }
    if old.customizations != new.customizations {
        crate::customizations::reapply_all(app);
    }
    if old.system.audio_muted != new.system.audio_muted {
        webviews.set_muted_all(app, new.system.audio_muted);
    }
    if old.system.start_system != new.system.start_system {
        crate::autostart::sync(app, new.system.start_system);
    }
    super::panel::on_settings_changed(app, old, new);
    let (oa, na) = (&old.appearance, &new.appearance);
    if oa.tray_icon != na.tray_icon
        || oa.tray_theme != na.tray_theme
        || oa.notification_counter != na.notification_counter
        || oa.floating_panel.enabled != na.floating_panel.enabled
        || old.system.audio_muted != new.system.audio_muted
        || old.system.interface_language != new.system.interface_language
    {
        crate::tray::refresh(app);
    }
}

pub fn on_accounts_changed<R: Runtime>(app: &AppHandle<R>) {
    let state = app.state::<AppState>();
    if let Err(e) = state.webviews.sync(app) {
        log::warn!("failed to sync account webviews: {e}");
    }
    // Disabled accounts stop reporting, so their last count must not linger.
    if let Ok(accounts) = state.accounts.list() {
        let enabled: Vec<i64> = accounts
            .iter()
            .filter(|a| a.enabled)
            .map(|a| a.id)
            .collect();
        if let Some(snapshot) = state.unread.retain(|id| enabled.contains(&id)) {
            let _ = app.emit(UNREAD_CHANGED, &snapshot);
        }
    }
    let _ = app.emit(LAYOUT_CHANGED, state.webviews.state());
    refresh_account_selectors(app);
    crate::tray::update_counter(app);
}

pub fn on_unread_changed<R: Runtime>(app: &AppHandle<R>, snapshot: &UnreadSnapshot) {
    let _ = app.emit(UNREAD_CHANGED, snapshot);
    refresh_account_selectors(app);
    crate::tray::update_counter(app);
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
