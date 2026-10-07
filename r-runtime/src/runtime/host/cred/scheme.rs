//! заголовков) или ключ как есть в `Authorization: Bearer`.
use r_config::config::HttpAuthKind;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Part {
    Timestamp,
    ApiKey,
    RecvWindow,
    Payload,
}

pub(super) struct Headers {
    pub api_key: &'static str,
    pub timestamp: &'static str,
    pub recv_window: &'static str,
    pub sign: &'static str,
}

pub(super) enum Auth {
            Hmac {
        prehash: &'static [Part],
        headers: Headers,
    },
        Bearer,
}

pub(super) fn auth(kind: HttpAuthKind) -> Auth {
    match kind {
        HttpAuthKind::BybitV5 => Auth::Hmac {
            prehash: &[
                Part::Timestamp,
                Part::ApiKey,
                Part::RecvWindow,
                Part::Payload,
            ],
            headers: Headers {
                api_key: "X-BAPI-API-KEY",
                timestamp: "X-BAPI-TIMESTAMP",
                recv_window: "X-BAPI-RECV-WINDOW",
                sign: "X-BAPI-SIGN",
            },
        },
        HttpAuthKind::Bearer => Auth::Bearer,
    }
}

pub(super) fn needs_secret(kind: HttpAuthKind) -> bool {
    matches!(auth(kind), Auth::Hmac { .. })
}
