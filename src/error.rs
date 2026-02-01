//! Application error types.

use thiserror::Error;

/// Main error type for the application.
#[derive(Error, Debug)]
pub enum Error {
    #[error("Configuration error: {0}")]
    Config(String),

    #[error("Immich API error: {0}")]
    ImmichApi(String),

    #[error("HTTP request failed: {0}")]
    Http(#[from] reqwest::Error),

    #[error("Image processing error: {0}")]
    ImageProcessing(String),

    #[error("Face detection error: {0}")]
    FaceDetection(String),

    #[error("No face found in image")]
    NoFaceFound,

    #[error("Face quality check failed: {0}")]
    QualityCheck(String),

    #[error("Video compilation error: {0}")]
    VideoCompilation(String),

    #[error("FFmpeg error: {0}")]
    FFmpeg(String),

    #[error("ML model error: {0}")]
    Model(String),

    #[error("Landmark detection error: {0}")]
    LandmarkDetection(String),

    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("JSON parsing error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("Job cancelled")]
    Cancelled,
}

/// Convenience Result type.
pub type Result<T> = std::result::Result<T, Error>;
