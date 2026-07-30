//! Application configuration, read from `MOVIEDB_*` environment variables.
//!
//! Variable names and defaults mirror the original Java MovieDB so existing
//! deployments can be pointed at this binary unchanged. See
//! `docs/original-dockerhub-readme.md` for the upstream documentation.

use std::collections::BTreeSet;
use std::path::PathBuf;

use crate::error::{AppError, Result};

/// Unsupported-but-recognised integrations. The original app could push metrics
/// to InfluxDB, sync a library from Emby and gate access behind Keycloak. Those
/// are not implemented here; we still parse the variables so we can warn loudly
/// instead of silently ignoring a configured deployment.
#[derive(Debug, Clone, Default)]
pub struct UnsupportedIntegrations {
    pub emby_base_url: Option<String>,
    pub influxdb_server_url: Option<String>,
    pub oauth2_enabled: bool,
}

impl UnsupportedIntegrations {
    /// Names of integrations that were configured but will not be honoured.
    pub fn configured(&self) -> Vec<&'static str> {
        let mut out = Vec::new();
        if self.emby_base_url.is_some() {
            out.push("Emby");
        }
        if self.influxdb_server_url.is_some() {
            out.push("InfluxDB");
        }
        if self.oauth2_enabled {
            out.push("OAuth2/Keycloak");
        }
        out
    }
}

#[derive(Debug, Clone)]
pub struct Config {
    pub port: u16,
    pub tmdb_api_key: String,
    pub tmdb_language: String,
    pub tmdb_region: String,
    pub database_path: PathBuf,
    pub image_cache_path: PathBuf,

    pub subscribed_watch_providers: BTreeSet<String>,
    pub show_movie_cast: bool,
    pub show_tv_cast: bool,
    pub show_recommendations: bool,
    pub use_movie_backgrounds: bool,
    pub add_media_type_header: bool,
    pub support_detail_cards: bool,
    pub show_tvshows_in_videolist: bool,
    pub show_tvseasons_in_videolist: bool,

    pub max_cards: i64,
    pub max_light_cards: i64,
    pub number_of_recommendations: i64,
    pub number_of_top_recommendations: i64,
    pub number_of_movie_cast_references: i64,
    pub number_of_tv_cast_references: i64,
    pub number_of_directed_movies: i64,
    pub default_mobile_poster_width: i64,
    pub default_desktop_poster_width: i64,
    pub low_rating_threshold: i64,
    pub high_rating_threshold: i64,

    pub unsupported: UnsupportedIntegrations,
}

