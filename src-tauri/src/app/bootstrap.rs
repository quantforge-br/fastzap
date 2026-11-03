//! Startup sequence: paths, settings, one-shot ZapZap import and accounts.

use std::sync::Arc;

use tauri::{AppHandle, Manager, Runtime};

use super::state::AppState;
use crate::accounts::AccountStore;
use crate::migration;
use crate::paths::AppPaths;
use crate::secrets::{KeyringStore, SecretStore};
use crate::settings::store::SettingsStore;

pub fn init_state<R: Runtime>(app: &AppHandle<R>) -> anyhow::Result<()> {
    let paths = AppPaths::resolve(app)?;
    let settings = SettingsStore::load(paths.settings_file());
    let accounts = AccountStore::open(&paths.accounts_db())?;
    let secrets: Arc<dyn SecretStore> = Arc::new(KeyringStore);

    let mut current = settings.get();
    if !current.migration.zapzap_checked {
        let fresh = !settings.exists_on_disk() && accounts.count()? == 0;
        if fresh && let Some(source) = dirs::home_dir().and_then(|h| migration::detect(&h)) {
            let report = migration::import(
                &source,
                &mut current,
                &accounts,
                &paths.customizations_dir(),
                secrets.as_ref(),
            );
            log::info!(
                "imported ZapZap data from {}: {} account(s), {} file(s), {} warning(s)",
                report.source,
                report.accounts_imported,
                report.customization_files_copied,
                report.warnings.len()
            );
            for warning in &report.warnings {
                log::warn!("ZapZap import: {warning}");
            }
        }
        current.migration.zapzap_checked = true;
        settings.replace(current)?;
    }
    accounts.ensure_default()?;

    app.manage(AppState {
        paths,
        settings,
        accounts,
        secrets,
        webviews: Default::default(),
        unread: Default::default(),
    });
    Ok(())
}
