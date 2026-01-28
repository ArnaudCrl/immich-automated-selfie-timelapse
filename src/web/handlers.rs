//! HTTP route handlers.

use crate::config::{ProcessingConfig, VideoConfig};
use crate::immich_api::ImmichClient;
use crate::job::{run_job, JobParams};
use crate::web::state::{AppState, JobStatus, Progress, SkipStats};
use axum::{
    body::Body,
    extract::{Path, State},
    http::{header, StatusCode},
    response::{Html, IntoResponse, Json, Response},
    routing::{delete, get, post, put},
    Router,
};
use serde::{Deserialize, Serialize};
use tower_http::services::ServeDir;

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
        .route("/api/start", post(start_processing))
        .route("/api/cancel", post(cancel_processing))
        .route("/api/output", get(list_output_folders))
        .route("/api/output", delete(cleanup_all_output))
        .route("/api/output/{folder_name}", delete(cleanup_output_folder))
        .route("/api/config", get(get_config))
        .route("/api/config", put(update_config))
        // Serve output files (video, images)
        .nest_service("/output", ServeDir::new(output_dir))
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

/// Health check endpoint.
async fn health_check() -> &'static str {
    "OK"
}

/// Check connection to Immich.
#[derive(Serialize)]
struct ConnectionStatus {
    connected: bool,
    version: Option<String>,
    error: Option<String>,
}

async fn check_connection(State(state): State<AppState>) -> Json<ConnectionStatus> {
    let config = state.config.read().await;
    let client = match ImmichClient::new(&config.api) {
        Ok(c) => c,
        Err(e) => {
            return Json(ConnectionStatus {
                connected: false,
                version: None,
                error: Some(e.to_string()),
            });
        }
    };

    match client.validate_connection().await {
        Ok(info) => Json(ConnectionStatus {
            connected: true,
            version: Some(info.version),
            error: None,
        }),
        Err(e) => Json(ConnectionStatus {
            connected: false,
            version: None,
            error: Some(e.to_string()),
        }),
    }
}

/// Get list of people from Immich.
#[derive(Serialize)]
struct PersonInfo {
    id: String,
    name: Option<String>,
}

async fn get_people(
    State(state): State<AppState>,
) -> Result<Json<Vec<PersonInfo>>, (StatusCode, String)> {
    let config = state.config.read().await;
    let client = ImmichClient::new(&config.api).map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Failed to create client: {}", e),
        )
    })?;

    let people = client.get_people().await.map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Failed to get people: {}", e),
        )
    })?;

    let people_info: Vec<PersonInfo> = people
        .into_iter()
        .map(|p| PersonInfo {
            id: p.id,
            name: p.name,
        })
        .collect();

    Ok(Json(people_info))
}

/// Get a person's thumbnail image.
async fn get_person_thumbnail(
    State(state): State<AppState>,
    Path(person_id): Path<String>,
) -> Result<Response<Body>, (StatusCode, String)> {
    let config = state.config.read().await;
    let client = ImmichClient::new(&config.api).map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Failed to create client: {}", e),
        )
    })?;

    let (bytes, content_type) = client.get_person_thumbnail(&person_id).await.map_err(|e| {
        tracing::error!("Thumbnail fetch failed for {}: {}", person_id, e);
        (
            StatusCode::NOT_FOUND,
            format!("Failed to get thumbnail: {}", e),
        )
    })?;

    // Return the image with appropriate headers
    Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, content_type)
        .header(header::CACHE_CONTROL, "public, max-age=3600")
        .body(Body::from(bytes))
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Failed to build response: {}", e),
            )
        })
}

/// Asset count response for a person.
#[derive(Serialize)]
struct AssetCountResponse {
    total_assets: u32,
    assets_with_faces: u32,
}

