//! Store-level tests, run against an in-memory SQLite database.

use tmdbcache::model::{
    Cast, Crew, Flag, Genre, Image, PersonBase, ProviderKind, TvEpisode, VideoType, WatchProvider,
};
use tmdbcache::store::sqlite::SqliteStore;
use tmdbcache::store::{ImageOwner, PersonCreditLimits, Store, VideoOverrides, VideoUpsert};

async fn store() -> SqliteStore {
    SqliteStore::connect_in_memory()
        .await
        .expect("migrations should apply cleanly")
}

fn movie(id: i64, name: &str) -> VideoUpsert {
    VideoUpsert {
        video_type: VideoType::Movie,
        id,
        display_name: name.to_string(),
        overview: Some("An overview".into()),
        poster_path: Some("/poster.jpg".into()),
        release_date: Some("1999-03-31".into()),
        runtime: Some(136),
        vote_average: Some(82),
        vote_count: Some(24000),
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
async fn migrations_apply_and_roundtrip_a_movie() {
    let store = store().await;
    store.upsert_video(&movie(603, "The Matrix")).await.unwrap();

    let found = store
        .get_video(VideoType::Movie, 603)
        .await
        .unwrap()
        .expect("movie should be cached");

    assert_eq!(found.id, 603);
    assert_eq!(found.display_name, "The Matrix");
    assert_eq!(found.vote_average, Some(82));
    assert_eq!(found.runtime, Some(136));
    assert_eq!(found.genres.len(), 1);
    assert_eq!(found.genres[0].name, "Action");
    assert!(!found.favorite);
    assert!(found.tags.is_empty());
}

#[tokio::test]
async fn upserting_metadata_preserves_user_state() {
    let store = store().await;
    store.upsert_video(&movie(603, "The Matrix")).await.unwrap();

    store
        .set_flag(VideoType::Movie, 603, Flag::Favorite, true)
        .await
        .unwrap();
    store
        .set_tag(VideoType::Movie, 603, "rewatch", true)
        .await
        .unwrap();
    store
        .apply_overrides(
            VideoType::Movie,
            603,
            &VideoOverrides {
                poster_path: Some("/custom.jpg".into()),
                ..Default::default()
            },
        )
        .await
        .unwrap();

    // A refresh from TMDB must not clobber flags, tags or overrides.
    let mut refreshed = movie(603, "The Matrix (1999)");
    refreshed.poster_path = Some("/upstream.jpg".into());
    store.upsert_video(&refreshed).await.unwrap();

    let found = store
        .get_video(VideoType::Movie, 603)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(found.display_name, "The Matrix (1999)");
    assert!(found.favorite, "favorite flag should survive a refresh");
    assert_eq!(found.tags, vec!["rewatch".to_string()]);
    assert_eq!(
        found.poster_path.as_deref(),
        Some("/custom.jpg"),
        "user override should win over the upstream poster"
    );
}

#[tokio::test]
async fn tag_filtering_supports_negation() {
    let store = store().await;
    store.upsert_video(&movie(603, "The Matrix")).await.unwrap();
    store
        .upsert_video(&movie(604, "The Matrix Reloaded"))
        .await
        .unwrap();
    store
        .set_tag(VideoType::Movie, 603, "4k", true)
        .await
        .unwrap();

    let tagged = store
        .list_videos_by_tag(VideoType::Movie, "4k", false)
        .await
        .unwrap();
    assert_eq!(tagged.len(), 1);
    assert_eq!(tagged[0].id, 603);

    let untagged = store
        .list_videos_by_tag(VideoType::Movie, "4k", true)
        .await
        .unwrap();
    assert_eq!(untagged.len(), 1);
    assert_eq!(untagged[0].id, 604);
}

#[tokio::test]
async fn watch_providers_are_grouped_by_kind() {
    let store = store().await;
    let mut m = movie(603, "The Matrix");
    m.watch_providers = vec![
        (
            ProviderKind::Flatrate,
            WatchProvider {
                logo_path: Some("/nf.jpg".into()),
                provider_id: 8,
                provider_name: "Netflix".into(),
                display_priority: Some(1),
            },
        ),
        (
            ProviderKind::Buy,
            WatchProvider {
                logo_path: None,
                provider_id: 10,
                provider_name: "Amazon Video".into(),
                display_priority: Some(2),
            },
        ),
    ];
    store.upsert_video(&m).await.unwrap();

    let found = store
        .get_video(VideoType::Movie, 603)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(found.flatrate_watch_providers.len(), 1);
    assert_eq!(found.flatrate_watch_providers[0].provider_name, "Netflix");
    assert_eq!(found.buy_watch_providers.len(), 1);
    assert!(found.rent_watch_providers.is_empty());
}

#[tokio::test]
async fn credits_resolve_directors_and_person_backreferences() {
    let store = store().await;
    store.upsert_video(&movie(603, "The Matrix")).await.unwrap();

    let keanu = PersonBase {
        id: 6384,
        name: "Keanu Reeves".into(),
        profile_path: Some("/keanu.jpg".into()),
        ..Default::default()
    };
    let lana = PersonBase {
        id: 9339,
        name: "Lana Wachowski".into(),
        ..Default::default()
    };

    store
        .replace_credits(
            VideoType::Movie,
            603,
            &[Cast {
                person: keanu,
                cast_id: Some(34),
                character: Some("Neo".into()),
                order: Some(0),
            }],
            &[Crew {
                person: lana,
                department: Some("Directing".into()),
                job: Some("Director".into()),
            }],
        )
        .await
        .unwrap();

    let credits = store
        .get_credits(VideoType::Movie, 603)
        .await
        .unwrap()
        .expect("credits should be cached");
    assert_eq!(credits.cast.len(), 1);
    assert_eq!(credits.cast[0].character.as_deref(), Some("Neo"));
    assert_eq!(credits.crew.len(), 1);

    let directors = store.get_directors(603).await.unwrap();
    assert_eq!(directors.len(), 1);
    assert_eq!(directors[0].person.name, "Lana Wachowski");

    let limits = PersonCreditLimits {
        movie_cast: 12,
        tv_cast: 12,
        directed: 12,
    };
    let refs = store.person_credits(6384, limits).await.unwrap();
    assert_eq!(refs.movie_cast.len(), 1);
    assert_eq!(refs.movie_cast[0].cast_id, 603);
    assert_eq!(refs.movie_cast[0].character, "Neo");

    let directed = store.person_credits(9339, limits).await.unwrap();
    assert_eq!(directed.directed_movies.len(), 1);
    assert_eq!(directed.directed_movies[0].cast_id, 603);
}

#[tokio::test]
async fn images_distinguish_unfetched_from_empty() {
    let store = store().await;

    assert!(
        store
            .get_images(ImageOwner::Movie, 603)
            .await
            .unwrap()
            .is_none(),
        "never-fetched image sets must read as None so the service knows to fetch"
    );

    store
        .replace_images(ImageOwner::Movie, 603, &[], &[], &[])
        .await
        .unwrap();
    let fetched = store.get_images(ImageOwner::Movie, 603).await.unwrap();
    assert!(
        fetched.is_some(),
        "an empty-but-fetched set must not trigger a refetch"
    );
    assert!(fetched.unwrap().posters.is_empty());

    store
        .replace_images(
            ImageOwner::Movie,
            603,
            &[Image {
                file_path: Some("/back.jpg".into()),
                width: Some(1920),
                ..Default::default()
            }],
            &[Image {
                file_path: Some("/post.jpg".into()),
                ..Default::default()
            }],
            &[],
        )
        .await
        .unwrap();
    let images = store
        .get_images(ImageOwner::Movie, 603)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(images.backdrops.len(), 1);
    assert_eq!(images.posters.len(), 1);
    assert_eq!(images.backdrops[0].width, Some(1920));
}

#[tokio::test]
async fn episodes_keep_local_state_across_refresh() {
    let store = store().await;

    let episodes = vec![TvEpisode {
        id: Some(63056),
        episode_number: 1,
        name: Some("Winter Is Coming".into()),
        season_number: Some(1),
        vote_average: Some(7.6),
        ..Default::default()
    }];
    store.replace_episodes(1399, 1, &episodes).await.unwrap();
    store
        .set_episode_flag(1399, 1, 1, Flag::Watched, true)
        .await
        .unwrap();
    store
        .set_episode_tag(1399, 1, 1, "favourite-scene", true)
        .await
        .unwrap();

    let mut refreshed = episodes.clone();
    refreshed[0].name = Some("Winter Is Coming (remastered)".into());
    store.replace_episodes(1399, 1, &refreshed).await.unwrap();

    let stored = store.get_episode(1399, 1, 1).await.unwrap().unwrap();
    assert_eq!(
        stored.name.as_deref(),
        Some("Winter Is Coming (remastered)")
    );
    assert!(stored.watched, "watched flag should survive a refresh");
    assert_eq!(stored.tags, vec!["favourite-scene".to_string()]);
}

#[tokio::test]
async fn deleting_a_tv_show_removes_its_seasons_and_episodes() {
    let store = store().await;

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
            ..Default::default()
        })
        .await
        .unwrap();
    store
        .replace_episodes(
            1399,
            1,
            &[TvEpisode {
                episode_number: 1,
                ..Default::default()
            }],
        )
        .await
        .unwrap();

    assert_eq!(store.list_seasons(1399).await.unwrap().len(), 1);
    assert!(store.delete_video(VideoType::Tvshow, 1399).await.unwrap());

    assert!(
        store
            .get_video(VideoType::Tvshow, 1399)
            .await
            .unwrap()
            .is_none()
    );
    assert!(store.list_seasons(1399).await.unwrap().is_empty());
    assert!(store.list_episodes(1399, 1).await.unwrap().is_empty());
}

