//! WebKitGTK download completions, observed directly instead of through wry.
//!
//! wry still decides each download's destination (`on_download`, which
//! stages it): a handler connected only after the webview exists never saw
//! `decide-destination`, and WebKit saved straight to the system folder.
//! wry's completion report is ignored on Linux, though. Tauri keeps an
//! account's web context for the app's lifetime and wry connects its handler
//! to that context again for every recreated webview, so each completion
//! arrived once per registration. Each registration also shares one
//! `failed` flag that is never reset: after one failed or cancelled download
//! (a blocked one included), every later download of the account was
//! reported as failed and its file discarded. Here the context is observed
//! once and each download keeps its own state.

use std::cell::Cell;
use std::path::Path;
use std::rc::Rc;

use gtk::glib;
use gtk::glib::prelude::ObjectExt;
use tauri::{AppHandle, Manager, Runtime, Webview};
use webkit2gtk::{Download, DownloadExt, URIRequestExt, WebContext, WebContextExt, WebViewExt};

thread_local! {
    /// Web contexts already observed (GTK thread).
    static OBSERVED: std::cell::RefCell<Vec<glib::WeakRef<WebContext>>> =
        const { std::cell::RefCell::new(Vec::new()) };
}

/// Report the completions of `account_id`'s downloads; a recreated webview
/// of the same account reuses the observer already installed.
pub fn install<R: Runtime>(webview: &Webview<R>, account_id: i64) {
    let app = webview.app_handle().clone();
    let _ = webview.with_webview(move |platform| {
        let Some(context) = platform.inner().context() else {
            return;
        };
        let first = OBSERVED.with(|observed| {
            let mut observed = observed.borrow_mut();
            observed.retain(|weak| weak.upgrade().is_some());
            if observed
                .iter()
                .any(|weak| weak.upgrade().as_ref() == Some(&context))
            {
                return false;
            }
            observed.push(context.downgrade());
            true
        });
        if first {
            context.connect_download_started(move |_, download| {
                observe(&app, account_id, download);
            });
        }
    });
}

fn observe<R: Runtime>(app: &AppHandle<R>, account_id: i64, download: &Download) {
    let failed = Rc::new(Cell::new(false));
    download.connect_failed({
        let failed = failed.clone();
        move |_, _| failed.set(true)
    });
    let app = app.clone();
    download.connect_finished(move |download| {
        // No destination: cancelled before staging (blocked) or failed
        // before the response, so nothing is waiting for it.
        let Some(destination) = download.destination() else {
            return;
        };
        let url = download
            .request()
            .and_then(|r| r.uri())
            .map(|u| u.to_string())
            .unwrap_or_default();
        super::finished(
            &app,
            account_id,
            &url,
            Some(Path::new(destination.as_str())),
            !failed.get(),
        );
    });
}
