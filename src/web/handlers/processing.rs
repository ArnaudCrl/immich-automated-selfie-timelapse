//! Processing job control endpoints (progress, start, cancel).

use crate::job::{run_job, JobParams};
use crate::web::state::{AppState, JobStatus, Progress, SkipStats};
use axum::{
    extract::State,
    http::StatusCode,
    response::Json,
};
use serde::{Deserialize, Serialize};

use super::StartResponse;

/// Skip statistics for API response.
#[derive(Serialize)]
pub struct SkipStatsResponse {
    pub face_too_small: u32,
    pub eyes_closed: u32,
    pub head_turned: u32,
    pub too_dark: u32,
    pub too_bright: u32,
    pub no_face_detected: u32,
    pub download_failed: u32,
    pub decode_failed: u32,
    pub crop_failed: u32,
    pub total: u32,
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
}

/// Get current progress.
pub async fn get_progress(State(state): State<AppState>) -> Json<ProgressResponse> {
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
pub struct StartRequest {
    pub person_id: String,
    pub person_name: Option<String>,
    pub date_from: Option<String>,
    pub date_to: Option<String>,
}

/// Start processing for a person.
pub async fn start_processing(
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

    // Reset progress with person info (bypasses terminal state check)
    state
        .reset_progress(Progress {
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
