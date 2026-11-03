//! XDG desktop portal backend (Flatpak).

use ashpd::desktop::Icon;
use ashpd::desktop::notification::{Notification, NotificationProxy, Priority};
use futures_util::StreamExt;

use super::{Activation, Outgoing};

pub struct Portal {
    proxy: NotificationProxy<'static>,
}

/// Extract `activation-token` from the action's platform data, if any.
fn activation_token(params: &[ashpd::zvariant::OwnedValue]) -> Option<String> {
    params.iter().find_map(|value| {
        let dict = ashpd::zvariant::Dict::try_from(value.try_clone().ok()?).ok()?;
        dict.get::<&str, String>(&"activation-token").ok().flatten()
    })
}

impl Portal {
    pub async fn connect(
        on_activate: impl Fn(Activation) + Send + Sync + 'static,
    ) -> ashpd::Result<Self> {
        let proxy = NotificationProxy::new().await?;
        let mut actions = proxy.receive_action_invoked().await?;
        tokio::spawn(async move {
            while let Some(action) = actions.next().await {
                if action.name() == "focus" {
                    on_activate(Activation {
                        native: action.id().to_owned(),
                        token: activation_token(action.parameter()),
                    });
                }
            }
        });
        Ok(Self { proxy })
    }

    pub async fn notify(&self, n: &Outgoing, native: &str) -> ashpd::Result<()> {
        let mut notification = Notification::new(&n.title)
            .body(n.body.as_str())
            .priority(Priority::Normal)
            .default_action("focus");
        if let Some(png) = &n.image_png {
            notification = notification.icon(Icon::Bytes(png.clone()));
        }
        self.proxy.add_notification(native, notification).await
    }

    pub async fn close(&self, native: &str) {
        let _ = self.proxy.remove_notification(native).await;
    }
}
