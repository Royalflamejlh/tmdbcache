//! Persistence, kept behind the [`Store`] trait.
//!
//! Nothing above this module touches SQL. To move onto a different engine —
//! Turso's Rust rewrite, say, once its MVCC backend supports indexes — add a
//! second `Store` implementation and repoint [`ActiveStore`]; no service or
//! handler code changes.

pub mod sqlite;

use std::collections::BTreeSet;

use crate::error::Result;
use crate::model::{
    Cast, CastReference, Collection, Credits, Crew, Flag, Genre, Image, Images, Network,
    ProviderKind, Recommendation, Recommendations, TvEpisode, TvShowCredits, TvShowRecommendation,
    TvShowRecommendations, VideoBase, VideoType, WatchProvider,
};

pub use sqlite::SqliteStore;

/// The storage backend the binary is built against.
pub type ActiveStore = SqliteStore;

/// Which flavour of images a set belongs to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImageOwner {
    Movie,
    TvShow,
    /// Season posters, addressed by show id + season number.
    TvSeason {
        season_number: i64,
    },
    Person,
}

impl ImageOwner {
    pub fn type_str(&self) -> &'static str {
        match self {
            ImageOwner::Movie => "movie",
            ImageOwner::TvShow => "tvshow",
            ImageOwner::TvSeason { .. } => "tvseason",
            ImageOwner::Person => "person",
        }
    }

    /// Season number, or `-1` for owners where it does not apply.
    pub fn season_key(&self) -> i64 {
        match self {
            ImageOwner::TvSeason { season_number } => *season_number,
            _ => -1,
        }
    }
}

/// Everything TMDB told us about a title, ready to be written.
///
/// User-owned state (flags, tags, overrides) is deliberately absent: upserting
/// fresh metadata must never disturb it.
#[derive(Debug, Clone, Default)]
pub struct VideoUpsert {
    pub video_type: VideoType,
    pub id: i64,
    pub display_name: String,
    pub original_title: Option<String>,
    pub original_language: Option<String>,
    pub overview: Option<String>,
    pub poster_path: Option<String>,
    pub backdrop_path: Option<String>,
    pub release_date: Option<String>,
    pub runtime: Option<i32>,
    pub tagline: Option<String>,
    pub vote_average: Option<i64>,
    pub vote_count: Option<i64>,
    pub popularity: Option<f32>,
    pub adult: bool,
    pub age_rating: Option<String>,
    pub imdb_id: Option<String>,
    pub tvdb_id: Option<String>,
    pub wikidata_id: Option<String>,
    pub facebook_id: Option<String>,
    pub instagram_id: Option<String>,
    pub twitter_id: Option<String>,
    pub collection_id: Option<i64>,
    pub trailer_key: Option<String>,
    pub tv_show_id: Option<i64>,
    pub season_number: Option<i64>,
    pub external_id: Option<String>,
    pub air_date: Option<String>,
    pub episode_count: Option<i64>,
    pub details_loaded: bool,

    pub genres: Vec<Genre>,
    pub networks: Vec<Network>,
    pub watch_providers: Vec<(ProviderKind, WatchProvider)>,
}

/// A person record as stored, before local library cross-references are added.
#[derive(Debug, Clone, Default)]
pub struct PersonUpsert {
    pub id: i64,
    pub name: Option<String>,
    pub original_name: Option<String>,
    pub profile_path: Option<String>,
    pub place_of_birth: Option<String>,
    pub biography: Option<String>,
    pub birthday: Option<String>,
    pub deathday: Option<String>,
    pub gender: Option<i64>,
    pub imdb_id: Option<String>,
    pub adult: Option<bool>,
    pub popularity: Option<f32>,
    pub known_for_department: Option<String>,
}

/// The stored half of a `Person` response.
#[derive(Debug, Clone, Default)]
pub struct PersonRecord {
    pub id: i64,
    pub name: Option<String>,
    pub profile_path: Option<String>,
    pub place_of_birth: Option<String>,
    pub biography: Option<String>,
    pub birthday: Option<String>,
    pub deathday: Option<String>,
    pub gender: Option<i64>,
    pub imdb_id: Option<String>,
    pub adult: Option<bool>,
    /// `None` while the row is only a stub created from a credit list.
    pub fetched_at: Option<String>,
}

/// Recommendation list plus its paging metadata.
#[derive(Debug, Clone, Default)]
pub struct RecommendationSet {
    pub page: Option<i64>,
    pub total_pages: Option<i64>,
    pub total_results: Option<i64>,
    pub items: Vec<StoredRecommendation>,
}

