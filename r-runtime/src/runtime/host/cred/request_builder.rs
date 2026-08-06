use std::sync::{Arc, RwLock};
use std::time::{SystemTime, UNIX_EPOCH};

use r_config::config::HttpAuthConfig;
use r_error::runtime::error::RuntimeError;
use reqwest::{Client, Method, RequestBuilder, Url};

use super::scheme::{self, Auth};
use super::{CredSigner, Credentials};

#[derive(Clone)]
pub struct CredRequestBuilder {
    base_url: Url,
    signer: CredSigner,
}

impl CredRequestBuilder {
    pub fn new(base_url: Url, signer: CredSigner) -> Self {
        Self { base_url, signer }
    }

    pub fn from_config(name: &str, cfg: &HttpAuthConfig) -> Result<Self, RuntimeError> {
        let env = |var: &str| {
            std::env::var(var)
                .map_err(|_| RuntimeError::Load(format!("http_auth {name}: env {var} is not set")))
        };
        let secret = if scheme::needs_secret(cfg.kind) {
            let var = cfg.api_secret_env.as_deref().ok_or_else(|| {
                RuntimeError::Load(format!(
                    "http_auth {name}: kind {:?} needs api_secret_env",
                    cfg.kind
                ))
            })?;
            env(var)?
        } else {
            String::new()
        };
        let credentials = Credentials::new(env(&cfg.api_key_env)?, secret);
        let base_url = Url::parse(&cfg.base_url).map_err(|e| {
            RuntimeError::Load(format!("http_auth {name}: base_url {}: {e}", cfg.base_url))
        })?;
        let signer = CredSigner::new(
            cfg.kind,
            Arc::new(RwLock::new(credentials)),
            cfg.recv_window,
        );
        Ok(Self::new(base_url, signer))
    }

    pub fn signer(&self) -> &CredSigner {
        &self.signer
    }

    pub fn build(
        &self,
        client: &Client,
        method: Method,
        url: &str,
        body: Option<String>,
    ) -> Result<RequestBuilder, RuntimeError> {
        let url = self.resolve(url)?;
        let rb = match scheme::auth(self.signer.kind()) {
            Auth::Bearer => client
                .request(method, url)
                .bearer_auth(self.signer.api_key()),
            Auth::Hmac { prehash, headers } => {
                let payload = if method == Method::GET {
                    url.query().unwrap_or("")
                } else {
                    body.as_deref().unwrap_or("")
                };
                let sig = self.signer.hmac(prehash, now_ms(), payload);
                client
                    .request(method, url)
                    .header(headers.api_key, sig.api_key)
                    .header(headers.timestamp, sig.timestamp)
                    .header(headers.recv_window, sig.recv_window)
                    .header(headers.sign, sig.sign)
            }
        };
        Ok(match body {
            Some(body) => rb.body(body),
            None => rb,
        })
    }

    fn resolve(&self, url: &str) -> Result<Url, RuntimeError> {
        let resolved = self
            .base_url
            .join(url)
            .map_err(|e| RuntimeError::Decode(format!("bad url {url}: {e}")))?;
        if resolved.origin() != self.base_url.origin() {
            return Err(RuntimeError::Decode(format!(
                "url {resolved} is outside http_auth base_url {}",
                self.base_url
            )));
        }
        Ok(resolved)
    }
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}
