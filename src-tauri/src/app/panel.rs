//! Quick access panel (ZapZap's `FloatingMonitoringPanel` and its controller):
//! an independent window listing accounts with unread counts and a few quick
//! actions. Its visibility follows `appearance.floating_panel.mode`.

use std::sync::Mutex;
use std::time::Duration;

use tauri::{
    AppHandle, LogicalPosition, Manager, PhysicalPosition, Runtime, WebviewUrl, WebviewWindow,
    WebviewWindowBuilder, WindowEvent,
};

use super::state::AppState;
use super::window::main_window;
use crate::i18n::tr;
use crate::settings::{FloatingPanelMode, FloatingPanelSettings, Settings};

pub const PANEL_WINDOW: &str = "panel";
const WIDTH: f64 = 320.0;
const MIN_HEIGHT: f64 = 220.0;
const HEIGHT: f64 = 420.0;
/// Offset of the default position from the work area's top-right corner.
const DEFAULT_OFFSET: (f64, f64) = (15.0, 16.0);
/// Quiet period before a moved position is saved.
const SAVE_DELAY: Duration = Duration::from_millis(500);

#[derive(Default)]
struct Flags {
    /// Closed by the user; stays hidden until the mode or host state changes.
    dismissed: bool,
    /// Opened explicitly (tray, `show_panel`), also in on-demand mode.
    manual: bool,
    /// Last seen main window state (hidden or minimized).
    host_unavailable: Option<bool>,
    /// Window requests are queued (tao), so `is_visible` lags behind `show`
    /// and `hide`; both windows' visibility is tracked here instead.
    host_visible: bool,
    shown: bool,
    stopped: bool,
    /// Wayland does not let clients place windows.
    positioning: bool,
    locked_at: Option<PhysicalPosition<i32>>,
    /// Ignore `Moved` events caused by our own placement.
    placing: bool,
    save_generation: u64,
}

#[derive(Default)]
pub struct PanelController(Mutex<Flags>);

/// Whether the panel should be visible (ZapZap's `_sync_floating_visibility`).
pub fn should_show(
    enabled: bool,
    mode: FloatingPanelMode,
    dismissed: bool,
    manual: bool,
    host_unavailable: bool,
) -> bool {
    enabled
        && !dismissed
        && (manual
            || mode == FloatingPanelMode::Always
            || (mode == FloatingPanelMode::WhenHidden && host_unavailable))
}

/// Top-left position inside `area` (x, y, width, height), keeping the frame
/// visible; without a saved position, the top-right corner minus the offset.
pub fn placement(
    area: (f64, f64, f64, f64),
    frame: (f64, f64),
    saved: Option<(f64, f64)>,
) -> (f64, f64) {
    let (ax, ay, aw, ah) = area;
    let (x, y) = saved.unwrap_or((ax + aw - frame.0 - DEFAULT_OFFSET.0, ay + DEFAULT_OFFSET.1));
    (
        x.min(ax + aw - frame.0).max(ax),
        y.min(ay + ah - frame.1).max(ay),
    )
}

fn controller<R: Runtime>(app: &AppHandle<R>) -> &PanelController {
    app.state::<PanelController>().inner()
}

fn panel<R: Runtime>(app: &AppHandle<R>) -> Option<WebviewWindow<R>> {
    app.get_webview_window(PANEL_WINDOW)
}

fn panel_settings<R: Runtime>(app: &AppHandle<R>) -> FloatingPanelSettings {
    app.state::<AppState>()
        .settings
        .get()
        .appearance
        .floating_panel
}

#[cfg(target_os = "linux")]
fn supports_positioning() -> bool {
    use gtk::glib::prelude::ObjectExt;
    gtk::gdk::Display::default().is_none_or(|d| !d.type_().name().contains("Wayland"))
}

#[cfg(not(target_os = "linux"))]
fn supports_positioning() -> bool {
    true
}

