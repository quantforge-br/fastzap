//! Accounts persisted in SQLite (`accounts.db`).

pub mod user_agent;

use std::path::Path;
use std::sync::Mutex;

use rusqlite::{Connection, OptionalExtension, Row, params};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub enum AccountKind {
    WhatsappWeb,
    EvolutionGo,
}

impl AccountKind {
    fn as_str(self) -> &'static str {
        match self {
            Self::WhatsappWeb => "whatsapp_web",
            Self::EvolutionGo => "evolution_go",
        }
    }

    fn parse(value: &str) -> Self {
        match value {
            "evolution_go" => Self::EvolutionGo,
            _ => Self::WhatsappWeb,
        }
    }
}

/// Non-secret Evolution Go connection data. Keys and tokens live in the keyring.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct EvolutionConfig {
    pub base_url: String,
    pub instance_id: Option<String>,
    pub instance_name: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct Account {
    #[ts(type = "number")]
    pub id: i64,
    pub name: String,
    /// SVG markup or a `data:image/png;base64,...` URL.
    pub icon: Option<String>,
    pub enabled: bool,
    pub zoom: f64,
    pub kind: AccountKind,
    #[ts(type = "number")]
    pub position: i64,
    /// A key of [`user_agent::USER_AGENTS`] (`"Default"` by default).
    pub user_agent: String,
    /// Per-account notifications; `false` is "do not disturb".
    pub notifications: bool,
    pub evolution: Option<EvolutionConfig>,
}

#[derive(Debug, Clone, Deserialize, TS)]
#[ts(export)]
pub struct NewAccount {
    pub name: String,
    pub icon: Option<String>,
    pub kind: AccountKind,
    pub evolution: Option<EvolutionConfig>,
}

#[derive(Debug, Clone, Default, Deserialize, TS)]
#[serde(default)]
#[ts(export)]
pub struct AccountPatch {
    pub name: Option<String>,
    #[ts(type = "string | null | undefined")]
    pub icon: Option<Option<String>>,
    pub enabled: Option<bool>,
    pub zoom: Option<f64>,
    pub user_agent: Option<String>,
    pub notifications: Option<bool>,
    pub evolution: Option<EvolutionConfig>,
}

#[derive(Debug, thiserror::Error)]
pub enum AccountError {
    #[error("account limit of {0} reached")]
    LimitReached(u32),
    #[error("account {0} not found")]
    NotFound(i64),
    #[error("an Evolution Go account requires its connection settings")]
    MissingEvolutionConfig,
    #[error("database error: {0}")]
    Db(#[from] rusqlite::Error),
}

pub const MIN_ZOOM: f64 = 0.25;
pub const MAX_ZOOM: f64 = 5.0;

const SCHEMA: &str = r#"
CREATE TABLE IF NOT EXISTS accounts (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    name TEXT NOT NULL DEFAULT '',
    icon TEXT,
    enabled INTEGER NOT NULL DEFAULT 1,
    zoom REAL NOT NULL DEFAULT 1.0,
    kind TEXT NOT NULL DEFAULT 'whatsapp_web',
    position INTEGER NOT NULL DEFAULT 0,
    user_agent TEXT NOT NULL DEFAULT 'Default',
    notifications INTEGER NOT NULL DEFAULT 1,
    evo_base_url TEXT,
    evo_instance_id TEXT,
    evo_instance_name TEXT
);
"#;

pub struct AccountStore {
    conn: Mutex<Connection>,
}

impl AccountStore {
    pub fn open(path: &Path) -> Result<Self, AccountError> {
        if let Some(dir) = path.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        Self::with_connection(Connection::open(path)?)
    }

    pub fn in_memory() -> Result<Self, AccountError> {
        Self::with_connection(Connection::open_in_memory()?)
    }

    fn with_connection(conn: Connection) -> Result<Self, AccountError> {
        conn.execute_batch(SCHEMA)?;
        conn.pragma_update(None, "user_version", 1)?;
        Ok(Self {
            conn: Mutex::new(conn),
        })
    }

