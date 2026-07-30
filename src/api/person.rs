use axum::Json;
use axum::extract::{Path, Query, State};

use super::RefreshQuery;
use crate::error::Result;
use crate::model::{Person, PersonPatch, PersonProfiles};
use crate::service::{SharedState, person};

pub async fn get_person(
    State(state): State<SharedState>,
    Path(person_id): Path<i64>,
    Query(query): Query<RefreshQuery>,
) -> Result<Json<Person>> {
    Ok(Json(
        person::get_person(&state, person_id, query.refresh()).await?,
    ))
}

pub async fn patch_person(
    State(state): State<SharedState>,
    Path(person_id): Path<i64>,
    Json(patch): Json<PersonPatch>,
) -> Result<Json<Person>> {
    Ok(Json(person::patch_person(&state, person_id, &patch).await?))
}

pub async fn get_profiles(
    State(state): State<SharedState>,
    Path(person_id): Path<i64>,
    Query(query): Query<RefreshQuery>,
) -> Result<Json<PersonProfiles>> {
    Ok(Json(
        person::get_profiles(&state, person_id, query.refresh()).await?,
    ))
}