#[derive(Debug, Clone, Default)]
pub struct StoredRecommendation {
    pub id: i64,
    pub display_name: String,
    pub poster_path: Option<String>,
    pub backdrop_path: Option<String>,
    pub vote_average: Option<i64>,
    pub adult: Option<bool>,
    pub rec_type: Option<String>,
    pub release_date: Option<String>,
    pub first_air_date: Option<String>,
    pub age_rating: Option<String>,
}

impl StoredRecommendation {
    /// Local flags and tags are resolved against the library when converting, so
    /// a recommendation already in the library shows the right badges.
    fn base(&self, local: Option<&LocalState>) -> crate::model::RecommendationBase {
        let local = local.cloned().unwrap_or_default();
        crate::model::RecommendationBase {
            id: self.id,
            poster_path: self.poster_path.clone(),
            backdrop_path: self.backdrop_path.clone(),
            favorite: local.favorite,
            on_watchlist: local.on_watchlist,
            watched: local.watched,
            display_name: self.display_name.clone(),
            vote_average: self.vote_average,
            adult: self.adult,
            rec_type: self.rec_type.clone(),
            emby_id: None,
            emby_server_id: None,
            tags: local.tags,
        }
    }

    pub fn into_movie_recommendation(self, local: Option<&LocalState>) -> Recommendation {
        Recommendation {
            base: self.base(local),
            release_date: self.release_date.clone(),
            age_rating: self.age_rating.clone(),
        }
    }

    pub fn into_tv_recommendation(self, local: Option<&LocalState>) -> TvShowRecommendation {
        TvShowRecommendation {
            base: self.base(local),
            first_air_date: self.first_air_date.clone(),
        }
    }
}

/// User-owned state for one title.
#[derive(Debug, Clone, Default)]
pub struct LocalState {
    pub favorite: bool,
    pub on_watchlist: bool,
    pub watched: bool,
    pub tags: Vec<String>,
}

/// Fields a PATCH may override. `None` leaves the stored value alone.
#[derive(Debug, Clone, Default)]
pub struct VideoOverrides {
    pub poster_path: Option<String>,
    pub backdrop_path: Option<String>,
    pub overview: Option<String>,
    pub imdb_id: Option<String>,
    pub wer_streamt_es_id: Option<i32>,
}

impl VideoOverrides {
    pub fn is_empty(&self) -> bool {
        self.poster_path.is_none()
            && self.backdrop_path.is_none()
            && self.overview.is_none()
            && self.imdb_id.is_none()
            && self.wer_streamt_es_id.is_none()
    }
}

