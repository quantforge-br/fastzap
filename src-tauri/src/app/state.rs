//! Shared application state registered with `app.manage()`.

use std::sync::Arc;

use crate::accounts::AccountStore;
use crate::paths::AppPaths;
use crate::secrets::SecretStore;
use crate::settings::store::SettingsStore;

pub struct AppState {
    pub paths: AppPaths,
    pub settings: SettingsStore,
    pub accounts: AccountStore,
    pub secrets: Arc<dyn SecretStore>,
}
