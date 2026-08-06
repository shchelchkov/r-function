use std::fmt::Write;
use std::sync::{Arc, PoisonError, RwLock};

use hmac::{Hmac, Mac};
use r_config::config::HttpAuthKind;
use sha2::Sha256;

use super::Credentials;
use super::scheme::{self, Auth, Part};

#[derive(Clone)]
pub struct CredSigner {
    kind: HttpAuthKind,
    credentials: Arc<RwLock<Credentials>>,
    recv_window: String,
}

#[derive(Debug)]
pub struct Signature {
    pub api_key: String,
    pub timestamp: String,
    pub recv_window: String,
    pub sign: String,
}

impl CredSigner {
    pub fn new(
        kind: HttpAuthKind,
        credentials: Arc<RwLock<Credentials>>,
        recv_window: u64,
    ) -> Self {
        Self {
            kind,
            credentials,
            recv_window: recv_window.to_string(),
        }
    }

    pub fn kind(&self) -> HttpAuthKind {
        self.kind
    }

    pub fn credentials(&self) -> &Arc<RwLock<Credentials>> {
        &self.credentials
    }

    pub fn api_key(&self) -> String {
        self.credentials
            .read()
            .unwrap_or_else(PoisonError::into_inner)
            .api_key
            .clone()
    }

    pub fn sign(&self, timestamp_ms: u64, payload: &str) -> Option<Signature> {
        let Auth::Hmac { prehash, .. } = scheme::auth(self.kind) else {
            return None;
        };
        Some(self.hmac(prehash, timestamp_ms, payload))
    }

    pub(super) fn hmac(&self, prehash: &[Part], timestamp_ms: u64, payload: &str) -> Signature {
        let timestamp = timestamp_ms.to_string();
        let credentials = self
            .credentials
            .read()
            .unwrap_or_else(PoisonError::into_inner);
        let mut mac = Hmac::<Sha256>::new_from_slice(credentials.api_secret.as_bytes())
            .expect("HMAC accepts a key of any length");
        for part in prehash {
            mac.update(match part {
                Part::Timestamp => timestamp.as_bytes(),
                Part::ApiKey => credentials.api_key.as_bytes(),
                Part::RecvWindow => self.recv_window.as_bytes(),
                Part::Payload => payload.as_bytes(),
            });
        }
        let sign =
            mac.finalize()
                .into_bytes()
                .iter()
                .fold(String::with_capacity(64), |mut s, b| {
                    let _ = write!(s, "{b:02x}");
                    s
                });
        Signature {
            api_key: credentials.api_key.clone(),
            timestamp,
            recv_window: self.recv_window.clone(),
            sign,
        }
    }
}
