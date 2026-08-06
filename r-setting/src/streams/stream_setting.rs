use serde::{Deserialize, Serialize};

#[derive(Default, Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct StreamSetting {
    id: Option<u8>,

    is_active: Option<bool>,
    key: Option<String>,
    code: Option<String>,
    setting_code: Option<String>,
    setting_code_stream: Option<String>,
    channel: Option<String>,
    stream_filter: Option<String>,
    stream_filter_key: Option<String>,
}

impl StreamSetting {
    #[must_use]
    pub fn key(&self) -> Option<&str> {
        self.key.as_deref()
    }

    #[must_use]
    pub fn channel(&self) -> Option<&str> {
        self.channel.as_deref()
    }

    #[must_use]
    pub fn setting_code_stream(&self) -> Option<&str> {
        self.setting_code_stream.as_deref()
    }

    #[must_use]
    pub fn is_active(&self) -> bool {
        self.is_active.unwrap_or(false)
    }
}
