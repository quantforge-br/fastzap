//! Live events over `GET /ws?token=<admin key>&instanceId=<id>`.
//!
//! The server keeps one socket per instance and has no replay, so the caller
//! catches up through the journal after each `Up`. The URL carries the admin
//! key: it is never logged, and connection errors are reduced to generic text.

use std::time::{Duration, Instant};

use futures_util::{SinkExt, StreamExt};
use serde_json::Value;
use tokio::sync::{mpsc, watch};
use tokio_tungstenite::tungstenite::Message as Frame;
use tokio_tungstenite::tungstenite::protocol::WebSocketConfig;
use url::Url;

use super::client::AdminKey;

/// Cloudflare closes idle sockets after 100 s.
const PING_EVERY: Duration = Duration::from_secs(20);
const BACKOFF_MIN: Duration = Duration::from_secs(1);
const BACKOFF_MAX: Duration = Duration::from_secs(60);
const CONNECT_TIMEOUT: Duration = Duration::from_secs(15);
/// No frame (not even a pong) for this long means the link is dead.
const SILENCE_LIMIT: Duration = Duration::from_secs(65);
const WRITE_TIMEOUT: Duration = Duration::from_secs(10);

#[derive(Debug)]
pub enum Signal {
    Up,
    /// Lost or refused; the loop retries after a delay.
    Down(&'static str),
    Event(Value),
}

pub fn ws_url(base: &Url, admin: &AdminKey, instance_id: &str) -> Url {
    let mut url = base.clone();
    let scheme = if base.scheme() == "http" { "ws" } else { "wss" };
    let _ = url.set_scheme(scheme);
    let path = format!("{}/ws", url.path().trim_end_matches('/'));
    url.set_path(&path);
    url.query_pairs_mut()
        .append_pair("token", admin.0.expose())
        .append_pair("instanceId", instance_id);
    url
}

/// `{queue, payload}` → the event, if it belongs to `instance_id`. The
/// instance token is removed before anything else sees it.
pub fn decode_frame(text: &str, instance_id: &str) -> Option<Value> {
    let outer: Value = serde_json::from_str(text).ok()?;
    let mut payload = match outer.get("payload") {
        Some(Value::String(inner)) => serde_json::from_str(inner).ok()?,
        Some(Value::Object(_)) => outer.get("payload").cloned()?,
        // Some builds send the event itself.
        _ if outer.get("event").is_some() => outer,
        _ => return None,
    };
    let map = payload.as_object_mut()?;
    map.remove("instanceToken");
    (map.get("instanceId").and_then(Value::as_str) == Some(instance_id)).then_some(payload)
}

fn next_backoff(current: Duration) -> Duration {
    (current * 2).min(BACKOFF_MAX)
}

/// Keep a socket open until `stop` turns true or `signals` is dropped.
pub async fn run(
    url: Url,
    instance_id: String,
    signals: mpsc::UnboundedSender<Signal>,
    mut stop: watch::Receiver<bool>,
) {
    let mut backoff = BACKOFF_MIN;
    while !*stop.borrow() {
        let config = WebSocketConfig::default()
            .max_message_size(None)
            .max_frame_size(None);
        let connect =
            tokio_tungstenite::connect_async_with_config(url.as_str(), Some(config), false);
        let reason = match tokio::time::timeout(CONNECT_TIMEOUT, connect).await {
            Ok(Ok((socket, _))) => {
                backoff = BACKOFF_MIN;
                if signals.send(Signal::Up).is_err() {
                    return;
                }
                session(socket, &instance_id, &signals, &mut stop).await
            }
            Ok(Err(tokio_tungstenite::tungstenite::Error::Http(response)))
                if matches!(response.status().as_u16(), 401 | 403) =>
            {
                "refused"
            }
            Ok(Err(_)) => "unreachable",
            Err(_) => "timeout",
        };
        if *stop.borrow() || signals.send(Signal::Down(reason)).is_err() {
            return;
        }
        tokio::select! {
            _ = tokio::time::sleep(backoff) => {}
            _ = stop.changed() => {}
        }
        backoff = next_backoff(backoff);
    }
}

async fn session<S>(
    socket: tokio_tungstenite::WebSocketStream<S>,
    instance_id: &str,
    signals: &mpsc::UnboundedSender<Signal>,
    stop: &mut watch::Receiver<bool>,
) -> &'static str
where
    S: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin,
{
    let (mut sink, mut stream) = socket.split();
    let mut ping = tokio::time::interval(PING_EVERY);
    ping.tick().await;
    let mut last_seen = Instant::now();
    let reason = loop {
        tokio::select! {
            _ = stop.changed() => break "stopped",
            _ = ping.tick() => {
                // Writes only fill local buffers on a silently dead network.
                if last_seen.elapsed() > SILENCE_LIMIT {
                    break "lost";
                }
                let sent = tokio::select! {
                    sent = tokio::time::timeout(WRITE_TIMEOUT, sink.send(Frame::Ping(Vec::new().into()))) => sent,
                    _ = stop.changed() => break "stopped",
                };
                if !matches!(sent, Ok(Ok(()))) {
                    break "lost";
                }
            }
            frame = stream.next() => match frame {
                Some(Ok(Frame::Text(text))) => {
                    last_seen = Instant::now();
                    if let Some(event) = decode_frame(&text, instance_id)
                        && signals.send(Signal::Event(event)).is_err()
                    {
                        break "stopped";
                    }
                }
                Some(Ok(Frame::Close(_))) | None => break "closed",
                Some(Ok(_)) => last_seen = Instant::now(),
                Some(Err(_)) => break "lost",
            },
        }
    };
    // Close before the next connect: the server drops the newer socket when
    // the old one's cleanup runs late.
    let close = async {
        let _ = sink.send(Frame::Close(None)).await;
        let _ = sink.close().await;
    };
    let _ = tokio::time::timeout(WRITE_TIMEOUT, close).await;
    reason
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::secrets::Secret;
    use serde_json::json;
    use tokio::net::TcpListener;

    #[test]
    fn urls_and_frames() {
        let admin = AdminKey(Secret::new("k&y"));
        let url = ws_url(
            &Url::parse("https://evo.example.com/p").unwrap(),
            &admin,
            "u1",
        );
        assert_eq!(
            url.as_str(),
            "wss://evo.example.com/p/ws?token=k%26y&instanceId=u1"
        );

        let inner =
            json!({"event": "Message", "instanceId": "u1", "instanceToken": "secret", "data": {}});
        let frame = json!({"queue": "u1.message", "payload": inner.to_string()}).to_string();
        let event = decode_frame(&frame, "u1").unwrap();
        assert!(event.get("instanceToken").is_none());
        assert!(!event.to_string().contains("secret"));
        assert!(decode_frame(&frame, "u2").is_none());
        assert!(decode_frame("not json", "u1").is_none());
        assert_eq!(next_backoff(Duration::from_secs(40)), BACKOFF_MAX);
    }

    #[tokio::test]
    async fn socket_delivers_events_and_reconnects() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move {
            for round in 0..2 {
                let (tcp, _) = listener.accept().await.unwrap();
                let mut ws = tokio_tungstenite::accept_async(tcp).await.unwrap();
                let big = "x".repeat(20 * 1024 * 1024);
                let payload = json!({"event": "HistorySync", "instanceId": "u1",
                    "instanceToken": "t", "data": {"round": round, "big": big}});
                let frame = json!({"queue": "q", "payload": payload.to_string()}).to_string();
                ws.send(Frame::Text(frame.into())).await.unwrap();
                let other = json!({"queue": "q", "payload": json!({"event": "Message", "instanceId": "u2"}).to_string()});
                ws.send(Frame::Text(other.to_string().into()))
                    .await
                    .unwrap();
                ws.close(None).await.unwrap();
            }
        });
        let base = Url::parse(&format!("http://{addr}")).unwrap();
        let url = ws_url(&base, &AdminKey(Secret::new("k")), "u1");
        let (tx, mut rx) = mpsc::unbounded_channel();
        let (stop_tx, stop_rx) = watch::channel(false);
        let task = tokio::spawn(run(url, "u1".into(), tx, stop_rx));

        let mut rounds = Vec::new();
        while rounds.len() < 2 {
            match tokio::time::timeout(Duration::from_secs(10), rx.recv())
                .await
                .unwrap()
                .unwrap()
            {
                Signal::Event(event) => {
                    assert!(event.get("instanceToken").is_none());
                    rounds.push(event["data"]["round"].as_i64().unwrap());
                }
                Signal::Up | Signal::Down(_) => {}
            }
        }
        assert_eq!(rounds, [0, 1]);
        stop_tx.send(true).unwrap();
        tokio::time::timeout(Duration::from_secs(5), task)
            .await
            .unwrap()
            .unwrap();
    }
}
