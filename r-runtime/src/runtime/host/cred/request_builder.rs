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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::runtime::host::test_server::{header, serve_once};
    use r_config::config::HttpAuthKind;

    fn builder(base_url: &str) -> CredRequestBuilder {
        let credentials = Credentials::new("testkey123", "testsecret456");
        CredRequestBuilder::new(
            Url::parse(base_url).unwrap(),
            CredSigner::new(
                HttpAuthKind::BybitV5,
                Arc::new(RwLock::new(credentials)),
                5000,
            ),
        )
    }

    #[test]
    fn path_resolves_against_base_url() {
        let url = builder("https://api.bybit.com")
            .resolve("/v5/position/list?category=inverse&symbol=BTCUSD")
            .unwrap();
        assert_eq!(
            url.as_str(),
            "https://api.bybit.com/v5/position/list?category=inverse&symbol=BTCUSD"
        );
    }

    #[test]
    fn other_origin_is_not_signed() {
        let b = builder("https://api.bybit.com");
        for url in [
            "https://example.com/v5/position/list",
            "//example.com/v5/position/list",
            "http://api.bybit.com/v5/position/list",
        ] {
            assert!(
                b.build(&Client::new(), Method::GET, url, None).is_err(),
                "{url}"
            );
        }
    }

    #[tokio::test]
    async fn get_signs_the_query() {
        let (base, rx) = serve_once(r#"{"retCode":0}"#);
        let b = builder(&base);
        let url = "/v5/position/list?category=inverse&symbol=BTCUSD";
        b.build(&Client::new(), Method::GET, url, None)
            .unwrap()
            .send()
            .await
            .unwrap();

        let (head, _) = rx.recv().unwrap();
        assert!(head.starts_with(&format!("GET {url} HTTP/1.1\r\n")));
        let ts = header(&head, "x-bapi-timestamp").parse().unwrap();
        assert_eq!(header(&head, "x-bapi-api-key"), "testkey123");
        assert_eq!(header(&head, "x-bapi-recv-window"), "5000");
        assert_eq!(
            header(&head, "x-bapi-sign"),
            b.signer()
                .sign(ts, "category=inverse&symbol=BTCUSD")
                .unwrap()
                .sign
        );
    }

    #[tokio::test]
    async fn post_signs_the_body() {
        let (base, rx) = serve_once(r#"{"retCode":0}"#);
        let b = builder(&base);
        let body = r#"{"category":"linear","symbol":"BTCUSDT","side":"Buy","orderType":"Market","qty":"0.001"}"#;
        b.build(
            &Client::new(),
            Method::POST,
            "/v5/order/create",
            Some(body.into()),
        )
        .unwrap()
        .send()
        .await
        .unwrap();

        let (head, sent) = rx.recv().unwrap();
        assert!(head.starts_with("POST /v5/order/create HTTP/1.1\r\n"));
        assert_eq!(sent, body);
        let ts = header(&head, "x-bapi-timestamp").parse().unwrap();
        assert_eq!(
            header(&head, "x-bapi-sign"),
            b.signer().sign(ts, body).unwrap().sign
        );
    }

    #[tokio::test]
    async fn bearer_puts_the_key_into_authorization() {
        let (base, rx) = serve_once(r#"{"ok":true}"#);
        let credentials = Credentials::new("testkey123", "");
        let b = CredRequestBuilder::new(
            Url::parse(&base).unwrap(),
            CredSigner::new(
                HttpAuthKind::Bearer,
                Arc::new(RwLock::new(credentials)),
                5000,
            ),
        );
        let body = r#"{"input":"ping"}"#;
        b.build(
            &Client::new(),
            Method::POST,
            "/v1/systemone",
            Some(body.into()),
        )
        .unwrap()
        .send()
        .await
        .unwrap();

        let (head, sent) = rx.recv().unwrap();
        assert!(head.starts_with("POST /v1/systemone HTTP/1.1\r\n"));
        assert_eq!(header(&head, "authorization"), "Bearer testkey123");
        assert!(!head.to_ascii_lowercase().contains("x-bapi-"));
        assert_eq!(sent, body);
    }
}
