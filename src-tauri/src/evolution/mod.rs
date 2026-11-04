//! Evolution Go account mode: a native chat client over an Evolution Go
//! instance (REST + WebSocket) instead of a web.whatsapp.com page.
//!
//! Each enabled account runs one session: make sure the instance is connected
//! with the WebSocket on, keep the socket open, and after every (re)connect
//! read the server's private journal from the saved cursor so events missed
//! while offline are recovered. Changes land in the account's `ChatStore`;
//! unread counts and notifications go through the same pipeline as WhatsApp
//! Web pages.

pub mod client;
pub mod model;
pub mod store;
pub mod ws;

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager};
use tokio::sync::{mpsc, watch};
use ts_rs::TS;

use crate::accounts::{Account, AccountKind};
use crate::app::{effects, state::AppState};
use crate::i18n::tr;
use crate::notifications::policy::WebNotification;
use crate::secrets;
use client::{AdminKey, Client, EvoError, InstanceToken, QrCode};
use model::{Event, Link, Message, MessageKind};
use store::{Applied, ChatStore};

pub const EVOLUTION_EVENT: &str = "evolution://event";

/// Live messages older than this (journal catch-up) are not notified.
const NOTIFY_MAX_AGE: i64 = 10 * 60;
const NOTIFY_MAX_BATCH: usize = 5;
/// Retry connecting the instance after WhatsApp dropped it.
const RECONNECT_DELAY: Duration = Duration::from_secs(10);
const AVATAR_TTL: Duration = Duration::from_secs(6 * 3600);
const ARCHIVED_REFRESH: Duration = Duration::from_secs(5 * 60);
const JOURNAL_MAX_PAGES: usize = 200;
/// Store meta keys tying local data to its source.
const BIND_SERVER: &str = "bind:server";
const BIND_OWNER: &str = "bind:owner";
const JOURNAL_PAUSE: Duration = Duration::from_secs(1);
const JOURNAL_RETRY: Duration = Duration::from_secs(30);
const JOURNAL_RETRY_MAX: Duration = Duration::from_secs(30 * 60);

#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[serde(tag = "state", rename_all = "snake_case")]
#[ts(export)]
pub enum Connection {
    /// No session: disabled, unconfigured or missing keyring secrets.
    Stopped {
        reason: String,
    },
    Connecting,
    /// WhatsApp is not linked yet: show QR / phone pairing.
    Pairing,
    Online,
    /// The server or WhatsApp is unreachable; retrying.
    Offline {
        reason: String,
    },
    LoggedOut,
    Banned,
}

#[derive(Debug, Clone, Serialize, TS)]
#[serde(tag = "type", rename_all = "snake_case")]
#[ts(export, rename = "EvolutionEvent")]
pub enum UiEvent {
    Connection {
        #[ts(type = "number")]
        account_id: i64,
        connection: Connection,
    },
    Chats {
        #[ts(type = "number")]
        account_id: i64,
    },
    Messages {
        #[ts(type = "number")]
        account_id: i64,
        chat: String,
    },
    Typing {
        #[ts(type = "number")]
        account_id: i64,
        chat: String,
        sender: String,
        state: String,
    },
    Qr {
        #[ts(type = "number")]
        account_id: i64,
        qr: QrCode,
    },
    Sync {
        #[ts(type = "number")]
        account_id: i64,
        #[ts(type = "number | null")]
        progress: Option<i64>,
    },
    OpenChat {
        #[ts(type = "number")]
        account_id: i64,
        chat: String,
    },
}

pub struct Session {
    pub account_id: i64,
    pub client: Client,
    pub instance_id: String,
    admin: AdminKey,
    pub token: InstanceToken,
    pub store: ChatStore,
    pub media_dir: PathBuf,
    /// The linked phone's JID, for quoting our own messages.
    pub own_jid: Mutex<String>,
    connection: Mutex<Connection>,
    open_chat: Mutex<Option<String>>,
    journal: AtomicBool,
    journal_reader: tokio::sync::Mutex<()>,
    journal_failures: AtomicU32,
    contacts_loaded: AtomicBool,
    avatars: Mutex<HashMap<String, (Instant, Option<String>)>>,
    stop: watch::Sender<bool>,
    config: (String, String),
}

