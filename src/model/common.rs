//! Shared building blocks. `VideoBase` stands in for the original's
//! `AbstractVideo` schema and is `#[serde(flatten)]`ed into every video-ish
//! response so the wire format matches the inheritance the Java version used.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum VideoType {
    #[default]
    Movie,
    Tvshow,
    Tvseason,
    Video,
}

impl VideoType {
    pub fn as_str(&self) -> &'static str {
        match self {
            VideoType::Movie => "movie",
            VideoType::Tvshow => "tvshow",
            VideoType::Tvseason => "tvseason",
            VideoType::Video => "video",
        }
    }
}

impl std::fmt::Display for VideoType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Genre {
    pub id: i64,
    #[serde(rename = "genreId", skip_serializing_if = "Option::is_none")]
    pub genre_id: Option<i64>,
    pub name: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Network {
    pub id: i64,
    #[serde(rename = "networkId", skip_serializing_if = "Option::is_none")]
    pub network_id: Option<i64>,
    pub name: String,
    #[serde(rename = "logoPath", skip_serializing_if = "Option::is_none")]
    pub logo_path: Option<String>,
    #[serde(rename = "originCountry", skip_serializing_if = "Option::is_none")]
    pub origin_country: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub headquarters: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub homepage: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WatchProvider {
    #[serde(rename = "logoPath", skip_serializing_if = "Option::is_none")]
    pub logo_path: Option<String>,
    #[serde(rename = "providerId")]
    pub provider_id: i64,
    #[serde(rename = "providerName")]
    pub provider_name: String,
    #[serde(rename = "displayPriority", skip_serializing_if = "Option::is_none")]
    pub display_priority: Option<i64>,
}

/// How a title can be watched on a given provider.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProviderKind {
    Buy,
    Rent,
    Flatrate,
}

impl ProviderKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            ProviderKind::Buy => "buy",
            ProviderKind::Rent => "rent",
            ProviderKind::Flatrate => "flatrate",
        }
    }

    /// Parses the value stored in `video_watch_provider.kind`.
    ///
    /// Deliberately not `FromStr`: an unrecognised kind is a row this build does
    /// not understand rather than an error, so `Option` is the honest return.
    pub fn from_tag(s: &str) -> Option<Self> {
        match s {
            "buy" => Some(ProviderKind::Buy),
            "rent" => Some(ProviderKind::Rent),
            "flatrate" => Some(ProviderKind::Flatrate),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct Image {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub aspect_ratio: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub height: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub file_path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub vote_average: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub vote_count: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub width: Option<i32>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct Images {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<i64>,
    #[serde(rename = "tvShowId", skip_serializing_if = "Option::is_none")]
    pub tv_show_id: Option<i64>,
    #[serde(rename = "seasonNumber", skip_serializing_if = "Option::is_none")]
    pub season_number: Option<i64>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub backdrops: Vec<Image>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub logos: Vec<Image>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub posters: Vec<Image>,
}

/// The original's `AbstractVideo`: everything a movie, TV show, season, search
/// hit or collection part has in common.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VideoBase {
    pub id: i64,
    #[serde(rename = "type")]
    pub video_type: VideoType,
    #[serde(rename = "displayName")]
    pub display_name: String,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub poster_path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub backdrop_path: Option<String>,

    pub favorite: bool,
    #[serde(rename = "onWatchlist")]
    pub on_watchlist: bool,
    pub watched: bool,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub release_date: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub season_number: Option<i64>,
    #[serde(rename = "tvShowId", skip_serializing_if = "Option::is_none")]
    pub tv_show_id: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub overview: Option<String>,

    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub genres: Vec<Genre>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub networks: Vec<Network>,

    /// Scaled to 0..=100, matching the original's rating thresholds.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub vote_average: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub vote_count: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub popularity: Option<f32>,

    pub tags: Vec<String>,

    #[serde(rename = "ageRating", skip_serializing_if = "Option::is_none")]
    pub age_rating: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub runtime: Option<i32>,
    pub adult: bool,

    #[serde(rename = "werStreamtEsId", skip_serializing_if = "Option::is_none")]
    pub wer_streamt_es_id: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub imdb_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tvdb_id: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub emby_id: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub emby_server_id: Option<String>,
    #[serde(rename = "embyVideoCodecs", skip_serializing_if = "Vec::is_empty")]
    pub emby_video_codecs: Vec<String>,

    #[serde(rename = "buyWatchProviders", skip_serializing_if = "Vec::is_empty")]
    pub buy_watch_providers: Vec<WatchProvider>,
    #[serde(rename = "rentWatchProviders", skip_serializing_if = "Vec::is_empty")]
    pub rent_watch_providers: Vec<WatchProvider>,
    #[serde(
        rename = "flatrateWatchProviders",
        skip_serializing_if = "Vec::is_empty"
    )]
    pub flatrate_watch_providers: Vec<WatchProvider>,

    #[serde(rename = "wikidataId", skip_serializing_if = "Option::is_none")]
    pub wikidata_id: Option<String>,
    #[serde(rename = "facebookId", skip_serializing_if = "Option::is_none")]
    pub facebook_id: Option<String>,
    #[serde(rename = "instagramId", skip_serializing_if = "Option::is_none")]
    pub instagram_id: Option<String>,
    #[serde(rename = "twitterId", skip_serializing_if = "Option::is_none")]
    pub twitter_id: Option<String>,
}

impl Default for VideoBase {
    fn default() -> Self {
        Self {
            id: 0,
            video_type: VideoType::Movie,
            display_name: String::new(),
            poster_path: None,
            backdrop_path: None,
            favorite: false,
            on_watchlist: false,
            watched: false,
            release_date: None,
            season_number: None,
            tv_show_id: None,
            overview: None,
            genres: Vec::new(),
            networks: Vec::new(),
            vote_average: None,
            vote_count: None,
            popularity: None,
            tags: Vec::new(),
            age_rating: None,
            runtime: None,
            adult: false,
            wer_streamt_es_id: None,
            imdb_id: None,
            tvdb_id: None,
            emby_id: None,
            emby_server_id: None,
            emby_video_codecs: Vec::new(),
            buy_watch_providers: Vec::new(),
            rent_watch_providers: Vec::new(),
            flatrate_watch_providers: Vec::new(),
            wikidata_id: None,
            facebook_id: None,
            instagram_id: None,
            twitter_id: None,
        }
    }
}

/// TMDB reports votes on a 0..=10 scale; the UI thresholds are percentages.
pub fn scale_vote(vote: Option<f64>) -> Option<i64> {
    vote.map(|v| (v * 10.0).round() as i64)
}
