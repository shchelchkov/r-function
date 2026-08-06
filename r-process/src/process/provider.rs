use std::sync::Arc;

use r_setting::functions::function_setting::FunctionSetting;
use r_setting::functions::functions::Function;

pub trait SettingProvider: Send + Sync {
    fn get_cached_setting(&self, setting_code: &str) -> Option<Arc<Vec<FunctionSetting>>>;

    fn get_function_setting(&self, setting_code: &str) -> Option<Arc<Vec<FunctionSetting>>>;

    fn get_value_key(&self, setting_code: &str) -> Option<Arc<Vec<String>>>;
}

impl SettingProvider for Function {
    fn get_cached_setting(&self, setting_code: &str) -> Option<Arc<Vec<FunctionSetting>>> {
        Function::get_cached_setting(self, setting_code)
    }

    fn get_function_setting(&self, setting_code: &str) -> Option<Arc<Vec<FunctionSetting>>> {
        Function::get_function_setting(self, setting_code)
    }

    fn get_value_key(&self, setting_code: &str) -> Option<Arc<Vec<String>>> {
        Function::get_value_key(self, setting_code)
    }
}
