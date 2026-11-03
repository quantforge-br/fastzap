//! Downloads started by WhatsApp Web pages.

use tauri::webview::DownloadEvent;
use tauri::{AppHandle, Runtime};

pub fn handle_event<R: Runtime>(
    _app: &AppHandle<R>,
    _account_id: i64,
    event: DownloadEvent<'_>,
) -> bool {
    match event {
        DownloadEvent::Requested { destination, .. } => {
            if let Some(dir) = dirs::download_dir() {
                let name = destination
                    .file_name()
                    .map(|n| n.to_owned())
                    .unwrap_or_else(|| "download".into());
                *destination = dir.join(name);
            }
            true
        }
        DownloadEvent::Finished { .. } => true,
        _ => true,
    }
}
