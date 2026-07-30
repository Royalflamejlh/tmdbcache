//! Translation from TMDB payloads into this app's models and store records.

use crate::model::{
    Cast, Crew, Genre, Image, Network, PersonBase, ProviderKind, TvEpisode, VideoType,
    WatchProvider, scale_vote,
};
use crate::store::{PersonUpsert, RecommendationSet, StoredRecommendation, VideoUpsert};
use crate::tmdb::dto::*;

pub fn genre(dto: &TmdbGenre) -> Genre {
    Genre {
        id: dto.id,
        genre_id: Some(dto.id),
        name: dto.name.clone(),
    }
}

pub fn network(dto: &TmdbNetwork) -> Network {
    Network {
        id: dto.id,
        network_id: Some(dto.id),
        name: dto.name.clone(),
        logo_path: dto.logo_path.clone(),
        origin_country: dto.origin_country.clone(),
        headquarters: dto.headquarters.clone(),
        homepage: dto.homepage.clone(),
    }
}

pub fn image(dto: &TmdbImage) -> Image {
    Image {
        aspect_ratio: dto.aspect_ratio.map(|v| v as f32),
        height: dto.height,
        file_path: dto.file_path.clone(),
        vote_average: dto.vote_average.map(|v| v as f32),
        vote_count: dto.vote_count,
        width: dto.width,
    }
}

pub fn images(list: &[TmdbImage]) -> Vec<Image> {
    list.iter().map(image).collect()
}

pub fn cast(dto: &TmdbCast) -> Cast {
    Cast {
        person: PersonBase {
            id: dto.id,
            adult: dto.adult,
            gender: dto.gender,
            known_for_department: dto.known_for_department.clone(),
            name: dto.name.clone(),
            original_name: dto.original_name.clone(),
            popularity: dto.popularity.map(|p| p as f32),
            profile_path: dto.profile_path.clone(),
            credit_id: dto.credit_id.clone(),
        },
        cast_id: dto.cast_id,
        character: dto.character.clone(),
        order: dto.order,
    }
}

pub fn crew(dto: &TmdbCrew) -> Crew {
    Crew {
        person: PersonBase {
            id: dto.id,
            adult: dto.adult,
            gender: dto.gender,
            known_for_department: dto.known_for_department.clone(),
            name: dto.name.clone(),
            original_name: dto.original_name.clone(),
            popularity: dto.popularity.map(|p| p as f32),
            profile_path: dto.profile_path.clone(),
            credit_id: dto.credit_id.clone(),
        },
        department: dto.department.clone(),
        job: dto.job.clone(),
    }
}

pub fn episode(dto: &TmdbEpisode) -> TvEpisode {
    TvEpisode {
        id: dto.id,
        air_date: dto.air_date.clone(),
        episode_number: dto.episode_number,
        crew: dto.crew.iter().map(crew).collect(),
        name: dto.name.clone(),
        overview: dto.overview.clone(),
        production_code: dto.production_code.clone(),
        season_number: dto.season_number,
        still_path: dto.still_path.clone(),
        vote_average: dto.vote_average.map(|v| v as f32),
        vote_count: dto.vote_count,
        // Local state is layered on by the store; these are the defaults for a
        // freshly fetched episode.
        on_watchlist: false,
        favorite: false,
        watched: false,
        tags: Vec::new(),
    }
}

/// Flattens TMDB's per-region provider map down to the configured region.
pub fn watch_providers(
    dto: &TmdbWatchProviders,
    region: &str,
) -> Vec<(ProviderKind, WatchProvider)> {
    let Some(entry) = dto.results.get(region) else {
        return Vec::new();
    };
    let convert = |p: &TmdbProvider| WatchProvider {
        logo_path: p.logo_path.clone(),
        provider_id: p.provider_id,
        provider_name: p.provider_name.clone(),
        display_priority: p.display_priority,
    };

    let mut out = Vec::new();
    for p in &entry.flatrate {
        out.push((ProviderKind::Flatrate, convert(p)));
    }
    for p in &entry.rent {
        out.push((ProviderKind::Rent, convert(p)));
    }
    for p in &entry.buy {
        out.push((ProviderKind::Buy, convert(p)));
    }
    out
}

pub fn movie_upsert(
    dto: &TmdbMovie,
    age_rating: Option<String>,
    providers: Vec<(ProviderKind, WatchProvider)>,
    details_loaded: bool,
) -> VideoUpsert {
    let external = dto.external_ids.clone().unwrap_or_default();
    VideoUpsert {
        video_type: VideoType::Movie,
        id: dto.id,
        display_name: dto.title.clone().unwrap_or_default(),
        original_title: dto.original_title.clone(),
        original_language: dto.original_language.clone(),
        overview: dto.overview.clone(),
        poster_path: dto.poster_path.clone(),
        backdrop_path: dto.backdrop_path.clone(),
        release_date: dto.release_date.clone(),
        runtime: dto.runtime,
        tagline: dto.tagline.clone(),
        vote_average: scale_vote(dto.vote_average),
        vote_count: dto.vote_count,
        popularity: dto.popularity.map(|p| p as f32),
        adult: dto.adult,
        age_rating,
        // TMDB puts imdb_id on the movie itself and again under external_ids.
        imdb_id: dto.imdb_id.clone().or(external.imdb_id.clone()),
        tvdb_id: external.tvdb_id_string(),
        wikidata_id: external.wikidata_id.clone(),
        facebook_id: external.facebook_id.clone(),
        instagram_id: external.instagram_id.clone(),
        twitter_id: external.twitter_id.clone(),
        collection_id: dto.belongs_to_collection.as_ref().map(|c| c.id),
        trailer_key: dto.videos.as_ref().and_then(|v| v.best_trailer_key()),
        details_loaded,
        genres: dto.genres.iter().map(genre).collect(),
        watch_providers: providers,
        ..Default::default()
    }
}

