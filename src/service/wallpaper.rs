//! User-supplied wallpapers.
//!
//! Files dropped into `<imageCache>/wallpapers` become available without a
//! restart: a filesystem watcher rescans the directory shortly after any change,
//! matching the original's "available after ~8 seconds" behaviour.

use std::sync::Arc;
use std::time::Duration;

use notify::{RecursiveMode, Watcher};

use super::{AppState, image::CachedImage};
use crate::error::{AppError, Result};
use crate::store::scan_wallpapers;

/// How long to wait for filesystem activity to settle before rescanning.
const DEBOUNCE: Duration = Duration::from_secs(2);

/// Serves a wallpaper by filename.
///
/// The name is checked against the scanned directory listing rather than being
/// sanitised, so only files the watcher actually saw can be served — which rules
/// out traversal without needing to reason about path syntax.
pub async fn get_wallpaper(state: &AppState, name: &str) -> Result<CachedImage> {
    if !state.wallpapers().contains(name) {
        return Err(AppError::NotFound(format!("wallpaper {name}")));
    }

    let path = state.cfg.wallpaper_dir().join(name);
    let bytes = tokio::fs::read(&path).await?;
    let content_type = mime_guess::from_path(&path)
        .first_raw()
        .unwrap_or("image/jpeg")
        .to_string();

    Ok(CachedImage {
        bytes,
        content_type,
    })
}

/// Starts watching the wallpaper directory, keeping `state`'s listing current.
///
/// Returns the watcher, which must be held alive for the watch to continue.
pub fn spawn_watcher(state: Arc<AppState>) -> Result<Option<notify::RecommendedWatcher>> {
    let dir = state.cfg.wallpaper_dir();
    if let Err(err) = std::fs::create_dir_all(&dir) {
        tracing::warn!(?dir, error = %err, "could not create wallpaper directory");
        return Ok(None);
    }

    let (tx, rx) = std::sync::mpsc::channel();
    let mut watcher = match notify::recommended_watcher(move |event| {
        // A send failure just means the receiver thread has gone away.
        let _ = tx.send(event);
    }) {
        Ok(watcher) => watcher,
        Err(err) => {
            tracing::warn!(error = %err, "wallpaper watcher unavailable");
            return Ok(None);
        }
    };

    if let Err(err) = watcher.watch(&dir, RecursiveMode::NonRecursive) {
        tracing::warn!(?dir, error = %err, "could not watch wallpaper directory");
        return Ok(None);
    }

    // The notify callback is synchronous, so debouncing lives on its own thread.
    std::thread::Builder::new()
        .name("wallpaper-watcher".into())
        .spawn(move || {
            let dir = state.cfg.wallpaper_dir();
            while let Ok(_event) = rx.recv() {
                // Drain the burst that a file copy produces before rescanning.
                while rx.recv_timeout(DEBOUNCE).is_ok() {}

                let wallpapers = scan_wallpapers(&dir);
                tracing::info!(count = wallpapers.len(), "wallpaper listing refreshed");
                state.set_wallpapers(wallpapers);
            }
        })
        .map_err(|err| AppError::Internal(err.into()))?;

    tracing::info!(?dir, "watching wallpaper directory");
    Ok(Some(watcher))
}
