//! Commands of the native Evolution Go chat. The admin key is accepted only by
//! the setup commands and goes straight to the keyring; nothing returned here
//! carries the admin key or an instance token.

use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use base64::Engine;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use tauri::{AppHandle, Emitter, Manager, State};
use ts_rs::TS;

use super::CommandResult;
use crate::accounts::{Account, AccountKind, AccountPatch, EvolutionConfig, NewAccount};
use crate::app::events::ACCOUNTS_CHANGED;
use crate::app::{effects, state::AppState};
use crate::evolution::client::{
    self, AdminKey, Client, InstanceSummary, InstanceToken, MediaKind, MediaUpload, QrCode, Quote,
};
use crate::evolution::model::{Change, Message, MessageKind, QuoteRef, Status};
use crate::evolution::store::{self, ChatMessage, ChatSummary};
use crate::evolution::{self as evo, Connection, Evolution, Session};
use crate::i18n::tr;
use crate::secrets::{self, Secret};

/// Upload limit; WhatsApp itself caps documents at 2 GB but the server holds
/// the whole multipart body in memory.
const MAX_UPLOAD: u64 = 100 * 1024 * 1024;
const MAX_TEXT: usize = 65_536;
const PAGE_MAX: u32 = 200;

fn session(app: &AppHandle, account_id: i64) -> CommandResult<Arc<Session>> {
    app.state::<Evolution>()
        .session(account_id)
        .ok_or_else(|| tr("This account is not connected to Evolution Go.").into())
}

fn admin_client(base_url: &str, admin_key: &str) -> CommandResult<(Client, AdminKey)> {
    let base = client::parse_base_url(base_url)?;
    let key = admin_key.trim();
    if key.is_empty() {
        return Err(tr("Enter the server's admin key.").into());
    }
    Ok((Client::new(base)?, AdminKey(Secret::new(key))))
}

/// Instances on a server, without their tokens.
#[tauri::command]
pub async fn evo_instances(
    base_url: String,
    admin_key: String,
) -> CommandResult<Vec<InstanceSummary>> {
    let (client, admin) = admin_client(&base_url, &admin_key)?;
    Ok(client
        .instances(&admin)
        .await?
        .iter()
        .map(|row| row.summary())
        .collect())
}

#[derive(Debug, Deserialize, TS)]
#[serde(tag = "mode", rename_all = "snake_case")]
#[ts(export)]
pub enum InstanceChoice {
    Existing { id: String },
    Create { name: String },
}

#[derive(Debug, Deserialize, TS)]
#[ts(export)]
pub struct EvolutionSetup {
    /// Reconfigure this account instead of creating one.
    #[ts(type = "number | null")]
    pub account_id: Option<i64>,
    pub name: String,
    pub icon: Option<String>,
    pub base_url: String,
    /// Empty keeps the stored key (reconfiguring only).
    pub admin_key: String,
    pub instance: InstanceChoice,
}

