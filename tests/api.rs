//! End-to-end tests over the real axum router.
//!
//! These exercise every path that does not require TMDB: the library endpoints,
//! configuration, the bundled UI, patch semantics and the image guard rails. The
//! TMDB client is constructed with a dummy key and simply never reached.

use std::collections::BTreeSet;
use std::path::PathBuf;

use axum::Router;
use axum::body::Body;
use axum::http::{Request, StatusCode};
use http_body_util::BodyExt;
use tower::ServiceExt;

use tmdbcache::config::{Config, UnsupportedIntegrations};
use tmdbcache::model::{Genre, VideoType};
use tmdbcache::service::AppState;
use tmdbcache::store::sqlite::SqliteStore;
use tmdbcache::store::{Store, VideoUpsert};
use tmdbcache::tmdb::TmdbClient;
use tmdbcache::{api, web};

fn test_config() -> Config {
    Config {
        port: 0,
        tmdb_api_key: "dummy-key-for-tests".into(),
        tmdb_language: "en-US".into(),
        tmdb_region: "US".into(),
        database_path: PathBuf::from("/nonexistent"),
        image_cache_path: std::env::temp_dir().join("tmdbcache-test-images"),
        subscribed_watch_providers: BTreeSet::new(),
        show_movie_cast: true,
        show_tv_cast: true,
        show_recommendations: true,
        use_movie_backgrounds: true,
        add_media_type_header: true,
        support_detail_cards: false,
        show_tvshows_in_videolist: true,
        show_tvseasons_in_videolist: true,
        max_cards: 200,
        max_light_cards: 300,
        number_of_recommendations: 12,
        number_of_top_recommendations: 12,
        number_of_movie_cast_references: 12,
        number_of_tv_cast_references: 12,
        number_of_directed_movies: 12,
        default_mobile_poster_width: 133,
        default_desktop_poster_width: 220,
        low_rating_threshold: 40,
        high_rating_threshold: 70,
        unsupported: UnsupportedIntegrations::default(),
    }
}

/// Builds the app with an in-memory store, returning it alongside the store so
/// tests can seed data directly.
async fn app() -> (Router, SqliteStore) {
    let store = SqliteStore::connect_in_memory().await.unwrap();
    let cfg = test_config();
    let tmdb = TmdbClient::new(&cfg.tmdb_api_key, &cfg.tmdb_language, &cfg.tmdb_region).unwrap();
    let state = AppState::new(cfg, store.clone(), tmdb);

    let router = api::router().merge(web::router()).with_state(state);

    (router, store)
}

async fn get(router: &Router, uri: &str) -> (StatusCode, String) {
    let response = router
        .clone()
        .oneshot(Request::builder().uri(uri).body(Body::empty()).unwrap())
        .await
        .unwrap();
    let status = response.status();
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    (status, String::from_utf8_lossy(&bytes).to_string())
}

