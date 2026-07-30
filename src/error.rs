use axum::Json;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use serde::Serialize;

pub type Result<T, E = AppError> = std::result::Result<T, E>;

#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("configuration error: {0}")]
    Config(String),

    #[error("{0} not found")]
    NotFound(String),

    #[error("bad request: {0}")]
    BadRequest(String),

    /// TMDB answered, but with a non-success status.
    #[error("TMDB request failed with status {status}: {body}")]
    TmdbStatus { status: StatusCode, body: String },

    #[error("TMDB request failed: {0}")]
    Tmdb(#[from] reqwest::Error),

    #[error("database error: {0}")]
    Database(#[from] sqlx::Error),

    #[error("migration error: {0}")]
    Migrate(#[from] sqlx::migrate::MigrateError),

    #[error("io error: {0}")]
    Io(#[from] std::io::Error),

    #[error("{0}")]
    Internal(#[from] anyhow::Error),
}

#[derive(Serialize)]
struct ErrorBody {
    error: String,
    message: String,
}

impl AppError {
    fn status(&self) -> StatusCode {
        match self {
            AppError::NotFound(_) => StatusCode::NOT_FOUND,
            AppError::BadRequest(_) => StatusCode::BAD_REQUEST,
            // A 404 from TMDB means the caller asked for something that does not
            // exist upstream, which is a 404 for our caller too.
            AppError::TmdbStatus { status, .. } if *status == StatusCode::NOT_FOUND => {
                StatusCode::NOT_FOUND
            }
            AppError::TmdbStatus { .. } | AppError::Tmdb(_) => StatusCode::BAD_GATEWAY,
            AppError::Config(_)
            | AppError::Database(_)
            | AppError::Migrate(_)
            | AppError::Io(_)
            | AppError::Internal(_) => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let status = self.status();
        let message = self.to_string();

        if status.is_server_error() {
            tracing::error!(%status, error = %message, "request failed");
        } else {
            tracing::debug!(%status, error = %message, "request rejected");
        }

        let body = ErrorBody {
            error: status.canonical_reason().unwrap_or("Error").to_string(),
            message,
        };
        (status, Json(body)).into_response()
    }
}