/// Get the count of assets for a person.
async fn get_person_asset_count(
    State(state): State<AppState>,
    Path(person_id): Path<String>,
) -> Result<Json<AssetCountResponse>, (StatusCode, String)> {
    let config = state.config.read().await;
    let client = ImmichClient::new(&config.api).map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Failed to create client: {}", e),
        )
    })?;

    // Fetch assets for this person
    let assets = client
        .get_assets_with_person(&person_id, None, None)
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Failed to get assets: {}", e),
            )
        })?;

    let total_assets = assets.len() as u32;

    // Count assets that have face data for the target person
    let assets_with_faces = assets
        .iter()
        .filter(|asset| {
            asset.people.as_ref().map_or(false, |people| {
                people.iter().any(|p| {
                    p.id == person_id && p.faces.as_ref().map_or(false, |faces| !faces.is_empty())
                })
            })
        })
        .count() as u32;

    Ok(Json(AssetCountResponse {
        total_assets,
        assets_with_faces,
    }))
}

/// Skip statistics for API response.
#[derive(Serialize)]
struct SkipStatsResponse {
    face_too_small: u32,
    eyes_closed: u32,
    head_turned: u32,
    too_dark: u32,
    too_bright: u32,
    no_face_detected: u32,
    download_failed: u32,
    decode_failed: u32,
    crop_failed: u32,
    total: u32,
}

/// Get current progress.
#[derive(Serialize)]
struct ProgressResponse {
    status: String,
    completed: u32,
    total: u32,
    message: Option<String>,
    skip_stats: SkipStatsResponse,
    person_id: Option<String>,
    person_name: Option<String>,
}

async fn get_progress(State(state): State<AppState>) -> Json<ProgressResponse> {
    let progress = state.progress.read().await;

    let status_str = match &progress.status {
        JobStatus::Idle => "idle",
        JobStatus::Running => "running",
        JobStatus::Cancelling => "cancelling",
        JobStatus::CompilingVideo => "compiling_video",
        JobStatus::Completed => "completed",
        JobStatus::Cancelled => "cancelled",
        JobStatus::Error(_) => "error",
    };

    let skip_stats = &progress.skip_stats;

    Json(ProgressResponse {
        status: status_str.to_string(),
        completed: progress.completed,
        total: progress.total,
        message: progress.message.clone(),
        skip_stats: SkipStatsResponse {
            face_too_small: skip_stats.face_too_small,
            eyes_closed: skip_stats.eyes_closed,
            head_turned: skip_stats.head_turned,
            too_dark: skip_stats.too_dark,
            too_bright: skip_stats.too_bright,
            no_face_detected: skip_stats.no_face_detected,
            download_failed: skip_stats.download_failed,
            decode_failed: skip_stats.decode_failed,
            crop_failed: skip_stats.crop_failed,
            total: skip_stats.total(),
        },
        person_id: progress.person_id.clone(),
        person_name: progress.person_name.clone(),
    })
}

/// Start processing request.
#[derive(Deserialize)]
struct StartRequest {
    person_id: String,
    person_name: Option<String>,
    date_from: Option<String>,
    date_to: Option<String>,
}

#[derive(Serialize)]
struct StartResponse {
    success: bool,
    message: String,
}

async fn start_processing(
    State(state): State<AppState>,
    Json(request): Json<StartRequest>,
) -> Result<Json<StartResponse>, (StatusCode, String)> {
    // Check if already running
    {
        let progress = state.progress.read().await;
        if progress.status == JobStatus::Running || progress.status == JobStatus::CompilingVideo {
            return Err((StatusCode::CONFLICT, "A job is already running".to_string()));
        }
    }

    // Reset progress with person info
    state
        .update_progress(Progress {
            status: JobStatus::Running,
            completed: 0,
            total: 0,
            message: Some("Starting...".to_string()),
            skip_stats: SkipStats::default(),
            person_id: Some(request.person_id.clone()),
            person_name: request.person_name.clone(),
        })
        .await;

    // Create cancellation token
    let cancel_token = state.create_cancel_token().await;

    tracing::info!(
        "Starting processing for person {} (date range: {:?} - {:?})",
        request.person_id,
        request.date_from,
        request.date_to
    );

    // Spawn the processing job in the background
    let job_params = JobParams {
        person_id: request.person_id,
        person_name: request.person_name,
        date_from: request.date_from,
        date_to: request.date_to,
    };

    let job_state = state.clone();
    tokio::spawn(async move {
        run_job(job_state, job_params, cancel_token).await;
    });

    Ok(Json(StartResponse {
        success: true,
        message: "Processing started".to_string(),
    }))
}

