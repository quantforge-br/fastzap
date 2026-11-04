//! Evolution Go event payloads (whatsmeow types serialized with Go's
//! `encoding/json`) mapped to the chat domain. Unknown events and message
//! types are kept as `Other`/`Unsupported` instead of failing.

use chrono::DateTime;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use ts_rs::TS;

/// whatsmeow `ProtocolMessage` types.
const PROTOCOL_REVOKE: i64 = 0;
const PROTOCOL_EDIT: i64 = 14;
/// Fields that carry media bytes or credentials, dropped before storing.
const HEAVY_FIELDS: &[&str] = &["JPEGThumbnail", "jpegThumbnail", "base64", "mediaUrl"];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub enum MessageKind {
    Text,
    Image,
    Video,
    Audio,
    Document,
    Sticker,
    Location,
    Contact,
    Unsupported,
}

impl MessageKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Text => "text",
            Self::Image => "image",
            Self::Video => "video",
            Self::Audio => "audio",
            Self::Document => "document",
            Self::Sticker => "sticker",
            Self::Location => "location",
            Self::Contact => "contact",
            Self::Unsupported => "unsupported",
        }
    }

    pub fn parse(value: &str) -> Self {
        match value {
            "text" => Self::Text,
            "image" => Self::Image,
            "video" => Self::Video,
            "audio" => Self::Audio,
            "document" => Self::Document,
            "sticker" => Self::Sticker,
            "location" => Self::Location,
            "contact" => Self::Contact,
            _ => Self::Unsupported,
        }
    }

    pub fn has_media(self) -> bool {
        matches!(
            self,
            Self::Image | Self::Video | Self::Audio | Self::Document | Self::Sticker
        )
    }
}

/// Delivery state of our own messages, ordered so it only moves forward.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub enum Status {
    /// In the outbox, not answered yet.
    Pending,
    /// The send timed out; it may or may not have gone out.
    Unknown,
    Failed,
    Sent,
    Delivered,
    Read,
}

impl Status {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Unknown => "unknown",
            Self::Failed => "failed",
            Self::Sent => "sent",
            Self::Delivered => "delivered",
            Self::Read => "read",
        }
    }

    pub fn parse(value: &str) -> Self {
        match value {
            "pending" => Self::Pending,
            "unknown" => Self::Unknown,
            "failed" => Self::Failed,
            "delivered" => Self::Delivered,
            "read" => Self::Read,
            _ => Self::Sent,
        }
    }

    /// whatsmeow `WebMessageInfo.status` (history).
    fn from_web(status: i64) -> Self {
        match status {
            3 => Self::Delivered,
            4 | 5 => Self::Read,
            _ => Self::Sent,
        }
    }
}

