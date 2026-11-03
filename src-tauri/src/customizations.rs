//! User CSS/JS customizations injected into WhatsApp Web.

use tauri::{AppHandle, Runtime, Webview};

pub fn apply_to_webview<R: Runtime>(_app: &AppHandle<R>, _account_id: i64, _webview: &Webview<R>) {}

pub fn reapply_all<R: Runtime>(_app: &AppHandle<R>) {}
