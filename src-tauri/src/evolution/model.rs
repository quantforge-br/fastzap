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
    Poll,
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
            Self::Poll => "poll",
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
            "poll" => Self::Poll,
            _ => Self::Unsupported,
        }
    }

    /// Field of a media message proto, as `/message/downloadmedia` takes it.
    pub fn media_field(self) -> Option<&'static str> {
        match self {
            Self::Image => Some("imageMessage"),
            Self::Video => Some("videoMessage"),
            Self::Audio => Some("audioMessage"),
            Self::Document => Some("documentMessage"),
            Self::Sticker => Some("stickerMessage"),
            _ => None,
        }
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

/// The message a reply quotes, as the reply carried it.
#[derive(Debug, Clone, PartialEq)]
pub struct QuoteRef {
    pub id: String,
    pub participant: String,
    /// Kind of the quoted message (`Text` when the reply did not carry it).
    pub kind: MessageKind,
    /// Text, caption or document name of the quoted message, when carried.
    pub text: String,
}

/// A button, list row or poll option shown under a message. Only links act;
/// answering the rest needs the phone.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct MessageButton {
    pub text: String,
    /// An `http(s)` link the button opens; empty for a label.
    pub url: String,
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
    /// Text, the caption of media, the text of a business message or the
    /// question of a poll.
    pub body: String,
    pub file_name: String,
    pub mime: String,
    pub buttons: Vec<MessageButton>,
    pub push_name: String,
    pub quote: Option<QuoteRef>,
    pub status: Status,
    /// The proto message without thumbnails, for `/message/downloadmedia`.
    pub raw: Value,
}

// Nearly every change is a new message, so boxing it would only add an
// allocation per message.
#[allow(clippy::large_enum_variant)]
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
        // A photo or video of an album.
        "associatedChildMessage",
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

/// Message fields that hold their content directly.
const PLAIN: &[(&str, MessageKind)] = &[
    ("imageMessage", MessageKind::Image),
    ("videoMessage", MessageKind::Video),
    // A round video note.
    ("ptvMessage", MessageKind::Video),
    ("audioMessage", MessageKind::Audio),
    ("documentMessage", MessageKind::Document),
    ("stickerMessage", MessageKind::Sticker),
    ("locationMessage", MessageKind::Location),
    ("liveLocationMessage", MessageKind::Location),
    ("contactMessage", MessageKind::Contact),
    ("contactsArrayMessage", MessageKind::Contact),
    ("extendedTextMessage", MessageKind::Text),
];

/// Polls, as each WhatsApp version creates them.
const POLLS: &[&str] = &[
    "pollCreationMessage",
    "pollCreationMessageV2",
    "pollCreationMessageV3",
];

/// Answers to business buttons and lists, shown as the chosen text:
/// `(field, object holding the text or "" for the field itself, key)`.
const ANSWERS: &[(&str, &str, &str)] = &[
    ("buttonsResponseMessage", "Response", "SelectedDisplayText"),
    ("listResponseMessage", "", "title"),
    ("templateButtonReplyMessage", "", "selectedDisplayText"),
    ("interactiveResponseMessage", "body", "text"),
];

/// What an unwrapped message proto shows.
struct Content<'a> {
    kind: MessageKind,
    /// Text, caption, place, contact name, business text or poll question.
    body: String,
    /// The media proto, in the field `kind.media_field()` names.
    media: Option<&'a Value>,
    /// The object whose `contextInfo` names the message it answers.
    context: &'a Value,
    buttons: Vec<MessageButton>,
    /// An album's opening message: it only announces the photos and videos
    /// that follow as messages of their own.
    album: bool,
}

impl<'a> Content<'a> {
    fn new(kind: MessageKind, body: impl Into<String>, context: &'a Value) -> Self {
        Self {
            kind,
            body: body.into(),
            media: None,
            context,
            buttons: Vec::new(),
            album: false,
        }
    }

