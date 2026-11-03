//! Child webviews hosting WhatsApp Web, one per enabled account.

use std::collections::BTreeSet;
use std::sync::Mutex;

use serde::{Deserialize, Serialize};
use tauri::webview::{NewWindowResponse, PageLoadEvent, WebviewBuilder};
use tauri::{
    AppHandle, LogicalPosition, LogicalSize, Manager, Runtime, Webview, WebviewUrl, Window,
};
use ts_rs::TS;
use url::Url;

use super::{navigation, scripts};
use crate::accounts::{Account, AccountKind, user_agent};
use crate::app::state::AppState;
use crate::app::{theme, window::MAIN_WINDOW};

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

    pub fn toggle_grid<R: Runtime>(&self, app: &AppHandle<R>) -> bool {
        let grid = {
            let mut l = self.layout.lock().unwrap();
            l.grid = !l.grid;
            l.grid
        };
        self.apply_layout(app);
        grid
    }

    pub fn set_overlay<R: Runtime>(&self, app: &AppHandle<R>, overlay: bool) {
        self.layout.lock().unwrap().overlay = overlay;
        self.apply_layout(app);
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

        let nav_app = app.clone();
        let popup_app = app.clone();
        let mut builder = WebviewBuilder::new(label(account.id), WebviewUrl::External(url))
            .user_agent(user_agent::resolve(&account.user_agent))
            .initialization_script(scripts::document_start(&settings))
            .zoom_hotkeys_enabled(false)
            .devtools(true)
            .on_navigation(move |url| {
                if navigation::is_internal(url) {
                    return true;
                }
                open_external(&nav_app, url);
                false
            })
            .on_new_window(move |url, _features| {
                if navigation::is_internal(&url) && url.scheme() != "about" {
                    return NewWindowResponse::Allow;
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

        let webview = window.add_child(
            builder,
            LogicalPosition::new(0.0, 0.0),
            LogicalSize::new(1.0, 1.0),
        )?;
        #[cfg(target_os = "linux")]
        super::stage::attach_child(&webview);
        let _ = webview.hide();
        let _ = webview.set_zoom(account.zoom);
        set_muted(&webview, settings.system.audio_muted);
        #[cfg(target_os = "linux")]
        if scripts::strict_proxy(&settings) {
            let _ = webview.with_webview(|platform| {
                use webkit2gtk::{SettingsExt, WebViewExt};
                if let Some(web_settings) = platform.inner().settings() {
                    web_settings.set_enable_webrtc(false);
                }
            });
        }
        #[cfg(target_os = "linux")]
        install_plain_paste_shortcut(&webview, account.id);
        Ok(webview)
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
        let mut webviews: Vec<(i64, Webview<R>)> = app
            .webviews()
            .into_iter()
            .filter_map(|(l, w)| account_id_from_label(&l).map(|id| (id, w)))
            .collect();
        let order = app
            .state::<AppState>()
            .accounts
            .list()
            .map(|list| list.into_iter().map(|a| a.id).collect::<Vec<_>>())
            .unwrap_or_default();
        webviews.sort_by_key(|(id, _)| order.iter().position(|o| o == id).unwrap_or(usize::MAX));

        if overlay || bounds.width < 1.0 || bounds.height < 1.0 {
            for (_, w) in &webviews {
                let _ = w.hide();
            }
            return;
        }

        if grid && !webviews.is_empty() {
            let cols = app
                .state::<AppState>()
                .settings
                .get()
                .appearance
                .grid_cols
                .clamp(1, webviews.len() as u32) as usize;
            let rows = webviews.len().div_ceil(cols);
            let cell_w = bounds.width / cols as f64;
            let cell_h = bounds.height / rows as f64;
            for (index, (_, w)) in webviews.iter().enumerate() {
                let (col, row) = (index % cols, index / cols);
                place(
                    w,
                    Rect {
                        x: bounds.x + col as f64 * cell_w,
                        y: bounds.y + row as f64 * cell_h,
                        width: cell_w,
                        height: cell_h,
                    },
                );
            }
            return;
        }

        for (id, w) in &webviews {
            if Some(*id) == active {
                place(w, bounds);
            } else {
                let _ = w.hide();
            }
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

    pub fn set_zoom<R: Runtime>(&self, app: &AppHandle<R>, account_id: i64, zoom: f64) {
        if let Some(webview) = app.get_webview(&label(account_id)) {
            let _ = webview.set_zoom(zoom);
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
    fn labels_round_trip() {
        assert_eq!(label(7), "wa-7");
        assert_eq!(account_id_from_label("wa-7"), Some(7));
        assert_eq!(account_id_from_label("ui"), None);
        assert_eq!(account_id_from_label("wa-x"), None);
    }
}
