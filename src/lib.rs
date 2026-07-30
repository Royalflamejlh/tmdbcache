//! A Rust reimplementation of MovieDB, a self-hosted web app that catalogues a
//! video library, pulling metadata from TMDB and caching it locally.
//!
//! The original project (`justsomebody42/movieDB`, a Spring Boot app) vanished
//! from GitHub; its API contract was recovered from the published container image
//! and is preserved in `docs/openapi-original.yaml`.

pub mod api;
pub mod config;
pub mod error;
pub mod model;
pub mod service;
pub mod store;
pub mod tmdb;
pub mod web;

pub use config::Config;
pub use error::{AppError, Result};
