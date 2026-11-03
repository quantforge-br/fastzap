//! Commands callable from https://web.whatsapp.com (see capabilities/whatsapp-remote.json).
//! The calling account is always derived from the webview label, never from arguments.

use tauri::menu::{Menu, MenuEvent, MenuItem};
use tauri::{AppHandle, Emitter, LogicalPosition, Manager, Webview};

use super::CommandResult;
use crate::accounts::AccountPatch;
use crate::app::events::ACCOUNTS_CHANGED;
use crate::app::{actions, effects, state::AppState, theme};
use crate::i18n::tr;
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

/// Shortcuts a page may trigger. Clipboard access is excluded: on Linux the
/// paste shortcut is handled natively before the page sees the key, and
/// elsewhere it is only honored while the window has focus.
const REMOTE_ACTIONS: &[&str] = &[
    "reload",
    "fullscreen",
    "theme_auto",
    "theme_light",
    "theme_dark",
    "quit",
    "hide",
    "settings",
    "zoom_reset",
    "zoom_in",
    "zoom_out",
    "toggle_grid",
    "downloads",
    "add_account",
    "open_chat_by_number",
    "new_chat",
    "show_shortcuts",
    "devtools",
];

fn remote_action_allowed(action: &str) -> bool {
    REMOTE_ACTIONS.contains(&action)
        || action
            .strip_prefix("account_")
            .is_some_and(|n| matches!(n.parse::<u8>(), Ok(1..=9)))
}

#[tauri::command]
pub fn wa_shortcut(app: AppHandle, webview: Webview, action: String) -> CommandResult<()> {
    let id = caller(&webview)?;
    if action == "paste_plain_text" {
        let focused = !cfg!(target_os = "linux") && webview.window().is_focused().unwrap_or(false);
        if !focused {
            return Err("paste is not allowed from the page".into());
        }
    } else if !remote_action_allowed(&action) {
        return Err("action is not allowed from the page".into());
    }
    actions::dispatch_for(&app, &action, Some(id));
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

#[cfg(test)]
mod tests {
    use super::remote_action_allowed;

    #[test]
    fn only_listed_actions_are_remote() {
        assert!(remote_action_allowed("reload"));
        assert!(remote_action_allowed("account_9"));
        assert!(!remote_action_allowed("account_0"));
        assert!(!remote_action_allowed("account_10"));
        assert!(!remote_action_allowed("paste_plain_text"));
        assert!(!remote_action_allowed("toggle_mute"));
        assert!(!remote_action_allowed("donate"));
    }
}

pub const SELECT_ACCOUNT_MENU_PREFIX: &str = "select-account:";

/// Integrated account selector: a native popup listing the other enabled
/// accounts below the page's button, like ZapZap's selector popover.
#[tauri::command]
pub fn wa_open_account_selector(
    app: AppHandle,
    webview: Webview,
    rect: [f64; 4],
) -> CommandResult<()> {
    let id = caller(&webview)?;
    let state = app.state::<AppState>();
    if !state
        .settings
        .get()
        .appearance
        .integrated_account_selector_enabled
    {
        return Ok(());
    }
    let Some(origin) = state.webviews.placed_rect(id) else {
        return Ok(());
    };
    let zoom = state.accounts.get(id).map(|a| a.zoom).unwrap_or(1.0);
    let unread = state.unread.snapshot();
    let menu = Menu::new(&app)?;
    let mut others = 0;
    for account in state.accounts.list()? {
        if account.id == id || !account.enabled {
            continue;
        }
        let label = match unread.per_account.get(&account.id) {
            Some(count) if *count > 0 => format!("{} ({count})", account.name),
            _ => account.name.clone(),
        };
        menu.append(&MenuItem::with_id(
            &app,
            format!("{SELECT_ACCOUNT_MENU_PREFIX}{}", account.id),
            label,
            true,
            None::<&str>,
        )?)?;
        others += 1;
    }
    if others == 0 {
        menu.append(&MenuItem::new(
            &app,
            tr("No active accounts to display."),
            false,
            None::<&str>,
        )?)?;
    }
    let [x, y, _w, h] = rect;
    webview.window().popup_menu_at(
        &menu,
        LogicalPosition::new(origin.x + x * zoom, origin.y + (y + h) * zoom),
    )?;
    Ok(())
}

/// Activate the account picked in the integrated selector.
pub fn on_menu_event<R: tauri::Runtime>(app: &AppHandle<R>, event: MenuEvent) {
    if let Some(id) = event
        .id()
        .as_ref()
        .strip_prefix(SELECT_ACCOUNT_MENU_PREFIX)
        .and_then(|id| id.parse().ok())
    {
        actions::activate(app, id);
    }
}

#[tauri::command]
pub async fn wa_notify(
    app: AppHandle,
    webview: Webview,
    notification: crate::notifications::policy::WebNotification,
) -> CommandResult<()> {
    let id = caller(&webview)?;
    crate::notifications::notify(&app, id, notification).await;
    Ok(())
}

#[tauri::command]
pub async fn wa_notification_close(app: AppHandle, webview: Webview, id: u64) -> CommandResult<()> {
    let account_id = caller(&webview)?;
    crate::notifications::close_web(&app, account_id, id).await;
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