/// Create or reconfigure an Evolution Go account. Creating an instance is a
/// server-side change made only from this explicit request.
#[tauri::command]
pub async fn evo_setup(
    app: AppHandle,
    state: State<'_, AppState>,
    setup: EvolutionSetup,
) -> CommandResult<Account> {
    let admin_key = match (setup.admin_key.trim(), setup.account_id) {
        ("", Some(id)) => state
            .secrets
            .get(&secrets::evolution_admin_key(id))?
            .map(|s| s.expose().to_owned())
            .unwrap_or_default(),
        (key, _) => key.to_owned(),
    };
    let (client, admin) = admin_client(&setup.base_url, &admin_key)?;
    let base_url = client.base().to_string().trim_end_matches('/').to_owned();
    let (instance_id, instance_name, token) = match &setup.instance {
        InstanceChoice::Existing { id } => {
            let row = client.instance(&admin, id).await?;
            let token = row
                .token()
                .ok_or_else(|| tr("The instance has no token."))?;
            (row.id.clone(), row.name.clone(), token)
        }
        InstanceChoice::Create { name } => {
            let name = name.trim();
            if name.is_empty() {
                return Err(tr("Enter a name for the new instance.").into());
            }
            let token = InstanceToken::generate();
            let id = client.create_instance(&admin, name, &token).await?;
            (id, name.to_owned(), token)
        }
    };
    let evolution = EvolutionConfig {
        base_url,
        instance_id: Some(instance_id),
        instance_name: Some(instance_name),
    };
    let account = match setup.account_id {
        Some(id) => {
            if state.accounts.get(id)?.kind != AccountKind::EvolutionGo {
                return Err("not an Evolution Go account".into());
            }
            state.accounts.update(
                id,
                AccountPatch {
                    name: Some(setup.name.trim().to_owned()).filter(|n| !n.is_empty()),
                    evolution: Some(evolution),
                    ..Default::default()
                },
            )?
        }
        None => {
            let limit = state.settings.get().system.max_accounts;
            state.accounts.create(
                NewAccount {
                    name: setup.name.trim().to_owned(),
                    icon: setup.icon,
                    kind: AccountKind::EvolutionGo,
                    evolution: Some(evolution),
                },
                limit,
            )?
        }
    };
    let stored = state
        .secrets
        .set(&secrets::evolution_admin_key(account.id), &admin.0)
        .and_then(|()| {
            state
                .secrets
                .set(&secrets::evolution_instance_token(account.id), &token.0)
        });
    if let Err(e) = stored {
        if setup.account_id.is_none() {
            let _ = state.accounts.delete(account.id);
        }
        return Err(format!(
            "{}: {e}",
            tr("Could not save the credentials in the keyring")
        )
        .into());
    }
    // New credentials or instance: restart the session.
    evo::stop(&app, account.id);
    let list = state.accounts.list()?;
    app.emit(ACCOUNTS_CHANGED, &list)?;
    effects::on_accounts_changed(&app);
    evo::sync(&app);
    Ok(account)
}

#[tauri::command]
pub fn evo_connection(app: AppHandle, account_id: i64) -> Connection {
    app.state::<Evolution>().connection(account_id)
}

/// Retry now (e.g. after the server came back).
#[tauri::command]
pub async fn evo_reconnect(app: AppHandle, account_id: i64) -> CommandResult<()> {
    let session = session(&app, account_id)?;
    let status = evo::ensure_connected(&app, &session).await;
    if status.is_some_and(|s| s.logged_in && !s.connected) {
        session.client.reconnect(&session.token).await?;
        evo::spawn_refresh(&app, account_id, Some(Duration::from_secs(3)));
    }
    Ok(())
}

/// The server restarts a logged-in client to make a new QR, so this only
/// runs while WhatsApp is not linked.
async fn unlinked(session: &Session) -> CommandResult<()> {
    if session.client.status(&session.token).await?.logged_in {
        return Err(tr("WhatsApp is already linked to this instance.").into());
    }
    Ok(())
}

#[tauri::command]
pub async fn evo_qr(app: AppHandle, account_id: i64) -> CommandResult<QrCode> {
    let session = session(&app, account_id)?;
    unlinked(&session).await?;
    Ok(session.client.qr(&session.token).await?)
}

/// Open the passkey ceremony page (web.whatsapp.com with the server's
/// helper extension) in the system browser.
#[tauri::command]
pub async fn evo_open_passkey(app: AppHandle, account_id: i64) -> CommandResult<()> {
    let session = session(&app, account_id)?;
    unlinked(&session).await?;
    let url = session.client.qr(&session.token).await?.passkey_open_url;
    if !url.starts_with("https://web.whatsapp.com/#wapk=") {
        return Err(tr("No passkey request is pending.").into());
    }
    tauri_plugin_opener::open_url(url, None::<&str>)?;
    Ok(())
}

