//! Native notifications for messages from WhatsApp Web pages.
//!
//! Backend selection follows ZapZap: the XDG portal inside Flatpak,
//! `org.freedesktop.Notifications` on other Linux installs and the system
//! notification center (via `tauri-plugin-notification`) on Windows/macOS.

#[cfg(target_os = "linux")]
mod freedesktop;
pub mod image;
pub mod policy;
#[cfg(target_os = "linux")]
mod portal;

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use sha2::{Digest, Sha256};
use tauri::{AppHandle, Manager};

use crate::app::{actions, state::AppState, window};
use crate::webview::scripts;
use policy::WebNotification;

/// A notification as handed to a backend.
pub struct Outgoing {
    pub title: String,
    pub body: String,
    pub silent: bool,
    pub image_png: Option<Vec<u8>>,
    /// Same image on disk, for backends that need a path.
    pub image_path: Option<PathBuf>,
}

/// The user activated a notification.
pub struct Activation {
    pub native: String,
    pub token: Option<String>,
}

enum Backend {
    #[cfg(target_os = "linux")]
    Freedesktop(freedesktop::Freedesktop),
    #[cfg(target_os = "linux")]
    Portal(portal::Portal),
    #[cfg_attr(target_os = "linux", allow(dead_code))]
    System,
}

struct Record {
    account_id: i64,
    web_id: u64,
    tag: String,
    native: String,
}

#[derive(Default)]
pub struct NotificationService {
    backend: tokio::sync::Mutex<Option<Arc<Backend>>>,
    records: Mutex<Vec<Record>>,
    /// Notifications being submitted, and whether the page closed them
    /// meanwhile, by (account, page id).
    pending: Mutex<HashMap<(i64, u64), bool>>,
}

async fn connect(app: &AppHandle) -> Option<Backend> {
    #[cfg(target_os = "linux")]
    {
        let handle = app.clone();
        let on_activate = move |a: Activation| activated(&handle, a);
        if crate::platform::is_flatpak() {
            match portal::Portal::connect(on_activate).await {
                Ok(p) => return Some(Backend::Portal(p)),
                Err(e) => log::warn!("notification portal unavailable: {e}"),
            }
        } else {
            match freedesktop::Freedesktop::connect(on_activate).await {
                Ok(f) => return Some(Backend::Freedesktop(f)),
                Err(e) => log::warn!("org.freedesktop.Notifications unavailable: {e}"),
            }
        }
        None
    }
    #[cfg(not(target_os = "linux"))]
    {
        let _ = app;
        Some(Backend::System)
    }
}

impl NotificationService {
    /// The current backend, (re)connecting lazily after a failure.
    async fn backend(&self, app: &AppHandle) -> Option<Arc<Backend>> {
        let mut guard = self.backend.lock().await;
        if guard.is_none() {
            *guard = connect(app).await.map(Arc::new);
        }
        guard.clone()
    }

    #[cfg(target_os = "linux")]
    async fn drop_backend(&self) {
        *self.backend.lock().await = None;
    }

    fn take_records(&self, pred: impl Fn(&Record) -> bool) -> Vec<Record> {
        let mut records = self.records.lock().unwrap();
        let (taken, kept) = records.drain(..).partition(pred);
        *records = kept;
        taken
    }
}

#[cfg_attr(not(target_os = "linux"), allow(unused_variables))]
async fn close_native(backend: &Backend, native: &str) {
    match backend {
        #[cfg(target_os = "linux")]
        Backend::Freedesktop(f) => f.close(native).await,
        #[cfg(target_os = "linux")]
        Backend::Portal(p) => p.close(native).await,
        Backend::System => {}
    }
}

fn image_for(app: &AppHandle, account_id: i64, title: &str, photo: Option<&str>) -> Outgoing {
    let state = app.state::<AppState>();
    let (png, name) = match photo.and_then(image::circular_photo) {
        Some(png) => {
            let digest = Sha256::digest(format!("{account_id}\0{title}"));
            let name = digest[..12]
                .iter()
                .map(|b| format!("{b:02x}"))
                .collect::<String>();
            (Some(png), format!("{name}.png"))
        }
        None => (image::app_icon(), "fastzap.png".into()),
    };
    let dir = state.paths.notification_images_dir();
    let path = png.as_ref().and_then(|bytes| {
        std::fs::create_dir_all(&dir).ok()?;
        let path = dir.join(name);
        std::fs::write(&path, bytes).ok()?;
        Some(path)
    });
    Outgoing {
        title: String::new(),
        body: String::new(),
        silent: false,
        image_png: png,
        image_path: path,
    }
}

static IMAGE_JOBS: tokio::sync::Semaphore = tokio::sync::Semaphore::const_new(2);

