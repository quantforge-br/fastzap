//! Local copy of an Evolution Go account's chats (SQLite in the account's
//! `evolution` directory). The server keeps no browsable history, so this is
//! the only place older messages can be read from.

use std::collections::BTreeSet;
use std::path::Path;
use std::sync::Mutex;

use rusqlite::{Connection, OptionalExtension, Row, params};
use serde::Serialize;
use serde_json::{Value, json};
use ts_rs::TS;

use super::client::ArchivedChat;
use super::model::{
    Change, Event, History, Message, MessageKind, QuoteRef, Receipt, Status, is_lid, is_phone,
};

pub type StoreResult<T> = Result<T, rusqlite::Error>;

const SCHEMA: &str = r#"
CREATE TABLE IF NOT EXISTS chats (
    jid TEXT PRIMARY KEY,
    name TEXT NOT NULL DEFAULT '',
    ts INTEGER NOT NULL DEFAULT 0,
    unread INTEGER NOT NULL DEFAULT 0,
    preview TEXT NOT NULL DEFAULT '',
    preview_kind TEXT NOT NULL DEFAULT 'text',
    preview_from_me INTEGER NOT NULL DEFAULT 0,
    archived INTEGER NOT NULL DEFAULT 0
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
-- The chat list reads the status of our newest message per chat.
CREATE INDEX IF NOT EXISTS own_messages_by_time ON messages (chat, from_me, ts, seq);
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
-- The phone names a one-to-one chat by its LID or its phone JID; chats,
-- senders and receipts are stored under the phone JID once it is known.
CREATE TABLE IF NOT EXISTS lid_map (
    lid TEXT PRIMARY KEY,
    pn TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS meta (
    key TEXT PRIMARY KEY,
    value TEXT NOT NULL
);
"#;

/// Meta key holding the linked phone number (see `ChatStore::bind`).
pub const BIND_OWNER: &str = "bind:owner";

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
    /// Delivery state of the newest message when it is ours.
    pub preview_status: Option<Status>,
    pub is_group: bool,
    pub archived: bool,
    /// The chat with yourself (notes to yourself).
    pub own: bool,
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
    /// Live messages stored now rather than already held: a journal read
    /// that finds some recovered what the socket never carried.
    pub fresh: usize,
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
        let version: i64 = conn.pragma_query_value(None, "user_version", |r| r.get(0))?;
        // Version 1 stores predate `chats.archived`.
        if version == 1 {
            conn.execute_batch("ALTER TABLE chats ADD COLUMN archived INTEGER NOT NULL DEFAULT 0")?;
        }
        // Version 2 stores kept LID chats apart from their phone chats; the
        // journal is read again to learn the mappings and merge them.
        if (1..3).contains(&version) {
            conn.execute(
                "DELETE FROM meta WHERE key = ?
                 AND EXISTS (SELECT 1 FROM chats WHERE jid LIKE '%@lid')",
                [CURSOR],
            )?;
        }
        conn.pragma_update(None, "user_version", 3)?;
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

    /// Tie the store to `value` under `key` (server and instance, or the
    /// linked phone). When it was tied to something else, every chat,
    /// message and cursor is dropped, keeping only the meta keys in `keep`.
    /// Returns whether it was cleared.
    pub fn bind(&self, key: &str, value: &str, keep: &[&str]) -> StoreResult<bool> {
        let mut conn = self.conn.lock().unwrap();
        let tx = conn.transaction()?;
        let current: Option<String> = tx
            .query_row("SELECT value FROM meta WHERE key = ?", [key], |r| r.get(0))
            .optional()?;
        let cleared = current.is_some_and(|c| c != value);
        if cleared {
            tx.execute_batch(
                "DELETE FROM reactions; DELETE FROM messages; DELETE FROM chats;
                 DELETE FROM contacts; DELETE FROM lid_map;",
            )?;
            let mut stmt = tx.prepare("SELECT key FROM meta")?;
            let keys: Vec<String> = stmt
                .query_map([], |r| r.get(0))?
                .collect::<rusqlite::Result<_>>()?;
            drop(stmt);
            for k in keys.iter().filter(|k| !keep.contains(&k.as_str())) {
                tx.execute("DELETE FROM meta WHERE key = ?", [k])?;
            }
        }
        tx.execute(
            "INSERT INTO meta (key, value) VALUES (?1, ?2)
             ON CONFLICT (key) DO UPDATE SET value = excluded.value",
            params![key, value],
        )?;
        tx.commit()?;
        Ok(cleared)
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
        apply_history(&tx, history, &mut applied)?;
        tx.commit()?;
        Ok(applied)
    }

    /// A journal page, in order, in one transaction with its end `cursor`:
    /// a receipt never lands before the message it confirms, and the cursor
    /// only moves when every event was stored.
    pub fn apply_journal(&self, events: &[Event], cursor: Option<&str>) -> StoreResult<Applied> {
        let mut conn = self.conn.lock().unwrap();
        let tx = conn.transaction()?;
        let mut applied = Applied::default();
        for event in events {
            match event {
                Event::Changes(changes) => {
                    for change in changes {
                        apply_change(&tx, change, true, &mut applied)?;
                    }
                }
                Event::Receipt(receipt) => {
                    if let Some(chat) = apply_receipt(&tx, receipt)? {
                        applied.touch(&chat);
                    }
                }
                Event::History(history) => apply_history(&tx, history, &mut applied)?,
                Event::PushName { jid, name } if !jid.is_empty() && !name.is_empty() => {
                    upsert_push_name(&tx, jid, name)?;
                }
                _ => {}
            }
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

    /// Receipts move our messages forward; a read-self receipt clears unread.
    /// Returns the chat that changed.
    pub fn apply_receipt(&self, receipt: &Receipt) -> StoreResult<Option<String>> {
        apply_receipt(&self.conn.lock().unwrap(), receipt)
    }

    /// Outbox state of a message the server has not confirmed yet; a
    /// confirmed one (sent, delivered, read) is left alone. Returns whether
    /// it changed.
    pub fn set_status(&self, chat: &str, id: &str, status: Status) -> StoreResult<bool> {
        let changed = self.conn.lock().unwrap().execute(
            "UPDATE messages SET status = ?3 WHERE chat = ?1 AND id = ?2 AND status < ?4",
            params![chat, id, status_rank(status), status_rank(Status::Sent)],
        )?;
        Ok(changed > 0)
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
            "SELECT COUNT(*) FROM chats WHERE unread > 0 AND archived = 0",
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

    /// Replace the archived set. A LID chat with a known phone JID is
    /// archived under it; archived chats without messages are created so
    /// they can still be listed. Returns whether anything changed.
    pub fn set_archived(&self, chats: &[ArchivedChat]) -> StoreResult<bool> {
        let mut conn = self.conn.lock().unwrap();
        let tx = conn.transaction()?;
        let mut merged = false;
        let mut after = BTreeSet::new();
        for chat in chats {
            if let Some(phone) = &chat.phone_jid {
                merged |= learn_lid(&tx, &chat.jid, phone)?;
            }
            after.insert(canonical(&tx, &chat.jid)?);
        }
        let before: BTreeSet<String> = tx
            .prepare("SELECT jid FROM chats WHERE archived = 1")?
            .query_map([], |r| r.get(0))?
            .collect::<StoreResult<_>>()?;
        if before != after {
            tx.execute("UPDATE chats SET archived = 0 WHERE archived = 1", [])?;
            for jid in &after {
                tx.execute(
                    "INSERT INTO chats (jid, archived) VALUES (?, 1)
                     ON CONFLICT (jid) DO UPDATE SET archived = 1",
                    [jid],
                )?;
            }
        }
        tx.commit()?;
        Ok(merged || before != after)
    }

    /// The phone JID of a known LID, else `jid` itself.
    pub fn canonical(&self, jid: &str) -> StoreResult<String> {
        canonical(&self.conn.lock().unwrap(), jid)
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
            "SELECT jid, unread, ts, preview, preview_kind, preview_from_me, archived,
                CASE WHEN preview_from_me THEN (SELECT status FROM messages
                    WHERE chat = chats.jid AND from_me = 1 ORDER BY ts DESC, seq DESC LIMIT 1)
                END
             FROM chats WHERE ts > 0 OR archived = 1 ORDER BY ts DESC",
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
                    r.get(6)?,
                    r.get::<_, Option<i64>>(7)?,
                ))
            })?
            .collect::<StoreResult<Vec<_>>>()?;
        let own_jid = own_chat(&conn)?;
        rows.into_iter()
            .map(
                |(jid, unread, ts, preview, kind, from_me, archived, status)| {
                    let own = own_jid.as_deref() == Some(jid.as_str());
                    let preview_status = status.map(|rank| shown_status(rank_status(rank), own));
                    Ok(ChatSummary {
                        name: display_name(&conn, &jid)?,
                        is_group: is_group(&jid),
                        jid,
                        ts,
                        unread,
                        preview,
                        preview_kind: MessageKind::parse(&kind),
                        preview_from_me: from_me,
                        preview_status,
                        archived,
                        own,
                    })
                },
            )
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
        let own = own_chat(&conn)?.is_some_and(|own| own == chat);
        for message in &mut list {
            if message.from_me {
                message.status = shown_status(message.status, own);
            }
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

fn apply_history(conn: &Connection, history: &History, applied: &mut Applied) -> StoreResult<()> {
    for (lid, pn) in &history.lid_map {
        if learn_lid(conn, lid, pn)? {
            applied.touch(pn);
        }
    }
    for chat in &history.chats {
        let jid = canonical(conn, &chat.jid)?;
        // An on-demand sync carries the counts of when those messages were new.
        let unread = (!history.on_demand).then_some(chat.unread);
        conn.execute(
            "INSERT INTO chats (jid, name, ts, unread) VALUES (?1, ?2, ?3, COALESCE(?4, 0))
             ON CONFLICT (jid) DO UPDATE SET
                name = CASE WHEN excluded.name <> '' THEN excluded.name ELSE chats.name END,
                ts = MAX(chats.ts, excluded.ts),
                unread = COALESCE(?4, chats.unread)",
            params![jid, chat.name, chat.ts, unread],
        )?;
        for change in &chat.messages {
            apply_change(conn, change, false, applied)?;
        }
        applied.touch(&jid);
    }
    for (jid, name) in &history.push_names {
        upsert_push_name(conn, &canonical(conn, jid)?, name)?;
    }
    Ok(())
}

/// The phone JID of a known LID, else `jid` itself.
fn canonical(conn: &Connection, jid: &str) -> StoreResult<String> {
    if !is_lid(jid) {
        return Ok(jid.to_owned());
    }
    Ok(conn
        .query_row("SELECT pn FROM lid_map WHERE lid = ?", [jid], |r| r.get(0))
        .optional()?
        .unwrap_or_else(|| jid.to_owned()))
}

/// Record that `lid` and `pn` name the same contact and move whatever was
/// stored under the LID to the phone JID. Returns whether anything changed.
fn learn_lid(conn: &Connection, lid: &str, pn: &str) -> StoreResult<bool> {
    if !is_lid(lid) || !is_phone(pn) {
        return Ok(false);
    }
    let learned = conn.execute(
        "INSERT INTO lid_map (lid, pn) VALUES (?1, ?2)
         ON CONFLICT (lid) DO UPDATE SET pn = excluded.pn WHERE lid_map.pn <> excluded.pn",
        params![lid, pn],
    )? > 0;
    if !learned {
        return Ok(false);
    }
    // A message both chats hold keeps the phone chat's copy.
    for sql in [
        "UPDATE OR IGNORE messages SET chat = ?2 WHERE chat = ?1",
        "UPDATE messages SET sender = ?2 WHERE sender = ?1",
        "UPDATE OR IGNORE reactions SET chat = ?2 WHERE chat = ?1",
        "UPDATE OR IGNORE reactions SET sender = ?2 WHERE sender = ?1",
    ] {
        conn.execute(sql, params![lid, pn])?;
    }
    for sql in [
        "DELETE FROM messages WHERE chat = ?",
        "DELETE FROM reactions WHERE chat = ?1 OR sender = ?1",
    ] {
        conn.execute(sql, [lid])?;
    }
    let merged = conn.execute(
        "INSERT INTO chats (jid, name, ts, unread, archived)
         SELECT ?2, name, ts, unread, archived FROM chats WHERE jid = ?1
         ON CONFLICT (jid) DO UPDATE SET
            name = CASE WHEN chats.name = '' THEN excluded.name ELSE chats.name END,
            ts = MAX(chats.ts, excluded.ts),
            unread = MAX(chats.unread, excluded.unread),
            archived = MAX(chats.archived, excluded.archived)",
        params![lid, pn],
    )? > 0;
    conn.execute("DELETE FROM chats WHERE jid = ?", [lid])?;
    conn.execute(
        "INSERT INTO contacts (jid, push_name)
         SELECT ?2, push_name FROM contacts WHERE jid = ?1 AND push_name <> ''
         ON CONFLICT (jid) DO UPDATE SET push_name =
            CASE WHEN contacts.push_name = '' THEN excluded.push_name ELSE contacts.push_name END",
        params![lid, pn],
    )?;
    if merged {
        refresh_preview(conn, pn)?;
    }
    Ok(true)
}

/// The stored chat a receipt is about. One-to-one receipts may name the
/// contact's LID while the messages are stored under the phone JID, so an
/// unknown direct chat is found through the message IDs.
fn receipt_chat(conn: &Connection, receipt: &Receipt) -> StoreResult<Option<String>> {
    let chat = canonical(conn, &receipt.chat)?;
    for id in &receipt.ids {
        let known: bool = conn.query_row(
            "SELECT EXISTS (SELECT 1 FROM messages WHERE chat = ?1 AND id = ?2)",
            params![chat, id],
            |r| r.get(0),
        )?;
        if known {
            return Ok(Some(chat));
        }
    }
    if is_group(&chat) {
        return Ok(None);
    }
    for id in &receipt.ids {
        let found: Option<String> = conn
            .query_row(
                "SELECT chat FROM messages WHERE id = ?1 AND chat NOT LIKE '%@g.us' LIMIT 1",
                [id],
                |r| r.get(0),
            )
            .optional()?;
        if found.is_some() {
            return Ok(found);
        }
    }
    Ok(None)
}

fn apply_receipt(conn: &Connection, receipt: &Receipt) -> StoreResult<Option<String>> {
    let Some(chat) = receipt_chat(conn, receipt)? else {
        return Ok(None);
    };
    let changed = if receipt.read_self {
        conn.execute(
            "UPDATE chats SET unread = 0 WHERE jid = ? AND unread <> 0",
            [&chat],
        )? > 0
    } else {
        let mut changed = false;
        for id in &receipt.ids {
            changed |= conn.execute(
                "UPDATE messages SET status = ?3
                 WHERE chat = ?1 AND id = ?2 AND from_me = 1 AND status < ?3",
                params![chat, id, status_rank(receipt.status)],
            )? > 0;
        }
        changed
    };
    Ok(changed.then_some(chat))
}

/// The chat with the linked number itself ("message yourself").
fn own_chat(conn: &Connection) -> StoreResult<Option<String>> {
    let owner: Option<String> = conn
        .query_row("SELECT value FROM meta WHERE key = ?", [BIND_OWNER], |r| {
            r.get(0)
        })
        .optional()?;
    Ok(owner.map(|o| format!("{o}@s.whatsapp.net")))
}

// WhatsApp sends no usable receipt for notes to yourself; they count as read
// once sent, as the phone shows them.
fn shown_status(status: Status, own: bool) -> Status {
    if own && matches!(status, Status::Sent | Status::Delivered) {
        Status::Read
    } else {
        status
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

/// `change` with its chat and sender under their phone JIDs.
fn canonical_change(conn: &Connection, change: &Change) -> StoreResult<Change> {
    let mut change = change.clone();
    match &mut change {
        Change::New(m) => {
            m.chat = canonical(conn, &m.chat)?;
            m.sender = canonical(conn, &m.sender)?;
        }
        Change::Reaction { chat, sender, .. } => {
            *chat = canonical(conn, chat)?;
            *sender = canonical(conn, sender)?;
        }
        Change::Edit { chat, .. } | Change::Revoke { chat, .. } => *chat = canonical(conn, chat)?,
    }
    Ok(change)
}

fn apply_change(
    conn: &Connection,
    change: &Change,
    live: bool,
    out: &mut Applied,
) -> StoreResult<()> {
    match &canonical_change(conn, change)? {
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
                out.fresh += 1;
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
    fn journal_pages_apply_in_order_with_their_cursor() {
        let store = ChatStore::in_memory().unwrap();
        let mut ours = message("M", 10, true, "hi");
        ours.status = Status::Sent;
        let receipt = |read_self: bool, status: Status| Receipt {
            chat: CHAT.into(),
            ids: vec!["M".into()],
            read_self,
            status,
        };
        let events = [
            Event::Changes(vec![Change::New(ours)]),
            Event::Receipt(receipt(false, Status::Read)),
            Event::Changes(vec![Change::New(message("N", 11, false, "yo"))]),
            Event::Receipt(receipt(true, Status::Read)),
        ];
        let applied = store.apply_journal(&events, Some("9")).unwrap();
        assert_eq!(applied.chats, vec![CHAT.to_owned()]);
        assert_eq!(
            store.message(CHAT, "M").unwrap().unwrap().status,
            Status::Read
        );
        assert_eq!(store.unread_chats().unwrap(), 0);
        assert_eq!(store.cursor().unwrap(), "9");
    }

    #[test]
    fn journal_reads_count_only_messages_the_socket_missed() {
        let store = ChatStore::in_memory().unwrap();
        let live = store
            .apply(&[Change::New(message("A", 10, false, "oi"))], true, None)
            .unwrap();
        assert_eq!(live.fresh, 1);
        let mut ours = message("B", 11, true, "from the phone");
        ours.status = Status::Sent;
        let history = History {
            chats: vec![HistoryChat {
                jid: CHAT.into(),
                name: String::new(),
                unread: 0,
                ts: 5,
                messages: vec![Change::New(message("H", 5, false, "old"))],
            }],
            ..Default::default()
        };
        let events = [
            Event::Changes(vec![Change::New(message("A", 10, false, "oi"))]),
            Event::Changes(vec![Change::New(ours)]),
            Event::History(history),
            Event::Changes(vec![Change::New(message("C", 12, false, "missed"))]),
        ];
        let applied = store.apply_journal(&events, Some("4")).unwrap();
        assert_eq!(applied.fresh, 2);
        let replay = store.apply_journal(&events, Some("4")).unwrap();
        assert_eq!(replay.fresh, 0);
    }

    #[test]
    fn direct_receipts_naming_the_lid_reach_the_phone_chat() {
        let store = ChatStore::in_memory().unwrap();
        let mut ours = message("M", 10, true, "hi");
        ours.status = Status::Sent;
        store.apply(&[Change::New(ours)], true, None).unwrap();
        store
            .apply(&[Change::New(message("N", 11, false, "yo"))], true, None)
            .unwrap();
        let receipt = |ids: &[&str], read_self: bool, chat: &str| Receipt {
            chat: chat.into(),
            ids: ids.iter().map(|i| (*i).into()).collect(),
            read_self,
            status: Status::Delivered,
        };
        let changed = store
            .apply_receipt(&receipt(&["M"], false, "77@lid"))
            .unwrap();
        assert_eq!(changed.as_deref(), Some(CHAT));
        assert_eq!(
            store.message(CHAT, "M").unwrap().unwrap().status,
            Status::Delivered
        );
        // Read on another device, reported under the LID.
        assert_eq!(store.unread_chats().unwrap(), 1);
        let changed = store
            .apply_receipt(&receipt(&["N"], true, "77@lid"))
            .unwrap();
        assert_eq!(changed.as_deref(), Some(CHAT));
        assert_eq!(store.unread_chats().unwrap(), 0);
        // Group receipts never match by ID alone.
        assert_eq!(
            store
                .apply_receipt(&receipt(&["M"], false, "1@g.us"))
                .unwrap(),
            None
        );
    }

    #[test]
    fn notes_to_yourself_show_as_read_once_sent() {
        let store = ChatStore::in_memory().unwrap();
        store.bind(BIND_OWNER, "5511", &[]).unwrap();
        let mut sent = message("S", 10, true, "note");
        sent.status = Status::Sent;
        let pending = message("P", 11, true, "later");
        store
            .apply(&[Change::New(sent), Change::New(pending)], true, None)
            .unwrap();
        let statuses: Vec<_> = store
            .messages(CHAT, None, 10)
            .unwrap()
            .iter()
            .map(|m| m.status)
            .collect();
        assert_eq!(statuses, [Status::Read, Status::Pending]);
        // Another chat keeps the real status.
        store.bind(BIND_OWNER, "5599", &[]).unwrap();
        let mut other = message("O", 12, true, "hi");
        other.status = Status::Sent;
        store.apply(&[Change::New(other)], true, None).unwrap();
        assert_eq!(
            store.messages(CHAT, None, 10).unwrap()[0].status,
            Status::Sent
        );
    }

    #[test]
    fn chat_list_carries_the_status_of_our_newest_message() {
        let store = ChatStore::in_memory().unwrap();
        store.bind(BIND_OWNER, "5511", &[]).unwrap();
        let mut sent = message("S", 10, true, "note");
        sent.status = Status::Delivered;
        store.apply(&[Change::New(sent)], true, None).unwrap();
        let chat = &store.chats().unwrap()[0];
        assert!(chat.own);
        assert_eq!(chat.preview_status, Some(Status::Read));
        store
            .apply(&[Change::New(message("R", 11, false, "oi"))], true, None)
            .unwrap();
        assert_eq!(store.chats().unwrap()[0].preview_status, None);
        store.bind(BIND_OWNER, "5599", &[]).unwrap();
        let mut ours = message("O", 12, true, "hi");
        ours.status = Status::Delivered;
        store.apply(&[Change::New(ours)], true, None).unwrap();
        let chat = &store.chats().unwrap()[0];
        assert!(!chat.own);
        assert_eq!(chat.preview_status, Some(Status::Delivered));
    }

    #[test]
    fn rebinding_clears_another_identity() {
        let store = ChatStore::in_memory().unwrap();
        assert!(!store.bind("bind:server", "a|1", &[]).unwrap());
        assert!(!store.bind("bind:owner", "5511", &["bind:server"]).unwrap());
        store
            .apply(
                &[Change::New(message("A", 10, false, "oi"))],
                true,
                Some("7"),
            )
            .unwrap();
        assert!(!store.bind("bind:owner", "5511", &["bind:server"]).unwrap());
        assert_eq!(store.cursor().unwrap(), "7");
        assert!(store.bind("bind:owner", "5522", &["bind:server"]).unwrap());
        assert!(store.chats().unwrap().is_empty());
        assert_eq!(store.cursor().unwrap(), "");
        assert_eq!(store.meta("bind:server").unwrap().as_deref(), Some("a|1"));
        assert!(store.bind("bind:server", "b|2", &[]).unwrap());
        assert_eq!(store.meta("bind:owner").unwrap(), None);
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
        // A late HTTP failure must not downgrade a confirmed message.
        assert!(!store.set_status(CHAT, "SERVER", Status::Unknown).unwrap());
        assert_eq!(
            store.message(CHAT, "SERVER").unwrap().unwrap().status,
            Status::Read
        );
    }

    #[test]
    fn archived_chats_are_listed_and_left_out_of_unread() {
        let store = ChatStore::in_memory().unwrap();
        store
            .apply(&[Change::New(message("A", 10, false, "oi"))], true, None)
            .unwrap();
        assert_eq!(store.unread_chats().unwrap(), 1);
        let archive = |jid: &str, phone: Option<&str>| ArchivedChat {
            jid: jid.into(),
            phone_jid: phone.map(Into::into),
        };
        // The LID resolves to the known phone chat; the group has no messages.
        let set = [archive("9@lid", Some(CHAT)), archive("1@g.us", None)];
        assert!(store.set_archived(&set).unwrap());
        assert!(!store.set_archived(&set).unwrap());
        assert_eq!(store.unread_chats().unwrap(), 0);
        let chats = store.chats().unwrap();
        let archived: Vec<_> = chats
            .iter()
            .filter(|c| c.archived)
            .map(|c| c.jid.as_str())
            .collect();
        assert_eq!(archived, [CHAT, "1@g.us"]);
        assert!(store.set_archived(&[]).unwrap());
        assert_eq!(store.chats().unwrap().len(), 1);
        assert_eq!(store.unread_chats().unwrap(), 1);
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
            ..Default::default()
        };
        let applied = store.apply_history(&history).unwrap();
        assert!(applied.incoming.is_empty());
        let chats = store.chats().unwrap();
        assert_eq!((chats[0].name.as_str(), chats[0].unread), ("Family", 3));
        let list = store.messages("120@g.us", None, 10).unwrap();
        assert_eq!(list[0].sender_name, "Bia");
        assert_eq!(store.display_name("5599@s.whatsapp.net").unwrap(), "+5599");
    }

    #[test]
    fn a_learned_lid_merges_its_chat_into_the_phone_chat() {
        const LID: &str = "77@lid";
        let store = ChatStore::in_memory().unwrap();
        let on_lid = |id: &str, ts: i64| {
            Change::New(Message {
                chat: LID.into(),
                sender: LID.into(),
                ..message(id, ts, false, "older")
            })
        };
        store
            .apply(&[Change::New(message("A", 30, false, "new"))], true, None)
            .unwrap();
        // Older messages loaded from the phone, named by the LID: a second chat.
        store
            .apply(&[on_lid("B", 10), on_lid("A", 30)], false, None)
            .unwrap();
        assert_eq!(store.chats().unwrap().len(), 2);

        let history = History {
            chats: vec![HistoryChat {
                jid: LID.into(),
                unread: 5,
                ts: 10,
                messages: vec![on_lid("C", 5)],
                ..Default::default()
            }],
            lid_map: vec![(LID.into(), CHAT.into())],
            on_demand: true,
            ..Default::default()
        };
        let applied = store.apply_history(&history).unwrap();
        assert_eq!(applied.chats, [CHAT]);
        let chats = store.chats().unwrap();
        assert_eq!(chats.len(), 1);
        assert_eq!((chats[0].jid.as_str(), chats[0].unread), (CHAT, 1));
        assert_eq!(chats[0].preview, "new");
        let ids: Vec<_> = store
            .messages(CHAT, None, 10)
            .unwrap()
            .into_iter()
            .map(|m| (m.id, m.sender))
            .collect();
        let sender = String::from(CHAT);
        assert_eq!(
            ids,
            [
                ("C".into(), sender.clone()),
                ("B".into(), sender.clone()),
                ("A".into(), sender)
            ]
        );

        // Later events naming the LID reach the phone chat.
        store.apply(&[on_lid("D", 40)], true, None).unwrap();
        assert_eq!(store.chats().unwrap().len(), 1);
        assert_eq!(store.canonical(LID).unwrap(), CHAT);
        assert_eq!(store.canonical("88@lid").unwrap(), "88@lid");
    }

    #[test]
    fn archived_lids_teach_their_phone_jid() {
        const LID: &str = "9@lid";
        let store = ChatStore::in_memory().unwrap();
        store
            .apply(
                &[Change::New(Message {
                    chat: LID.into(),
                    sender: LID.into(),
                    ..message("A", 10, false, "oi")
                })],
                false,
                None,
            )
            .unwrap();
        let set = [ArchivedChat {
            jid: LID.into(),
            phone_jid: Some(CHAT.into()),
        }];
        assert!(store.set_archived(&set).unwrap());
        let chats = store.chats().unwrap();
        assert_eq!(chats.len(), 1);
        assert!(chats[0].archived);
        assert_eq!(chats[0].jid, CHAT);
    }

    #[test]
    fn older_stores_with_lid_chats_read_the_journal_again() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("chats.db");
        {
            let conn = Connection::open(&path).unwrap();
            conn.execute_batch(SCHEMA).unwrap();
            conn.execute_batch(
                "INSERT INTO meta (key, value) VALUES ('journal_cursor', '996');
                 INSERT INTO chats (jid, ts) VALUES ('77@lid', 1);
                 PRAGMA user_version = 2;",
            )
            .unwrap();
        }
        let store = ChatStore::open(&path).unwrap();
        assert_eq!(store.cursor().unwrap(), "");
        store.set_meta(CURSOR, "997").unwrap();
        drop(store);
        assert_eq!(ChatStore::open(&path).unwrap().cursor().unwrap(), "997");
    }
}
