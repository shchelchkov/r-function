use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use r_value::value::value::ValueError;

pub enum ApiError {
    NotFound,
    Value(ValueError),
}

impl From<ValueError> for ApiError {
    fn from(err: ValueError) -> Self {
        Self::Value(err)
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        match self {
            Self::NotFound => StatusCode::NOT_FOUND.into_response(),

            Self::Value(err) => {
                tracing::error!(error = %err, "value error");
                StatusCode::INTERNAL_SERVER_ERROR.into_response()
            }
        }
    }
}