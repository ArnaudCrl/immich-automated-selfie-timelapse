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
}

impl From<&SkipStats> for SkipStatsResponse {
    fn from(stats: &SkipStats) -> Self {
        Self {
            counts: stats.counts().clone(),
            total: stats.total(),
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

    Json(ProgressResponse {
        status: status_str.to_string(),
        completed: progress.completed,
        total: progress.total,
        message: progress.message.clone(),
        skip_stats: SkipStatsResponse::from(&progress.skip_stats),
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
