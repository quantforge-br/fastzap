//! What a notification shows, following ZapZap's privacy settings.

use serde::Deserialize;

use crate::accounts::Account;
use crate::i18n::tr;
use crate::settings::Settings;

const MAX_TITLE: usize = 256;
const MAX_BODY: usize = 1024;
/// Data URLs larger than this are ignored (the bridge already caps blobs at 2 MiB).
pub const MAX_ICON_URL: usize = 3 * 1024 * 1024;

/// A `Notification` created by WhatsApp Web, as reported by `bridge.js`.
#[derive(Debug, Clone, Deserialize)]
pub struct WebNotification {
    pub id: u64,
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub body: String,
    #[serde(default)]
    pub tag: String,
    #[serde(default)]
    pub icon: Option<String>,
    #[serde(default)]
    pub silent: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Content {
    pub title: String,
    pub body: String,
    /// Use the sender photo (`show_photo`); otherwise the app icon.
    pub photo: bool,
    pub silent: bool,
}

fn truncate(text: &str, max: usize) -> String {
    text.chars().take(max).collect()
}

/// `None` when notifications are off globally or for the account (DND).
pub fn compose(settings: &Settings, account: &Account, web: &WebNotification) -> Option<Content> {
    let n = &settings.notifications;
    if !n.enabled || !account.notifications {
        return None;
    }
    Some(Content {
        title: if n.show_name {
            truncate(&web.title, MAX_TITLE)
        } else {
            "FastZap".into()
        },
        body: if n.show_message {
            truncate(&web.body, MAX_BODY)
        } else {
            tr("New message...")
        },
        photo: n.show_photo,
        silent: !n.sound || settings.system.audio_muted || web.silent,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::accounts::AccountKind;

    fn account(notifications: bool) -> Account {
        Account {
            id: 1,
            name: "A".into(),
            icon: None,
            enabled: true,
            zoom: 1.0,
            kind: AccountKind::WhatsappWeb,
            position: 0,
            user_agent: "Default".into(),
            notifications,
            evolution: None,
        }
    }

    fn web() -> WebNotification {
        WebNotification {
            id: 1,
            title: "Ana".into(),
            body: "hi".into(),
            tag: "t".into(),
            icon: None,
            silent: false,
        }
    }

    #[test]
    fn respects_global_and_account_switches() {
        let mut s = Settings::default();
        assert!(compose(&s, &account(false), &web()).is_none());
        s.notifications.enabled = false;
        assert!(compose(&s, &account(true), &web()).is_none());
    }

    #[test]
    fn hides_private_fields() {
        let mut s = Settings::default();
        s.notifications.show_name = false;
        s.notifications.show_message = false;
        let c = compose(&s, &account(true), &web()).unwrap();
        assert_eq!(c.title, "FastZap");
        assert_eq!(c.body, "New message...");
    }

    #[test]
    fn silent_when_sound_off_or_muted() {
        let mut s = Settings::default();
        assert!(!compose(&s, &account(true), &web()).unwrap().silent);
        s.system.audio_muted = true;
        assert!(compose(&s, &account(true), &web()).unwrap().silent);
        s.system.audio_muted = false;
        s.notifications.sound = false;
        assert!(compose(&s, &account(true), &web()).unwrap().silent);
    }
}
