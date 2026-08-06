use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use r_config::config::WatchdogConfig;
use r_runtime_api::Runtime;
use serde::Serialize;
use tokio::sync::watch;
use tokio::task::JoinHandle;
use tokio::time::MissedTickBehavior;

#[derive(Clone)]
pub struct Watchdog {
    shared: Arc<Shared>,
}

struct Shared {
    cfg: WatchdogConfig,
    runtime: Arc<dyn Runtime>,
}

impl Watchdog {
    #[must_use]
    pub fn new(cfg: WatchdogConfig, runtime: Arc<dyn Runtime>) -> Watchdog {
        Watchdog {
            shared: Arc::new(Shared { cfg, runtime }),
        }
    }

    pub fn spawn(&self, mut shutdown: watch::Receiver<bool>) -> JoinHandle<()> {
        let shared = self.shared.clone();
        tokio::spawn(async move {
            let mut tick = tokio::time::interval(Duration::from_secs(shared.cfg.interval_secs));
            tick.set_missed_tick_behavior(MissedTickBehavior::Skip);
            loop {
                tokio::select! {
                    _ = tick.tick() => shared.tick_once().await,
                    _ = shutdown.changed() => break,
                }
            }
        })
    }
}

impl Shared {
    async fn tick_once(&self) {
        let payload = build_payload(&self.cfg.setting_code, &self.cfg.key, now_ns());
        if let Err(e) = self
            .runtime
            .invoke_raw(&self.cfg.module_name, payload)
            .await
        {
            tracing::warn!(module = %self.cfg.module_name, error = %e, "watchdog tick failed");
        }
    }
}

fn now_ns() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos() as u64)
        .unwrap_or(0)
}

#[derive(Serialize)]
struct Tick<'a> {
    setting_code: &'a str,
    key: &'a str,
    now_ns: u64,
}

fn build_payload(setting_code: &str, key: &str, now_ns: u64) -> Vec<u8> {
    sonic_rs::to_vec(&Tick {
        setting_code,
        key,
        now_ns,
    })
    .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::build_payload;
    use sonic_rs::JsonValueTrait;

    #[test]
    fn payload_is_well_formed_json() {
        let bytes = build_payload("sc", "bybit_orderbook0", 42);
        let v: sonic_rs::Value = sonic_rs::from_slice(&bytes).expect("valid json");
        assert_eq!(v["setting_code"].as_str(), Some("sc"));
        assert_eq!(v["now_ns"].as_u64(), Some(42));
    }

    #[test]
    fn key_with_quotes_does_not_break_payload() {
        let bytes = build_payload("sc", r#"a"b\c"#, 1);
        let v: sonic_rs::Value = sonic_rs::from_slice(&bytes).expect("valid json");
        assert_eq!(v["key"].as_str(), Some(r#"a"b\c"#));
    }
}