    fn file_name(&self) -> &'a str {
        self.media.map_or("", |m| str_at(m, "fileName"))
    }

    fn mime(&self) -> &'a str {
        self.media.map_or("", |m| str_at(m, "mimetype"))
    }
}

fn items<'a>(value: &'a Value, key: &str) -> impl Iterator<Item = &'a Value> + use<'a> {
    value
        .get(key)
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
}

/// `value.<field>.text`.
fn text_in<'a>(value: &'a Value, field: &str) -> &'a str {
    value.get(field).map_or("", |v| str_at(v, "text"))
}

/// Title, text and footer of a business message, as paragraphs.
fn paragraphs(parts: &[&str]) -> String {
    parts
        .iter()
        .map(|p| p.trim())
        .filter(|p| !p.is_empty())
        .collect::<Vec<_>>()
        .join("\n\n")
}

/// A button labeled `text` that opens `url` when it is a web link.
fn button(text: &str, url: &str) -> Option<MessageButton> {
    let text = text.trim();
    let scheme = url.split_once("://").map(|(s, _)| s.to_ascii_lowercase());
    let web = matches!(scheme.as_deref(), Some("http" | "https"));
    (!text.is_empty()).then(|| MessageButton {
        text: text.to_owned(),
        url: if web { url.to_owned() } else { String::new() },
    })
}

/// A `HydratedButton`, a Go oneof: `UrlButton`, `QuickReplyButton` or
/// `CallButton`.
fn hydrated_button(item: &Value) -> Option<MessageButton> {
    let (kind, inner) = item.get("HydratedButton")?.as_object()?.iter().next()?;
    let url = if kind == "UrlButton" {
        str_at(inner, "URL")
    } else {
        ""
    };
    button(str_at(inner, "displayText"), url)
}

/// A native flow button; its label and link are in `buttonParamsJSON`.
fn flow_button(item: &Value) -> Option<MessageButton> {
    let params: Value = serde_json::from_str(str_at(item, "buttonParamsJSON")).ok()?;
    let label = match str_at(&params, "display_text") {
        "" => str_at(&params, "flow_cta"),
        text => text,
    };
    match str_at(&params, "copy_code") {
        "" => button(label, str_at(&params, "url")),
        code => button(&format!("{label}: {code}"), ""),
    }
}

/// A business message whose kind and media come from its header, a Go
/// oneof whose media fields are capitalized.
fn with_header<'a>(
    header: &'a Value,
    body: String,
    buttons: Vec<MessageButton>,
    context: &'a Value,
) -> Content<'a> {
    let media = [
        ("ImageMessage", MessageKind::Image),
        ("VideoMessage", MessageKind::Video),
        ("DocumentMessage", MessageKind::Document),
    ]
    .into_iter()
    .find_map(|(field, kind)| {
        header
            .get(field)
            .filter(|m| m.is_object())
            .map(|m| (kind, m))
    });
    Content {
        kind: media.map_or(MessageKind::Text, |(kind, _)| kind),
        media: media.map(|(_, m)| m),
        buttons,
        ..Content::new(MessageKind::Text, body, context)
    }
}

/// An `InteractiveMessage`, alone or inside a template.
fn interactive(message: &Value) -> Content<'_> {
    let header = message.get("header").unwrap_or(&Value::Null);
    let body = paragraphs(&[
        str_at(header, "title"),
        text_in(message, "body"),
        text_in(message, "footer"),
    ]);
    let buttons = message
        .get("InteractiveMessage")
        .and_then(|m| m.get("NativeFlowMessage"))
        .map(|flow| items(flow, "buttons").filter_map(flow_button).collect())
        .unwrap_or_default();
    with_header(
        header.get("Media").unwrap_or(&Value::Null),
        body,
        buttons,
        message,
    )
}

