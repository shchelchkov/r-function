use axum::Json;
use axum::extract::State;
use axum::http::StatusCode;
use r_setting::git::GitHandle;
use serde::Serialize;
use std::sync::Arc;

use crate::route::values::ApiResponse;

#[derive(Serialize)]
pub struct RefreshResponse {
    pub previous_head: String,
    pub current_head: String,
    pub changed: bool,
    pub changed_paths: Vec<String>,
}

#[derive(Serialize)]
pub struct HeadResponse {
    pub current_head: String,
    pub revision: String,
}

pub async fn post_git_refresh(
    State(git): State<Arc<GitHandle>>,
) -> Result<Json<ApiResponse<RefreshResponse>>, (StatusCode, String)> {
    let out = git
        .refresh()
        .await
        .map_err(|e| (StatusCode::BAD_GATEWAY, e.to_string()))?;
    Ok(Json(ApiResponse {
        data: RefreshResponse {
            previous_head: out.previous.to_string(),
            current_head: out.current.to_string(),
            changed: out.changed,
            changed_paths: out.changed_paths,
        },
    }))
}

pub async fn get_git_head(State(git): State<Arc<GitHandle>>) -> Json<ApiResponse<HeadResponse>> {
    let oid = **git.head.load();
    Json(ApiResponse {
        data: HeadResponse {
            current_head: oid.to_string(),
            revision: git.revision.to_string(),
        },
    })
}
