//! Movie read/write paths. Everything here is get-or-fetch: the store is
//! consulted first, TMDB only when the data is missing or a refresh is asked for.

use super::{AppState, mapper};
use crate::error::{AppError, Result};
use crate::model::{
    Flag, Images, Movie, MovieCollection, Recommendations, Trailer, VideoBase, VideoPatch,
    VideoType, reserved_flag,
};
use crate::store::{ImageOwner, Store, VideoOverrides, build_movie_recommendations};

/// Fetches a movie from TMDB and writes it, its credits, recommendations and
/// collection stub to the store.
pub async fn cache_movie(state: &AppState, id: i64) -> Result<()> {
    // The auxiliary calls are advisory: a failure there should not lose the movie.
    let (movie, release_dates, providers) = tokio::join!(
        state.tmdb.movie(id),
        state.tmdb.movie_release_dates(id),
        state.tmdb.movie_watch_providers(id),
    );

    let movie = movie?;

    let age_rating = match release_dates {
        Ok(dates) => dates.certification(state.tmdb.region()),
        Err(err) => {
            tracing::warn!(movie_id = id, error = %err, "could not load release dates");
            None
        }
    };
    let providers = match providers {
        Ok(p) => mapper::watch_providers(&p, state.tmdb.region()),
        Err(err) => {
            tracing::warn!(movie_id = id, error = %err, "could not load watch providers");
            Vec::new()
        }
    };

    let upsert = mapper::movie_upsert(&movie, age_rating, providers, true);
    state.store.upsert_video(&upsert).await?;

    if let Some(credits) = &movie.credits {
        let cast: Vec<_> = credits.cast.iter().map(mapper::cast).collect();
        let crew: Vec<_> = credits.crew.iter().map(mapper::crew).collect();
        state
            .store
            .replace_credits(VideoType::Movie, id, &cast, &crew)
            .await?;
    }

    if let Some(recommendations) = &movie.recommendations {
        let set = mapper::recommendation_set(recommendations, VideoType::Movie);
        state
            .store
            .replace_recommendations(VideoType::Movie, id, &set)
            .await?;
    }

    // Store just enough of the collection to render the "part of…" link; the
    // full part list arrives when the collection endpoint is hit.
    if let Some(reference) = &movie.belongs_to_collection {
        state
            .store
            .upsert_collection(&crate::model::Collection {
                id: reference.id,
                name: reference.name.clone(),
                poster_path: reference.poster_path.clone(),
                backdrop_path: reference.backdrop_path.clone(),
                ..Default::default()
            })
            .await?;
    }

    Ok(())
}

/// True when the store cannot satisfy this request on its own.
async fn needs_fetch(state: &AppState, id: i64, refresh: bool, load_details: bool) -> Result<bool> {
    if refresh {
        return Ok(true);
    }
    if !state.store.video_exists(VideoType::Movie, id).await? {
        return Ok(true);
    }
    if load_details
        && !state
            .store
            .video_details_loaded(VideoType::Movie, id)
            .await?
    {
        return Ok(true);
    }
    Ok(false)
}

pub async fn get_movie(
    state: &AppState,
    id: i64,
    refresh: bool,
    load_details: bool,
) -> Result<Movie> {
    if needs_fetch(state, id, refresh, load_details).await? {
        cache_movie(state, id).await?;
    }
    assemble(state, id).await
}

/// Builds the full `Movie` response from cached data alone.
pub async fn assemble(state: &AppState, id: i64) -> Result<Movie> {
    let base = state
        .store
        .get_video(VideoType::Movie, id)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("movie {id}")))?;
    let extras = state
        .store
        .video_extras(VideoType::Movie, id)
        .await?
        .unwrap_or_default();

    let credits = if state.cfg.show_movie_cast {
        state.store.get_credits(VideoType::Movie, id).await?
    } else {
        None
    };

    let recommendations = if state.cfg.show_recommendations {
        movie_recommendations(state, id).await?
    } else {
        None
    };

    let belongs_to_collection = match extras.collection_id {
        Some(collection_id) => {
            state
                .store
                .get_collection(collection_id)
                .await?
                .map(|c| MovieCollection {
                    id: c.id,
                    poster_path: c.poster_path,
                    backdrop_path: c.backdrop_path,
                    favorite: false,
                    on_watchlist: false,
                    watched: false,
                    name: c.name,
                })
        }
        None => None,
    };

    Ok(Movie {
        directors: state.store.get_directors(id).await?,
        credits,
        recommendations,
        original_title: extras.original_title,
        tagline: extras.tagline,
        belongs_to_collection,
        trailer_key: extras.trailer_key,
        base,
    })
}

