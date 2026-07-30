//! Thin HTTP client for the TMDB v3 API.
//!
//! The endpoints and `append_to_response` combinations here were recovered from
//! the original container's compiled classes, so the number of upstream calls per
//! cached title matches the Java implementation.

use std::time::Duration;

use axum::http::StatusCode;
use reqwest::Client;
use serde::de::DeserializeOwned;

use super::dto::*;
use crate::error::{AppError, Result};

const API_BASE: &str = "https://api.themoviedb.org/3";
const IMAGE_BASE: &str = "https://image.tmdb.org/t/p";

/// How the API key is presented to TMDB.
#[derive(Debug, Clone)]
enum Auth {
    /// v3 API key, sent as an `api_key` query parameter.
    QueryKey(String),
    /// v4 read access token (a JWT), sent as a bearer token.
    Bearer(String),
}

#[derive(Clone)]
pub struct TmdbClient {
    http: Client,
    auth: Auth,
    language: String,
    region: String,
}

impl TmdbClient {
    pub fn new(api_key: &str, language: &str, region: &str) -> Result<Self> {
        let http = Client::builder()
            .timeout(Duration::from_secs(20))
            .user_agent(concat!("tmdbcache/", env!("CARGO_PKG_VERSION")))
            .build()?;

        // A v4 read access token is a JWT; a v3 key is a 32-char hex string.
        let auth = if api_key.starts_with("ey") && api_key.matches('.').count() == 2 {
            Auth::Bearer(api_key.to_string())
        } else {
            Auth::QueryKey(api_key.to_string())
        };

        Ok(Self {
            http,
            auth,
            language: language.to_string(),
            region: region.to_string(),
        })
    }

    pub fn region(&self) -> &str {
        &self.region
    }

    /// Issues a GET against `path` with auth, language and `extra` query pairs.
    async fn get<T: DeserializeOwned>(&self, path: &str, extra: &[(&str, &str)]) -> Result<T> {
        let url = format!("{API_BASE}/{path}");
        let mut request = self.http.get(&url).query(&[("language", &self.language)]);

        match &self.auth {
            Auth::QueryKey(key) => request = request.query(&[("api_key", key)]),
            Auth::Bearer(token) => request = request.bearer_auth(token),
        }
        if !extra.is_empty() {
            request = request.query(extra);
        }

        let response = request.send().await?;
        let status = response.status();
        if !status.is_success() {
            let body = response.text().await.unwrap_or_default();
            return Err(AppError::TmdbStatus {
                status: StatusCode::from_u16(status.as_u16()).unwrap_or(StatusCode::BAD_GATEWAY),
                body: body.chars().take(500).collect(),
            });
        }
        Ok(response.json().await?)
    }

    pub async fn configuration(&self) -> Result<TmdbConfiguration> {
        self.get("configuration", &[]).await
    }

    /// One call fetches the movie plus its credits, recommendations, trailers and
    /// external ids.
    pub async fn movie(&self, id: i64) -> Result<TmdbMovie> {
        self.get(
            &format!("movie/{id}"),
            &[(
                "append_to_response",
                "videos,external_ids,credits,recommendations",
            )],
        )
        .await
    }

    pub async fn movie_credits(&self, id: i64) -> Result<TmdbCredits> {
        self.get(&format!("movie/{id}/credits"), &[]).await
    }

    pub async fn movie_recommendations(
        &self,
        id: i64,
        page: i64,
    ) -> Result<TmdbPaged<TmdbMovieBrief>> {
        self.get(
            &format!("movie/{id}/recommendations"),
            &[("page", &page.to_string())],
        )
        .await
    }

    pub async fn movie_images(&self, id: i64) -> Result<TmdbImages> {
        // `include_image_language` keeps language-neutral artwork in the results.
        self.get(
            &format!("movie/{id}/images"),
            &[("include_image_language", "en,null")],
        )
        .await
    }

    pub async fn movie_videos(&self, id: i64) -> Result<TmdbVideos> {
        self.get(&format!("movie/{id}/videos"), &[]).await
    }

