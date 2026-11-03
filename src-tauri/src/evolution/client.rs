//! REST adapter for an Evolution Go server (`pkg/routes/routes.go`).
//!
//! Admin routes take the server's global key and instance routes the
//! instance token; the two are distinct types so a call cannot pick the wrong
//! scope. Non-2xx responses are mapped before `{message, data}` is unwrapped,
//! and server messages are sanitized because some echo request data.

use std::time::Duration;

use reqwest::{Method, StatusCode};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use ts_rs::TS;
use url::Url;

use crate::secrets::Secret;

const TIMEOUT: Duration = Duration::from_secs(20);
/// Uploads and media downloads carry whole files.
const MEDIA_TIMEOUT: Duration = Duration::from_secs(120);
const MAX_ERROR_CHARS: usize = 200;

/// Events this client consumes. `POST /instance/connect` replaces the list, so
/// any category the instance already had is kept as well (see `connect`).
pub const SUBSCRIPTIONS: &[&str] = &[
    "MESSAGE",
    "SEND_MESSAGE",
    "READ_RECEIPT",
    "CONNECTION",
    "QRCODE",
    "PRESENCE",
    "HISTORY_SYNC",
    "CHAT_PRESENCE",
    "CONTACT",
    "GROUP",
    "PICTURE",
];

/// The server's `GLOBAL_API_KEY`.
#[derive(Clone)]
pub struct AdminKey(pub Secret);

/// One instance's token; it also selects the instance on the server.
#[derive(Clone)]
pub struct InstanceToken(pub Secret);

impl InstanceToken {
    /// A new random token for `POST /instance/create`.
    pub fn generate() -> Self {
        let bytes: [u8; 32] = rand::random();
        Self(Secret::new(hex::encode(bytes)))
    }
}

#[derive(Debug, thiserror::Error)]
pub enum EvoError {
    #[error("the server refused the credentials")]
    Unauthorized,
    #[error("the server address or route was not found")]
    NotFound,
    #[error("the server returned {status}: {message}")]
    Http { status: u16, message: String },
    #[error("the server did not answer in time")]
    Timeout,
    /// A send whose outcome is unknown (timeout or connection loss after the
    /// request left); it must not be retried blindly.
    #[error("the message may or may not have been sent")]
    Ambiguous,
    #[error("could not reach the server: {0}")]
    Network(String),
    #[error("unexpected server response: {0}")]
    Invalid(String),
    #[error("invalid server address: {0}")]
    BadUrl(String),
}

pub type EvoResult<T> = Result<T, EvoError>;

/// Accept `https://host[:port][/prefix]`; plain HTTP only for loopback.
pub fn parse_base_url(input: &str) -> EvoResult<Url> {
    let trimmed = input.trim().trim_end_matches('/');
    let url = Url::parse(trimmed).map_err(|e| EvoError::BadUrl(e.to_string()))?;
    let loopback = matches!(
        url.host_str(),
        Some("localhost" | "127.0.0.1" | "[::1]" | "::1")
    );
    match url.scheme() {
        "https" => {}
        "http" if loopback => {}
        _ => return Err(EvoError::BadUrl("use an https:// address".into())),
    }
    if url.host_str().is_none_or(str::is_empty)
        || url.query().is_some()
        || url.fragment().is_some()
        || !url.username().is_empty()
        || url.password().is_some()
    {
        return Err(EvoError::BadUrl("use only scheme, host and path".into()));
    }
    Ok(url)
}

#[derive(Debug, Clone, Serialize, TS)]
#[ts(export)]
pub struct InstanceSummary {
    pub id: String,
    pub name: String,
    pub connected: bool,
    /// Paired account JID, empty before pairing.
    pub jid: String,
}

/// An instance row including what `connect` must preserve. Never logged.
#[derive(Clone, Deserialize)]
pub struct InstanceRow {
    pub id: String,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    token: String,
    #[serde(default)]
    pub connected: bool,
    #[serde(default)]
    pub jid: String,
    #[serde(default)]
    pub webhook: String,
    #[serde(default, rename = "rabbitmqEnable")]
    pub rabbitmq_enable: String,
    #[serde(default, rename = "natsEnable")]
    pub nats_enable: String,
    #[serde(default, rename = "websocketEnable")]
    pub websocket_enable: String,
    #[serde(default)]
    pub events: String,
}