#[derive(Default)]
pub struct Evolution {
    sessions: Mutex<HashMap<i64, Arc<Session>>>,
    /// Held while starting or stopping sessions, so concurrent account
    /// changes never start two sessions for one account.
    lifecycle: Mutex<()>,
    notification_ids: AtomicU64,
}

impl Evolution {
    pub fn session(&self, account_id: i64) -> Option<Arc<Session>> {
        self.sessions.lock().unwrap().get(&account_id).cloned()
    }

    pub fn connection(&self, account_id: i64) -> Connection {
        match self.session(account_id) {
            Some(s) => s.connection(),
            None => Connection::Stopped {
                reason: "not running".into(),
            },
        }
    }
}

impl Session {
    pub fn connection(&self) -> Connection {
        self.connection.lock().unwrap().clone()
    }

    pub fn set_open_chat(&self, chat: Option<String>) {
        *self.open_chat.lock().unwrap() = chat;
    }

    pub fn own_jid(&self) -> String {
        self.own_jid.lock().unwrap().clone()
    }

    /// Profile picture URL, cached for a few hours.
    pub async fn avatar(&self, jid: &str) -> Option<String> {
        if let Some((at, url)) = self.avatars.lock().unwrap().get(jid)
            && at.elapsed() < AVATAR_TTL
        {
            return url.clone();
        }
        let url = self.client.avatar(&self.token, jid).await.ok()?;
        self.avatars
            .lock()
            .unwrap()
            .insert(jid.to_owned(), (Instant::now(), url.clone()));
        url
    }
}

fn emit(app: &AppHandle, event: UiEvent) {
    let _ = app.emit(EVOLUTION_EVENT, event);
}

fn set_connection(app: &AppHandle, session: &Session, connection: Connection) {
    {
        let mut current = session.connection.lock().unwrap();
        if *current == connection {
            return;
        }
        *current = connection.clone();
    }
    emit(
        app,
        UiEvent::Connection {
            account_id: session.account_id,
            connection,
        },
    );
}

fn reason(err: &EvoError) -> String {
    err.to_string()
}

/// Start, restart or stop sessions to match the enabled Evolution accounts.
pub fn sync(app: &AppHandle) {
    let evolution = app.state::<Evolution>();
    let _lifecycle = evolution.lifecycle.lock().unwrap();
    let state = app.state::<AppState>();
    let Ok(accounts) = state.accounts.list() else {
        return;
    };
    let wanted: Vec<&Account> = accounts
        .iter()
        .filter(|a| a.enabled && a.kind == AccountKind::EvolutionGo)
        .collect();
    let stale: Vec<i64> = {
        let sessions = evolution.sessions.lock().unwrap();
        sessions
            .iter()
            .filter(|(id, session)| {
                !wanted
                    .iter()
                    .any(|a| a.id == **id && config_key(a).as_ref() == Some(&session.config))
            })
            .map(|(id, _)| *id)
            .collect()
    };
    for id in stale {
        stop_session(app, id);
    }
    for account in wanted {
        if evolution.session(account.id).is_none() {
            start(app, account);
        }
    }
}

fn config_key(account: &Account) -> Option<(String, String)> {
    let config = account.evolution.as_ref()?;
    Some((config.base_url.clone(), config.instance_id.clone()?))
}

pub fn stop(app: &AppHandle, account_id: i64) {
    let evolution = app.state::<Evolution>();
    let _lifecycle = evolution.lifecycle.lock().unwrap();
    stop_session(app, account_id);
}

fn stop_session(app: &AppHandle, account_id: i64) {
    let removed = app
        .state::<Evolution>()
        .sessions
        .lock()
        .unwrap()
        .remove(&account_id);
    if let Some(session) = removed {
        let _ = session.stop.send(true);
    }
}

