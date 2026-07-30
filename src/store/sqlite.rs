//! SQLite implementation of [`Store`], running in WAL mode.
//!
//! Queries are built at runtime rather than with `sqlx::query!` so the crate
//! compiles without a live database or a checked-in offline query cache.

use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::str::FromStr;
use std::time::Duration;

use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions, SqliteSynchronous};
use sqlx::{AssertSqlSafe, Row, SqlitePool};

use super::{
    ImageOwner, LocalState, PersonCreditLimits, PersonCredits, PersonRecord, PersonUpsert,
    RecommendationSet, Store, StoredRecommendation, VideoOverrides, VideoUpsert,
};
use crate::error::Result;
use crate::model::{
    Cast, CastReference, Collection, CollectionPart, Credits, Crew, Flag, Genre, Image, Images,
    Network, PersonBase, ProviderKind, TvEpisode, TvShowCast, TvShowCredits, VideoBase, VideoType,
    WatchProvider,
};

/// Columns of `video`, with user overrides collapsed over the TMDB values so
/// callers never have to remember to apply them.
const VIDEO_COLS: &str = "
    video_type, video_id, display_name, original_title, original_language,
    COALESCE(overview_override, overview)           AS overview,
    COALESCE(poster_path_override, poster_path)     AS poster_path,
    COALESCE(backdrop_path_override, backdrop_path) AS backdrop_path,
    release_date, runtime, tagline, vote_average, vote_count, popularity,
    adult, age_rating,
    COALESCE(imdb_id_override, imdb_id)             AS imdb_id,
    tvdb_id, wikidata_id, facebook_id, instagram_id, twitter_id,
    emby_id, emby_server_id, collection_id, trailer_key,
    tv_show_id, season_number, external_id, air_date, episode_count,
    favorite, on_watchlist, watched, wer_streamt_es_id, details_loaded
";

#[derive(Debug, sqlx::FromRow)]
struct VideoRow {
    video_type: String,
    video_id: i64,
    display_name: String,
    original_title: Option<String>,
    #[allow(dead_code)]
    original_language: Option<String>,
    overview: Option<String>,
    poster_path: Option<String>,
    backdrop_path: Option<String>,
    release_date: Option<String>,
    runtime: Option<i64>,
    tagline: Option<String>,
    vote_average: Option<i64>,
    vote_count: Option<i64>,
    popularity: Option<f64>,
    adult: bool,
    age_rating: Option<String>,
    imdb_id: Option<String>,
    tvdb_id: Option<String>,
    wikidata_id: Option<String>,
    facebook_id: Option<String>,
    instagram_id: Option<String>,
    twitter_id: Option<String>,
    emby_id: Option<i64>,
    emby_server_id: Option<String>,
    collection_id: Option<i64>,
    trailer_key: Option<String>,
    tv_show_id: Option<i64>,
    season_number: Option<i64>,
    external_id: Option<String>,
    air_date: Option<String>,
    episode_count: Option<i64>,
    favorite: bool,
    on_watchlist: bool,
    watched: bool,
    wer_streamt_es_id: Option<i64>,
    details_loaded: bool,
}

/// Extra season/movie-specific bits carried alongside a [`VideoBase`].
#[derive(Debug, Clone, Default)]
pub struct VideoExtras {
    pub tagline: Option<String>,
    pub original_title: Option<String>,
    pub collection_id: Option<i64>,
    pub trailer_key: Option<String>,
    pub external_id: Option<String>,
    pub air_date: Option<String>,
    pub episode_count: Option<i64>,
    pub details_loaded: bool,
}

impl VideoRow {
    fn extras(&self) -> VideoExtras {
        VideoExtras {
            tagline: self.tagline.clone(),
            original_title: self.original_title.clone(),
            collection_id: self.collection_id,
            trailer_key: self.trailer_key.clone(),
            external_id: self.external_id.clone(),
            air_date: self.air_date.clone(),
            episode_count: self.episode_count,
            details_loaded: self.details_loaded,
        }
    }

    fn into_base(self) -> VideoBase {
        VideoBase {
            id: self.video_id,
            video_type: parse_video_type(&self.video_type),
            display_name: self.display_name,
            poster_path: self.poster_path,
            backdrop_path: self.backdrop_path,
            favorite: self.favorite,
            on_watchlist: self.on_watchlist,
            watched: self.watched,
            release_date: self.release_date,
            season_number: self.season_number,
            tv_show_id: self.tv_show_id,
            overview: self.overview,
            genres: Vec::new(),
            networks: Vec::new(),
            vote_average: self.vote_average,
            vote_count: self.vote_count,
            popularity: self.popularity.map(|p| p as f32),
            tags: Vec::new(),
            age_rating: self.age_rating,
            runtime: self.runtime.map(|r| r as i32),
            adult: self.adult,
            wer_streamt_es_id: self.wer_streamt_es_id.map(|v| v as i32),
            imdb_id: self.imdb_id,
            tvdb_id: self.tvdb_id,
            emby_id: self.emby_id,
            emby_server_id: self.emby_server_id,
            emby_video_codecs: Vec::new(),
            buy_watch_providers: Vec::new(),
            rent_watch_providers: Vec::new(),
            flatrate_watch_providers: Vec::new(),
            wikidata_id: self.wikidata_id,
            facebook_id: self.facebook_id,
            instagram_id: self.instagram_id,
            twitter_id: self.twitter_id,
        }
    }
}

pub fn parse_video_type(s: &str) -> VideoType {
    match s {
        "tvshow" => VideoType::Tvshow,
        "tvseason" => VideoType::Tvseason,
        "video" => VideoType::Video,
        _ => VideoType::Movie,
    }
}

fn now() -> String {
    chrono::Utc::now().to_rfc3339()
}

/// Largest number of ids bound into a single `IN` clause.
///
/// SQLite caps bound parameters per statement (`SQLITE_MAX_VARIABLE_NUMBER`), so
/// id sets are walked in chunks rather than assuming a library fits in one query.
const BIND_CHUNK: usize = 500;

/// `?,?,?` for an `IN` clause of `n` bindings.
fn placeholders(n: usize) -> String {
    std::iter::repeat_n("?", n).collect::<Vec<_>>().join(",")
}

#[derive(Clone)]
pub struct SqliteStore {
    pool: SqlitePool,
}

impl SqliteStore {
    /// Opens (creating if needed) the database at `path` and runs migrations.
    pub async fn connect(path: &Path) -> Result<Self> {
        if let Some(parent) = path.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }

        let options = SqliteConnectOptions::from_str(&format!("sqlite://{}", path.display()))?
            .create_if_missing(true)
            // WAL lets readers run concurrently with the single writer, which is
            // the shape of this workload: bursty writes when caching a title,
            // reads everywhere else.
            .journal_mode(SqliteJournalMode::Wal)
            .synchronous(SqliteSynchronous::Normal)
            .busy_timeout(Duration::from_secs(5))
            .foreign_keys(true);

        let pool = SqlitePoolOptions::new()
            .max_connections(8)
            .acquire_timeout(Duration::from_secs(10))
            .connect_with(options)
            .await?;

        sqlx::migrate!("./migrations").run(&pool).await?;

