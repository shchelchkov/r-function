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

#[cfg(test)]
mod tests {
    use super::*;

    const QUERY: &str = "category=inverse&symbol=BTCUSD";

    fn signer(credentials: Credentials) -> CredSigner {
        CredSigner::new(
            HttpAuthKind::BybitV5,
            Arc::new(RwLock::new(credentials)),
            5000,
        )
    }

    #[test]
    fn sign_matches_reference() {
        let sig = signer(Credentials::new("testkey123", "testsecret456"))
            .sign(1672280218882, QUERY)
            .unwrap();
        assert_eq!(sig.api_key, "testkey123");
        assert_eq!(sig.timestamp, "1672280218882");
        assert_eq!(sig.recv_window, "5000");
        assert_eq!(
            sig.sign,
            "9ab22ecb2a105f7a2a06eeee5753c0fb542cad77b7be9023122334931d92fa2e"
        );
    }

    #[test]
    fn rotated_credentials_apply_to_next_sign() {
        let signer = signer(Credentials::new("testkey123", "testsecret456"));
        *signer.credentials().write().unwrap() = Credentials::new("newkey", "newsecret");

        let sig = signer.sign(1672280218882, QUERY).unwrap();
        assert_eq!(sig.api_key, "newkey");
        assert_eq!(
            sig.sign,
            "68e2c12e69c976f86832af0f74d9e69ecb08b906e4dfc99ff73986a799fc3cb4"
        );
    }

    #[test]
    fn bearer_has_no_signature_only_the_key() {
        let signer = CredSigner::new(
            HttpAuthKind::Bearer,
            Arc::new(RwLock::new(Credentials::new("testkey123", ""))),
            5000,
        );
        assert!(signer.sign(1672280218882, QUERY).is_none());
        assert_eq!(signer.api_key(), "testkey123");
    }
}
