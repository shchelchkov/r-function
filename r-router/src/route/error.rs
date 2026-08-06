use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use r_db::db::error::DatabaseError;
use r_value::value::error::ValueError;

pub enum ApiError {
    NotFound,
    Value(ValueError),
    Database(DatabaseError),
    Internal(String),
}

impl From<ValueError> for ApiError {
    fn from(err: ValueError) -> Self {
        Self::Value(err)
    }
}

impl From<DatabaseError> for ApiError {
    fn from(err: DatabaseError) -> Self {
        Self::Database(err)
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

            Self::Database(err) => {
                tracing::error!(error = %err, "database error");
                StatusCode::INTERNAL_SERVER_ERROR.into_response()
            }

            Self::Internal(err) => {
                tracing::error!(error = %err, "internal error");
                StatusCode::INTERNAL_SERVER_ERROR.into_response()
            }
        }
    }
}
