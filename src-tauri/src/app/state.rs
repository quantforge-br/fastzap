//! Shared application state registered with `app.manage()`.

use std::sync::Arc;

use crate::accounts::AccountStore;
use crate::downloads::Downloads;
use crate::paths::AppPaths;
use crate::secrets::SecretStore;
use crate::settings::store::SettingsStore;
use crate::webview::manager::WebviewManager;
use crate::webview::unread::UnreadRegistry;

pub struct AppState {
    pub paths: AppPaths,
    pub settings: SettingsStore,
    pub accounts: AccountStore,
    pub secrets: Arc<dyn SecretStore>,
    pub webviews: WebviewManager,
    pub unread: UnreadRegistry,
    pub downloads: Downloads,
    pub permissions: crate::permissions::Granted,
}