impl std::fmt::Debug for InstanceRow {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("InstanceRow")
            .field("id", &self.id)
            .field("name", &self.name)
            .finish_non_exhaustive()
    }
}

impl InstanceRow {
    pub fn summary(&self) -> InstanceSummary {
        InstanceSummary {
            id: self.id.clone(),
            name: self.name.clone(),
            connected: self.connected,
            jid: self.jid.clone(),
        }
    }

    /// Whether `connect` must run: the client is down, the WebSocket is off
    /// or a category this app needs is not subscribed.
    pub fn needs_connect(&self) -> bool {
        let subscribed: Vec<&str> = self.events.split(',').map(str::trim).collect();
        !self.connected
            || !matches!(self.websocket_enable.as_str(), "true" | "enabled")
            || !(subscribed.contains(&"ALL")
                || SUBSCRIPTIONS.iter().all(|s| subscribed.contains(s)))
    }

    pub fn token(&self) -> Option<InstanceToken> {
        (!self.token.is_empty()).then(|| InstanceToken(Secret::new(self.token.clone())))
    }
}

/// The `POST /instance/connect` body: WebSocket on, our subscriptions plus
/// the instance's own, and its webhook/RabbitMQ/NATS settings untouched.
pub fn connect_payload(existing: Option<&InstanceRow>) -> Value {
    let mut subscribe: Vec<String> = SUBSCRIPTIONS.iter().map(|s| s.to_string()).collect();
    // Older servers store "enabled"; anything else unset is off.
    let enabled = |v: &str| {
        if v == "true" || v == "enabled" {
            v.to_owned()
        } else {
            "false".to_owned()
        }
    };
    let (webhook, rabbitmq, nats) = match existing {
        Some(row) => {
            for event in row.events.split(',').map(str::trim) {
                if !event.is_empty() && !subscribe.iter().any(|s| s == event) {
                    subscribe.push(event.to_owned());
                }
            }
            (
                row.webhook.clone(),
                enabled(&row.rabbitmq_enable),
                enabled(&row.nats_enable),
            )
        }
        None => (String::new(), "false".into(), "false".into()),
    };
    json!({
        "subscribe": subscribe,
        "webhookUrl": webhook,
        "websocketEnable": "true",
        "rabbitmqEnable": rabbitmq,
        "natsEnable": nats,
    })
}

#[derive(Debug, Clone, Default, Serialize, TS)]
#[ts(export)]
pub struct ConnectionStatus {
    pub connected: bool,
    pub logged_in: bool,
    pub name: String,
}

/// `GET /instance/qr`; the QR data and passkey fields are credentials.
#[derive(Clone, Default, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct QrCode {
    /// Image representation (a data URL when prefixed).
    #[serde(default)]
    pub qrcode: String,
    #[serde(default)]
    pub code: String,
    #[serde(default, rename(deserialize = "passkeyStage"))]
    pub passkey_stage: String,
    #[serde(default, rename(deserialize = "passkeyOpenUrl"))]
    pub passkey_open_url: String,
    #[serde(default, rename(deserialize = "passkeyCode"))]
    pub passkey_code: String,
}

impl std::fmt::Debug for QrCode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("QrCode(<redacted>)")
    }
}

/// `quoted` of `/send/text` and `/send/media`.
#[derive(Debug, Clone, Default, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct Quote {
    #[serde(rename = "messageId")]
    pub message_id: String,
    pub participant: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "lowercase")]
#[ts(export)]
pub enum MediaKind {
    Image,
    Video,
    Audio,
    Document,
}

impl MediaKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Image => "image",
            Self::Video => "video",
            Self::Audio => "audio",
            Self::Document => "document",
        }
    }
}

pub struct MediaUpload {
    pub kind: MediaKind,
    pub bytes: Vec<u8>,
    pub filename: String,
    pub mime: String,
    pub caption: String,
}

/// A page of the server's private journal (`GET /chat/events`).
#[derive(Debug, Clone, Deserialize)]
pub struct EventPage {
    #[serde(default)]
    pub events: Vec<JournalEntry>,
    #[serde(default, rename = "nextCursor")]
    pub next_cursor: String,
    #[serde(default, rename = "hasMore")]
    pub has_more: bool,
}

#[derive(Debug, Clone, Deserialize)]
pub struct JournalEntry {
    pub id: String,
    pub payload: Value,
}