/// Create the hidden panel window; call from the main thread during setup.
pub fn init<R: Runtime>(app: &AppHandle<R>) -> tauri::Result<()> {
    app.manage(PanelController::default());
    controller(app).0.lock().unwrap().positioning = supports_positioning();
    let settings = panel_settings(app);
    let window = WebviewWindowBuilder::new(app, PANEL_WINDOW, WebviewUrl::App("panel".into()))
        .title(format!("FastZap — {}", tr("Quick access")))
        .inner_size(WIDTH, HEIGHT)
        .min_inner_size(WIDTH, MIN_HEIGHT)
        .max_inner_size(WIDTH, 10_000.0)
        .skip_taskbar(true)
        .minimizable(false)
        .maximizable(false)
        .always_on_top(settings.on_top)
        .theme(super::theme::window_theme(
            &app.state::<AppState>().settings.get(),
        ))
        .focused(false)
        .visible(false)
        .build()?;
    let handle = app.clone();
    window.on_window_event(move |event| match event {
        WindowEvent::CloseRequested { api, .. } => {
            api.prevent_close();
            dismiss(&handle);
        }
        WindowEvent::Moved(position) => on_moved(&handle, *position),
        _ => {}
    });
    let saved = settings
        .remember_position
        .then_some(settings.position)
        .flatten();
    place(app, &window, saved.map(|(x, y)| (x as f64, y as f64)));
    apply_options(app);
    Ok(())
}

fn place<R: Runtime>(app: &AppHandle<R>, window: &WebviewWindow<R>, saved: Option<(f64, f64)>) {
    if !controller(app).0.lock().unwrap().positioning {
        return;
    }
    let monitor = saved
        .and_then(|(x, y)| window.monitor_from_point(x, y).ok().flatten())
        .or_else(|| window.current_monitor().ok().flatten())
        .or_else(|| window.primary_monitor().ok().flatten());
    let Some(monitor) = monitor else { return };
    let scale = monitor.scale_factor();
    let area = monitor.work_area();
    let origin = area.position.to_logical::<f64>(scale);
    let extent = area.size.to_logical::<f64>(scale);
    let frame = window
        .outer_size()
        .map(|s| s.to_logical::<f64>(scale))
        .map(|s| (s.width, s.height))
        .unwrap_or((WIDTH, HEIGHT));
    let (x, y) = placement(
        (origin.x, origin.y, extent.width, extent.height),
        frame,
        saved,
    );
    controller(app).0.lock().unwrap().placing = true;
    let _ = window.set_position(LogicalPosition::new(x, y));
    let position = window.outer_position().ok();
    let mut flags = controller(app).0.lock().unwrap();
    flags.placing = false;
    if flags.locked_at.is_some() {
        flags.locked_at = position;
    }
}

/// Apply on-top and position lock without changing visibility.
pub fn apply_options<R: Runtime>(app: &AppHandle<R>) {
    let Some(window) = panel(app) else { return };
    let settings = panel_settings(app);
    let _ = window.set_always_on_top(settings.on_top);
    // Never call into the window while holding the lock: off the main
    // thread it waits for the main thread, which may be in `on_moved`.
    let position = window.outer_position().ok();
    let mut flags = controller(app).0.lock().unwrap();
    let locked = settings.lock_position && flags.positioning;
    flags.locked_at = if locked { position } else { None };
}

fn on_moved<R: Runtime>(app: &AppHandle<R>, position: PhysicalPosition<i32>) {
    let Some(window) = panel(app) else { return };
    let mut flags = controller(app).0.lock().unwrap();
    if flags.placing || !flags.positioning || flags.stopped {
        return;
    }
    if let Some(locked) = flags.locked_at {
        if locked != position {
            flags.placing = true;
            drop(flags);
            let _ = window.set_position(locked);
            controller(app).0.lock().unwrap().placing = false;
        }
        return;
    }
    if !flags.shown {
        return;
    }
    flags.save_generation += 1;
    let generation = flags.save_generation;
    drop(flags);
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(SAVE_DELAY).await;
        if controller(&app).0.lock().unwrap().save_generation == generation {
            remember_position(&app);
        }
    });
}

fn remember_position<R: Runtime>(app: &AppHandle<R>) {
    let Some(window) = panel(app) else { return };
    if !panel_settings(app).remember_position {
        return;
    }
    let (Ok(position), Ok(scale)) = (window.outer_position(), window.scale_factor()) else {
        return;
    };
    let logical = position.to_logical::<f64>(scale);
    let position = Some((logical.x.round() as i32, logical.y.round() as i32));
    if panel_settings(app).position != position {
        let _ = super::effects::change_settings(app, |s| {
            s.appearance.floating_panel.position = position;
        });
    }
}

fn show_panel<R: Runtime>(app: &AppHandle<R>, window: &WebviewWindow<R>) {
    let saved = window
        .outer_position()
        .ok()
        .zip(window.scale_factor().ok())
        .map(|(p, s)| p.to_logical::<f64>(s))
        .map(|p| (p.x, p.y));
    controller(app).0.lock().unwrap().shown = true;
    let _ = window.show();
    // Recover a position left on a disconnected monitor.
    place(app, window, saved);
}

