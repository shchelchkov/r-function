use serde::{Deserialize, Serialize};

#[derive(Default, Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct FunctionSetting {
    id: Option<u8>,
    code: Option<String>,
    setting_code: Option<String>,
    value_code: Option<String>,
    is_key: Option<bool>,
    key: Option<String>,
    target_key: Option<String>,
    mask: Option<String>,
    type_data: Option<String>,
    def_value: Option<String>,
    is_present: Option<bool>,
    is_attribute: Option<bool>,
    is_required: Option<bool>,
    is_active: Option<bool>,
    is_cached: Option<bool>,
    module: Option<Vec<String>>,
    formula: Option<Vec<String>>,
    function: Option<Vec<String>>,
    string_matcher: Option<String>,
    split: Option<String>,
    channel: Option<String>,
    check_attribute: Option<bool>,
}

impl FunctionSetting {
    #[must_use]
    pub fn key(&self) -> Option<&str> {
        self.key.as_deref()
    }

    #[must_use]
    pub fn type_data(&self) -> Option<&str> {
        self.type_data.as_deref()
    }

    #[must_use]
    pub fn def_value(&self) -> Option<&str> {
        self.def_value.as_deref()
    }

    #[must_use]
    pub fn module(&self) -> Option<&[String]> {
        self.module.as_deref()
    }

    #[must_use]
    pub fn formula(&self) -> Option<&[String]> {
        self.formula.as_deref()
    }

    #[must_use]
    pub fn function(&self) -> Option<&[String]> {
        self.function.as_deref()
    }

    #[must_use]
    pub fn is_key(&self) -> bool {
        self.is_key.unwrap_or(false)
    }

    #[must_use]
    pub fn is_active(&self) -> bool {
        self.is_active.unwrap_or(true)
    }
}