#[derive(Debug, Clone, Serialize, TS)]
#[ts(export)]
pub struct Contact {
    pub jid: String,
    pub name: String,
}

pub struct Client {
    http: reqwest::Client,
    base: Url,
    timeout: Duration,
}

fn key_header(secret: &Secret) -> (&'static str, &str) {
    ("apikey", secret.expose())
}

fn sanitize_message(raw: &str) -> String {
    let text = crate::reporting::sanitize::text(raw);
    text.chars().take(MAX_ERROR_CHARS).collect()
}

fn map_send_error(err: reqwest::Error) -> EvoError {
    // Once the request may have reached the server the outcome is unknown.
    if err.is_connect() {
        EvoError::Network(sanitize_message(&err.to_string()))
    } else {
        EvoError::Ambiguous
    }
}

fn map_error(err: reqwest::Error) -> EvoError {
    if err.is_timeout() {
        EvoError::Timeout
    } else {
        // reqwest errors name the URL; ours has no secrets, but keep it short.
        EvoError::Network(sanitize_message(&err.without_url().to_string()))
    }
}

impl Client {
    pub fn new(base: Url) -> EvoResult<Self> {
        let http = reqwest::Client::builder()
            .connect_timeout(Duration::from_secs(10))
            .redirect(reqwest::redirect::Policy::none())
            .user_agent(concat!("FastZap/", env!("CARGO_PKG_VERSION")))
            .build()
            .map_err(|e| EvoError::Network(e.to_string()))?;
        Ok(Self {
            http,
            base,
            timeout: TIMEOUT,
        })
    }

    pub fn base(&self) -> &Url {
        &self.base
    }

    fn url(&self, path: &str) -> Url {
        let mut url = self.base.clone();
        let joined = format!("{}{}", url.path().trim_end_matches('/'), path);
        url.set_path(&joined);
        url
    }

    /// Check the status, then unwrap `data`.
    async fn finish(response: reqwest::Response) -> EvoResult<Value> {
        let status = response.status();
        let body: Value = response.json().await.unwrap_or(Value::Null);
        match status {
            s if s.is_success() => Ok(body.get("data").cloned().unwrap_or(Value::Null)),
            StatusCode::UNAUTHORIZED | StatusCode::FORBIDDEN => Err(EvoError::Unauthorized),
            StatusCode::NOT_FOUND => Err(EvoError::NotFound),
            s => Err(EvoError::Http {
                status: s.as_u16(),
                message: sanitize_message(
                    body.get("error")
                        .or_else(|| body.get("message"))
                        .and_then(Value::as_str)
                        .unwrap_or(""),
                ),
            }),
        }
    }

    async fn call(
        &self,
        method: Method,
        path: &str,
        key: &Secret,
        body: Option<&Value>,
    ) -> EvoResult<Value> {
        let (name, value) = key_header(key);
        let mut request = self
            .http
            .request(method, self.url(path))
            .header(name, value)
            .timeout(self.timeout);
        if let Some(body) = body {
            request = request.json(body);
        }
        Self::finish(request.send().await.map_err(map_error)?).await
    }

    /// A message-creating call: a timeout after sending is `Ambiguous`.
    async fn send_call(&self, path: &str, key: &Secret, body: &Value) -> EvoResult<Value> {
        let (name, value) = key_header(key);
        let response = self
            .http
            .post(self.url(path))
            .header(name, value)
            .timeout(self.timeout)
            .json(body)
            .send()
            .await
            .map_err(map_send_error)?;
        Self::finish(response).await
    }

    pub async fn server_ok(&self) -> EvoResult<()> {
        let response = self
            .http
            .get(self.url("/server/ok"))
            .timeout(self.timeout)
            .send()
            .await
            .map_err(map_error)?;
        if response.status().is_success() {
            Ok(())
        } else {
            Err(EvoError::Http {
                status: response.status().as_u16(),
                message: String::new(),
            })
        }
    }

    // ---- Admin scope ----

    pub async fn instances(&self, admin: &AdminKey) -> EvoResult<Vec<InstanceRow>> {
        let data = self
            .call(Method::GET, "/instance/all", &admin.0, None)
            .await?;
        serde_json::from_value(data).map_err(|e| EvoError::Invalid(e.to_string()))
    }