        Ok(Self { pool })
    }

    /// In-memory store, used by the tests.
    pub async fn connect_in_memory() -> Result<Self> {
        let options = SqliteConnectOptions::from_str("sqlite::memory:")?.foreign_keys(true);
        // A single connection keeps every query on the same in-memory database.
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(options)
            .await?;
        sqlx::migrate!("./migrations").run(&pool).await?;
        Ok(Self { pool })
    }

    pub fn pool(&self) -> &SqlitePool {
        &self.pool
    }

    /// Fills in tags, genres, networks and watch providers for `rows` using one
    /// batch query per relation rather than per row.
    async fn hydrate(&self, rows: Vec<VideoRow>) -> Result<Vec<VideoBase>> {
        if rows.is_empty() {
            return Ok(Vec::new());
        }

        let keys: Vec<(String, i64)> = rows
            .iter()
            .map(|r| (r.video_type.clone(), r.video_id))
            .collect();

        let mut tags: HashMap<(String, i64), Vec<String>> = HashMap::new();
        let mut genres: HashMap<(String, i64), Vec<Genre>> = HashMap::new();
        let mut networks: HashMap<(String, i64), Vec<Network>> = HashMap::new();
        let mut providers: HashMap<(String, i64), Vec<(String, WatchProvider)>> = HashMap::new();

        // All rows in one call share a video_type in every current caller, but
        // group defensively so mixed batches stay correct.
        let types: HashSet<&str> = rows.iter().map(|r| r.video_type.as_str()).collect();
        for vtype in types {
            let all_ids: Vec<i64> = rows
                .iter()
                .filter(|r| r.video_type == vtype)
                .map(|r| r.video_id)
                .collect();

            // One placeholder per id would blow SQLITE_MAX_VARIABLE_NUMBER on a
            // large library, so the id set is walked in chunks.
            for ids in all_ids.chunks(BIND_CHUNK) {
                let ph = placeholders(ids.len());

                let sql = format!(
                    "SELECT video_id, tag FROM video_tag
                     WHERE video_type = ? AND video_id IN ({ph}) ORDER BY tag"
                );
                let mut q = sqlx::query(AssertSqlSafe(sql.as_str())).bind(vtype);
                for id in ids {
                    q = q.bind(id);
                }
                for row in q.fetch_all(&self.pool).await? {
                    let id: i64 = row.get("video_id");
                    tags.entry((vtype.to_string(), id))
                        .or_default()
                        .push(row.get("tag"));
                }

                let sql = format!(
                    "SELECT vg.video_id, g.id, g.name FROM video_genre vg
                     JOIN genre g ON g.id = vg.genre_id
                     WHERE vg.video_type = ? AND vg.video_id IN ({ph}) ORDER BY g.name"
                );
                let mut q = sqlx::query(AssertSqlSafe(sql.as_str())).bind(vtype);
                for id in ids {
                    q = q.bind(id);
                }
                for row in q.fetch_all(&self.pool).await? {
                    let vid: i64 = row.get("video_id");
                    let gid: i64 = row.get("id");
                    genres
                        .entry((vtype.to_string(), vid))
                        .or_default()
                        .push(Genre {
                            id: gid,
                            genre_id: Some(gid),
                            name: row.get("name"),
                        });
                }

                let sql = format!(
                    "SELECT vn.video_id, n.id, n.name, n.logo_path, n.origin_country,
                            n.headquarters, n.homepage
                     FROM video_network vn
                     JOIN network n ON n.id = vn.network_id
                     WHERE vn.video_type = ? AND vn.video_id IN ({ph}) ORDER BY n.name"
                );
                let mut q = sqlx::query(AssertSqlSafe(sql.as_str())).bind(vtype);
                for id in ids {
                    q = q.bind(id);
                }
                for row in q.fetch_all(&self.pool).await? {
                    let vid: i64 = row.get("video_id");
                    let nid: i64 = row.get("id");
                    networks
                        .entry((vtype.to_string(), vid))
                        .or_default()
                        .push(Network {
                            id: nid,
                            network_id: Some(nid),
                            name: row.get("name"),
                            logo_path: row.get("logo_path"),
                            origin_country: row.get("origin_country"),
                            headquarters: row.get("headquarters"),
                            homepage: row.get("homepage"),
                        });
                }

                let sql = format!(
                    "SELECT vwp.video_id, vwp.kind, wp.provider_id, wp.provider_name,
                            wp.logo_path, wp.display_priority
                     FROM video_watch_provider vwp
                     JOIN watch_provider wp ON wp.provider_id = vwp.provider_id
                     WHERE vwp.video_type = ? AND vwp.video_id IN ({ph})
                     ORDER BY wp.display_priority, wp.provider_name"
                );
                let mut q = sqlx::query(AssertSqlSafe(sql.as_str())).bind(vtype);
                for id in ids {
                    q = q.bind(id);
                }
                for row in q.fetch_all(&self.pool).await? {
                    let vid: i64 = row.get("video_id");
                    providers
                        .entry((vtype.to_string(), vid))
                        .or_default()
                        .push((
                            row.get("kind"),
                            WatchProvider {
                                logo_path: row.get("logo_path"),
                                provider_id: row.get("provider_id"),
                                provider_name: row.get("provider_name"),
                                display_priority: row.get("display_priority"),
                            },
                        ));
                }
            }
        }

        let mut out = Vec::with_capacity(rows.len());
        for (row, key) in rows.into_iter().zip(keys) {
            let mut base = row.into_base();
            base.tags = tags.remove(&key).unwrap_or_default();
            base.genres = genres.remove(&key).unwrap_or_default();
            base.networks = networks.remove(&key).unwrap_or_default();
            for (kind, provider) in providers.remove(&key).unwrap_or_default() {
                match ProviderKind::from_tag(&kind) {
                    Some(ProviderKind::Buy) => base.buy_watch_providers.push(provider),
                    Some(ProviderKind::Rent) => base.rent_watch_providers.push(provider),
                    Some(ProviderKind::Flatrate) => base.flatrate_watch_providers.push(provider),
                    None => {}
                }
            }
            out.push(base);
        }
        Ok(out)
    }

    async fn fetch_one_video(&self, video_type: VideoType, id: i64) -> Result<Option<VideoBase>> {
        let sql = format!("SELECT {VIDEO_COLS} FROM video WHERE video_type = ? AND video_id = ?");
        let row = sqlx::query_as::<_, VideoRow>(AssertSqlSafe(sql.as_str()))
            .bind(video_type.as_str())
            .bind(id)
            .fetch_optional(&self.pool)
            .await?;
        match row {
            None => Ok(None),
            Some(row) => Ok(self.hydrate(vec![row]).await?.into_iter().next()),
        }
    }

    /// Movie/season extras that do not live on [`VideoBase`].
    pub async fn video_extras(
        &self,
        video_type: VideoType,
        id: i64,
    ) -> Result<Option<VideoExtras>> {
        let sql = format!("SELECT {VIDEO_COLS} FROM video WHERE video_type = ? AND video_id = ?");
        Ok(sqlx::query_as::<_, VideoRow>(AssertSqlSafe(sql.as_str()))
            .bind(video_type.as_str())
            .bind(id)
            .fetch_optional(&self.pool)
            .await?
            .map(|r| r.extras()))
    }

    async fn upsert_person_stub(&self, person: &PersonBase) -> Result<()> {
        sqlx::query(
            "INSERT INTO person (id, name, original_name, profile_path, gender,
                                 popularity, known_for_department, adult)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?)
             ON CONFLICT(id) DO UPDATE SET
                name                 = COALESCE(excluded.name, person.name),
                original_name        = COALESCE(excluded.original_name, person.original_name),
                profile_path         = COALESCE(excluded.profile_path, person.profile_path),
                gender               = COALESCE(excluded.gender, person.gender),
                popularity           = COALESCE(excluded.popularity, person.popularity),
                known_for_department = COALESCE(excluded.known_for_department,
                                               person.known_for_department),
                adult                = COALESCE(excluded.adult, person.adult)",
        )
        .bind(person.id)
        .bind(&person.name)
        .bind(&person.original_name)
        .bind(&person.profile_path)
        .bind(person.gender)
        .bind(person.popularity.map(|p| p as f64))
        .bind(&person.known_for_department)
        .bind(person.adult)
        .execute(&self.pool)
        .await?;
        Ok(())
    }
}

fn person_base_from_row(row: &sqlx::sqlite::SqliteRow) -> PersonBase {
    PersonBase {
        id: row.get("person_id"),
        adult: row.get("adult"),
        gender: row.get("gender"),
        known_for_department: row.get("known_for_department"),
        name: row.get::<Option<String>, _>("name").unwrap_or_default(),
        original_name: row.get("original_name"),
        popularity: row.get::<Option<f64>, _>("popularity").map(|p| p as f32),
        profile_path: row.get("profile_path"),
        credit_id: row.get("credit_id"),
    }
}

/// Joined credit columns, shared by the movie and TV credit queries.
const CREDIT_COLS: &str = "
    c.person_id, c.character, c.department, c.job, c.cast_id, c.credit_id, c.ord,
    p.name, p.original_name, p.gender, p.popularity, p.known_for_department, p.adult,
    COALESCE(p.profile_path_override, p.profile_path) AS profile_path
";

impl Store for SqliteStore {
    async fn get_video(&self, video_type: VideoType, id: i64) -> Result<Option<VideoBase>> {
        self.fetch_one_video(video_type, id).await
    }

    async fn video_exists(&self, video_type: VideoType, id: i64) -> Result<bool> {
        let row = sqlx::query("SELECT 1 FROM video WHERE video_type = ? AND video_id = ?")
            .bind(video_type.as_str())
            .bind(id)
            .fetch_optional(&self.pool)
            .await?;
        Ok(row.is_some())
    }

