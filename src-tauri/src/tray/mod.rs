//! System tray: icon with unread counter, launcher badge and the ZapZap menu.

pub mod icon;

use tauri::image::Image;
use tauri::menu::{Menu, MenuBuilder, MenuEvent};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Manager, Runtime};

use crate::app::{actions, state::AppState, window};
use crate::i18n::tr;

const TRAY_ID: &str = "main";

/// Unread total shown by the tray and the launcher badge: enabled accounts
/// only, and zero when the counter is turned off (as in ZapZap).
pub fn counter_value<R: Runtime>(app: &AppHandle<R>) -> u32 {
    let state = app.state::<AppState>();
    if !state.settings.get().appearance.notification_counter {
        return 0;
    }
    let enabled: Vec<i64> = state
        .accounts
        .list()
        .map(|list| {
            list.into_iter()
                .filter(|a| a.enabled)
                .map(|a| a.id)
                .collect()
        })
        .unwrap_or_default();
    state
        .unread
        .snapshot()
        .per_account
        .iter()
        .filter(|(id, _)| enabled.contains(id))
        .map(|(_, n)| *n)
        .sum()
}

fn build_menu<R: Runtime>(app: &AppHandle<R>) -> tauri::Result<Menu<R>> {
    let settings = app.state::<AppState>().settings.get();
    let panel_enabled = settings.appearance.floating_panel.enabled;
    let mute_label = if settings.system.audio_muted {
        tr("Unmute")
    } else {
        tr("Mute")
    };
    let panel = tauri::menu::MenuItem::with_id(
        app,
        "show_panel",
        tr("Show monitoring panel"),
        panel_enabled,
        None::<&str>,
    )?;
    MenuBuilder::new(app)
        .text("show", tr("Show"))
        .text("accounts", tr("Accounts"))
        .item(&panel)
        .text("toggle_mute", mute_label)
        .text("settings", tr("Settings"))
        .text("donate", tr("Support FastZap"))
        .separator()
        .text("quit", tr("Quit"))
        .build()
}

fn on_menu<R: Runtime>(app: &AppHandle<R>, event: MenuEvent) {
    actions::dispatch(app, event.id().as_ref());
}

fn on_icon<R: Runtime>(app: &AppHandle<R>, event: TrayIconEvent) {
    // Linux trays (StatusNotifierItem) only expose the menu; these events come
    // from Windows and macOS.
    match event {
        TrayIconEvent::Click {
            button: MouseButton::Left,
            button_state: MouseButtonState::Up,
            ..
        }
        | TrayIconEvent::DoubleClick {
            button: MouseButton::Left,
            ..
        } => window::toggle_visibility(app),
        _ => {}
    }
}

fn icon_image(theme: crate::settings::TrayTheme, count: u32) -> Option<Image<'static>> {
    match icon::render(theme, count) {
        Ok(rgba) => Some(Image::new_owned(rgba, icon::SIZE, icon::SIZE)),
        Err(e) => {
            log::warn!("tray icon rendering failed: {e}");
            None
        }
    }
}

pub fn init<R: Runtime>(app: &AppHandle<R>) -> tauri::Result<()> {
    let settings = app.state::<AppState>().settings.get();
    let mut builder = TrayIconBuilder::with_id(TRAY_ID)
        .tooltip("FastZap")
        .menu(&build_menu(app)?)
        .show_menu_on_left_click(false)
        .on_menu_event(on_menu)
        .on_tray_icon_event(|tray, event| on_icon(tray.app_handle(), event));
    if let Some(image) = icon_image(settings.appearance.tray_theme, counter_value(app)) {
        builder = builder.icon(image);
    }
    let tray = builder.build(app)?;
    tray.set_visible(settings.appearance.tray_icon)?;
    update_counter(app);
    Ok(())
}

/// Re-render the icon and the launcher badge after an unread or setting change.
pub fn update_counter<R: Runtime>(app: &AppHandle<R>) {
    let count = counter_value(app);
    let theme = app.state::<AppState>().settings.get().appearance.tray_theme;
    if let Some(tray) = app.tray_by_id(TRAY_ID) {
        let _ = tray.set_icon(icon_image(theme, count));
    }
    if let Some(w) = window::main_window(app) {
        let _ = w.set_badge_count((count > 0).then_some(count as i64));
    }
}

/// Rebuild menu labels and visibility (language, mute or panel changes).
pub fn refresh<R: Runtime>(app: &AppHandle<R>) {
    let Some(tray) = app.tray_by_id(TRAY_ID) else {
        return;
    };
    let settings = app.state::<AppState>().settings.get();
    if let Ok(menu) = build_menu(app) {
        let _ = tray.set_menu(Some(menu));
    }
    let _ = tray.set_visible(settings.appearance.tray_icon);
    update_counter(app);
}