/// The message a reply quotes.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct QuoteRef {
    pub id: String,
    pub participant: String,
    /// Preview text when the event carried the quoted message.
    pub text: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Message {
    pub id: String,
    pub chat: String,
    pub sender: String,
    pub from_me: bool,
    /// Unix seconds.
    pub ts: i64,
    pub kind: MessageKind,
    /// Text, or the caption of media.
    pub body: String,
    pub file_name: String,
    pub mime: String,
    pub push_name: String,
    pub quote: Option<QuoteRef>,
    pub status: Status,
    /// The proto message without thumbnails, for `/message/downloadmedia`.
    pub raw: Value,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Change {
    New(Message),
    /// An empty emoji removes the sender's reaction.
    Reaction {
        chat: String,
        target: String,
        sender: String,
        emoji: String,
    },
    Edit {
        chat: String,
        target: String,
        body: String,
    },
    Revoke {
        chat: String,
        target: String,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub struct Receipt {
    pub chat: String,
    pub ids: Vec<String>,
    /// Our own read receipt from another device.
    pub read_self: bool,
    pub status: Status,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct HistoryChat {
    pub jid: String,
    pub name: String,
    pub unread: i64,
    pub ts: i64,
    pub messages: Vec<Change>,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct History {
    pub chats: Vec<HistoryChat>,
    pub push_names: Vec<(String, String)>,
    /// `(LID, phone JID)` pairs: the phone names a chat by either.
    pub lid_map: Vec<(String, String)>,
    /// Older messages asked for by the user; the unread counts are stale.
    pub on_demand: bool,
    pub progress: Option<i64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub enum Link {
    Connected,
    Disconnected,
    LoggedOut,
    Banned,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Event {
    Changes(Vec<Change>),
    Receipt(Receipt),
    /// `composing`, `recording` or `paused` in a chat.
    Typing {
        chat: String,
        sender: String,
        state: String,
    },
    Presence {
        jid: String,
        online: bool,
    },
    History(History),
    Qr {
        image: String,
        code: String,
    },
    /// Pairing state changed (pair success, passkey stages).
    Pairing,
    Link(Link),
    PushName {
        jid: String,
        name: String,
    },
    Picture {
        jid: String,
    },
    Other,
}

fn str_at<'a>(value: &'a Value, key: &str) -> &'a str {
    value.get(key).and_then(Value::as_str).unwrap_or("")
}

fn bool_at(value: &Value, key: &str) -> bool {
    value.get(key).and_then(Value::as_bool).unwrap_or(false)
}

fn int_at(value: &Value, key: &str) -> i64 {
    match value.get(key) {
        Some(Value::Number(n)) => n.as_i64().unwrap_or(0),
        Some(Value::String(s)) => s.parse().unwrap_or(0),
        _ => 0,
    }
}

fn rfc3339(value: &str) -> i64 {
    DateTime::parse_from_rfc3339(value)
        .map(|t| t.timestamp())
        .unwrap_or(0)
}

/// Drop thumbnails and inline media recursively.
pub fn strip_heavy(value: &mut Value) {
    match value {
        Value::Object(map) => {
            for field in HEAVY_FIELDS {
                map.remove(*field);
            }
            map.values_mut().for_each(strip_heavy);
        }
        Value::Array(items) => items.iter_mut().for_each(strip_heavy),
        _ => {}
    }
}

/// Remove whatsmeow wrappers that hold the real message in `.message`.
fn unwrap(message: &Value) -> &Value {
    const WRAPPERS: &[&str] = &[
        "ephemeralMessage",
        "viewOnceMessage",
        "viewOnceMessageV2",
        "viewOnceMessageV2Extension",
        "documentWithCaptionMessage",
        "editedMessage",
        "deviceSentMessage",
    ];
    let mut current = message;
    for _ in 0..4 {
        let inner = WRAPPERS
            .iter()
            .find_map(|w| current.get(*w).and_then(|v| v.get("message")));
        match inner {
            Some(inner) => current = inner,
            None => break,
        }
    }
    current
}

/// Text shown for a message proto, used for quotes and edits.
pub fn preview_text(message: &Value) -> String {
    let message = unwrap(message);
    if let Some(text) = message.get("conversation").and_then(Value::as_str) {
        return text.to_owned();
    }
    for (field, key) in [
        ("extendedTextMessage", "text"),
        ("imageMessage", "caption"),
        ("videoMessage", "caption"),
        ("documentMessage", "caption"),
    ] {
        let text = message
            .get(field)
            .and_then(|m| m.get(key))
            .and_then(Value::as_str)
            .unwrap_or("");
        if !text.is_empty() {
            return text.to_owned();
        }
    }
    String::new()
}

struct Envelope<'a> {
    id: &'a str,
    chat: &'a str,
    sender: &'a str,
    from_me: bool,
    ts: i64,
    push_name: &'a str,
    status: Status,
}

fn parse_body(env: Envelope<'_>, message: &Value) -> Option<Change> {
    let message = unwrap(message);
    let chat = env.chat.to_owned();
    if let Some(protocol) = message.get("protocolMessage") {
        let target = protocol
            .get("key")
            .map(|k| str_at(k, "ID"))
            .unwrap_or("")
            .to_owned();
        return match int_at(protocol, "type") {
            PROTOCOL_REVOKE if !target.is_empty() => Some(Change::Revoke { chat, target }),
            PROTOCOL_EDIT if !target.is_empty() => Some(Change::Edit {
                chat,
                target,
                body: protocol
                    .get("editedMessage")
                    .map(preview_text)
                    .unwrap_or_default(),
            }),
            // History sync notices, key shares, ephemeral settings, …
            _ => None,
        };
    }
    if let Some(reaction) = message.get("reactionMessage") {
        let key = reaction.get("key").cloned().unwrap_or_default();
        let target = str_at(&key, "ID").to_owned();
        return (!target.is_empty()).then(|| Change::Reaction {
            chat,
            target,
            sender: if env.from_me {
                String::new()
            } else {
                env.sender.to_owned()
            },
            emoji: str_at(reaction, "text").to_owned(),
        });
    }
    if message.get("senderKeyDistributionMessage").is_some() && message.as_object()?.len() == 1 {
        return None;
    }

    let (kind, inner) = [
        ("imageMessage", MessageKind::Image),
        ("videoMessage", MessageKind::Video),
        ("audioMessage", MessageKind::Audio),
        ("documentMessage", MessageKind::Document),
        ("stickerMessage", MessageKind::Sticker),
        ("locationMessage", MessageKind::Location),
        ("liveLocationMessage", MessageKind::Location),
        ("contactMessage", MessageKind::Contact),
        ("contactsArrayMessage", MessageKind::Contact),
        ("extendedTextMessage", MessageKind::Text),
    ]
    .into_iter()
    .find_map(|(field, kind)| message.get(field).map(|inner| (kind, inner)))
    .unwrap_or_else(|| {
        if message.get("conversation").is_some() {
            (MessageKind::Text, &Value::Null)
        } else {
            (MessageKind::Unsupported, &Value::Null)
        }
    });

    let body = match kind {
        MessageKind::Location => {
            let name = str_at(inner, "name");
            if name.is_empty() {
                format!(
                    "{}, {}",
                    inner.get("degreesLatitude").unwrap_or(&Value::Null),
                    inner.get("degreesLongitude").unwrap_or(&Value::Null)
                )
            } else {
                name.to_owned()
            }
        }
        MessageKind::Contact => str_at(inner, "displayName").to_owned(),
        _ => preview_text(message),
    };
    let quote = inner
        .get("contextInfo")
        .filter(|ctx| !str_at(ctx, "stanzaID").is_empty())
        .map(|ctx| QuoteRef {
            id: str_at(ctx, "stanzaID").to_owned(),
            participant: str_at(ctx, "participant").to_owned(),
            text: ctx
                .get("quotedMessage")
                .map(preview_text)
                .unwrap_or_default(),
        });
    let mut raw = message.clone();
    strip_heavy(&mut raw);
    Some(Change::New(Message {
        id: env.id.to_owned(),
        chat,
        sender: env.sender.to_owned(),
        from_me: env.from_me,
        ts: env.ts,
        kind,
        body,
        file_name: str_at(inner, "fileName").to_owned(),
        mime: str_at(inner, "mimetype").to_owned(),
        push_name: env.push_name.to_owned(),
        quote,
        status: env.status,
        raw,
    }))
}

/// A live `Message`/`SendMessage` event (`data.Info` + `data.Message`).
fn parse_live_message(data: &Value) -> Option<Change> {
    let info = data.get("Info")?;
    let id = str_at(info, "ID");
    let chat = str_at(info, "Chat");
    if id.is_empty() || chat.is_empty() || chat == "status@broadcast" {
        return None;
    }
    let from_me = bool_at(info, "IsFromMe");
    parse_body(
        Envelope {
            id,
            chat,
            sender: str_at(info, "Sender"),
            from_me,
            ts: rfc3339(str_at(info, "Timestamp")),
            push_name: str_at(info, "PushName"),
            status: Status::Sent,
        },
        data.get("Message")?,
    )
}

/// `HistorySync.syncType` of a sync the user asked for.
const ON_DEMAND_SYNC: i64 = 6;

pub fn is_lid(jid: &str) -> bool {
    jid.len() > 4 && jid.ends_with("@lid")
}

pub fn is_phone(jid: &str) -> bool {
    jid.len() > 15 && jid.ends_with("@s.whatsapp.net")
}

/// One `WebMessageInfo` of a HistorySync conversation.
fn parse_web_message(chat: &str, item: &Value) -> Option<Change> {
    let info = item.get("message")?;
    let key = info.get("key")?;
    let from_me = bool_at(key, "fromMe");
    let participant = match str_at(key, "participant") {
        "" => str_at(info, "participant"),
        p => p,
    };
    let sender = if participant.is_empty() && !from_me {
        chat
    } else {
        participant
    };
    parse_body(
        Envelope {
            id: str_at(key, "ID"),
            chat,
            sender,
            from_me,
            ts: int_at(info, "messageTimestamp"),
            push_name: str_at(info, "pushName"),
            status: Status::from_web(int_at(info, "status")),
        },
        info.get("message")?,
    )
}

fn parse_history(data: &Value) -> History {
    let sync = data.get("Data").unwrap_or(data);
    let chats = sync
        .get("conversations")
        .and_then(Value::as_array)
        .map(|list| {
            list.iter()
                .filter_map(|conv| {
                    let jid = str_at(conv, "ID");
                    if jid.is_empty() || jid == "status@broadcast" {
                        return None;
                    }
                    let messages = conv
                        .get("messages")
                        .and_then(Value::as_array)
                        .map(|m| m.iter().filter_map(|i| parse_web_message(jid, i)).collect())
                        .unwrap_or_default();
                    Some(HistoryChat {
                        jid: jid.to_owned(),
                        name: str_at(conv, "name").to_owned(),
                        unread: int_at(conv, "unreadCount"),
                        ts: int_at(conv, "conversationTimestamp"),
                        messages,
                    })
                })
                .collect()
        })
        .unwrap_or_default();
    let push_names = sync
        .get("pushnames")
        .and_then(Value::as_array)
        .map(|list| {
            list.iter()
                .filter_map(|p| {
                    let (jid, name) = (str_at(p, "ID"), str_at(p, "pushname"));
                    (!jid.is_empty() && !name.is_empty()).then(|| (jid.to_owned(), name.to_owned()))
                })
                .collect()
        })
        .unwrap_or_default();
    let mut lid_map: Vec<(String, String)> = sync
        .get("phoneNumberToLidMappings")
        .and_then(Value::as_array)
        .map(|list| {
            list.iter()
                .map(|m| {
                    (
                        str_at(m, "lidJID").to_owned(),
                        str_at(m, "pnJID").to_owned(),
                    )
                })
                .collect()
        })
        .unwrap_or_default();
    if let Some(list) = sync.get("conversations").and_then(Value::as_array) {
        for conv in list {
            let (lid, pn) = match str_at(conv, "ID") {
                id if id.ends_with("@lid") => (id, str_at(conv, "pnJID")),
                id => (str_at(conv, "lidJID"), id),
            };
            lid_map.push((lid.to_owned(), pn.to_owned()));
        }
    }
    lid_map.retain(|(lid, pn)| is_lid(lid) && is_phone(pn));
    History {
        chats,
        push_names,
        lid_map,
        on_demand: int_at(sync, "syncType") == ON_DEMAND_SYNC,
        progress: sync.get("progress").and_then(Value::as_i64),
    }
}

/// Parse `{event, data, state?}`; the caller already checked the instance.
pub fn parse(payload: &Value) -> Event {
    let data = payload.get("data").unwrap_or(&Value::Null);
    let state = str_at(payload, "state");
    match str_at(payload, "event") {
        "Message" | "SendMessage" => Event::Changes(parse_live_message(data).into_iter().collect()),
        "Receipt" => {
            let ids = data
                .get("MessageIDs")
                .and_then(Value::as_array)
                .map(|ids| {
                    ids.iter()
                        .filter_map(Value::as_str)
                        .map(str::to_owned)
                        .collect()
                })
                .unwrap_or_default();
            let kind = match state {
                "" => str_at(data, "Type"),
                s => s,
            };
            let (status, read_self) = match kind {
                "Read" | "read" => (Status::Read, false),
                "ReadSelf" | "read-self" => (Status::Read, true),
                "Delivered" | "" => (Status::Delivered, false),
                _ => return Event::Other,
            };
            Event::Receipt(Receipt {
                chat: str_at(data, "Chat").to_owned(),
                ids,
                read_self,
                status,
            })
        }
        "ChatPresence" => Event::Typing {
            chat: str_at(data, "Chat").to_owned(),
            sender: str_at(data, "Sender").to_owned(),
            state: match (str_at(data, "State"), str_at(data, "Media")) {
                ("composing", "audio") => "recording".into(),
                (s, _) => s.to_owned(),
            },
        },
        "Presence" => Event::Presence {
            jid: str_at(data, "From").to_owned(),
            online: state == "online",
        },
        "HistorySync" => Event::History(parse_history(data)),
        "QRCode" => Event::Qr {
            image: str_at(data, "qrcode").to_owned(),
            code: str_at(data, "code").to_owned(),
        },
        "PairSuccess"
        | "QRSuccess"
        | "QRTimeout"
        | "PasskeyRequest"
        | "PasskeyConfirmation"
        | "PasskeyError" => Event::Pairing,
        "Connected" => Event::Link(Link::Connected),
        "Disconnected" | "ConnectFailure" | "StreamReplaced" => Event::Link(Link::Disconnected),
        "LoggedOut" => Event::Link(Link::LoggedOut),
        "TemporaryBan" => Event::Link(Link::Banned),
        "PushName" => Event::PushName {
            jid: str_at(data, "JID").to_owned(),
            name: str_at(data, "NewPushName").to_owned(),
        },
        "Picture" => Event::Picture {
            jid: str_at(data, "JID").to_owned(),
        },
        _ => Event::Other,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn live(message: Value) -> Value {
        json!({"event": "Message", "data": {
            "Info": {"ID": "M1", "Chat": "551@s.whatsapp.net", "Sender": "551@s.whatsapp.net",
                     "IsFromMe": false, "PushName": "Ana", "Timestamp": "2026-01-02T03:04:05Z"},
            "Message": message
        }})
    }

    fn single(event: Event) -> Change {
        match event {
            Event::Changes(mut changes) if changes.len() == 1 => changes.remove(0),
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn text_and_replies() {
        let Change::New(m) = single(parse(&live(json!({"conversation": "oi"})))) else {
            panic!()
        };
        assert_eq!(
            (m.kind, m.body.as_str(), m.ts),
            (MessageKind::Text, "oi", 1767323045)
        );
        assert_eq!(m.push_name, "Ana");

        let reply = live(
            json!({"extendedTextMessage": {"text": "yes", "contextInfo": {
            "stanzaID": "Q1", "participant": "552@s.whatsapp.net",
            "quotedMessage": {"conversation": "question"}}}}),
        );
        let Change::New(m) = single(parse(&reply)) else {
            panic!()
        };
        assert_eq!(
            m.quote,
            Some(QuoteRef {
                id: "Q1".into(),
                participant: "552@s.whatsapp.net".into(),
                text: "question".into()
            })
        );
    }

    #[test]
    fn media_is_unwrapped_and_stripped() {
        let event = live(json!({"ephemeralMessage": {"message": {"imageMessage": {
            "caption": "pic", "mimetype": "image/jpeg", "mediaKey": "a2V5",
            "JPEGThumbnail": "big"}}}}));
        let Change::New(m) = single(parse(&event)) else {
            panic!()
        };
        assert_eq!(m.kind, MessageKind::Image);
        assert_eq!(m.body, "pic");
        assert_eq!(m.raw["imageMessage"]["mediaKey"], "a2V5");
        assert!(m.raw["imageMessage"].get("JPEGThumbnail").is_none());
    }

    #[test]
    fn reactions_edits_and_revokes() {
        let reaction = live(json!({"reactionMessage": {"key": {"ID": "T1"}, "text": "👍"}}));
        assert!(matches!(
            single(parse(&reaction)),
            Change::Reaction { ref target, ref emoji, .. } if target == "T1" && emoji == "👍"
        ));
        let edit = live(json!({"protocolMessage": {"type": 14, "key": {"ID": "T1"},
            "editedMessage": {"conversation": "fixed"}}}));
        assert_eq!(
            single(parse(&edit)),
            Change::Edit {
                chat: "551@s.whatsapp.net".into(),
                target: "T1".into(),
                body: "fixed".into()
            }
        );
        let revoke = live(json!({"protocolMessage": {"type": 0, "key": {"ID": "T1"}}}));
        assert!(matches!(single(parse(&revoke)), Change::Revoke { .. }));
        let notice = live(json!({"protocolMessage": {"type": 5}}));
        assert_eq!(parse(&notice), Event::Changes(vec![]));
    }

    #[test]
    fn receipts_use_the_top_level_state() {
        let event = json!({"event": "Receipt", "state": "Read", "data": {
            "Chat": "551@s.whatsapp.net", "MessageIDs": ["A", "B"], "Type": "read"}});
        assert_eq!(
            parse(&event),
            Event::Receipt(Receipt {
                chat: "551@s.whatsapp.net".into(),
                ids: vec!["A".into(), "B".into()],
                read_self: false,
                status: Status::Read
            })
        );
        let unknown = json!({"event": "Receipt", "data": {"Type": "retry"}});
        assert_eq!(parse(&unknown), Event::Other);
    }

    #[test]
    fn history_sync_conversations() {
        let event = json!({"event": "HistorySync", "data": {"Data": {
            "syncType": 2, "progress": 40,
            "pushnames": [{"ID": "552@s.whatsapp.net", "pushname": "Bia"}],
            "conversations": [{
                "ID": "120@g.us", "name": "Group", "unreadCount": 2, "conversationTimestamp": 1700000000,
                "messages": [{"message": {
                    "key": {"remoteJID": "120@g.us", "fromMe": false, "ID": "H1", "participant": "552@s.whatsapp.net"},
                    "message": {"conversation": "old"}, "messageTimestamp": 1700000000, "status": 4}}]
            }, {"ID": "status@broadcast", "messages": []}]
        }}});
        let Event::History(history) = parse(&event) else {
            panic!()
        };
        assert_eq!(history.progress, Some(40));
        assert_eq!(history.chats.len(), 1);
        let chat = &history.chats[0];
        assert_eq!((chat.name.as_str(), chat.unread), ("Group", 2));
        let Change::New(m) = &chat.messages[0] else {
            panic!()
        };
        assert_eq!(
            (m.sender.as_str(), m.status),
            ("552@s.whatsapp.net", Status::Read)
        );
        assert_eq!(
            history.push_names,
            vec![("552@s.whatsapp.net".into(), "Bia".into())]
        );
    }

    #[test]
    fn history_sync_maps_lids_to_phone_numbers() {
        let event = json!({"event": "HistorySync", "data": {"Data": {
            "syncType": 6,
            "phoneNumberToLidMappings": [
                {"lidJID": "77@lid", "pnJID": "5511999990000@s.whatsapp.net"},
                {"lidJID": "", "pnJID": "5511999990001@s.whatsapp.net"}],
            "conversations": [
                {"ID": "88@lid", "pnJID": "5511999990002@s.whatsapp.net", "messages": []},
                {"ID": "5511999990003@s.whatsapp.net", "lidJID": "99@lid", "messages": []},
                {"ID": "120@g.us", "messages": []}]
        }}});
        let Event::History(history) = parse(&event) else {
            panic!()
        };
        assert!(history.on_demand);
        assert_eq!(
            history.lid_map,
            vec![
                ("77@lid".into(), "5511999990000@s.whatsapp.net".into()),
                ("88@lid".into(), "5511999990002@s.whatsapp.net".into()),
                ("99@lid".into(), "5511999990003@s.whatsapp.net".into()),
            ]
        );
    }

    #[test]
    fn connection_and_unknown_events() {
        assert_eq!(
            parse(&json!({"event": "LoggedOut", "data": {}})),
            Event::Link(Link::LoggedOut)
        );
        assert_eq!(
            parse(&json!({"event": "CallOffer", "data": {}})),
            Event::Other
        );
        assert!(matches!(
            parse(
                &json!({"event": "QRCode", "data": {"qrcode": "data:image/png;base64,x", "code": "c"}})
            ),
            Event::Qr { .. }
        ));
    }
}
