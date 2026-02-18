//! Processing job control endpoints (progress, start, cancel).

use crate::job::{run_job, JobParams};
use crate::web::state::{AppState, JobStatus, Progress, SkipStats};
use axum::{extract::State, http::StatusCode, response::Json};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use super::StartResponse;

/// Skip statistics for API response.
///
/// Uses a dynamic HashMap to support arbitrary skip reasons from pipeline steps.
#[derive(Serialize)]
pub struct SkipStatsResponse {
    /// Map of skip reason ID to count.
    #[serde(flatten)]
    pub counts: HashMap<String, u32>,
    /// Total number of skipped images.
    pub total: u32,
    /// Number of images kept (passed all filters).
    pub kept: u32,
}

impl From<&SkipStats> for SkipStatsResponse {
    fn from(stats: &SkipStats) -> Self {
        Self {
            counts: stats.counts().clone(),
            total: stats.total(),
            kept: stats.kept(),
        }
    }
}

/// Progress response.
#[derive(Serialize)]
pub struct ProgressResponse {
    pub status: String,
    pub completed: u32,
    pub total: u32,
    pub message: Option<String>,
    pub skip_stats: SkipStatsResponse,
    pub person_id: Option<String>,
    pub person_name: Option<String>,
    pub album_id: Option<String>,
    pub album_name: Option<String>,
}

impl From<&Progress> for ProgressResponse {
    fn from(p: &Progress) -> Self {
        Self {
            status: p.status.as_str().to_string(),
            completed: p.completed,
            total: p.total,
            message: p.message.clone(),
            skip_stats: SkipStatsResponse::from(&p.skip_stats),
            person_id: p.person_id.clone(),
            person_name: p.person_name.clone(),
            album_id: p.album_id.clone(),
            album_name: p.album_name.clone(),
        }
    }
}

/// Get current progress.
pub async fn get_progress(State(state): State<AppState>) -> Json<ProgressResponse> {
    Json(ProgressResponse::from(&*state.progress.read().await))
}

/// Start processing request.
#[derive(Deserialize)]
pub struct StartRequest {
    pub person_id: String,
    pub person_name: Option<String>,
    pub date_from: Option<String>,
    pub date_to: Option<String>,
    pub album_id: Option<String>,
    pub album_name: Option<String>,
}

/// Validate that a person_id looks reasonable (non-empty, reasonable length, safe characters).
fn validate_person_id(person_id: &str) -> Result<(), (StatusCode, String)> {
    if person_id.is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            "person_id must not be empty".to_string(),
        ));
    }
    if person_id.len() > 128 {
        return Err((
            StatusCode::BAD_REQUEST,
            "person_id is too long (max 128 characters)".to_string(),
        ));
    }
    if !person_id
        .chars()
        .all(|c| c.is_alphanumeric() || c == '-' || c == '_')
    {
        return Err((
            StatusCode::BAD_REQUEST,
            "person_id contains invalid characters (only alphanumeric, hyphens, underscores allowed)"
                .to_string(),
        ));
    }
    Ok(())
}

/// Start processing for a person.
pub async fn start_processing(
    State(state): State<AppState>,
    Json(request): Json<StartRequest>,
) -> Result<Json<StartResponse>, (StatusCode, String)> {
    validate_person_id(&request.person_id)?;

    // Atomically check-and-set: hold write lock across both check and status update
    // to prevent TOCTOU race conditions between concurrent requests.
    {
        let mut progress = state.progress.write().await;
        if progress.status == JobStatus::Running || progress.status == JobStatus::CompilingVideo {
            return Err((StatusCode::CONFLICT, "A job is already running".to_string()));
        }
        let new_progress = Progress {
            status: JobStatus::Running,
            message: Some("Starting...".to_string()),
            person_id: Some(request.person_id.clone()),
            person_name: request.person_name.clone(),
            album_id: request.album_id.clone(),
            album_name: request.album_name.clone(),
            ..Progress::default()
        };
        *progress = new_progress.clone();
        let _ = state.progress_tx.send(new_progress);
    }

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
        album_id: request.album_id,
        album_name: request.album_name,
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
pub async fn cancel_processing(State(state): State<AppState>) -> Json<StartResponse> {
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