#[tauri::command]
pub async fn evo_pair_phone(
    app: AppHandle,
    account_id: i64,
    phone: String,
) -> CommandResult<String> {
    let digits: String = phone.chars().filter(char::is_ascii_digit).collect();
    if !(7..=15).contains(&digits.len()) {
        return Err(tr("Enter the phone number with country code.").into());
    }
    let session = session(&app, account_id)?;
    unlinked(&session).await?;
    Ok(session.client.pair_phone(&session.token, &digits).await?)
}

/// Unlink WhatsApp from the instance (the instance itself is kept).
#[tauri::command]
pub async fn evo_logout(app: AppHandle, account_id: i64) -> CommandResult<()> {
    let session = session(&app, account_id)?;
    // The LoggedOut event updates the state; pairing again is a separate
    // request (`evo_reconnect`).
    session.client.logout(&session.token).await?;
    Ok(())
}

#[tauri::command]
pub fn evo_chats(app: AppHandle, account_id: i64) -> CommandResult<Vec<ChatSummary>> {
    Ok(session(&app, account_id)?.store.chats()?)
}

#[tauri::command]
pub fn evo_messages(
    app: AppHandle,
    account_id: i64,
    chat: String,
    before: Option<i64>,
    limit: u32,
) -> CommandResult<Vec<ChatMessage>> {
    Ok(session(&app, account_id)?
        .store
        .messages(&chat, before, limit.clamp(1, PAGE_MAX))?)
}

/// The chat shown in the UI (no notifications for it while focused).
#[tauri::command]
pub fn evo_set_open_chat(
    app: AppHandle,
    account_id: i64,
    chat: Option<String>,
) -> CommandResult<()> {
    session(&app, account_id)?.set_open_chat(chat);
    Ok(())
}

/// Clear the chat's unread count and send read receipts.
#[tauri::command]
pub async fn evo_mark_read(app: AppHandle, account_id: i64, chat: String) -> CommandResult<()> {
    let session = session(&app, account_id)?;
    let ids = session.store.mark_read(&chat)?;
    evo::refresh_unread(&app, &session);
    let _ = app.emit(evo::EVOLUTION_EVENT, evo::UiEvent::Chats { account_id });
    if !ids.is_empty() {
        session
            .client
            .mark_read(&session.token, &chat, &ids)
            .await?;
    }
    Ok(())
}

#[tauri::command]
pub async fn evo_typing(
    app: AppHandle,
    account_id: i64,
    chat: String,
    composing: bool,
) -> CommandResult<()> {
    let session = session(&app, account_id)?;
    let state = if composing { "composing" } else { "paused" };
    session
        .client
        .presence(&session.token, &chat, state)
        .await?;
    Ok(())
}

/// The reply target of a send, with the kind and text shown above our copy.
fn quote_for(session: &Session, chat: &str, id: Option<&str>) -> CommandResult<Option<QuoteRef>> {
    let Some(id) = id.filter(|id| !id.is_empty()) else {
        return Ok(None);
    };
    let Some(quoted) = session.store.message(chat, id)? else {
        return Ok(None);
    };
    let participant = if quoted.from_me {
        session.own_jid()
    } else if quoted.sender.is_empty() {
        chat.to_owned()
    } else {
        quoted.sender
    };
    let text = if quoted.body.is_empty() && quoted.kind == MessageKind::Document {
        quoted.file_name
    } else {
        quoted.body
    };
    Ok(Some(QuoteRef {
        id: quoted.id,
        participant,
        kind: quoted.kind,
        text,
    }))
}

fn client_quote(quote: &QuoteRef) -> Quote {
    Quote {
        message_id: quote.id.clone(),
        participant: quote.participant.clone(),
    }
}

