//! Resolve the effective color scheme.

use tauri::{AppHandle, Runtime, Theme};

use super::window::main_window;
use crate::settings::{Settings, ThemeMode};

pub fn is_dark<R: Runtime>(app: &AppHandle<R>, settings: &Settings) -> bool {
    match settings.appearance.theme {
        ThemeMode::Light => false,
        ThemeMode::Dark => true,
        ThemeMode::Auto => main_window(app)
            .and_then(|w| w.theme().ok())
            .is_some_and(|t| t == Theme::Dark),
    }
}

/// Window theme override for native decorations (`None` follows the system).
pub fn window_theme(settings: &Settings) -> Option<Theme> {
    match settings.appearance.theme {
        ThemeMode::Light => Some(Theme::Light),
        ThemeMode::Dark => Some(Theme::Dark),
        ThemeMode::Auto => None,
    }
}
