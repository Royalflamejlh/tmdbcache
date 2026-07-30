use serde::{Deserialize, Serialize};

/// The original's `AbstractPerson`, shared by cast and crew entries.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct PersonBase {
    pub id: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub adult: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub gender: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub known_for_department: Option<String>,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub original_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub popularity: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub profile_path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub credit_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Cast {
    #[serde(flatten)]
    pub person: PersonBase,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cast_id: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub character: Option<String>,
    #[serde(rename = "order", skip_serializing_if = "Option::is_none")]
    pub order: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Crew {
    #[serde(flatten)]
    pub person: PersonBase,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub department: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub job: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Credits {
    pub id: i64,
    pub cast: Vec<Cast>,
    pub crew: Vec<Crew>,
}

/// TV cast uses `castId` (camelCase) where movie cast uses `cast_id`. Preserved
/// verbatim from the recovered spec rather than normalised.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct TvShowCast {
    #[serde(flatten)]
    pub person: PersonBase,
    #[serde(rename = "castId", skip_serializing_if = "Option::is_none")]
    pub cast_id: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub character: Option<String>,
    #[serde(rename = "order", skip_serializing_if = "Option::is_none")]
    pub order: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct TvShowCredits {
    pub id: i64,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub cast: Vec<TvShowCast>,
}

/// Back-reference from a person to a title in the local library.
///
/// The upstream schema names the fields `cast_id`/`character` without saying
/// which id it carries. We emit the *video* id so the UI can link straight to
/// the title, with `character` holding the role (or the job, for crew credits).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CastReference {
    pub cast_id: i64,
    pub character: String,
}