    pub async fn instance(&self, admin: &AdminKey, id: &str) -> EvoResult<InstanceRow> {
        self.instances(admin)
            .await?
            .into_iter()
            .find(|row| row.id == id)
            .ok_or(EvoError::NotFound)
    }

    /// Create an instance; returns its id.
    pub async fn create_instance(
        &self,
        admin: &AdminKey,
        name: &str,
        token: &InstanceToken,
    ) -> EvoResult<String> {
        let body = json!({"name": name, "token": token.0.expose()});
        let data = self
            .call(Method::POST, "/instance/create", &admin.0, Some(&body))
            .await?;
        data.get("id")
            .and_then(Value::as_str)
            .map(str::to_owned)
            .ok_or_else(|| EvoError::Invalid("missing instance id".into()))
    }

    // ---- Instance scope ----

    pub async fn connect(&self, token: &InstanceToken, payload: &Value) -> EvoResult<()> {
        self.call(Method::POST, "/instance/connect", &token.0, Some(payload))
            .await
            .map(drop)
    }

    pub async fn status(&self, token: &InstanceToken) -> EvoResult<ConnectionStatus> {
        let data = self
            .call(Method::GET, "/instance/status", &token.0, None)
            .await?;
        let flag = |k: &str| data.get(k).and_then(Value::as_bool).unwrap_or(false);
        Ok(ConnectionStatus {
            connected: flag("Connected"),
            logged_in: flag("LoggedIn"),
            name: data
                .get("Name")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_owned(),
        })
    }

    /// Starts pairing on the server; call only while the account needs it.
    pub async fn qr(&self, token: &InstanceToken) -> EvoResult<QrCode> {
        let data = self
            .call(Method::GET, "/instance/qr", &token.0, None)
            .await?;
        serde_json::from_value(data).map_err(|e| EvoError::Invalid(e.to_string()))
    }

    pub async fn pair_phone(&self, token: &InstanceToken, phone: &str) -> EvoResult<String> {
        let body = json!({"phone": phone, "subscribe": SUBSCRIPTIONS});
        let data = self
            .call(Method::POST, "/instance/pair", &token.0, Some(&body))
            .await?;
        data.get("PairingCode")
            .and_then(Value::as_str)
            .map(str::to_owned)
            .ok_or_else(|| EvoError::Invalid("missing pairing code".into()))
    }

    pub async fn logout(&self, token: &InstanceToken) -> EvoResult<()> {
        self.call(Method::DELETE, "/instance/logout", &token.0, None)
            .await
            .map(drop)
    }

    /// Send text; returns the WhatsApp message id (`data.Info.ID`).
    pub async fn send_text(
        &self,
        token: &InstanceToken,
        number: &str,
        text: &str,
        id: &str,
        quote: Option<&Quote>,
    ) -> EvoResult<String> {
        let mut body = json!({"number": number, "text": text, "id": id});
        // A known JID must not be reformatted (Brazilian ninth digit, LIDs).
        if number.contains('@') {
            body["formatJid"] = false.into();
        }
        if let Some(quote) = quote {
            body["quoted"] = serde_json::to_value(quote).unwrap_or_default();
        }
        let data = self.send_call("/send/text", &token.0, &body).await?;
        sent_id(&data, id)
    }

    pub async fn send_media(
        &self,
        token: &InstanceToken,
        number: &str,
        media: MediaUpload,
        id: &str,
        quote: Option<&Quote>,
    ) -> EvoResult<String> {
        let part = reqwest::multipart::Part::bytes(media.bytes)
            .file_name(media.filename.clone())
            .mime_str(&media.mime)
            .map_err(|e| EvoError::Invalid(e.to_string()))?;
        let mut form = reqwest::multipart::Form::new()
            .text("number", number.to_owned())
            .text("type", media.kind.as_str())
            .text("id", id.to_owned())
            .text("filename", media.filename)
            .part("file", part);
        if number.contains('@') {
            form = form.text("formatJid", "false");
        }
        if !media.caption.is_empty() {
            form = form.text("caption", media.caption);
        }
        if let Some(quote) = quote {
            form = form
                .text("quoted.messageId", quote.message_id.clone())
                .text("quoted.participant", quote.participant.clone());
        }
        let (name, value) = key_header(&token.0);
        let response = self
            .http
            .post(self.url("/send/media"))
            .header(name, value)
            .timeout(MEDIA_TIMEOUT)
            .multipart(form)
            .send()
            .await
            .map_err(map_send_error)?;
        let data = Self::finish(response).await?;
        sent_id(&data, id)
    }

