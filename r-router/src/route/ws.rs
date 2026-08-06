use std::collections::HashMap;
use std::time::Duration;

use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::extract::{Query, State};
use axum::response::Response;
use bytes::Bytes;
use futures::{SinkExt, StreamExt};
use r_feed::{FeedEvent, FeedHub};
use serde::Deserialize;
use tokio::sync::{broadcast, mpsc, oneshot, watch};
use tracing::{debug, info};

use crate::route::state::WsState;

const MAX_SUBSCRIPTIONS: usize = 64;
const OUT_BUFFER: usize = 256;
const PING_INTERVAL: Duration = Duration::from_secs(30);

#[derive(Deserialize)]
pub struct WsQuery {
    channels: Option<String>,
}

#[derive(Deserialize)]
struct ClientMsg {
    #[serde(rename = "type")]
    kind: String,
    #[serde(default)]
    channels: Vec<String>,
}

pub async fn ws_handler(
    State(WsState { feed, shutdown }): State<WsState>,
    Query(q): Query<WsQuery>,
    ws: WebSocketUpgrade,
) -> Response {
    let initial = q.channels.as_deref().map(parse_csv).unwrap_or_default();
    ws.on_upgrade(move |socket| handle_socket(socket, feed, shutdown, initial))
}

fn parse_csv(s: &str) -> Vec<String> {
    s.split(',')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(String::from)
        .collect()
}

async fn handle_socket(
    socket: WebSocket,
    hub: FeedHub,
    mut shutdown: watch::Receiver<bool>,
    initial: Vec<String>,
) {
    let (mut sink, mut stream) = socket.split();
    let (out_tx, mut out_rx) = mpsc::channel::<Message>(OUT_BUFFER);
    let mut subs: HashMap<String, oneshot::Sender<()>> = HashMap::new();

    for ch in initial {
        subscribe(&hub, &out_tx, &mut subs, ch);
    }

    let mut ping = tokio::time::interval(PING_INTERVAL);
    ping.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

    loop {
        tokio::select! {
            Some(frame) = out_rx.recv() => {
                if sink.send(frame).await.is_err() {
                    break;
                }
            }
            incoming = stream.next() => match incoming {
                Some(Ok(Message::Text(text))) => on_control(text.as_str(), &hub, &out_tx, &mut subs),
                Some(Ok(Message::Close(_))) | None => break,
                Some(Ok(_)) => {} 
                Some(Err(_)) => break,
            },
            _ = ping.tick() => {
                if sink.send(Message::Ping(Bytes::new())).await.is_err() {
                    break;
                }
            }
            _ = shutdown.changed() => {
                let _ = sink.send(Message::Close(None)).await;
                break;
            }
        }
    }

    drop(subs);
    debug!("ws connection closed");
}

fn subscribe(
    hub: &FeedHub,
    out: &mpsc::Sender<Message>,
    subs: &mut HashMap<String, oneshot::Sender<()>>,
    channel: String,
) {
    info!("subscribe {channel}");
    if subs.contains_key(&channel) {
        return;
    }
    if subs.len() >= MAX_SUBSCRIPTIONS {
        let _ = out.try_send(text(error_frame(&format!(
            "subscription limit reached ({MAX_SUBSCRIPTIONS})"
        ))));
        return;
    }

    let rx = hub.subscribe(&channel);
    let (cancel_tx, cancel_rx) = oneshot::channel::<()>();
    let name = channel.clone();

    let _ = out.try_send(text(subscribed_frame(&name)));

    tokio::spawn(forward(hub.clone(), rx, out.clone(), name, cancel_rx));

    subs.insert(channel, cancel_tx);
}

async fn forward(
    hub: FeedHub,
    mut rx: broadcast::Receiver<FeedEvent>,
    out: mpsc::Sender<Message>,
    name: String,
    mut cancel: oneshot::Receiver<()>,
) {
    loop {
        tokio::select! {
            _ = &mut cancel => break, 
            res = rx.recv() => match res {
                Ok(ev) => {
                    if out.send(text(data_frame(&name, &ev))).await.is_err() {
                        break;
                    }
                }
                Err(broadcast::error::RecvError::Lagged(n)) => {
                    if out.send(text(lagged_frame(&name, n))).await.is_err() {
                        break;
                    }
                }
                Err(broadcast::error::RecvError::Closed) => break,
            }
        }
    }
    drop(rx);
    hub.gc(&name);
}

fn on_control(
    raw: &str,
    hub: &FeedHub,
    out: &mpsc::Sender<Message>,
    subs: &mut HashMap<String, oneshot::Sender<()>>,
) {
    let ClientMsg { kind, channels } = match sonic_rs::from_str(raw) {
        Ok(m) => m,
        Err(e) => {
            let _ = out.try_send(text(error_frame(&format!("invalid message: {e}"))));
            return;
        }
    };

    match kind.as_str() {
        "subscribe" => {
            for ch in channels {
                subscribe(hub, out, subs, ch);
            }
        }
        "unsubscribe" => {
            for ch in channels {
                subs.remove(&ch); 
                let _ = out.try_send(text(unsubscribed_frame(&ch)));
            }
        }
        "ping" => {
            let _ = out.try_send(text(PONG_FRAME.to_string()));
        }
        other => {
            let _ = out.try_send(text(error_frame(&format!("unknown type: {other}"))));
        }
    }
}

const PONG_FRAME: &str = r#"{"type":"pong"}"#;

fn text(s: String) -> Message {
    Message::Text(s.into())
}