    pub async fn movie_release_dates(&self, id: i64) -> Result<TmdbReleaseDates> {
        self.get(&format!("movie/{id}/release_dates"), &[]).await
    }

    pub async fn movie_watch_providers(&self, id: i64) -> Result<TmdbWatchProviders> {
        self.get(&format!("movie/{id}/watch/providers"), &[]).await
    }

    pub async fn tv_show(&self, id: i64) -> Result<TmdbTvShow> {
        self.get(
            &format!("tv/{id}"),
            &[(
                "append_to_response",
                "videos,external_ids,credits,recommendations",
            )],
        )
        .await
    }

    pub async fn tv_season(&self, tv_show_id: i64, season_number: i64) -> Result<TmdbSeason> {
        self.get(&format!("tv/{tv_show_id}/season/{season_number}"), &[])
            .await
    }

    pub async fn tv_images(&self, id: i64) -> Result<TmdbImages> {
        self.get(
            &format!("tv/{id}/images"),
            &[("include_image_language", "en,null")],
        )
        .await
    }

    pub async fn tv_season_images(
        &self,
        tv_show_id: i64,
        season_number: i64,
    ) -> Result<TmdbImages> {
        self.get(
            &format!("tv/{tv_show_id}/season/{season_number}/images"),
            &[("include_image_language", "en,null")],
        )
        .await
    }

    pub async fn tv_content_ratings(&self, id: i64) -> Result<TmdbContentRatings> {
        self.get(&format!("tv/{id}/content_ratings"), &[]).await
    }

    pub async fn tv_watch_providers(&self, id: i64) -> Result<TmdbWatchProviders> {
        self.get(&format!("tv/{id}/watch/providers"), &[]).await
    }

    pub async fn tv_recommendations(
        &self,
        id: i64,
        page: i64,
    ) -> Result<TmdbPaged<TmdbMovieBrief>> {
        self.get(
            &format!("tv/{id}/recommendations"),
            &[("page", &page.to_string())],
        )
        .await
    }

    pub async fn person(&self, id: i64) -> Result<TmdbPerson> {
        self.get(
            &format!("person/{id}"),
            &[("append_to_response", "movie_credits,tv_credits")],
        )
        .await
    }

    pub async fn person_images(&self, id: i64) -> Result<TmdbImages> {
        self.get(&format!("person/{id}/images"), &[]).await
    }

    pub async fn collection(&self, id: i64) -> Result<TmdbCollection> {
        self.get(&format!("collection/{id}"), &[]).await
    }

    pub async fn search_movies(&self, query: &str, page: i64) -> Result<TmdbPaged<TmdbMovieBrief>> {
        self.get(
            "search/movie",
            &[("query", query), ("page", &page.to_string())],
        )
        .await
    }

    pub async fn search_tv(&self, query: &str, page: i64) -> Result<TmdbPaged<TmdbMovieBrief>> {
        self.get(
            "search/tv",
            &[("query", query), ("page", &page.to_string())],
        )
        .await
    }

    /// Downloads an image, returning its bytes and content type.
    ///
    /// `size` is a TMDB size token such as `w500` or `original`; `path` is the
    /// leading-slash path TMDB reports (e.g. `/abc123.jpg`).
    pub async fn download_image(&self, size: &str, path: &str) -> Result<(Vec<u8>, String)> {
        let path = path.trim_start_matches('/');
        let url = format!("{IMAGE_BASE}/{size}/{path}");

        let response = self.http.get(&url).send().await?;
        let status = response.status();
        if !status.is_success() {
            return Err(AppError::TmdbStatus {
                status: StatusCode::from_u16(status.as_u16()).unwrap_or(StatusCode::BAD_GATEWAY),
                body: format!("image fetch failed for {url}"),
            });
        }

        let content_type = response
            .headers()
            .get(reqwest::header::CONTENT_TYPE)
            .and_then(|v| v.to_str().ok())
            .unwrap_or("image/jpeg")
            .to_string();

        Ok((response.bytes().await?.to_vec(), content_type))
    }
}
