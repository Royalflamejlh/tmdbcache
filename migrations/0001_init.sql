-- Core cache of TMDB metadata plus the local, user-owned state layered on top.
--
-- Movies, TV shows and TV seasons all live in `video`, keyed by
-- (video_type, video_id). That keeps tags, credits, images and the
-- favorite/watched/watchlist flags uniform across media types, and makes the
-- /api/v1/videos endpoint a single scan.
--
-- Fields the user can override via PATCH are stored in dedicated `*_override`
-- columns so that re-fetching from TMDB never clobbers a manual edit. Reads
-- COALESCE the override over the upstream value.

CREATE TABLE video (
    video_type              TEXT    NOT NULL,
    video_id                INTEGER NOT NULL,

    display_name            TEXT    NOT NULL,
    original_title          TEXT,
    original_language       TEXT,
    overview                TEXT,
    poster_path             TEXT,
    backdrop_path           TEXT,
    release_date            TEXT,
    runtime                 INTEGER,
    tagline                 TEXT,

    -- Scaled to 0..=100 to match the app's rating thresholds.
    vote_average            INTEGER,
    vote_count              INTEGER,
    popularity              REAL,
    adult                   INTEGER NOT NULL DEFAULT 0,
    age_rating              TEXT,

    imdb_id                 TEXT,
    tvdb_id                 TEXT,
    wikidata_id             TEXT,
    facebook_id             TEXT,
    instagram_id            TEXT,
    twitter_id              TEXT,

    emby_id                 INTEGER,
    emby_server_id          TEXT,

    collection_id           INTEGER,
    trailer_key             TEXT,

    -- Season-only columns.
    tv_show_id              INTEGER,
    season_number           INTEGER,
    external_id             TEXT,
    air_date                TEXT,
    episode_count           INTEGER,

    -- User-owned state.
    favorite                INTEGER NOT NULL DEFAULT 0,
    on_watchlist            INTEGER NOT NULL DEFAULT 0,
    watched                 INTEGER NOT NULL DEFAULT 0,

    -- User overrides, applied over the TMDB values on read.
    poster_path_override    TEXT,
    backdrop_path_override  TEXT,
    overview_override       TEXT,
    imdb_id_override        TEXT,
    wer_streamt_es_id       INTEGER,

    -- True once the expensive append_to_response fetch has been done.
    details_loaded          INTEGER NOT NULL DEFAULT 0,
    fetched_at              TEXT    NOT NULL,

    PRIMARY KEY (video_type, video_id)
);

CREATE INDEX idx_video_type ON video (video_type);
CREATE INDEX idx_video_collection ON video (collection_id);
CREATE UNIQUE INDEX idx_video_season
    ON video (tv_show_id, season_number)
    WHERE video_type = 'tvseason';

CREATE TABLE tv_episode (
    tv_show_id      INTEGER NOT NULL,
    season_number   INTEGER NOT NULL,
    episode_number  INTEGER NOT NULL,

    episode_id      INTEGER,
    name            TEXT,
    overview        TEXT,
    air_date        TEXT,
    still_path      TEXT,
    -- Episodes keep TMDB's 0..=10 float scale.
    vote_average    REAL,
    vote_count      INTEGER,
    production_code TEXT,

    favorite        INTEGER NOT NULL DEFAULT 0,
    on_watchlist    INTEGER NOT NULL DEFAULT 0,
    watched         INTEGER NOT NULL DEFAULT 0,

    PRIMARY KEY (tv_show_id, season_number, episode_number)
);

CREATE TABLE video_tag (
    video_type  TEXT NOT NULL,
    video_id    INTEGER NOT NULL,
    tag         TEXT NOT NULL,
    PRIMARY KEY (video_type, video_id, tag)
);

CREATE INDEX idx_video_tag_tag ON video_tag (tag);

CREATE TABLE episode_tag (
    tv_show_id      INTEGER NOT NULL,
    season_number   INTEGER NOT NULL,
    episode_number  INTEGER NOT NULL,
    tag             TEXT    NOT NULL,
    PRIMARY KEY (tv_show_id, season_number, episode_number, tag)
);

CREATE TABLE genre (
    id   INTEGER PRIMARY KEY,
    name TEXT NOT NULL
);

CREATE TABLE video_genre (
    video_type TEXT    NOT NULL,
    video_id   INTEGER NOT NULL,
    genre_id   INTEGER NOT NULL,
    PRIMARY KEY (video_type, video_id, genre_id)
);

CREATE TABLE network (
    id             INTEGER PRIMARY KEY,
    name           TEXT NOT NULL,
    logo_path      TEXT,
    origin_country TEXT,
    headquarters   TEXT,
    homepage       TEXT
);

CREATE TABLE video_network (
    video_type TEXT    NOT NULL,
    video_id   INTEGER NOT NULL,
    network_id INTEGER NOT NULL,
    PRIMARY KEY (video_type, video_id, network_id)
);

CREATE TABLE watch_provider (
    provider_id      INTEGER PRIMARY KEY,
    provider_name    TEXT NOT NULL,
    logo_path        TEXT,
    display_priority INTEGER
);

CREATE TABLE video_watch_provider (
    video_type  TEXT    NOT NULL,
    video_id    INTEGER NOT NULL,
    provider_id INTEGER NOT NULL,
    -- 'buy' | 'rent' | 'flatrate'
    kind        TEXT    NOT NULL,
    PRIMARY KEY (video_type, video_id, provider_id, kind)
);