fn pending_message(
    chat: &str,
    kind: MessageKind,
    body: &str,
    file_name: &str,
    mime: &str,
) -> Message {
    Message {
        id: store::new_message_id(),
        chat: chat.to_owned(),
        sender: String::new(),
        from_me: true,
        ts: chrono::Utc::now().timestamp(),
        kind,
        body: body.to_owned(),
        file_name: file_name.to_owned(),
        mime: mime.to_owned(),
        push_name: String::new(),
        quote: None,
        status: Status::Pending,
        raw: serde_json::json!({"conversation": body}),
    }
}

/// Record the outcome of a send. A timeout is `Unknown`: it may have gone
/// out, so it is never retried automatically.
fn finish_send(
    app: &AppHandle,
    session: &Session,
    chat: &str,
    local: &str,
    result: Result<String, client::EvoError>,
) -> CommandResult<()> {
    let outcome = match result {
        Ok(server) => {
            session.store.confirm_sent(chat, local, &server)?;
            Ok(())
        }
        Err(client::EvoError::Ambiguous) => {
            session.store.set_status(chat, local, Status::Unknown)?;
            Err(tr("The server did not confirm the message; it may have been sent.").into())
        }
        Err(e) => {
            session.store.set_status(chat, local, Status::Failed)?;
            Err(e.into())
        }
    };
    let _ = app.emit(
        evo::EVOLUTION_EVENT,
        evo::UiEvent::Messages {
            account_id: session.account_id,
            chat: chat.to_owned(),
        },
    );
    // The chat list shows the ticks of our newest message.
    let _ = app.emit(
        evo::EVOLUTION_EVENT,
        evo::UiEvent::Chats {
            account_id: session.account_id,
        },
    );
    outcome
}

fn insert_pending(
    app: &AppHandle,
    session: &Session,
    mut message: Message,
    quote: Option<QuoteRef>,
) -> CommandResult<()> {
    message.quote = quote;
    let applied = session.store.apply(&[Change::New(message)], true, None)?;
    evo::after_apply(app, session, applied);
    Ok(())
}

#[tauri::command]
pub async fn evo_send_text(
    app: AppHandle,
    account_id: i64,
    chat: String,
    text: String,
    quote: Option<String>,
) -> CommandResult<()> {
    let text = text.trim_end().to_owned();
    if text.trim().is_empty() || text.len() > MAX_TEXT {
        return Err(tr("The message is empty or too long.").into());
    }
    let session = session(&app, account_id)?;
    let quote = quote_for(&session, &chat, quote.as_deref())?;
    let sent_quote = quote.as_ref().map(client_quote);
    let message = pending_message(&chat, MessageKind::Text, &text, "", "");
    let local = message.id.clone();
    insert_pending(&app, &session, message, quote)?;
    let result = session
        .client
        .send_text(&session.token, &chat, &text, &local, sent_quote.as_ref())
        .await;
    finish_send(&app, &session, &chat, &local, result)
}

/// Send a failed or unconfirmed text message again with the same id, so
/// WhatsApp can drop a duplicate if the first attempt did arrive.
#[tauri::command]
pub async fn evo_retry(
    app: AppHandle,
    account_id: i64,
    chat: String,
    id: String,
) -> CommandResult<()> {
    let session = session(&app, account_id)?;
    let Some(message) = session.store.message(&chat, &id)? else {
        return Err("message not found".into());
    };
    if !message.from_me
        || message.kind != MessageKind::Text
        || !matches!(message.status, Status::Failed | Status::Unknown)
    {
        return Err(tr("Only failed text messages can be sent again.").into());
    }
    let quote = message.quote.as_ref().map(|q| Quote {
        message_id: q.id.clone(),
        participant: q.participant.clone(),
    });
    // A receipt may have confirmed it meanwhile.
    if !session.store.set_status(&chat, &id, Status::Pending)? {
        return Err(tr("Only failed text messages can be sent again.").into());
    }
    let result = session
        .client
        .send_text(&session.token, &chat, &message.body, &id, quote.as_ref())
        .await;
    finish_send(&app, &session, &chat, &id, result)
}

