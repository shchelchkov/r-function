
use std::time::Duration;

use axum::Router;
use axum::routing::get;
use futures::stream::Stream;
use futures::{SinkExt, StreamExt};
use r_feed::FeedHub;
use r_router::route::state::WsState;
use r_router::route::ws::ws_handler;
use tokio::net::TcpListener;
use tokio::sync::watch;
use tokio_tungstenite::connect_async;
use tokio_tungstenite::tungstenite::Error as WsError;
use tokio_tungstenite::tungstenite::Message as TMessage;

async fn spawn_server(feed: FeedHub, shutdown: watch::Receiver<bool>) -> String {
    let app = Router::new()
        .route("/ws", get(ws_handler))
        .with_state(WsState { feed, shutdown });
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
    format!("ws://{addr}/ws")
}

async fn next_text<S>(ws: &mut S) -> String
where
    S: Stream<Item = Result<TMessage, WsError>> + Unpin,
{
    loop {
        let msg = tokio::time::timeout(Duration::from_secs(5), ws.next())
            .await
            .expect("timeout waiting for frame")
            .expect("stream ended")
            .expect("ws error");
        match msg {
            TMessage::Text(t) => return t.to_string(),
            TMessage::Ping(_) | TMessage::Pong(_) => continue,
            other => panic!("unexpected frame: {other:?}"),
        }
    }
}

