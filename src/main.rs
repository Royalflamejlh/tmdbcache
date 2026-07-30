use std::net::SocketAddr;
use std::time::Duration;

use tower_http::compression::CompressionLayer;
use tower_http::trace::TraceLayer;
use tracing_subscriber::EnvFilter;

use tmdbcache::service::{AppState, wallpaper};
use tmdbcache::store::SqliteStore;
use tmdbcache::tmdb::TmdbClient;
use tmdbcache::{Config, Result, api, web};

#[tokio::main]
async fn main() -> Result<()> {
    // `--healthcheck` probes a running instance and exits, so the container image
    // does not need curl or wget for its HEALTHCHECK.
    if std::env::args().any(|arg| arg == "--healthcheck") {
        return healthcheck().await;
    }

    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| EnvFilter::new("info,tmdbcache=debug")),
        )
        .init();

    let cfg = Config::from_env()?;

    // The original supported these; this port does not. Fail loudly in the log
    // rather than quietly ignoring a deliberately configured integration.
    let unsupported = cfg.unsupported.configured();
    if !unsupported.is_empty() {
        tracing::warn!(
            integrations = ?unsupported,
            "configured but not implemented in this port; these settings are ignored"
        );
    }

    let store = SqliteStore::connect(&cfg.database_file()).await?;
    tracing::info!(path = ?cfg.database_file(), "database ready");

    let tmdb = TmdbClient::new(&cfg.tmdb_api_key, &cfg.tmdb_language, &cfg.tmdb_region)?;

    let port = cfg.port;
    let state = AppState::new(cfg, store, tmdb);

    // Held for the process lifetime; dropping it stops the watch.
    let _watcher = wallpaper::spawn_watcher(state.clone())?;

    let app = api::router()
        .merge(web::router())
        .layer(CompressionLayer::new())
        .layer(TraceLayer::new_for_http())
        .with_state(state);

    let addr = SocketAddr::from(([0, 0, 0, 0], port));
    let listener = tokio::net::TcpListener::bind(addr).await?;
    tracing::info!(%addr, "listening");

    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await?;

    Ok(())
}

/// Probes `/health` on the configured port, exiting non-zero if it is not up.
async fn healthcheck() -> Result<()> {
    // Read the port directly; a full Config would demand an API key the probe
    // has no use for.
    let port: u16 = std::env::var("MOVIEDB_PORT")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(8081);

    let url = format!("http://127.0.0.1:{port}/health");
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(3))
        .build()
        .map_err(tmdbcache::AppError::from)?;

    match client.get(&url).send().await {
        Ok(response) if response.status().is_success() => {
            println!("healthy");
            Ok(())
        }
        Ok(response) => {
            eprintln!("unhealthy: {} returned {}", url, response.status());
            std::process::exit(1);
        }
        Err(err) => {
            eprintln!("unhealthy: {url} unreachable: {err}");
            std::process::exit(1);
        }
    }
}

/// Resolves on SIGINT or SIGTERM so containers stop promptly.
async fn shutdown_signal() {
    let ctrl_c = async {
        let _ = tokio::signal::ctrl_c().await;
    };

    #[cfg(unix)]
    let terminate = async {
        match tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate()) {
            Ok(mut stream) => {
                stream.recv().await;
            }
            // Without a SIGTERM handler, fall back to Ctrl-C only.
            Err(err) => {
                tracing::warn!(error = %err, "could not install SIGTERM handler");
                std::future::pending::<()>().await;
            }
        }
    };
    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {}
        _ = terminate => {}
    }

    tracing::info!("shutdown signal received");
    // Give in-flight image writes a moment to land.
    tokio::time::sleep(Duration::from_millis(100)).await;
}