CREATE TABLE person (
    id                    INTEGER PRIMARY KEY,
    name                  TEXT,
    original_name         TEXT,
    profile_path          TEXT,
    profile_path_override TEXT,
    place_of_birth        TEXT,
    biography             TEXT,
    birthday              TEXT,
    deathday              TEXT,
    gender                INTEGER,
    imdb_id               TEXT,
    adult                 INTEGER,
    popularity            REAL,
    known_for_department  TEXT,
    -- NULL until the person's own TMDB record has been fetched; rows created as
    -- a side effect of caching credits start out as stubs.
    fetched_at            TEXT
);

CREATE TABLE credit (
    video_type TEXT    NOT NULL,
    video_id   INTEGER NOT NULL,
    person_id  INTEGER NOT NULL,
    -- 'cast' | 'crew'
    kind       TEXT    NOT NULL,
    character  TEXT,
    department TEXT,
    job        TEXT,
    cast_id    INTEGER,
    credit_id  TEXT,
    ord        INTEGER
);

-- A person can hold several crew jobs on the same title, so the identity
-- includes job and character.
CREATE UNIQUE INDEX idx_credit_identity ON credit (
    video_type, video_id, person_id, kind, ifnull(job, ''), ifnull(character, '')
);
CREATE INDEX idx_credit_person ON credit (person_id);
CREATE INDEX idx_credit_video ON credit (video_type, video_id);

CREATE TABLE episode_crew (
    tv_show_id     INTEGER NOT NULL,
    season_number  INTEGER NOT NULL,
    episode_number INTEGER NOT NULL,
    person_id      INTEGER NOT NULL,
    department     TEXT,
    job            TEXT
);

-- As with `credit`, one person may hold several jobs on an episode.
CREATE UNIQUE INDEX idx_episode_crew_identity ON episode_crew (
    tv_show_id, season_number, episode_number, person_id, ifnull(job, '')
);
CREATE INDEX idx_episode_crew_episode
    ON episode_crew (tv_show_id, season_number, episode_number);

CREATE TABLE image (
    -- 'movie' | 'tvshow' | 'tvseason' | 'person'
    owner_type    TEXT    NOT NULL,
    owner_id      INTEGER NOT NULL,
    -- Only set for tvseason owners; -1 stands in for "not applicable" so it can
    -- participate in the primary key.
    season_number INTEGER NOT NULL DEFAULT -1,
    -- 'backdrop' | 'poster' | 'logo' | 'profile'
    kind          TEXT    NOT NULL,
    file_path     TEXT    NOT NULL,
    aspect_ratio  REAL,
    height        INTEGER,
    width         INTEGER,
    vote_average  REAL,
    vote_count    INTEGER,
    ord           INTEGER NOT NULL DEFAULT 0,
    PRIMARY KEY (owner_type, owner_id, season_number, kind, file_path)
);

-- Distinguishes "no images fetched yet" from "fetched, and there were none".
CREATE TABLE image_fetch (
    owner_type    TEXT    NOT NULL,
    owner_id      INTEGER NOT NULL,
    season_number INTEGER NOT NULL DEFAULT -1,
    fetched_at    TEXT    NOT NULL,
    PRIMARY KEY (owner_type, owner_id, season_number)
);

CREATE TABLE recommendation (
    source_type    TEXT    NOT NULL,
    source_id      INTEGER NOT NULL,
    rec_id         INTEGER NOT NULL,
    ord            INTEGER NOT NULL DEFAULT 0,
    display_name   TEXT    NOT NULL,
    poster_path    TEXT,
    backdrop_path  TEXT,
    vote_average   INTEGER,
    adult          INTEGER,
    rec_type       TEXT,
    release_date   TEXT,
    first_air_date TEXT,
    age_rating     TEXT,
    PRIMARY KEY (source_type, source_id, rec_id)
);

CREATE TABLE recommendation_meta (
    source_type   TEXT    NOT NULL,
    source_id     INTEGER NOT NULL,
    page          INTEGER,
    total_pages   INTEGER,
    total_results INTEGER,
    fetched_at    TEXT    NOT NULL,
    PRIMARY KEY (source_type, source_id)
);

CREATE TABLE collection (
    id            INTEGER PRIMARY KEY,
    name          TEXT NOT NULL,
    overview      TEXT,
    poster_path   TEXT,
    backdrop_path TEXT,
    fetched_at    TEXT NOT NULL
);

CREATE TABLE collection_part (
    collection_id     INTEGER NOT NULL,
    movie_id          INTEGER NOT NULL,
    ord               INTEGER NOT NULL DEFAULT 0,
    display_name      TEXT    NOT NULL,
    title             TEXT,
    original_title    TEXT,
    original_language TEXT,
    poster_path       TEXT,
    backdrop_path     TEXT,
    release_date      TEXT,
    overview          TEXT,
    vote_average      INTEGER,
    vote_count        INTEGER,
    popularity        REAL,
    adult             INTEGER NOT NULL DEFAULT 0,
    video             INTEGER,
    PRIMARY KEY (collection_id, movie_id)
);

-- Single-row cache of TMDB's /configuration response.
CREATE TABLE tmdb_configuration (
    id         INTEGER PRIMARY KEY CHECK (id = 1),
    payload    TEXT NOT NULL,
    fetched_at TEXT NOT NULL
);
