use super::AppState;
use crate::error::{AppError, Result};
use crate::model::{Collection, CollectionPart, VideoBase, VideoType, scale_vote};
use crate::store::Store;
use crate::tmdb::dto::TmdbCollection;

fn map_collection(dto: &TmdbCollection) -> Collection {
    Collection {
        id: dto.id,
        poster_path: dto.poster_path.clone(),
        backdrop_path: dto.backdrop_path.clone(),
        favorite: false,
        on_watchlist: false,
        watched: false,
        name: dto.name.clone(),
        overview: dto.overview.clone(),
        parts: dto
            .parts
            .iter()
            .map(|part| CollectionPart {
                base: VideoBase {
                    id: part.id,
                    video_type: VideoType::Movie,
                    display_name: part.display_name(),
                    poster_path: part.poster_path.clone(),
                    backdrop_path: part.backdrop_path.clone(),
                    release_date: part.release_date.clone(),
                    overview: part.overview.clone(),
                    vote_average: scale_vote(part.vote_average),
                    vote_count: part.vote_count,
                    popularity: part.popularity.map(|p| p as f32),
                    adult: part.adult,
                    ..Default::default()
                },
                original_language: part.original_language.clone(),
                original_title: part.original_title.clone(),
                title: part.title.clone(),
                video: part.video,
            })
            .collect(),
    }
}

/// A collection cached only as a `belongs_to_collection` stub has no parts, so an
/// empty part list means the full record has not been fetched yet.
pub async fn get_collection(state: &AppState, id: i64, refresh: bool) -> Result<Collection> {
    let cached = state.store.get_collection(id).await?;
    let needs_fetch =
        refresh || cached.is_none() || cached.as_ref().is_some_and(|c| c.parts.is_empty());

    if needs_fetch {
        let dto = state.tmdb.collection(id).await?;
        state.store.upsert_collection(&map_collection(&dto)).await?;
    }

    state
        .store
        .get_collection(id)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("collection {id}")))
}