fn start(app: &AppHandle, account: &Account) {
    let id = account.id;
    let stopped = |reason: &str| {
        emit(
            app,
            UiEvent::Connection {
                account_id: id,
                connection: Connection::Stopped {
                    reason: reason.into(),
                },
            },
        );
    };
    let Some((base, instance_id)) = config_key(account) else {
        return stopped("no instance");
    };
    let state = app.state::<AppState>();
    let secret = |key: String| state.secrets.get(&key).ok().flatten();
    let (Some(admin), Some(token)) = (
        secret(secrets::evolution_admin_key(id)),
        secret(secrets::evolution_instance_token(id)),
    ) else {
        return stopped("missing credentials");
    };
    let built = client::parse_base_url(&base).and_then(Client::new);
    let Ok(client) = built else {
        return stopped("invalid server address");
    };
    let dir = state.paths.evolution_dir(id);
    let store = match ChatStore::open(&dir.join("chats.db")) {
        Ok(store) => store,
        Err(e) => {
            log::warn!("evolution store for account {id} failed to open: {e}");
            return stopped("local storage unavailable");
        }
    };
    let media_dir = dir.join("media");
    // Chats of another server or instance must not show up under this one.
    match store.bind(BIND_SERVER, &format!("{base}|{instance_id}"), &[]) {
        Ok(true) => clear_media(&media_dir),
        Ok(false) => {}
        Err(e) => {
            log::warn!("evolution store for account {id} failed to open: {e}");
            return stopped("local storage unavailable");
        }
    }
    let (stop_tx, stop_rx) = watch::channel(false);
    let session = Arc::new(Session {
        account_id: id,
        client,
        instance_id: instance_id.clone(),
        admin: AdminKey(admin),
        token: InstanceToken(token),
        store,
        media_dir,
        own_jid: Mutex::new(String::new()),
        connection: Mutex::new(Connection::Connecting),
        open_chat: Mutex::new(None),
        journal: AtomicBool::new(true),
        journal_reader: tokio::sync::Mutex::new(()),
        journal_failures: AtomicU32::new(0),
        contacts_loaded: AtomicBool::new(false),
        avatars: Mutex::new(HashMap::new()),
        stop: stop_tx,
        config: (base, instance_id),
    });
    app.state::<Evolution>()
        .sessions
        .lock()
        .unwrap()
        .insert(id, session.clone());
    emit(
        app,
        UiEvent::Connection {
            account_id: id,
            connection: Connection::Connecting,
        },
    );
    refresh_unread(app, &session);
    tauri::async_runtime::spawn(run(app.clone(), session, stop_rx));
}

async fn run(app: AppHandle, session: Arc<Session>, mut stop: watch::Receiver<bool>) {
    let url = ws::ws_url(session.client.base(), &session.admin, &session.instance_id);
    let (tx, mut rx) = mpsc::unbounded_channel();
    tauri::async_runtime::spawn(ws::run(url, session.instance_id.clone(), tx, stop.clone()));
    // Archiving happens on the phone and arrives through no event.
    let mut archived = tokio::time::interval(ARCHIVED_REFRESH);
    // Stopping cancels the work in flight too: a deleted or reconfigured
    // account must not connect the instance or write its store afterwards.
    loop {
        let signal = tokio::select! {
            signal = rx.recv() => signal.map(Some),
            _ = archived.tick() => Some(None),
            _ = stop.wait_for(|s| *s) => None,
        };
        let Some(signal) = signal else {
            break;
        };
        let Some(signal) = signal else {
            tokio::select! {
                () = load_archived(&app, &session) => {}
                _ = stop.wait_for(|s| *s) => break,
            }
            continue;
        };
        tokio::select! {
            () = on_signal(&app, &session, signal) => {}
            _ = stop.wait_for(|s| *s) => break,
        }
    }
}

async fn on_signal(app: &AppHandle, session: &Session, signal: ws::Signal) {
    match signal {
        ws::Signal::Up => {
            ensure_connected(app, session).await;
            catch_up(app, session).await;
            load_archived(app, session).await;
        }
        ws::Signal::Down(why) => {
            let text = match why {
                "refused" => tr("The server refused the admin key."),
                _ => tr("The Evolution Go server is unreachable."),
            };
            set_connection(app, session, Connection::Offline { reason: text });
        }
        ws::Signal::Event(payload) => handle(app, session, &payload, true),
    }
}

/// Connect the instance when it is down or misconfigured, keeping its other
/// integrations, then report whether WhatsApp is linked.
pub async fn ensure_connected(
    app: &AppHandle,
    session: &Session,
) -> Option<client::ConnectionStatus> {
    let row = match session
        .client
        .instance(&session.admin, &session.instance_id)
        .await
    {
        Ok(row) => row,
        Err(e) => {
            set_connection(app, session, Connection::Offline { reason: reason(&e) });
            return None;
        }
    };
    *session.own_jid.lock().unwrap() = row.jid.clone();
    bind_owner(app, session, &row.jid);
    if row.needs_connect() {
        let payload = client::connect_payload(Some(&row));
        if let Err(e) = session.client.connect(&session.token, &payload).await {
            set_connection(app, session, Connection::Offline { reason: reason(&e) });
            return None;
        }
    }
    refresh_status(app, session).await
}

