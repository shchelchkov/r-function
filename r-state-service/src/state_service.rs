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

