use axum::Json;
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;

use super::{DetailQuery, LimitQuery, RefreshQuery, TagQuery};
use crate::error::Result;
use crate::model::{Credits, Images, Movie, MoviesResult, Recommendations, Trailer, VideoPatch};
use crate::service::{SharedState, movie};

pub async fn get_movie(
    State(state): State<SharedState>,
    Path(movie_id): Path<i64>,
    Query(query): Query<DetailQuery>,
) -> Result<Json<Movie>> {
    let movie = movie::get_movie(&state, movie_id, query.refresh(), query.load_details()).await?;
    Ok(Json(movie))
}

pub async fn delete_movie(
    State(state): State<SharedState>,
    Path(movie_id): Path<i64>,
) -> Result<StatusCode> {
    movie::delete_movie(&state, movie_id).await?;
    Ok(StatusCode::NO_CONTENT)
}

pub async fn patch_movie(
    State(state): State<SharedState>,
    Path(movie_id): Path<i64>,
    Json(patch): Json<VideoPatch>,
) -> Result<Json<Movie>> {
    Ok(Json(movie::patch_movie(&state, movie_id, &patch).await?))
}

pub async fn get_trailer(
    State(state): State<SharedState>,
    Path(movie_id): Path<i64>,
) -> Result<Json<Trailer>> {
    Ok(Json(movie::get_trailer(&state, movie_id).await?))
}

pub async fn get_backdrops(
    State(state): State<SharedState>,
    Path(movie_id): Path<i64>,
    Query(query): Query<RefreshQuery>,
) -> Result<Json<Images>> {
    Ok(Json(
        movie::get_backdrops(&state, movie_id, query.refresh()).await?,
    ))
}

pub async fn get_posters(
    State(state): State<SharedState>,
    Path(movie_id): Path<i64>,
    Query(query): Query<RefreshQuery>,
) -> Result<Json<Images>> {
    Ok(Json(
        movie::get_posters(&state, movie_id, query.refresh()).await?,
    ))
}

pub async fn get_credits(
    State(state): State<SharedState>,
    Path(movie_id): Path<i64>,
    Query(query): Query<RefreshQuery>,
) -> Result<Json<Credits>> {
    Ok(Json(
        movie::get_credits(&state, movie_id, query.refresh()).await?,
    ))
}

pub async fn get_recommendations(
    State(state): State<SharedState>,
    Path(movie_id): Path<i64>,
    Query(query): Query<RefreshQuery>,
) -> Result<Json<Recommendations>> {
    Ok(Json(
        movie::get_recommendations(&state, movie_id, query.refresh()).await?,
    ))
}

pub async fn get_movies(
    State(state): State<SharedState>,
    Query(query): Query<TagQuery>,
) -> Result<Json<MoviesResult>> {
    let movies = movie::list_movies(&state, query.tag.as_deref(), query.negate()).await?;
    Ok(Json(MoviesResult { movies }))
}

pub async fn get_favorites(State(state): State<SharedState>) -> Result<Json<MoviesResult>> {
    Ok(Json(MoviesResult {
        movies: movie::list_favorites(&state).await?,
    }))
}

pub async fn get_top_recommendations(
    State(state): State<SharedState>,
    Query(query): Query<LimitQuery>,
) -> Result<Json<MoviesResult>> {
    Ok(Json(MoviesResult {
        movies: movie::top_recommendations(&state, query.limit).await?,
    }))
}
