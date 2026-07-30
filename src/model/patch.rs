//! Patch bodies. Every field is optional; absent fields are left untouched.
//!
//! `tag` + `checked` together toggle a single tag: `checked: true` adds it,
//! `false` removes it. The upstream `additionalProperties: false` is enforced by
//! `#[serde(deny_unknown_fields)]`.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub struct VideoPatch {
    pub tag: Option<String>,
    pub checked: Option<bool>,
    pub poster_path: Option<String>,
    pub backdrop_path: Option<String>,
    pub overview: Option<String>,
    #[serde(rename = "imdbId")]
    pub imdb_id: Option<String>,
    #[serde(rename = "werStreamtEsId")]
    pub wer_streamt_es_id: Option<i32>,
}

impl VideoPatch {
    /// True when the patch carries a tag toggle that needs `checked` to resolve.
    pub fn is_tag_toggle(&self) -> bool {
        self.tag.is_some()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub struct PersonPatch {
    pub profile_path: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct TvEpisodePatch {
    pub tag: Option<String>,
    pub checked: Option<bool>,
}