#[tokio::test]
async fn deleting_a_missing_video_reports_false() {
    let store = store().await;
    assert!(!store.delete_video(VideoType::Movie, 12345).await.unwrap());
}

/// A library larger than the bind-chunk size must not hit SQLite's
/// bound-parameter limit, and every row must still be hydrated exactly once.
#[tokio::test]
async fn large_libraries_are_hydrated_across_bind_chunks() {
    let store = store().await;

    // 1200 rows spans several chunks at the 500 used internally.
    const COUNT: i64 = 1200;
    for id in 1..=COUNT {
        store
            .upsert_video(&movie(id, &format!("Movie {id:04}")))
            .await
            .unwrap();
    }
    // Tag a row in the first, a middle, and the last chunk.
    for id in [1, 700, COUNT] {
        store
            .set_tag(VideoType::Movie, id, "spot-check", true)
            .await
            .unwrap();
    }

    let all = store.list_videos(Some(VideoType::Movie)).await.unwrap();
    assert_eq!(all.len() as i64, COUNT);
    // Genres come from a chunked join, so every row should still have its own.
    assert!(
        all.iter().all(|v| v.genres.len() == 1),
        "every row should be hydrated with its genre"
    );

    let tagged: Vec<i64> = all
        .iter()
        .filter(|v| v.tags.iter().any(|t| t == "spot-check"))
        .map(|v| v.id)
        .collect();
    assert_eq!(tagged, vec![1, 700, COUNT], "tags land on the right rows");

    // local_states is chunked too.
    let ids: Vec<i64> = (1..=COUNT).collect();
    let states = store.local_states(VideoType::Movie, &ids).await.unwrap();
    assert_eq!(states.len() as i64, COUNT);
    assert_eq!(states[&700].tags, vec!["spot-check".to_string()]);
    assert!(states[&699].tags.is_empty());
}
