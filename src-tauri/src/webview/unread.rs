//! Unread counters per account, extracted from page titles or native chats.

use std::collections::BTreeMap;
use std::sync::Mutex;

use serde::Serialize;
use ts_rs::TS;

/// ZapZap reads every digit of the title: `"(12) WhatsApp"` → 12.
pub fn count_from_title(title: &str) -> u32 {
    let digits: String = title.chars().filter(char::is_ascii_digit).collect();
    digits.parse().unwrap_or(0)
}

#[derive(Debug, Clone, Default, Serialize, TS)]
#[ts(export)]
pub struct UnreadSnapshot {
    #[ts(type = "Record<string, number>")]
    pub per_account: BTreeMap<i64, u32>,
    pub total: u32,
}

#[derive(Default)]
pub struct UnreadRegistry(Mutex<BTreeMap<i64, u32>>);

impl UnreadRegistry {
    /// Store a count and return the new snapshot when it changed.
    pub fn set(&self, account_id: i64, count: u32) -> Option<UnreadSnapshot> {
        let mut map = self.0.lock().unwrap();
        if map.get(&account_id).copied().unwrap_or(0) == count {
            return None;
        }
        if count == 0 {
            map.remove(&account_id);
        } else {
            map.insert(account_id, count);
        }
        Some(snapshot(&map))
    }

    pub fn remove(&self, account_id: i64) -> UnreadSnapshot {
        let mut map = self.0.lock().unwrap();
        map.remove(&account_id);
        snapshot(&map)
    }

    pub fn snapshot(&self) -> UnreadSnapshot {
        snapshot(&self.0.lock().unwrap())
    }
}

fn snapshot(map: &BTreeMap<i64, u32>) -> UnreadSnapshot {
    UnreadSnapshot {
        per_account: map.clone(),
        total: map.values().copied().sum(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn title_digits_become_the_count() {
        assert_eq!(count_from_title("(12) WhatsApp"), 12);
        assert_eq!(count_from_title("WhatsApp"), 0);
    }

    #[test]
    fn registry_reports_only_changes() {
        let reg = UnreadRegistry::default();
        assert_eq!(reg.set(1, 3).unwrap().total, 3);
        assert!(reg.set(1, 3).is_none());
        assert_eq!(reg.set(2, 2).unwrap().total, 5);
        assert_eq!(reg.set(1, 0).unwrap().total, 2);
        assert_eq!(reg.remove(2).total, 0);
    }
}