impl Config {
    pub fn from_env() -> Result<Self> {
        // The original refuses to start without these two.
        let tmdb_api_key = req_str("MOVIEDB_TMDB_APIKEY")?;
        let oauth2_enabled = bool_either("MOVIEDB_OAUTH2_ENABLED", "MOVIEDB_KEYCLOAK_ENABLED")
            .ok_or_else(|| {
                AppError::Config(
                    "MOVIEDB_KEYCLOAK_ENABLED (or MOVIEDB_OAUTH2_ENABLED) must be set".into(),
                )
            })?;

        // 2.0.0 split MOVIEDB_SHOW_CAST into per-media-type variables; accept the
        // old name as the fallback for both so 1.0.x configs keep working.
        let show_cast = env_bool("MOVIEDB_SHOW_CAST", true);
        let cast_refs = env_i64("MOVIEDB_NUMBER_OF_CAST_REFERENCES", 12);

        Ok(Self {
            port: env_i64("MOVIEDB_PORT", 8081) as u16,
            tmdb_api_key,
            tmdb_language: env_str("MOVIEDB_TMDB_LANGUAGE", "en-US"),
            tmdb_region: env_str("MOVIEDB_TMDB_REGION", "US"),
            database_path: env_str("MOVIEDB_DATABASE_PATH", "./database").into(),
            image_cache_path: env_str("MOVIEDB_IMAGE_CACHE_PATH", "./imageCache").into(),

            subscribed_watch_providers: env_csv("MOVIEDB_SUBSCRIBED_WATCH_PROVIDERS"),
            show_movie_cast: env_bool("MOVIEDB_SHOW_MOVIE_CAST", show_cast),
            show_tv_cast: env_bool("MOVIEDB_SHOW_TV_CAST", show_cast),
            show_recommendations: env_bool("MOVIEDB_SHOW_RECOMMENDATIONS", true),
            use_movie_backgrounds: env_bool("MOVIEDB_USE_MOVIEBACKGROUNDS", true),
            add_media_type_header: env_bool("MOVIEDB_ADD_MEDIATYPE_HEADER_TO_VIDEOCARD", true),
            support_detail_cards: env_bool("MOVIEDB_SUPPORT_DETAIL_CARDS", false),
            show_tvshows_in_videolist: env_bool("MOVIEDB_SHOW_TVSHOWS_IN_VIDEOLIST", true),
            show_tvseasons_in_videolist: env_bool("MOVIEDB_SHOW_TVSEASONS_IN_VIDEOLIST", true),

            max_cards: env_i64("MOVIEDB_LIST_MAX_CARDS", 200),
            max_light_cards: env_i64("MOVIEDB_LIST_MAX_LIGHT_CARDS", 300),
            number_of_recommendations: env_i64("MOVIEDB_NUMBER_OF_RECOMMENDATIONS", 12),
            number_of_top_recommendations: env_i64("MOVIEDB_NUMBER_OF_TOP_RECOMMENDATIONS", 12),
            number_of_movie_cast_references: env_i64(
                "MOVIEDB_NUMBER_OF_MOVIE_CAST_REFERENCES",
                cast_refs,
            ),
            number_of_tv_cast_references: env_i64(
                "MOVIEDB_NUMBER_OF_TV_CAST_REFERENCES",
                cast_refs,
            ),
            number_of_directed_movies: env_i64("MOVIEDB_NUMBER_OF_DIRECTED_MOVIES", 12),
            default_mobile_poster_width: env_i64("MOVIEDB_DEFAULT_MOBILE_POSTERWIDTH", 133),
            default_desktop_poster_width: env_i64("MOVIEDB_DEFAULT_DESKTOP_POSTERWIDTH", 220),
            low_rating_threshold: env_i64("MOVIEDB_LOW_RATING_THRESHOLD", 40),
            high_rating_threshold: env_i64("MOVIEDB_HIGH_RATING_THRESHOLD", 70),

            unsupported: UnsupportedIntegrations {
                emby_base_url: opt_str("MOVIEDB_EMBY_BASEURL"),
                influxdb_server_url: opt_str("MOVIEDB_INFLUXDB_SERVER_URL"),
                oauth2_enabled,
            },
        })
    }

    /// Directory holding the SQLite database file.
    pub fn database_file(&self) -> PathBuf {
        self.database_path.join("moviedb.db")
    }

    /// Directory that caches TMDB images, keyed by their TMDB path.
    pub fn image_dir(&self) -> PathBuf {
        self.image_cache_path.clone()
    }

    /// User-supplied wallpapers, watched for changes at runtime.
    pub fn wallpaper_dir(&self) -> PathBuf {
        self.image_cache_path.join("wallpapers")
    }
}

fn opt_str(key: &str) -> Option<String> {
    match std::env::var(key) {
        Ok(v) if !v.trim().is_empty() => Some(v),
        _ => None,
    }
}

fn req_str(key: &str) -> Result<String> {
    opt_str(key).ok_or_else(|| AppError::Config(format!("{key} must be set")))
}

fn env_str(key: &str, default: &str) -> String {
    opt_str(key).unwrap_or_else(|| default.to_string())
}

fn env_bool(key: &str, default: bool) -> bool {
    opt_str(key)
        .and_then(|v| v.trim().parse::<bool>().ok())
        .unwrap_or(default)
}

/// Reads a boolean from `primary`, falling back to the legacy `secondary` name.
fn bool_either(primary: &str, secondary: &str) -> Option<bool> {
    opt_str(primary)
        .or_else(|| opt_str(secondary))
        .and_then(|v| v.trim().parse::<bool>().ok())
}

fn env_i64(key: &str, default: i64) -> i64 {
    opt_str(key)
        .and_then(|v| v.trim().parse::<i64>().ok())
        .unwrap_or(default)
}

fn env_csv(key: &str) -> BTreeSet<String> {
    opt_str(key)
        .map(|v| {
            v.split(',')
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_default()
}
