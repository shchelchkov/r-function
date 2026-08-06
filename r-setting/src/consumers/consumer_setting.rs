use serde::{Deserialize, Serialize};

#[derive(Default, Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ConsumerSetting {
    id: Option<u8>,

    code: Option<String>,
    key: Option<String>,
    label: Option<String>,
    n_order: Option<u8>,
    setting_code: Option<String>,
    value_code: Option<String>,
    target_key: Option<String>,
    is_active: Option<bool>,
}
