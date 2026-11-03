//! Secret storage backed by the operating system keyring.
//!
//! Secrets never enter `settings.json`, logs or IPC events. Values are wrapped in
//! [`Secret`] so `Debug` output cannot leak them by accident.

use std::collections::HashMap;
use std::fmt;
use std::sync::Mutex;

pub const SERVICE: &str = "io.github.quantmind_br.fastzap";
pub const PROXY_PASSWORD: &str = "proxy-password";

pub fn evolution_admin_key(account_id: i64) -> String {
    format!("evolution-admin-key:{account_id}")
}

pub fn evolution_instance_token(account_id: i64) -> String {
    format!("evolution-instance-token:{account_id}")
}

#[derive(Clone, PartialEq, Eq)]
pub struct Secret(String);

impl Secret {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    pub fn expose(&self) -> &str {
        &self.0
    }
}

impl fmt::Debug for Secret {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Secret(<redacted>)")
    }
}

#[derive(Debug, thiserror::Error)]
#[error("keyring operation failed: {0}")]
pub struct SecretError(String);

pub trait SecretStore: Send + Sync {
    fn get(&self, key: &str) -> Result<Option<Secret>, SecretError>;
    fn set(&self, key: &str, value: &Secret) -> Result<(), SecretError>;
    fn delete(&self, key: &str) -> Result<(), SecretError>;
}

pub struct KeyringStore;

impl KeyringStore {
    fn entry(key: &str) -> Result<keyring::Entry, SecretError> {
        keyring::Entry::new(SERVICE, key).map_err(|e| SecretError(e.to_string()))
    }
}

impl SecretStore for KeyringStore {
    fn get(&self, key: &str) -> Result<Option<Secret>, SecretError> {
        match Self::entry(key)?.get_password() {
            Ok(value) => Ok(Some(Secret(value))),
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(e) => Err(SecretError(e.to_string())),
        }
    }

    fn set(&self, key: &str, value: &Secret) -> Result<(), SecretError> {
        Self::entry(key)?
            .set_password(value.expose())
            .map_err(|e| SecretError(e.to_string()))
    }

    fn delete(&self, key: &str) -> Result<(), SecretError> {
        match Self::entry(key)?.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(e) => Err(SecretError(e.to_string())),
        }
    }
}

/// In-memory store used by tests and as a fallback when no keyring is reachable.
#[derive(Default)]
pub struct MemoryStore(Mutex<HashMap<String, String>>);

impl SecretStore for MemoryStore {
    fn get(&self, key: &str) -> Result<Option<Secret>, SecretError> {
        Ok(self.0.lock().unwrap().get(key).cloned().map(Secret))
    }

    fn set(&self, key: &str, value: &Secret) -> Result<(), SecretError> {
        self.0
            .lock()
            .unwrap()
            .insert(key.to_owned(), value.expose().to_owned());
        Ok(())
    }

    fn delete(&self, key: &str) -> Result<(), SecretError> {
        self.0.lock().unwrap().remove(key);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn debug_never_prints_the_value() {
        let s = Secret::new("hunter2");
        assert!(!format!("{s:?}").contains("hunter2"));
    }

    #[test]
    fn memory_store_round_trip() {
        let store = MemoryStore::default();
        store.set("k", &Secret::new("v")).unwrap();
        assert_eq!(store.get("k").unwrap().unwrap().expose(), "v");
        store.delete("k").unwrap();
        assert!(store.get("k").unwrap().is_none());
    }
}
