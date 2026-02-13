//! Immich Selfie Timelapse Server
//!
//! Web server for creating selfie timelapses from Immich.

use immich_timelapse::{
    config::{Config, CONFIG_PATH},
    models::DlibLandmarks,
    web,
};
use std::net::SocketAddr;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Load .env file if present (before anything else)
    if let Err(e) = dotenvy::dotenv() {
        // Not an error if .env doesn't exist
        if !matches!(e, dotenvy::Error::Io(_)) {
            eprintln!("Warning: Failed to load .env file: {}", e);
        }
    }

    // Initialize logging
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "immich_timelapse=debug,tower_http=debug".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    // Load configuration
    let config = load_config()?;
    tracing::info!("Configuration loaded");

    // Check ffmpeg availability
    match immich_timelapse::video::check_ffmpeg().await {
        Ok(version) => tracing::info!("FFmpeg available: {}", version),
        Err(e) => tracing::warn!("FFmpeg not available: {} - video compilation will fail", e),
    }

    // Pre-load ML models to avoid loading during processing
    match DlibLandmarks::init() {
        Ok(_) => tracing::info!("Dlib landmarks model loaded"),
        Err(e) => tracing::warn!(
            "Dlib landmarks model not available: {} - landmark detection will be skipped",
            e
        ),
    }

    // Create application state
    let state = web::AppState::new(config);

    // Create router
    let app = web::create_router(state);

    // Start server
    let addr = SocketAddr::from(([0, 0, 0, 0], 5000));
    tracing::info!("Starting server on http://{}", addr);

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}

fn load_config() -> anyhow::Result<Config> {
    let config_path = std::path::Path::new(CONFIG_PATH);

    // Try loading from config/config.toml, or create a default one
    let config = if config_path.exists() {
        tracing::info!("Loading configuration from {}", CONFIG_PATH);
        Config::from_file(config_path)?.with_env()
    } else {
        tracing::info!("No {} found, creating default config file", CONFIG_PATH);
        let default_config = Config::from_env();
        // Write default config so it can be customized via volume mount
        match default_config.save_to_file(config_path) {
            Ok(()) => tracing::info!("Default config written to {}", CONFIG_PATH),
            Err(e) => {
                let msg = e.to_string();
                if msg.contains("permission denied") || msg.contains("Permission denied") {
                    tracing::warn!(
                        "Could not write default config to {}: permission denied. \
                        If running in Docker, ensure the config directory is writable \
                        (e.g. mount a volume with the correct permissions: \
                        -v /host/config:/app/config). Continuing with in-memory defaults.",
                        CONFIG_PATH
                    );
                } else {
                    tracing::warn!("Could not write default config to {}: {}", CONFIG_PATH, e);
                }
            }
        }
        default_config
    };

    // Warn but don't abort if config is incomplete - the server can still start
    // but jobs will fail until IMMICH_API_KEY and IMMICH_BASE_URL are set
    if let Err(e) = config.validate() {
        tracing::warn!(
            "Configuration incomplete: {} - some features may not work",
            e
        );
    }

    Ok(config)
}
