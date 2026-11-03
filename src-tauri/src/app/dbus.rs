//! `org.freedesktop.Application` for Flatpak (ZapZap's
//! `desktop_application_dbus.py`): desktops activate the running instance
//! through the application ID instead of starting the binary again.

use std::collections::HashMap;

use tauri::AppHandle;
use zbus::zvariant::{OwnedValue, Value};

use super::{lifecycle, window};

struct Application {
    app: AppHandle,
}

/// Activation token from `platform-data`: `activation-token`, or the X11
/// `desktop-startup-id`.
fn activation_token(platform_data: &HashMap<String, OwnedValue>) -> Option<String> {
    ["activation-token", "desktop-startup-id"]
        .iter()
        .find_map(|key| match platform_data.get(*key).map(|v| &**v) {
            Some(Value::Str(token)) if !token.is_empty() => Some(token.to_string()),
            _ => None,
        })
}

#[zbus::interface(name = "org.freedesktop.Application")]
impl Application {
    fn activate(&self, platform_data: HashMap<String, OwnedValue>) {
        window::show_with_token(&self.app, activation_token(&platform_data));
    }

    fn open(&self, uris: Vec<String>, platform_data: HashMap<String, OwnedValue>) {
        window::show_with_token(&self.app, activation_token(&platform_data));
        for uri in uris {
            lifecycle::open_link(&self.app, &uri);
        }
    }

    fn activate_action(
        &self,
        _action_name: String,
        _parameter: Vec<OwnedValue>,
        platform_data: HashMap<String, OwnedValue>,
    ) {
        window::show_with_token(&self.app, activation_token(&platform_data));
    }
}

/// Own the application ID on the session bus while the app runs. The
/// returned connection releases the name when dropped.
pub async fn start(app: AppHandle) -> zbus::Result<zbus::Connection> {
    let path = format!("/{}", crate::APP_ID.replace('.', "/"));
    zbus::connection::Builder::session()?
        .name(crate::APP_ID)?
        .serve_at(path, Application { app })?
        .build()
        .await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn activation_token_prefers_wayland_token() {
        let mut data = HashMap::new();
        data.insert(
            "desktop-startup-id".to_owned(),
            OwnedValue::try_from(Value::from("x11")).unwrap(),
        );
        assert_eq!(activation_token(&data).as_deref(), Some("x11"));
        data.insert(
            "activation-token".to_owned(),
            OwnedValue::try_from(Value::from("wl")).unwrap(),
        );
        assert_eq!(activation_token(&data).as_deref(), Some("wl"));
        assert_eq!(activation_token(&HashMap::new()), None);
    }
}