    pub async fn react(
        &self,
        token: &InstanceToken,
        chat: &str,
        message_id: &str,
        from_me: bool,
        participant: &str,
        reaction: &str,
    ) -> EvoResult<()> {
        let mut body = json!({
            "number": chat, "id": message_id, "fromMe": from_me, "reaction": reaction,
        });
        if !participant.is_empty() {
            body["participant"] = participant.into();
        }
        self.send_call("/message/react", &token.0, &body)
            .await
            .map(drop)
    }

    pub async fn mark_read(
        &self,
        token: &InstanceToken,
        chat: &str,
        ids: &[String],
    ) -> EvoResult<()> {
        let body = json!({"number": chat, "id": ids});
        self.call(Method::POST, "/message/markread", &token.0, Some(&body))
            .await
            .map(drop)
    }

    /// `composing` or `paused`; no `delay`, which would keep a server loop alive.
    pub async fn presence(&self, token: &InstanceToken, chat: &str, state: &str) -> EvoResult<()> {
        let body = json!({"number": chat, "state": state, "isAudio": false});
        self.call(Method::POST, "/message/presence", &token.0, Some(&body))
            .await
            .map(drop)
    }

    pub async fn edit(
        &self,
        token: &InstanceToken,
        chat: &str,
        message_id: &str,
        text: &str,
    ) -> EvoResult<()> {
        let body = json!({"chat": chat, "messageId": message_id, "message": text});
        self.send_call("/message/edit", &token.0, &body)
            .await
            .map(drop)
    }

    pub async fn delete(
        &self,
        token: &InstanceToken,
        chat: &str,
        message_id: &str,
    ) -> EvoResult<()> {
        let body = json!({"chat": chat, "messageId": message_id});
        self.send_call("/message/delete", &token.0, &body)
            .await
            .map(drop)
    }

    /// Decrypt a media message; returns a data URL.
    pub async fn download_media(
        &self,
        token: &InstanceToken,
        message: &Value,
    ) -> EvoResult<String> {
        let (name, value) = key_header(&token.0);
        let response = self
            .http
            .post(self.url("/message/downloadmedia"))
            .header(name, value)
            .timeout(MEDIA_TIMEOUT)
            .json(&json!({"message": message}))
            .send()
            .await
            .map_err(map_error)?;
        let data = Self::finish(response).await?;
        data.get("base64")
            .and_then(Value::as_str)
            .map(str::to_owned)
            .ok_or_else(|| EvoError::Invalid("missing media".into()))
    }

    /// Profile picture URL; `None` when the contact has none.
    pub async fn avatar(&self, token: &InstanceToken, jid: &str) -> EvoResult<Option<String>> {
        let body = json!({"number": jid, "preview": true});
        match self
            .call(Method::POST, "/user/avatar", &token.0, Some(&body))
            .await
        {
            Ok(data) => Ok(data
                .get("url")
                .or_else(|| data.get("URL"))
                .and_then(Value::as_str)
                .filter(|u| u.starts_with("https://"))
                .map(str::to_owned)),
            Err(EvoError::Http { .. } | EvoError::NotFound) => Ok(None),
            Err(other) => Err(other),
        }
    }

    pub async fn contacts(&self, token: &InstanceToken) -> EvoResult<Vec<Contact>> {
        let data = self
            .call(Method::GET, "/user/contacts", &token.0, None)
            .await?;
        let rows = data.as_array().cloned().unwrap_or_default();
        Ok(rows
            .iter()
            .filter_map(|row| {
                let text = |k: &str| row.get(k).and_then(Value::as_str).unwrap_or("").trim();
                let jid = text("Jid");
                let name = [
                    text("FullName"),
                    text("FirstName"),
                    text("BusinessName"),
                    text("PushName"),
                ]
                .into_iter()
                .find(|n| !n.is_empty())?;
                (!jid.is_empty()).then(|| Contact {
                    jid: jid.to_owned(),
                    name: name.to_owned(),
                })
            })
            .collect())
    }

