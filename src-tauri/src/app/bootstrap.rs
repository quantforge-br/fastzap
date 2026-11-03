//! Startup sequence: paths, settings, one-shot ZapZap import and accounts.

use std::sync::Arc;

use tauri::{AppHandle, Manager, Runtime};

use super::state::AppState;
use crate::accounts::AccountStore;
use crate::downloads::Downloads;
use crate::migration;
use crate::paths::AppPaths;
use crate::secrets::{KeyringStore, SecretStore};
use crate::settings::store::SettingsStore;

/// An import with warnings is retried on the next starts, up to this many times.
const MAX_IMPORT_ATTEMPTS: u32 = 3;

pub fn init_state<R: Runtime>(app: &AppHandle<R>) -> anyhow::Result<()> {
    let paths = AppPaths::resolve(app)?;
    let settings = SettingsStore::load(paths.settings_file())?;
    let accounts = AccountStore::open(&paths.accounts_db())?;
    let secrets: Arc<dyn SecretStore> = Arc::new(KeyringStore);

    let mut current = settings.get();
    if !current.migration.zapzap_checked {
        let retry = current.migration.zapzap_imported_from.is_some();
        let eligible = retry || (!settings.exists_on_disk() && accounts.count()? == 0);
        let source = eligible
            .then(|| dirs::home_dir().and_then(|h| migration::detect(&h)))
            .flatten();
        let complete = match source {
            Some(source) => {
                let report = migration::import(
                    &source,
                    &mut current,
                    &accounts,
                    &paths.customizations_dir(),
                    secrets.as_ref(),
                    !retry,
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
                current.migration.zapzap_attempts += 1;
                report.warnings.is_empty()
                    || current.migration.zapzap_attempts >= MAX_IMPORT_ATTEMPTS
            }
            None => true,
        };
        current.migration.zapzap_checked = complete;
        settings.replace(current)?;
    }
    accounts.ensure_default()?;

    let downloads = Downloads::new(paths.download_staging_dir());
    app.manage(AppState {
        paths,
        settings,
        accounts,
        secrets,
        webviews: Default::default(),
        unread: Default::default(),
        downloads,
        permissions: Default::default(),
    });
    Ok(())
}
