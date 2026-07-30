//! TV shows, seasons and episodes.

use super::movie::apply_video_patch;
use super::{AppState, mapper};
use crate::error::{AppError, Result};
use crate::model::{
    Flag, Images, TvEpisode, TvEpisodePatch, TvSeason, TvShow, TvShowRecommendations, VideoBase,
    VideoPatch, VideoType, reserved_flag,
};
use crate::store::{ImageOwner, Store, build_tv_recommendations};

pub async fn cache_tv_show(state: &AppState, id: i64) -> Result<()> {
    let (show, ratings, providers) = tokio::join!(
        state.tmdb.tv_show(id),
        state.tmdb.tv_content_ratings(id),
        state.tmdb.tv_watch_providers(id),
    );

    let show = show?;

    let age_rating = match ratings {
        Ok(r) => r.rating(state.tmdb.region()),
        Err(err) => {
            tracing::warn!(tv_show_id = id, error = %err, "could not load content ratings");
            None
        }
    };
    let providers = match providers {
        Ok(p) => mapper::watch_providers(&p, state.tmdb.region()),
        Err(err) => {
            tracing::warn!(tv_show_id = id, error = %err, "could not load watch providers");
            Vec::new()
        }
    };

    let upsert = mapper::tv_upsert(&show, age_rating, providers, true);
    state.store.upsert_video(&upsert).await?;

    if let Some(credits) = &show.credits {
        let cast: Vec<_> = credits.cast.iter().map(mapper::cast).collect();
        let crew: Vec<_> = credits.crew.iter().map(mapper::crew).collect();
        state
            .store
            .replace_credits(VideoType::Tvshow, id, &cast, &crew)
            .await?;
    }

    if let Some(recommendations) = &show.recommendations {
        let set = mapper::recommendation_set(recommendations, VideoType::Tvshow);
        state
            .store
            .replace_recommendations(VideoType::Tvshow, id, &set)
            .await?;
    }

    // Season rows are written shallow here; episodes arrive when a season is
    // opened.
    for season in &show.seasons {
        if let Some(upsert) = mapper::season_brief_upsert(id, season) {
            state.store.upsert_video(&upsert).await?;
        }
    }

    Ok(())
}

async fn needs_fetch(state: &AppState, id: i64, refresh: bool, load_details: bool) -> Result<bool> {
    if refresh {
        return Ok(true);
    }
    if !state.store.video_exists(VideoType::Tvshow, id).await? {
        return Ok(true);
    }
    if load_details
        && !state
            .store
            .video_details_loaded(VideoType::Tvshow, id)
            .await?
    {
        return Ok(true);
    }
    Ok(false)
}

pub async fn get_tv_show(
    state: &AppState,
    id: i64,
    refresh: bool,
    load_details: bool,
) -> Result<TvShow> {
    if needs_fetch(state, id, refresh, load_details).await? {
        cache_tv_show(state, id).await?;
    }
    assemble_show(state, id).await
}

pub async fn assemble_show(state: &AppState, id: i64) -> Result<TvShow> {
    let base = state
        .store
        .get_video(VideoType::Tvshow, id)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("tv show {id}")))?;
    let extras = state
        .store
        .video_extras(VideoType::Tvshow, id)
        .await?
        .unwrap_or_default();

    // Seasons are listed without their episodes; the season endpoint fills those.
    let mut seasons = Vec::new();
    for season_base in state.store.list_seasons(id).await? {
        let season_extras = state
            .store
            .video_extras(VideoType::Tvseason, season_base.id)
            .await?
            .unwrap_or_default();
        seasons.push(TvSeason {
            external_id: season_extras.external_id,
            air_date: season_extras.air_date,
            episode_count: season_extras.episode_count,
            episodes: Vec::new(),
            base: season_base,
        });
    }

    let credits = if state.cfg.show_tv_cast {
        state.store.get_tv_credits(id).await?
    } else {
        None
    };

    let recommendations = if state.cfg.show_recommendations {
        tv_recommendations(state, id).await?
    } else {
        None
    };

    Ok(TvShow {
        tagline: extras.tagline,
        seasons,
        credits,
        recommendations,
        base,
    })
}

pub async fn tv_recommendations(
    state: &AppState,
    id: i64,
) -> Result<Option<TvShowRecommendations>> {
    let Some(mut set) = state
        .store
        .get_recommendations(VideoType::Tvshow, id)
        .await?
    else {
        return Ok(None);
    };
    set.items
        .truncate(state.cfg.number_of_recommendations.max(0) as usize);

    let ids: Vec<i64> = set.items.iter().map(|r| r.id).collect();
    let locals = state.store.local_states(VideoType::Tvshow, &ids).await?;
    Ok(Some(build_tv_recommendations(id, set, &locals)))
}