/// Show a notification created by `account_id`'s page.
pub async fn notify(app: &AppHandle, account_id: i64, web: WebNotification) {
    let state = app.state::<AppState>();
    let Ok(account) = state.accounts.get(account_id) else {
        return;
    };
    let Some(content) = policy::compose(&state.settings.get(), &account, &web) else {
        return;
    };
    let key = (account_id, web.id);
    let service = app.state::<NotificationService>();
    service.pending.lock().unwrap().insert(key, false);
    let outcome = submit(app, account_id, &web, content).await;
    let cancelled = service.pending.lock().unwrap().remove(&key) == Some(true);
    let Some((backend, native)) = outcome else {
        return;
    };
    if cancelled {
        close_native(&backend, &native).await;
        return;
    }
    // A newer notification with the same tag replaces older ones (ZapZap closes
    // them after posting the new one).
    let replaced = if web.tag.is_empty() {
        Vec::new()
    } else {
        service.take_records(|r| r.account_id == account_id && r.tag == web.tag)
    };
    service.records.lock().unwrap().push(Record {
        account_id,
        web_id: web.id,
        tag: web.tag,
        native,
    });
    for record in replaced {
        close_native(&backend, &record.native).await;
    }
}

/// Post the notification; returns the backend and the native id to track.
/// The system backend (Windows/macOS) has no id, so it returns `None`.
#[cfg_attr(not(target_os = "linux"), allow(unreachable_code))]
async fn submit(
    app: &AppHandle,
    account_id: i64,
    web: &WebNotification,
    content: policy::Content,
) -> Option<(Arc<Backend>, String)> {
    let photo = content.photo.then(|| web.icon.clone()).flatten();
    let mut outgoing = {
        // Decoding is CPU-bound: keep it off the async workers, two at a time.
        let Ok(_permit) = IMAGE_JOBS.acquire().await else {
            return None;
        };
        let (app, title) = (app.clone(), content.title.clone());
        match tauri::async_runtime::spawn_blocking(move || {
            image_for(&app, account_id, &title, photo.as_deref())
        })
        .await
        {
            Ok(outgoing) => outgoing,
            Err(_) => return None,
        }
    };
    outgoing.title = content.title;
    outgoing.body = content.body;
    outgoing.silent = content.silent;

    let service = app.state::<NotificationService>();
    let backend = service.backend(app).await?;
    let native = match backend.as_ref() {
        #[cfg(target_os = "linux")]
        Backend::Freedesktop(f) => match f.notify(&outgoing).await {
            Ok(id) => id,
            Err(e) => {
                log::warn!("notification failed, reconnecting next time: {e}");
                service.drop_backend().await;
                return None;
            }
        },
        #[cfg(target_os = "linux")]
        Backend::Portal(p) => {
            let id = format!(
                "fastzap-account-{account_id}-{:016x}",
                rand::random::<u64>()
            );
            if let Err(e) = p.notify(&outgoing, &id).await {
                log::warn!("portal notification failed: {e}");
                return None;
            }
            id
        }
        Backend::System => {
            use tauri_plugin_notification::NotificationExt;
            let mut builder = app
                .notification()
                .builder()
                .title(&outgoing.title)
                .body(&outgoing.body);
            if !outgoing.silent {
                builder = builder.sound("default");
            }
            if let Err(e) = builder.show() {
                log::warn!("system notification failed: {e}");
            }
            return None;
        }
    };
    Some((backend, native))
}

/// The page closed its notification (e.g. the chat was read).
pub async fn close_web(app: &AppHandle, account_id: i64, web_id: u64) {
    let service = app.state::<NotificationService>();
    // Still being submitted: `notify` withdraws it once posted.
    if let Some(cancelled) = service
        .pending
        .lock()
        .unwrap()
        .get_mut(&(account_id, web_id))
    {
        *cancelled = true;
        return;
    }
    let records = service.take_records(|r| r.account_id == account_id && r.web_id == web_id);
    if records.is_empty() {
        return;
    }
    let Some(backend) = service.backend.lock().await.clone() else {
        return;
    };
    for record in records {
        close_native(&backend, &record.native).await;
    }
}

/// Focus the window and account, then let the page open the chat.
#[cfg_attr(not(target_os = "linux"), allow(dead_code))]
fn activated(app: &AppHandle, activation: Activation) {
    let service = app.state::<NotificationService>();
    // Notification daemons may broadcast actions of other applications.
    let Some(record) = service
        .take_records(|r| r.native == activation.native)
        .pop()
    else {
        return;
    };
    window::show_with_token(app, activation.token);
    actions::activate(app, record.account_id);
    app.state::<AppState>().webviews.eval(
        app,
        record.account_id,
        &scripts::notification_clicked(record.web_id),
    );
    #[cfg(target_os = "linux")]
    if crate::platform::is_flatpak() {
        let app = app.clone();
        tauri::async_runtime::spawn(async move {
            let service = app.state::<NotificationService>();
            if let Some(backend) = service.backend.lock().await.clone() {
                close_native(&backend, &record.native).await;
            }
        });
    }
}

/// Withdraw every notification still shown (shutdown).
pub async fn close_all(app: &AppHandle) {
    let service = app.state::<NotificationService>();
    let records = service.take_records(|_| true);
    let Some(backend) = service.backend.lock().await.clone() else {
        return;
    };
    for record in records {
        close_native(&backend, &record.native).await;
    }
}