fn media_kind(mime: &str) -> (MediaKind, MessageKind) {
    match mime.split('/').next() {
        Some("image") if mime != "image/svg+xml" && mime != "image/gif" => {
            (MediaKind::Image, MessageKind::Image)
        }
        Some("video") => (MediaKind::Video, MessageKind::Video),
        Some("audio") => (MediaKind::Audio, MessageKind::Audio),
        _ => (MediaKind::Document, MessageKind::Document),
    }
}

fn mime_for(path: &Path) -> &'static str {
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .map(str::to_ascii_lowercase)
        .unwrap_or_default();
    match ext.as_str() {
        "jpg" | "jpeg" => "image/jpeg",
        "png" => "image/png",
        "webp" => "image/webp",
        "gif" => "image/gif",
        "mp4" | "m4v" => "video/mp4",
        "3gp" => "video/3gpp",
        "mov" => "video/quicktime",
        "mp3" => "audio/mpeg",
        "ogg" | "opus" => "audio/ogg",
        "m4a" => "audio/mp4",
        "aac" => "audio/aac",
        "wav" => "audio/wav",
        "pdf" => "application/pdf",
        "txt" => "text/plain",
        "zip" => "application/zip",
        "doc" => "application/msword",
        "docx" => "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
        "xls" => "application/vnd.ms-excel",
        "xlsx" => "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
        "ppt" => "application/vnd.ms-powerpoint",
        "pptx" => "application/vnd.openxmlformats-officedocument.presentationml.presentation",
        _ => "application/octet-stream",
    }
}

/// Pick a file and send it; `false` when the picker was cancelled.
#[tauri::command]
pub async fn evo_send_file(
    app: AppHandle,
    account_id: i64,
    chat: String,
    caption: String,
    quote: Option<String>,
) -> CommandResult<bool> {
    use tauri_plugin_dialog::DialogExt;
    let session = session(&app, account_id)?;
    let Some(path) = app
        .dialog()
        .file()
        .set_title(tr("Attach file"))
        .blocking_pick_file()
        .and_then(|p| p.into_path().ok())
    else {
        return Ok(false);
    };
    if std::fs::metadata(&path)?.len() > MAX_UPLOAD {
        return Err(tr("The file is larger than 100 MB.").into());
    }
    let bytes = tokio::fs::read(&path).await?;
    let mime = mime_for(&path);
    let (media, kind) = media_kind(mime);
    let file_name = path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("file")
        .to_owned();
    let quote = quote_for(&session, &chat, quote.as_deref())?;
    let sent_quote = quote.as_ref().map(client_quote);
    let message = pending_message(&chat, kind, caption.trim(), &file_name, mime);
    let local = message.id.clone();
    insert_pending(&app, &session, message, quote)?;
    // Our copy serves as the media cache of the sent message.
    if let Ok(cached) = cache_bytes(&session, &chat, &local, &bytes, mime) {
        let _ = session.store.set_media_file(&chat, &local, &cached);
    }
    let upload = MediaUpload {
        kind: media,
        bytes,
        filename: file_name,
        mime: mime.to_owned(),
        caption: caption.trim().to_owned(),
    };
    let result = session
        .client
        .send_media(&session.token, &chat, upload, &local, sent_quote.as_ref())
        .await;
    finish_send(&app, &session, &chat, &local, result).map(|()| true)
}

#[tauri::command]
pub async fn evo_react(
    app: AppHandle,
    account_id: i64,
    chat: String,
    id: String,
    emoji: String,
) -> CommandResult<()> {
    let session = session(&app, account_id)?;
    let Some(target) = session.store.message(&chat, &id)? else {
        return Err("message not found".into());
    };
    let participant = if store::is_group(&chat) && !target.from_me {
        target.sender.clone()
    } else {
        String::new()
    };
    session
        .client
        .react(
            &session.token,
            &chat,
            &id,
            target.from_me,
            &participant,
            &emoji,
        )
        .await?;
    let applied = session.store.apply(
        &[Change::Reaction {
            chat: chat.clone(),
            target: id,
            sender: String::new(),
            emoji,
        }],
        false,
        None,
    )?;
    evo::after_apply(&app, &session, applied);
    Ok(())
}

