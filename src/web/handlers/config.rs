//! Configuration endpoints.

use crate::config::{ProcessingConfig, VideoConfig};
use crate::web::state::{AppState, JobStatus};
use axum::{
    extract::State,
    http::StatusCode,
    response::Json,
};
use serde::{Deserialize, Serialize};

/// Configuration response (excludes sensitive API credentials).
#[derive(Serialize)]
pub struct ConfigResponse {
    pub processing: ProcessingConfig,
    pub video: VideoConfig,
}

/// Get current configuration (excluding sensitive data).
pub async fn get_config(State(state): State<AppState>) -> Json<ConfigResponse> {
    let config = state.config.read().await;

    Json(ConfigResponse {
        processing: config.processing.clone(),
        video: config.video.clone(),
    })
}

/// Configuration update request.
#[derive(Deserialize)]
pub struct ConfigUpdateRequest {
    pub processing: Option<ProcessingConfigUpdate>,
    pub video: Option<VideoConfigUpdate>,
}

/// Processing configuration update fields.
#[derive(Deserialize)]
pub struct ProcessingConfigUpdate {
    pub resize_size: Option<u32>,
    pub face_resolution_threshold: Option<u32>,
    pub pose_threshold: Option<f32>,
    pub ear_threshold: Option<f32>,
    pub max_workers: Option<usize>,
    pub keep_intermediates: Option<bool>,
}

/// Video configuration update fields.
#[derive(Deserialize)]
pub struct VideoConfigUpdate {
    pub framerate: Option<u32>,
    pub enabled: Option<bool>,
    pub codec: Option<String>,
    pub crf: Option<u32>,
}

/// Update configuration.
pub async fn update_config(
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
