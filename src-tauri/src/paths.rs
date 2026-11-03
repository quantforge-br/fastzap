//! Filesystem locations used by FastZap.

use std::path::PathBuf;

use tauri::{AppHandle, Manager, Runtime};

pub const DICTIONARIES_DIR: &str = "dictionaries";

#[derive(Debug, Clone)]
pub struct AppPaths {
    pub config_dir: PathBuf,
    pub data_dir: PathBuf,
    pub cache_dir: PathBuf,
}

impl AppPaths {
    /// Named after the app ID, not Tauri's bundle identifier: bundle
    /// identifiers cannot contain `_`, and the folders must not move.
    pub fn resolve<R: Runtime>(app: &AppHandle<R>) -> tauri::Result<Self> {
        let path = app.path();
        Ok(Self {
            config_dir: path.config_dir()?.join(crate::APP_ID),
            data_dir: path.data_dir()?.join(crate::APP_ID),
            cache_dir: path.cache_dir()?.join(crate::APP_ID),
        })
    }

    pub fn settings_file(&self) -> PathBuf {
        self.config_dir.join("settings.json")
    }

    pub fn accounts_db(&self) -> PathBuf {
        self.data_dir.join("accounts.db")
    }

    /// Isolated WebView data directory of a WhatsApp Web account.
    pub fn profile_dir(&self, account_id: i64) -> PathBuf {
        self.data_dir.join("profiles").join(account_id.to_string())
    }

    pub fn customizations_dir(&self) -> PathBuf {
        self.data_dir.join("customizations")
    }

    pub fn evolution_dir(&self, account_id: i64) -> PathBuf {
        self.data_dir.join("evolution").join(account_id.to_string())
    }

    /// Enchant configuration directory; dictionaries go to its `hunspell/`.
    pub fn dictionaries_dir(&self) -> PathBuf {
        self.data_dir.join(DICTIONARIES_DIR)
    }

    pub fn reports_dir(&self) -> PathBuf {
        self.data_dir.join("reports")
    }

    /// Where the engine writes downloads until the user's decision.
    pub fn download_staging_dir(&self) -> PathBuf {
        self.cache_dir.join("download-staging")
    }

    pub fn notification_images_dir(&self) -> PathBuf {
        self.cache_dir.join("notification-images")
    }
}
