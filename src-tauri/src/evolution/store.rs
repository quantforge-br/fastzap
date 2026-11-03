//! Local copy of an Evolution Go account's chats (SQLite in the account's
//! `evolution` directory). The server keeps no browsable history, so this is
//! the only place older messages can be read from.

use std::path::Path;
use std::sync::Mutex;

use rusqlite::{Connection, OptionalExtension, Row, params};
use serde::Serialize;
use serde_json::{Value, json};
use ts_rs::TS;

use super::model::{Change, History, Message, MessageKind, QuoteRef, Receipt, Status};

pub type StoreResult<T> = Result<T, rusqlite::Error>;

const SCHEMA: &str = r#"
CREATE TABLE IF NOT EXISTS chats (
    jid TEXT PRIMARY KEY,
    name TEXT NOT NULL DEFAULT '',
    ts INTEGER NOT NULL DEFAULT 0,
    unread INTEGER NOT NULL DEFAULT 0,
    preview TEXT NOT NULL DEFAULT '',
    preview_kind TEXT NOT NULL DEFAULT 'text',
    preview_from_me INTEGER NOT NULL DEFAULT 0
);
CREATE TABLE IF NOT EXISTS messages (
    seq INTEGER PRIMARY KEY AUTOINCREMENT,
    chat TEXT NOT NULL,
    id TEXT NOT NULL,
    sender TEXT NOT NULL DEFAULT '',
    from_me INTEGER NOT NULL DEFAULT 0,
    ts INTEGER NOT NULL DEFAULT 0,
    kind TEXT NOT NULL,
    body TEXT NOT NULL DEFAULT '',
    file_name TEXT NOT NULL DEFAULT '',
    mime TEXT NOT NULL DEFAULT '',
    push_name TEXT NOT NULL DEFAULT '',
    quote_id TEXT NOT NULL DEFAULT '',
    quote_participant TEXT NOT NULL DEFAULT '',
    quote_text TEXT NOT NULL DEFAULT '',
    status INTEGER NOT NULL DEFAULT 3,
    edited INTEGER NOT NULL DEFAULT 0,
    revoked INTEGER NOT NULL DEFAULT 0,
    raw TEXT NOT NULL DEFAULT 'null',
    media_file TEXT NOT NULL DEFAULT '',
    UNIQUE (chat, id)
);
CREATE INDEX IF NOT EXISTS messages_by_time ON messages (chat, ts, seq);
CREATE TABLE IF NOT EXISTS reactions (
    chat TEXT NOT NULL,
    target TEXT NOT NULL,
    sender TEXT NOT NULL,
    emoji TEXT NOT NULL,
    PRIMARY KEY (chat, target, sender)
);
CREATE TABLE IF NOT EXISTS contacts (
    jid TEXT PRIMARY KEY,
    name TEXT NOT NULL DEFAULT '',
    push_name TEXT NOT NULL DEFAULT ''
);
CREATE TABLE IF NOT EXISTS meta (
    key TEXT PRIMARY KEY,
    value TEXT NOT NULL
);
"#;

/// Journal position of `GET /chat/events`.
const CURSOR: &str = "journal_cursor";

fn status_rank(status: Status) -> i64 {
    match status {
        Status::Pending => 0,
        Status::Unknown => 1,
        Status::Failed => 2,
        Status::Sent => 3,
        Status::Delivered => 4,
        Status::Read => 5,
    }
}

fn rank_status(rank: i64) -> Status {
    match rank {
        0 => Status::Pending,
        1 => Status::Unknown,
        2 => Status::Failed,
        4 => Status::Delivered,
        5 => Status::Read,
        _ => Status::Sent,
    }
}

#[derive(Debug, Clone, Serialize, TS)]
#[ts(export)]
pub struct ChatSummary {
    pub jid: String,
    pub name: String,
    #[ts(type = "number")]
    pub ts: i64,
    #[ts(type = "number")]
    pub unread: i64,
    pub preview: String,
    pub preview_kind: MessageKind,
    pub preview_from_me: bool,
    pub is_group: bool,
}

#[derive(Debug, Clone, Serialize, TS)]
#[ts(export)]
pub struct ReactionCount {
    pub emoji: String,
    #[ts(type = "number")]
    pub count: i64,
    pub mine: bool,
}

