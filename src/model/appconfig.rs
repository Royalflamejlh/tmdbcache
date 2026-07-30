use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};

use crate::config::Config;

/// TMDB's `/configuration` image block, cached and passed through to the client
/// so the frontend can build image URLs itself.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct TmdbConfigurationImages {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub base_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub secure_base_url: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub backdrop_sizes: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub logo_sizes: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub poster_sizes: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub profile_sizes: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub still_sizes: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub images: Option<TmdbConfigurationImages>,
    #[serde(rename = "subscribedWatchProviders")]
    pub subscribed_watch_providers: BTreeSet<String>,

    #[serde(rename = "showMovieCast")]
    pub show_movie_cast: bool,
    #[serde(rename = "showTvCast")]
    pub show_tv_cast: bool,
    #[serde(rename = "showRecommendations")]
    pub show_recommendations: bool,
    #[serde(rename = "requireLogin")]
    pub require_login: bool,
    #[serde(rename = "oauth2Enabled")]
    pub oauth2_enabled: bool,
    #[serde(rename = "useMovieBackgrounds")]
    pub use_movie_backgrounds: bool,
    #[serde(rename = "addMediaTypeHeader")]
    pub add_media_type_header: bool,
    #[serde(rename = "darkState")]
    pub dark_state: bool,
    #[serde(rename = "supportDetailCards")]
    pub support_detail_cards: bool,

    #[serde(rename = "maxCards")]
    pub max_cards: i64,
    #[serde(rename = "maxLightCards")]
    pub max_light_cards: i64,
    #[serde(rename = "numberOfRecommendations")]
    pub number_of_recommendations: i64,
    #[serde(rename = "numberOfTopRecommendations")]
    pub number_of_top_recommendations: i64,
    #[serde(rename = "defaultMobilePosterWidth")]
    pub default_mobile_poster_width: i64,
    #[serde(rename = "defaultDesktopPosterWidth")]
    pub default_desktop_poster_width: i64,
    #[serde(rename = "lowRatingThreshold")]
    pub low_rating_threshold: i64,
    #[serde(rename = "highRatingThreshold")]
    pub high_rating_threshold: i64,

    #[serde(rename = "embyBaseUrl", skip_serializing_if = "Option::is_none")]
    pub emby_base_url: Option<String>,
    #[serde(rename = "wallpapers")]
    pub wallpapers: BTreeSet<String>,

    // Extensions beyond the original spec, used by the bundled UI.
    #[serde(rename = "showTvShowsInVideoList")]
    pub show_tvshows_in_videolist: bool,
    #[serde(rename = "showTvSeasonsInVideoList")]
    pub show_tvseasons_in_videolist: bool,
}

impl AppConfig {
    pub fn build(
        cfg: &Config,
        images: Option<TmdbConfigurationImages>,
        wallpapers: BTreeSet<String>,
    ) -> Self {
        Self {
            images,
            subscribed_watch_providers: cfg.subscribed_watch_providers.clone(),
            show_movie_cast: cfg.show_movie_cast,
            show_tv_cast: cfg.show_tv_cast,
            show_recommendations: cfg.show_recommendations,
            // Auth is not implemented, so the UI must never gate on it.
            require_login: false,
            oauth2_enabled: false,
            use_movie_backgrounds: cfg.use_movie_backgrounds,
            add_media_type_header: cfg.add_media_type_header,
            dark_state: true,
            support_detail_cards: cfg.support_detail_cards,
            max_cards: cfg.max_cards,
            max_light_cards: cfg.max_light_cards,
            number_of_recommendations: cfg.number_of_recommendations,
            number_of_top_recommendations: cfg.number_of_top_recommendations,
            default_mobile_poster_width: cfg.default_mobile_poster_width,
            default_desktop_poster_width: cfg.default_desktop_poster_width,
            low_rating_threshold: cfg.low_rating_threshold,
            high_rating_threshold: cfg.high_rating_threshold,
            emby_base_url: None,
            wallpapers,
            show_tvshows_in_videolist: cfg.show_tvshows_in_videolist,
            show_tvseasons_in_videolist: cfg.show_tvseasons_in_videolist,
        }
    }
}
