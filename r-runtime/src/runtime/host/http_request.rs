use std::collections::HashMap;

use async_trait::async_trait;
use r_config::config::HttpAuthConfig;
use r_error::runtime::error::RuntimeError;
use serde::{Deserialize, Serialize};

use super::HostFn;
use super::cred::CredRequestBuilder;

pub struct HttpRequest {
    client: reqwest::Client,
    auth: HashMap<String, CredRequestBuilder>,
}

impl HttpRequest {
            pub fn new(
        client: reqwest::Client,
        profiles: &HashMap<String, HttpAuthConfig>,
    ) -> Result<Self, RuntimeError> {
        let mut auth = HashMap::with_capacity(profiles.len());
        for (name, cfg) in profiles {
            auth.insert(name.clone(), CredRequestBuilder::from_config(name, cfg)?);
        }
        Ok(Self { client, auth })
    }
}

#[derive(Deserialize)]
struct Req {
    method: String,
    url: String,
    body: Option<String>,
    content_type: Option<String>,
        auth: Option<String>,
}

#[derive(Serialize)]
struct Resp {
    status: u16,
    body: String,
}

#[async_trait]
impl HostFn for HttpRequest {
    fn name(&self) -> &'static str {
        "http_request"
    }

    async fn call(&self, input: &[u8]) -> Result<Option<Vec<u8>>, RuntimeError> {
        let req: Req =
            sonic_rs::from_slice(input).map_err(|e| RuntimeError::Decode(e.to_string()))?;
        let method = reqwest::Method::from_bytes(req.method.as_bytes())
            .map_err(|e| RuntimeError::Decode(format!("bad method {}: {e}", req.method)))?;

        let mut rb = match &req.auth {
            None => {
                let rb = self.client.request(method.clone(), &req.url);
                match req.body {
                    Some(b) => rb.body(b),
                    None => rb,
                }
            }
            Some(name) => self
                .auth
                .get(name)
                .ok_or_else(|| RuntimeError::Decode(format!("unknown http_auth profile {name}")))?
                .build(&self.client, method.clone(), &req.url, req.body)?,
        };
        if let Some(ct) = &req.content_type {
            rb = rb.header(reqwest::header::CONTENT_TYPE, ct);
        }

        tracing::debug!(%method, url = %req.url, "http_request");
        let resp = rb.send().await.map_err(|e| {
            RuntimeError::Internal(format!("http_request {method} {}: {e}", req.url))
        })?;

        let status = resp.status().as_u16();
        let body = resp.text().await.map_err(|e| {
            RuntimeError::Internal(format!("http_request {method} {}: read body: {e}", req.url))
        })?;

        let bytes = sonic_rs::to_vec(&Resp { status, body })
            .map_err(|e| RuntimeError::Internal(e.to_string()))?;
        Ok(Some(bytes))
    }
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, RwLock};

    use reqwest::Url;
    use sonic_rs::JsonValueTrait;

    use super::*;
    use crate::runtime::host::cred::{CredSigner, Credentials};
    use crate::runtime::host::test_server::{header, serve_once};
    use r_config::config::HttpAuthKind;

    fn host(auth: HashMap<String, CredRequestBuilder>) -> HttpRequest {
        HttpRequest {
            client: reqwest::Client::new(),
            auth,
        }
    }

    #[tokio::test]
    async fn auth_profile_signs_the_request() {
        let body = r#"{"retCode":0,"retMsg":"OK","result":{"list":[]}}"#;
        let (base, rx) = serve_once(body);
        let credentials = Credentials::new("testkey123", "testsecret456");
        let builder = CredRequestBuilder::new(
            Url::parse(&base).unwrap(),
            CredSigner::new(
                HttpAuthKind::BybitV5,
                Arc::new(RwLock::new(credentials)),
                5000,
            ),
        );
        let out = host(HashMap::from([("bybit".to_string(), builder)]))
            .call(br#"{"method":"GET","url":"/v5/position/list?category=inverse&symbol=BTCUSD","auth":"bybit"}"#)
            .await
            .unwrap()
            .unwrap();

        let resp: sonic_rs::Value = sonic_rs::from_slice(&out).unwrap();
        assert_eq!(resp["status"].as_u64(), Some(200));
        assert_eq!(resp["body"].as_str(), Some(body));
        let (head, _) = rx.recv().unwrap();
        assert_eq!(header(&head, "x-bapi-api-key"), "testkey123");
    }

    #[tokio::test]
    async fn bearer_profile_posts_json() {
        let body = r#"{"output":"pong"}"#;
        let (base, rx) = serve_once(body);
        let credentials = Credentials::new("testkey123", "");
        let builder = CredRequestBuilder::new(
            Url::parse(&base).unwrap(),
            CredSigner::new(
                HttpAuthKind::Bearer,
                Arc::new(RwLock::new(credentials)),
                5000,
            ),
        );
        let out = host(HashMap::from([("typesafe".to_string(), builder)]))
            .call(br#"{"method":"POST","url":"/v1/systemone","body":"{\"input\":\"ping\"}","content_type":"application/json","auth":"typesafe"}"#)
            .await
            .unwrap()
            .unwrap();

        let resp: sonic_rs::Value = sonic_rs::from_slice(&out).unwrap();
        assert_eq!(resp["status"].as_u64(), Some(200));
        assert_eq!(resp["body"].as_str(), Some(body));
        let (head, sent) = rx.recv().unwrap();
        assert!(head.starts_with("POST /v1/systemone HTTP/1.1\r\n"));
        assert_eq!(header(&head, "authorization"), "Bearer testkey123");
        assert_eq!(header(&head, "content-type"), "application/json");
        assert_eq!(sent, r#"{"input":"ping"}"#);
    }

    #[tokio::test]
    async fn unknown_profile_is_an_error() {
        let err = host(HashMap::new())
            .call(br#"{"method":"GET","url":"/v5/position/list","auth":"bybit"}"#)
            .await
            .unwrap_err();
        assert!(err.to_string().contains("unknown http_auth profile bybit"));
    }
}
