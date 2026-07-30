use serde::{Deserialize, Serialize};

use super::common::VideoBase;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CollectionPart {
    #[serde(flatten)]
    pub base: VideoBase,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub original_language: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub original_title: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub video: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Collection {
    pub id: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub poster_path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub backdrop_path: Option<String>,
    pub favorite: bool,
    #[serde(rename = "onWatchlist")]
    pub on_watchlist: bool,
    pub watched: bool,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub overview: Option<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub parts: Vec<CollectionPart>,
}