fn own_message(session: &Session, chat: &str, id: &str) -> CommandResult<ChatMessage> {
    match session.store.message(chat, id)? {
        Some(m) if m.from_me && !m.revoked && m.status >= Status::Sent => Ok(m),
        _ => Err(tr("Only your own sent messages can be changed.").into()),
    }
}

#[tauri::command]
pub async fn evo_edit(
    app: AppHandle,
    account_id: i64,
    chat: String,
    id: String,
    text: String,
) -> CommandResult<()> {
    let session = session(&app, account_id)?;
    let message = own_message(&session, &chat, &id)?;
    if message.kind != MessageKind::Text || text.trim().is_empty() || text.len() > MAX_TEXT {
        return Err(tr("Only text messages can be edited.").into());
    }
    session
        .client
        .edit(&session.token, &chat, &id, &text)
        .await?;
    let applied = session.store.apply(
        &[Change::Edit {
            chat: chat.clone(),
            target: id,
            body: text,
        }],
        false,
        None,
    )?;
    evo::after_apply(&app, &session, applied);
    Ok(())
}

/// Delete for everyone (own messages only).
#[tauri::command]
pub async fn evo_delete(
    app: AppHandle,
    account_id: i64,
    chat: String,
    id: String,
) -> CommandResult<()> {
    let session = session(&app, account_id)?;
    own_message(&session, &chat, &id)?;
    session.client.delete(&session.token, &chat, &id).await?;
    let applied = session.store.apply(
        &[Change::Revoke {
            chat: chat.clone(),
            target: id,
        }],
        false,
        None,
    )?;
    evo::after_apply(&app, &session, applied);
    Ok(())
}

fn cache_bytes(
    session: &Session,
    chat: &str,
    id: &str,
    bytes: &[u8],
    mime: &str,
) -> std::io::Result<String> {
    let digest = Sha256::digest(format!("{chat}\0{id}"));
    let name = format!("{}.bin", hex::encode(&digest[..16]));
    std::fs::create_dir_all(&session.media_dir)?;
    std::fs::write(session.media_dir.join(&name), bytes)?;
    std::fs::write(session.media_dir.join(format!("{name}.type")), mime)?;
    Ok(name)
}

/// The message's media as `(mime, bytes)`, downloading it once.
async fn media_bytes(session: &Session, chat: &str, id: &str) -> CommandResult<(String, Vec<u8>)> {
    let Some((raw, file)) = session.store.media(chat, id)? else {
        return Err("message not found".into());
    };
    if !file.is_empty() && !file.contains(['/', '\\']) {
        let path = session.media_dir.join(&file);
        if let Ok(bytes) = tokio::fs::read(&path).await {
            let mime = std::fs::read_to_string(session.media_dir.join(format!("{file}.type")))
                .unwrap_or_else(|_| "application/octet-stream".into());
            return Ok((mime, bytes));
        }
    }
    if raw.is_null() {
        return Err(tr("This media is no longer available.").into());
    }
    let data_url = session.client.download_media(&session.token, &raw).await?;
    let (header, data) = data_url
        .split_once(',')
        .ok_or_else(|| tr("This media is no longer available."))?;
    let mime = header
        .trim_start_matches("data:")
        .split(';')
        .next()
        .filter(|m| !m.is_empty())
        .unwrap_or("application/octet-stream")
        .to_owned();
    let bytes = base64::engine::general_purpose::STANDARD.decode(data)?;
    if let Ok(name) = cache_bytes(session, chat, id, &bytes, &mime) {
        let _ = session.store.set_media_file(chat, id, &name);
    }
    Ok((mime, bytes))
}

