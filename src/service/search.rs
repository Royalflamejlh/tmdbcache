//! TMDB search. Results are decorated with local library state so the UI can
//! show at a glance what is already held.

use std::collections::HashMap;

use super::AppState;
use crate::error::{AppError, Result};
use crate::model::{SearchResponse, SearchResult, VideoBase, VideoType, scale_vote};
use crate::store::{LocalState, Store};
use crate::tmdb::dto::TmdbMovieBrief;

fn to_result(brief: &TmdbMovieBrief, kind: VideoType, local: Option<&LocalState>) -> SearchResult {
    let local = local.cloned().unwrap_or_default();
    SearchResult {
        base: VideoBase {
            id: brief.id,
            video_type: kind,
            display_name: brief.display_name(),
            poster_path: brief.poster_path.clone(),
            backdrop_path: brief.backdrop_path.clone(),
            favorite: local.favorite,
            on_watchlist: local.on_watchlist,
            watched: local.watched,
            // TV hits report `first_air_date`; the API exposes both as
            // `release_date`.
            release_date: brief
                .release_date
                .clone()
                .or_else(|| brief.first_air_date.clone()),
            overview: brief.overview.clone(),
            vote_average: scale_vote(brief.vote_average),
            vote_count: brief.vote_count,
            popularity: brief.popularity.map(|p| p as f32),
            tags: local.tags,
            adult: brief.adult,
            ..Default::default()
        },
        genre_ids: brief.genre_ids.clone(),
    }
}

pub async fn search(state: &AppState, query: &str) -> Result<SearchResponse> {
    let query = query.trim();
    if query.is_empty() {
        return Err(AppError::BadRequest("query must not be blank".into()));
    }

    let (movies, shows) = tokio::join!(
        state.tmdb.search_movies(query, 1),
        state.tmdb.search_tv(query, 1)
    );

    // One arm failing is tolerable since the other still returns hits. Both failing
    // means TMDB itself is unreachable or the API key is wrong, which the caller
    // needs to see rather than read as "no results".
    if let (Err(movie_err), Err(tv_err)) = (&movies, &shows) {
        tracing::error!(error = %movie_err, "movie search failed");
        tracing::error!(error = %tv_err, "tv search failed");
        return Err(movies.err().unwrap());
    }

    let movies = match movies {
        Ok(paged) => paged.results,
        Err(err) => {
            tracing::warn!(error = %err, "movie search failed");
            Vec::new()
        }
    };
    let shows = match shows {
        Ok(paged) => paged.results,
        Err(err) => {
            tracing::warn!(error = %err, "tv search failed");
            Vec::new()
        }
    };

    if movies.is_empty() && shows.is_empty() {
        return Ok(SearchResponse::default());
    }

    let movie_ids: Vec<i64> = movies.iter().map(|m| m.id).collect();
    let show_ids: Vec<i64> = shows.iter().map(|s| s.id).collect();
    let (movie_locals, show_locals) = tokio::try_join!(
        state.store.local_states(VideoType::Movie, &movie_ids),
        state.store.local_states(VideoType::Tvshow, &show_ids),
    )?;

    let mut results: Vec<SearchResult> = movies
        .iter()
        .map(|m| to_result(m, VideoType::Movie, movie_locals.get(&m.id)))
        .chain(
            shows
                .iter()
                .map(|s| to_result(s, VideoType::Tvshow, show_locals.get(&s.id))),
        )
        .collect();

    // Interleaving movies and shows by popularity beats showing all movies first.
    results.sort_by(|a, b| {
        b.base
            .popularity
            .unwrap_or(0.0)
            .total_cmp(&a.base.popularity.unwrap_or(0.0))
    });

    Ok(SearchResponse { results })
}

/// Local state lookups keyed by id, exposed for tests.
pub type LocalStates = HashMap<i64, LocalState>;