/// Cancel the current processing job.
async fn cancel_processing(State(state): State<AppState>) -> Json<StartResponse> {
    let cancelled = state.request_cancel().await;

    if cancelled {
        // The job will update its own status when it detects cancellation
        Json(StartResponse {
            success: true,
            message: "Cancellation requested".to_string(),
        })
    } else {
        Json(StartResponse {
            success: false,
            message: "No job running to cancel".to_string(),
        })
    }
}

/// Output folder information.
#[derive(Serialize)]
struct OutputFolderInfo {
    name: String,
    image_count: u32,
    size_bytes: u64,
    has_video: bool,
}

/// List all output folders with their stats.
async fn list_output_folders(
    State(state): State<AppState>,
) -> Result<Json<Vec<OutputFolderInfo>>, (StatusCode, String)> {
    let config = state.config.read().await;
    let output_dir = &config.output_dir;

    let mut folders = Vec::new();

    if output_dir.exists() {
        let mut entries = tokio::fs::read_dir(output_dir).await.map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Failed to read output directory: {}", e),
            )
        })?;

        while let Some(entry) = entries.next_entry().await.map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Failed to read directory entry: {}", e),
            )
        })? {
            let path = entry.path();
            if path.is_dir() {
                let name = entry.file_name().to_string_lossy().to_string();

                // Count images in the images subfolder
                let images_dir = path.join("images");
                let mut image_count = 0u32;
                let mut size_bytes = 0u64;

                if images_dir.exists() {
                    if let Ok(mut img_entries) = tokio::fs::read_dir(&images_dir).await {
                        while let Ok(Some(img_entry)) = img_entries.next_entry().await {
                            let img_path = img_entry.path();
                            if img_path.extension().map_or(false, |ext| ext == "jpg") {
                                image_count += 1;
                                if let Ok(metadata) = tokio::fs::metadata(&img_path).await {
                                    size_bytes += metadata.len();
                                }
                            }
                        }
                    }
                }

                // Check for video file
                let has_video = path.join("timelapse.mp4").exists();

                folders.push(OutputFolderInfo {
                    name,
                    image_count,
                    size_bytes,
                    has_video,
                });
            }
        }
    }

    // Sort by name
    folders.sort_by(|a, b| a.name.cmp(&b.name));

    Ok(Json(folders))
}

/// Clean up all output folders.
async fn cleanup_all_output(
    State(state): State<AppState>,
) -> Result<Json<StartResponse>, (StatusCode, String)> {
    let config = state.config.read().await;
    let output_dir = &config.output_dir;

    // Check if a job is running
    {
        let progress = state.progress.read().await;
        if progress.status == JobStatus::Running || progress.status == JobStatus::CompilingVideo {
            return Err((
                StatusCode::CONFLICT,
                "Cannot cleanup while a job is running".to_string(),
            ));
        }
    }

    // Remove all contents of output directory
    if output_dir.exists() {
        let mut entries = tokio::fs::read_dir(output_dir).await.map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Failed to read output directory: {}", e),
            )
        })?;

        let mut deleted_count = 0;
        while let Some(entry) = entries.next_entry().await.map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Failed to read directory entry: {}", e),
            )
        })? {
            let path = entry.path();
            if path.is_dir() {
                tokio::fs::remove_dir_all(&path).await.map_err(|e| {
                    (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        format!("Failed to remove directory {}: {}", path.display(), e),
                    )
                })?;
                deleted_count += 1;
            }
        }

        tracing::info!("Cleaned up {} output folders", deleted_count);

        Ok(Json(StartResponse {
            success: true,
            message: format!("Deleted {} output folders", deleted_count),
        }))
    } else {
        Ok(Json(StartResponse {
            success: true,
            message: "Output directory does not exist".to_string(),
        }))
    }
}

