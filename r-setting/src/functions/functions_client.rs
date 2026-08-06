use anyhow::{Context, Result};
use reqwest::Client;
use tracing::{error, info};

use crate::functions::function_setting::FunctionSetting;

const SETTING_URL: &str =
    "http://ts-directory-setting:8080/api/directory/setting/function/fluxByMap";
const CATALOG_SETTING_PARAM: &str = "catalog_setting";

async fn get_function(catalog_setting: &str) -> Result<Vec<FunctionSetting>> {
    let client = Client::new();

    let url = format!(
        "{}?{}={}",
        SETTING_URL, CATALOG_SETTING_PARAM, catalog_setting
    );

    match fetch_setting(client, &url).await {
        Ok(settings) => {
            info!("Parsed response: {:?}", settings);
            Ok(settings)
        }
        Err(e) => {
            error!("Error fetching settings: {}", e);
            Err(e)
        }
    }
}

async fn fetch_setting(client: Client, url: &str) -> Result<Vec<FunctionSetting>> {
    let response = get_data(client, url)
        .await
        .context("Failed to send request to settings endpoint")?;

    let settings = response
        .json::<Vec<FunctionSetting>>()
        .await
        .context("Failed to deserialize settings response as JSON")?;

    Ok(settings)
}

async fn get_data(client: Client, url: &str) -> Result<reqwest::Response> {
    let response = client
        .get(url)
        .send()
        .await
        .context("Failed to send HTTP request")?;

    Ok(response)
}
