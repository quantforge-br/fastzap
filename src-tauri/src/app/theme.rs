//! Resolve the effective color scheme. In `auto`, the XDG Settings portal's
//! `org.freedesktop.appearance color-scheme` wins when it states a preference
//! (ZapZap's `SystemThemeMonitor`); otherwise the window theme reported by the
//! toolkit is used.

use std::sync::atomic::{AtomicU8, Ordering};

use tauri::{AppHandle, Runtime, Theme};

use super::window::main_window;
use crate::settings::{Settings, ThemeMode};

const NO_PREFERENCE: u8 = 0;
const PREFER_DARK: u8 = 1;
const PREFER_LIGHT: u8 = 2;

/// Last value read from the portal, in its own encoding.
static SYSTEM_SCHEME: AtomicU8 = AtomicU8::new(NO_PREFERENCE);

fn system_theme() -> Option<Theme> {
    match SYSTEM_SCHEME.load(Ordering::Relaxed) {
        PREFER_DARK => Some(Theme::Dark),
        PREFER_LIGHT => Some(Theme::Light),
        _ => None,
    }
}

pub fn is_dark<R: Runtime>(app: &AppHandle<R>, settings: &Settings) -> bool {
    match window_theme(settings) {
        Some(theme) => theme == Theme::Dark,
        None => main_window(app)
            .and_then(|w| w.theme().ok())
            .is_some_and(|t| t == Theme::Dark),
    }
}

/// Window theme override for native decorations and the UI webview's
/// `prefers-color-scheme` (`None` follows the toolkit).
pub fn window_theme(settings: &Settings) -> Option<Theme> {
    match settings.appearance.theme {
        ThemeMode::Light => Some(Theme::Light),
        ThemeMode::Dark => Some(Theme::Dark),
        ThemeMode::Auto => system_theme(),
    }
}

/// Follow the portal's color scheme and reapply the theme when it changes.
#[cfg(target_os = "linux")]
pub fn monitor_system<R: Runtime>(app: &AppHandle<R>) {
    use ashpd::desktop::settings::{ColorScheme, Settings as Portal};
    use futures_util::StreamExt;

    fn encode(scheme: ColorScheme) -> u8 {
        match scheme {
            ColorScheme::PreferDark => PREFER_DARK,
            ColorScheme::PreferLight => PREFER_LIGHT,
            ColorScheme::NoPreference => NO_PREFERENCE,
        }
    }

    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        let run = async {
            let portal = Portal::new().await?;
            let mut changes = portal.receive_color_scheme_changed().await?;
            store(&app, encode(portal.color_scheme().await?));
            while let Some(scheme) = changes.next().await {
                store(&app, encode(scheme));
            }
            anyhow::Ok(())
        };
        if let Err(e) = run.await {
            log::info!("color-scheme portal unavailable: {e}");
        }
    });
}

#[cfg(target_os = "linux")]
fn store<R: Runtime>(app: &AppHandle<R>, scheme: u8) {
    if SYSTEM_SCHEME.swap(scheme, Ordering::Relaxed) != scheme {
        super::actions::apply_theme(app);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn auto_follows_the_portal_preference_and_explicit_modes_win() {
        let mut settings = Settings::default();
        settings.appearance.theme = ThemeMode::Auto;
        SYSTEM_SCHEME.store(PREFER_DARK, Ordering::Relaxed);
        assert_eq!(window_theme(&settings), Some(Theme::Dark));
        settings.appearance.theme = ThemeMode::Light;
        assert_eq!(window_theme(&settings), Some(Theme::Light));
        settings.appearance.theme = ThemeMode::Auto;
        SYSTEM_SCHEME.store(NO_PREFERENCE, Ordering::Relaxed);
        assert_eq!(window_theme(&settings), None);
    }
}
