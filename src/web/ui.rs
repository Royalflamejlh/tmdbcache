//! The bundled single-page UI.
//!
//! It is one self-contained HTML document compiled into the binary, so there is
//! no build step, no asset pipeline and nothing to mount at runtime. It talks to
//! the same public `/api/v1` endpoints as any other client.

use axum::Router;
use axum::extract::Request;
use axum::http::{HeaderValue, StatusCode, header};
use axum::response::{IntoResponse, Response};
use axum::routing::get;

use crate::error::AppError;
use crate::service::SharedState;

const INDEX_HTML: &str = include_str!("index.html");

/// Catches everything the API and UI routes did not claim.
///
/// Unrecognised `/api/` paths must fail as JSON — answering a mistyped endpoint
/// with `200 text/html` would let a client mistake the UI document for a
/// successful response.
async fn fallback(request: Request) -> Response {
    if request.uri().path().starts_with("/api/") {
        return AppError::NotFound(format!("endpoint {}", request.uri().path())).into_response();
    }
    index().await
}

async fn index() -> Response {
    (
        StatusCode::OK,
        [
            (
                header::CONTENT_TYPE,
                HeaderValue::from_static("text/html; charset=utf-8"),
            ),
            // The document is versioned with the binary, so revalidate each load
            // rather than pinning a stale copy in the browser after an upgrade.
            (
                header::CACHE_CONTROL,
                HeaderValue::from_static("no-cache, must-revalidate"),
            ),
        ],
        INDEX_HTML,
    )
        .into_response()
}

pub fn router() -> Router<SharedState> {
    Router::new()
        .route("/", get(index))
        // Routing is hash-based, so `/` is the only entry point the server needs
        // to serve; this fallback keeps a stray deep link from 404ing.
        .fallback(fallback)
}