pub fn tv_upsert(
    dto: &TmdbTvShow,
    age_rating: Option<String>,
    providers: Vec<(ProviderKind, WatchProvider)>,
    details_loaded: bool,
) -> VideoUpsert {
    let external = dto.external_ids.clone().unwrap_or_default();
    VideoUpsert {
        video_type: VideoType::Tvshow,
        id: dto.id,
        display_name: dto.name.clone().unwrap_or_default(),
        original_title: dto.original_name.clone(),
        original_language: dto.original_language.clone(),
        overview: dto.overview.clone(),
        poster_path: dto.poster_path.clone(),
        backdrop_path: dto.backdrop_path.clone(),
        // The API exposes a show's first air date through `release_date`.
        release_date: dto.first_air_date.clone(),
        runtime: dto.episode_run_time.first().copied(),
        tagline: dto.tagline.clone(),
        vote_average: scale_vote(dto.vote_average),
        vote_count: dto.vote_count,
        popularity: dto.popularity.map(|p| p as f32),
        adult: dto.adult,
        age_rating,
        imdb_id: external.imdb_id.clone(),
        tvdb_id: external.tvdb_id_string(),
        wikidata_id: external.wikidata_id.clone(),
        facebook_id: external.facebook_id.clone(),
        instagram_id: external.instagram_id.clone(),
        twitter_id: external.twitter_id.clone(),
        trailer_key: dto.videos.as_ref().and_then(|v| v.best_trailer_key()),
        details_loaded,
        genres: dto.genres.iter().map(genre).collect(),
        networks: dto.networks.iter().map(network).collect(),
        watch_providers: providers,
        ..Default::default()
    }
}

/// A season row built from the shallow `seasons` array on a TV show payload.
pub fn season_brief_upsert(tv_show_id: i64, dto: &TmdbSeasonBrief) -> Option<VideoUpsert> {
    let season_number = dto.season_number?;
    Some(VideoUpsert {
        video_type: VideoType::Tvseason,
        // Specials (season 0) have ids too, but fall back to a stable synthetic
        // key so the row can still be addressed.
        id: dto.id.unwrap_or(-(tv_show_id * 1000 + season_number)),
        display_name: dto
            .name
            .clone()
            .unwrap_or_else(|| format!("Season {season_number}")),
        overview: dto.overview.clone(),
        poster_path: dto.poster_path.clone(),
        release_date: dto.air_date.clone(),
        air_date: dto.air_date.clone(),
        episode_count: dto.episode_count,
        season_number: Some(season_number),
        tv_show_id: Some(tv_show_id),
        vote_average: scale_vote(dto.vote_average),
        details_loaded: false,
        ..Default::default()
    })
}

/// A season row built from a full season fetch.
pub fn season_upsert(tv_show_id: i64, dto: &TmdbSeason) -> VideoUpsert {
    let season_number = dto.season_number.unwrap_or(0);
    VideoUpsert {
        video_type: VideoType::Tvseason,
        id: dto.id.unwrap_or(-(tv_show_id * 1000 + season_number)),
        display_name: dto
            .name
            .clone()
            .unwrap_or_else(|| format!("Season {season_number}")),
        overview: dto.overview.clone(),
        poster_path: dto.poster_path.clone(),
        release_date: dto.air_date.clone(),
        air_date: dto.air_date.clone(),
        episode_count: Some(dto.episodes.len() as i64),
        season_number: Some(season_number),
        tv_show_id: Some(tv_show_id),
        external_id: dto.external_id.clone(),
        vote_average: scale_vote(dto.vote_average),
        details_loaded: true,
        ..Default::default()
    }
}

pub fn person_upsert(dto: &TmdbPerson) -> PersonUpsert {
    PersonUpsert {
        id: dto.id,
        name: dto.name.clone(),
        original_name: None,
        profile_path: dto.profile_path.clone(),
        place_of_birth: dto.place_of_birth.clone(),
        biography: dto.biography.clone(),
        birthday: dto.birthday.clone(),
        deathday: dto.deathday.clone(),
        gender: dto.gender,
        imdb_id: dto.imdb_id.clone(),
        adult: Some(dto.adult),
        popularity: dto.popularity.map(|p| p as f32),
        known_for_department: dto.known_for_department.clone(),
    }
}

/// Converts a paged recommendation response into a storable set.
///
/// `default_kind` labels entries whose `media_type` TMDB omits, which it does for
/// the per-title recommendation endpoints.
pub fn recommendation_set(
    paged: &TmdbPaged<TmdbMovieBrief>,
    default_kind: VideoType,
) -> RecommendationSet {
    RecommendationSet {
        page: paged.page,
        total_pages: paged.total_pages,
        total_results: paged.total_results,
        items: paged
            .results
            .iter()
            .map(|r| {
                let kind = match r.media_type.as_deref() {
                    Some("tv") => VideoType::Tvshow,
                    Some("movie") => VideoType::Movie,
                    _ => default_kind,
                };
                StoredRecommendation {
                    id: r.id,
                    display_name: r.display_name(),
                    poster_path: r.poster_path.clone(),
                    backdrop_path: r.backdrop_path.clone(),
                    vote_average: scale_vote(r.vote_average),
                    adult: Some(r.adult),
                    rec_type: Some(kind.as_str().to_string()),
                    release_date: r.release_date.clone(),
                    first_air_date: r.first_air_date.clone(),
                    age_rating: None,
                }
            })
            .collect(),
    }
}