    pub async fn groups(&self, token: &InstanceToken) -> EvoResult<Vec<Contact>> {
        let data = self
            .call(Method::GET, "/group/list", &token.0, None)
            .await?;
        let rows = data.as_array().cloned().unwrap_or_default();
        Ok(rows
            .iter()
            .filter_map(|row| {
                let jid = row.get("JID").and_then(Value::as_str)?;
                let name = row.get("Name").and_then(Value::as_str).unwrap_or("");
                Some(Contact {
                    jid: jid.to_owned(),
                    name: name.to_owned(),
                })
            })
            .collect())
    }

    /// The private journal after `after`; works while WhatsApp is offline.
    pub async fn chat_events(&self, token: &InstanceToken, after: &str) -> EvoResult<EventPage> {
        let mut url = self.url("/chat/events");
        // The server rejects an empty cursor; without one it starts at 0.
        if !after.is_empty() {
            url.query_pairs_mut().append_pair("after", after);
        }
        let (name, value) = key_header(&token.0);
        let response = self
            .http
            .get(url)
            .header(name, value)
            .timeout(MEDIA_TIMEOUT)
            .send()
            .await
            .map_err(map_error)?;
        let data = Self::finish(response).await?;
        serde_json::from_value(data).map_err(|e| EvoError::Invalid(e.to_string()))
    }

    /// Ask the phone for `count` messages older than `anchor` (a whatsmeow
    /// `MessageInfo`); they arrive later as a HistorySync event.
    pub async fn history_sync(
        &self,
        token: &InstanceToken,
        anchor: &Value,
        count: u32,
    ) -> EvoResult<()> {
        let body = json!({"messageInfo": anchor, "count": count});
        self.call(Method::POST, "/chat/history-sync", &token.0, Some(&body))
            .await
            .map(drop)
    }
}

