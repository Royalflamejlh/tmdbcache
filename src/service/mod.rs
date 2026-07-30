//! Application services: the get-or-fetch caching layer between the HTTP
//! handlers and either the store or TMDB.

pub mod collection;
pub mod configuration;
pub mod image;
pub mod mapper;
pub mod movie;
pub mod person;
pub mod search;
pub mod tvshow;
pub mod wallpaper;

use std::collections::BTreeSet;
use std::sync::{Arc, RwLock};

use crate::config::Config;
use crate::store::ActiveStore;
use crate::tmdb::TmdbClient;

/// Shared handler state.
pub struct AppState {
    pub cfg: Config,
    pub store: ActiveStore,
    pub tmdb: TmdbClient,
    /// Wallpaper filenames, refreshed by the directory watcher.
    wallpapers: RwLock<BTreeSet<String>>,
}

impl AppState {
    pub fn new(cfg: Config, store: ActiveStore, tmdb: TmdbClient) -> Arc<Self> {
        let wallpapers = crate::store::scan_wallpapers(&cfg.wallpaper_dir());
        Arc::new(Self {
            cfg,
            store,
            tmdb,
            wallpapers: RwLock::new(wallpapers),
        })
    }

    pub fn wallpapers(&self) -> BTreeSet<String> {
        self.wallpapers
            .read()
            .map(|guard| guard.clone())
            .unwrap_or_default()
    }

    pub fn set_wallpapers(&self, wallpapers: BTreeSet<String>) {
        if let Ok(mut guard) = self.wallpapers.write() {
            *guard = wallpapers;
        }
    }
}

pub type SharedState = Arc<AppState>;