    async fn video_details_loaded(&self, video_type: VideoType, id: i64) -> Result<bool> {
        let row =
            sqlx::query("SELECT details_loaded FROM video WHERE video_type = ? AND video_id = ?")
                .bind(video_type.as_str())
                .bind(id)
                .fetch_optional(&self.pool)
                .await?;
        Ok(row
            .map(|r| r.get::<bool, _>("details_loaded"))
            .unwrap_or(false))
    }

    async fn list_videos(&self, video_type: Option<VideoType>) -> Result<Vec<VideoBase>> {
        let rows = match video_type {
            Some(vt) => {
                let sql = format!(
                    "SELECT {VIDEO_COLS} FROM video WHERE video_type = ?
                     ORDER BY display_name COLLATE NOCASE"
                );
                sqlx::query_as::<_, VideoRow>(AssertSqlSafe(sql.as_str()))
                    .bind(vt.as_str())
                    .fetch_all(&self.pool)
                    .await?
            }
            None => {
                let sql =
                    format!("SELECT {VIDEO_COLS} FROM video ORDER BY display_name COLLATE NOCASE");
                sqlx::query_as::<_, VideoRow>(AssertSqlSafe(sql.as_str()))
                    .fetch_all(&self.pool)
                    .await?
            }
        };
        self.hydrate(rows).await
    }

    async fn list_videos_by_tag(
        &self,
        video_type: VideoType,
        tag: &str,
        negate: bool,
    ) -> Result<Vec<VideoBase>> {
        let predicate = if negate { "NOT EXISTS" } else { "EXISTS" };
        let sql = format!(
            "SELECT {VIDEO_COLS} FROM video v
             WHERE v.video_type = ?
               AND {predicate} (
                   SELECT 1 FROM video_tag t
                   WHERE t.video_type = v.video_type
                     AND t.video_id = v.video_id
                     AND t.tag = ?
               )
             ORDER BY v.display_name COLLATE NOCASE"
        );
        let rows = sqlx::query_as::<_, VideoRow>(AssertSqlSafe(sql.as_str()))
            .bind(video_type.as_str())
            .bind(tag)
            .fetch_all(&self.pool)
            .await?;
        self.hydrate(rows).await
    }

    async fn list_videos_by_flag(
        &self,
        video_type: VideoType,
        flag: Flag,
    ) -> Result<Vec<VideoBase>> {
        // `flag.column()` is a fixed identifier from a closed enum, never input.
        let column = flag.column();
        let sql = format!(
            "SELECT {VIDEO_COLS} FROM video
             WHERE video_type = ? AND {column} = 1
             ORDER BY display_name COLLATE NOCASE"
        );
        let rows = sqlx::query_as::<_, VideoRow>(AssertSqlSafe(sql.as_str()))
            .bind(video_type.as_str())
            .fetch_all(&self.pool)
            .await?;
        self.hydrate(rows).await
    }

    async fn upsert_video(&self, u: &VideoUpsert) -> Result<()> {
        let mut tx = self.pool.begin().await?;

        sqlx::query(
            "INSERT INTO video (
                video_type, video_id, display_name, original_title, original_language,
                overview, poster_path, backdrop_path, release_date, runtime, tagline,
                vote_average, vote_count, popularity, adult, age_rating,
                imdb_id, tvdb_id, wikidata_id, facebook_id, instagram_id, twitter_id,
                collection_id, trailer_key, tv_show_id, season_number, external_id,
                air_date, episode_count, details_loaded, fetched_at
             ) VALUES (
                ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?,
                ?, ?, ?, ?, ?, ?, ?, ?, ?
             )
             ON CONFLICT(video_type, video_id) DO UPDATE SET
                display_name      = excluded.display_name,
                original_title    = excluded.original_title,
                original_language = excluded.original_language,
                overview          = excluded.overview,
                poster_path       = excluded.poster_path,
                backdrop_path     = excluded.backdrop_path,
                release_date      = excluded.release_date,
                runtime           = excluded.runtime,
                tagline           = excluded.tagline,
                vote_average      = excluded.vote_average,
                vote_count        = excluded.vote_count,
                popularity        = excluded.popularity,
                adult             = excluded.adult,
                age_rating        = COALESCE(excluded.age_rating, video.age_rating),
                imdb_id           = COALESCE(excluded.imdb_id, video.imdb_id),
                tvdb_id           = COALESCE(excluded.tvdb_id, video.tvdb_id),
                wikidata_id       = COALESCE(excluded.wikidata_id, video.wikidata_id),
                facebook_id       = COALESCE(excluded.facebook_id, video.facebook_id),
                instagram_id      = COALESCE(excluded.instagram_id, video.instagram_id),
                twitter_id        = COALESCE(excluded.twitter_id, video.twitter_id),
                collection_id     = COALESCE(excluded.collection_id, video.collection_id),
                trailer_key       = COALESCE(excluded.trailer_key, video.trailer_key),
                tv_show_id        = COALESCE(excluded.tv_show_id, video.tv_show_id),
                season_number     = COALESCE(excluded.season_number, video.season_number),
                external_id       = COALESCE(excluded.external_id, video.external_id),
                air_date          = COALESCE(excluded.air_date, video.air_date),
                episode_count     = COALESCE(excluded.episode_count, video.episode_count),
                -- Never downgrade a fully-loaded row to a stub.
                details_loaded    = video.details_loaded OR excluded.details_loaded,
                fetched_at        = excluded.fetched_at",
        )
        .bind(u.video_type.as_str())
        .bind(u.id)
        .bind(&u.display_name)
        .bind(&u.original_title)
        .bind(&u.original_language)
        .bind(&u.overview)
        .bind(&u.poster_path)
        .bind(&u.backdrop_path)
        .bind(&u.release_date)
        .bind(u.runtime.map(|r| r as i64))
        .bind(&u.tagline)
        .bind(u.vote_average)
        .bind(u.vote_count)
        .bind(u.popularity.map(|p| p as f64))
        .bind(u.adult)
        .bind(&u.age_rating)
        .bind(&u.imdb_id)
        .bind(&u.tvdb_id)
        .bind(&u.wikidata_id)
        .bind(&u.facebook_id)
        .bind(&u.instagram_id)
        .bind(&u.twitter_id)
        .bind(u.collection_id)
        .bind(&u.trailer_key)
        .bind(u.tv_show_id)
        .bind(u.season_number)
        .bind(&u.external_id)
        .bind(&u.air_date)
        .bind(u.episode_count)
        .bind(u.details_loaded)
        .bind(now())
        .execute(&mut *tx)
        .await?;

        // Genres, networks and providers are replaced wholesale: TMDB is the
        // authority and stale links would otherwise linger.
        if !u.genres.is_empty() {
            for genre in &u.genres {
                sqlx::query(
                    "INSERT INTO genre (id, name) VALUES (?, ?)
                     ON CONFLICT(id) DO UPDATE SET name = excluded.name",
                )
                .bind(genre.id)
                .bind(&genre.name)
                .execute(&mut *tx)
                .await?;
            }
        }
        sqlx::query("DELETE FROM video_genre WHERE video_type = ? AND video_id = ?")
            .bind(u.video_type.as_str())
            .bind(u.id)
            .execute(&mut *tx)
            .await?;
        for genre in &u.genres {
            sqlx::query(
                "INSERT OR IGNORE INTO video_genre (video_type, video_id, genre_id)
                 VALUES (?, ?, ?)",
            )
            .bind(u.video_type.as_str())
            .bind(u.id)
            .bind(genre.id)
            .execute(&mut *tx)
            .await?;
        }

        for network in &u.networks {
            sqlx::query(
                "INSERT INTO network (id, name, logo_path, origin_country, headquarters, homepage)
                 VALUES (?, ?, ?, ?, ?, ?)
                 ON CONFLICT(id) DO UPDATE SET
                    name           = excluded.name,
                    logo_path      = COALESCE(excluded.logo_path, network.logo_path),
                    origin_country = COALESCE(excluded.origin_country, network.origin_country),
                    headquarters   = COALESCE(excluded.headquarters, network.headquarters),
                    homepage       = COALESCE(excluded.homepage, network.homepage)",
            )
            .bind(network.id)
            .bind(&network.name)
            .bind(&network.logo_path)
            .bind(&network.origin_country)
            .bind(&network.headquarters)
            .bind(&network.homepage)
            .execute(&mut *tx)
            .await?;
        }
        sqlx::query("DELETE FROM video_network WHERE video_type = ? AND video_id = ?")
            .bind(u.video_type.as_str())
            .bind(u.id)
            .execute(&mut *tx)
            .await?;
        for network in &u.networks {
            sqlx::query(
                "INSERT OR IGNORE INTO video_network (video_type, video_id, network_id)
                 VALUES (?, ?, ?)",
            )
            .bind(u.video_type.as_str())
            .bind(u.id)
            .bind(network.id)
            .execute(&mut *tx)
            .await?;
        }

