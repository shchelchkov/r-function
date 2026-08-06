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