#[derive(Debug, Clone, Serialize, TS)]
#[ts(export)]
pub struct ChatMessage {
    /// Page cursor.
    #[ts(type = "number")]
    pub seq: i64,
    pub id: String,
    pub chat: String,
    pub sender: String,
    pub sender_name: String,
    pub from_me: bool,
    #[ts(type = "number")]
    pub ts: i64,
    pub kind: MessageKind,
    pub body: String,
    pub file_name: String,
    pub mime: String,
    pub quote: Option<QuoteRef>,
    pub status: Status,
    pub edited: bool,
    pub revoked: bool,
    pub reactions: Vec<ReactionCount>,
}

/// What an applied change means for the UI and notifications.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Applied {
    /// Chats whose message list changed.
    pub chats: Vec<String>,
    /// New incoming messages (not from history, not ours).
    pub incoming: Vec<Message>,
}

impl Applied {
    fn touch(&mut self, chat: &str) {
        if !self.chats.iter().any(|c| c == chat) {
            self.chats.push(chat.to_owned());
        }
    }
}

pub fn is_group(jid: &str) -> bool {
    jid.ends_with("@g.us")
}

/// A message id in whatsmeow's format, for sends and the outbox.
pub fn new_message_id() -> String {
    let bytes: [u8; 8] = rand::random();
    format!("3EB0{}", hex::encode_upper(bytes))
}

pub struct ChatStore {
    conn: Mutex<Connection>,
}

impl ChatStore {
    pub fn open(path: &Path) -> StoreResult<Self> {
        if let Some(dir) = path.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        Self::with_connection(Connection::open(path)?)
    }

    pub fn in_memory() -> StoreResult<Self> {
        Self::with_connection(Connection::open_in_memory()?)
    }

    fn with_connection(conn: Connection) -> StoreResult<Self> {
        conn.pragma_update(None, "journal_mode", "WAL")?;
        conn.execute_batch(SCHEMA)?;
        conn.pragma_update(None, "user_version", 1)?;
        Ok(Self {
            conn: Mutex::new(conn),
        })
    }

    pub fn cursor(&self) -> StoreResult<String> {
        self.meta(CURSOR).map(Option::unwrap_or_default)
    }

    pub fn meta(&self, key: &str) -> StoreResult<Option<String>> {
        self.conn
            .lock()
            .unwrap()
            .query_row("SELECT value FROM meta WHERE key = ?", [key], |r| r.get(0))
            .optional()
    }

    pub fn set_meta(&self, key: &str, value: &str) -> StoreResult<()> {
        self.conn.lock().unwrap().execute(
            "INSERT INTO meta (key, value) VALUES (?1, ?2)
             ON CONFLICT (key) DO UPDATE SET value = excluded.value",
            params![key, value],
        )?;
        Ok(())
    }

    /// Apply live changes; `cursor` (a journal page's end) is saved in the
    /// same transaction so a crash never skips events.
    pub fn apply(
        &self,
        changes: &[Change],
        live: bool,
        cursor: Option<&str>,
    ) -> StoreResult<Applied> {
        let mut conn = self.conn.lock().unwrap();
        let tx = conn.transaction()?;
        let mut applied = Applied::default();
        for change in changes {
            apply_change(&tx, change, live, &mut applied)?;
        }
        if let Some(cursor) = cursor {
            tx.execute(
                "INSERT INTO meta (key, value) VALUES (?1, ?2)
                 ON CONFLICT (key) DO UPDATE SET value = excluded.value",
                params![CURSOR, cursor],
            )?;
        }
        tx.commit()?;
        Ok(applied)
    }

    /// History sync: messages, chat names, unread counts and push names.
    pub fn apply_history(&self, history: &History) -> StoreResult<Applied> {
        let mut conn = self.conn.lock().unwrap();
        let tx = conn.transaction()?;
        let mut applied = Applied::default();
        for chat in &history.chats {
            tx.execute(
                "INSERT INTO chats (jid, name, ts, unread) VALUES (?1, ?2, ?3, ?4)
                 ON CONFLICT (jid) DO UPDATE SET
                    name = CASE WHEN excluded.name <> '' THEN excluded.name ELSE chats.name END,
                    ts = MAX(chats.ts, excluded.ts),
                    unread = excluded.unread",
                params![chat.jid, chat.name, chat.ts, chat.unread],
            )?;
            for change in &chat.messages {
                apply_change(&tx, change, false, &mut applied)?;
            }
            applied.touch(&chat.jid);
        }
        for (jid, name) in &history.push_names {
            upsert_push_name(&tx, jid, name)?;
        }
        tx.commit()?;
        Ok(applied)
    }