/// Read an unwrapped message proto.
fn content(message: &Value) -> Content<'_> {
    if let Some((kind, inner)) = PLAIN
        .iter()
        .find_map(|&(field, kind)| message.get(field).map(|inner| (kind, inner)))
    {
        let body = match kind {
            MessageKind::Location => match str_at(inner, "name") {
                "" => format!(
                    "{}, {}",
                    inner.get("degreesLatitude").unwrap_or(&Value::Null),
                    inner.get("degreesLongitude").unwrap_or(&Value::Null)
                ),
                name => name.to_owned(),
            },
            MessageKind::Contact => str_at(inner, "displayName").to_owned(),
            MessageKind::Text => str_at(inner, "text").to_owned(),
            _ => str_at(inner, "caption").to_owned(),
        };
        return Content {
            media: kind.media_field().map(|_| inner),
            ..Content::new(kind, body, inner)
        };
    }
    if let Some(text) = message.get("conversation").and_then(Value::as_str) {
        return Content::new(MessageKind::Text, text, &Value::Null);
    }
    if let Some(template) = message.get("templateMessage") {
        let format = template.get("Format").unwrap_or(&Value::Null);
        let rows = template
            .get("hydratedTemplate")
            .filter(|t| t.is_object())
            .or_else(|| format.get("HydratedFourRowTemplate"));
        if let Some(rows) = rows {
            let title = rows.get("Title").unwrap_or(&Value::Null);
            let body = paragraphs(&[
                str_at(title, "HydratedTitleText"),
                str_at(rows, "hydratedContentText"),
                str_at(rows, "hydratedFooterText"),
            ]);
            let buttons = items(rows, "hydratedButtons")
                .filter_map(hydrated_button)
                .collect();
            return with_header(title, body, buttons, template);
        }
        if let Some(inner) = format.get("InteractiveMessageTemplate") {
            return interactive(inner);
        }
    }
    if let Some(inner) = message.get("interactiveMessage") {
        return interactive(inner);
    }
    if let Some(inner) = message.get("buttonsMessage") {
        let header = inner.get("Header").unwrap_or(&Value::Null);
        let body = paragraphs(&[
            str_at(header, "Text"),
            str_at(inner, "contentText"),
            str_at(inner, "footerText"),
        ]);
        let buttons = items(inner, "buttons")
            .filter_map(|b| button(str_at(b.get("buttonText")?, "displayText"), ""))
            .collect();
        return with_header(header, body, buttons, inner);
    }
    if let Some(inner) = message.get("listMessage") {
        let body = paragraphs(&[
            str_at(inner, "title"),
            str_at(inner, "description"),
            str_at(inner, "footerText"),
        ]);
        return Content {
            buttons: items(inner, "sections")
                .flat_map(|s| items(s, "rows"))
                .filter_map(|r| button(str_at(r, "title"), ""))
                .collect(),
            ..Content::new(MessageKind::Text, body, inner)
        };
    }
    if let Some(poll) = POLLS.iter().find_map(|f| message.get(*f)) {
        return Content {
            buttons: items(poll, "options")
                .filter_map(|o| button(str_at(o, "optionName"), ""))
                .collect(),
            ..Content::new(MessageKind::Poll, str_at(poll, "name"), poll)
        };
    }
    for &(field, holder, key) in ANSWERS {
        if let Some(inner) = message.get(field) {
            let holder = match holder {
                "" => inner,
                holder => inner.get(holder).unwrap_or(&Value::Null),
            };
            return Content::new(MessageKind::Text, str_at(holder, key), inner);
        }
    }
    if let Some(album) = message.get("albumMessage") {
        // Quoted as the media it holds.
        let kind = if int_at(album, "expectedImageCount") > 0 {
            MessageKind::Image
        } else {
            MessageKind::Video
        };
        return Content {
            album: true,
            ..Content::new(kind, "", album)
        };
    }
    Content::new(MessageKind::Unsupported, "", &Value::Null)
}

