//! Event names emitted to the local UI.

pub const SETTINGS_CHANGED: &str = "settings://changed";
pub const ACCOUNTS_CHANGED: &str = "accounts://changed";
pub const LAYOUT_CHANGED: &str = "layout://changed";
pub const UNREAD_CHANGED: &str = "unread://changed";
/// Ask the UI to perform an action it owns (open settings, dialogs, …).
pub const UI_ACTION: &str = "ui://action";
pub const ACCOUNT_SELECTOR: &str = "ui://account-selector";
