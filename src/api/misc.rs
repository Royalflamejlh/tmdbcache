use axum::Json;
use axum::extract::{Path, Query, State};
use axum::http::{HeaderValue, StatusCode, header};
use axum::response::{IntoResponse, Response};

use super::{ImageQuery, RefreshQuery, SearchQuery};
use crate::error::Result;
use crate::model::{AppConfig, Collection, SearchResponse, VideoBase};
use crate::service::image::CachedImage;
use crate::service::{
    SharedState, collection, configuration, image, search as search_service, tvshow, wallpaper,
};

/// TMDB artwork is content-addressed, so a cached copy never goes stale.
const IMMUTABLE_CACHE: &str = "public, max-age=31536000, immutable";

fn image_response(image: CachedImage, cache_control: &str) -> Response {
    (
        StatusCode::OK,
        [
            (
                header::CONTENT_TYPE,
                HeaderValue::from_str(&image.content_type)
                    .unwrap_or_else(|_| HeaderValue::from_static("image/jpeg")),
            ),
            (
                header::CACHE_CONTROL,
                HeaderValue::from_str(cache_control)
                    .unwrap_or_else(|_| HeaderValue::from_static("no-cache")),
            ),
        ],
        image.bytes,
    )
        .into_response()
}

pub async fn get_image(
    State(state): State<SharedState>,
    Query(query): Query<ImageQuery>,
) -> Result<Response> {
    let image = image::get_image(&state, &query.image_path, query.backdrop_size.as_deref()).await?;
    Ok(image_response(image, IMMUTABLE_CACHE))
}

pub async fn get_wallpaper(
    State(state): State<SharedState>,
    Path(name): Path<String>,
) -> Result<Response> {
    let image = wallpaper::get_wallpaper(&state, &name).await?;
    // Wallpapers are user files that can be replaced in place, so they must be
    // revalidated rather than cached forever.
    Ok(image_response(image, "public, max-age=300"))
}

pub async fn get_configuration(State(state): State<SharedState>) -> Result<Json<AppConfig>> {
    Ok(Json(configuration::get_app_config(&state).await?))
}

pub async fn search(
    State(state): State<SharedState>,
    Query(query): Query<SearchQuery>,
) -> Result<Json<SearchResponse>> {
    Ok(Json(search_service::search(&state, &query.query).await?))
}

pub async fn get_videos(State(state): State<SharedState>) -> Result<Json<Vec<VideoBase>>> {
    Ok(Json(tvshow::list_all_videos(&state).await?))
}

pub async fn get_collection(
    State(state): State<SharedState>,
    Path(collection_id): Path<i64>,
    Query(query): Query<RefreshQuery>,
) -> Result<Json<Collection>> {
    Ok(Json(
        collection::get_collection(&state, collection_id, query.refresh()).await?,
    ))
}

pub async fn health() -> Json<serde_json::Value> {
    Json(serde_json::json!({
        "status": "UP",
        "version": env!("CARGO_PKG_VERSION"),
    }))
}