    pub fn list(&self) -> Result<Vec<Account>, AccountError> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare("SELECT * FROM accounts ORDER BY position, id")?;
        let rows = stmt.query_map([], row_to_account)?;
        Ok(rows.collect::<Result<_, _>>()?)
    }

    pub fn get(&self, id: i64) -> Result<Account, AccountError> {
        let conn = self.conn.lock().unwrap();
        conn.query_row("SELECT * FROM accounts WHERE id = ?1", [id], row_to_account)
            .optional()?
            .ok_or(AccountError::NotFound(id))
    }

    pub fn count(&self) -> Result<u32, AccountError> {
        let conn = self.conn.lock().unwrap();
        Ok(conn.query_row("SELECT COUNT(*) FROM accounts", [], |r| r.get(0))?)
    }

    pub fn create(&self, new: NewAccount, limit: u32) -> Result<Account, AccountError> {
        if new.kind == AccountKind::EvolutionGo && new.evolution.is_none() {
            return Err(AccountError::MissingEvolutionConfig);
        }
        if self.count()? >= limit {
            return Err(AccountError::LimitReached(limit));
        }
        let id = {
            let conn = self.conn.lock().unwrap();
            let position: i64 = conn.query_row(
                "SELECT COALESCE(MAX(position), -1) + 1 FROM accounts",
                [],
                |r| r.get(0),
            )?;
            let evo = new.evolution.as_ref();
            conn.execute(
                "INSERT INTO accounts (name, icon, kind, position, evo_base_url, evo_instance_id, evo_instance_name)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                params![
                    new.name.trim(),
                    new.icon,
                    new.kind.as_str(),
                    position,
                    evo.map(|e| e.base_url.clone()),
                    evo.and_then(|e| e.instance_id.clone()),
                    evo.and_then(|e| e.instance_name.clone()),
                ],
            )?;
            conn.last_insert_rowid()
        };
        self.get(id)
    }

    /// Insert an account preserving an explicit id (used by the ZapZap import).
    pub fn insert_imported(&self, account: &Account) -> Result<(), AccountError> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT OR REPLACE INTO accounts (id, name, icon, enabled, zoom, kind, position, user_agent, notifications)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            params![
                account.id,
                account.name,
                account.icon,
                account.enabled,
                account.zoom.clamp(MIN_ZOOM, MAX_ZOOM),
                account.kind.as_str(),
                account.position,
                account.user_agent,
                account.notifications,
            ],
        )?;
        Ok(())
    }

    pub fn update(&self, id: i64, patch: AccountPatch) -> Result<Account, AccountError> {
        let mut account = self.get(id)?;
        if let Some(name) = patch.name {
            account.name = name.trim().to_owned();
        }
        if let Some(icon) = patch.icon {
            account.icon = icon;
        }
        if let Some(enabled) = patch.enabled {
            account.enabled = enabled;
        }
        if let Some(zoom) = patch.zoom {
            account.zoom = zoom.clamp(MIN_ZOOM, MAX_ZOOM);
        }
        if let Some(ua) = patch.user_agent {
            account.user_agent = ua;
        }
        if let Some(n) = patch.notifications {
            account.notifications = n;
        }
        if let Some(evo) = patch.evolution {
            account.evolution = Some(evo);
        }
        let evo = account.evolution.as_ref();
        self.conn.lock().unwrap().execute(
            "UPDATE accounts SET name = ?2, icon = ?3, enabled = ?4, zoom = ?5, user_agent = ?6,
             notifications = ?7, evo_base_url = ?8, evo_instance_id = ?9, evo_instance_name = ?10
             WHERE id = ?1",
            params![
                id,
                account.name,
                account.icon,
                account.enabled,
                account.zoom,
                account.user_agent,
                account.notifications,
                evo.map(|e| e.base_url.clone()),
                evo.and_then(|e| e.instance_id.clone()),
                evo.and_then(|e| e.instance_name.clone()),
            ],
        )?;
        Ok(account)
    }

    pub fn delete(&self, id: i64) -> Result<(), AccountError> {
        let changed = self
            .conn
            .lock()
            .unwrap()
            .execute("DELETE FROM accounts WHERE id = ?1", [id])?;
        if changed == 0 {
            return Err(AccountError::NotFound(id));
        }
        Ok(())
    }

    /// Persist a new order; ids missing from `ids` keep their relative order after it.
    pub fn reorder(&self, ids: &[i64]) -> Result<(), AccountError> {
        let mut conn = self.conn.lock().unwrap();
        let tx = conn.transaction()?;
        for (position, id) in ids.iter().enumerate() {
            tx.execute(
                "UPDATE accounts SET position = ?2 WHERE id = ?1",
                params![id, position as i64],
            )?;
        }
        tx.commit()?;
        Ok(())
    }

    /// Create the initial WhatsApp Web account when the store is empty.
    pub fn ensure_default(&self) -> Result<(), AccountError> {
        if self.count()? == 0 {
            self.create(
                NewAccount {
                    name: String::new(),
                    icon: None,
                    kind: AccountKind::WhatsappWeb,
                    evolution: None,
                },
                u32::MAX,
            )?;
        }
        Ok(())
    }
}

