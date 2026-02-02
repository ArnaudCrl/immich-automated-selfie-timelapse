//! Configuration endpoints.

use crate::config::{
    AlignmentConfig, BrightnessConfig, FaceResolutionConfig, OutputConfig, ProcessingConfig,
    VideoConfig,
};
use crate::web::state::AppState;
use axum::{extract::State, http::StatusCode, response::Json};
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
///
/// Uses the same nested structure as the config itself for consistency.
#[derive(Deserialize)]
pub struct ConfigUpdateRequest {
    pub processing: Option<ProcessingConfigUpdate>,
    pub video: Option<VideoConfigUpdate>,
}

/// Processing configuration update fields.
#[derive(Deserialize)]
pub struct ProcessingConfigUpdate {
    pub max_workers: Option<usize>,
    pub face_resolution: Option<FaceResolutionConfig>,
    pub brightness: Option<BrightnessConfig>,
    pub output: Option<OutputConfig>,
    pub alignment: Option<AlignmentConfig>,
}

/// Video configuration update fields.
#[derive(Deserialize)]
pub struct VideoConfigUpdate {
    pub enabled: Option<bool>,
    pub framerate: Option<u32>,
    pub codec: Option<String>,
    pub crf: Option<u32>,
}

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

/// Validate processing configuration update values.
fn validate_processing_config(proc: &ProcessingConfigUpdate) -> Result<(), ValidationError> {
    if let Some(v) = proc.max_workers {
        if v == 0 || v > 64 {
            return Err(ValidationError::new(
                "processing.max_workers",
                format!("must be between 1 and 64, got {}", v),
            ));
        }
    }

    if let Some(ref fr) = proc.face_resolution {
        if fr.enabled && fr.min_size == 0 {
            return Err(ValidationError::new(
                "processing.face_resolution.min_size",
                "must be greater than 0 when enabled",
            ));
        }
        if fr.min_size > 1000 {
            return Err(ValidationError::new(
                "processing.face_resolution.min_size",
                format!("must be at most 1000, got {}", fr.min_size),
            ));
        }
    }

    if let Some(ref br) = proc.brightness {
        if br.enabled {
            if br.min_brightness < 0.0 || br.min_brightness > 1.0 {
                return Err(ValidationError::new(
                    "processing.brightness.min_brightness",
                    format!("must be between 0.0 and 1.0, got {}", br.min_brightness),
                ));
            }
            if br.max_brightness < 0.0 || br.max_brightness > 1.0 {
                return Err(ValidationError::new(
                    "processing.brightness.max_brightness",
                    format!("must be between 0.0 and 1.0, got {}", br.max_brightness),
                ));
            }
            if br.min_brightness >= br.max_brightness {
                return Err(ValidationError::new(
                    "processing.brightness",
                    "min_brightness must be less than max_brightness",
                ));
            }
        }
    }

    if let Some(ref out) = proc.output {
        if out.size < 64 {
            return Err(ValidationError::new(
                "processing.output.size",
                format!("must be at least 64, got {}", out.size),
            ));
        }
        if out.size > 4096 {
            return Err(ValidationError::new(
                "processing.output.size",
                format!("must be at most 4096, got {}", out.size),
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
    state
        .ensure_no_job_running()
        .await
        .map_err(|e| (StatusCode::CONFLICT, e.to_string()))?;

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
            if let Some(v) = proc.max_workers {
                config.processing.max_workers = v;
            }
            if let Some(v) = proc.face_resolution {
                config.processing.face_resolution = v;
            }
            if let Some(v) = proc.brightness {
                config.processing.brightness = v;
            }
            if let Some(v) = proc.output {
                config.processing.output = v;
            }
            if let Some(v) = proc.alignment {
                config.processing.alignment = v;
            }
        }

        if let Some(vid) = update.video {
            if let Some(v) = vid.enabled {
                config.video.enabled = v;
            }
            if let Some(v) = vid.framerate {
                config.video.framerate = v;
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