/// Cached recommendations, trimmed to the configured count and decorated with
/// local library state.
pub async fn movie_recommendations(state: &AppState, id: i64) -> Result<Option<Recommendations>> {
    let Some(mut set) = state
        .store
        .get_recommendations(VideoType::Movie, id)
        .await?
    else {
        return Ok(None);
    };
    set.items
        .truncate(state.cfg.number_of_recommendations.max(0) as usize);

    let ids: Vec<i64> = set.items.iter().map(|r| r.id).collect();
    let locals = state.store.local_states(VideoType::Movie, &ids).await?;
    Ok(Some(build_movie_recommendations(id, set, &locals)))
}

/// Recommendations endpoint: fetches on demand when nothing is cached.
pub async fn get_recommendations(
    state: &AppState,
    id: i64,
    refresh: bool,
) -> Result<Recommendations> {
    let cached = state
        .store
        .get_recommendations(VideoType::Movie, id)
        .await?;
    if refresh || cached.is_none() {
        match state.tmdb.movie_recommendations(id, 1).await {
            Ok(paged) => {
                let set = mapper::recommendation_set(&paged, VideoType::Movie);
                state
                    .store
                    .replace_recommendations(VideoType::Movie, id, &set)
                    .await?;
            }
            // A cached copy is better than an error if TMDB is unreachable.
            Err(err) if cached.is_some() => {
                tracing::warn!(movie_id = id, error = %err, "recommendation refresh failed");
            }
            Err(err) => return Err(err),
        }
    }

    movie_recommendations(state, id)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("recommendations for movie {id}")))
}

pub async fn get_credits(
    state: &AppState,
    id: i64,
    refresh: bool,
) -> Result<crate::model::Credits> {
    if refresh || !state.store.has_credits(VideoType::Movie, id).await? {
        let credits = state.tmdb.movie_credits(id).await?;
        let cast: Vec<_> = credits.cast.iter().map(mapper::cast).collect();
        let crew: Vec<_> = credits.crew.iter().map(mapper::crew).collect();
        state
            .store
            .replace_credits(VideoType::Movie, id, &cast, &crew)
            .await?;
    }
    state
        .store
        .get_credits(VideoType::Movie, id)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("credits for movie {id}")))
}

/// Shared by the backdrops and posters endpoints.
async fn ensure_images(state: &AppState, id: i64, refresh: bool) -> Result<Images> {
    let cached = state.store.get_images(ImageOwner::Movie, id).await?;
    if refresh || cached.is_none() {
        let images = state.tmdb.movie_images(id).await?;
        state
            .store
            .replace_images(
                ImageOwner::Movie,
                id,
                &mapper::images(&images.backdrops),
                &mapper::images(&images.posters),
                &mapper::images(&images.logos),
            )
            .await?;
    }
    state
        .store
        .get_images(ImageOwner::Movie, id)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("images for movie {id}")))
}

pub async fn get_backdrops(state: &AppState, id: i64, refresh: bool) -> Result<Images> {
    let images = ensure_images(state, id, refresh).await?;
    Ok(Images {
        id: images.id,
        backdrops: images.backdrops,
        logos: images.logos,
        ..Default::default()
    })
}

pub async fn get_posters(state: &AppState, id: i64, refresh: bool) -> Result<Images> {
    let images = ensure_images(state, id, refresh).await?;
    Ok(Images {
        id: images.id,
        posters: images.posters,
        ..Default::default()
    })
}

pub async fn get_trailer(state: &AppState, id: i64) -> Result<Trailer> {
    // Caching the movie also persists its trailer key, so a cold read costs one
    // movie fetch rather than a videos call on every request.
    if !state.store.video_exists(VideoType::Movie, id).await? {
        cache_movie(state, id).await?;
    }
    let extras = state
        .store
        .video_extras(VideoType::Movie, id)
        .await?
        .unwrap_or_default();

    match extras.trailer_key {
        Some(trailer_key) => Ok(Trailer { trailer_key }),
        None => Err(AppError::NotFound(format!("trailer for movie {id}"))),
    }
}

