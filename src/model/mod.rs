//! Wire types for the public API.
//!
//! These mirror the schemas recovered from the original container's OpenAPI
//! document (`docs/openapi-original.yaml`). Field names — including the
//! inconsistent mix of `snake_case` and `camelCase` — are reproduced exactly so
//! existing clients keep working.

pub mod appconfig;
pub mod collection;
pub mod common;
pub mod credits;
pub mod patch;
pub mod person;
pub mod recommendation;
pub mod video;

pub use appconfig::{AppConfig, TmdbConfigurationImages};
pub use collection::{Collection, CollectionPart};
pub use common::{
    Genre, Image, Images, Network, ProviderKind, VideoBase, VideoType, WatchProvider, scale_vote,
};
pub use credits::{Cast, CastReference, Credits, Crew, PersonBase, TvShowCast, TvShowCredits};
pub use patch::{PersonPatch, TvEpisodePatch, VideoPatch};
pub use person::{Person, PersonProfiles};
pub use recommendation::{
    Recommendation, RecommendationBase, Recommendations, TvShowRecommendation,
    TvShowRecommendations,
};
pub use video::{
    Movie, MovieCollection, MoviesResult, SearchResponse, SearchResult, Trailer, TvEpisode,
    TvSeason, TvShow, TvShowsResult,
};

/// Tags that map onto dedicated columns rather than the freeform tag table.
///
/// The upstream `VideoPatch` exposes only `tag` + `checked`, yet responses carry
/// `favorite`, `watched` and `onWatchlist` booleans — so these tag names are the
/// mechanism for toggling them.
pub const TAG_FAVORITE: &str = "favorite";
pub const TAG_WATCHED: &str = "watched";
pub const TAG_WATCHLIST: &str = "onWatchlist";

/// Resolves a tag name to the flag column it controls, if any.
pub fn reserved_flag(tag: &str) -> Option<Flag> {
    match tag {
        TAG_FAVORITE => Some(Flag::Favorite),
        TAG_WATCHED => Some(Flag::Watched),
        TAG_WATCHLIST | "watchlist" => Some(Flag::Watchlist),
        _ => None,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Flag {
    Favorite,
    Watched,
    Watchlist,
}

impl Flag {
    pub fn column(&self) -> &'static str {
        match self {
            Flag::Favorite => "favorite",
            Flag::Watched => "watched",
            Flag::Watchlist => "on_watchlist",
        }
    }
}
