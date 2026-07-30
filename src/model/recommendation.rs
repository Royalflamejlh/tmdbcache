use serde::{Deserialize, Serialize};

/// The original's `AbstractRecommendation`: a lightweight poster card.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct RecommendationBase {
    pub id: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub poster_path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub backdrop_path: Option<String>,
    pub favorite: bool,
    #[serde(rename = "onWatchlist")]
    pub on_watchlist: bool,
    pub watched: bool,
    #[serde(rename = "displayName")]
    pub display_name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub vote_average: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub adult: Option<bool>,
    #[serde(rename = "type", skip_serializing_if = "Option::is_none")]
    pub rec_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub emby_id: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub emby_server_id: Option<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub tags: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Recommendation {
    #[serde(flatten)]
    pub base: RecommendationBase,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub release_date: Option<String>,
    #[serde(rename = "ageRating", skip_serializing_if = "Option::is_none")]
    pub age_rating: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Recommendations {
    #[serde(rename = "movieId")]
    pub movie_id: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub page: Option<i64>,
    #[serde(rename = "movieRecommendations")]
    pub movie_recommendations: Vec<Recommendation>,
    #[serde(rename = "totalPages", skip_serializing_if = "Option::is_none")]
    pub total_pages: Option<i64>,
    #[serde(rename = "totalResults", skip_serializing_if = "Option::is_none")]
    pub total_results: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct TvShowRecommendation {
    #[serde(flatten)]
    pub base: RecommendationBase,
    #[serde(rename = "firstAirDate", skip_serializing_if = "Option::is_none")]
    pub first_air_date: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct TvShowRecommendations {
    /// Named `movieId` upstream even for TV shows; kept as-is.
    #[serde(rename = "movieId")]
    pub movie_id: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub page: Option<i64>,
    #[serde(rename = "tvShowRecommendations")]
    pub tv_show_recommendations: Vec<TvShowRecommendation>,
    #[serde(rename = "totalPages", skip_serializing_if = "Option::is_none")]
    pub total_pages: Option<i64>,
    #[serde(rename = "totalResults", skip_serializing_if = "Option::is_none")]
    pub total_results: Option<i64>,
}
