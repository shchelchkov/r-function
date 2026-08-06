use crate::functions::functions::FunctionSetting;
use moka::future::Cache;
use std::sync::Arc;
use std::time::Duration;

pub type Db = Arc<Cache<String, FunctionSetting>>;

pub fn init() -> Db {
    Arc::new(
        Cache::builder()
            .max_capacity(1000)
            .time_to_live(Duration::from_secs(60))
            .build(),
    )
}

pub async fn get_function_setting(
    db: &Db,
    setting_code: &str,
) -> Result<FunctionSetting, Box<dyn std::error::Error>> {
    if let Some(function_setting) = db.get(setting_code).await {
        return Ok(function_setting);
    }

    let function_setting = FunctionSetting::default();
    Ok(function_setting)
}

pub async fn put_function_setting(
    db: &Db,
    setting_code: String,
    function_setting: FunctionSetting,
) -> Result<bool, Box<dyn std::error::Error>> {
    db.insert(setting_code, function_setting).await;
    Ok(true)
}
