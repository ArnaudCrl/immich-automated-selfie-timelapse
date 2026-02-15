//! Immich Selfie Timelapse Server
//!
//! Web server for creating selfie timelapses from Immich.

use immich_timelapse::{
    config::{Config, CONFIG_PATH},
    error::PERMISSION_HINT,
    models::DlibLandmarks,
    web::{self, AppState},
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

    // Check AVX support (required by ONNX models, x86_64 only)
    #[cfg(target_arch = "x86_64")]
    {
        if !std::arch::is_x86_feature_detected!("avx") {
            tracing::error!(
                "This CPU does not support AVX instructions, which are required by the ML models \
                (dlib landmarks, DMHead pose estimation). The application cannot run on this hardware."
            );
            std::process::exit(1);
        }
    }

    // Check output directory writability
    check_output_dir(&config);

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
    let app = web::create_router(state.clone());

    // Start server
    let addr = SocketAddr::from(([0, 0, 0, 0], 5000));
    tracing::info!("Starting server on http://{}", addr);

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal(state))
        .await?;

    Ok(())
}

async fn shutdown_signal(state: AppState) {
    let ctrl_c = async {
        tokio::signal::ctrl_c()
            .await
            .expect("failed to install Ctrl+C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
            .expect("failed to install SIGTERM handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => tracing::info!("Received Ctrl+C, shutting down"),
        _ = terminate => tracing::info!("Received SIGTERM, shutting down"),
    }

    state.request_cancel().await;
}

fn check_output_dir(config: &Config) {
    let dir = &config.output_dir;
    if let Err(e) = std::fs::create_dir_all(dir) {
        if e.kind() == std::io::ErrorKind::PermissionDenied {
            tracing::warn!(
                "Cannot create output directory '{}': permission denied. {}",
                dir.display(),
                PERMISSION_HINT
            );
        } else {
            tracing::warn!("Cannot create output directory '{}': {}", dir.display(), e);
        }
        return;
    }

    let test_file = dir.join(".writetest");
    match std::fs::write(&test_file, b"ok") {
        Ok(()) => {
            let _ = std::fs::remove_file(&test_file);
        }
        Err(e) if e.kind() == std::io::ErrorKind::PermissionDenied => {
            tracing::warn!(
                "Output directory '{}' is not writable: permission denied. {}",
                dir.display(),
                PERMISSION_HINT
            );
        }
        Err(e) => {
            tracing::warn!(
                "Output directory '{}' is not writable: {}",
                dir.display(),
                e
            );
        }
    }
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
                        {} Continuing with in-memory defaults.",
                        CONFIG_PATH,
                        PERMISSION_HINT
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