    /// Receipts move our messages forward; a read-self receipt clears unread.
    pub fn apply_receipt(&self, receipt: &Receipt) -> StoreResult<bool> {
        let conn = self.conn.lock().unwrap();
        if receipt.read_self {
            let n = conn.execute(
                "UPDATE chats SET unread = 0 WHERE jid = ? AND unread <> 0",
                [&receipt.chat],
            )?;
            return Ok(n > 0);
        }
        let mut changed = false;
        for id in &receipt.ids {
            changed |= conn.execute(
                "UPDATE messages SET status = ?3
                 WHERE chat = ?1 AND id = ?2 AND from_me = 1 AND status < ?3",
                params![receipt.chat, id, status_rank(receipt.status)],
            )? > 0;
        }
        Ok(changed)
    }

    pub fn set_status(&self, chat: &str, id: &str, status: Status) -> StoreResult<()> {
        self.conn.lock().unwrap().execute(
            "UPDATE messages SET status = ?3 WHERE chat = ?1 AND id = ?2",
            params![chat, id, status_rank(status)],
        )?;
        Ok(())
    }

    /// Replace a pending message's local id with the server's.
    pub fn confirm_sent(&self, chat: &str, local: &str, server: &str) -> StoreResult<()> {
        let conn = self.conn.lock().unwrap();
        if local != server {
            // The live SendMessage event may already have stored it.
            let exists: bool = conn.query_row(
                "SELECT EXISTS (SELECT 1 FROM messages WHERE chat = ?1 AND id = ?2)",
                params![chat, server],
                |r| r.get(0),
            )?;
            if exists {
                conn.execute(
                    "DELETE FROM messages WHERE chat = ?1 AND id = ?2",
                    params![chat, local],
                )?;
                return Ok(());
            }
            conn.execute(
                "UPDATE messages SET id = ?3 WHERE chat = ?1 AND id = ?2",
                params![chat, local, server],
            )?;
        }
        conn.execute(
            "UPDATE messages SET status = MAX(status, ?3) WHERE chat = ?1 AND id = ?2",
            params![chat, server, status_rank(Status::Sent)],
        )?;
        Ok(())
    }

    pub fn mark_read(&self, chat: &str) -> StoreResult<Vec<String>> {
        let conn = self.conn.lock().unwrap();
        conn.execute("UPDATE chats SET unread = 0 WHERE jid = ?", [chat])?;
        // The newest incoming messages are enough for the read receipt.
        let mut stmt = conn.prepare(
            "SELECT id FROM messages WHERE chat = ? AND from_me = 0
             ORDER BY ts DESC, seq DESC LIMIT 20",
        )?;
        let ids = stmt.query_map([chat], |r| r.get(0))?;
        ids.collect()
    }

    /// Chats with unread messages (the WhatsApp Web title count).
    pub fn unread_chats(&self) -> StoreResult<i64> {
        self.conn.lock().unwrap().query_row(
            "SELECT COUNT(*) FROM chats WHERE unread > 0",
            [],
            |r| r.get(0),
        )
    }

    pub fn set_contacts(&self, contacts: &[(String, String)]) -> StoreResult<()> {
        let mut conn = self.conn.lock().unwrap();
        let tx = conn.transaction()?;
        for (jid, name) in contacts {
            tx.execute(
                "INSERT INTO contacts (jid, name) VALUES (?1, ?2)
                 ON CONFLICT (jid) DO UPDATE SET name = excluded.name",
                params![jid, name],
            )?;
        }
        tx.commit()
    }

    pub fn set_push_name(&self, jid: &str, name: &str) -> StoreResult<()> {
        upsert_push_name(&self.conn.lock().unwrap(), jid, name)
    }

