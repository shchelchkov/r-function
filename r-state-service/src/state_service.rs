use std::fmt::Write as _;
use std::sync::Arc;
use std::time::{Duration, Instant};

use r_config::config::RestoreServiceConfig;
use r_db::db::db::Database;
use sonic_rs::{JsonContainerTrait, JsonValueTrait, Value};
use tracing::{debug, info, warn};

pub const SNAPSHOT_CODE: &str = "values_snapshot";

type Error = Box<dyn std::error::Error + Send + Sync>;

type Answer = Vec<(String, Vec<Value>)>;

#[derive(Debug)]
pub struct StateService {
    host: String,
    port: u16,
    path: String,
    codes: Vec<String>,
    client: reqwest::Client,
}

impl StateService {
    pub fn new(cfg: &RestoreServiceConfig) -> Result<Self, String> {
        let bad = |why: &str| format!("values.restore_service.url {:?}: {why}", cfg.url);
        let rest = cfg
            .url
            .strip_prefix("http://")
            .ok_or_else(|| bad("only http:// is supported"))?;
        let (authority, path) = rest.split_once('/').unwrap_or((rest, ""));
        let (host, port) = match authority.rsplit_once(':') {
            Some((host, port)) => (host, port.parse().map_err(|_| bad("bad port"))?),
            None => (authority, 80),
        };
        if host.is_empty() {
            return Err(bad("no host"));
        }
        let path = path.trim_matches('/');
        let client = reqwest::Client::builder()
            .timeout(Duration::from_millis(cfg.timeout_ms.max(1)))
            .no_proxy()
            .build()
            .map_err(|e| bad(&e.to_string()))?;
        Ok(Self {
            host: host.to_owned(),
            port,
            path: if path.is_empty() {
                String::new()
            } else {
                format!("/{path}")
            },
            codes: cfg.setting_codes.clone(),
            client,
        })
    }

    pub fn url(&self) -> String {
        format!("http://{}:{}{}", self.host, self.port, self.path)
    }

    fn split<'k>(&self, full_key: &'k str) -> Option<(&str, &'k str)> {
        self.codes.iter().find_map(|code| {
            let key = full_key.strip_prefix(code.as_str())?.strip_prefix('.')?;
            (!key.is_empty()).then_some((code.as_str(), key))
        })
    }

    pub fn restore(&self, db: &Database, key: &str, at: Option<i64>) -> Option<Arc<Vec<Value>>> {
        let (code, k) = self.split(key)?;
        let started = Instant::now();
        let values = match self.fetch(code, k, at) {
            Ok(Some(values)) => values,
            Ok(None) => {
                info!(key, at, "values: no state in state service");
                return None;
            }
            Err(e) => {
                warn!(key, at, error = %e, "values: state service request failed");
                return None;
            }
        };
        let mut found = None;
        for (other, list) in values {
            if other == key {
                found = Some(list);
            } else if self.split(&other).is_some() {
                keep_in_snapshot(db, &other, list);
            }
        }
        match &found {
            Some(_) => {
                info!(key, at, elapsed = ?started.elapsed(), "values: restored from state service")
            }
            None => warn!(key, "values: state service answered without the key"),
        }
        found.map(Arc::new)
    }

    fn fetch(&self, code: &str, key: &str, at: Option<i64>) -> Result<Option<Answer>, Error> {
        let mut path = format!("{}/{}/{}", self.path, encode(code), encode(key));
        if let Some(at) = at {
            let _ = write!(path, "?date={at}");
        }
        debug!(key, at, path = %path, "values: state service request");
        let (status, body) = block_on(self.get(&path))?;
        match status {
            200 => {}
            404 => return Ok(None),
            _ => {
                let text = String::from_utf8_lossy(&body[..body.len().min(300)]).into_owned();
                return Err(format!("HTTP {status}: {text}").into());
            }
        }
        let doc: Value = sonic_rs::from_slice(&body)?;
        let values = doc
            .get("values")
            .and_then(|v| v.as_object())
            .ok_or("response without `values`")?;
        let mut out = Vec::with_capacity(values.len());
        for (name, list) in values.iter() {
            let list = list
                .as_array()
                .ok_or_else(|| format!("`values.{name}` is not an array"))?;
            out.push((name.to_owned(), list.iter().cloned().collect()));
        }
        Ok(Some(out))
    }

    async fn get(&self, path: &str) -> Result<(u16, Vec<u8>), Error> {
        let url = format!("http://{}:{}{}", self.host, self.port, path);

        let response = self.client.get(&url).send().await?;
        let status = response.status().as_u16();
        let body = response.bytes().await?.to_vec();

        Ok((status, body))
    }
}

fn block_on<T>(fut: impl Future<Output = Result<T, Error>>) -> Result<T, Error> {
    let handle = tokio::runtime::Handle::try_current()?;
    tokio::task::block_in_place(|| handle.block_on(fut))
}