fn sent_id(data: &Value, fallback: &str) -> EvoResult<String> {
    Ok(data
        .pointer("/Info/ID")
        .and_then(Value::as_str)
        .unwrap_or(fallback)
        .to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;
    use wiremock::matchers::{
        body_json, header, method, path, query_param, query_param_is_missing,
    };
    use wiremock::{Mock, MockServer, ResponseTemplate};

    fn admin() -> AdminKey {
        AdminKey(Secret::new("admin-key"))
    }

    fn token() -> InstanceToken {
        InstanceToken(Secret::new("instance-token"))
    }

    async fn client(server: &MockServer) -> Client {
        Client::new(parse_base_url(&server.uri()).unwrap()).unwrap()
    }

    #[test]
    fn base_urls_require_https_except_loopback() {
        assert!(parse_base_url("https://evo.example.com/").is_ok());
        assert!(parse_base_url("http://127.0.0.1:8080").is_ok());
        assert!(parse_base_url("http://evo.example.com").is_err());
        assert!(parse_base_url("https://user:pw@evo.example.com").is_err());
        assert!(parse_base_url("https://evo.example.com/?apikey=x").is_err());
        assert_eq!(
            parse_base_url("https://evo.example.com/prefix/")
                .unwrap()
                .as_str(),
            "https://evo.example.com/prefix"
        );
    }

    #[tokio::test]
    async fn scopes_use_their_own_key() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/instance/all"))
            .and(header("apikey", "admin-key"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "message": "success",
                "data": [{"id": "u1", "name": "n", "token": "t", "connected": true, "jid": "1@s.whatsapp.net"}]
            })))
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path("/instance/status"))
            .and(header("apikey", "instance-token"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "message": "success",
                "data": {"Connected": true, "LoggedIn": false, "Name": "Me"}
            })))
            .mount(&server)
            .await;
        let c = client(&server).await;
        let rows = c.instances(&admin()).await.unwrap();
        assert_eq!(rows[0].summary().id, "u1");
        assert_eq!(rows[0].token().unwrap().0.expose(), "t");
        assert!(!format!("{:?}", rows[0]).contains("\"t\""));
        let status = c.status(&token()).await.unwrap();
        assert!(status.connected && !status.logged_in);
        assert_eq!(status.name, "Me");
    }

    #[tokio::test]
    async fn errors_are_mapped_before_unwrapping() {
        let server = MockServer::start().await;
        Mock::given(path("/instance/status"))
            .respond_with(
                ResponseTemplate::new(401).set_body_json(json!({"error": "not authorized"})),
            )
            .mount(&server)
            .await;
        Mock::given(path("/instance/qr"))
            .respond_with(
                ResponseTemplate::new(500)
                    .set_body_json(json!({"error": "bad token=abcdefghijklmnop"})),
            )
            .mount(&server)
            .await;
        let c = client(&server).await;
        assert!(matches!(
            c.status(&token()).await,
            Err(EvoError::Unauthorized)
        ));
        match c.qr(&token()).await {
            Err(EvoError::Http { status, message }) => {
                assert_eq!(status, 500);
                assert!(!message.contains("abcdefghijklmnop"));
            }
            other => panic!("{other:?}"),
        }
    }

    #[tokio::test]
    async fn connect_sends_the_complete_configuration() {
        let row: InstanceRow = serde_json::from_value(json!({
            "id": "u1", "webhook": "http://hook:3000/x", "events": "MESSAGE,CALL",
            "rabbitmqEnable": "enabled", "natsEnable": ""
        }))
        .unwrap();
        let payload = connect_payload(Some(&row));
        assert_eq!(payload["websocketEnable"], "true");
        assert_eq!(payload["webhookUrl"], "http://hook:3000/x");
        assert_eq!(payload["rabbitmqEnable"], "enabled");
        assert_eq!(payload["natsEnable"], "false");
        let subscribe: Vec<&str> = payload["subscribe"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(Value::as_str)
            .collect();
        assert!(subscribe.contains(&"CALL") && subscribe.contains(&"HISTORY_SYNC"));
        assert_eq!(subscribe.iter().filter(|s| **s == "MESSAGE").count(), 1);
        assert!(row.needs_connect());
        let ready: InstanceRow = serde_json::from_value(json!({
            "id": "u1", "connected": true, "websocketEnable": "true", "events": "ALL"
        }))
        .unwrap();
        assert!(!ready.needs_connect());

        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/instance/connect"))
            .and(header("apikey", "instance-token"))
            .and(body_json(connect_payload(None)))
            .respond_with(
                ResponseTemplate::new(200).set_body_json(json!({"message": "success", "data": {}})),
            )
            .expect(1)
            .mount(&server)
            .await;
        client(&server)
            .await
            .connect(&token(), &connect_payload(None))
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn sends_return_the_message_id_and_timeouts_are_ambiguous() {
        let server = MockServer::start().await;
        Mock::given(path("/send/text"))
            .and(body_json(json!({
                "number": "5511999999999", "text": "hi", "id": "LOCAL1",
                "quoted": {"messageId": "Q1", "participant": "1@s.whatsapp.net"}
            })))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "message": "success", "data": {"Info": {"ID": "SERVER1"}}
            })))
            .mount(&server)
            .await;
        Mock::given(path("/message/edit"))
            .respond_with(ResponseTemplate::new(200).set_delay(Duration::from_secs(5)))
            .mount(&server)
            .await;
        let mut c = client(&server).await;
        c.timeout = Duration::from_millis(200);
        let quote = Quote {
            message_id: "Q1".into(),
            participant: "1@s.whatsapp.net".into(),
        };
        let id = c
            .send_text(&token(), "5511999999999", "hi", "LOCAL1", Some(&quote))
            .await
            .unwrap();
        assert_eq!(id, "SERVER1");
        assert!(matches!(
            c.edit(&token(), "1@s.whatsapp.net", "SERVER1", "x").await,
            Err(EvoError::Ambiguous)
        ));
    }

    #[tokio::test]
    async fn journal_pages_are_read_with_the_instance_token() {
        let server = MockServer::start().await;
        Mock::given(path("/chat/events"))
            .and(query_param("after", "41"))
            .and(header("apikey", "instance-token"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "message": "success",
                "data": {"events": [{"id": "42", "payload": {"event": "Message", "data": {}}}],
                         "nextCursor": "42", "hasMore": false}
            })))
            .mount(&server)
            .await;
        let page = client(&server)
            .await
            .chat_events(&token(), "41")
            .await
            .unwrap();
        assert_eq!(page.events[0].id, "42");
        assert_eq!(page.next_cursor, "42");
    }

    #[tokio::test]
    async fn first_journal_read_sends_no_cursor() {
        let server = MockServer::start().await;
        Mock::given(path("/chat/events"))
            .and(query_param_is_missing("after"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "message": "success",
                "data": {"events": [], "nextCursor": "0", "hasMore": false}
            })))
            .mount(&server)
            .await;
        let page = client(&server)
            .await
            .chat_events(&token(), "")
            .await
            .unwrap();
        assert!(page.events.is_empty());
    }
}
