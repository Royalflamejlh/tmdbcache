use super::AppState;
use crate::error::Result;
use crate::model::{AppConfig, TmdbConfigurationImages};
use crate::store::Store;

/// Builds the client-facing configuration, fetching and caching TMDB's image
/// configuration on first use.
///
/// A TMDB outage must not take the UI down, so a failed fetch degrades to no
/// image configuration rather than an error.
pub async fn get_app_config(state: &AppState) -> Result<AppConfig> {
    let images = match load_images(state).await {
        Ok(images) => images,
        Err(err) => {
            tracing::warn!(error = %err, "could not load TMDB image configuration");
            None
        }
    };
    Ok(AppConfig::build(&state.cfg, images, state.wallpapers()))
}

async fn load_images(state: &AppState) -> Result<Option<TmdbConfigurationImages>> {
    if let Some(payload) = state.store.get_tmdb_configuration().await? {
        if let Ok(images) = serde_json::from_str::<TmdbConfigurationImages>(&payload) {
            return Ok(Some(images));
        }
        tracing::warn!("cached TMDB configuration could not be parsed; refetching");
    }

    let configuration = state.tmdb.configuration().await?;
    if let Some(images) = &configuration.images {
        let payload = serde_json::to_string(images).unwrap_or_default();
        state.store.put_tmdb_configuration(&payload).await?;
    }
    Ok(configuration.images)
}
