//! HTTP route handlers.

use crate::immich_api::ImmichClient;
use crate::job::{run_job, JobParams};
use crate::web::state::{AppState, JobStatus, Progress};
use axum::{
    extract::State,
    http::StatusCode,
    response::Json,
    routing::{get, post},
    Router,
};
use serde::{Deserialize, Serialize};

/// Create the router with all routes.
pub fn create_router(state: AppState) -> Router {
    Router::new()
        .route("/api/health", get(health_check))
        .route("/api/connection", get(check_connection))
        .route("/api/people", get(get_people))
        .route("/api/progress", get(get_progress))
        .route("/api/start", post(start_processing))
        .route("/api/cancel", post(cancel_processing))
        .with_state(state)
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

/// Get current progress.
#[derive(Serialize)]
struct ProgressResponse {
    status: String,
    completed: u32,
    total: u32,
    message: Option<String>,
}

async fn get_progress(State(state): State<AppState>) -> Json<ProgressResponse> {
    let progress = state.progress.read().await;

    let status_str = match &progress.status {
        JobStatus::Idle => "idle",
        JobStatus::Running => "running",
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
    })
}

/// Start processing request.
#[derive(Deserialize)]
struct StartRequest {
    person_id: String,
    date_from: Option<String>,
    date_to: Option<String>,
    // Processing options can be added here
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
            return Err((
                StatusCode::CONFLICT,
                "A job is already running".to_string(),
            ));
        }
    }

    // Reset progress
    state
        .update_progress(Progress {
            status: JobStatus::Running,
            completed: 0,
            total: 0,
            message: Some("Starting...".to_string()),
        })
        .await;

    // Create cancellation channel
    let (cancel_tx, cancel_rx) = tokio::sync::oneshot::channel();
    state.set_cancel_sender(cancel_tx).await;

    tracing::info!(
        "Starting processing for person {} (date range: {:?} - {:?})",
        request.person_id,
        request.date_from,
        request.date_to
    );

    // Spawn the processing job in the background
    let job_params = JobParams {
        person_id: request.person_id,
        date_from: request.date_from,
        date_to: request.date_to,
    };

    let job_state = state.clone();
    tokio::spawn(async move {
        run_job(job_state, job_params, cancel_rx).await;
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
        state
            .update_progress(Progress {
                status: JobStatus::Cancelled,
                completed: 0,
                total: 0,
                message: Some("Cancelled by user".to_string()),
            })
            .await;

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