/// Library listing. Cards need only the base fields, so credits and
/// recommendations are deliberately left off.
pub async fn list_movies(state: &AppState, tag: Option<&str>, negate: bool) -> Result<Vec<Movie>> {
    let bases = match tag {
        Some(tag) => {
            state
                .store
                .list_videos_by_tag(VideoType::Movie, tag, negate)
                .await?
        }
        None => state.store.list_videos(Some(VideoType::Movie)).await?,
    };
    Ok(truncate_to_cards(state, bases))
}

pub async fn list_favorites(state: &AppState) -> Result<Vec<Movie>> {
    let bases = state
        .store
        .list_videos_by_flag(VideoType::Movie, Flag::Favorite)
        .await?;
    Ok(truncate_to_cards(state, bases))
}

fn truncate_to_cards(state: &AppState, mut bases: Vec<VideoBase>) -> Vec<Movie> {
    let limit = state.cfg.max_cards.max(0) as usize;
    if bases.len() > limit {
        bases.truncate(limit);
    }
    bases
        .into_iter()
        .map(|base| Movie {
            base,
            ..Default::default()
        })
        .collect()
}

/// Titles most often recommended across the library and not already held.
pub async fn top_recommendations(state: &AppState, limit: Option<i64>) -> Result<Vec<Movie>> {
    let limit = limit
        .unwrap_or(state.cfg.number_of_top_recommendations)
        .max(0);
    let items = state.store.top_recommendations(limit).await?;
    Ok(items
        .into_iter()
        .map(|r| Movie {
            base: VideoBase {
                id: r.id,
                video_type: VideoType::Movie,
                display_name: r.display_name,
                poster_path: r.poster_path,
                backdrop_path: r.backdrop_path,
                vote_average: r.vote_average,
                release_date: r.release_date,
                age_rating: r.age_rating,
                adult: r.adult.unwrap_or(false),
                ..Default::default()
            },
            ..Default::default()
        })
        .collect())
}

pub async fn delete_movie(state: &AppState, id: i64) -> Result<()> {
    if state.store.delete_video(VideoType::Movie, id).await? {
        Ok(())
    } else {
        Err(AppError::NotFound(format!("movie {id}")))
    }
}

/// Applies a patch, then returns the updated movie.
pub async fn patch_movie(state: &AppState, id: i64, patch: &VideoPatch) -> Result<Movie> {
    if !state.store.video_exists(VideoType::Movie, id).await? {
        return Err(AppError::NotFound(format!("movie {id}")));
    }
    apply_video_patch(state, VideoType::Movie, id, patch).await?;
    assemble(state, id).await
}

/// Shared patch handling for movies, TV shows and seasons.
///
/// `tag` + `checked` toggle either a reserved flag column
/// (`favorite`/`watched`/`onWatchlist`) or a freeform tag; the remaining fields
/// are stored as user overrides.
pub async fn apply_video_patch(
    state: &AppState,
    video_type: VideoType,
    id: i64,
    patch: &VideoPatch,
) -> Result<()> {
    if let Some(tag) = patch.tag.as_deref() {
        let tag = tag.trim();
        if tag.is_empty() {
            return Err(AppError::BadRequest("tag must not be blank".into()));
        }
        // Absent `checked` is treated as "add", matching the UI's behaviour when
        // it only sends a tag name.
        let checked = patch.checked.unwrap_or(true);
        match reserved_flag(tag) {
            Some(flag) => state.store.set_flag(video_type, id, flag, checked).await?,
            None => state.store.set_tag(video_type, id, tag, checked).await?,
        }
    }

    let overrides = VideoOverrides {
        poster_path: patch.poster_path.clone(),
        backdrop_path: patch.backdrop_path.clone(),
        overview: patch.overview.clone(),
        imdb_id: patch.imdb_id.clone(),
        wer_streamt_es_id: patch.wer_streamt_es_id,
    };
    state
        .store
        .apply_overrides(video_type, id, &overrides)
        .await?;

    Ok(())
}
