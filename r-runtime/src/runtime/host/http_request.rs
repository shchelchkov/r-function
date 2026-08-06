use async_trait::async_trait;
use r_error::runtime::error::RuntimeError;
use serde::Deserialize;

use super::HostFn;

pub struct HttpRequest {
    pub client: reqwest::Client,
}

#[derive(Deserialize)]
struct Req {
    method: String,
    url: String,
    body: Option<String>,
    content_type: Option<String>,
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

        let mut rb = self.client.request(method.clone(), &req.url);
        if let Some(ct) = &req.content_type {
            rb = rb.header(reqwest::header::CONTENT_TYPE, ct);
        }
        if let Some(b) = req.body {
            rb = rb.body(b);
        }

        tracing::debug!(%method, url = %req.url, "http_request");
        rb.send()
            .await
            .map_err(|e| RuntimeError::Internal(format!("http_request {method} {}: {e}", req.url)))?;

        Ok(None)
    }
}
