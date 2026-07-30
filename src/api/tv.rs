use axum::Json;
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use serde::Deserialize;

use super::{DetailQuery, RefreshQuery, TagQuery};
use crate::error::Result;
use crate::model::{
    Images, TvEpisode, TvEpisodePatch, TvSeason, TvShow, TvShowsResult, VideoPatch,
};
use crate::service::{SharedState, tvshow};

pub async fn get_tv_show(
    State(state): State<SharedState>,
    Path(tv_show_id): Path<i64>,
    Query(query): Query<DetailQuery>,
) -> Result<Json<TvShow>> {
    Ok(Json(
        tvshow::get_tv_show(&state, tv_show_id, query.refresh(), query.load_details()).await?,
    ))
}

pub async fn delete_tv_show(
    State(state): State<SharedState>,
    Path(tv_show_id): Path<i64>,
) -> Result<StatusCode> {
    tvshow::delete_tv_show(&state, tv_show_id).await?;
    Ok(StatusCode::NO_CONTENT)
}

pub async fn patch_tv_show(
    State(state): State<SharedState>,
    Path(tv_show_id): Path<i64>,
    Json(patch): Json<VideoPatch>,
) -> Result<Json<TvShow>> {
    Ok(Json(
        tvshow::patch_tv_show(&state, tv_show_id, &patch).await?,
    ))
}

pub async fn get_show_backdrops(
    State(state): State<SharedState>,
    Path(tv_show_id): Path<i64>,
    Query(query): Query<RefreshQuery>,
) -> Result<Json<Images>> {
    Ok(Json(
        tvshow::get_show_backdrops(&state, tv_show_id, query.refresh()).await?,
    ))
}

pub async fn get_show_posters(
    State(state): State<SharedState>,
    Path(tv_show_id): Path<i64>,
    Query(query): Query<RefreshQuery>,
) -> Result<Json<Images>> {
    Ok(Json(
        tvshow::get_show_posters(&state, tv_show_id, query.refresh()).await?,
    ))
}

pub async fn get_tv_shows(
    State(state): State<SharedState>,
    Query(query): Query<TagQuery>,
) -> Result<Json<TvShowsResult>> {
    let tv_shows = tvshow::list_tv_shows(&state, query.tag.as_deref(), query.negate()).await?;
    Ok(Json(TvShowsResult { tv_shows }))
}

pub async fn get_tv_season(
    State(state): State<SharedState>,
    Path((tv_show_id, season_number)): Path<(i64, i64)>,
    Query(query): Query<RefreshQuery>,
) -> Result<Json<TvSeason>> {
    Ok(Json(
        tvshow::get_tv_season(&state, tv_show_id, season_number, query.refresh()).await?,
    ))
}

pub async fn patch_tv_season(
    State(state): State<SharedState>,
    Path((tv_show_id, season_number)): Path<(i64, i64)>,
    Json(patch): Json<VideoPatch>,
) -> Result<Json<TvSeason>> {
    Ok(Json(
        tvshow::patch_tv_season(&state, tv_show_id, season_number, &patch).await?,
    ))
}

pub async fn get_season_posters(
    State(state): State<SharedState>,
    Path((tv_show_id, season_number)): Path<(i64, i64)>,
    Query(query): Query<RefreshQuery>,
) -> Result<Json<Images>> {
    Ok(Json(
        tvshow::get_season_posters(&state, tv_show_id, season_number, query.refresh()).await?,
    ))
}

#[derive(Debug, Deserialize)]
pub struct WatchedBody {
    pub watched: bool,
}

pub async fn patch_season_watched(
    State(state): State<SharedState>,
    Path((tv_show_id, season_number)): Path<(i64, i64)>,
    Json(body): Json<WatchedBody>,
) -> Result<Json<TvSeason>> {
    Ok(Json(
        tvshow::set_season_watched(&state, tv_show_id, season_number, body.watched).await?,
    ))
}

pub async fn patch_tv_episode(
    State(state): State<SharedState>,
    Path((tv_show_id, season_number, episode_number)): Path<(i64, i64, i64)>,
    Json(patch): Json<TvEpisodePatch>,
) -> Result<Json<TvEpisode>> {
    Ok(Json(
        tvshow::patch_tv_episode(&state, tv_show_id, season_number, episode_number, &patch).await?,
    ))
}