/// Recompute visibility after a preference or window state change.
pub fn sync<R: Runtime>(app: &AppHandle<R>) {
    let minimized = main_window(app).is_some_and(|w| w.is_minimized().unwrap_or(false));
    let Some(window) = panel(app) else { return };
    let settings = panel_settings(app);
    let (visible, shown) = {
        let mut flags = controller(app).0.lock().unwrap();
        if flags.stopped {
            return;
        }
        let unavailable = !flags.host_visible || minimized;
        if flags.host_unavailable != Some(unavailable) {
            flags.host_unavailable = Some(unavailable);
            flags.dismissed = false;
            flags.manual = false;
        }
        let visible = should_show(
            settings.enabled,
            settings.mode,
            flags.dismissed,
            flags.manual,
            unavailable,
        );
        (visible, flags.shown)
    };
    match (visible, shown) {
        (true, false) => show_panel(app, &window),
        (false, true) => hide_panel(app, &window),
        _ => {}
    }
}

/// The main window was just shown or hidden.
pub fn host_changed<R: Runtime>(app: &AppHandle<R>, visible: bool) {
    if let Some(controller) = app.try_state::<PanelController>() {
        controller.0.lock().unwrap().host_visible = visible;
        sync(app);
    }
}

fn hide_panel<R: Runtime>(app: &AppHandle<R>, window: &WebviewWindow<R>) {
    controller(app).0.lock().unwrap().shown = false;
    let _ = window.hide();
}

/// Explicitly open the enabled panel, including in on-demand mode.
pub fn show<R: Runtime>(app: &AppHandle<R>) {
    let Some(window) = panel(app) else { return };
    let shown = {
        let mut flags = controller(app).0.lock().unwrap();
        if !panel_settings(app).enabled || flags.stopped {
            return;
        }
        flags.dismissed = false;
        flags.manual = true;
        flags.shown
    };
    if !shown {
        show_panel(app, &window);
    }
    let _ = window.set_focus();
}

/// The user closed the panel (close button or Escape).
pub fn dismiss<R: Runtime>(app: &AppHandle<R>) {
    {
        let mut flags = controller(app).0.lock().unwrap();
        flags.dismissed = true;
        flags.manual = false;
    }
    if let Some(window) = panel(app) {
        hide_panel(app, &window);
    }
}

pub fn on_settings_changed<R: Runtime>(app: &AppHandle<R>, old: &Settings, new: &Settings) {
    let (old, new) = (
        &old.appearance.floating_panel,
        &new.appearance.floating_panel,
    );
    if old.on_top != new.on_top || old.lock_position != new.lock_position {
        apply_options(app);
    }
    if old.enabled != new.enabled || old.mode != new.mode {
        let mut flags = controller(app).0.lock().unwrap();
        flags.dismissed = false;
        flags.manual = false;
    }
    if old.enabled != new.enabled || old.mode != new.mode {
        sync(app);
    }
    // "Reset position" clears the saved position.
    if old.position.is_some()
        && new.position.is_none()
        && let Some(window) = panel(app)
    {
        place(app, &window, None);
        show(app);
    }
}

/// Keep the last position and stop reacting before the app exits.
pub fn shutdown<R: Runtime>(app: &AppHandle<R>) {
    let Some(window) = panel(app) else { return };
    let (positioning, shown) = {
        let mut flags = controller(app).0.lock().unwrap();
        flags.stopped = true;
        (flags.positioning, flags.shown)
    };
    if positioning && shown {
        remember_position(app);
    }
    let _ = window.hide();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn visibility_follows_the_mode() {
        use FloatingPanelMode::*;
        assert!(should_show(true, Always, false, false, false));
        assert!(!should_show(false, Always, false, true, true));
        assert!(!should_show(true, Always, true, false, false));
        assert!(!should_show(true, WhenHidden, false, false, false));
        assert!(should_show(true, WhenHidden, false, false, true));
        assert!(!should_show(true, OnDemand, false, false, true));
        assert!(should_show(true, OnDemand, false, true, false));
    }

    #[test]
    fn placement_defaults_to_the_top_right_and_clamps() {
        let area = (0.0, 30.0, 1920.0, 1050.0);
        assert_eq!(placement(area, (320.0, 420.0), None), (1585.0, 46.0));
        assert_eq!(
            placement(area, (320.0, 420.0), Some((5000.0, -40.0))),
            (1600.0, 30.0)
        );
        assert_eq!(
            placement(area, (320.0, 420.0), Some((100.0, 200.0))),
            (100.0, 200.0)
        );
    }
}