        if !u.watch_providers.is_empty() {
            for (_, provider) in &u.watch_providers {
                sqlx::query(
                    "INSERT INTO watch_provider
                        (provider_id, provider_name, logo_path, display_priority)
                     VALUES (?, ?, ?, ?)
                     ON CONFLICT(provider_id) DO UPDATE SET
                        provider_name    = excluded.provider_name,
                        logo_path        = COALESCE(excluded.logo_path,
                                                    watch_provider.logo_path),
                        display_priority = COALESCE(excluded.display_priority,
                                                    watch_provider.display_priority)",
                )
                .bind(provider.provider_id)
                .bind(&provider.provider_name)
                .bind(&provider.logo_path)
                .bind(provider.display_priority)
                .execute(&mut *tx)
                .await?;
            }
            sqlx::query("DELETE FROM video_watch_provider WHERE video_type = ? AND video_id = ?")
                .bind(u.video_type.as_str())
                .bind(u.id)
                .execute(&mut *tx)
                .await?;
            for (kind, provider) in &u.watch_providers {
                sqlx::query(
                    "INSERT OR IGNORE INTO video_watch_provider
                        (video_type, video_id, provider_id, kind)
                     VALUES (?, ?, ?, ?)",
                )
                .bind(u.video_type.as_str())
                .bind(u.id)
                .bind(provider.provider_id)
                .bind(kind.as_str())
                .execute(&mut *tx)
                .await?;
            }
        }

