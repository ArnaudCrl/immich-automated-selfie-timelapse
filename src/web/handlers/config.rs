//! Configuration endpoints.

use crate::config::{ProcessingConfig, VideoConfig};
use crate::web::state::{AppState, JobStatus};
use axum::{
    extract::State,
    http::StatusCode,
    response::Json,
};
use serde::{Deserialize, Serialize};

/// Validation error with field name and message.
struct ValidationError {
    field: &'static str,
    message: String,
}

impl ValidationError {
    fn new(field: &'static str, message: impl Into<String>) -> Self {
        Self {
            field,
            message: message.into(),
        }
    }
}

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

/// Validate processing configuration update values.
fn validate_processing_config(proc: &ProcessingConfigUpdate) -> Result<(), ValidationError> {
    if let Some(v) = proc.resize_size {
        if v < 64 || v > 4096 {
            return Err(ValidationError::new(
                "processing.resize_size",
                format!("must be between 64 and 4096, got {}", v),
            ));
        }
    }
    if let Some(v) = proc.face_resolution_threshold {
        if v == 0 || v > 1000 {
            return Err(ValidationError::new(
                "processing.face_resolution_threshold",
                format!("must be between 1 and 1000, got {}", v),
            ));
        }
    }
    if let Some(v) = proc.pose_threshold {
        if v <= 0.0 || v > 90.0 {
            return Err(ValidationError::new(
                "processing.pose_threshold",
                format!("must be between 0 and 90 degrees, got {}", v),
            ));
        }
    }
    if let Some(v) = proc.ear_threshold {
        if v <= 0.0 || v > 1.0 {
            return Err(ValidationError::new(
                "processing.ear_threshold",
                format!("must be between 0 and 1, got {}", v),
            ));
        }
    }
    if let Some(v) = proc.max_workers {
        if v == 0 || v > 64 {
            return Err(ValidationError::new(
                "processing.max_workers",
                format!("must be between 1 and 64, got {}", v),
            ));
        }
    }
    Ok(())
}

/// Validate video configuration update values.
fn validate_video_config(vid: &VideoConfigUpdate) -> Result<(), ValidationError> {
    if let Some(v) = vid.framerate {
        if v == 0 || v > 120 {
            return Err(ValidationError::new(
                "video.framerate",
                format!("must be between 1 and 120, got {}", v),
            ));
        }
    }
    if let Some(v) = vid.crf {
        if v > 51 {
            return Err(ValidationError::new(
                "video.crf",
                format!("must be between 0 and 51, got {}", v),
            ));
        }
    }
    if let Some(ref v) = vid.codec {
        // Allow common video codecs
        let valid_codecs = ["libx264", "libx265", "libvpx", "libvpx-vp9", "libaom-av1"];
        if !valid_codecs.contains(&v.as_str()) {
            return Err(ValidationError::new(
                "video.codec",
                format!(
                    "must be one of: {}, got '{}'",
                    valid_codecs.join(", "),
                    v
                ),
            ));
        }
    }
    Ok(())
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

    // Validate input before updating
    if let Some(ref proc) = update.processing {
        validate_processing_config(proc).map_err(|e| {
            (
                StatusCode::BAD_REQUEST,
                format!("Invalid {}: {}", e.field, e.message),
            )
        })?;
    }
    if let Some(ref vid) = update.video {
        validate_video_config(vid).map_err(|e| {
            (
                StatusCode::BAD_REQUEST,
                format!("Invalid {}: {}", e.field, e.message),
            )
        })?;
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

        // Persist to file (non-blocking, best-effort)
        if let Err(e) = config.save_to_file("config.toml") {
            tracing::warn!("Failed to persist config to file: {}", e);
        } else {
            tracing::info!("Configuration updated and persisted to config.toml");
        }
    }

    // Return updated config
    Ok(get_config(State(state)).await)
}
