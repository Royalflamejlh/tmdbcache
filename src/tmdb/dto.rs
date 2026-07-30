//! Deserialisation targets for the TMDB v3 responses this app consumes.
//!
//! Only the fields MovieDB actually surfaces are modelled; everything else in
//! TMDB's (large) payloads is ignored.

use std::collections::HashMap;

use serde::Deserialize;

#[derive(Debug, Clone, Deserialize, Default)]
pub struct TmdbConfiguration {
    pub images: Option<crate::model::TmdbConfigurationImages>,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct TmdbGenre {
    pub id: i64,
    #[serde(default)]
    pub name: String,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct TmdbNetwork {
    pub id: i64,
    #[serde(default)]
    pub name: String,
    pub logo_path: Option<String>,
    pub origin_country: Option<String>,
    pub headquarters: Option<String>,
    pub homepage: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct TmdbPaged<T> {
    pub page: Option<i64>,
    #[serde(default)]
    pub results: Vec<T>,
    pub total_pages: Option<i64>,
    pub total_results: Option<i64>,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct TmdbExternalIds {
    pub imdb_id: Option<String>,
    /// TMDB reports this as a number for TV; accept either shape.
    pub tvdb_id: Option<serde_json::Value>,
    pub wikidata_id: Option<String>,
    pub facebook_id: Option<String>,
    pub instagram_id: Option<String>,
    pub twitter_id: Option<String>,
}

impl TmdbExternalIds {
    /// TVDB ids arrive as either `12345` or `"12345"`.
    pub fn tvdb_id_string(&self) -> Option<String> {
        match self.tvdb_id.as_ref()? {
            serde_json::Value::String(s) if !s.is_empty() => Some(s.clone()),
            serde_json::Value::Number(n) => Some(n.to_string()),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct TmdbVideo {
    pub key: Option<String>,
    pub site: Option<String>,
    #[serde(rename = "type")]
    pub video_type: Option<String>,
    #[serde(default)]
    pub official: bool,
    pub published_at: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct TmdbVideos {
    #[serde(default)]
    pub results: Vec<TmdbVideo>,
}

impl TmdbVideos {
    /// Best YouTube trailer: official trailers first, then any trailer, then any
    /// YouTube clip at all.
    pub fn best_trailer_key(&self) -> Option<String> {
        let youtube = |v: &&TmdbVideo| {
            v.site
                .as_deref()
                .is_some_and(|s| s.eq_ignore_ascii_case("YouTube"))
                && v.key.is_some()
        };
        let is_trailer = |v: &&TmdbVideo| {
            v.video_type
                .as_deref()
                .is_some_and(|t| t.eq_ignore_ascii_case("Trailer"))
        };

        self.results
            .iter()
            .filter(youtube)
            .find(|v| is_trailer(v) && v.official)
            .or_else(|| self.results.iter().filter(youtube).find(is_trailer))
            .or_else(|| self.results.iter().find(youtube))
            .and_then(|v| v.key.clone())
    }
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct TmdbCast {
    pub id: i64,
    #[serde(default)]
    pub name: String,
    pub original_name: Option<String>,
    pub adult: Option<bool>,
    pub gender: Option<i64>,
    pub known_for_department: Option<String>,
    pub popularity: Option<f64>,
    pub profile_path: Option<String>,
    pub credit_id: Option<String>,
    pub cast_id: Option<i64>,
    pub character: Option<String>,
    pub order: Option<i64>,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct TmdbCrew {
    pub id: i64,
    #[serde(default)]
    pub name: String,
    pub original_name: Option<String>,
    pub adult: Option<bool>,
    pub gender: Option<i64>,
    pub known_for_department: Option<String>,
    pub popularity: Option<f64>,
    pub profile_path: Option<String>,
    pub credit_id: Option<String>,
    pub department: Option<String>,
    pub job: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct TmdbCredits {
    #[serde(default)]
    pub cast: Vec<TmdbCast>,
    #[serde(default)]
    pub crew: Vec<TmdbCrew>,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct TmdbImage {
    pub aspect_ratio: Option<f64>,
    pub height: Option<i32>,
    pub width: Option<i32>,
    pub file_path: Option<String>,
    pub vote_average: Option<f64>,
    pub vote_count: Option<i32>,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct TmdbImages {
    pub id: Option<i64>,
    #[serde(default)]
    pub backdrops: Vec<TmdbImage>,
    #[serde(default)]
    pub posters: Vec<TmdbImage>,
    #[serde(default)]
    pub logos: Vec<TmdbImage>,
    #[serde(default)]
    pub profiles: Vec<TmdbImage>,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct TmdbProvider {
    pub logo_path: Option<String>,
    pub provider_id: i64,
    #[serde(default)]
    pub provider_name: String,
    pub display_priority: Option<i64>,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct TmdbWatchProviderRegion {
    pub link: Option<String>,
    #[serde(default)]
    pub buy: Vec<TmdbProvider>,
    #[serde(default)]
    pub rent: Vec<TmdbProvider>,
    #[serde(default)]
    pub flatrate: Vec<TmdbProvider>,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct TmdbWatchProviders {
    #[serde(default)]
    pub results: HashMap<String, TmdbWatchProviderRegion>,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct TmdbReleaseDateEntry {
    pub certification: Option<String>,
    #[serde(rename = "type")]
    pub release_type: Option<i64>,
    pub release_date: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct TmdbReleaseDateResult {
    pub iso_3166_1: Option<String>,
    #[serde(default)]
    pub release_dates: Vec<TmdbReleaseDateEntry>,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct TmdbReleaseDates {
    #[serde(default)]
    pub results: Vec<TmdbReleaseDateResult>,
}

impl TmdbReleaseDates {
    /// Certification for `region`, falling back to the first non-empty one.
    pub fn certification(&self, region: &str) -> Option<String> {
        let pick = |r: &TmdbReleaseDateResult| {
            r.release_dates
                .iter()
                .find_map(|d| d.certification.as_ref().filter(|c| !c.is_empty()).cloned())
        };
        self.results
            .iter()
            .find(|r| r.iso_3166_1.as_deref() == Some(region))
            .and_then(pick)
            .or_else(|| self.results.iter().find_map(pick))
    }
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct TmdbContentRating {
    pub iso_3166_1: Option<String>,
    pub rating: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct TmdbContentRatings {
    #[serde(default)]
    pub results: Vec<TmdbContentRating>,
}

impl TmdbContentRatings {
    pub fn rating(&self, region: &str) -> Option<String> {
        let non_empty = |r: &&TmdbContentRating| r.rating.as_deref().is_some_and(|s| !s.is_empty());
        self.results
            .iter()
            .find(|r| r.iso_3166_1.as_deref() == Some(region) && non_empty(r))
            .or_else(|| self.results.iter().find(non_empty))
            .and_then(|r| r.rating.clone())
    }
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct TmdbCollectionRef {
    pub id: i64,
    #[serde(default)]
    pub name: String,
    pub poster_path: Option<String>,
    pub backdrop_path: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct TmdbMovie {
    pub id: i64,
    pub title: Option<String>,
    pub original_title: Option<String>,
    pub original_language: Option<String>,
    pub overview: Option<String>,
    pub poster_path: Option<String>,
    pub backdrop_path: Option<String>,
    pub release_date: Option<String>,
    pub runtime: Option<i32>,
    pub tagline: Option<String>,
    pub vote_average: Option<f64>,
    pub vote_count: Option<i64>,
    pub popularity: Option<f64>,
    #[serde(default)]
    pub adult: bool,
    pub imdb_id: Option<String>,
    #[serde(default)]
    pub genres: Vec<TmdbGenre>,
    pub belongs_to_collection: Option<TmdbCollectionRef>,

    // Populated via append_to_response.
    pub credits: Option<TmdbCredits>,
    pub recommendations: Option<TmdbPaged<TmdbMovieBrief>>,
    pub videos: Option<TmdbVideos>,
    pub external_ids: Option<TmdbExternalIds>,
}

/// Compact movie shape used by search, recommendations and collection parts.
#[derive(Debug, Clone, Deserialize, Default)]
pub struct TmdbMovieBrief {
    pub id: i64,
    pub title: Option<String>,
    pub name: Option<String>,
    pub original_title: Option<String>,
    pub original_language: Option<String>,
    pub overview: Option<String>,
    pub poster_path: Option<String>,
    pub backdrop_path: Option<String>,
    pub release_date: Option<String>,
    pub first_air_date: Option<String>,
    pub vote_average: Option<f64>,
    pub vote_count: Option<i64>,
    pub popularity: Option<f64>,
    #[serde(default)]
    pub adult: bool,
    pub video: Option<bool>,
    pub media_type: Option<String>,
    #[serde(default)]
    pub genre_ids: Vec<i64>,
}

impl TmdbMovieBrief {
    /// Movies carry `title`, TV carries `name`.
    pub fn display_name(&self) -> String {
        self.title
            .clone()
            .or_else(|| self.name.clone())
            .unwrap_or_default()
    }
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct TmdbSeasonBrief {
    pub id: Option<i64>,
    pub name: Option<String>,
    pub overview: Option<String>,
    pub poster_path: Option<String>,
    pub season_number: Option<i64>,
    pub episode_count: Option<i64>,
    pub air_date: Option<String>,
    pub vote_average: Option<f64>,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct TmdbTvShow {
    pub id: i64,
    pub name: Option<String>,
    pub original_name: Option<String>,
    pub original_language: Option<String>,
    pub overview: Option<String>,
    pub poster_path: Option<String>,
    pub backdrop_path: Option<String>,
    pub first_air_date: Option<String>,
    pub tagline: Option<String>,
    pub vote_average: Option<f64>,
    pub vote_count: Option<i64>,
    pub popularity: Option<f64>,
    #[serde(default)]
    pub adult: bool,
    #[serde(default)]
    pub episode_run_time: Vec<i32>,
    #[serde(default)]
    pub genres: Vec<TmdbGenre>,
    #[serde(default)]
    pub networks: Vec<TmdbNetwork>,
    #[serde(default)]
    pub seasons: Vec<TmdbSeasonBrief>,

    pub credits: Option<TmdbCredits>,
    pub recommendations: Option<TmdbPaged<TmdbMovieBrief>>,
    pub videos: Option<TmdbVideos>,
    pub external_ids: Option<TmdbExternalIds>,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct TmdbEpisode {
    pub id: Option<i64>,
    pub name: Option<String>,
    pub overview: Option<String>,
    pub air_date: Option<String>,
    #[serde(default)]
    pub episode_number: i64,
    pub season_number: Option<i64>,
    pub still_path: Option<String>,
    pub vote_average: Option<f64>,
    pub vote_count: Option<i64>,
    pub production_code: Option<String>,
    #[serde(default)]
    pub crew: Vec<TmdbCrew>,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct TmdbSeason {
    pub id: Option<i64>,
    /// TMDB's opaque season identifier, exposed as `externalId`.
    #[serde(rename = "_id")]
    pub external_id: Option<String>,
    pub name: Option<String>,
    pub overview: Option<String>,
    pub poster_path: Option<String>,
    pub season_number: Option<i64>,
    pub air_date: Option<String>,
    pub vote_average: Option<f64>,
    #[serde(default)]
    pub episodes: Vec<TmdbEpisode>,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct TmdbPersonCredits {
    #[serde(default)]
    pub cast: Vec<TmdbCast>,
    #[serde(default)]
    pub crew: Vec<TmdbCrew>,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct TmdbPerson {
    pub id: i64,
    pub name: Option<String>,
    pub biography: Option<String>,
    pub birthday: Option<String>,
    pub deathday: Option<String>,
    pub gender: Option<i64>,
    pub imdb_id: Option<String>,
    pub known_for_department: Option<String>,
    pub place_of_birth: Option<String>,
    pub popularity: Option<f64>,
    pub profile_path: Option<String>,
    #[serde(default)]
    pub adult: bool,
    pub movie_credits: Option<TmdbPersonCredits>,
    pub tv_credits: Option<TmdbPersonCredits>,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct TmdbCollection {
    pub id: i64,
    #[serde(default)]
    pub name: String,
    pub overview: Option<String>,
    pub poster_path: Option<String>,
    pub backdrop_path: Option<String>,
    #[serde(default)]
    pub parts: Vec<TmdbMovieBrief>,
}
