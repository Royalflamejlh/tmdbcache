use serde::{Deserialize, Serialize};

use super::common::VideoBase;
use super::credits::{Credits, Crew, TvShowCredits};
use super::recommendation::{Recommendations, TvShowRecommendations};

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct MovieCollection {
    pub id: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub poster_path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub backdrop_path: Option<String>,
    pub favorite: bool,
    #[serde(rename = "onWatchlist")]
    pub on_watchlist: bool,
    pub watched: bool,
    pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Movie {
    #[serde(flatten)]
    pub base: VideoBase,

    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub directors: Vec<Crew>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub credits: Option<Credits>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub recommendations: Option<Recommendations>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub original_title: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tagline: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub belongs_to_collection: Option<MovieCollection>,
    #[serde(rename = "trailerKey", skip_serializing_if = "Option::is_none")]
    pub trailer_key: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct TvEpisode {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub air_date: Option<String>,
    pub episode_number: i64,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub crew: Vec<Crew>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub overview: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub production_code: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub season_number: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub still_path: Option<String>,
    /// Episodes keep TMDB's 0..=10 float, unlike the 0..=100 video scale.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub vote_average: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub vote_count: Option<i64>,
    #[serde(rename = "onWatchlist")]
    pub on_watchlist: bool,
    pub favorite: bool,
    pub watched: bool,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub tags: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct TvSeason {
    #[serde(flatten)]
    pub base: VideoBase,
    #[serde(rename = "externalId", skip_serializing_if = "Option::is_none")]
    pub external_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub air_date: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub episode_count: Option<i64>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub episodes: Vec<TvEpisode>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct TvShow {
    #[serde(flatten)]
    pub base: VideoBase,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tagline: Option<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub seasons: Vec<TvSeason>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub credits: Option<TvShowCredits>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub recommendations: Option<TvShowRecommendations>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SearchResult {
    #[serde(flatten)]
    pub base: VideoBase,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub genre_ids: Vec<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SearchResponse {
    pub results: Vec<SearchResult>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct MoviesResult {
    pub movies: Vec<Movie>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct TvShowsResult {
    #[serde(rename = "tvShows")]
    pub tv_shows: Vec<TvShow>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Trailer {
    #[serde(rename = "trailerKey")]
    pub trailer_key: String,
}