pub async fn get_tv_season(
    state: &AppState,
    tv_show_id: i64,
    season_number: i64,
    refresh: bool,
) -> Result<TvSeason> {
    // Holding a season implies holding its show, so pull the show in first.
    if !state
        .store
        .video_exists(VideoType::Tvshow, tv_show_id)
        .await?
    {
        cache_tv_show(state, tv_show_id).await?;
    }

    let cached = state.store.get_season(tv_show_id, season_number).await?;
    let details_loaded = match &cached {
        Some(base) => state
            .store
            .video_extras(VideoType::Tvseason, base.id)
            .await?
            .map(|e| e.details_loaded)
            .unwrap_or(false),
        None => false,
    };

    if refresh || cached.is_none() || !details_loaded {
        let season = state.tmdb.tv_season(tv_show_id, season_number).await?;
        let upsert = mapper::season_upsert(tv_show_id, &season);
        state.store.upsert_video(&upsert).await?;

        let episodes: Vec<TvEpisode> = season.episodes.iter().map(mapper::episode).collect();
        state
            .store
            .replace_episodes(tv_show_id, season_number, &episodes)
            .await?;
    }

    assemble_season(state, tv_show_id, season_number).await
}

pub async fn assemble_season(
    state: &AppState,
    tv_show_id: i64,
    season_number: i64,
) -> Result<TvSeason> {
    let base = state
        .store
        .get_season(tv_show_id, season_number)
        .await?
        .ok_or_else(|| {
            AppError::NotFound(format!("season {season_number} of tv show {tv_show_id}"))
        })?;
    let extras = state
        .store
        .video_extras(VideoType::Tvseason, base.id)
        .await?
        .unwrap_or_default();

    Ok(TvSeason {
        external_id: extras.external_id,
        air_date: extras.air_date,
        episode_count: extras.episode_count,
        episodes: state.store.list_episodes(tv_show_id, season_number).await?,
        base,
    })
}

async fn ensure_show_images(state: &AppState, id: i64, refresh: bool) -> Result<Images> {
    let cached = state.store.get_images(ImageOwner::TvShow, id).await?;
    if refresh || cached.is_none() {
        let images = state.tmdb.tv_images(id).await?;
        state
            .store
            .replace_images(
                ImageOwner::TvShow,
                id,
                &mapper::images(&images.backdrops),
                &mapper::images(&images.posters),
                &mapper::images(&images.logos),
            )
            .await?;
    }
    state
        .store
        .get_images(ImageOwner::TvShow, id)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("images for tv show {id}")))
}

pub async fn get_show_backdrops(state: &AppState, id: i64, refresh: bool) -> Result<Images> {
    let images = ensure_show_images(state, id, refresh).await?;
    Ok(Images {
        id: images.id,
        backdrops: images.backdrops,
        logos: images.logos,
        ..Default::default()
    })
}

pub async fn get_show_posters(state: &AppState, id: i64, refresh: bool) -> Result<Images> {
    let images = ensure_show_images(state, id, refresh).await?;
    Ok(Images {
        id: images.id,
        posters: images.posters,
        ..Default::default()
    })
}

pub async fn get_season_posters(
    state: &AppState,
    tv_show_id: i64,
    season_number: i64,
    refresh: bool,
) -> Result<Images> {
    let owner = ImageOwner::TvSeason { season_number };
    let cached = state.store.get_images(owner, tv_show_id).await?;
    if refresh || cached.is_none() {
        let images = state
            .tmdb
            .tv_season_images(tv_show_id, season_number)
            .await?;
        state
            .store
            .replace_images(
                owner,
                tv_show_id,
                &mapper::images(&images.backdrops),
                &mapper::images(&images.posters),
                &[],
            )
            .await?;
    }

    let images = state
        .store
        .get_images(owner, tv_show_id)
        .await?
        .ok_or_else(|| {
            AppError::NotFound(format!(
                "images for season {season_number} of tv show {tv_show_id}"
            ))
        })?;
    Ok(Images {
        id: images.id,
        tv_show_id: Some(tv_show_id),
        season_number: Some(season_number),
        posters: images.posters,
        ..Default::default()
    })
}