async fn send(router: &Router, method: &str, uri: &str, body: &str) -> (StatusCode, String) {
    let response = router
        .clone()
        .oneshot(
            Request::builder()
                .method(method)
                .uri(uri)
                .header("content-type", "application/json")
                .body(Body::from(body.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    let status = response.status();
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    (status, String::from_utf8_lossy(&bytes).to_string())
}

fn seed_movie(id: i64, name: &str) -> VideoUpsert {
    VideoUpsert {
        video_type: VideoType::Movie,
        id,
        display_name: name.into(),
        overview: Some("Neo learns the truth.".into()),
        poster_path: Some("/poster.jpg".into()),
        release_date: Some("1999-03-31".into()),
        runtime: Some(136),
        vote_average: Some(82),
        adult: false,
        details_loaded: true,
        genres: vec![Genre {
            id: 28,
            genre_id: Some(28),
            name: "Action".into(),
        }],
        ..Default::default()
    }
}

#[tokio::test]
async fn serves_the_bundled_ui() {
    let (router, _) = app().await;
    let (status, body) = get(&router, "/").await;

    assert_eq!(status, StatusCode::OK);
    assert!(body.contains("<title>Movie DB</title>"));
    assert!(
        body.contains("/api/v1/tmdb/configuration"),
        "the UI should bootstrap from the configuration endpoint"
    );
}

#[tokio::test]
async fn health_reports_up() {
    let (router, _) = app().await;
    for uri in ["/health", "/actuator/health"] {
        let (status, body) = get(&router, uri).await;
        assert_eq!(status, StatusCode::OK, "{uri}");
        assert!(body.contains("\"UP\""), "{uri} -> {body}");
    }
}

#[tokio::test]
async fn empty_library_returns_empty_lists() {
    let (router, _) = app().await;

    let (status, body) = get(&router, "/api/v1/movies").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body, r#"{"movies":[]}"#);

    let (status, body) = get(&router, "/api/v1/tvshows").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body, r#"{"tvShows":[]}"#);

    let (status, body) = get(&router, "/api/v1/videos").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body, "[]");

    let (status, body) = get(&router, "/api/v1/movies/favorites").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body, r#"{"movies":[]}"#);
}

#[tokio::test]
async fn cached_movie_is_served_without_touching_tmdb() {
    let (router, store) = app().await;
    store
        .upsert_video(&seed_movie(603, "The Matrix"))
        .await
        .unwrap();

    let (status, body) = get(&router, "/api/v1/movie/603").await;
    assert_eq!(status, StatusCode::OK, "{body}");

    let movie: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(movie["id"], 603);
    assert_eq!(movie["displayName"], "The Matrix");
    assert_eq!(movie["type"], "movie");
    assert_eq!(movie["runtime"], 136);
    assert_eq!(movie["vote_average"], 82);
    assert_eq!(movie["favorite"], false);
    assert_eq!(movie["watched"], false);
    assert_eq!(movie["onWatchlist"], false);
    assert_eq!(movie["genres"][0]["name"], "Action");
    // `non_null` inclusion: absent values are omitted, not null.
    assert!(movie.get("tagline").is_none());
}

#[tokio::test]
async fn wire_format_uses_the_original_field_names() {
    let (router, store) = app().await;
    store
        .upsert_video(&seed_movie(603, "The Matrix"))
        .await
        .unwrap();

    let (_, body) = get(&router, "/api/v1/movie/603").await;

    // The recovered spec mixes snake_case and camelCase; both must survive.
    for field in [
        "displayName",
        "onWatchlist",
        "poster_path",
        "release_date",
        "vote_average",
    ] {
        assert!(
            body.contains(&format!("\"{field}\"")),
            "missing {field} in {body}"
        );
    }
}

#[tokio::test]
async fn patching_a_tag_toggles_it() {
    let (router, store) = app().await;
    store
        .upsert_video(&seed_movie(603, "The Matrix"))
        .await
        .unwrap();

    let (status, body) = send(
        &router,
        "PATCH",
        "/api/v1/movie/603",
        r#"{"tag":"4k","checked":true}"#,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let movie: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(movie["tags"][0], "4k");

    // Filtering by that tag should now find it, and the negated filter should not.
    let (_, body) = get(&router, "/api/v1/movies?tag=4k").await;
    assert!(body.contains("The Matrix"));
    let (_, body) = get(&router, "/api/v1/movies?tag=4k&not=true").await;
    assert_eq!(body, r#"{"movies":[]}"#);

    let (status, body) = send(
        &router,
        "PATCH",
        "/api/v1/movie/603",
        r#"{"tag":"4k","checked":false}"#,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let movie: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert!(movie["tags"].as_array().unwrap().is_empty());
}

#[tokio::test]
async fn reserved_tags_drive_the_flag_columns() {
    let (router, store) = app().await;
    store
        .upsert_video(&seed_movie(603, "The Matrix"))
        .await
        .unwrap();

    for (tag, field) in [
        ("favorite", "favorite"),
        ("watched", "watched"),
        ("onWatchlist", "onWatchlist"),
    ] {
        let (status, body) = send(
            &router,
            "PATCH",
            "/api/v1/movie/603",
            &format!(r#"{{"tag":"{tag}","checked":true}}"#),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{body}");

        let movie: serde_json::Value = serde_json::from_str(&body).unwrap();
        assert_eq!(movie[field], true, "{tag} should set {field}");
        assert!(
            !movie["tags"].as_array().unwrap().iter().any(|t| t == tag),
            "{tag} is a flag, not a freeform tag"
        );
    }

    // And it shows up in the favorites listing.
    let (_, body) = get(&router, "/api/v1/movies/favorites").await;
    assert!(body.contains("The Matrix"));
}

#[tokio::test]
async fn patch_overrides_win_over_upstream_values() {
    let (router, store) = app().await;
    store
        .upsert_video(&seed_movie(603, "The Matrix"))
        .await
        .unwrap();

    let (status, body) = send(
        &router,
        "PATCH",
        "/api/v1/movie/603",
        r#"{"poster_path":"/mine.jpg","overview":"My own summary"}"#,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");

    let movie: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(movie["poster_path"], "/mine.jpg");
    assert_eq!(movie["overview"], "My own summary");
}

#[tokio::test]
async fn unknown_patch_fields_are_rejected() {
    let (router, store) = app().await;
    store
        .upsert_video(&seed_movie(603, "The Matrix"))
        .await
        .unwrap();

    // The spec sets additionalProperties: false on VideoPatch.
    let (status, _) = send(
        &router,
        "PATCH",
        "/api/v1/movie/603",
        r#"{"nonsense":"value"}"#,
    )
    .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
}

#[tokio::test]
async fn deleting_a_movie_empties_the_library() {
    let (router, store) = app().await;
    store
        .upsert_video(&seed_movie(603, "The Matrix"))
        .await
        .unwrap();

    let (status, _) = send(&router, "DELETE", "/api/v1/movie/603", "").await;
    assert_eq!(status, StatusCode::NO_CONTENT);

    let (status, _) = get(&router, "/api/v1/movie/603").await;
    // Gone locally; the handler would now have to reach TMDB, which the dummy
    // key cannot satisfy — so anything but 200 proves the delete landed.
    assert_ne!(status, StatusCode::OK);

    let (_, body) = get(&router, "/api/v1/movies").await;
    assert_eq!(body, r#"{"movies":[]}"#);
}

#[tokio::test]
async fn deleting_an_absent_movie_is_a_404() {
    let (router, _) = app().await;
    let (status, _) = send(&router, "DELETE", "/api/v1/movie/999999", "").await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn videos_list_spans_media_types() {
    let (router, store) = app().await;
    store
        .upsert_video(&seed_movie(603, "The Matrix"))
        .await
        .unwrap();
    store
        .upsert_video(&VideoUpsert {
            video_type: VideoType::Tvshow,
            id: 1399,
            display_name: "Game of Thrones".into(),
            ..Default::default()
        })
        .await
        .unwrap();

    let (status, body) = get(&router, "/api/v1/videos").await;
    assert_eq!(status, StatusCode::OK);

    let videos: Vec<serde_json::Value> = serde_json::from_str(&body).unwrap();
    assert_eq!(videos.len(), 2);
    // Sorted case-insensitively by display name.
    assert_eq!(videos[0]["displayName"], "Game of Thrones");
    assert_eq!(videos[0]["type"], "tvshow");
    assert_eq!(videos[1]["type"], "movie");
}

#[tokio::test]
async fn configuration_reports_the_env_derived_settings() {
    let (router, _) = app().await;
    let (status, body) = get(&router, "/api/v1/tmdb/configuration").await;

    assert_eq!(status, StatusCode::OK, "{body}");
    let config: serde_json::Value = serde_json::from_str(&body).unwrap();

    assert_eq!(config["maxCards"], 200);
    assert_eq!(config["lowRatingThreshold"], 40);
    assert_eq!(config["highRatingThreshold"], 70);
    assert_eq!(config["defaultDesktopPosterWidth"], 220);
    // Auth is not implemented, so the UI must never be told to gate on it.
    assert_eq!(config["requireLogin"], false);
    assert_eq!(config["oauth2Enabled"], false);
}

#[tokio::test]
async fn image_endpoint_rejects_path_traversal() {
    let (router, _) = app().await;

    for path in [
        "..%2F..%2Fetc%2Fpasswd",
        "%2F..%2Fsecret.jpg",
        "nested%2Fdir.jpg",
        "payload.sh",
    ] {
        let (status, _) = get(&router, &format!("/api/v1/images?imagePath={path}")).await;
        assert_eq!(
            status,
            StatusCode::BAD_REQUEST,
            "imagePath={path} should be rejected"
        );
    }
}

#[tokio::test]
async fn image_endpoint_rejects_bad_size_tokens() {
    let (router, _) = app().await;

    for size in ["large", "w", "..%2F..", "w500%2F..%2F.."] {
        let (status, _) = get(
            &router,
            &format!("/api/v1/images?imagePath=ok.jpg&backdropSize={size}"),
        )
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "backdropSize={size}");
    }
}

#[tokio::test]
async fn unknown_wallpaper_is_a_404() {
    let (router, _) = app().await;
    // Only files the directory scan saw can be served, so an arbitrary name and a
    // traversal attempt both fall through to 404.
    let (status, _) = get(&router, "/api/v1/images/wallpaper/nope.jpg").await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn blank_search_is_a_bad_request() {
    let (router, _) = app().await;
    let (status, _) = get(&router, "/api/v1/search/tmdb?query=%20").await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn season_and_episode_patches_reach_the_right_rows() {
    let (router, store) = app().await;

    store
        .upsert_video(&VideoUpsert {
            video_type: VideoType::Tvshow,
            id: 1399,
            display_name: "Game of Thrones".into(),
            ..Default::default()
        })
        .await
        .unwrap();
    store
        .upsert_video(&VideoUpsert {
            video_type: VideoType::Tvseason,
            id: 3624,
            display_name: "Season 1".into(),
            tv_show_id: Some(1399),
            season_number: Some(1),
            details_loaded: true,
            ..Default::default()
        })
        .await
        .unwrap();
    store
        .replace_episodes(
            1399,
            1,
            &[
                tmdbcache::model::TvEpisode {
                    episode_number: 1,
                    name: Some("Winter Is Coming".into()),
                    ..Default::default()
                },
                tmdbcache::model::TvEpisode {
                    episode_number: 2,
                    name: Some("The Kingsroad".into()),
                    ..Default::default()
                },
            ],
        )
        .await
        .unwrap();

    // A season is addressed by show id + season number, not by its own TMDB id.
    let (status, body) = get(&router, "/api/v1/tvseason/1399/1").await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let season: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(season["season_number"], 1);
    assert_eq!(season["tvShowId"], 1399);
    assert_eq!(season["episodes"].as_array().unwrap().len(), 2);

    // Mark one episode watched.
    let (status, body) = send(
        &router,
        "PATCH",
        "/api/v1/tvepisode/1399/1/2",
        r#"{"tag":"watched","checked":true}"#,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let episode: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(episode["episode_number"], 2);
    assert_eq!(episode["watched"], true);

    // The other episode is untouched.
    let (_, body) = get(&router, "/api/v1/tvseason/1399/1").await;
    let season: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(season["episodes"][0]["watched"], false);
    assert_eq!(season["episodes"][1]["watched"], true);

    // Bulk-mark the season.
    let (status, body) = send(
        &router,
        "PATCH",
        "/api/v1/tvseason/1399/1/watched",
        r#"{"watched":true}"#,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let season: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(season["episodes"][0]["watched"], true);
    assert_eq!(season["episodes"][1]["watched"], true);
    assert_eq!(season["watched"], true);
}

#[tokio::test]
async fn patching_an_absent_episode_is_a_404() {
    let (router, _) = app().await;
    let (status, _) = send(
        &router,
        "PATCH",
        "/api/v1/tvepisode/1399/1/1",
        r#"{"tag":"watched","checked":true}"#,
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn unknown_api_paths_fail_as_json_not_html() {
    let (router, _) = app().await;

    let (status, body) = get(&router, "/api/v1/nonexistent").await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert!(
        body.starts_with('{'),
        "an unknown API path must not return the UI document: {body}"
    );

    // Deep links into the hash-routed UI still get the document.
    let (status, body) = get(&router, "/some/deep/link").await;
    assert_eq!(status, StatusCode::OK);
    assert!(body.contains("<title>Movie DB</title>"));
}

#[tokio::test]
async fn top_recommendations_rank_by_overlap_and_exclude_held_titles() {
    let (router, store) = app().await;

    // Two library movies, both recommending 700; one also recommends 800.
    // 900 is recommended once but is already in the library, so it must not show.
    store
        .upsert_video(&seed_movie(603, "The Matrix"))
        .await
        .unwrap();
    store
        .upsert_video(&seed_movie(604, "Reloaded"))
        .await
        .unwrap();
    store
        .upsert_video(&seed_movie(900, "Already Held"))
        .await
        .unwrap();

    let rec = |id: i64, name: &str, vote: i64| tmdbcache::store::StoredRecommendation {
        id,
        display_name: name.into(),
        vote_average: Some(vote),
        rec_type: Some("movie".into()),
        ..Default::default()
    };

    for source in [603, 604] {
        let mut items = vec![
            rec(700, "Twice Recommended", 75),
            rec(900, "Already Held", 90),
        ];
        if source == 603 {
            items.push(rec(800, "Once Recommended", 60));
        }
        store
            .replace_recommendations(
                VideoType::Movie,
                source,
                &tmdbcache::store::RecommendationSet {
                    page: Some(1),
                    items,
                    ..Default::default()
                },
            )
            .await
            .unwrap();
    }

    let (status, body) = get(&router, "/api/v1/movies/topRecommendations").await;
    assert_eq!(status, StatusCode::OK, "{body}");

    let result: serde_json::Value = serde_json::from_str(&body).unwrap();
    let movies = result["movies"].as_array().unwrap();

    let ids: Vec<i64> = movies.iter().map(|m| m["id"].as_i64().unwrap()).collect();
    assert_eq!(
        ids,
        vec![700, 800],
        "most-recommended first, and titles already held are excluded"
    );
    assert_eq!(movies[0]["displayName"], "Twice Recommended");

    // The limit parameter is honoured.
    let (_, body) = get(&router, "/api/v1/movies/topRecommendations?limit=1").await;
    let result: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(result["movies"].as_array().unwrap().len(), 1);
}

#[tokio::test]
async fn cached_recommendations_carry_local_library_state() {
    let (router, store) = app().await;
    store
        .upsert_video(&seed_movie(603, "The Matrix"))
        .await
        .unwrap();
    // 604 is both a recommendation of 603 and held locally, marked watched.
    store
        .upsert_video(&seed_movie(604, "Reloaded"))
        .await
        .unwrap();
    store
        .set_flag(VideoType::Movie, 604, tmdbcache::model::Flag::Watched, true)
        .await
        .unwrap();

    store
        .replace_recommendations(
            VideoType::Movie,
            603,
            &tmdbcache::store::RecommendationSet {
                page: Some(1),
                total_pages: Some(1),
                total_results: Some(1),
                items: vec![tmdbcache::store::StoredRecommendation {
                    id: 604,
                    display_name: "Reloaded".into(),
                    vote_average: Some(70),
                    ..Default::default()
                }],
            },
        )
        .await
        .unwrap();

    let (status, body) = get(&router, "/api/v1/movie/recommendations/603").await;
    assert_eq!(status, StatusCode::OK, "{body}");

    let recommendations: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(recommendations["movieId"], 603);
    let first = &recommendations["movieRecommendations"][0];
    assert_eq!(first["id"], 604);
    assert_eq!(
        first["watched"], true,
        "a recommendation already in the library should report its own state"
    );
}

#[tokio::test]
async fn collection_parts_reflect_library_state() {
    let (router, store) = app().await;
    store
        .upsert_video(&seed_movie(603, "The Matrix"))
        .await
        .unwrap();
    store
        .set_flag(
            VideoType::Movie,
            603,
            tmdbcache::model::Flag::Favorite,
            true,
        )
        .await
        .unwrap();

    store
        .upsert_collection(&tmdbcache::model::Collection {
            id: 2344,
            name: "The Matrix Collection".into(),
            overview: Some("Neo's journey.".into()),
            parts: vec![
                tmdbcache::model::CollectionPart {
                    base: tmdbcache::model::VideoBase {
                        id: 603,
                        display_name: "The Matrix".into(),
                        ..Default::default()
                    },
                    ..Default::default()
                },
                tmdbcache::model::CollectionPart {
                    base: tmdbcache::model::VideoBase {
                        id: 605,
                        display_name: "Revolutions".into(),
                        ..Default::default()
                    },
                    ..Default::default()
                },
            ],
            ..Default::default()
        })
        .await
        .unwrap();

    let (status, body) = get(&router, "/api/v1/collection/2344").await;
    assert_eq!(status, StatusCode::OK, "{body}");

    let collection: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(collection["name"], "The Matrix Collection");
    let parts = collection["parts"].as_array().unwrap();
    assert_eq!(parts.len(), 2);
    assert_eq!(parts[0]["favorite"], true, "held part shows its flag");
    assert_eq!(parts[1]["favorite"], false, "unheld part defaults to false");
}

#[tokio::test]
async fn a_collection_stub_does_not_lose_known_parts() {
    let (_, store) = app().await;

    store
        .upsert_collection(&tmdbcache::model::Collection {
            id: 2344,
            name: "The Matrix Collection".into(),
            parts: vec![tmdbcache::model::CollectionPart {
                base: tmdbcache::model::VideoBase {
                    id: 603,
                    display_name: "The Matrix".into(),
                    ..Default::default()
                },
                ..Default::default()
            }],
            ..Default::default()
        })
        .await
        .unwrap();

    // Caching a movie writes a partless stub from `belongs_to_collection`; it must
    // not wipe the part list a full fetch established.
    store
        .upsert_collection(&tmdbcache::model::Collection {
            id: 2344,
            name: "The Matrix Collection".into(),
            ..Default::default()
        })
        .await
        .unwrap();

    let collection = store.get_collection(2344).await.unwrap().unwrap();
    assert_eq!(
        collection.parts.len(),
        1,
        "parts should survive a stub write"
    );
}

#[tokio::test]
async fn person_page_cross_references_only_held_titles() {
    let (router, store) = app().await;
    store
        .upsert_video(&seed_movie(603, "The Matrix"))
        .await
        .unwrap();

    store
        .replace_credits(
            VideoType::Movie,
            603,
            &[tmdbcache::model::Cast {
                person: tmdbcache::model::PersonBase {
                    id: 6384,
                    name: "Keanu Reeves".into(),
                    ..Default::default()
                },
                character: Some("Neo".into()),
                order: Some(0),
                ..Default::default()
            }],
            &[],
        )
        .await
        .unwrap();
    // Mark the stub as fully fetched so the handler does not reach for TMDB.
    store
        .upsert_person(
            &tmdbcache::store::PersonUpsert {
                id: 6384,
                name: Some("Keanu Reeves".into()),
                biography: Some("Actor.".into()),
                ..Default::default()
            },
            true,
        )
        .await
        .unwrap();

    let (status, body) = get(&router, "/api/v1/person/6384").await;
    assert_eq!(status, StatusCode::OK, "{body}");

    let person: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(person["name"], "Keanu Reeves");
    // cast_id carries the *movie* id so the UI can link into the library.
    assert_eq!(person["movieCast"][0]["cast_id"], 603);
    assert_eq!(person["movieCast"][0]["character"], "Neo");

    // A profile override replaces the upstream path.
    let (status, body) = send(
        &router,
        "PATCH",
        "/api/v1/person/6384",
        r#"{"profile_path":"/custom-face.jpg"}"#,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let person: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(person["profile_path"], "/custom-face.jpg");
}

#[tokio::test]
async fn movie_credits_are_served_from_cache() {
    let (router, store) = app().await;
    store
        .upsert_video(&seed_movie(603, "The Matrix"))
        .await
        .unwrap();
    store
        .replace_credits(
            VideoType::Movie,
            603,
            &[tmdbcache::model::Cast {
                person: tmdbcache::model::PersonBase {
                    id: 6384,
                    name: "Keanu Reeves".into(),
                    ..Default::default()
                },
                character: Some("Neo".into()),
                order: Some(0),
                ..Default::default()
            }],
            &[tmdbcache::model::Crew {
                person: tmdbcache::model::PersonBase {
                    id: 9339,
                    name: "Lana Wachowski".into(),
                    ..Default::default()
                },
                department: Some("Directing".into()),
                job: Some("Director".into()),
            }],
        )
        .await
        .unwrap();

    let (status, body) = get(&router, "/api/v1/movie/credits/603").await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let credits: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(credits["cast"][0]["name"], "Keanu Reeves");
    assert_eq!(credits["cast"][0]["character"], "Neo");
    assert_eq!(credits["crew"][0]["job"], "Director");

    // The movie response surfaces directors separately.
    let (_, body) = get(&router, "/api/v1/movie/603").await;
    let movie: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(movie["directors"][0]["name"], "Lana Wachowski");
}

#[tokio::test]
async fn blank_tags_are_rejected() {
    let (router, store) = app().await;
    store
        .upsert_video(&seed_movie(603, "The Matrix"))
        .await
        .unwrap();

    let (status, _) = send(
        &router,
        "PATCH",
        "/api/v1/movie/603",
        r#"{"tag":"   ","checked":true}"#,
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}
