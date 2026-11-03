//! `org.freedesktop.Notifications` backend (native Linux installs).

use std::collections::HashMap;

use futures_util::StreamExt;
use zbus::zvariant::Value;

use super::{Activation, Outgoing};

#[zbus::proxy(
    interface = "org.freedesktop.Notifications",
    default_service = "org.freedesktop.Notifications",
    default_path = "/org/freedesktop/Notifications"
)]
trait Notifications {
    #[allow(clippy::too_many_arguments)]
    fn notify(
        &self,
        app_name: &str,
        replaces_id: u32,
        app_icon: &str,
        summary: &str,
        body: &str,
        actions: &[&str],
        hints: HashMap<&str, Value<'_>>,
        expire_timeout: i32,
    ) -> zbus::Result<u32>;

    fn close_notification(&self, id: u32) -> zbus::Result<()>;

    #[zbus(signal)]
    fn action_invoked(&self, id: u32, action_key: String) -> zbus::Result<()>;

    #[zbus(signal)]
    fn activation_token(&self, id: u32, activation_token: String) -> zbus::Result<()>;
}

pub struct Freedesktop {
    proxy: NotificationsProxy<'static>,
}

const TIMEOUT_MS: i32 = 3000;

impl Freedesktop {
    /// Connect and forward `default` actions (with their activation token).
    pub async fn connect(
        on_activate: impl Fn(Activation) + Send + Sync + 'static,
    ) -> zbus::Result<Self> {
        let conn = zbus::Connection::session().await?;
        let proxy = NotificationsProxy::new(&conn).await?;
        // One stream for both signals keeps their order: the daemon sends
        // ActivationToken before ActionInvoked for the same id, and two
        // separate streams could deliver the action first.
        let rule = zbus::MatchRule::builder()
            .msg_type(zbus::message::Type::Signal)
            .interface("org.freedesktop.Notifications")?
            .path("/org/freedesktop/Notifications")?
            .build();
        let mut signals = zbus::MessageStream::for_match_rule(rule, &conn, None).await?;
        tokio::spawn(async move {
            let mut pending: HashMap<u32, String> = HashMap::new();
            while let Some(Ok(message)) = signals.next().await {
                let header = message.header();
                match header.member().map(|m| m.as_str()) {
                    Some("ActivationToken") => {
                        if let Ok((id, token)) = message.body().deserialize::<(u32, String)>() {
                            pending.insert(id, token);
                        }
                    }
                    Some("ActionInvoked") => {
                        if let Ok((id, key)) = message.body().deserialize::<(u32, String)>() {
                            let token = pending.remove(&id);
                            if key == "default" {
                                on_activate(Activation {
                                    native: id.to_string(),
                                    token,
                                });
                            }
                        }
                    }
                    _ => {}
                }
            }
        });
        Ok(Self { proxy })
    }

    pub async fn notify(&self, n: &Outgoing) -> zbus::Result<String> {
        let mut hints: HashMap<&str, Value<'_>> = HashMap::new();
        hints.insert("urgency", Value::U8(1));
        hints.insert("category", Value::from("im.received"));
        hints.insert("suppress-sound", Value::Bool(n.silent));
        hints.insert("desktop-entry", Value::from(crate::APP_ID));
        let icon = n
            .image_path
            .as_deref()
            .map(|p| p.to_string_lossy().into_owned())
            .unwrap_or_default();
        if !icon.is_empty() {
            hints.insert("image-path", Value::from(icon.as_str()));
        }
        let id = self
            .proxy
            .notify(
                "FastZap",
                0,
                &icon,
                &n.title,
                &n.body,
                &["default", ""],
                hints,
                TIMEOUT_MS,
            )
            .await?;
        Ok(id.to_string())
    }

    pub async fn close(&self, native: &str) {
        if let Ok(id) = native.parse() {
            let _ = self.proxy.close_notification(id).await;
        }
    }
}
