use serde::Deserialize;

#[derive(Deserialize)]
pub struct FluxByMapQuery {
    pub setting_code: String,
}