fn row_to_account(row: &Row<'_>) -> rusqlite::Result<Account> {
    let kind = AccountKind::parse(&row.get::<_, String>("kind")?);
    let evolution = match row.get::<_, Option<String>>("evo_base_url")? {
        Some(base_url) => Some(EvolutionConfig {
            base_url,
            instance_id: row.get("evo_instance_id")?,
            instance_name: row.get("evo_instance_name")?,
        }),
        None => None,
    };
    Ok(Account {
        id: row.get("id")?,
        name: row.get("name")?,
        icon: row.get("icon")?,
        enabled: row.get("enabled")?,
        zoom: row.get("zoom")?,
        kind,
        position: row.get("position")?,
        user_agent: row.get("user_agent")?,
        notifications: row.get("notifications")?,
        evolution,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn web(name: &str) -> NewAccount {
        NewAccount {
            name: name.into(),
            icon: None,
            kind: AccountKind::WhatsappWeb,
            evolution: None,
        }
    }

    #[test]
    fn create_respects_limit_and_order() {
        let store = AccountStore::in_memory().unwrap();
        let a = store.create(web("A"), 2).unwrap();
        let b = store.create(web("B"), 2).unwrap();
        assert!(matches!(
            store.create(web("C"), 2),
            Err(AccountError::LimitReached(2))
        ));
        assert_eq!(a.position, 0);
        assert_eq!(b.position, 1);
        store.reorder(&[b.id, a.id]).unwrap();
        let names: Vec<_> = store.list().unwrap().into_iter().map(|a| a.name).collect();
        assert_eq!(names, ["B", "A"]);
    }

    #[test]
    fn evolution_account_requires_config() {
        let store = AccountStore::in_memory().unwrap();
        let mut new = web("Evo");
        new.kind = AccountKind::EvolutionGo;
        assert!(matches!(
            store.create(new.clone(), 5),
            Err(AccountError::MissingEvolutionConfig)
        ));
        new.evolution = Some(EvolutionConfig {
            base_url: "https://evo.example".into(),
            instance_id: Some("uuid".into()),
            instance_name: None,
        });
        let acc = store.create(new, 5).unwrap();
        assert_eq!(acc.kind, AccountKind::EvolutionGo);
        assert_eq!(acc.evolution.unwrap().instance_id.as_deref(), Some("uuid"));
    }

    #[test]
    fn update_clamps_zoom_and_clears_icon() {
        let store = AccountStore::in_memory().unwrap();
        let mut new = web("A");
        new.icon = Some("<svg/>".into());
        let a = store.create(new, 5).unwrap();
        let updated = store
            .update(
                a.id,
                AccountPatch {
                    zoom: Some(9.0),
                    icon: Some(None),
                    notifications: Some(false),
                    ..Default::default()
                },
            )
            .unwrap();
        assert_eq!(updated.zoom, MAX_ZOOM);
        assert!(updated.icon.is_none());
        assert!(!updated.notifications);
    }

    #[test]
    fn ensure_default_creates_one_account() {
        let store = AccountStore::in_memory().unwrap();
        store.ensure_default().unwrap();
        store.ensure_default().unwrap();
        assert_eq!(store.count().unwrap(), 1);
    }
}
