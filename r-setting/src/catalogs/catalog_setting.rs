use serde::{Deserialize, Serialize};

#[derive(Default, Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct CatalogSetting {
    id: Option<u8>,

    code: Option<String>,
    routing_key: Option<String>,
    directory_code: Option<String>,
    label: Option<String>,

    setting_code: Option<String>,
    value_code: Option<String>,
    channel_stream: Option<String>,
    channel: Option<String>,
    validate_code: Option<String>,
    setting_code_list: Option<Vec<String>>,
    cache_is_active: Option<bool>,
    cache_name: Option<String>,
    cache_key: Option<Vec<String>>,
    cache_key_value: Option<Vec<String>>,
    cache_ttl: Option<u64>,
    cache_ttl_unit: Option<String>,
    only_function: Option<bool>,

    sort_list: Option<Vec<String>>,
    workspace_code: Option<String>,
    is_active: Option<bool>,
}