pub async fn list_tv_shows(
    state: &AppState,
    tag: Option<&str>,
    negate: bool,
) -> Result<Vec<TvShow>> {
    let mut bases = match tag {
        Some(tag) => {
            state
                .store
                .list_videos_by_tag(VideoType::Tvshow, tag, negate)
                .await?
        }
        None => state.store.list_videos(Some(VideoType::Tvshow)).await?,
    };
    let limit = state.cfg.max_cards.max(0) as usize;
    if bases.len() > limit {
        bases.truncate(limit);
    }
    Ok(bases
        .into_iter()
        .map(|base| TvShow {
            base,
            ..Default::default()
        })
        .collect())
}

/// The `/videos` list: every held title, filtered by the media types the
/// configuration says to include.
pub async fn list_all_videos(state: &AppState) -> Result<Vec<VideoBase>> {
    let mut out = state.store.list_videos(Some(VideoType::Movie)).await?;

    if state.cfg.show_tvshows_in_videolist {
        out.extend(state.store.list_videos(Some(VideoType::Tvshow)).await?);
    }
    if state.cfg.show_tvseasons_in_videolist {
        out.extend(state.store.list_videos(Some(VideoType::Tvseason)).await?);
    }

    out.sort_by(|a, b| {
        a.display_name
            .to_lowercase()
            .cmp(&b.display_name.to_lowercase())
    });

    let limit = state.cfg.max_light_cards.max(0) as usize;
    if out.len() > limit {
        out.truncate(limit);
    }
    Ok(out)
}

pub async fn delete_tv_show(state: &AppState, id: i64) -> Result<()> {
    if state.store.delete_video(VideoType::Tvshow, id).await? {
        Ok(())
    } else {
        Err(AppError::NotFound(format!("tv show {id}")))
    }
}

pub async fn patch_tv_show(state: &AppState, id: i64, patch: &VideoPatch) -> Result<TvShow> {
    if !state.store.video_exists(VideoType::Tvshow, id).await? {
        return Err(AppError::NotFound(format!("tv show {id}")));
    }
    apply_video_patch(state, VideoType::Tvshow, id, patch).await?;
    assemble_show(state, id).await
}

pub async fn patch_tv_season(
    state: &AppState,
    tv_show_id: i64,
    season_number: i64,
    patch: &VideoPatch,
) -> Result<TvSeason> {
    let base = state
        .store
        .get_season(tv_show_id, season_number)
        .await?
        .ok_or_else(|| {
            AppError::NotFound(format!("season {season_number} of tv show {tv_show_id}"))
        })?;
    apply_video_patch(state, VideoType::Tvseason, base.id, patch).await?;
    assemble_season(state, tv_show_id, season_number).await
}

pub async fn patch_tv_episode(
    state: &AppState,
    tv_show_id: i64,
    season_number: i64,
    episode_number: i64,
    patch: &TvEpisodePatch,
) -> Result<TvEpisode> {
    if state
        .store
        .get_episode(tv_show_id, season_number, episode_number)
        .await?
        .is_none()
    {
        return Err(AppError::NotFound(format!(
            "episode {episode_number} of season {season_number}, tv show {tv_show_id}"
        )));
    }

    if let Some(tag) = patch.tag.as_deref() {
        let tag = tag.trim();
        if tag.is_empty() {
            return Err(AppError::BadRequest("tag must not be blank".into()));
        }
        let checked = patch.checked.unwrap_or(true);
        match reserved_flag(tag) {
            Some(flag) => {
                state
                    .store
                    .set_episode_flag(tv_show_id, season_number, episode_number, flag, checked)
                    .await?
            }
            None => {
                state
                    .store
                    .set_episode_tag(tv_show_id, season_number, episode_number, tag, checked)
                    .await?
            }
        }
    }

    state
        .store
        .get_episode(tv_show_id, season_number, episode_number)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("episode {episode_number}")))
}

/// Marks every episode of a season watched or unwatched in one call.
///
/// An addition beyond the recovered spec; the bundled UI uses it.
pub async fn set_season_watched(
    state: &AppState,
    tv_show_id: i64,
    season_number: i64,
    watched: bool,
) -> Result<TvSeason> {
    let episodes = state.store.list_episodes(tv_show_id, season_number).await?;
    for episode in &episodes {
        state
            .store
            .set_episode_flag(
                tv_show_id,
                season_number,
                episode.episode_number,
                Flag::Watched,
                watched,
            )
            .await?;
    }
    if let Some(base) = state.store.get_season(tv_show_id, season_number).await? {
        state
            .store
            .set_flag(VideoType::Tvseason, base.id, Flag::Watched, watched)
            .await?;
    }
    assemble_season(state, tv_show_id, season_number).await
}