fn keep_in_snapshot(db: &Database, key: &str, list: Vec<Value>) {
    let saved = match db.get_value(SNAPSHOT_CODE, key) {
        Ok(Some(_)) => return,
        Ok(None) => db.put_value(SNAPSHOT_CODE, key, Value::from(list)),
        Err(e) => Err(e),
    };
    if let Err(e) = saved {
        warn!(key, error = %e, "values: state from state service not kept in snapshot");
    }
}

fn encode(segment: &str) -> String {
    let mut out = String::with_capacity(segment.len());
    for b in segment.bytes() {
        if b.is_ascii_alphanumeric() || matches!(b, b'-' | b'.' | b'_' | b'~') {
            out.push(b as char);
        } else {
            let _ = write!(out, "%{b:02X}");
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use r_db::db::db::DatabaseOptions;
    use std::io::{Read, Write};
    use std::net::TcpListener;
    use std::thread::JoinHandle;

    const TOPIC: &str = "topic";

    fn open() -> (tempfile::TempDir, Database) {
        let dir = tempfile::tempdir().unwrap();
        let db = Database::open(
            dir.path(),
            DatabaseOptions {
                limit: 3,
                history: 3,
            },
        )
        .unwrap();
        (dir, db)
    }

    fn service(port: u16, timeout_ms: u64) -> StateService {
        StateService::new(&RestoreServiceConfig {
            url: format!("http://127.0.0.1:{port}/state/"),
            setting_codes: vec!["movement_zone".into(), "movement_zone_window".into()],
            timeout_ms,
        })
        .unwrap()
    }

    fn serve_once(status: &str, body: &str) -> (u16, JoinHandle<String>) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        let response = format!(
            "HTTP/1.0 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{body}",
            body.len()
        );
        let handle = std::thread::spawn(move || {
            let (mut conn, _) = listener.accept().unwrap();
            let mut request = Vec::new();
            let mut buf = [0u8; 1024];
            while !request.windows(4).any(|w| w == b"\r\n\r\n") {
                let n = conn.read(&mut buf).unwrap();
                request.extend_from_slice(&buf[..n]);
            }
            conn.write_all(response.as_bytes()).unwrap();
            String::from_utf8(request)
                .unwrap()
                .lines()
                .next()
                .unwrap()
                .to_owned()
        });
        (port, handle)
    }

    fn json(list: &Arc<Vec<Value>>) -> String {
        sonic_rs::to_string(&**list).unwrap()
    }

    fn snapshot(db: &Database, key: &str) -> Option<Arc<Vec<Value>>> {
        let list = db.get_value(SNAPSHOT_CODE, key).unwrap()?;
        let a = list.first()?.as_array()?;
        Some(Arc::new(a.iter().cloned().collect()))
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn restores_key_and_keeps_the_rest_of_the_answer_in_snapshot() {
        let (_dir, db) = open();
        let setting_code = "movement_zone";
        let body = format!(
            r#"{{"values":{{"{setting_code}.{TOPIC}":[{{"state":2}}],"movement_zone_window.{TOPIC}":[{{"ok":[1]}}],"order_book_depth.{TOPIC}":[{{"u":1}}]}},"as_of":900000}}"#
        );
        let (port, server) = serve_once("200 OK", &body);
        let got = service(port, 5_000)
            .restore(&db, &format!("{setting_code}.{TOPIC}"), None)
            .expect("из сервиса");
        assert_eq!(json(&got), r#"[{"state":2}]"#);
        assert_eq!(
            server.join().unwrap(),
            format!("GET /state/{setting_code}/{TOPIC} HTTP/1.1")
        );

        let window =
            snapshot(&db, &format!("movement_zone_window.{TOPIC}")).expect("окно — в снимке");
        assert_eq!(json(&window), r#"[{"ok":[1]}]"#);
        assert!(
            snapshot(&db, &format!("order_book_depth.{TOPIC}")).is_none(),
            "код не из setting_codes"
        );
        assert!(
            snapshot(&db, &format!("{setting_code}.{TOPIC}")).is_none(),
            "запрошенный ключ уходит в память"
        );
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn asks_on_the_date_of_the_frame() {
        let (_dir, db) = open();
        let body = format!(r#"{{"values":{{"movement_zone.{TOPIC}":[{{}}]}}}}"#);
        let (port, server) = serve_once("200 OK", &body);
        let key = format!("movement_zone.{TOPIC}");
        service(port, 5_000)
            .restore(&db, &key, Some(1_791_724_500_000))
            .expect("из сервиса");
        assert_eq!(
            server.join().unwrap(),
            format!("GET /state/movement_zone/{TOPIC}?date=1791724500000 HTTP/1.1")
        );
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn snapshot_is_newer_than_the_answer() {
        let (_dir, db) = open();
        let window = format!("movement_zone_window.{TOPIC}");
        db.put_value(
            SNAPSHOT_CODE,
            &window,
            sonic_rs::from_str(r#"[{"ok":[7]}]"#).unwrap(),
        )
        .unwrap();
        let body =
            format!(r#"{{"values":{{"movement_zone.{TOPIC}":[{{}}],"{window}":[{{"ok":[1]}}]}}}}"#);
        let (port, server) = serve_once("200 OK", &body);
        service(port, 5_000)
            .restore(&db, &format!("movement_zone.{TOPIC}"), None)
            .expect("из сервиса");
        server.join().unwrap();
        assert_eq!(json(&snapshot(&db, &window).unwrap()), r#"[{"ok":[7]}]"#);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn missing_state_and_failures_give_none() {
        let (_dir, db) = open();
        let key = format!("movement_zone.{TOPIC}");
        for (status, body) in [
            ("404 Not Found", r#"{"error":"no state"}"#),
            ("502 Bad Gateway", r#"{"error":"clickhouse: down"}"#),
            ("200 OK", "not json"),
            ("200 OK", r#"{"values":{}}"#),
        ] {
            let (port, server) = serve_once(status, body);
            assert!(
                service(port, 5_000).restore(&db, &key, None).is_none(),
                "{status} {body}"
            );
            server.join().unwrap();
        }
        assert!(
            db.entries_by_setting_code(SNAPSHOT_CODE)
                .unwrap()
                .is_empty()
        );
    }

    #[test]
    fn other_codes_are_not_requested() {
        let (_dir, db) = open();
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        let svc = service(listener.local_addr().unwrap().port(), 5_000);
        assert!(
            svc.restore(&db, &format!("order_book_depth.{TOPIC}"), Some(1))
                .is_none()
        );
        assert!(
            svc.restore(&db, "movement_zone_windowless.x", None)
                .is_none()
        );
        assert!(svc.restore(&db, "movement_zone.", None).is_none());
        let err = listener.accept().expect_err("соединений не было");
        assert_eq!(err.kind(), std::io::ErrorKind::WouldBlock);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn silent_service_times_out() {
        let (_dir, db) = open();
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        let started = Instant::now();
        assert!(
            service(port, 200)
                .restore(&db, &format!("movement_zone.{TOPIC}"), None)
                .is_none()
        );
        assert!(
            started.elapsed() < Duration::from_secs(3),
            "ждали {:?}",
            started.elapsed()
        );
        drop(listener);
    }

    #[test]
    fn url_and_path_encoding() {
        let svc = service(8230, 1);
        assert_eq!(svc.url(), "http://127.0.0.1:8230/state");
        let cfg = |url: &str| RestoreServiceConfig {
            url: url.into(),
            setting_codes: vec![],
            timeout_ms: 1,
        };
        assert_eq!(
            StateService::new(&cfg("http://ts-state")).unwrap().url(),
            "http://ts-state:80"
        );
        assert!(StateService::new(&cfg("https://ts-state/state")).is_err());
        assert!(StateService::new(&cfg("http://:8230/state")).is_err());
        assert_eq!(
            encode("orderbook.200.linear.bybit.BTCUSDT"),
            "orderbook.200.linear.bybit.BTCUSDT"
        );
        assert_eq!(encode("a/b c?"), "a%2Fb%20c%3F");
    }

    #[tokio::test(flavor = "multi_thread")]
    #[ignore]
    async fn live_service() {
        let url = std::env::var("STATE_SERVICE_URL").expect("STATE_SERVICE_URL");
        let topic = std::env::var("STATE_TOPIC").unwrap_or_else(|_| TOPIC.to_owned());
        let date = std::env::var("STATE_DATE")
            .ok()
            .map(|d| d.parse::<i64>().expect("STATE_DATE — мс UTC"));
        let svc = StateService::new(&RestoreServiceConfig {
            url,
            setting_codes: vec!["movement_zone".into(), "movement_zone_window".into()],
            timeout_ms: 60_000,
        })
        .unwrap();
        let (_dir, db) = open();
        let state = svc
            .restore(&db, &format!("movement_zone.{topic}"), date)
            .expect("состояние от сервиса");
        let window =
            snapshot(&db, &format!("movement_zone_window.{topic}")).expect("окно — в снимке");
        let field = |list: &Arc<Vec<Value>>, name: &str| {
            list[0].get(name).map(|v| v.to_string()).unwrap_or_default()
        };
        println!(
            "state {} last_bar {} ps {}; window {} bars",
            field(&state, "state"),
            field(&state, "last_bar"),
            field(&state, "ps"),
            window[0]
                .get("start")
                .and_then(|v| v.as_array())
                .map_or(0, |a| a.len())
        );
    }
}
