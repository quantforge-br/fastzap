//! Local report records: `<reports>/<id>.json`, at most 20, kept 30 days.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use serde_json::Value;
use ts_rs::TS;

pub const MAX_REPORTS: usize = 20;
pub const MAX_AGE_DAYS: i64 = 30;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub enum ReportStatus {
    Pending,
    PendingReview,
    Copied,
    OpenedOnGithub,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct ReportRecord {
    pub id: String,
    pub status: ReportStatus,
    pub created_at: String,
    #[ts(type = "Record<string, unknown>")]
    pub document: Value,
}

pub struct ReportStore {
    dir: PathBuf,
}

fn valid_id(id: &str) -> bool {
    id.len() == 32 && id.bytes().all(|b| b.is_ascii_hexdigit())
}

pub fn new_id() -> String {
    hex::encode(rand::random::<[u8; 16]>())
}

impl ReportStore {
    pub fn new(dir: PathBuf) -> Self {
        Self { dir }
    }

    pub fn dir(&self) -> &Path {
        &self.dir
    }

    fn path(&self, id: &str) -> Option<PathBuf> {
        valid_id(id).then(|| self.dir.join(format!("{id}.json")))
    }

    fn write(&self, record: &ReportRecord) -> std::io::Result<()> {
        std::fs::create_dir_all(&self.dir)?;
        let path = self.dir.join(format!("{}.json", record.id));
        let tmp = path.with_extension("json.tmp");
        std::fs::write(&tmp, serde_json::to_vec_pretty(record)?)?;
        std::fs::rename(tmp, path)
    }

    pub fn save(&self, document: Value, status: ReportStatus) -> std::io::Result<ReportRecord> {
        let record = ReportRecord {
            id: new_id(),
            status,
            created_at: super::document::now(),
            document,
        };
        self.write(&record)?;
        self.prune();
        Ok(record)
    }

    pub fn get(&self, id: &str) -> Option<ReportRecord> {
        let bytes = std::fs::read(self.path(id)?).ok()?;
        serde_json::from_slice(&bytes).ok()
    }

    pub fn set_status(&self, id: &str, status: ReportStatus) -> std::io::Result<()> {
        let Some(mut record) = self.get(id) else {
            return Err(std::io::ErrorKind::NotFound.into());
        };
        record.status = status;
        self.write(&record)
    }

    pub fn delete(&self, id: &str) -> std::io::Result<()> {
        match self.path(id) {
            Some(path) => std::fs::remove_file(path),
            None => Err(std::io::ErrorKind::InvalidInput.into()),
        }
    }

    pub fn clear(&self) {
        for record in self.read_all() {
            let _ = self.delete(&record.id);
        }
    }

    fn read_all(&self) -> Vec<ReportRecord> {
        let Ok(entries) = std::fs::read_dir(&self.dir) else {
            return Vec::new();
        };
        entries
            .flatten()
            .filter_map(|e| {
                let name = e.file_name().into_string().ok()?;
                let id = name.strip_suffix(".json")?;
                if !valid_id(id) {
                    return None;
                }
                match self.get(id) {
                    Some(record) => Some(record),
                    None => {
                        // Unreadable records are dropped, as in ZapZap.
                        let _ = std::fs::remove_file(e.path());
                        None
                    }
                }
            })
            .collect()
    }

    /// Records newest first, after dropping old and surplus ones.
    pub fn list(&self) -> Vec<ReportRecord> {
        self.prune()
    }

    fn prune(&self) -> Vec<ReportRecord> {
        let cutoff = chrono::Utc::now() - chrono::Duration::days(MAX_AGE_DAYS);
        let mut records = self.read_all();
        records.sort_by(|a, b| b.created_at.cmp(&a.created_at));
        let mut kept = Vec::new();
        for record in records {
            let fresh =
                chrono::DateTime::parse_from_rfc3339(&record.created_at).is_ok_and(|t| t >= cutoff);
            if fresh && kept.len() < MAX_REPORTS {
                kept.push(record);
            } else {
                let _ = self.delete(&record.id);
            }
        }
        kept
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn saves_lists_and_prunes() {
        let dir = tempfile::tempdir().unwrap();
        let store = ReportStore::new(dir.path().to_path_buf());
        let first = store.save(json!({"n": 0}), ReportStatus::Pending).unwrap();
        store.set_status(&first.id, ReportStatus::Copied).unwrap();
        assert_eq!(store.get(&first.id).unwrap().status, ReportStatus::Copied);

        // An old record and garbage are removed on the next listing.
        let mut old = first.clone();
        old.id = new_id();
        old.created_at = "2000-01-01T00:00:00Z".into();
        store.write(&old).unwrap();
        std::fs::write(dir.path().join(format!("{}.json", new_id())), "nope").unwrap();
        assert_eq!(store.list().len(), 1);

        for n in 0..MAX_REPORTS + 3 {
            store
                .save(json!({ "n": n }), ReportStatus::Pending)
                .unwrap();
        }
        assert_eq!(store.list().len(), MAX_REPORTS);
        assert!(store.get("../../etc/passwd").is_none());
        store.clear();
        assert!(store.list().is_empty());
    }
}