        tx.commit().await?;
        Ok(())
    }

    async fn delete_video(&self, video_type: VideoType, id: i64) -> Result<bool> {
        let mut tx = self.pool.begin().await?;
        let vt = video_type.as_str();

        // Deleting a show takes its seasons and episodes with it.
        if video_type == VideoType::Tvshow {
            let seasons: Vec<i64> = sqlx::query(
                "SELECT video_id FROM video WHERE video_type = 'tvseason' AND tv_show_id = ?",
            )
            .bind(id)
            .fetch_all(&mut *tx)
            .await?
            .into_iter()
            .map(|r| r.get::<i64, _>("video_id"))
            .collect();
            for season_id in seasons {
                for table in [
                    "video_tag",
                    "video_genre",
                    "video_network",
                    "video_watch_provider",
                ] {
                    sqlx::query(AssertSqlSafe(format!(
                        "DELETE FROM {table} WHERE video_type = 'tvseason' AND video_id = ?"
                    )))
                    .bind(season_id)
                    .execute(&mut *tx)
                    .await?;
                }
                sqlx::query("DELETE FROM credit WHERE video_type = 'tvseason' AND video_id = ?")
                    .bind(season_id)
                    .execute(&mut *tx)
                    .await?;
            }
            sqlx::query("DELETE FROM video WHERE video_type = 'tvseason' AND tv_show_id = ?")
                .bind(id)
                .execute(&mut *tx)
                .await?;
            sqlx::query("DELETE FROM tv_episode WHERE tv_show_id = ?")
                .bind(id)
                .execute(&mut *tx)
                .await?;
            sqlx::query("DELETE FROM episode_tag WHERE tv_show_id = ?")
                .bind(id)
                .execute(&mut *tx)
                .await?;
            sqlx::query("DELETE FROM episode_crew WHERE tv_show_id = ?")
                .bind(id)
                .execute(&mut *tx)
                .await?;
            sqlx::query("DELETE FROM image WHERE owner_type = 'tvseason' AND owner_id = ?")
                .bind(id)
                .execute(&mut *tx)
                .await?;
            sqlx::query("DELETE FROM image_fetch WHERE owner_type = 'tvseason' AND owner_id = ?")
                .bind(id)
                .execute(&mut *tx)
                .await?;
        }

        for table in [
            "video_tag",
            "video_genre",
            "video_network",
            "video_watch_provider",
        ] {
            sqlx::query(AssertSqlSafe(format!(
                "DELETE FROM {table} WHERE video_type = ? AND video_id = ?"
            )))
            .bind(vt)
            .bind(id)
            .execute(&mut *tx)
            .await?;
        }
        sqlx::query("DELETE FROM credit WHERE video_type = ? AND video_id = ?")
            .bind(vt)
            .bind(id)
            .execute(&mut *tx)
            .await?;
        sqlx::query("DELETE FROM recommendation WHERE source_type = ? AND source_id = ?")
            .bind(vt)
            .bind(id)
            .execute(&mut *tx)
            .await?;
        sqlx::query("DELETE FROM recommendation_meta WHERE source_type = ? AND source_id = ?")
            .bind(vt)
            .bind(id)
            .execute(&mut *tx)
            .await?;

        let owner_type = match video_type {
            VideoType::Tvshow => "tvshow",
            VideoType::Tvseason => "tvseason",
            _ => "movie",
        };
        sqlx::query("DELETE FROM image WHERE owner_type = ? AND owner_id = ?")
            .bind(owner_type)
            .bind(id)
            .execute(&mut *tx)
            .await?;
        sqlx::query("DELETE FROM image_fetch WHERE owner_type = ? AND owner_id = ?")
            .bind(owner_type)
            .bind(id)
            .execute(&mut *tx)
            .await?;

        let result = sqlx::query("DELETE FROM video WHERE video_type = ? AND video_id = ?")
            .bind(vt)
            .bind(id)
            .execute(&mut *tx)
            .await?;

        tx.commit().await?;
        Ok(result.rows_affected() > 0)
    }

    async fn set_flag(
        &self,
        video_type: VideoType,
        id: i64,
        flag: Flag,
        value: bool,
    ) -> Result<()> {
        let column = flag.column();
        let sql = format!("UPDATE video SET {column} = ? WHERE video_type = ? AND video_id = ?");
        sqlx::query(AssertSqlSafe(sql.as_str()))
            .bind(value)
            .bind(video_type.as_str())
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    async fn set_tag(&self, video_type: VideoType, id: i64, tag: &str, on: bool) -> Result<()> {
        if on {
            sqlx::query(
                "INSERT OR IGNORE INTO video_tag (video_type, video_id, tag) VALUES (?, ?, ?)",
            )
            .bind(video_type.as_str())
            .bind(id)
            .bind(tag)
            .execute(&self.pool)
            .await?;
        } else {
            sqlx::query("DELETE FROM video_tag WHERE video_type = ? AND video_id = ? AND tag = ?")
                .bind(video_type.as_str())
                .bind(id)
                .bind(tag)
                .execute(&self.pool)
                .await?;
        }
        Ok(())
    }

    async fn apply_overrides(
        &self,
        video_type: VideoType,
        id: i64,
        o: &VideoOverrides,
    ) -> Result<()> {
        if o.is_empty() {
            return Ok(());
        }
        // Only the fields actually present in the patch are written.
        let mut sets: Vec<&str> = Vec::new();
        if o.poster_path.is_some() {
            sets.push("poster_path_override = ?");
        }
        if o.backdrop_path.is_some() {
            sets.push("backdrop_path_override = ?");
        }
        if o.overview.is_some() {
            sets.push("overview_override = ?");
        }
        if o.imdb_id.is_some() {
            sets.push("imdb_id_override = ?");
        }
        if o.wer_streamt_es_id.is_some() {
            sets.push("wer_streamt_es_id = ?");
        }
        let sql = format!(
            "UPDATE video SET {} WHERE video_type = ? AND video_id = ?",
            sets.join(", ")
        );
        let mut q = sqlx::query(AssertSqlSafe(sql.as_str()));
        if let Some(v) = &o.poster_path {
            q = q.bind(v);
        }
        if let Some(v) = &o.backdrop_path {
            q = q.bind(v);
        }
        if let Some(v) = &o.overview {
            q = q.bind(v);
        }
        if let Some(v) = &o.imdb_id {
            q = q.bind(v);
        }
        if let Some(v) = o.wer_streamt_es_id {
            q = q.bind(v as i64);
        }
        q.bind(video_type.as_str())
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    async fn local_states(
        &self,
        video_type: VideoType,
        ids: &[i64],
    ) -> Result<HashMap<i64, LocalState>> {
        let mut out: HashMap<i64, LocalState> = HashMap::new();
        if ids.is_empty() {
            return Ok(out);
        }

        // Chunked for the same reason as `hydrate`: the caller's id list is not
        // guaranteed to fit inside SQLite's bound-parameter limit.
        for ids in ids.chunks(BIND_CHUNK) {
            let ph = placeholders(ids.len());
            let sql = format!(
                "SELECT video_id, favorite, on_watchlist, watched FROM video
                 WHERE video_type = ? AND video_id IN ({ph})"
            );
            let mut q = sqlx::query(AssertSqlSafe(sql.as_str())).bind(video_type.as_str());
            for id in ids {
                q = q.bind(id);
            }
            for row in q.fetch_all(&self.pool).await? {
                out.insert(
                    row.get("video_id"),
                    LocalState {
                        favorite: row.get("favorite"),
                        on_watchlist: row.get("on_watchlist"),
                        watched: row.get("watched"),
                        tags: Vec::new(),
                    },
                );
            }

            let sql = format!(
                "SELECT video_id, tag FROM video_tag
                 WHERE video_type = ? AND video_id IN ({ph}) ORDER BY tag"
            );
            let mut q = sqlx::query(AssertSqlSafe(sql.as_str())).bind(video_type.as_str());
            for id in ids {
                q = q.bind(id);
            }
            for row in q.fetch_all(&self.pool).await? {
                let id: i64 = row.get("video_id");
                out.entry(id).or_default().tags.push(row.get("tag"));
            }
        }
        Ok(out)
    }

    async fn list_seasons(&self, tv_show_id: i64) -> Result<Vec<VideoBase>> {
        let sql = format!(
            "SELECT {VIDEO_COLS} FROM video
             WHERE video_type = 'tvseason' AND tv_show_id = ?
             ORDER BY season_number"
        );
        let rows = sqlx::query_as::<_, VideoRow>(AssertSqlSafe(sql.as_str()))
            .bind(tv_show_id)
            .fetch_all(&self.pool)
            .await?;
        self.hydrate(rows).await
    }

    async fn get_season(&self, tv_show_id: i64, season_number: i64) -> Result<Option<VideoBase>> {
        let sql = format!(
            "SELECT {VIDEO_COLS} FROM video
             WHERE video_type = 'tvseason' AND tv_show_id = ? AND season_number = ?"
        );
        let row = sqlx::query_as::<_, VideoRow>(AssertSqlSafe(sql.as_str()))
            .bind(tv_show_id)
            .bind(season_number)
            .fetch_optional(&self.pool)
            .await?;
        match row {
            None => Ok(None),
            Some(row) => Ok(self.hydrate(vec![row]).await?.into_iter().next()),
        }
    }

    async fn list_episodes(&self, tv_show_id: i64, season_number: i64) -> Result<Vec<TvEpisode>> {
        let rows = sqlx::query(
            "SELECT episode_id, episode_number, name, overview, air_date, still_path,
                    vote_average, vote_count, production_code, season_number,
                    favorite, on_watchlist, watched
             FROM tv_episode
             WHERE tv_show_id = ? AND season_number = ?
             ORDER BY episode_number",
        )
        .bind(tv_show_id)
        .bind(season_number)
        .fetch_all(&self.pool)
        .await?;

        let mut episodes: Vec<TvEpisode> = rows
            .iter()
            .map(|row| TvEpisode {
                id: row.get("episode_id"),
                air_date: row.get("air_date"),
                episode_number: row.get("episode_number"),
                crew: Vec::new(),
                name: row.get("name"),
                overview: row.get("overview"),
                production_code: row.get("production_code"),
                season_number: row.get("season_number"),
                still_path: row.get("still_path"),
                vote_average: row.get::<Option<f64>, _>("vote_average").map(|v| v as f32),
                vote_count: row.get("vote_count"),
                on_watchlist: row.get("on_watchlist"),
                favorite: row.get("favorite"),
                watched: row.get("watched"),
                tags: Vec::new(),
            })
            .collect();

        // Batch the per-episode tags and crew.
        let mut tags: HashMap<i64, Vec<String>> = HashMap::new();
        for row in sqlx::query(
            "SELECT episode_number, tag FROM episode_tag
             WHERE tv_show_id = ? AND season_number = ? ORDER BY tag",
        )
        .bind(tv_show_id)
        .bind(season_number)
        .fetch_all(&self.pool)
        .await?
        {
            tags.entry(row.get("episode_number"))
                .or_default()
                .push(row.get("tag"));
        }

        let mut crew: HashMap<i64, Vec<Crew>> = HashMap::new();
        for row in sqlx::query(
            "SELECT ec.episode_number, ec.person_id, ec.department, ec.job,
                    p.name, p.original_name, p.gender, p.popularity,
                    p.known_for_department, p.adult, NULL AS credit_id,
                    COALESCE(p.profile_path_override, p.profile_path) AS profile_path
             FROM episode_crew ec
             JOIN person p ON p.id = ec.person_id
             WHERE ec.tv_show_id = ? AND ec.season_number = ?",
        )
        .bind(tv_show_id)
        .bind(season_number)
        .fetch_all(&self.pool)
        .await?
        {
            let ep: i64 = row.get("episode_number");
            crew.entry(ep).or_default().push(Crew {
                person: person_base_from_row(&row),
                department: row.get("department"),
                job: row.get("job"),
            });
        }

        for episode in &mut episodes {
            episode.tags = tags.remove(&episode.episode_number).unwrap_or_default();
            episode.crew = crew.remove(&episode.episode_number).unwrap_or_default();
        }
        Ok(episodes)
    }

    async fn replace_episodes(
        &self,
        tv_show_id: i64,
        season_number: i64,
        episodes: &[TvEpisode],
    ) -> Result<()> {
        let mut tx = self.pool.begin().await?;

        // Upsert rather than delete-and-insert so watched/favorite flags and tags
        // survive a refresh.
        for episode in episodes {
            sqlx::query(
                "INSERT INTO tv_episode (
                    tv_show_id, season_number, episode_number, episode_id, name,
                    overview, air_date, still_path, vote_average, vote_count,
                    production_code
                 ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
                 ON CONFLICT(tv_show_id, season_number, episode_number) DO UPDATE SET
                    episode_id      = excluded.episode_id,
                    name            = excluded.name,
                    overview        = excluded.overview,
                    air_date        = excluded.air_date,
                    still_path      = excluded.still_path,
                    vote_average    = excluded.vote_average,
                    vote_count      = excluded.vote_count,
                    production_code = excluded.production_code",
            )
            .bind(tv_show_id)
            .bind(season_number)
            .bind(episode.episode_number)
            .bind(episode.id)
            .bind(&episode.name)
            .bind(&episode.overview)
            .bind(&episode.air_date)
            .bind(&episode.still_path)
            .bind(episode.vote_average.map(|v| v as f64))
            .bind(episode.vote_count)
            .bind(&episode.production_code)
            .execute(&mut *tx)
            .await?;

            sqlx::query(
                "DELETE FROM episode_crew
                 WHERE tv_show_id = ? AND season_number = ? AND episode_number = ?",
            )
            .bind(tv_show_id)
            .bind(season_number)
            .bind(episode.episode_number)
            .execute(&mut *tx)
            .await?;

            for member in &episode.crew {
                sqlx::query(
                    "INSERT INTO person (id, name, original_name, profile_path, gender,
                                         popularity, known_for_department, adult)
                     VALUES (?, ?, ?, ?, ?, ?, ?, ?)
                     ON CONFLICT(id) DO UPDATE SET
                        name         = COALESCE(excluded.name, person.name),
                        profile_path = COALESCE(excluded.profile_path, person.profile_path)",
                )
                .bind(member.person.id)
                .bind(&member.person.name)
                .bind(&member.person.original_name)
                .bind(&member.person.profile_path)
                .bind(member.person.gender)
                .bind(member.person.popularity.map(|p| p as f64))
                .bind(&member.person.known_for_department)
                .bind(member.person.adult)
                .execute(&mut *tx)
                .await?;

                sqlx::query(
                    "INSERT OR IGNORE INTO episode_crew
                        (tv_show_id, season_number, episode_number, person_id, department, job)
                     VALUES (?, ?, ?, ?, ?, ?)",
                )
                .bind(tv_show_id)
                .bind(season_number)
                .bind(episode.episode_number)
                .bind(member.person.id)
                .bind(&member.department)
                .bind(&member.job)
                .execute(&mut *tx)
                .await?;
            }
        }

        tx.commit().await?;
        Ok(())
    }

    async fn get_episode(
        &self,
        tv_show_id: i64,
        season_number: i64,
        episode_number: i64,
    ) -> Result<Option<TvEpisode>> {
        Ok(self
            .list_episodes(tv_show_id, season_number)
            .await?
            .into_iter()
            .find(|e| e.episode_number == episode_number))
    }

    async fn set_episode_flag(
        &self,
        tv_show_id: i64,
        season_number: i64,
        episode_number: i64,
        flag: Flag,
        value: bool,
    ) -> Result<()> {
        let column = flag.column();
        let sql = format!(
            "UPDATE tv_episode SET {column} = ?
             WHERE tv_show_id = ? AND season_number = ? AND episode_number = ?"
        );
        sqlx::query(AssertSqlSafe(sql.as_str()))
            .bind(value)
            .bind(tv_show_id)
            .bind(season_number)
            .bind(episode_number)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    async fn set_episode_tag(
        &self,
        tv_show_id: i64,
        season_number: i64,
        episode_number: i64,
        tag: &str,
        on: bool,
    ) -> Result<()> {
        if on {
            sqlx::query(
                "INSERT OR IGNORE INTO episode_tag
                    (tv_show_id, season_number, episode_number, tag)
                 VALUES (?, ?, ?, ?)",
            )
            .bind(tv_show_id)
            .bind(season_number)
            .bind(episode_number)
            .bind(tag)
            .execute(&self.pool)
            .await?;
        } else {
            sqlx::query(
                "DELETE FROM episode_tag
                 WHERE tv_show_id = ? AND season_number = ? AND episode_number = ? AND tag = ?",
            )
            .bind(tv_show_id)
            .bind(season_number)
            .bind(episode_number)
            .bind(tag)
            .execute(&self.pool)
            .await?;
        }
        Ok(())
    }

    async fn get_credits(&self, video_type: VideoType, id: i64) -> Result<Option<Credits>> {
        if !self.has_credits(video_type, id).await? {
            return Ok(None);
        }
        let sql = format!(
            "SELECT {CREDIT_COLS}, c.kind FROM credit c
             JOIN person p ON p.id = c.person_id
             WHERE c.video_type = ? AND c.video_id = ?
             ORDER BY c.kind, c.ord, p.name"
        );
        let rows = sqlx::query(AssertSqlSafe(sql.as_str()))
            .bind(video_type.as_str())
            .bind(id)
            .fetch_all(&self.pool)
            .await?;

        let mut credits = Credits {
            id,
            cast: Vec::new(),
            crew: Vec::new(),
        };
        for row in &rows {
            let person = person_base_from_row(row);
            match row.get::<String, _>("kind").as_str() {
                "cast" => credits.cast.push(Cast {
                    person,
                    cast_id: row.get("cast_id"),
                    character: row.get("character"),
                    order: row.get("ord"),
                }),
                _ => credits.crew.push(Crew {
                    person,
                    department: row.get("department"),
                    job: row.get("job"),
                }),
            }
        }
        Ok(Some(credits))
    }

    async fn get_tv_credits(&self, id: i64) -> Result<Option<TvShowCredits>> {
        if !self.has_credits(VideoType::Tvshow, id).await? {
            return Ok(None);
        }
        let sql = format!(
            "SELECT {CREDIT_COLS} FROM credit c
             JOIN person p ON p.id = c.person_id
             WHERE c.video_type = 'tvshow' AND c.video_id = ? AND c.kind = 'cast'
             ORDER BY c.ord, p.name"
        );
        let rows = sqlx::query(AssertSqlSafe(sql.as_str()))
            .bind(id)
            .fetch_all(&self.pool)
            .await?;
        Ok(Some(TvShowCredits {
            id,
            cast: rows
                .iter()
                .map(|row| TvShowCast {
                    person: person_base_from_row(row),
                    cast_id: row.get("cast_id"),
                    character: row.get("character"),
                    order: row.get("ord"),
                })
                .collect(),
        }))
    }

    async fn get_directors(&self, id: i64) -> Result<Vec<Crew>> {
        let sql = format!(
            "SELECT {CREDIT_COLS} FROM credit c
             JOIN person p ON p.id = c.person_id
             WHERE c.video_type = 'movie' AND c.video_id = ?
               AND c.kind = 'crew' AND c.job = 'Director'
             ORDER BY c.ord, p.name"
        );
        let rows = sqlx::query(AssertSqlSafe(sql.as_str()))
            .bind(id)
            .fetch_all(&self.pool)
            .await?;
        Ok(rows
            .iter()
            .map(|row| Crew {
                person: person_base_from_row(row),
                department: row.get("department"),
                job: row.get("job"),
            })
            .collect())
    }

    async fn replace_credits(
        &self,
        video_type: VideoType,
        id: i64,
        cast: &[Cast],
        crew: &[Crew],
    ) -> Result<()> {
        for member in cast {
            self.upsert_person_stub(&member.person).await?;
        }
        for member in crew {
            self.upsert_person_stub(&member.person).await?;
        }

        let mut tx = self.pool.begin().await?;
        sqlx::query("DELETE FROM credit WHERE video_type = ? AND video_id = ?")
            .bind(video_type.as_str())
            .bind(id)
            .execute(&mut *tx)
            .await?;

        for (index, member) in cast.iter().enumerate() {
            sqlx::query(
                "INSERT OR IGNORE INTO credit
                    (video_type, video_id, person_id, kind, character, cast_id, credit_id, ord)
                 VALUES (?, ?, ?, 'cast', ?, ?, ?, ?)",
            )
            .bind(video_type.as_str())
            .bind(id)
            .bind(member.person.id)
            .bind(&member.character)
            .bind(member.cast_id)
            .bind(&member.person.credit_id)
            .bind(member.order.unwrap_or(index as i64))
            .execute(&mut *tx)
            .await?;
        }

        for (index, member) in crew.iter().enumerate() {
            sqlx::query(
                "INSERT OR IGNORE INTO credit
                    (video_type, video_id, person_id, kind, department, job, credit_id, ord)
                 VALUES (?, ?, ?, 'crew', ?, ?, ?, ?)",
            )
            .bind(video_type.as_str())
            .bind(id)
            .bind(member.person.id)
            .bind(&member.department)
            .bind(&member.job)
            .bind(&member.person.credit_id)
            .bind(index as i64)
            .execute(&mut *tx)
            .await?;
        }

        tx.commit().await?;
        Ok(())
    }

    async fn has_credits(&self, video_type: VideoType, id: i64) -> Result<bool> {
        let row = sqlx::query("SELECT 1 FROM credit WHERE video_type = ? AND video_id = ? LIMIT 1")
            .bind(video_type.as_str())
            .bind(id)
            .fetch_optional(&self.pool)
            .await?;
        Ok(row.is_some())
    }

    async fn get_person(&self, id: i64) -> Result<Option<PersonRecord>> {
        let row = sqlx::query(
            "SELECT id, name, place_of_birth, biography, birthday, deathday, gender,
                    imdb_id, adult, fetched_at,
                    COALESCE(profile_path_override, profile_path) AS profile_path
             FROM person WHERE id = ?",
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await?;

        Ok(row.map(|row| PersonRecord {
            id: row.get("id"),
            name: row.get("name"),
            profile_path: row.get("profile_path"),
            place_of_birth: row.get("place_of_birth"),
            biography: row.get("biography"),
            birthday: row.get("birthday"),
            deathday: row.get("deathday"),
            gender: row.get("gender"),
            imdb_id: row.get("imdb_id"),
            adult: row.get("adult"),
            fetched_at: row.get("fetched_at"),
        }))
    }

    async fn upsert_person(&self, person: &PersonUpsert, mark_fetched: bool) -> Result<()> {
        sqlx::query(
            "INSERT INTO person (id, name, original_name, profile_path, place_of_birth,
                                 biography, birthday, deathday, gender, imdb_id, adult,
                                 popularity, known_for_department, fetched_at)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
             ON CONFLICT(id) DO UPDATE SET
                name                 = COALESCE(excluded.name, person.name),
                original_name        = COALESCE(excluded.original_name, person.original_name),
                profile_path         = COALESCE(excluded.profile_path, person.profile_path),
                place_of_birth       = COALESCE(excluded.place_of_birth, person.place_of_birth),
                biography            = COALESCE(excluded.biography, person.biography),
                birthday             = COALESCE(excluded.birthday, person.birthday),
                deathday             = COALESCE(excluded.deathday, person.deathday),
                gender               = COALESCE(excluded.gender, person.gender),
                imdb_id              = COALESCE(excluded.imdb_id, person.imdb_id),
                adult                = COALESCE(excluded.adult, person.adult),
                popularity           = COALESCE(excluded.popularity, person.popularity),
                known_for_department = COALESCE(excluded.known_for_department,
                                                person.known_for_department),
                fetched_at           = COALESCE(excluded.fetched_at, person.fetched_at)",
        )
        .bind(person.id)
        .bind(&person.name)
        .bind(&person.original_name)
        .bind(&person.profile_path)
        .bind(&person.place_of_birth)
        .bind(&person.biography)
        .bind(&person.birthday)
        .bind(&person.deathday)
        .bind(person.gender)
        .bind(&person.imdb_id)
        .bind(person.adult)
        .bind(person.popularity.map(|p| p as f64))
        .bind(&person.known_for_department)
        .bind(if mark_fetched { Some(now()) } else { None })
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    async fn set_person_profile_override(&self, id: i64, profile_path: Option<&str>) -> Result<()> {
        sqlx::query("UPDATE person SET profile_path_override = ? WHERE id = ?")
            .bind(profile_path)
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    async fn person_credits(
        &self,
        person_id: i64,
        limits: PersonCreditLimits,
    ) -> Result<PersonCredits> {
        // Only titles held locally are cross-referenced; the person page links
        // into the library, not out to TMDB.
        let movie_cast = sqlx::query(
            "SELECT c.video_id, c.character FROM credit c
             JOIN video v ON v.video_type = 'movie' AND v.video_id = c.video_id
             WHERE c.person_id = ? AND c.kind = 'cast' AND c.video_type = 'movie'
             ORDER BY v.release_date DESC, v.display_name COLLATE NOCASE
             LIMIT ?",
        )
        .bind(person_id)
        .bind(limits.movie_cast)
        .fetch_all(&self.pool)
        .await?;

        let directed = sqlx::query(
            "SELECT c.video_id, c.job FROM credit c
             JOIN video v ON v.video_type = 'movie' AND v.video_id = c.video_id
             WHERE c.person_id = ? AND c.kind = 'crew' AND c.video_type = 'movie'
               AND c.job = 'Director'
             ORDER BY v.release_date DESC, v.display_name COLLATE NOCASE
             LIMIT ?",
        )
        .bind(person_id)
        .bind(limits.directed)
        .fetch_all(&self.pool)
        .await?;

        let tv_cast = sqlx::query(
            "SELECT c.video_id, c.character FROM credit c
             JOIN video v ON v.video_type = 'tvshow' AND v.video_id = c.video_id
             WHERE c.person_id = ? AND c.kind = 'cast' AND c.video_type = 'tvshow'
             ORDER BY v.display_name COLLATE NOCASE
             LIMIT ?",
        )
        .bind(person_id)
        .bind(limits.tv_cast)
        .fetch_all(&self.pool)
        .await?;

        Ok(PersonCredits {
            movie_cast: movie_cast
                .iter()
                .map(|row| CastReference {
                    cast_id: row.get("video_id"),
                    character: row
                        .get::<Option<String>, _>("character")
                        .unwrap_or_default(),
                })
                .collect(),
            directed_movies: directed
                .iter()
                .map(|row| CastReference {
                    cast_id: row.get("video_id"),
                    character: row
                        .get::<Option<String>, _>("job")
                        .unwrap_or_else(|| "Director".to_string()),
                })
                .collect(),
            tv_cast: tv_cast
                .iter()
                .map(|row| CastReference {
                    cast_id: row.get("video_id"),
                    character: row
                        .get::<Option<String>, _>("character")
                        .unwrap_or_default(),
                })
                .collect(),
        })
    }

    async fn get_images(&self, owner: ImageOwner, owner_id: i64) -> Result<Option<Images>> {
        let fetched = sqlx::query(
            "SELECT 1 FROM image_fetch
             WHERE owner_type = ? AND owner_id = ? AND season_number = ?",
        )
        .bind(owner.type_str())
        .bind(owner_id)
        .bind(owner.season_key())
        .fetch_optional(&self.pool)
        .await?;
        if fetched.is_none() {
            return Ok(None);
        }

        let rows = sqlx::query(
            "SELECT kind, file_path, aspect_ratio, height, width, vote_average, vote_count
             FROM image
             WHERE owner_type = ? AND owner_id = ? AND season_number = ?
             ORDER BY kind, ord",
        )
        .bind(owner.type_str())
        .bind(owner_id)
        .bind(owner.season_key())
        .fetch_all(&self.pool)
        .await?;

        let mut images = Images {
            id: Some(owner_id),
            tv_show_id: matches!(owner, ImageOwner::TvSeason { .. }).then_some(owner_id),
            season_number: match owner {
                ImageOwner::TvSeason { season_number } => Some(season_number),
                _ => None,
            },
            ..Default::default()
        };

        for row in &rows {
            let image = Image {
                aspect_ratio: row.get::<Option<f64>, _>("aspect_ratio").map(|v| v as f32),
                height: row.get("height"),
                file_path: row.get("file_path"),
                vote_average: row.get::<Option<f64>, _>("vote_average").map(|v| v as f32),
                vote_count: row.get("vote_count"),
                width: row.get("width"),
            };
            match row.get::<String, _>("kind").as_str() {
                "backdrop" => images.backdrops.push(image),
                "logo" => images.logos.push(image),
                // Person profiles are surfaced through the `posters` slot; the
                // /profiles endpoint reads them back out.
                "poster" | "profile" => images.posters.push(image),
                _ => {}
            }
        }
        Ok(Some(images))
    }

    async fn replace_images(
        &self,
        owner: ImageOwner,
        owner_id: i64,
        backdrops: &[Image],
        posters: &[Image],
        logos: &[Image],
    ) -> Result<()> {
        let mut tx = self.pool.begin().await?;
        let season_key = owner.season_key();

        sqlx::query(
            "DELETE FROM image WHERE owner_type = ? AND owner_id = ? AND season_number = ?",
        )
        .bind(owner.type_str())
        .bind(owner_id)
        .bind(season_key)
        .execute(&mut *tx)
        .await?;

        let poster_kind = if owner == ImageOwner::Person {
            "profile"
        } else {
            "poster"
        };

        for (kind, list) in [
            ("backdrop", backdrops),
            (poster_kind, posters),
            ("logo", logos),
        ] {
            for (index, image) in list.iter().enumerate() {
                let Some(path) = &image.file_path else {
                    continue;
                };
                sqlx::query(
                    "INSERT OR REPLACE INTO image
                        (owner_type, owner_id, season_number, kind, file_path,
                         aspect_ratio, height, width, vote_average, vote_count, ord)
                     VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
                )
                .bind(owner.type_str())
                .bind(owner_id)
                .bind(season_key)
                .bind(kind)
                .bind(path)
                .bind(image.aspect_ratio.map(|v| v as f64))
                .bind(image.height)
                .bind(image.width)
                .bind(image.vote_average.map(|v| v as f64))
                .bind(image.vote_count)
                .bind(index as i64)
                .execute(&mut *tx)
                .await?;
            }
        }

        sqlx::query(
            "INSERT OR REPLACE INTO image_fetch
                (owner_type, owner_id, season_number, fetched_at)
             VALUES (?, ?, ?, ?)",
        )
        .bind(owner.type_str())
        .bind(owner_id)
        .bind(season_key)
        .bind(now())
        .execute(&mut *tx)
        .await?;

        tx.commit().await?;
        Ok(())
    }

    async fn get_recommendations(
        &self,
        source_type: VideoType,
        id: i64,
    ) -> Result<Option<RecommendationSet>> {
        let meta = sqlx::query(
            "SELECT page, total_pages, total_results FROM recommendation_meta
             WHERE source_type = ? AND source_id = ?",
        )
        .bind(source_type.as_str())
        .bind(id)
        .fetch_optional(&self.pool)
        .await?;
        let Some(meta) = meta else {
            return Ok(None);
        };

        let rows = sqlx::query(
            "SELECT rec_id, display_name, poster_path, backdrop_path, vote_average,
                    adult, rec_type, release_date, first_air_date, age_rating
             FROM recommendation
             WHERE source_type = ? AND source_id = ?
             ORDER BY ord",
        )
        .bind(source_type.as_str())
        .bind(id)
        .fetch_all(&self.pool)
        .await?;

        Ok(Some(RecommendationSet {
            page: meta.get("page"),
            total_pages: meta.get("total_pages"),
            total_results: meta.get("total_results"),
            items: rows
                .iter()
                .map(|row| StoredRecommendation {
                    id: row.get("rec_id"),
                    display_name: row.get("display_name"),
                    poster_path: row.get("poster_path"),
                    backdrop_path: row.get("backdrop_path"),
                    vote_average: row.get("vote_average"),
                    adult: row.get("adult"),
                    rec_type: row.get("rec_type"),
                    release_date: row.get("release_date"),
                    first_air_date: row.get("first_air_date"),
                    age_rating: row.get("age_rating"),
                })
                .collect(),
        }))
    }

    async fn replace_recommendations(
        &self,
        source_type: VideoType,
        id: i64,
        set: &RecommendationSet,
    ) -> Result<()> {
        let mut tx = self.pool.begin().await?;

        sqlx::query("DELETE FROM recommendation WHERE source_type = ? AND source_id = ?")
            .bind(source_type.as_str())
            .bind(id)
            .execute(&mut *tx)
            .await?;

        for (index, item) in set.items.iter().enumerate() {
            sqlx::query(
                "INSERT OR REPLACE INTO recommendation
                    (source_type, source_id, rec_id, ord, display_name, poster_path,
                     backdrop_path, vote_average, adult, rec_type, release_date,
                     first_air_date, age_rating)
                 VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
            )
            .bind(source_type.as_str())
            .bind(id)
            .bind(item.id)
            .bind(index as i64)
            .bind(&item.display_name)
            .bind(&item.poster_path)
            .bind(&item.backdrop_path)
            .bind(item.vote_average)
            .bind(item.adult)
            .bind(&item.rec_type)
            .bind(&item.release_date)
            .bind(&item.first_air_date)
            .bind(&item.age_rating)
            .execute(&mut *tx)
            .await?;
        }

        sqlx::query(
            "INSERT OR REPLACE INTO recommendation_meta
                (source_type, source_id, page, total_pages, total_results, fetched_at)
             VALUES (?, ?, ?, ?, ?, ?)",
        )
        .bind(source_type.as_str())
        .bind(id)
        .bind(set.page)
        .bind(set.total_pages)
        .bind(set.total_results)
        .bind(now())
        .execute(&mut *tx)
        .await?;

        tx.commit().await?;
        Ok(())
    }

    async fn top_recommendations(&self, limit: i64) -> Result<Vec<StoredRecommendation>> {
        // Titles recommended by the most library entries, excluding anything
        // already held. `MAX(...)` picks an arbitrary-but-stable representative
        // row for the display fields.
        let rows = sqlx::query(
            "SELECT r.rec_id,
                    MAX(r.display_name)   AS display_name,
                    MAX(r.poster_path)    AS poster_path,
                    MAX(r.backdrop_path)  AS backdrop_path,
                    MAX(r.vote_average)   AS vote_average,
                    MAX(r.adult)          AS adult,
                    MAX(r.rec_type)       AS rec_type,
                    MAX(r.release_date)   AS release_date,
                    MAX(r.first_air_date) AS first_air_date,
                    MAX(r.age_rating)     AS age_rating,
                    COUNT(*)              AS hits
             FROM recommendation r
             WHERE r.source_type = 'movie'
               AND NOT EXISTS (
                   SELECT 1 FROM video v
                   WHERE v.video_type = 'movie' AND v.video_id = r.rec_id
               )
             GROUP BY r.rec_id
             ORDER BY hits DESC, vote_average DESC
             LIMIT ?",
        )
        .bind(limit)
        .fetch_all(&self.pool)
        .await?;

        Ok(rows
            .iter()
            .map(|row| StoredRecommendation {
                id: row.get("rec_id"),
                display_name: row.get("display_name"),
                poster_path: row.get("poster_path"),
                backdrop_path: row.get("backdrop_path"),
                vote_average: row.get("vote_average"),
                adult: row.get("adult"),
                rec_type: row.get("rec_type"),
                release_date: row.get("release_date"),
                first_air_date: row.get("first_air_date"),
                age_rating: row.get("age_rating"),
            })
            .collect())
    }

    async fn get_collection(&self, id: i64) -> Result<Option<Collection>> {
        let row = sqlx::query(
            "SELECT id, name, overview, poster_path, backdrop_path FROM collection WHERE id = ?",
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await?;
        let Some(row) = row else {
            return Ok(None);
        };

        let part_rows = sqlx::query(
            "SELECT movie_id, display_name, title, original_title, original_language,
                    poster_path, backdrop_path, release_date, overview, vote_average,
                    vote_count, popularity, adult, video
             FROM collection_part WHERE collection_id = ? ORDER BY ord",
        )
        .bind(id)
        .fetch_all(&self.pool)
        .await?;

        // Decorate parts already in the library with their local state.
        let ids: Vec<i64> = part_rows.iter().map(|r| r.get("movie_id")).collect();
        let locals = self.local_states(VideoType::Movie, &ids).await?;

        let parts = part_rows
            .iter()
            .map(|row| {
                let movie_id: i64 = row.get("movie_id");
                let local = locals.get(&movie_id).cloned().unwrap_or_default();
                CollectionPart {
                    base: VideoBase {
                        id: movie_id,
                        video_type: VideoType::Movie,
                        display_name: row.get("display_name"),
                        poster_path: row.get("poster_path"),
                        backdrop_path: row.get("backdrop_path"),
                        favorite: local.favorite,
                        on_watchlist: local.on_watchlist,
                        watched: local.watched,
                        release_date: row.get("release_date"),
                        overview: row.get("overview"),
                        vote_average: row.get("vote_average"),
                        vote_count: row.get("vote_count"),
                        popularity: row.get::<Option<f64>, _>("popularity").map(|p| p as f32),
                        tags: local.tags,
                        adult: row.get("adult"),
                        ..Default::default()
                    },
                    original_language: row.get("original_language"),
                    original_title: row.get("original_title"),
                    title: row.get("title"),
                    video: row.get("video"),
                }
            })
            .collect();

        Ok(Some(Collection {
            id: row.get("id"),
            poster_path: row.get("poster_path"),
            backdrop_path: row.get("backdrop_path"),
            favorite: false,
            on_watchlist: false,
            watched: false,
            name: row.get("name"),
            overview: row.get("overview"),
            parts,
        }))
    }

    async fn upsert_collection(&self, collection: &Collection) -> Result<()> {
        let mut tx = self.pool.begin().await?;

        sqlx::query(
            "INSERT INTO collection (id, name, overview, poster_path, backdrop_path, fetched_at)
             VALUES (?, ?, ?, ?, ?, ?)
             ON CONFLICT(id) DO UPDATE SET
                name          = excluded.name,
                overview      = excluded.overview,
                poster_path   = excluded.poster_path,
                backdrop_path = excluded.backdrop_path,
                fetched_at    = excluded.fetched_at",
        )
        .bind(collection.id)
        .bind(&collection.name)
        .bind(&collection.overview)
        .bind(&collection.poster_path)
        .bind(&collection.backdrop_path)
        .bind(now())
        .execute(&mut *tx)
        .await?;

        // A movie payload's `belongs_to_collection` carries no parts, so an empty
        // list means "nothing new to say" rather than "the collection is empty".
        // Replacing parts only when some are supplied keeps a previously fetched
        // part list intact.
        if !collection.parts.is_empty() {
            sqlx::query("DELETE FROM collection_part WHERE collection_id = ?")
                .bind(collection.id)
                .execute(&mut *tx)
                .await?;
        }

        for (index, part) in collection.parts.iter().enumerate() {
            sqlx::query(
                "INSERT OR REPLACE INTO collection_part
                    (collection_id, movie_id, ord, display_name, title, original_title,
                     original_language, poster_path, backdrop_path, release_date, overview,
                     vote_average, vote_count, popularity, adult, video)
                 VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
            )
            .bind(collection.id)
            .bind(part.base.id)
            .bind(index as i64)
            .bind(&part.base.display_name)
            .bind(&part.title)
            .bind(&part.original_title)
            .bind(&part.original_language)
            .bind(&part.base.poster_path)
            .bind(&part.base.backdrop_path)
            .bind(&part.base.release_date)
            .bind(&part.base.overview)
            .bind(part.base.vote_average)
            .bind(part.base.vote_count)
            .bind(part.base.popularity.map(|p| p as f64))
            .bind(part.base.adult)
            .bind(part.video)
            .execute(&mut *tx)
            .await?;
        }

        tx.commit().await?;
        Ok(())
    }

    async fn get_tmdb_configuration(&self) -> Result<Option<String>> {
        let row = sqlx::query("SELECT payload FROM tmdb_configuration WHERE id = 1")
            .fetch_optional(&self.pool)
            .await?;
        Ok(row.map(|r| r.get("payload")))
    }

    async fn put_tmdb_configuration(&self, payload: &str) -> Result<()> {
        sqlx::query(
            "INSERT OR REPLACE INTO tmdb_configuration (id, payload, fetched_at)
             VALUES (1, ?, ?)",
        )
        .bind(payload)
        .bind(now())
        .execute(&self.pool)
        .await?;
        Ok(())
    }
}
