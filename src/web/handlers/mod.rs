//! HTTP route handlers.
//!
//! This module is organized into submodules by domain:
//! - `health`: Health check and connection status
//! - `people`: People listing, thumbnails, asset counts
//! - `processing`: Job control (progress, start, cancel)
//! - `output`: Output folder and image management
//! - `config`: Configuration get/update

mod config;
mod health;
mod output;
mod people;
mod processing;
mod ws;

use crate::web::state::AppState;
use axum::{
    http::header,
    response::{Html, IntoResponse},
    routing::{delete, get, post, put},
    Router,
};
use serde::Serialize;
use tower::ServiceBuilder;
use tower_http::services::ServeDir;
use tower_http::set_header::SetResponseHeaderLayer;

// Re-export handler functions for use in router
use config::{get_config, get_config_defaults, update_config};
use health::{check_connection, health_check};
use output::{
    cleanup_all_output, cleanup_output_folder, compile_folder_video, delete_images_bulk,
    delete_single_image, list_folder_images, list_output_folders,
};
use people::{get_people, get_person_asset_count, get_person_thumbnail};
use processing::{cancel_processing, get_progress, start_processing};
use ws::ws_handler;

// Re-export types that may be needed by other modules
pub use config::ConfigResponse;
pub use health::ConnectionStatus;
pub use output::{BulkDeleteResponse, FolderImagesResponse, ImageInfo, OutputFolderInfo};
pub use people::{AssetCountResponse, PersonInfo};
pub use processing::{ProgressResponse, SkipStatsResponse};

/// Common response type for simple success/failure operations.
#[derive(Serialize)]
pub struct StartResponse {
    pub success: bool,
    pub message: String,
}

/// Create the router with all routes.
pub fn create_router(state: AppState) -> Router {
    // Get output directory from config for serving results
    let output_dir = state
        .config
        .try_read()
        .map(|c| c.output_dir.clone())
        .unwrap_or_else(|_| std::path::PathBuf::from("output"));

    Router::new()
        // API routes
        .route("/api/health", get(health_check))
        .route("/api/connection", get(check_connection))
        .route("/api/people", get(get_people))
        .route(
            "/api/people/{person_id}/thumbnail",
            get(get_person_thumbnail),
        )
        .route(
            "/api/people/{person_id}/asset-count",
            get(get_person_asset_count),
        )
        .route("/api/progress", get(get_progress))
        .route("/api/ws", get(ws_handler))
        .route("/api/start", post(start_processing))
        .route("/api/cancel", post(cancel_processing))
        .route("/api/output", get(list_output_folders))
        .route("/api/output", delete(cleanup_all_output))
        .route("/api/output/{folder_name}", delete(cleanup_output_folder))
        .route("/api/output/{folder_name}/images", get(list_folder_images))
        .route(
            "/api/output/{folder_name}/images",
            delete(delete_images_bulk),
        )
        .route(
            "/api/output/{folder_name}/images/{filename}",
            delete(delete_single_image),
        )
        .route(
            "/api/output/{folder_name}/compile",
            post(compile_folder_video),
        )
        .route("/api/config", get(get_config))
        .route("/api/config", put(update_config))
        .route("/api/config/defaults", get(get_config_defaults))
        // Serve output files (video, images) — no-cache so browsers revalidate
        // after reprocessing (filenames stay the same across runs)
        .nest_service(
            "/output",
            ServiceBuilder::new()
                .layer(SetResponseHeaderLayer::overriding(
                    header::CACHE_CONTROL,
                    header::HeaderValue::from_static("no-store"),
                ))
                .service(ServeDir::new(output_dir)),
        )
        // Serve frontend static files (fallback to index.html for SPA routing)
        .fallback_service(ServeDir::new("frontend/dist").fallback(get(serve_index)))
        .with_state(state)
}

/// Serve index.html for SPA routing.
async fn serve_index() -> impl IntoResponse {
    match tokio::fs::read_to_string("frontend/dist/index.html").await {
        Ok(html) => Html(html).into_response(),
        Err(_) => Html(
            r#"<!DOCTYPE html>
<html>
<head><title>Immich Timelapse</title></head>
<body style="font-family: sans-serif; display: flex; justify-content: center; align-items: center; height: 100vh; margin: 0; background: #0f0f0f; color: #e0e0e0;">
  <div style="text-align: center;">
    <h1>Frontend not built</h1>
    <p>Run <code style="background: #333; padding: 0.25rem 0.5rem; border-radius: 4px;">cd frontend && npm install && npm run build</code></p>
    <p style="margin-top: 1rem; color: #888;">Or use <code style="background: #333; padding: 0.25rem 0.5rem; border-radius: 4px;">npm run dev</code> for development</p>
  </div>
</body>
</html>"#,
        )
        .into_response(),
    }
}