    /// Group subjects only rename group chats.
    pub fn set_chat_names(&self, names: &[(String, String)]) -> StoreResult<()> {
        let mut conn = self.conn.lock().unwrap();
        let tx = conn.transaction()?;
        for (jid, name) in names {
            tx.execute(
                "INSERT INTO chats (jid, name) VALUES (?1, ?2)
                 ON CONFLICT (jid) DO UPDATE SET name = excluded.name",
                params![jid, name],
            )?;
        }
        tx.commit()
    }

    /// Contact, push or chat name, else the phone number.
    pub fn display_name(&self, jid: &str) -> StoreResult<String> {
        display_name(&self.conn.lock().unwrap(), jid)
    }

    pub fn chats(&self) -> StoreResult<Vec<ChatSummary>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT jid, unread, ts, preview, preview_kind, preview_from_me FROM chats
             WHERE ts > 0 ORDER BY ts DESC",
        )?;
        let rows = stmt
            .query_map([], |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get(1)?,
                    r.get(2)?,
                    r.get(3)?,
                    r.get::<_, String>(4)?,
                    r.get(5)?,
                ))
            })?
            .collect::<StoreResult<Vec<_>>>()?;
        rows.into_iter()
            .map(|(jid, unread, ts, preview, kind, from_me)| {
                Ok(ChatSummary {
                    name: display_name(&conn, &jid)?,
                    is_group: is_group(&jid),
                    jid,
                    ts,
                    unread,
                    preview,
                    preview_kind: MessageKind::parse(&kind),
                    preview_from_me: from_me,
                })
            })
            .collect()
    }

    /// The newest `limit` messages before `before` (a `seq`), oldest first.
    pub fn messages(
        &self,
        chat: &str,
        before: Option<i64>,
        limit: u32,
    ) -> StoreResult<Vec<ChatMessage>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT * FROM messages WHERE chat = ?1 AND (?2 IS NULL OR
                (ts, seq) < (SELECT ts, seq FROM messages WHERE seq = ?2))
             ORDER BY ts DESC, seq DESC LIMIT ?3",
        )?;
        let mut list = stmt
            .query_map(params![chat, before, limit], row_to_message)?
            .collect::<StoreResult<Vec<_>>>()?;
        list.reverse();
        for message in &mut list {
            if !message.from_me {
                message.sender_name = display_name(&conn, &message.sender)?;
            }
            message.reactions = reactions(&conn, chat, &message.id)?;
        }
        Ok(list)
    }

    pub fn message(&self, chat: &str, id: &str) -> StoreResult<Option<ChatMessage>> {
        let conn = self.conn.lock().unwrap();
        conn.query_row(
            "SELECT * FROM messages WHERE chat = ?1 AND id = ?2",
            params![chat, id],
            row_to_message,
        )
        .optional()
    }

    /// The proto message (for `/message/downloadmedia`) and cached file.
    pub fn media(&self, chat: &str, id: &str) -> StoreResult<Option<(Value, String)>> {
        let conn = self.conn.lock().unwrap();
        conn.query_row(
            "SELECT raw, media_file FROM messages WHERE chat = ?1 AND id = ?2",
            params![chat, id],
            |r| {
                let raw: String = r.get(0)?;
                Ok((serde_json::from_str(&raw).unwrap_or(Value::Null), r.get(1)?))
            },
        )
        .optional()
    }

    pub fn set_media_file(&self, chat: &str, id: &str, file: &str) -> StoreResult<()> {
        self.conn.lock().unwrap().execute(
            "UPDATE messages SET media_file = ?3 WHERE chat = ?1 AND id = ?2",
            params![chat, id, file],
        )?;
        Ok(())
    }

    /// whatsmeow `MessageInfo` of the oldest stored message, the anchor of
    /// `POST /chat/history-sync`.
    pub fn oldest_anchor(&self, chat: &str) -> StoreResult<Option<Value>> {
        let conn = self.conn.lock().unwrap();
        conn.query_row(
            "SELECT id, sender, from_me, ts FROM messages WHERE chat = ?
             AND status >= 3 ORDER BY ts ASC, seq ASC LIMIT 1",
            [chat],
            |r| {
                let (id, sender, from_me, ts): (String, String, bool, i64) =
                    (r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?);
                let time = chrono::DateTime::from_timestamp(ts, 0)
                    .unwrap_or_default()
                    .to_rfc3339();
                Ok(json!({
                    "Chat": chat,
                    "Sender": sender,
                    "IsFromMe": from_me,
                    "IsGroup": is_group(chat),
                    "ID": id,
                    "Timestamp": time,
                }))
            },
        )
        .optional()
    }
}

