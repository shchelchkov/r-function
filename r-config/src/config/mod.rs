use std::collections::HashMap;
use std::path::Path;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct AppConfig {
    pub tokio: TokioConfig,
    pub function_config: FunctionConfig,
    pub kafka_consumer: KafkaConfig,
    pub kafka_producer: KafkaConfig,
    #[serde(default)]
    pub pipeline: PipelineConfig,
    #[serde(default)]
    pub watchdog: Option<WatchdogConfig>,
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
    pub git_repo_url: String,
    pub git_workdir: String,
    pub git_function_settings: String,
    pub git_stream_setting: String,
    pub git_catalog_setting: String,
    pub git_consumer_setting: String,
    pub git_function_value: String,
    pub git_wasm_path: String,
    pub git_revision: String,
    #[serde(default = "default_git_fetch_interval_secs")]
    pub git_fetch_interval_secs: u64,
}

fn default_git_fetch_interval_secs() -> u64 {
    30
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct TokioConfig {
    pub port: u16,
    #[serde(default = "default_function_prefix")]
    pub api_prefix: String,
    #[serde(default = "default_directory_prefix")]
    pub api_directory: String,
    #[serde(default)]
    pub worker_threads: Option<usize>,
}

fn default_function_prefix() -> String {
    "/api/functions".to_string()
}

fn default_directory_prefix() -> String {
    "/api/directory".to_string()
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
