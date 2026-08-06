use std::collections::HashMap;
use std::path::Path;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct AppConfig {
    pub tokio: TokioConfig,
    pub values: ValuesConfig,
    pub function_config: FunctionConfig,
    pub kafka_consumer: KafkaConfig,
    pub kafka_producer: KafkaConfig,
    #[serde(default)]
    pub pipeline: PipelineConfig,
    #[serde(default)]
    pub wasm: WasmConfig,
    #[serde(default)]
    pub watchdog: Vec<WatchdogConfig>,
    #[serde(default)]
    pub http_auth: HashMap<String, HttpAuthConfig>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct HttpAuthConfig {
    pub kind: HttpAuthKind,
    pub base_url: String,
    pub api_key_env: String,
    #[serde(default)]
    pub api_secret_env: Option<String>,
    #[serde(default = "default_recv_window")]
    pub recv_window: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum HttpAuthKind {
    BybitV5,
    Bearer,
}

fn default_recv_window() -> u64 {
    5000
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct WatchdogConfig {
    pub module_name: String,
    pub setting_code: String,
    pub interval_secs: u64,
    pub key: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct KafkaConfig {
    pub bootstrap_servers: String,
    pub client_id: String,
    pub group_id: String,
    #[serde(default)]
    pub topics: Vec<String>,
    #[serde(default)]
    pub dlq_topic: Option<String>,
    #[serde(default)]
    pub parameter: HashMap<String, String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct FunctionConfig {
    #[serde(default = "def_setting_code")]
    pub def_setting_code: String,
    pub git_repo_url: String,
    pub git_workdir: String,
    pub git_function_settings: String,
    pub git_stream_setting: String,
    pub git_catalog_setting: String,
    pub git_consumer_setting: String,
    pub git_function_value: String,
    pub git_wasm_path: String,
    #[serde(default = "plugin_path")]
    pub git_plugin_path: String,
    pub git_revision: String,
    #[serde(default = "default_git_fetch_interval_secs")]
    pub git_fetch_interval_secs: u64,
}

fn default_git_fetch_interval_secs() -> u64 {
    30
}

pub fn plugin_path() -> String {
    "plugin/function_settings".to_string()
}

pub fn def_setting_code() -> String {
    "directory_value".to_string()
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct TokioConfig {
    pub port: u16,
    #[serde(default = "default_function_prefix")]
    pub api_function: String,
    #[serde(default = "default_values_prefix")]
    pub api_values: String,
    #[serde(default = "default_directory_prefix")]
    pub api_directory: String,
    #[serde(default = "default_ws_prefix")]
    pub api_ws: String,
    #[serde(default)]
    pub worker_threads: Option<usize>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ValuesConfig {
    #[serde(default = "default_path")]
    pub path: String,
    #[serde(default = "default_limit")]
    pub limit: usize,
    #[serde(default = "default_history")]
    pub history: usize,
    #[serde(default = "default_fsync_ms")]
    pub fsync_ms: u64,
    #[serde(default)]
    pub persist_state: bool,
    #[serde(default = "default_snapshot_secs")]
    pub snapshot_secs: u64,
    #[serde(default)]
    pub restore_service: Option<RestoreServiceConfig>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct RestoreServiceConfig {
    pub url: String,
    pub setting_codes: Vec<String>,
    #[serde(default = "default_restore_timeout_ms")]
    pub timeout_ms: u64,
}

fn default_function_prefix() -> String {
    "/api/functions".to_string()
}

fn default_ws_prefix() -> String {
    "/api/ws".to_string()
}

fn default_path() -> String {
    "/data/values".to_string()
}

fn default_limit() -> usize {
    100
}

fn default_history() -> usize {
    1000
}

fn default_fsync_ms() -> u64 {
    1000
}

fn default_snapshot_secs() -> u64 {
    900
}

fn default_restore_timeout_ms() -> u64 {
    20_000
}

fn default_directory_prefix() -> String {
    "/api/directory".to_string()
}

fn default_values_prefix() -> String {
    "/api/values".to_string()
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct PipelineConfig {
    #[serde(default = "default_concurrency")]
    pub concurrency: usize,
    #[serde(default = "default_per_worker_queue")]
    pub per_worker_queue: usize,
    #[serde(default = "default_ingress_queue")]
    pub ingress_queue: usize,
    #[serde(default = "default_chunk_max")]
    pub chunk_max: usize,
}

fn default_concurrency() -> usize {
    8
}
fn default_per_worker_queue() -> usize {
    256
}
fn default_ingress_queue() -> usize {
    1024
}
fn default_chunk_max() -> usize {
    500
}

impl Default for PipelineConfig {
    fn default() -> Self {
        Self {
            concurrency: default_concurrency(),
            per_worker_queue: default_per_worker_queue(),
            ingress_queue: default_ingress_queue(),
            chunk_max: default_chunk_max(),
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct WasmConfig {
    #[serde(default = "default_wasm_memory_mib")]
    pub memory_mib: usize,
    #[serde(default = "default_wasm_stdout_mib")]
    pub stdout_mib: usize,
    #[serde(default = "default_wasm_timeout_ms")]
    pub timeout_ms: u64,
}

fn default_wasm_memory_mib() -> usize {
    64
}
fn default_wasm_stdout_mib() -> usize {
    4
}
fn default_wasm_timeout_ms() -> u64 {
    5_000
}

impl Default for WasmConfig {
    fn default() -> Self {
        Self {
            memory_mib: default_wasm_memory_mib(),
            stdout_mib: default_wasm_stdout_mib(),
            timeout_ms: default_wasm_timeout_ms(),
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error("yaml: {0}")]
    Yaml(#[from] serde_yaml_ng::Error),
}

impl AppConfig {
    pub fn from_file<P: AsRef<Path>>(path: P) -> Result<Self, ConfigError> {
        let text = std::fs::read_to_string(path)?;
        let cfg = serde_yaml_ng::from_str(&text)?;
        Ok(cfg)
    }
}

#[cfg(test)]
mod values_config_tests {
    use super::ValuesConfig;

    #[test]
    fn persist_state_is_off_by_default() {
        let cfg: ValuesConfig = serde_yaml_ng::from_str("path: /data/values/replay").unwrap();
        assert!(!cfg.persist_state);
    }

    #[test]
    fn persist_state_reads_from_yaml() {
        let cfg: ValuesConfig =
            serde_yaml_ng::from_str("path: /data/values/orderbook\npersist_state: true").unwrap();
        assert!(cfg.persist_state);
        assert_eq!(cfg.path, "/data/values/orderbook");
    }

    #[test]
    fn snapshot_every_15_min_by_default() {
        let cfg: ValuesConfig = serde_yaml_ng::from_str("persist_state: true").unwrap();
        assert_eq!(cfg.snapshot_secs, 900);
        let cfg: ValuesConfig =
            serde_yaml_ng::from_str("persist_state: true\nsnapshot_secs: 0").unwrap();
        assert_eq!(cfg.snapshot_secs, 0, "0 - только при остановке");
    }

    #[test]
    fn restore_service_is_optional() {
        let cfg: ValuesConfig = serde_yaml_ng::from_str("persist_state: true").unwrap();
        assert!(cfg.restore_service.is_none());

        let yaml = "persist_state: true\nrestore_service:\n  url: http://ts-state:8230/state\n  setting_codes: [movement_zone, movement_zone_window]\n";
        let rs = serde_yaml_ng::from_str::<ValuesConfig>(yaml)
            .unwrap()
            .restore_service
            .expect("задан");
        assert_eq!(rs.url, "http://ts-state:8230/state");
        assert_eq!(rs.setting_codes, ["movement_zone", "movement_zone_window"]);
        assert_eq!(rs.timeout_ms, 20_000);
    }
}

#[cfg(test)]
mod watchdog_config_tests {
    use super::AppConfig;

    #[test]
    fn root_configs_parse() {
        let root = concat!(env!("CARGO_MANIFEST_DIR"), "/..");
        let mut parsed = 0;
        for entry in std::fs::read_dir(root).unwrap() {
            let path = entry.unwrap().path();
            if path.extension().is_none_or(|e| e != "yaml") {
                continue;
            }
            if let Err(e) = AppConfig::from_file(&path) {
                panic!("{}: {e}", path.display());
            }
            parsed += 1;
        }
        assert!(parsed > 0, "в {root} нет *.yaml");
    }
}

#[cfg(test)]
mod wasm_config_tests {
    use super::WasmConfig;

    #[test]
    fn limits_default_to_64_mib_4_mib_5_s() {
        let cfg: WasmConfig = serde_yaml_ng::from_str("{}").unwrap();
        assert_eq!(
            (cfg.memory_mib, cfg.stdout_mib, cfg.timeout_ms),
            (64, 4, 5_000)
        );
        let cfg: WasmConfig = serde_yaml_ng::from_str("memory_mib: 512").unwrap();
        assert_eq!(
            (cfg.memory_mib, cfg.stdout_mib, cfg.timeout_ms),
            (512, 4, 5_000)
        );
    }
}