fn upsert_push_name(conn: &Connection, jid: &str, name: &str) -> StoreResult<()> {
    conn.execute(
        "INSERT INTO contacts (jid, push_name) VALUES (?1, ?2)
         ON CONFLICT (jid) DO UPDATE SET push_name = excluded.push_name",
        params![jid, name],
    )?;
    Ok(())
}

fn display_name(conn: &Connection, jid: &str) -> StoreResult<String> {
    let found: Option<String> = conn
        .query_row(
            "SELECT COALESCE(
                NULLIF((SELECT name FROM contacts WHERE jid = ?1), ''),
                NULLIF((SELECT name FROM chats WHERE jid = ?1), ''),
                NULLIF((SELECT push_name FROM contacts WHERE jid = ?1), ''))",
            [jid],
            |r| r.get(0),
        )
        .optional()?
        .flatten();
    Ok(found.unwrap_or_else(|| {
        let user = jid.split(['@', ':']).next().unwrap_or(jid);
        if is_group(jid) || !user.chars().all(|c| c.is_ascii_digit()) {
            user.to_owned()
        } else {
            format!("+{user}")
        }
    }))
}

fn reactions(conn: &Connection, chat: &str, target: &str) -> StoreResult<Vec<ReactionCount>> {
    let mut stmt = conn.prepare(
        "SELECT emoji, COUNT(*), MAX(sender = '') FROM reactions
         WHERE chat = ?1 AND target = ?2 GROUP BY emoji ORDER BY COUNT(*) DESC",
    )?;
    let rows = stmt.query_map(params![chat, target], |r| {
        Ok(ReactionCount {
            emoji: r.get(0)?,
            count: r.get(1)?,
            mine: r.get(2)?,
        })
    })?;
    rows.collect()
}

fn row_to_message(r: &Row<'_>) -> rusqlite::Result<ChatMessage> {
    let quote_id: String = r.get("quote_id")?;
    Ok(ChatMessage {
        seq: r.get("seq")?,
        id: r.get("id")?,
        chat: r.get("chat")?,
        sender: r.get("sender")?,
        sender_name: String::new(),
        from_me: r.get("from_me")?,
        ts: r.get("ts")?,
        kind: MessageKind::parse(&r.get::<_, String>("kind")?),
        body: r.get("body")?,
        file_name: r.get("file_name")?,
        mime: r.get("mime")?,
        quote: (!quote_id.is_empty())
            .then(|| -> rusqlite::Result<QuoteRef> {
                Ok(QuoteRef {
                    id: quote_id,
                    participant: r.get("quote_participant")?,
                    text: r.get("quote_text")?,
                })
            })
            .transpose()?,
        status: rank_status(r.get("status")?),
        edited: r.get("edited")?,
        revoked: r.get("revoked")?,
        reactions: Vec::new(),
    })
}

/// Recompute a chat's preview from its newest message.
fn refresh_preview(conn: &Connection, chat: &str) -> StoreResult<()> {
    conn.execute(
        "UPDATE chats SET (preview, preview_kind, preview_from_me) = (
            SELECT CASE WHEN revoked = 1 THEN '' ELSE body END, kind, from_me
            FROM messages WHERE chat = ?1 ORDER BY ts DESC, seq DESC LIMIT 1)
         WHERE jid = ?1 AND EXISTS (SELECT 1 FROM messages WHERE chat = ?1)",
        [chat],
    )?;
    Ok(())
}