fn subscribed_frame(channel: &str) -> String {
    format!(r#"{{"type":"subscribed","channel":{}}}"#, json_str(channel))
}

fn unsubscribed_frame(channel: &str) -> String {
    format!(
        r#"{{"type":"unsubscribed","channel":{}}}"#,
        json_str(channel)
    )
}

fn error_frame(message: &str) -> String {
    format!(r#"{{"type":"error","message":{}}}"#, json_str(message))
}

fn lagged_frame(channel: &str, skipped: u64) -> String {
    format!(
        r#"{{"type":"lagged","channel":{},"skipped":{skipped}}}"#,
        json_str(channel)
    )
}

fn data_frame(channel: &str, ev: &FeedEvent) -> String {
    let payload = std::str::from_utf8(&ev.payload).unwrap_or("null");
    format!(
        r#"{{"type":"data","channel":{},"payload":{payload}}}"#,
        json_str(channel)
    )
}

fn json_str(s: &str) -> String {
    sonic_rs::to_string(&s).unwrap_or_else(|_| "\"\"".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    fn ev(payload: &[u8]) -> FeedEvent {
        FeedEvent {
            channel: Arc::from("c"),
            key: None,
            payload: Bytes::copy_from_slice(payload),
        }
    }

    fn text_of(msg: Message) -> String {
        match msg {
            Message::Text(t) => t.as_str().to_string(),
            other => panic!("expected text frame, got {other:?}"),
        }
    }


    #[tokio::test]
    async fn forward_reports_lagged_then_resumes() {
        let (tx, rx) = broadcast::channel::<FeedEvent>(2);
        for i in 0..5u8 {
            let _ = tx.send(ev(&[b'0' + i]));
        }
        let (out_tx, mut out_rx) = mpsc::channel::<Message>(16);
        let (_cancel_tx, cancel_rx) = oneshot::channel::<()>();
        tokio::spawn(forward(
            FeedHub::new(),
            rx,
            out_tx,
            "c".to_string(),
            cancel_rx,
        ));

        let first = text_of(out_rx.recv().await.expect("frame"));
        assert!(first.contains(r#""type":"lagged""#), "{first}");
        assert!(first.contains(r#""channel":"c""#), "{first}");

        let second = text_of(out_rx.recv().await.expect("frame"));
        assert!(second.contains(r#""type":"data""#), "{second}");
    }

    #[tokio::test]
    async fn forward_delivers_data_verbatim() {
        let (tx, rx) = broadcast::channel::<FeedEvent>(4);
        let (out_tx, mut out_rx) = mpsc::channel::<Message>(4);
        let (_cancel_tx, cancel_rx) = oneshot::channel::<()>();
        tokio::spawn(forward(
            FeedHub::new(),
            rx,
            out_tx,
            "c".to_string(),
            cancel_rx,
        ));

        let _ = tx.send(ev(br#"{"x":1}"#));
        let frame = text_of(out_rx.recv().await.expect("frame"));
        assert!(frame.contains(r#""type":"data""#), "{frame}");
        assert!(frame.contains(r#""payload":{"x":1}"#), "{frame}");
    }

    #[tokio::test]
    async fn forward_exits_on_cancel() {
        let (_tx, rx) = broadcast::channel::<FeedEvent>(4);
        let (out_tx, _out_rx) = mpsc::channel::<Message>(4);
        let (cancel_tx, cancel_rx) = oneshot::channel::<()>();
        let handle = tokio::spawn(forward(
            FeedHub::new(),
            rx,
            out_tx,
            "c".to_string(),
            cancel_rx,
        ));

        drop(cancel_tx); 
        tokio::time::timeout(Duration::from_secs(1), handle)
            .await
            .expect("forward должен завершиться по cancel")
            .expect("join");
    }

    #[tokio::test]
    async fn forward_exits_when_bus_closed() {
        let (tx, rx) = broadcast::channel::<FeedEvent>(4);
        let (out_tx, _out_rx) = mpsc::channel::<Message>(4);
        let (_cancel_tx, cancel_rx) = oneshot::channel::<()>();
        let handle = tokio::spawn(forward(
            FeedHub::new(),
            rx,
            out_tx,
            "c".to_string(),
            cancel_rx,
        ));

        drop(tx); 
        tokio::time::timeout(Duration::from_secs(1), handle)
            .await
            .expect("forward должен завершиться по Closed")
            .expect("join");
    }


    #[test]
    fn parse_csv_trims_and_drops_empty() {
        assert_eq!(parse_csv("a,b,c"), ["a", "b", "c"]);
        assert_eq!(parse_csv(" a , ,b, "), ["a", "b"]);
        assert!(parse_csv("").is_empty());
        assert!(parse_csv("  ,  ").is_empty());
    }

    #[test]
    fn data_frame_embeds_payload_verbatim() {
        assert_eq!(
            data_frame("c", &ev(br#"[1,2,3]"#)),
            r#"{"type":"data","channel":"c","payload":[1,2,3]}"#
        );
    }

    #[test]
    fn data_frame_invalid_utf8_becomes_null() {
        let bad = FeedEvent {
            channel: Arc::from("c"),
            key: None,
            payload: Bytes::from_static(&[0xff, 0xfe]),
        };
        assert_eq!(
            data_frame("c", &bad),
            r#"{"type":"data","channel":"c","payload":null}"#
        );
    }

    #[test]
    fn channel_name_is_json_escaped() {
        assert_eq!(
            subscribed_frame(r#"a"b"#),
            r#"{"type":"subscribed","channel":"a\"b"}"#
        );
    }

    #[test]
    fn control_frames_shape() {
        assert_eq!(
            unsubscribed_frame("c"),
            r#"{"type":"unsubscribed","channel":"c"}"#
        );
        assert_eq!(
            lagged_frame("c", 7),
            r#"{"type":"lagged","channel":"c","skipped":7}"#
        );
        assert_eq!(error_frame("boom"), r#"{"type":"error","message":"boom"}"#);
    }
}