/// Linked and connected is online; linked but disconnected waits for the
/// server's own reconnect (or an explicit retry).
/// A different phone linked to the instance has its own chats and journal.
fn bind_owner(app: &AppHandle, session: &Session, jid: &str) {
    let user = jid.split('@').next().unwrap_or("");
    let owner = user.split(':').next().unwrap_or("");
    if owner.is_empty() {
        return;
    }
    match session.store.bind(BIND_OWNER, owner, &[BIND_SERVER]) {
        Ok(true) => {
            clear_media(&session.media_dir);
            session.contacts_loaded.store(false, Ordering::SeqCst);
            emit(
                app,
                UiEvent::Chats {
                    account_id: session.account_id,
                },
            );
            refresh_unread(app, session);
        }
        Ok(false) => {}
        Err(e) => log::warn!("evolution store write failed: {e}"),
    }
}

fn clear_media(dir: &std::path::Path) {
    if let Err(e) = std::fs::remove_dir_all(dir)
        && e.kind() != std::io::ErrorKind::NotFound
    {
        log::warn!("evolution media cache cleanup failed: {e}");
    }
}

async fn refresh_status(app: &AppHandle, session: &Session) -> Option<client::ConnectionStatus> {
    let status = session.client.status(&session.token).await;
    match &status {
        Ok(status) if status.logged_in && !status.connected => set_connection(
            app,
            session,
            Connection::Offline {
                reason: tr("WhatsApp disconnected from the server."),
            },
        ),
        Ok(status) if status.logged_in => {
            set_connection(app, session, Connection::Online);
            if !session.contacts_loaded.swap(true, Ordering::SeqCst) {
                load_contacts(app, session).await;
            }
        }
        Ok(_) => set_connection(app, session, Connection::Pairing),
        Err(e) => set_connection(app, session, Connection::Offline { reason: reason(e) }),
    }
    status.ok()
}

/// Contact names and group subjects (read-only).
pub async fn load_contacts(app: &AppHandle, session: &Session) {
    if let Ok(contacts) = session.client.contacts(&session.token).await {
        let pairs: Vec<_> = contacts.into_iter().map(|c| (c.jid, c.name)).collect();
        let _ = session.store.set_contacts(&pairs);
    }
    if let Ok(groups) = session.client.groups(&session.token).await {
        let pairs: Vec<_> = groups
            .into_iter()
            .filter(|g| !g.name.is_empty())
            .map(|g| (g.jid, g.name))
            .collect();
        let _ = session.store.set_chat_names(&pairs);
    }
    emit(
        app,
        UiEvent::Chats {
            account_id: session.account_id,
        },
    );
}

/// Archived chats (read-only); the list is left as is when the server
/// cannot tell.
pub async fn load_archived(app: &AppHandle, session: &Session) {
    let Ok(chats) = session.client.archived_chats(&session.token).await else {
        return;
    };
    if session.store.set_archived(&chats).unwrap_or(false) {
        emit(
            app,
            UiEvent::Chats {
                account_id: session.account_id,
            },
        );
        refresh_unread(app, session);
    }
}