/// The quote of a message, from the `contextInfo` of its content.
fn quote_in(context: &Value) -> Option<QuoteRef> {
    let ctx = context
        .get("contextInfo")
        .filter(|ctx| !str_at(ctx, "stanzaID").is_empty())?;
    let (kind, text) = match ctx.get("quotedMessage") {
        Some(quoted) => {
            let quoted = content(unwrap(quoted));
            let text = if quoted.body.is_empty() && quoted.kind == MessageKind::Document {
                quoted.file_name().to_owned()
            } else {
                quoted.body
            };
            (quoted.kind, text)
        }
        None => (MessageKind::Text, String::new()),
    };
    Some(QuoteRef {
        id: str_at(ctx, "stanzaID").to_owned(),
        participant: str_at(ctx, "participant").to_owned(),
        kind,
        text,
    })
}

/// The quote of a stored message proto (`Message::raw`).
pub fn quote_of_raw(raw: &Value) -> Option<QuoteRef> {
    quote_in(content(unwrap(raw)).context)
}

/// What a stored message proto (`Message::raw`) shows, read again.
pub struct Shown {
    pub kind: MessageKind,
    pub body: String,
    pub file_name: String,
    pub mime: String,
    pub buttons: Vec<MessageButton>,
    /// An album's opening message, which is not kept as a message.
    pub album: bool,
}

pub fn shown(raw: &Value) -> Shown {
    let content = content(unwrap(raw));
    Shown {
        file_name: content.file_name().to_owned(),
        mime: content.mime().to_owned(),
        kind: content.kind,
        body: content.body,
        buttons: content.buttons,
        album: content.album,
    }
}

