use gix::ObjectId;
use gix::Repository;
use r_error::runtime::error::RuntimeError;
use serde::de::DeserializeOwned;
use sonic_rs::Value;
use tracing::error;
use tracing::info;

pub fn fetch_setting<T: DeserializeOwned>(
    repo: &Repository,
    spec: &str,
) -> Result<(ObjectId, Vec<T>), RuntimeError> {
    let oid = repo
        .rev_parse_single(spec)
        .map_err(|e| RuntimeError::Load(e.to_string()))?
        .detach();

    let blob = repo
        .find_object(oid)
        .map_err(|e| RuntimeError::Load(e.to_string()))?;

    let settings: Vec<T> =
        sonic_rs::from_slice(&blob.data).map_err(|e| RuntimeError::Load(e.to_string()))?;

    Ok((oid, settings))
}

pub fn fetch_setting_value(
    repo: &Repository,
    schema: &Value,
    spec: &str,
) -> Result<(ObjectId, Vec<Value>), RuntimeError> {
    let oid = repo
        .rev_parse_single(spec)
        .map_err(|e| RuntimeError::Load(e.to_string()))?
        .detach();

    let blob = repo
        .find_object(oid)
        .map_err(|e| RuntimeError::Load(e.to_string()))?;

    let elements: Vec<Value> = sonic_rs::from_slice(&blob.data).map_err(|e| {
        error!(error = %e, "fetch_setting_value parse json data failed");
        RuntimeError::Load(e.to_string())
    })?;
    info!("fetch_setting_value:::::::::::: 0001111 spec {:?}", &spec);

    let shaped = elements
        .into_iter()
        .map(|el| {
            let bytes = sonic_rs::to_vec(&el).map_err(|e| RuntimeError::Load(e.to_string()))?;
            sonic_rs::get_by_schema(&bytes[..], schema.clone()).map_err(|e| {
                error!(error = %e, "fetch_setting_value query json data failed");
                RuntimeError::Load(e.to_string())
            })
        })
        .collect::<Result<Vec<Value>, RuntimeError>>()?;
    info!("fetch_setting_value:::::::::::: 00011111 spec {:?}", &spec);

    Ok((oid, shaped))
}
