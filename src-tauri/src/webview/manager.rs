//! Child webviews hosting WhatsApp Web, one per enabled account.

use std::collections::BTreeSet;
use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};

use serde::{Deserialize, Serialize};
use tauri::webview::{NewWindowFeatures, NewWindowResponse, PageLoadEvent, WebviewBuilder};
use tauri::{
    AppHandle, LogicalPosition, LogicalSize, Manager, Runtime, Webview, WebviewUrl, WebviewWindow,
    WebviewWindowBuilder, Window,
};
use ts_rs::TS;
use url::Url;

use super::{navigation, scripts};
use crate::accounts::{Account, AccountKind, user_agent};
use crate::app::state::AppState;
use crate::app::theme;
use crate::app::window::{MAIN_WINDOW, UI_WEBVIEW};
use crate::settings::Settings;

const LABEL_PREFIX: &str = "wa-";

pub fn label(account_id: i64) -> String {
    format!("{LABEL_PREFIX}{account_id}")
}

pub fn account_id_from_label(label: &str) -> Option<i64> {
    label.strip_prefix(LABEL_PREFIX)?.parse().ok()
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct Rect {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

#[derive(Debug, Default)]
struct Layout {
    bounds: Rect,
    active: Option<i64>,
    grid: bool,
    overlay: bool,
    /// The visible account webview and where it was last placed.
    placed: Option<(i64, Rect)>,
}

#[derive(Debug, Clone, Serialize, TS)]
#[ts(export)]
pub struct LayoutState {
    #[ts(type = "number | null")]
    pub active: Option<i64>,
    pub grid: bool,
    pub overlay: bool,
}

#[derive(Default)]
pub struct WebviewManager {
    layout: Mutex<Layout>,
    /// Accounts whose page finished loading, and scripts waiting for it.
    /// One mutex, so a script cannot be queued after its page drained.
    loads: Mutex<PageLoads>,
}

#[derive(Default)]
struct PageLoads {
    ready: BTreeSet<i64>,
    pending: Vec<(i64, String)>,
}

impl WebviewManager {
    pub fn state(&self) -> LayoutState {
        let l = self.layout.lock().unwrap();
        LayoutState {
            active: l.active,
            grid: l.grid,
            overlay: l.overlay,
        }
    }

    pub fn active(&self) -> Option<i64> {
        self.layout.lock().unwrap().active
    }

    /// Bounds of the visible account webview in window coordinates.
    pub fn placed_rect(&self, account_id: i64) -> Option<Rect> {
        match self.layout.lock().unwrap().placed {
            Some((id, rect)) if id == account_id => Some(rect),
            _ => None,
        }
    }

    pub fn set_bounds<R: Runtime>(&self, app: &AppHandle<R>, bounds: Rect) {
        self.layout.lock().unwrap().bounds = bounds;
        self.apply_layout(app);
    }

    pub fn set_active<R: Runtime>(&self, app: &AppHandle<R>, account_id: Option<i64>) {
        {
            let mut l = self.layout.lock().unwrap();
            l.active = account_id;
            l.grid = false;
        }
        self.apply_layout(app);
        if let Some(id) = account_id
            && let Some(webview) = app.get_webview(&label(id))
        {
            let _ = webview.set_focus();
        }
    }

    pub fn toggle_grid<R: Runtime>(&self, app: &AppHandle<R>) {
        let grid = !self.layout.lock().unwrap().grid;
        self.set_grid(app, grid);
    }

    /// Grid view is ZapZap's account overview, rendered by the local UI; the
    /// account webviews are hidden meanwhile.
    pub fn set_grid<R: Runtime>(&self, app: &AppHandle<R>, grid: bool) {
        self.layout.lock().unwrap().grid = grid;
        self.apply_layout(app);
        self.focus_visible(app);
    }

    pub fn set_overlay<R: Runtime>(&self, app: &AppHandle<R>, overlay: bool) {
        self.layout.lock().unwrap().overlay = overlay;
        self.apply_layout(app);
        self.focus_visible(app);
    }

    /// Keyboard focus would otherwise stay in a hidden account webview.
    fn focus_visible<R: Runtime>(&self, app: &AppHandle<R>) {
        let target = {
            let l = self.layout.lock().unwrap();
            if l.overlay || l.grid { None } else { l.active }
        };
        let webview = match target {
            Some(id) => app.get_webview(&label(id)),
            None => app.get_webview(UI_WEBVIEW),
        };
        if let Some(webview) = webview {
            let _ = webview.set_focus();
        }
    }

    /// Create webviews for enabled WhatsApp Web accounts and close the others.
    pub fn sync<R: Runtime>(&self, app: &AppHandle<R>) -> anyhow::Result<()> {
        let state = app.state::<AppState>();
        let accounts = state.accounts.list()?;
        let wanted: BTreeSet<i64> = accounts
            .iter()
            .filter(|a| a.enabled && a.kind == AccountKind::WhatsappWeb)
            .map(|a| a.id)
            .collect();

        for (webview_label, webview) in app.webviews() {
            if let Some(id) = account_id_from_label(&webview_label)
                && !wanted.contains(&id)
            {
                let _ = webview.close();
            }
        }

        let window = app
            .get_window(MAIN_WINDOW)
            .ok_or_else(|| anyhow::anyhow!("main window missing"))?;
        for account in accounts.iter().filter(|a| wanted.contains(&a.id)) {
            if app.get_webview(&label(account.id)).is_none() {
                self.create(app, &window, account)?;
            }
        }

        {
            let mut l = self.layout.lock().unwrap();
            let active_valid = l
                .active
                .is_some_and(|id| accounts.iter().any(|a| a.id == id && a.enabled));
            if !active_valid {
                l.active = accounts.iter().find(|a| a.enabled).map(|a| a.id);
            }
        }
        self.apply_layout(app);
        Ok(())
    }

    fn create<R: Runtime>(
        &self,
        app: &AppHandle<R>,
        window: &Window<R>,
        account: &Account,
    ) -> anyhow::Result<Webview<R>> {
        let state = app.state::<AppState>();
        let settings = state.settings.get();
        let url = Url::parse(navigation::WHATSAPP_URL)?;

        let popup_app = app.clone();
        let mut builder = WebviewBuilder::new(label(account.id), WebviewUrl::External(url))
            .user_agent(user_agent::resolve(&account.user_agent))
            .initialization_script(scripts::document_start(&settings))
            .zoom_hotkeys_enabled(false)
            .devtools(true)
            // Without this Tauri swallows OS file drops for its own drag-drop
            // event; WhatsApp Web needs the native HTML5 drop to attach files.
            .disable_drag_drop_handler()
            // As in ZapZap, other hosts are only blocked: WhatsApp also loads
            // background frames (e.g. flows.whatsapp.net cache management)
            // that must not reach the browser. Links the user opens arrive
            // through on_new_window.
            .on_navigation(navigation::is_internal)
            .on_new_window(move |url, features| {
                if navigation::is_internal(&url) && url.scheme() != "about" {
                    return match open_internal_popup(&popup_app, features) {
                        Ok(window) => NewWindowResponse::Create { window },
                        Err(e) => {
                            log::warn!("failed to open WhatsApp popup: {e}");
                            NewWindowResponse::Deny
                        }
                    };
                }
                open_external(&popup_app, &url);
                NewWindowResponse::Deny
            })
            .on_page_load(|webview, payload| {
                if payload.event() == PageLoadEvent::Finished {
                    on_page_ready(&webview);
                }
            });

        if let Some(script) = scripts::all_frames(&settings) {
            builder = builder.initialization_script_for_all_frames(script);
        }

        let download_app = app.clone();
        let account_id = account.id;
        builder = builder.on_download(move |_webview, event| {
            crate::downloads::handle_event(&download_app, account_id, event)
        });

        #[cfg(any(target_os = "linux", target_os = "windows"))]
        {
            builder = builder.data_directory(state.paths.profile_dir(account.id));
        }
        #[cfg(target_os = "macos")]
        {
            builder = builder.data_store_identifier(data_store_identifier(account.id));
        }
        if let Some(proxy) = crate::proxy::webview_proxy_url(&settings.proxy) {
            builder = builder.proxy_url(proxy);
        }

        self.loads.lock().unwrap().ready.remove(&account.id);
        let webview = window.add_child(
            builder,
            LogicalPosition::new(0.0, 0.0),
            LogicalSize::new(1.0, 1.0),
        )?;
        #[cfg(target_os = "linux")]
        super::stage::attach_child(&webview);
        let _ = webview.hide();
        let _ = webview.set_zoom(account.zoom * interface_scale(&settings));
        set_muted(&webview, settings.system.audio_muted);
        #[cfg(target_os = "linux")]
        apply_engine_options(&webview, &settings);
        #[cfg(target_os = "linux")]
        install_plain_paste_shortcut(&webview, account.id);
        #[cfg(target_os = "linux")]
        crate::permissions::install(&webview, account.id);
        #[cfg(target_os = "linux")]
        crate::spellcheck::install_context_menu(&webview);
        #[cfg(target_os = "linux")]
        crate::downloads::linux::install(&webview, account.id);
        #[cfg(target_os = "linux")]
        reload_after_crash(&webview, account.id);
        Ok(webview)
    }

    /// Recreate every account webview, e.g. after a setting captured at
    /// creation changed.
    pub fn recreate_all<R: Runtime>(&self, app: &AppHandle<R>) -> anyhow::Result<()> {
        for (webview_label, webview) in app.webviews() {
            if account_id_from_label(&webview_label).is_some() {
                webview.close()?;
            }
        }
        self.sync(app)
    }

    /// Recreate one account webview, e.g. after its User-Agent changed.
    pub fn recreate<R: Runtime>(&self, app: &AppHandle<R>, account_id: i64) -> anyhow::Result<()> {
        if let Some(webview) = app.get_webview(&label(account_id)) {
            webview.close()?;
        }
        self.sync(app)
    }

    pub fn apply_layout<R: Runtime>(&self, app: &AppHandle<R>) {
        let (bounds, active, grid, overlay) = {
            let l = self.layout.lock().unwrap();
            (l.bounds, l.active, l.grid, l.overlay)
        };
        let hidden = overlay || grid || bounds.width < 1.0 || bounds.height < 1.0;
        let mut placed = None;
        for (webview_label, w) in app.webviews() {
            let Some(id) = account_id_from_label(&webview_label) else {
                continue;
            };
            if !hidden && Some(id) == active {
                place(&w, bounds);
                placed = Some((id, bounds));
            } else {
                let _ = w.hide();
            }
        }
        self.layout.lock().unwrap().placed = placed;
    }

    /// Evaluate `script` now if the page has loaded, or after it loads.
    pub fn eval_when_ready<R: Runtime>(&self, app: &AppHandle<R>, account_id: i64, script: String) {
        {
            let mut loads = self.loads.lock().unwrap();
            if !loads.ready.contains(&account_id) {
                loads.pending.push((account_id, script));
                return;
            }
        }
        self.eval(app, account_id, &script);
    }

    fn page_loaded<R: Runtime>(&self, webview: &Webview<R>, account_id: i64) {
        let scripts: Vec<String> = {
            let mut loads = self.loads.lock().unwrap();
            loads.ready.insert(account_id);
            let (now, later) = loads
                .pending
                .drain(..)
                .partition(|(id, _)| *id == account_id);
            loads.pending = later;
            now.into_iter().map(|(_, script)| script).collect()
        };
        for script in scripts {
            let _ = webview.eval(script);
        }
    }

    pub fn eval<R: Runtime>(&self, app: &AppHandle<R>, account_id: i64, script: &str) {
        if let Some(webview) = app.get_webview(&label(account_id)) {
            let _ = webview.eval(script);
        }
    }

    pub fn eval_all<R: Runtime>(&self, app: &AppHandle<R>, script: &str) {
        for (l, webview) in app.webviews() {
            if account_id_from_label(&l).is_some() {
                let _ = webview.eval(script);
            }
        }
    }

    pub fn reload<R: Runtime>(&self, app: &AppHandle<R>, account_id: i64) {
        if let Some(webview) = app.get_webview(&label(account_id)) {
            let _ = webview.reload();
        }
    }

    /// Apply an account zoom, combined with the interface scale.
    pub fn set_zoom<R: Runtime>(&self, app: &AppHandle<R>, account_id: i64, zoom: f64) {
        if let Some(webview) = app.get_webview(&label(account_id)) {
            let scale = interface_scale(&app.state::<AppState>().settings.get());
            let _ = webview.set_zoom(zoom * scale);
        }
    }

    pub fn set_muted_all<R: Runtime>(&self, app: &AppHandle<R>, muted: bool) {
        for (l, webview) in app.webviews() {
            if account_id_from_label(&l).is_some() {
                set_muted(&webview, muted);
            }
        }
    }

    pub fn navigate<R: Runtime>(&self, app: &AppHandle<R>, account_id: i64, url: Url) {
        if let Some(webview) = app.get_webview(&label(account_id)) {
            let _ = webview.navigate(url);
        }
    }
}

/// ZapZap's interface scale (`system/scale`) as a zoom factor.
pub fn interface_scale(settings: &Settings) -> f64 {
    f64::from(settings.appearance.scale) / 100.0
}

/// Settings baked into a webview when it is created. A reload keeps them, so
/// a change requires new webviews.
#[derive(Debug, PartialEq)]
pub struct CreationConfig {
    document_start: String,
    all_frames: Option<&'static str>,
    proxy: Option<Url>,
    engine: EngineOptions,
}

/// Engine preferences mapped from ZapZap's performance settings.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct EngineOptions {
    pub hardware_acceleration: bool,
    pub smooth_scrolling: bool,
}

impl EngineOptions {
    pub fn from_settings(settings: &Settings) -> Self {
        let p = &settings.performance;
        Self {
            hardware_acceleration: !(p.disable_gpu || p.software_rendering),
            smooth_scrolling: p.scroll_animator,
        }
    }
}

pub fn creation_config(settings: &Settings) -> CreationConfig {
    CreationConfig {
        document_start: scripts::document_start(settings),
        all_frames: scripts::all_frames(settings),
        proxy: crate::proxy::webview_proxy_url(&settings.proxy),
        engine: EngineOptions::from_settings(settings),
    }
}

#[cfg(target_os = "linux")]
fn apply_engine_options<R: Runtime>(webview: &Webview<R>, settings: &Settings) {
    let options = EngineOptions::from_settings(settings);
    let strict = scripts::strict_proxy(settings);
    let _ = webview.with_webview(move |platform| {
        use webkit2gtk::{HardwareAccelerationPolicy, SettingsExt, WebViewExt};
        let Some(web_settings) = platform.inner().settings() else {
            return;
        };
        if strict {
            web_settings.set_enable_webrtc(false);
        }
        if !options.hardware_acceleration {
            web_settings.set_hardware_acceleration_policy(HardwareAccelerationPolicy::Never);
        }
        web_settings.set_enable_smooth_scrolling(options.smooth_scrolling);
    });
}

/// Reload the page after its web process dies (crash or memory limit), as
/// ZapZap does after a renderer crash; the account would otherwise stay
/// blank until its webview is recreated.
#[cfg(target_os = "linux")]
fn reload_after_crash<R: Runtime>(webview: &Webview<R>, account_id: i64) {
    let _ = webview.with_webview(move |platform| {
        use gtk::glib::{self, prelude::ObjectExt};
        use webkit2gtk::{WebProcessTerminationReason, WebViewExt};
        platform
            .inner()
            .connect_web_process_terminated(move |view, reason| {
                if reason == WebProcessTerminationReason::TerminatedByApi {
                    return;
                }
                log::warn!("account {account_id} web process ended ({reason:?}), reloading");
                let view = view.downgrade();
                glib::timeout_add_local_once(std::time::Duration::from_secs(1), move || {
                    if let Some(view) = view.upgrade() {
                        view.reload();
                    }
                });
            });
    });
}

fn place<R: Runtime>(webview: &Webview<R>, rect: Rect) {
    #[cfg(target_os = "linux")]
    super::stage::place(webview, rect);
    let _ = webview.set_position(LogicalPosition::new(rect.x, rect.y));
    let _ = webview.set_size(LogicalSize::new(rect.width.max(1.0), rect.height.max(1.0)));
    let _ = webview.show();
}

/// Mute page audio (ZapZap's `system/audio_muted`).
fn set_muted<R: Runtime>(webview: &Webview<R>, muted: bool) {
    #[cfg(target_os = "linux")]
    {
        let _ = webview.with_webview(move |platform| {
            use webkit2gtk::WebViewExt;
            platform.inner().set_is_muted(muted);
        });
    }
    #[cfg(not(target_os = "linux"))]
    {
        // WebView2 and WKWebView expose no muting through Tauri yet.
        let _ = (webview, muted);
    }
}

/// Handle `Ctrl+Shift+V` natively so page scripts cannot trigger a
/// clipboard read through the bridge.
#[cfg(target_os = "linux")]
fn install_plain_paste_shortcut<R: Runtime>(webview: &Webview<R>, account_id: i64) {
    let app = webview.app_handle().clone();
    let _ = webview.with_webview(move |platform| {
        use gtk::gdk::{ModifierType, keys::constants as keys};
        use gtk::prelude::WidgetExt;
        platform.inner().connect_key_press_event(move |_, event| {
            let modifiers = event.state()
                & (ModifierType::CONTROL_MASK | ModifierType::SHIFT_MASK | ModifierType::MOD1_MASK);
            let key = event.keyval();
            if modifiers == ModifierType::CONTROL_MASK | ModifierType::SHIFT_MASK
                && (key == keys::V || key == keys::v)
            {
                let app = app.clone();
                // Reading the clipboard may block; keep it off the GTK thread.
                std::thread::spawn(move || {
                    crate::app::actions::paste_plain_text(&app, account_id);
                });
                return gtk::glib::Propagation::Stop;
            }
            gtk::glib::Propagation::Proceed
        });
    });
}

/// Open a WhatsApp popup (e.g. a media viewer) in its own window that keeps
/// the account's navigation allowlist, like ZapZap's internal popups. The
/// opener's request loads into the related view; nothing is exposed to IPC
/// because popup labels match no capability.
fn open_internal_popup<R: Runtime>(
    app: &AppHandle<R>,
    features: NewWindowFeatures,
) -> anyhow::Result<WebviewWindow<R>> {
    static NEXT_POPUP: AtomicU64 = AtomicU64::new(1);
    let label = format!("popup-{}", NEXT_POPUP.fetch_add(1, Ordering::Relaxed));
    let nav_app = app.clone();
    let popup_app = app.clone();
    Ok(
        WebviewWindowBuilder::new(app, label, WebviewUrl::External(Url::parse("about:blank")?))
            .window_features(features)
            .title("FastZap")
            .on_document_title_changed(|window, title| {
                let _ = window.set_title(&title);
            })
            .on_navigation(move |url| {
                if url.as_str() == "about:blank" || navigation::is_internal(url) {
                    return true;
                }
                open_external(&nav_app, url);
                false
            })
            .on_new_window(move |url, _features| {
                open_external(&popup_app, &url);
                NewWindowResponse::Deny
            })
            .build()?,
    )
}

fn open_external<R: Runtime>(app: &AppHandle<R>, url: &Url) {
    if !navigation::is_safe_external(url) {
        log::warn!("blocked navigation to {}", url.scheme());
        return;
    }
    use tauri_plugin_opener::OpenerExt;
    if let Err(e) = app.opener().open_url(url.as_str(), None::<&str>) {
        log::warn!("failed to open external link: {e}");
    }
}

/// Run ZapZap's DocumentReady scripts and apply theme-dependent state.
pub fn on_page_ready<R: Runtime>(webview: &Webview<R>) {
    let app = webview.app_handle();
    let state = app.state::<AppState>();
    let settings = state.settings.get();
    let dark = theme::is_dark(app, &settings);
    let labels = page_labels();
    let _ = webview.eval(scripts::document_ready(&settings, dark, &labels));
    if let Some(id) = account_id_from_label(webview.label()) {
        crate::customizations::apply_to_webview(app, id, webview);
        state.webviews.page_loaded(webview, id);
    }
}

pub fn page_labels() -> scripts::PageLabels {
    use crate::i18n::tr;
    scripts::PageLabels {
        selector: tr("Account selector"),
        selector_activity: tr("Notifications"),
        resize_handle: tr("Chat list width"),
        resize_hint: tr(
            "Drag or use the arrow keys to resize. Double-click or press Enter to restore the default width.",
        ),
    }
}

#[cfg(target_os = "macos")]
fn data_store_identifier(account_id: i64) -> [u8; 16] {
    use sha2::{Digest, Sha256};
    let digest = Sha256::digest(format!(
        "io.github.quantmind_br.fastzap/account/{account_id}"
    ));
    let mut id = [0u8; 16];
    id.copy_from_slice(&digest[..16]);
    id
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn privacy_and_proxy_changes_need_new_webviews() {
        let old = Settings::default();
        let mut new = old.clone();
        new.appearance.grid_cols += 1;
        assert_eq!(creation_config(&old), creation_config(&new));
        new.privacy.webrtc_shield = !old.privacy.webrtc_shield;
        assert_ne!(creation_config(&old), creation_config(&new));
        let mut gpu = old.clone();
        gpu.performance.disable_gpu = true;
        assert_ne!(creation_config(&old), creation_config(&gpu));
    }

    #[test]
    fn labels_round_trip() {
        assert_eq!(label(7), "wa-7");
        assert_eq!(account_id_from_label("wa-7"), Some(7));
        assert_eq!(account_id_from_label("ui"), None);
        assert_eq!(account_id_from_label("wa-x"), None);
    }
}
