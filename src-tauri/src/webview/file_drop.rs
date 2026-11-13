//! Files dropped on WhatsApp Web webviews (Linux).
//!
//! WebKitGTK hands an OS file drop to the page as a `text/uri-list` string
//! with an empty `dataTransfer.files`, and WhatsApp Web only attaches
//! `File` objects, so a native drop did nothing. These webviews keep Tauri's
//! drag-drop handler, which reports the dropped paths; each file is read
//! here, pushed into the page in base64 chunks, and the page replays the drop
//! with real `File` objects (`__FASTZAP__.drop*` in `scripts/bridge.js`).

use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

use base64::Engine;
use tauri::{DragDropEvent, PhysicalPosition, Runtime, Webview, WebviewEvent};
use webkit2gtk::WebViewExt;

use crate::upload::{MAX_UPLOAD, mime_for, read_upload};

/// Raw bytes per evaluated chunk; a multiple of 3, so every chunk is
/// standalone base64.
const CHUNK: usize = 3 * 256 * 1024;

static NEXT_DROP: AtomicU64 = AtomicU64::new(1);

/// Replay the file drops `webview` receives inside its page.
pub fn install<R: Runtime>(webview: &Webview<R>) {
    let target = webview.clone();
    webview.on_webview_event(move |event| {
        if let WebviewEvent::DragDrop(DragDropEvent::Drop { paths, position }) = event {
            let webview = target.clone();
            let (paths, position) = (paths.clone(), *position);
            tauri::async_runtime::spawn(async move { deliver(webview, paths, position).await });
        }
    });
}

async fn deliver<R: Runtime>(
    webview: Webview<R>,
    paths: Vec<PathBuf>,
    position: PhysicalPosition<f64>,
) {
    // Links dragged from a browser also arrive as "paths"; only local
    // files are dropped.
    let paths: Vec<PathBuf> = paths.into_iter().filter(|p| p.exists()).collect();
    if paths.is_empty() {
        return;
    }
    // wry reports GTK widget coordinates; the page works in CSS pixels.
    let zoom = zoom_level(&webview)
        .await
        .filter(|z| *z > 0.0)
        .unwrap_or(1.0);
    let token = NEXT_DROP.fetch_add(1, Ordering::Relaxed);
    let eval = |script: String| {
        if let Err(e) = webview.eval(script) {
            log::warn!("failed to deliver a dropped file: {e}");
        }
    };
    let mut errors = Vec::new();
    eval(format!("window.__FASTZAP__?.dropBegin({token})"));
    for path in paths {
        let name = path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| "file".to_owned());
        let read_path = path.clone();
        let bytes =
            match tokio::task::spawn_blocking(move || read_upload(&read_path, MAX_UPLOAD)).await {
                Ok(Ok(bytes)) => bytes,
                Ok(Err(e)) => {
                    errors.push(format!("{name}: {e}"));
                    continue;
                }
                Err(e) => {
                    log::warn!("dropped file reader failed: {e}");
                    continue;
                }
            };
        eval(format!(
            "window.__FASTZAP__?.dropFile({token}, {}, {})",
            json(&name),
            json(mime_for(&path)),
        ));
        for chunk in bytes.chunks(CHUNK) {
            let data = base64::engine::general_purpose::STANDARD.encode(chunk);
            eval(format!(
                "window.__FASTZAP__?.dropChunk({token}, \"{data}\")"
            ));
        }
    }
    eval(format!(
        "window.__FASTZAP__?.dropEnd({token}, {}, {}, {})",
        position.x / zoom,
        position.y / zoom,
        json(&errors),
    ));
}

async fn zoom_level<R: Runtime>(webview: &Webview<R>) -> Option<f64> {
    let (tx, rx) = tokio::sync::oneshot::channel();
    webview
        .with_webview(move |platform| {
            let _ = tx.send(platform.inner().zoom_level());
        })
        .ok()?;
    rx.await.ok()
}

fn json<T: serde::Serialize + ?Sized>(value: &T) -> String {
    serde_json::to_string(value).unwrap_or_else(|_| "null".to_owned())
}