/// Clean up a specific output folder by name.
async fn cleanup_output_folder(
    State(state): State<AppState>,
    Path(folder_name): Path<String>,
) -> Result<Json<StartResponse>, (StatusCode, String)> {
    let config = state.config.read().await;

    // Check if a job is running
    {
        let progress = state.progress.read().await;
        if progress.status == JobStatus::Running || progress.status == JobStatus::CompilingVideo {
            return Err((
                StatusCode::CONFLICT,
                "Cannot cleanup while a job is running".to_string(),
            ));
        }
    }

    // Sanitize folder name to prevent path traversal
    if folder_name.contains("..") || folder_name.contains('/') || folder_name.contains('\\') {
        return Err((StatusCode::BAD_REQUEST, "Invalid folder name".to_string()));
    }

    let folder_path = config.output_dir.join(&folder_name);

    if !folder_path.exists() {
        return Err((
            StatusCode::NOT_FOUND,
            format!("Folder '{}' not found", folder_name),
        ));
    }

    if !folder_path.is_dir() {
        return Err((
            StatusCode::BAD_REQUEST,
            format!("'{}' is not a directory", folder_name),
        ));
    }

    tokio::fs::remove_dir_all(&folder_path).await.map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Failed to remove folder: {}", e),
        )
    })?;

    tracing::info!("Cleaned up output folder: {}", folder_name);

    Ok(Json(StartResponse {
        success: true,
        message: format!("Deleted folder '{}'", folder_name),
    }))
}

/// Configuration response (excludes sensitive API credentials).
/// Reuses config types which already derive Serialize.
#[derive(Serialize)]
struct ConfigResponse {
    processing: ProcessingConfig,
    video: VideoConfig,
}

/// Get current configuration (excluding sensitive data).
async fn get_config(State(state): State<AppState>) -> Json<ConfigResponse> {
    let config = state.config.read().await;

    Json(ConfigResponse {
        processing: config.processing.clone(),
        video: config.video.clone(),
    })
}

/// Configuration update request.
#[derive(Deserialize)]
struct ConfigUpdateRequest {
    processing: Option<ProcessingConfigUpdate>,
    video: Option<VideoConfigUpdate>,
}

#[derive(Deserialize)]
struct ProcessingConfigUpdate {
    resize_size: Option<u32>,
    face_resolution_threshold: Option<u32>,
    pose_threshold: Option<f32>,
    ear_threshold: Option<f32>,
    max_workers: Option<usize>,
    keep_intermediates: Option<bool>,
}

#[derive(Deserialize)]
struct VideoConfigUpdate {
    framerate: Option<u32>,
    enabled: Option<bool>,
    codec: Option<String>,
    crf: Option<u32>,
}

/// Update configuration.
async fn update_config(
    State(state): State<AppState>,
    Json(update): Json<ConfigUpdateRequest>,
) -> Result<Json<ConfigResponse>, (StatusCode, String)> {
    // Check if a job is running
    {
        let progress = state.progress.read().await;
        if progress.status == JobStatus::Running || progress.status == JobStatus::CompilingVideo {
            return Err((
                StatusCode::CONFLICT,
                "Cannot update config while a job is running".to_string(),
            ));
        }
    }

    // Update config
    {
        let mut config = state.config.write().await;

        if let Some(proc) = update.processing {
            if let Some(v) = proc.resize_size {
                config.processing.resize_size = v;
            }
            if let Some(v) = proc.face_resolution_threshold {
                config.processing.face_resolution_threshold = v;
            }
            if let Some(v) = proc.pose_threshold {
                config.processing.pose_threshold = v;
            }
            if let Some(v) = proc.ear_threshold {
                config.processing.ear_threshold = v;
            }
            if let Some(v) = proc.max_workers {
                config.processing.max_workers = v;
            }
            if let Some(v) = proc.keep_intermediates {
                config.processing.keep_intermediates = v;
            }
        }

        if let Some(vid) = update.video {
            if let Some(v) = vid.framerate {
                config.video.framerate = v;
            }
            if let Some(v) = vid.enabled {
                config.video.enabled = v;
            }
            if let Some(v) = vid.codec {
                config.video.codec = v;
            }
            if let Some(v) = vid.crf {
                config.video.crf = v;
            }
        }

        tracing::info!("Configuration updated");
    }

    // Return updated config
    Ok(get_config(State(state)).await)
}