/// Persistence operations used by the service layer.
#[allow(async_fn_in_trait)]
pub trait Store: Send + Sync + 'static {
    // --- videos -----------------------------------------------------------
    async fn get_video(&self, video_type: VideoType, id: i64) -> Result<Option<VideoBase>>;
    async fn video_exists(&self, video_type: VideoType, id: i64) -> Result<bool>;
    async fn video_details_loaded(&self, video_type: VideoType, id: i64) -> Result<bool>;
    async fn list_videos(&self, video_type: Option<VideoType>) -> Result<Vec<VideoBase>>;
    async fn list_videos_by_tag(
        &self,
        video_type: VideoType,
        tag: &str,
        negate: bool,
    ) -> Result<Vec<VideoBase>>;
    async fn list_videos_by_flag(
        &self,
        video_type: VideoType,
        flag: Flag,
    ) -> Result<Vec<VideoBase>>;
    async fn upsert_video(&self, upsert: &VideoUpsert) -> Result<()>;
    async fn delete_video(&self, video_type: VideoType, id: i64) -> Result<bool>;
    async fn set_flag(&self, video_type: VideoType, id: i64, flag: Flag, value: bool)
    -> Result<()>;
    async fn set_tag(&self, video_type: VideoType, id: i64, tag: &str, on: bool) -> Result<()>;
    async fn apply_overrides(
        &self,
        video_type: VideoType,
        id: i64,
        overrides: &VideoOverrides,
    ) -> Result<()>;
    /// Local state for a batch of ids, so recommendation lists can be decorated
    /// without an N+1 query.
    async fn local_states(
        &self,
        video_type: VideoType,
        ids: &[i64],
    ) -> Result<std::collections::HashMap<i64, LocalState>>;

    // --- seasons and episodes --------------------------------------------
    async fn list_seasons(&self, tv_show_id: i64) -> Result<Vec<VideoBase>>;
    async fn get_season(&self, tv_show_id: i64, season_number: i64) -> Result<Option<VideoBase>>;
    async fn list_episodes(&self, tv_show_id: i64, season_number: i64) -> Result<Vec<TvEpisode>>;
    async fn replace_episodes(
        &self,
        tv_show_id: i64,
        season_number: i64,
        episodes: &[TvEpisode],
    ) -> Result<()>;
    async fn get_episode(
        &self,
        tv_show_id: i64,
        season_number: i64,
        episode_number: i64,
    ) -> Result<Option<TvEpisode>>;
    async fn set_episode_flag(
        &self,
        tv_show_id: i64,
        season_number: i64,
        episode_number: i64,
        flag: Flag,
        value: bool,
    ) -> Result<()>;
    async fn set_episode_tag(
        &self,
        tv_show_id: i64,
        season_number: i64,
        episode_number: i64,
        tag: &str,
        on: bool,
    ) -> Result<()>;

    // --- credits ----------------------------------------------------------
    async fn get_credits(&self, video_type: VideoType, id: i64) -> Result<Option<Credits>>;
    async fn get_tv_credits(&self, id: i64) -> Result<Option<TvShowCredits>>;
    async fn get_directors(&self, id: i64) -> Result<Vec<Crew>>;
    async fn replace_credits(
        &self,
        video_type: VideoType,
        id: i64,
        cast: &[Cast],
        crew: &[Crew],
    ) -> Result<()>;
    async fn has_credits(&self, video_type: VideoType, id: i64) -> Result<bool>;

    // --- people -----------------------------------------------------------
    async fn get_person(&self, id: i64) -> Result<Option<PersonRecord>>;
    async fn upsert_person(&self, person: &PersonUpsert, mark_fetched: bool) -> Result<()>;
    async fn set_person_profile_override(&self, id: i64, profile_path: Option<&str>) -> Result<()>;
    async fn person_credits(
        &self,
        person_id: i64,
        limits: PersonCreditLimits,
    ) -> Result<PersonCredits>;

    // --- images -----------------------------------------------------------
    async fn get_images(&self, owner: ImageOwner, owner_id: i64) -> Result<Option<Images>>;
    async fn replace_images(
        &self,
        owner: ImageOwner,
        owner_id: i64,
        backdrops: &[Image],
        posters: &[Image],
        logos: &[Image],
    ) -> Result<()>;

    // --- recommendations --------------------------------------------------
    async fn get_recommendations(
        &self,
        source_type: VideoType,
        id: i64,
    ) -> Result<Option<RecommendationSet>>;
    async fn replace_recommendations(
        &self,
        source_type: VideoType,
        id: i64,
        set: &RecommendationSet,
    ) -> Result<()>;
    /// Recommendations that recur most often across the library, excluding
    /// titles already held.
    async fn top_recommendations(&self, limit: i64) -> Result<Vec<StoredRecommendation>>;

    // --- collections ------------------------------------------------------
    async fn get_collection(&self, id: i64) -> Result<Option<Collection>>;
    async fn upsert_collection(&self, collection: &Collection) -> Result<()>;

    // --- misc -------------------------------------------------------------
    async fn get_tmdb_configuration(&self) -> Result<Option<String>>;
    async fn put_tmdb_configuration(&self, payload: &str) -> Result<()>;
}

#[derive(Debug, Clone, Copy)]
pub struct PersonCreditLimits {
    pub movie_cast: i64,
    pub tv_cast: i64,
    pub directed: i64,
}

#[derive(Debug, Clone, Default)]
pub struct PersonCredits {
    pub movie_cast: Vec<CastReference>,
    pub directed_movies: Vec<CastReference>,
    pub tv_cast: Vec<CastReference>,
}

/// Helper shared by the movie and TV recommendation endpoints.
pub fn build_movie_recommendations(
    movie_id: i64,
    set: RecommendationSet,
    locals: &std::collections::HashMap<i64, LocalState>,
) -> Recommendations {
    Recommendations {
        movie_id,
        page: set.page,
        total_pages: set.total_pages,
        total_results: set.total_results,
        movie_recommendations: set
            .items
            .into_iter()
            .map(|r| {
                let local = locals.get(&r.id);
                r.into_movie_recommendation(local)
            })
            .collect(),
    }
}

pub fn build_tv_recommendations(
    tv_show_id: i64,
    set: RecommendationSet,
    locals: &std::collections::HashMap<i64, LocalState>,
) -> TvShowRecommendations {
    TvShowRecommendations {
        movie_id: tv_show_id,
        page: set.page,
        total_pages: set.total_pages,
        total_results: set.total_results,
        tv_show_recommendations: set
            .items
            .into_iter()
            .map(|r| {
                let local = locals.get(&r.id);
                r.into_tv_recommendation(local)
            })
            .collect(),
    }
}

/// Wallpaper filenames present in the configured wallpaper directory.
pub fn scan_wallpapers(dir: &std::path::Path) -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    let Ok(entries) = std::fs::read_dir(dir) else {
        return out;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let ext = path
            .extension()
            .and_then(|e| e.to_str())
            .map(|e| e.to_ascii_lowercase());
        if !matches!(ext.as_deref(), Some("jpg" | "jpeg" | "png")) {
            continue;
        }
        if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
            out.insert(name.to_string());
        }
    }
    out
}
