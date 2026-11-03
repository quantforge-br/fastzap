//! JSON-backed settings store with atomic writes and merge-patch updates.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::RwLock;

use serde_json::Value;

use super::Settings;

#[derive(Debug, thiserror::Error)]
pub enum StoreError {
    #[error("settings I/O failed: {0}")]
    Io(#[from] std::io::Error),
    #[error("invalid settings: {0}")]
    Invalid(#[from] serde_json::Error),
}

pub struct SettingsStore {
    path: PathBuf,
    current: RwLock<Settings>,
}

impl SettingsStore {
    /// Load `settings.json` from `path`. A missing file yields defaults; a corrupt
    /// file is moved aside to `settings.json.corrupt` and defaults are used.
    pub fn load(path: impl Into<PathBuf>) -> Self {
        let path = path.into();
        let settings = match fs::read_to_string(&path) {
            Ok(text) => match parse(&text) {
                Ok(settings) => settings,
                Err(err) => {
                    log::warn!("discarding unreadable settings file: {err}");
                    let _ = fs::rename(&path, path.with_extension("json.corrupt"));
                    Settings::default()
                }
            },
            Err(_) => Settings::default(),
        };
        Self {
            path,
            current: RwLock::new(settings),
        }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn exists_on_disk(&self) -> bool {
        self.path.exists()
    }

    pub fn get(&self) -> Settings {
        self.current.read().expect("settings lock").clone()
    }

    /// Replace all settings and persist them.
    pub fn replace(&self, mut settings: Settings) -> Result<Settings, StoreError> {
        settings.normalize();
        write_atomic(&self.path, &settings)?;
        *self.current.write().expect("settings lock") = settings.clone();
        Ok(settings)
    }

    /// Apply a JSON merge patch (RFC 7396) and persist the result.
    pub fn patch(&self, patch: &Value) -> Result<Settings, StoreError> {
        let mut doc = serde_json::to_value(self.get())?;
        merge_patch(&mut doc, patch);
        let settings: Settings = serde_json::from_value(doc)?;
        self.replace(settings)
    }

    /// Mutate settings in place and persist them.
    pub fn update(&self, f: impl FnOnce(&mut Settings)) -> Result<Settings, StoreError> {
        let mut settings = self.get();
        f(&mut settings);
        self.replace(settings)
    }
}

fn parse(text: &str) -> Result<Settings, serde_json::Error> {
    let mut settings: Settings = serde_json::from_str(text)?;
    migrate(&mut settings);
    settings.normalize();
    Ok(settings)
}

/// Upgrade older on-disk formats. Add one step per `SETTINGS_VERSION` bump.
fn migrate(_settings: &mut Settings) {}

fn write_atomic(path: &Path, settings: &Settings) -> Result<(), StoreError> {
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir)?;
    }
    let tmp = path.with_extension("json.tmp");
    fs::write(&tmp, serde_json::to_vec_pretty(settings)?)?;
    fs::rename(&tmp, path)?;
    Ok(())
}

pub fn merge_patch(target: &mut Value, patch: &Value) {
    match (target, patch) {
        (Value::Object(target), Value::Object(patch)) => {
            for (key, value) in patch {
                if value.is_null() {
                    target.remove(key);
                } else {
                    merge_patch(target.entry(key.clone()).or_insert(Value::Null), value);
                }
            }
        }
        (target, patch) => *target = patch.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::settings::ThemeMode;
    use serde_json::json;

    #[test]
    fn missing_file_yields_defaults() {
        let dir = tempfile::tempdir().unwrap();
        let store = SettingsStore::load(dir.path().join("settings.json"));
        assert_eq!(store.get(), Settings::default());
        assert!(!store.exists_on_disk());
    }

    #[test]
    fn patch_persists_and_reloads() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.json");
        let store = SettingsStore::load(&path);
        store
            .patch(&json!({"appearance": {"theme": "dark", "scale": 125}}))
            .unwrap();
        let reloaded = SettingsStore::load(&path).get();
        assert_eq!(reloaded.appearance.theme, ThemeMode::Dark);
        assert_eq!(reloaded.appearance.scale, 125);
        assert!(reloaded.appearance.sidebar);
    }

    #[test]
    fn invalid_patch_is_rejected_without_changes() {
        let dir = tempfile::tempdir().unwrap();
        let store = SettingsStore::load(dir.path().join("settings.json"));
        assert!(
            store
                .patch(&json!({"appearance": {"theme": "purple"}}))
                .is_err()
        );
        assert_eq!(store.get().appearance.theme, ThemeMode::Auto);
    }

    #[test]
    fn corrupt_file_is_moved_aside() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.json");
        fs::write(&path, "{not json").unwrap();
        let store = SettingsStore::load(&path);
        assert_eq!(store.get(), Settings::default());
        assert!(dir.path().join("settings.json.corrupt").exists());
    }
}
