//! Web permission requests of WhatsApp Web pages (ZapZap's
//! `permissions_manager.py`): notifications are always granted, features set
//! to auto grant are allowed, and anything else asks the user. A "Yes" is
//! remembered in memory for that account's page; "No" is asked again.

use std::collections::{HashMap, HashSet};
use std::sync::Mutex;

use serde::Serialize;
use ts_rs::TS;

use crate::settings::AutoGrant;

pub const PERMISSION_REQUEST: &str = "permissions://request";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub enum Feature {
    Microphone,
    Camera,
    CameraMicrophone,
    Location,
    ScreenContents,
    ScreenContentsAudio,
    MouseLock,
}

impl Feature {
    pub fn auto_granted(self, auto: &AutoGrant) -> bool {
        match self {
            Feature::Microphone => auto.microphone,
            Feature::Camera => auto.camera,
            Feature::CameraMicrophone => auto.camera_microphone,
            Feature::Location => auto.location,
            Feature::ScreenContents => auto.screen_contents,
            Feature::ScreenContentsAudio => auto.screen_contents_audio,
            Feature::MouseLock => auto.mouse_lock,
        }
    }

    /// Qt's split of user media requests.
    pub fn from_media(audio: bool, video: bool, display: bool) -> Option<Self> {
        match (display, audio, video) {
            (true, true, _) => Some(Feature::ScreenContentsAudio),
            (true, false, _) => Some(Feature::ScreenContents),
            (false, true, true) => Some(Feature::CameraMicrophone),
            (false, true, false) => Some(Feature::Microphone),
            (false, false, true) => Some(Feature::Camera),
            (false, false, false) => None,
        }
    }
}

#[derive(Debug, Clone, Serialize, TS)]
#[ts(export)]
pub struct PermissionPrompt {
    #[ts(type = "number")]
    pub id: u64,
    #[ts(type = "number")]
    pub account_id: i64,
    pub feature: Feature,
}

/// Features the user allowed per account, for the life of its page.
#[derive(Default)]
pub struct Granted {
    granted: Mutex<HashMap<i64, HashSet<Feature>>>,
}

impl Granted {
    pub fn contains(&self, account_id: i64, feature: Feature) -> bool {
        self.granted
            .lock()
            .unwrap()
            .get(&account_id)
            .is_some_and(|set| set.contains(&feature))
    }

    pub fn insert(&self, account_id: i64, feature: Feature) {
        self.granted
            .lock()
            .unwrap()
            .entry(account_id)
            .or_default()
            .insert(feature);
    }

    /// A new page starts with nothing remembered.
    pub fn forget(&self, account_id: i64) {
        self.granted.lock().unwrap().remove(&account_id);
    }
}

#[cfg(target_os = "linux")]
mod linux {
    use std::cell::RefCell;
    use std::collections::HashMap;
    use std::sync::atomic::{AtomicU64, Ordering};

    use gtk::glib::prelude::*;
    use gtk::glib::translate::ToGlibPtr;
    use tauri::{AppHandle, Emitter, Manager, Runtime, Webview};
    use webkit2gtk::{
        GeolocationPermissionRequest, NotificationPermissionRequest, PermissionRequest,
        PermissionRequestExt, PointerLockPermissionRequest, UserMediaPermissionRequest,
        UserMediaPermissionRequestExt, WebViewExt,
    };

    use super::{Feature, PERMISSION_REQUEST, PermissionPrompt};
    use crate::app::state::AppState;

    static NEXT_ID: AtomicU64 = AtomicU64::new(1);

    thread_local! {
        // GObjects stay on the GTK thread until the user answers.
        static PENDING: RefCell<HashMap<u64, (i64, Feature, PermissionRequest)>> =
            RefCell::new(HashMap::new());
    }

