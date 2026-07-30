use serde::{Deserialize, Serialize};

use super::common::Image;
use super::credits::CastReference;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Person {
    pub id: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub profile_path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub place_of_birth: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub biography: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub birthday: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub deathday: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub gender: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub imdb_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub adult: Option<bool>,

    /// Movies in the local library this person appears in.
    #[serde(rename = "movieCast", skip_serializing_if = "Vec::is_empty")]
    pub movie_cast: Vec<CastReference>,
    /// Movies in the local library this person directed.
    #[serde(rename = "directedMovies", skip_serializing_if = "Vec::is_empty")]
    pub directed_movies: Vec<CastReference>,
    /// TV shows in the local library this person appears in.
    #[serde(rename = "tvCast", skip_serializing_if = "Vec::is_empty")]
    pub tv_cast: Vec<CastReference>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct PersonProfiles {
    pub id: i64,
    #[serde(rename = "personId")]
    pub person_id: i64,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub profiles: Vec<Image>,
}