fn apply_change(
    conn: &Connection,
    change: &Change,
    live: bool,
    out: &mut Applied,
) -> StoreResult<()> {
    match change {
        Change::New(m) => {
            let quote = m.quote.clone().unwrap_or_default();
            let inserted = conn.execute(
                "INSERT INTO messages (chat, id, sender, from_me, ts, kind, body, file_name,
                    mime, push_name, quote_id, quote_participant, quote_text, status, raw)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15)
                 ON CONFLICT (chat, id) DO NOTHING",
                params![
                    m.chat,
                    m.id,
                    m.sender,
                    m.from_me,
                    m.ts,
                    m.kind.as_str(),
                    m.body,
                    m.file_name,
                    m.mime,
                    m.push_name,
                    quote.id,
                    quote.participant,
                    quote.text,
                    status_rank(m.status),
                    m.raw.to_string(),
                ],
            )? > 0;
            if !inserted {
                // Our own pending send echoed back by the server.
                conn.execute(
                    "UPDATE messages SET status = MAX(status, ?3) WHERE chat = ?1 AND id = ?2",
                    params![m.chat, m.id, status_rank(m.status)],
                )?;
                return Ok(());
            }
            conn.execute(
                "INSERT INTO chats (jid, ts) VALUES (?1, ?2)
                 ON CONFLICT (jid) DO UPDATE SET ts = MAX(chats.ts, excluded.ts)",
                params![m.chat, m.ts],
            )?;
            if !m.push_name.is_empty() && !m.from_me && !m.sender.is_empty() {
                upsert_push_name(conn, &m.sender, &m.push_name)?;
            }
            if live {
                if m.from_me {
                    // Answered from another device.
                    conn.execute("UPDATE chats SET unread = 0 WHERE jid = ?", [&m.chat])?;
                } else {
                    conn.execute(
                        "UPDATE chats SET unread = unread + 1 WHERE jid = ?",
                        [&m.chat],
                    )?;
                    out.incoming.push(m.clone());
                }
            }
            refresh_preview(conn, &m.chat)?;
            out.touch(&m.chat);
        }
        Change::Reaction {
            chat,
            target,
            sender,
            emoji,
        } => {
            if emoji.is_empty() {
                conn.execute(
                    "DELETE FROM reactions WHERE chat = ?1 AND target = ?2 AND sender = ?3",
                    params![chat, target, sender],
                )?;
            } else {
                conn.execute(
                    "INSERT INTO reactions (chat, target, sender, emoji) VALUES (?1, ?2, ?3, ?4)
                     ON CONFLICT (chat, target, sender) DO UPDATE SET emoji = excluded.emoji",
                    params![chat, target, sender, emoji],
                )?;
            }
            out.touch(chat);
        }
        Change::Edit { chat, target, body } => {
            conn.execute(
                "UPDATE messages SET body = ?3, edited = 1 WHERE chat = ?1 AND id = ?2",
                params![chat, target, body],
            )?;
            refresh_preview(conn, chat)?;
            out.touch(chat);
        }
        Change::Revoke { chat, target } => {
            conn.execute(
                "UPDATE messages SET body = '', revoked = 1, raw = 'null'
                 WHERE chat = ?1 AND id = ?2",
                params![chat, target],
            )?;
            conn.execute(
                "DELETE FROM reactions WHERE chat = ?1 AND target = ?2",
                params![chat, target],
            )?;
            refresh_preview(conn, chat)?;
            out.touch(chat);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::evolution::model::HistoryChat;

    const CHAT: &str = "5511@s.whatsapp.net";

    fn message(id: &str, ts: i64, from_me: bool, body: &str) -> Message {
        Message {
            id: id.into(),
            chat: CHAT.into(),
            sender: if from_me { String::new() } else { CHAT.into() },
            from_me,
            ts,
            kind: MessageKind::Text,
            body: body.into(),
            file_name: String::new(),
            mime: String::new(),
            push_name: "Ana".into(),
            quote: None,
            status: if from_me {
                Status::Pending
            } else {
                Status::Sent
            },
            raw: json!({"conversation": body}),
        }
    }

    #[test]
    fn duplicates_are_ignored_and_unread_counts_live_messages() {
        let store = ChatStore::in_memory().unwrap();
        let first = store
            .apply(
                &[Change::New(message("A", 10, false, "oi"))],
                true,
                Some("5"),
            )
            .unwrap();
        assert_eq!(first.incoming.len(), 1);
        let replay = store
            .apply(
                &[Change::New(message("A", 10, false, "oi"))],
                true,
                Some("6"),
            )
            .unwrap();
        assert!(replay.incoming.is_empty());
        assert_eq!(store.cursor().unwrap(), "6");
        assert_eq!(store.unread_chats().unwrap(), 1);
        let chats = store.chats().unwrap();
        assert_eq!(chats[0].name, "Ana");
        assert_eq!(chats[0].preview, "oi");
        assert_eq!(store.mark_read(CHAT).unwrap(), vec!["A".to_owned()]);
        assert_eq!(store.unread_chats().unwrap(), 0);
    }

    #[test]
    fn outbox_messages_follow_the_server() {
        let store = ChatStore::in_memory().unwrap();
        store
            .apply(
                &[Change::New(message("LOCAL", 10, true, "hi"))],
                false,
                None,
            )
            .unwrap();
        store.confirm_sent(CHAT, "LOCAL", "SERVER").unwrap();
        let echo = Message {
            status: Status::Sent,
            ..message("SERVER", 10, true, "hi")
        };
        store.apply(&[Change::New(echo)], true, None).unwrap();
        store
            .apply_receipt(&Receipt {
                chat: CHAT.into(),
                ids: vec!["SERVER".into()],
                read_self: false,
                status: Status::Read,
            })
            .unwrap();
        store
            .apply_receipt(&Receipt {
                chat: CHAT.into(),
                ids: vec!["SERVER".into()],
                read_self: false,
                status: Status::Delivered,
            })
            .unwrap();
        let list = store.messages(CHAT, None, 50).unwrap();
        assert_eq!(list.len(), 1);
        assert_eq!(
            (list[0].id.as_str(), list[0].status),
            ("SERVER", Status::Read)
        );
    }

    #[test]
    fn reactions_edits_revokes_and_pages() {
        let store = ChatStore::in_memory().unwrap();
        let changes: Vec<Change> = (0..5)
            .map(|i| Change::New(message(&format!("M{i}"), 100 + i, false, "x")))
            .collect();
        store.apply(&changes, false, None).unwrap();
        store
            .apply(
                &[
                    Change::Reaction {
                        chat: CHAT.into(),
                        target: "M4".into(),
                        sender: String::new(),
                        emoji: "❤".into(),
                    },
                    Change::Edit {
                        chat: CHAT.into(),
                        target: "M4".into(),
                        body: "edited".into(),
                    },
                    Change::Revoke {
                        chat: CHAT.into(),
                        target: "M3".into(),
                    },
                ],
                true,
                None,
            )
            .unwrap();
        let page = store.messages(CHAT, None, 2).unwrap();
        assert_eq!(
            page.iter().map(|m| m.id.as_str()).collect::<Vec<_>>(),
            ["M3", "M4"]
        );
        assert!(page[0].revoked && page[0].body.is_empty());
        assert!(page[1].edited && page[1].reactions[0].mine);
        assert_eq!(store.chats().unwrap()[0].preview, "edited");
        let older = store.messages(CHAT, Some(page[0].seq), 10).unwrap();
        assert_eq!(older.len(), 3);
        let anchor = store.oldest_anchor(CHAT).unwrap().unwrap();
        assert_eq!(anchor["ID"], "M0");
        assert_eq!(anchor["IsGroup"], false);
    }

    #[test]
    fn history_sets_names_and_unread() {
        let store = ChatStore::in_memory().unwrap();
        let history = History {
            chats: vec![HistoryChat {
                jid: "120@g.us".into(),
                name: "Family".into(),
                unread: 3,
                ts: 50,
                messages: vec![Change::New(Message {
                    chat: "120@g.us".into(),
                    sender: "552@s.whatsapp.net".into(),
                    ..message("H", 50, false, "old")
                })],
            }],
            push_names: vec![("552@s.whatsapp.net".into(), "Bia".into())],
            progress: None,
        };
        let applied = store.apply_history(&history).unwrap();
        assert!(applied.incoming.is_empty());
        let chats = store.chats().unwrap();
        assert_eq!((chats[0].name.as_str(), chats[0].unread), ("Family", 3));
        let list = store.messages("120@g.us", None, 10).unwrap();
        assert_eq!(list[0].sender_name, "Bia");
        assert_eq!(store.display_name("5599@s.whatsapp.net").unwrap(), "+5599");
    }
}