/// Media as a data URL for display.
#[tauri::command]
pub async fn evo_media(
    app: AppHandle,
    account_id: i64,
    chat: String,
    id: String,
) -> CommandResult<String> {
    let session = session(&app, account_id)?;
    let (mime, bytes) = media_bytes(&session, &chat, &id).await?;
    Ok(format!(
        "data:{mime};base64,{}",
        base64::engine::general_purpose::STANDARD.encode(bytes)
    ))
}

/// Save a message's media where the user chooses.
#[tauri::command]
pub async fn evo_save_media(
    app: AppHandle,
    account_id: i64,
    chat: String,
    id: String,
) -> CommandResult<bool> {
    use tauri_plugin_dialog::DialogExt;
    let session = session(&app, account_id)?;
    let (_, bytes) = media_bytes(&session, &chat, &id).await?;
    let suggested = session
        .store
        .message(&chat, &id)?
        .map(|m| m.file_name)
        .filter(|n| !n.is_empty())
        .unwrap_or_else(|| id.clone());
    let Some(target) = app
        .dialog()
        .file()
        .set_file_name(&suggested)
        .blocking_save_file()
        .and_then(|p| p.into_path().ok())
    else {
        return Ok(false);
    };
    tokio::fs::write(target, bytes).await?;
    Ok(true)
}

/// Ask the phone for older messages of a chat; they arrive as a history sync.
#[tauri::command]
pub async fn evo_load_older(app: AppHandle, account_id: i64, chat: String) -> CommandResult<bool> {
    let session = session(&app, account_id)?;
    let Some(anchor) = session.store.oldest_anchor(&chat)? else {
        return Ok(false);
    };
    session
        .client
        .history_sync(&session.token, &anchor, 50)
        .await?;
    Ok(true)
}

#[tauri::command]
pub async fn evo_avatar(
    app: AppHandle,
    account_id: i64,
    jid: String,
) -> CommandResult<Option<String>> {
    Ok(session(&app, account_id)?.avatar(&jid).await)
}

#[tauri::command]
pub async fn evo_refresh_contacts(app: AppHandle, account_id: i64) -> CommandResult<()> {
    let session = session(&app, account_id)?;
    evo::load_contacts(&app, &session).await;
    Ok(())
}

#[derive(Debug, Clone, Serialize, TS)]
#[ts(export)]
pub struct EvolutionInfo {
    pub base_url: String,
    pub instance_id: String,
    pub instance_name: String,
    pub has_admin_key: bool,
}

/// Saved configuration for the settings dialog (no secrets).
#[tauri::command]
pub fn evo_info(
    state: State<'_, AppState>,
    account_id: i64,
) -> CommandResult<Option<EvolutionInfo>> {
    let account = state.accounts.get(account_id)?;
    Ok(account.evolution.map(|config| EvolutionInfo {
        base_url: config.base_url,
        instance_id: config.instance_id.unwrap_or_default(),
        instance_name: config.instance_name.unwrap_or_default(),
        has_admin_key: state
            .secrets
            .get(&secrets::evolution_admin_key(account_id))
            .ok()
            .flatten()
            .is_some(),
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn files_map_to_whatsapp_media_types() {
        assert_eq!(media_kind(mime_for(Path::new("a.JPG"))).0, MediaKind::Image);
        assert_eq!(
            media_kind(mime_for(Path::new("a.gif"))).0,
            MediaKind::Document
        );
        assert_eq!(
            media_kind(mime_for(Path::new("a.opus"))).0,
            MediaKind::Audio
        );
        assert_eq!(
            media_kind(mime_for(Path::new("a.mp4"))).1,
            MessageKind::Video
        );
        assert_eq!(
            media_kind(mime_for(Path::new("a.xyz"))).0,
            MediaKind::Document
        );
    }
}