    fn feature(request: &PermissionRequest) -> Option<Feature> {
        if let Some(media) = request.downcast_ref::<UserMediaPermissionRequest>() {
            // webkit2gtk 2.0 has no safe wrapper for this getter.
            let display = unsafe {
                webkit2gtk_sys::webkit_user_media_permission_is_for_display_device(
                    media.to_glib_none().0,
                )
            } != 0;
            return Feature::from_media(
                media.is_for_audio_device(),
                media.is_for_video_device(),
                display,
            );
        }
        if request.is::<GeolocationPermissionRequest>() {
            return Some(Feature::Location);
        }
        if request.is::<PointerLockPermissionRequest>() {
            return Some(Feature::MouseLock);
        }
        None
    }

    pub fn install<R: Runtime>(webview: &Webview<R>, account_id: i64) {
        let app = webview.app_handle().clone();
        app.state::<AppState>().permissions.forget(account_id);
        let _ = webview.with_webview(move |platform| {
            // Prompts of the replaced page must not grant the new one.
            PENDING.with(|p| {
                p.borrow_mut().retain(|_, (account, _, request)| {
                    if *account == account_id {
                        request.deny();
                    }
                    *account != account_id
                })
            });
            platform
                .inner()
                .connect_permission_request(move |_, request| {
                    if request.is::<NotificationPermissionRequest>() {
                        request.allow();
                        return true;
                    }
                    // Other kinds (device enumeration, storage access, …) get the
                    // engine's default answer, which denies.
                    let Some(feature) = feature(request) else {
                        return false;
                    };
                    let state = app.state::<AppState>();
                    let auto = state.settings.get().permissions.auto_grant;
                    if feature.auto_granted(&auto) {
                        state.permissions.insert(account_id, feature);
                        request.allow();
                        return true;
                    }
                    if state.permissions.contains(account_id, feature) {
                        request.allow();
                        return true;
                    }
                    let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
                    PENDING.with(|p| {
                        p.borrow_mut()
                            .insert(id, (account_id, feature, request.clone()))
                    });
                    let _ = app.emit(
                        PERMISSION_REQUEST,
                        PermissionPrompt {
                            id,
                            account_id,
                            feature,
                        },
                    );
                    true
                });
        });
    }

    /// Apply the user's answer on the GTK thread.
    pub fn answer<R: Runtime>(app: &AppHandle<R>, id: u64, allow: bool) {
        let handle = app.clone();
        let _ = app.run_on_main_thread(move || {
            let Some((account_id, feature, request)) = PENDING.with(|p| p.borrow_mut().remove(&id))
            else {
                return;
            };
            if allow {
                handle
                    .state::<AppState>()
                    .permissions
                    .insert(account_id, feature);
                request.allow();
            } else {
                request.deny();
            }
        });
    }
}

#[cfg(target_os = "linux")]
pub use linux::{answer, install};

/// WebView2 and WKWebView handle permissions with their own prompts.
#[cfg(not(target_os = "linux"))]
pub fn answer<R: tauri::Runtime>(_app: &tauri::AppHandle<R>, _id: u64, _allow: bool) {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn media_requests_map_to_zapzap_features() {
        assert_eq!(
            Feature::from_media(true, false, false),
            Some(Feature::Microphone)
        );
        assert_eq!(
            Feature::from_media(false, true, false),
            Some(Feature::Camera)
        );
        assert_eq!(
            Feature::from_media(true, true, false),
            Some(Feature::CameraMicrophone)
        );
        assert_eq!(
            Feature::from_media(false, true, true),
            Some(Feature::ScreenContents)
        );
        assert_eq!(
            Feature::from_media(true, true, true),
            Some(Feature::ScreenContentsAudio)
        );
        assert_eq!(Feature::from_media(false, false, false), None);
    }

    #[test]
    fn grants_are_per_account_and_forgotten_with_the_page() {
        let granted = Granted::default();
        granted.insert(1, Feature::Camera);
        assert!(granted.contains(1, Feature::Camera));
        assert!(!granted.contains(2, Feature::Camera));
        assert!(!granted.contains(1, Feature::Microphone));
        granted.forget(1);
        assert!(!granted.contains(1, Feature::Camera));
        let auto = AutoGrant {
            location: true,
            ..Default::default()
        };
        assert!(Feature::Location.auto_granted(&auto));
        assert!(!Feature::MouseLock.auto_granted(&auto));
    }
}
