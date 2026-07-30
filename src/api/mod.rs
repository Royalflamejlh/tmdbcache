//! HTTP surface. Paths mirror `docs/openapi-original.yaml` exactly.

pub mod misc;
pub mod movie;
pub mod person;
pub mod tv;

use axum::Router;
use axum::routing::{get, patch};
use serde::Deserialize;

use crate::service::SharedState;

/// `?refresh=true` forces a re-fetch from TMDB.
#[derive(Debug, Deserialize, Default)]
pub struct RefreshQuery {
    pub refresh: Option<bool>,
}

impl RefreshQuery {
    pub fn refresh(&self) -> bool {
        self.refresh.unwrap_or(false)
    }
}

/// `?refresh` plus `?loadDetails`, used by the movie and TV show endpoints.
#[derive(Debug, Deserialize, Default)]
pub struct DetailQuery {
    pub refresh: Option<bool>,
    #[serde(rename = "loadDetails")]
    pub load_details: Option<bool>,
}

impl DetailQuery {
    pub fn refresh(&self) -> bool {
        self.refresh.unwrap_or(false)
    }

    pub fn load_details(&self) -> bool {
        self.load_details.unwrap_or(false)
    }
}

/// `?tag=` with `?not=true` to invert the match.
#[derive(Debug, Deserialize, Default)]
pub struct TagQuery {
    pub tag: Option<String>,
    pub not: Option<bool>,
}

impl TagQuery {
    pub fn negate(&self) -> bool {
        self.not.unwrap_or(false)
    }
}

#[derive(Debug, Deserialize, Default)]
pub struct LimitQuery {
    pub limit: Option<i64>,
}

#[derive(Debug, Deserialize)]
pub struct SearchQuery {
    pub query: String,
}

#[derive(Debug, Deserialize)]
pub struct ImageQuery {
    #[serde(rename = "imagePath")]
    pub image_path: String,
    #[serde(rename = "backdropSize")]
    pub backdrop_size: Option<String>,
}

/// Everything under `/api/v1`, plus health probes.
pub fn router() -> Router<SharedState> {
    Router::new()
        // images
        .route("/api/v1/images", get(misc::get_image))
        .route(
            "/api/v1/images/wallpaper/{wallpaper}",
            get(misc::get_wallpaper),
        )
        // configuration
        .route("/api/v1/tmdb/configuration", get(misc::get_configuration))
        // movie
        .route(
            "/api/v1/movie/{movieId}",
            get(movie::get_movie)
                .delete(movie::delete_movie)
                .patch(movie::patch_movie),
        )
        .route("/api/v1/movie/{movieId}/trailer", get(movie::get_trailer))
        .route(
            "/api/v1/movie/{movieId}/backdrops",
            get(movie::get_backdrops),
        )
        .route("/api/v1/movie/{movieId}/posters", get(movie::get_posters))
        .route("/api/v1/movie/credits/{movieId}", get(movie::get_credits))
        .route(
            "/api/v1/movie/recommendations/{movieId}",
            get(movie::get_recommendations),
        )
        // movies
        .route("/api/v1/movies", get(movie::get_movies))
        .route("/api/v1/movies/favorites", get(movie::get_favorites))
        .route(
            "/api/v1/movies/topRecommendations",
            get(movie::get_top_recommendations),
        )
        // person
        .route(
            "/api/v1/person/{personId}",
            get(person::get_person).patch(person::patch_person),
        )
        .route(
            "/api/v1/person/{personId}/profiles",
            get(person::get_profiles),
        )
        // collection
        .route(
            "/api/v1/collection/{collectionId}",
            get(misc::get_collection),
        )
        // search
        .route("/api/v1/search/tmdb", get(misc::search))
        // tv show
        .route(
            "/api/v1/tvshow/{tvShowId}",
            get(tv::get_tv_show)
                .delete(tv::delete_tv_show)
                .patch(tv::patch_tv_show),
        )
        .route(
            "/api/v1/tvshow/{tvShowId}/backdrops",
            get(tv::get_show_backdrops),
        )
        .route(
            "/api/v1/tvshow/{tvShowId}/posters",
            get(tv::get_show_posters),
        )
        .route("/api/v1/tvshows", get(tv::get_tv_shows))
        // tv season
        .route(
            "/api/v1/tvseason/{tvShowId}/{seasonId}",
            get(tv::get_tv_season).patch(tv::patch_tv_season),
        )
        .route(
            "/api/v1/tvseason/{tvShowId}/{seasonId}/posters",
            get(tv::get_season_posters),
        )
        // Beyond the recovered spec: bulk-marks a season watched, which the
        // bundled UI needs and the original could only do episode by episode.
        .route(
            "/api/v1/tvseason/{tvShowId}/{seasonId}/watched",
            patch(tv::patch_season_watched),
        )
        // tv episode
        .route(
            "/api/v1/tvepisode/{tvShowId}/{tvSeasonId}/{tvEpisodeId}",
            patch(tv::patch_tv_episode),
        )
        // videos
        .route("/api/v1/videos", get(misc::get_videos))
        // health, for container probes
        .route("/actuator/health", get(misc::health))
        .route("/health", get(misc::health))
}
