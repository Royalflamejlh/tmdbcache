use super::{AppState, mapper};
use crate::error::{AppError, Result};
use crate::model::{Person, PersonPatch, PersonProfiles};
use crate::store::{ImageOwner, PersonCreditLimits, Store};

/// People are cached as stubs whenever credits are stored, so a row existing is
/// not enough. `fetched_at` marks a full record.
pub async fn get_person(state: &AppState, id: i64, refresh: bool) -> Result<Person> {
    let cached = state.store.get_person(id).await?;
    let needs_fetch =
        refresh || cached.is_none() || cached.as_ref().is_some_and(|p| p.fetched_at.is_none());

    if needs_fetch {
        let dto = state.tmdb.person(id).await?;
        state
            .store
            .upsert_person(&mapper::person_upsert(&dto), true)
            .await?;
    }

    assemble(state, id).await
}

pub async fn assemble(state: &AppState, id: i64) -> Result<Person> {
    let record = state
        .store
        .get_person(id)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("person {id}")))?;

    let credits = state
        .store
        .person_credits(
            id,
            PersonCreditLimits {
                movie_cast: state.cfg.number_of_movie_cast_references,
                tv_cast: state.cfg.number_of_tv_cast_references,
                directed: state.cfg.number_of_directed_movies,
            },
        )
        .await?;

    Ok(Person {
        id: record.id,
        name: record.name,
        profile_path: record.profile_path,
        place_of_birth: record.place_of_birth,
        biography: record.biography,
        birthday: record.birthday,
        deathday: record.deathday,
        gender: record.gender,
        imdb_id: record.imdb_id,
        adult: record.adult,
        movie_cast: credits.movie_cast,
        directed_movies: credits.directed_movies,
        tv_cast: credits.tv_cast,
    })
}

pub async fn get_profiles(state: &AppState, id: i64, refresh: bool) -> Result<PersonProfiles> {
    let cached = state.store.get_images(ImageOwner::Person, id).await?;
    if refresh || cached.is_none() {
        let images = state.tmdb.person_images(id).await?;
        // TMDB returns person artwork under `profiles`; the store keeps them in
        // the poster slot.
        state
            .store
            .replace_images(
                ImageOwner::Person,
                id,
                &[],
                &mapper::images(&images.profiles),
                &[],
            )
            .await?;
    }

    let images = state
        .store
        .get_images(ImageOwner::Person, id)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("profiles for person {id}")))?;

    Ok(PersonProfiles {
        id,
        person_id: id,
        profiles: images.posters,
    })
}

pub async fn patch_person(state: &AppState, id: i64, patch: &PersonPatch) -> Result<Person> {
    if state.store.get_person(id).await?.is_none() {
        return Err(AppError::NotFound(format!("person {id}")));
    }
    if patch.profile_path.is_some() {
        state
            .store
            .set_person_profile_override(id, patch.profile_path.as_deref())
            .await?;
    }
    assemble(state, id).await
}