/// The media of a stored message proto as `/message/downloadmedia` takes
/// it (`{"imageMessage": …}`), when the server can fetch it: whatsmeow needs
/// a direct path, which media WhatsApp serves from a static URL (its own
/// announcements) lacks.
pub fn media_message(raw: &Value) -> Option<Value> {
    let content = content(unwrap(raw));
    let media = content
        .media
        .filter(|m| !str_at(m, "directPath").is_empty())?;
    let mut message = serde_json::Map::new();
    message.insert(content.kind.media_field()?.to_owned(), media.clone());
    Some(Value::Object(message))
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
                    .map(|edited| content(unwrap(edited)).body)
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

    let content = content(message);
    if content.album {
        return None;
    }
    let quote = quote_in(content.context);
    let (file_name, mime) = (content.file_name().to_owned(), content.mime().to_owned());
    let mut raw = message.clone();
    strip_heavy(&mut raw);
    Some(Change::New(Message {
        id: env.id.to_owned(),
        chat,
        sender: env.sender.to_owned(),
        from_me: env.from_me,
        ts: env.ts,
        kind: content.kind,
        body: content.body,
        file_name,
        mime,
        buttons: content.buttons,
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
    jid.strip_suffix("@lid")
        .is_some_and(|user| !user.is_empty())
}

pub fn is_phone(jid: &str) -> bool {
    jid.strip_suffix("@s.whatsapp.net")
        .is_some_and(|number| !number.is_empty())
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
    // One pair per LID; the explicit mappings come first and win.
    let mut seen = std::collections::HashSet::new();
    lid_map.retain(|(lid, pn)| is_lid(lid) && is_phone(pn) && seen.insert(lid.clone()));
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
                kind: MessageKind::Text,
                text: "question".into()
            })
        );

        // Media without a caption is quoted by kind; a document by its name.
        for (quoted, kind, text) in [
            (
                json!({"imageMessage": {"mimetype": "image/jpeg"}}),
                MessageKind::Image,
                "",
            ),
            (
                json!({"documentWithCaptionMessage": {"message": {"documentMessage":
                    {"fileName": "a.pdf"}}}}),
                MessageKind::Document,
                "a.pdf",
            ),
            (json!({"stickerMessage": {}}), MessageKind::Sticker, ""),
        ] {
            let reply = live(
                json!({"extendedTextMessage": {"text": "yes", "contextInfo": {
                "stanzaID": "Q2", "quotedMessage": quoted}}}),
            );
            let Change::New(m) = single(parse(&reply)) else {
                panic!()
            };
            let quote = m.quote.unwrap();
            assert_eq!((quote.kind, quote.text.as_str()), (kind, text));
            assert_eq!(quote_of_raw(&m.raw).unwrap(), quote);
        }
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

    fn new_message(message: Value) -> Message {
        match single(parse(&live(message))) {
            Change::New(m) => m,
            other => panic!("{other:?}"),
        }
    }

    fn buttons(m: &Message) -> Vec<(&str, &str)> {
        m.buttons
            .iter()
            .map(|b| (b.text.as_str(), b.url.as_str()))
            .collect()
    }

    #[test]
    fn business_messages_show_their_text_media_and_buttons() {
        // WhatsApp's own announcements: the video only has a static URL.
        let flow = |name: &str, params: Value| json!({"name": name, "buttonParamsJSON": params.to_string()});
        let m = new_message(json!({"interactiveMessage": {
            "header": {"hasMediaAttachment": true, "Media": {"VideoMessage": {
                "mimetype": "video/mp4", "staticURL": "https://static.whatsapp.net/downloadable?id=1",
                "mediaKey": "a2V5"}}},
            "body": {"text": "*Pay faster* 🎁"},
            "InteractiveMessage": {"NativeFlowMessage": {"buttons": [
                flow("cta_url", json!({"display_text": "Start", "url": "https://wa.me/x"})),
                flow("cta_copy", json!({"display_text": "Copy code", "copy_code": "SAVE10"})),
                flow("quick_reply", json!({"display_text": "Not now", "id": "n"})),
                flow("cta_url", json!({"display_text": "Trap", "url": "javascript:alert(1)"})),
                flow("payment_info", json!({"currency": "BRL"}))]}}}}));
        assert_eq!(
            (m.kind, m.body.as_str()),
            (MessageKind::Video, "*Pay faster* 🎁")
        );
        assert_eq!(
            buttons(&m),
            [
                ("Start", "https://wa.me/x"),
                ("Copy code: SAVE10", ""),
                ("Not now", ""),
                ("Trap", "")
            ]
        );
        assert_eq!(media_message(&m.raw), None);

        let image = json!({"URL": "https://mmg.whatsapp.net/x", "directPath": "/v/x",
            "mimetype": "image/jpeg", "mediaKey": "a2V5"});
        let m = new_message(json!({"templateMessage": {
            "hydratedTemplate": {
                "Title": {"ImageMessage": image},
                "hydratedContentText": "Your plan is ready",
                "hydratedFooterText": "Reply STOP to opt out",
                "hydratedButtons": [
                    {"HydratedButton": {"UrlButton": {"displayText": "Open", "URL": "https://isp.example/plan"}}, "index": 0},
                    {"HydratedButton": {"QuickReplyButton": {"displayText": "Upgrade", "ID": "u"}}, "index": 1}]},
            "templateID": "1"}}));
        assert_eq!(
            (m.kind, m.body.as_str(), m.mime.as_str()),
            (
                MessageKind::Image,
                "Your plan is ready\n\nReply STOP to opt out",
                "image/jpeg"
            )
        );
        assert_eq!(
            buttons(&m),
            [("Open", "https://isp.example/plan"), ("Upgrade", "")]
        );
        assert_eq!(media_message(&m.raw), Some(json!({"imageMessage": image})));

        let m = new_message(
            json!({"templateMessage": {"Format": {"InteractiveMessageTemplate": {
            "header": {"Media": {"DocumentMessage": {"fileName": "bill.pdf", "directPath": "/d"}}},
            "body": {"text": "Your bill"}, "footer": {"text": "Thanks"}}}}}),
        );
        assert_eq!(
            (m.kind, m.body.as_str(), m.file_name.as_str()),
            (MessageKind::Document, "Your bill\n\nThanks", "bill.pdf")
        );

        let m = new_message(json!({"buttonsMessage": {"Header": {"Text": "Survey"},
            "contentText": "Did it work?", "footerText": "",
            "buttons": [{"buttonID": "y", "buttonText": {"displayText": "Yes"}, "type": 1}]}}));
        assert_eq!(
            (m.kind, m.body.as_str()),
            (MessageKind::Text, "Survey\n\nDid it work?")
        );
        assert_eq!(buttons(&m), [("Yes", "")]);

        let m = new_message(json!({"listMessage": {"description": "Pick a day",
            "buttonText": "Days", "sections": [{"rows": [{"title": "Mon", "rowID": "1"},
            {"title": "Tue", "rowID": "2"}]}]}}));
        assert_eq!((m.kind, m.body.as_str()), (MessageKind::Text, "Pick a day"));
        assert_eq!(buttons(&m), [("Mon", ""), ("Tue", "")]);
    }

    #[test]
    fn polls_and_answers_to_business_buttons() {
        let m = new_message(json!({"pollCreationMessageV3": {"name": "Lunch?",
            "options": [{"optionName": "Yes"}, {"optionName": "No"}], "selectableOptionsCount": 1}}));
        assert_eq!((m.kind, m.body.as_str()), (MessageKind::Poll, "Lunch?"));
        assert_eq!(buttons(&m), [("Yes", ""), ("No", "")]);

        // An answer quotes the business message it answers.
        let m = new_message(json!({"buttonsResponseMessage": {
            "Response": {"SelectedDisplayText": "Yes"}, "selectedButtonID": "y",
            "contextInfo": {"stanzaID": "B1", "participant": "552@s.whatsapp.net",
                "quotedMessage": {"buttonsMessage": {"contentText": "Did it work?"}}}}}));
        assert_eq!((m.kind, m.body.as_str()), (MessageKind::Text, "Yes"));
        let quote = m.quote.unwrap();
        assert_eq!(
            (quote.id.as_str(), quote.kind, quote.text.as_str()),
            ("B1", MessageKind::Text, "Did it work?")
        );
        for (answer, text) in [
            (
                json!({"listResponseMessage": {"title": "Mon", "singleSelectReply": {}}}),
                "Mon",
            ),
            (
                json!({"templateButtonReplyMessage": {"selectedDisplayText": "Upgrade"}}),
                "Upgrade",
            ),
            (
                json!({"interactiveResponseMessage": {"body": {"text": "Sent"}}}),
                "Sent",
            ),
        ] {
            let m = new_message(answer);
            assert_eq!((m.kind, m.body.as_str()), (MessageKind::Text, text));
        }
    }

    #[test]
    fn album_items_are_messages_and_the_album_is_not() {
        let album = json!({"albumMessage": {"expectedImageCount": 2, "expectedVideoCount": 0}});
        assert_eq!(parse(&live(album.clone())), Event::Changes(vec![]));

        let photo = json!({"caption": "beach", "mimetype": "image/jpeg", "directPath": "/p"});
        let m = new_message(
            json!({"associatedChildMessage": {"message": {"imageMessage": photo}},
            "messageContextInfo": {}}),
        );
        assert_eq!((m.kind, m.body.as_str()), (MessageKind::Image, "beach"));
        assert_eq!(media_message(&m.raw), Some(json!({"imageMessage": photo})));

        // A reply to an album quotes it as the media it holds.
        let m = new_message(
            json!({"extendedTextMessage": {"text": "nice", "contextInfo": {
            "stanzaID": "A1", "quotedMessage": album}}}),
        );
        let quote = m.quote.unwrap();
        assert_eq!((quote.kind, quote.text.as_str()), (MessageKind::Image, ""));

        // Round video notes download as videos.
        let note = json!({"mimetype": "video/mp4", "directPath": "/v", "seconds": 5});
        let m = new_message(json!({"ptvMessage": note}));
        assert_eq!(m.kind, MessageKind::Video);
        assert_eq!(media_message(&m.raw), Some(json!({"videoMessage": note})));
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
                {"lidJID": "", "pnJID": "5511999990001@s.whatsapp.net"},
                {"lidJID": "88@lid", "pnJID": "5511999990004@s.whatsapp.net"}],
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
                ("88@lid".into(), "5511999990004@s.whatsapp.net".into()),
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