/// Read the journal from the saved cursor. The cursor is stored with each
/// page's messages, so an interrupted catch-up resumes where it stopped.
async fn catch_up(app: &AppHandle, session: &Session) {
    if !session.journal.load(Ordering::SeqCst) {
        return;
    }
    // One reader at a time: socket reconnects and continuations overlap.
    let _reading = session.journal_reader.lock().await;
    for _ in 0..JOURNAL_MAX_PAGES {
        let Ok(cursor) = session.store.cursor() else {
            return;
        };
        let page = match session.client.chat_events(&session.token, &cursor).await {
            Ok(page) => {
                session.journal_failures.store(0, Ordering::SeqCst);
                page
            }
            Err(EvoError::NotFound) => {
                // The server predates the journal.
                session.journal.store(false, Ordering::SeqCst);
                return;
            }
            // 503 is both "PRIVATE_CHAT_HISTORY off" and a storage outage.
            Err(
                e @ (EvoError::Http { status: 503, .. } | EvoError::Network(_) | EvoError::Timeout),
            ) => {
                let failures = session.journal_failures.fetch_add(1, Ordering::SeqCst);
                let delay = JOURNAL_RETRY
                    .saturating_mul(1 << failures.min(5))
                    .min(JOURNAL_RETRY_MAX);
                log::warn!("evolution journal read failed, retrying in {delay:?}: {e}");
                spawn_catch_up(app, session.account_id, delay);
                return;
            }
            Err(e) => {
                log::warn!("evolution journal read failed: {e}");
                return;
            }
        };
        // Pairing and presence events are stale by now.
        let events: Vec<Event> = page
            .events
            .iter()
            .map(|entry| {
                let mut payload = entry.payload.clone();
                if let Some(map) = payload.as_object_mut() {
                    map.remove("instanceToken");
                }
                model::parse(&payload)
            })
            .filter(|event| {
                matches!(
                    event,
                    Event::Changes(_)
                        | Event::Receipt(_)
                        | Event::History(_)
                        | Event::PushName { .. }
                )
            })
            .collect();
        let next = (!page.next_cursor.is_empty()).then_some(page.next_cursor.as_str());
        match session.store.apply_journal(&events, next) {
            Ok(applied) => {
                // Push names change the list without touching a chat.
                if applied.chats.is_empty() && !events.is_empty() {
                    emit(
                        app,
                        UiEvent::Chats {
                            account_id: session.account_id,
                        },
                    );
                }
                after_apply(app, session, applied);
            }
            Err(e) => {
                log::warn!("evolution store write failed: {e}");
                return;
            }
        }
        if !page.has_more || next.is_none_or(|n| n == cursor) {
            return;
        }
    }
    // A long backlog: keep going from the saved cursor after a pause.
    spawn_catch_up(app, session.account_id, JOURNAL_PAUSE);
}

fn spawn_catch_up(app: &AppHandle, account_id: i64, delay: Duration) {
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(delay).await;
        if let Some(session) = app.state::<Evolution>().session(account_id) {
            catch_up(&app, &session).await;
        }
    });
}

fn handle(app: &AppHandle, session: &Session, payload: &serde_json::Value, live: bool) {
    let account_id = session.account_id;
    match model::parse(payload) {
        Event::Changes(changes) => match session.store.apply(&changes, live, None) {
            Ok(applied) => after_apply(app, session, applied),
            Err(e) => log::warn!("evolution store write failed: {e}"),
        },
        Event::Receipt(receipt) => {
            if let Ok(true) = session.store.apply_receipt(&receipt) {
                emit(
                    app,
                    UiEvent::Messages {
                        account_id,
                        chat: receipt.chat.clone(),
                    },
                );
                if receipt.read_self {
                    emit(app, UiEvent::Chats { account_id });
                    refresh_unread(app, session);
                }
            }
        }
        Event::History(history) => {
            if let Ok(applied) = session.store.apply_history(&history) {
                after_apply(app, session, applied);
            }
            emit(
                app,
                UiEvent::Sync {
                    account_id,
                    progress: history.progress,
                },
            );
        }
        Event::Typing {
            chat,
            sender,
            state,
        } => emit(
            app,
            UiEvent::Typing {
                account_id,
                chat,
                sender,
                state,
            },
        ),
        Event::Qr { image, code } => {
            set_connection(app, session, Connection::Pairing);
            emit(
                app,
                UiEvent::Qr {
                    account_id,
                    qr: QrCode {
                        qrcode: image,
                        code,
                        ..Default::default()
                    },
                },
            );
        }
        Event::Pairing | Event::Link(Link::Connected) => spawn_refresh(app, account_id, None),
        Event::Link(Link::Disconnected) => {
            set_connection(
                app,
                session,
                Connection::Offline {
                    reason: tr("WhatsApp disconnected from the server."),
                },
            );
            spawn_refresh(app, account_id, Some(RECONNECT_DELAY));
        }
        Event::Link(Link::LoggedOut) => {
            session.contacts_loaded.store(false, Ordering::SeqCst);
            set_connection(app, session, Connection::LoggedOut);
        }
        Event::Link(Link::Banned) => set_connection(app, session, Connection::Banned),
        Event::PushName { jid, name } => {
            if !jid.is_empty()
                && !name.is_empty()
                && session.store.set_push_name(&jid, &name).is_ok()
            {
                emit(app, UiEvent::Chats { account_id });
            }
        }
        Event::Picture { jid } => {
            session.avatars.lock().unwrap().remove(&jid);
        }
        Event::Presence { .. } | Event::Other => {}
    }
}