async fn wait_subscribers(feed: &FeedHub, channel: &str) {
    for _ in 0..100 {
        if feed.has_subscribers(channel) {
            return;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    panic!("подписчик {channel} не появился");
}

async fn wait_no_subscribers(feed: &FeedHub, channel: &str) {
    for _ in 0..100 {
        if !feed.has_subscribers(channel) {
            return;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    panic!("подписчик {channel} должен был исчезнуть");
}

#[tokio::test]
async fn subscribe_via_query_then_receive_data() {
    let feed = FeedHub::new();
    let (_tx, rx) = watch::channel(false);
    let url = spawn_server(feed.clone(), rx).await;

    let (mut ws, _resp) = connect_async(format!("{url}?channels=test"))
        .await
        .expect("connect");

    let ack = next_text(&mut ws).await;
    assert!(ack.contains(r#""type":"subscribed""#), "ack: {ack}");
    assert!(ack.contains(r#""channel":"test""#), "ack: {ack}");

    feed.publish("test", None, br#"{"x":1}"#);
    let data = next_text(&mut ws).await;
    assert!(data.contains(r#""type":"data""#), "data: {data}");
    assert!(data.contains(r#""channel":"test""#), "data: {data}");
    assert!(data.contains(r#""payload":{"x":1}"#), "data: {data}");
}

#[tokio::test]
async fn dynamic_subscribe_then_unsubscribe_stops_delivery() {
    let feed = FeedHub::new();
    let (_tx, rx) = watch::channel(false);
    let url = spawn_server(feed.clone(), rx).await;
    let (mut ws, _resp) = connect_async(url).await.expect("connect");

    ws.send(TMessage::Text(
        r#"{"type":"subscribe","channels":["c1"]}"#.into(),
    ))
    .await
    .unwrap();
    let ack = next_text(&mut ws).await;
    assert!(
        ack.contains(r#""type":"subscribed""#) && ack.contains("c1"),
        "{ack}"
    );

    feed.publish("c1", None, br#"{"n":1}"#);
    let data = next_text(&mut ws).await;
    assert!(data.contains(r#""payload":{"n":1}"#), "{data}");

    ws.send(TMessage::Text(
        r#"{"type":"unsubscribe","channels":["c1"]}"#.into(),
    ))
    .await
    .unwrap();
    let un = next_text(&mut ws).await;
    assert!(
        un.contains(r#""type":"unsubscribed""#) && un.contains("c1"),
        "{un}"
    );

    let mut gone = false;
    for _ in 0..100 {
        if !feed.has_subscribers("c1") {
            gone = true;
            break;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    assert!(gone, "подписчик c1 должен исчезнуть после unsubscribe");
}

#[tokio::test]
async fn ping_gets_pong_and_invalid_message_gets_error() {
    let feed = FeedHub::new();
    let (_tx, rx) = watch::channel(false);
    let url = spawn_server(feed, rx).await;
    let (mut ws, _resp) = connect_async(url).await.expect("connect");

    ws.send(TMessage::Text(r#"{"type":"ping"}"#.into()))
        .await
        .unwrap();
    let pong = next_text(&mut ws).await;
    assert!(pong.contains(r#""type":"pong""#), "{pong}");

    ws.send(TMessage::Text("not json".into())).await.unwrap();
    let err = next_text(&mut ws).await;
    assert!(err.contains(r#""type":"error""#), "{err}");
}

#[tokio::test]
async fn shutdown_signal_closes_socket() {
    let feed = FeedHub::new();
    let (tx, rx) = watch::channel(false);
    let url = spawn_server(feed, rx).await;
    let (mut ws, _resp) = connect_async(format!("{url}?channels=test"))
        .await
        .expect("connect");

    let _ = next_text(&mut ws).await;

    tx.send(true).unwrap();

    let closed = tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            match ws.next().await {
                Some(Ok(TMessage::Close(_))) | None => return true,
                Some(Ok(_)) => continue,
                Some(Err(_)) => return true,
            }
        }
    })
    .await
    .expect("timeout waiting for close");
    assert!(closed);
}

#[tokio::test]
async fn subscribe_limit_exceeded_returns_error() {
    let feed = FeedHub::new();
    let (_tx, rx) = watch::channel(false);
    let url = spawn_server(feed, rx).await;
    let (mut ws, _resp) = connect_async(url).await.expect("connect");

    let channels: Vec<String> = (0..65).map(|i| format!("\"c{i}\"")).collect();
    let msg = format!(
        r#"{{"type":"subscribe","channels":[{}]}}"#,
        channels.join(",")
    );
    ws.send(TMessage::Text(msg.into())).await.unwrap();

    let mut acks = 0;
    let mut got_limit_error = false;
    for _ in 0..65 {
        let f = next_text(&mut ws).await;
        if f.contains(r#""type":"subscribed""#) {
            acks += 1;
        } else if f.contains(r#""type":"error""#) {
            assert!(f.contains("subscription limit reached (64)"), "{f}");
            got_limit_error = true;
            break;
        } else {
            panic!("неожиданный фрейм: {f}");
        }
    }
    assert_eq!(acks, 64, "должно быть ровно 64 успешные подписки");
    assert!(got_limit_error, "65-я подписка должна вернуть error лимита");
}

#[tokio::test]
async fn duplicate_subscribe_is_idempotent() {
    let feed = FeedHub::new();
    let (_tx, rx) = watch::channel(false);
    let url = spawn_server(feed, rx).await;
    let (mut ws, _resp) = connect_async(url).await.expect("connect");

    ws.send(TMessage::Text(
        r#"{"type":"subscribe","channels":["c1"]}"#.into(),
    ))
    .await
    .unwrap();
    let a1 = next_text(&mut ws).await;
    assert!(
        a1.contains(r#""type":"subscribed""#) && a1.contains("c1"),
        "{a1}"
    );

    ws.send(TMessage::Text(
        r#"{"type":"subscribe","channels":["c1","c2"]}"#.into(),
    ))
    .await
    .unwrap();
    let a2 = next_text(&mut ws).await;
    assert!(a2.contains(r#""type":"subscribed""#), "{a2}");
    assert!(
        a2.contains(r#""channel":"c2""#),
        "повторный c1 не должен слать ack; ждём c2: {a2}"
    );
}

#[tokio::test]
async fn unknown_message_type_returns_error() {
    let feed = FeedHub::new();
    let (_tx, rx) = watch::channel(false);
    let url = spawn_server(feed, rx).await;
    let (mut ws, _resp) = connect_async(url).await.expect("connect");

    ws.send(TMessage::Text(r#"{"type":"resubscribe"}"#.into()))
        .await
        .unwrap();
    let f = next_text(&mut ws).await;
    assert!(f.contains(r#""type":"error""#), "{f}");
    assert!(f.contains("unknown type: resubscribe"), "{f}");
}

#[tokio::test]
async fn channels_are_isolated_between_connections() {
    let feed = FeedHub::new();
    let (_tx, rx) = watch::channel(false);
    let url = spawn_server(feed.clone(), rx).await;
    let (mut ws, _resp) = connect_async(format!("{url}?channels=c1"))
        .await
        .expect("connect");
    let ack = next_text(&mut ws).await;
    assert!(
        ack.contains(r#""type":"subscribed""#) && ack.contains("c1"),
        "{ack}"
    );
    wait_subscribers(&feed, "c1").await;

    feed.publish("c2", None, br#"{"other":1}"#);
    feed.publish("c1", None, br#"{"mine":1}"#);

    let data = next_text(&mut ws).await;
    assert!(data.contains(r#""type":"data""#), "{data}");
    assert!(
        data.contains(r#""channel":"c1""#),
        "должен прийти только c1: {data}"
    );
    assert!(data.contains(r#""payload":{"mine":1}"#), "{data}");
}

#[tokio::test]
async fn fanout_multiple_connections_same_channel() {
    let feed = FeedHub::new();
    let (_tx, rx) = watch::channel(false);
    let url = spawn_server(feed.clone(), rx).await;

    let mut conns = Vec::new();
    for _ in 0..3 {
        let (mut ws, _resp) = connect_async(format!("{url}?channels=c1"))
            .await
            .expect("connect");
        let ack = next_text(&mut ws).await;
        assert!(ack.contains(r#""type":"subscribed""#), "{ack}");
        conns.push(ws);
    }

    feed.publish("c1", None, br#"{"n":1}"#);
    for ws in conns.iter_mut() {
        let data = next_text(ws).await;
        assert!(
            data.contains(r#""type":"data""#) && data.contains(r#""payload":{"n":1}"#),
            "{data}"
        );
    }
}

#[tokio::test]
async fn client_disconnect_triggers_gc() {
    let feed = FeedHub::new();
    let (_tx, rx) = watch::channel(false);
    let url = spawn_server(feed.clone(), rx).await;
    let (mut ws, _resp) = connect_async(format!("{url}?channels=c1"))
        .await
        .expect("connect");
    let ack = next_text(&mut ws).await;
    assert!(ack.contains(r#""type":"subscribed""#), "{ack}");
    assert!(feed.has_subscribers("c1"));

    drop(ws);

    wait_no_subscribers(&feed, "c1").await;
    assert_eq!(feed.channel_count(), 0, "канал c1 должен быть удалён gc");
}

#[tokio::test]
async fn binary_and_ping_frames_are_ignored() {
    let feed = FeedHub::new();
    let (_tx, rx) = watch::channel(false);
    let url = spawn_server(feed.clone(), rx).await;
    let (mut ws, _resp) = connect_async(format!("{url}?channels=c1"))
        .await
        .expect("connect");
    let ack = next_text(&mut ws).await;
    assert!(ack.contains(r#""type":"subscribed""#), "{ack}");
    wait_subscribers(&feed, "c1").await;

    ws.send(TMessage::Binary(vec![1, 2, 3].into()))
        .await
        .unwrap();
    ws.send(TMessage::Ping(vec![9].into())).await.unwrap();

    feed.publish("c1", None, br#"{"ok":1}"#);
    let data = next_text(&mut ws).await;
    assert!(
        data.contains(r#""type":"data""#) && data.contains(r#""payload":{"ok":1}"#),
        "{data}"
    );
}

#[tokio::test]
async fn raw_payload_passthrough_non_object() {
    let feed = FeedHub::new();
    let (_tx, rx) = watch::channel(false);
    let url = spawn_server(feed.clone(), rx).await;
    let (mut ws, _resp) = connect_async(format!("{url}?channels=c1"))
        .await
        .expect("connect");
    let _ = next_text(&mut ws).await; 
    wait_subscribers(&feed, "c1").await;

    feed.publish("c1", None, br#"[1,2,3]"#);
    let arr = next_text(&mut ws).await;
    assert!(
        arr.contains(r#""payload":[1,2,3]"#),
        "массив как payload: {arr}"
    );

    feed.publish("c1", None, b"42");
    let num = next_text(&mut ws).await;
    assert!(num.contains(r#""payload":42"#), "число как payload: {num}");
}

#[tokio::test]
async fn no_channels_connection_stays_open() {
    let feed = FeedHub::new();
    let (_tx, rx) = watch::channel(false);
    let url = spawn_server(feed, rx).await;
    let (mut ws, _resp) = connect_async(url).await.expect("connect");

    ws.send(TMessage::Text(r#"{"type":"ping"}"#.into()))
        .await
        .unwrap();
    let pong = next_text(&mut ws).await;
    assert!(pong.contains(r#""type":"pong""#), "{pong}");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore = "нагрузочный smoke: запускать через --ignored"]
async fn load_many_connections_receive_all() {
    const CONNS: usize = 50;
    const MSGS: usize = 1000;

    let feed = FeedHub::new();
    let (_tx, rx) = watch::channel(false);
    let url = spawn_server(feed.clone(), rx).await;

    let mut conns = Vec::with_capacity(CONNS);
    for _ in 0..CONNS {
        let (mut ws, _resp) = connect_async(format!("{url}?channels=load"))
            .await
            .expect("connect");
        let ack = next_text(&mut ws).await;
        assert!(ack.contains(r#""type":"subscribed""#), "{ack}");
        conns.push(ws);
    }

    for i in 0..MSGS {
        feed.publish("load", None, format!(r#"{{"i":{i}}}"#).as_bytes());
    }

    for (idx, ws) in conns.iter_mut().enumerate() {
        for expected in 0..MSGS {
            let data = next_text(ws).await;
            assert!(
                data.contains(r#""type":"data""#),
                "conn {idx}, сообщение {expected}: {data}"
            );
        }
    }
}
