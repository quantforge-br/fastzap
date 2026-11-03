//! Main window: a local UI webview plus one child webview per WhatsApp Web account.

use tauri::webview::WebviewBuilder;
use tauri::window::WindowBuilder;
use tauri::{AppHandle, LogicalPosition, LogicalSize, Manager, Runtime, WebviewUrl, Window};

use super::state::AppState;

pub const MAIN_WINDOW: &str = "main";
pub const UI_WEBVIEW: &str = "ui";
const DEFAULT_SIZE: (f64, f64) = (1000.0, 700.0);

pub fn create_main<R: Runtime>(app: &AppHandle<R>) -> tauri::Result<Window<R>> {
    let settings = app.state::<AppState>().settings.get();
    let window = WindowBuilder::new(app, MAIN_WINDOW)
        .title("FastZap")
        .inner_size(DEFAULT_SIZE.0, DEFAULT_SIZE.1)
        .min_inner_size(480.0, 400.0)
        .decorations(!settings.appearance.csr)
        .theme(super::theme::window_theme(&settings))
        .visible(false)
        .build()?;
    let close_app = app.clone();
    window.on_window_event(move |event| {
        match event {
            tauri::WindowEvent::CloseRequested { api, .. } => {
                api.prevent_close();
                super::lifecycle::request_close(&close_app, false);
            }
            // The toolkit theme changed while `auto` follows it.
            tauri::WindowEvent::ThemeChanged(_) => super::actions::apply_page_theme(&close_app),
            _ => {}
        }
    });
    #[cfg(target_os = "linux")]
    crate::webview::stage::install(&window)?;
    let scale = window.scale_factor().unwrap_or(1.0);
    let size = window
        .inner_size()
        .map(|s| s.to_logical::<f64>(scale))
        .unwrap_or(LogicalSize::new(DEFAULT_SIZE.0, DEFAULT_SIZE.1));
    let ui = window.add_child(
        WebviewBuilder::new(UI_WEBVIEW, WebviewUrl::App("index.html".into())).auto_resize(),
        LogicalPosition::new(0.0, 0.0),
        size,
    )?;
    #[cfg(target_os = "linux")]
    crate::webview::stage::attach_main(&ui);
    #[cfg(not(target_os = "linux"))]
    let _ = ui;
    Ok(window)
}

pub fn main_window<R: Runtime>(app: &AppHandle<R>) -> Option<Window<R>> {
    app.get_window(MAIN_WINDOW)
}

pub fn show<R: Runtime>(app: &AppHandle<R>) {
    if let Some(window) = main_window(app) {
        let _ = window.unminimize();
        let _ = window.show();
        let _ = window.set_focus();
    }
}

/// Show and focus the window using an XDG activation / startup token so
/// Wayland and X11 compositors allow the focus change.
pub fn show_with_token<R: Runtime>(app: &AppHandle<R>, token: Option<String>) {
    let handle = app.clone();
    let _ = app.run_on_main_thread(move || {
        #[cfg(target_os = "linux")]
        if let (Some(token), Some(window)) = (token.as_deref(), main_window(&handle))
            && let Ok(gtk_window) = window.gtk_window()
        {
            use gtk::prelude::GtkWindowExt;
            gtk_window.set_startup_id(token);
        }
        #[cfg(not(target_os = "linux"))]
        let _ = token;
        show(&handle);
    });
}

pub fn hide<R: Runtime>(app: &AppHandle<R>) {
    if let Some(window) = main_window(app) {
        let _ = window.hide();
    }
}

pub fn toggle_visibility<R: Runtime>(app: &AppHandle<R>) {
    if let Some(window) = main_window(app) {
        if window.is_visible().unwrap_or(false) && window.is_focused().unwrap_or(false) {
            let _ = window.hide();
        } else {
            show(app);
        }
    }
}