/// Re-check the instance from a fresh task (events are handled in order and
/// must not wait on HTTP).
pub fn spawn_refresh(app: &AppHandle, account_id: i64, delay: Option<Duration>) {
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        if let Some(delay) = delay {
            tokio::time::sleep(delay).await;
        }
        if let Some(session) = app.state::<Evolution>().session(account_id) {
            ensure_connected(&app, &session).await;
        }
    });
}

pub fn after_apply(app: &AppHandle, session: &Session, applied: Applied) {
    let account_id = session.account_id;
    if applied.chats.is_empty() {
        return;
    }
    emit(app, UiEvent::Chats { account_id });
    for chat in &applied.chats {
        emit(
            app,
            UiEvent::Messages {
                account_id,
                chat: chat.clone(),
            },
        );
    }
    refresh_unread(app, session);
    notify(app, session, &applied.incoming);
}

pub fn refresh_unread(app: &AppHandle, session: &Session) {
    let Ok(count) = session.store.unread_chats() else {
        return;
    };
    let state = app.state::<AppState>();
    if let Some(snapshot) = state
        .unread
        .set(session.account_id, u32::try_from(count).unwrap_or(u32::MAX))
    {
        effects::on_unread_changed(app, &snapshot);
    }
}

fn window_focused(app: &AppHandle) -> bool {
    crate::app::window::main_window(app)
        .is_some_and(|w| w.is_visible().unwrap_or(false) && w.is_focused().unwrap_or(false))
}

/// Text for a message in notifications and chat previews.
pub fn describe(kind: MessageKind, body: &str) -> String {
    let label = match kind {
        MessageKind::Text => return body.to_owned(),
        MessageKind::Image => tr("Photo"),
        MessageKind::Video => tr("Video"),
        MessageKind::Audio => tr("Audio"),
        MessageKind::Document => tr("Document"),
        MessageKind::Sticker => tr("Sticker"),
        MessageKind::Location => tr("Location"),
        MessageKind::Contact => tr("Contact"),
        MessageKind::Unsupported => tr("Unsupported message"),
    };
    if body.is_empty() {
        label
    } else {
        format!("{label}: {body}")
    }
}

fn notify(app: &AppHandle, session: &Session, incoming: &[Message]) {
    let now = chrono::Utc::now().timestamp();
    let open = session.open_chat.lock().unwrap().clone();
    let focused = window_focused(app);
    let active = app.state::<AppState>().webviews.active() == Some(session.account_id);
    let fresh = incoming.iter().filter(|m| {
        now - m.ts < NOTIFY_MAX_AGE
            && !(focused && active && open.as_deref() == Some(m.chat.as_str()))
    });
    for message in fresh.take(NOTIFY_MAX_BATCH) {
        let chat_name = session
            .store
            .display_name(&message.chat)
            .unwrap_or_default();
        let text = describe(message.kind, &message.body);
        let body = if store::is_group(&message.chat) {
            let sender = session
                .store
                .display_name(&message.sender)
                .unwrap_or_default();
            format!("{sender}: {text}")
        } else {
            text
        };
        let web = WebNotification {
            id: app
                .state::<Evolution>()
                .notification_ids
                .fetch_add(1, Ordering::Relaxed),
            title: chat_name,
            body,
            tag: message.chat.clone(),
            icon: None,
            silent: false,
        };
        let (app, account_id) = (app.clone(), session.account_id);
        tauri::async_runtime::spawn(async move {
            crate::notifications::notify(&app, account_id, web).await;
        });
    }
}

/// A notification of an Evolution account was clicked: open its chat.
pub fn notification_clicked(app: &AppHandle, account_id: i64, chat: &str) {
    if app.state::<Evolution>().session(account_id).is_some() && !chat.is_empty() {
        emit(
            app,
            UiEvent::OpenChat {
                account_id,
                chat: chat.to_owned(),
            },
        );
    }
}
