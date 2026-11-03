//! Window close, quit, second-instance messages, signals and shutdown
//! (ZapZap's `window_lifecycle.py`, `single_application.py` and
//! `unix_signal_bridge.py`).

use std::sync::Mutex;

use tauri::{AppHandle, Emitter, Manager, Runtime};

use super::actions::UiAction;
use super::cli::{self, RESTART_MESSAGE, StartupOptions};
use super::events::UI_ACTION;
use super::state::AppState;
use super::{effects, window};

/// Close the main window (`quit == false`) or quit the app. With
/// `confirm_on_close` the UI asks first and answers through
/// [`close_confirmed`]; the question also guards Quit, as in ZapZap.
pub fn request_close<R: Runtime>(app: &AppHandle<R>, quit: bool) {
    let settings = app.state::<AppState>().settings.get();
    if settings.system.confirm_on_close {
        window::show(app);
        let _ = app.emit(
            UI_ACTION,
            UiAction {
                action: if quit {
                    "confirm_quit"
                } else {
                    "confirm_close"
                }
                .to_owned(),
                account_id: None,
            },
        );
        return;
    }
    finish_close(app, quit);
}

pub fn close_confirmed<R: Runtime>(app: &AppHandle<R>, quit: bool, dont_ask_again: bool) {
    if dont_ask_again {
        let _ = effects::change_settings(app, |s| s.system.confirm_on_close = false);
    }
    finish_close(app, quit);
}

fn finish_close<R: Runtime>(app: &AppHandle<R>, quit: bool) {
    let settings = app.state::<AppState>().settings.get();
    if quit || settings.system.quit_on_close {
        app.exit(0);
    } else {
        window::hide(app);
    }
}

/// Whether the window starts hidden: `start_background` or `--hideStart`,
/// unless the first-run setup still has to be shown.
pub fn starts_hidden(options: &StartupOptions, start_background: bool, onboarded: bool) -> bool {
    (start_background || options.hide_start) && onboarded
}

/// Handle the arguments of a second launch or of the first one.
pub fn handle_arguments<R: Runtime>(app: &AppHandle<R>, options: &StartupOptions) {
    if !options.set_settings.is_empty() {
        let _ = effects::change_settings(app, |s| {
            cli::apply_set_settings(options, s);
        });
    }
    for link in &options.links {
        if link == RESTART_MESSAGE {
            app.restart();
        }
        if open_link(app, link) {
            // ZapZap forwards only the first WhatsApp link.
            break;
        }
    }
}

/// Second launches received before setup finished; `None` once ready.
static EARLY_LAUNCHES: Mutex<Option<Vec<Vec<String>>>> = Mutex::new(Some(Vec::new()));

/// Second launch: forward arguments, or just bring the window up.
///
/// The plugin calls this from the D-Bus service (Linux), which starts before
/// setup registers the app state, and from `WM_COPYDATA` (Windows), where
/// creating webviews synchronously deadlocks WebView2. Launches are queued
/// until [`mark_ready`] and then handled on a worker.
pub fn on_second_instance<R: Runtime>(app: &AppHandle<R>, args: Vec<String>) {
    if let Some(queue) = EARLY_LAUNCHES.lock().unwrap().as_mut() {
        queue.push(args);
        return;
    }
    let app = app.clone();
    tauri::async_runtime::spawn(async move { forward_launch(&app, args) });
}

fn forward_launch<R: Runtime>(app: &AppHandle<R>, args: Vec<String>) {
    let options = cli::parse(args.into_iter().skip(1));
    handle_arguments(app, &options);
    window::show(app);
}

/// Setup finished: handle the launches that arrived meanwhile.
pub fn mark_ready<R: Runtime>(app: &AppHandle<R>) {
    let queued = EARLY_LAUNCHES.lock().unwrap().take().unwrap_or_default();
    if queued.is_empty() {
        return;
    }
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        for args in queued {
            forward_launch(&app, args);
        }
    });
}

/// Open a `whatsapp:`/wa.me link in the active WhatsApp Web account, once
/// its page has loaded.
pub fn open_link<R: Runtime>(app: &AppHandle<R>, link: &str) -> bool {
    let Some(script) = crate::webview::deeplink::open_deep_link_script(link) else {
        return false;
    };
    let state = app.state::<AppState>();
    let Some(id) = state.webviews.active() else {
        return false;
    };
    state.webviews.eval_when_ready(app, id, script);
    window::show(app);
    true
}

/// Turn SIGTERM and SIGINT into a normal exit, so shutdown cleanup runs.
#[cfg(unix)]
pub fn install_signal_handler<R: Runtime>(app: &AppHandle<R>) {
    use signal_hook::consts::{SIGINT, SIGTERM};
    let mut signals = match signal_hook::iterator::Signals::new([SIGTERM, SIGINT]) {
        Ok(signals) => signals,
        Err(e) => {
            log::warn!("graceful SIGTERM handling is unavailable: {e}");
            return;
        }
    };
    let app = app.clone();
    std::thread::spawn(move || {
        if signals.forever().next().is_some() {
            app.exit(0);
        }
    });
}

/// Desktop integration that must live as long as the app.
#[derive(Default)]
pub struct Services {
    #[cfg(target_os = "linux")]
    dbus: std::sync::Mutex<Option<zbus::Connection>>,
}

/// Start `org.freedesktop.Application` in Flatpak, where the desktop file is
/// D-Bus activatable.
#[cfg(target_os = "linux")]
pub fn start_dbus(app: &AppHandle) {
    if !crate::platform::is_flatpak() {
        return;
    }
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        match super::dbus::start(app.clone()).await {
            Ok(connection) => {
                *app.state::<Services>().dbus.lock().unwrap() = Some(connection);
            }
            Err(e) => log::warn!("could not own the D-Bus application name: {e}"),
        }
    });
}

/// Cleanup before the process ends.
pub fn shutdown(app: &AppHandle) {
    tauri::async_runtime::block_on(crate::notifications::close_all(app));
    #[cfg(target_os = "linux")]
    drop(app.state::<Services>().dbus.lock().unwrap().take());
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn first_run_setup_wins_over_hidden_start() {
        let hide = cli::parse(["--hideStart"]);
        let none = StartupOptions::default();
        assert!(starts_hidden(&hide, false, true));
        assert!(starts_hidden(&none, true, true));
        assert!(!starts_hidden(&hide, true, false));
        assert!(!starts_hidden(&none, false, true));
    }
}
