//! Application error types.

use thiserror::Error;

/// User-facing hint for Docker permission errors. Used in error messages and startup checks.
pub const PERMISSION_HINT: &str = "\
Make sure that the user running the Docker container has access to the config and output folders:\n\
- In docker-compose.yml add `user: 1000:1000`\n\
- Then run `chown -R 1000:1000 path/to/output path/to/config`";

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

impl Error {
    /// This is for messages shown in the UI. Keep `self.to_string()` for logs.
    pub fn user_message(&self) -> String {
        match self {
            Error::Io(e) if e.kind() == std::io::ErrorKind::PermissionDenied => {
                format!("Permission denied: {}\n\n{}", e, PERMISSION_HINT)
            }
            Error::Http(e) => {
                let msg = e.to_string();
                if e.is_connect() {
                    if msg.contains("dns error") || msg.contains("resolve") {
                        format!(
                            "Could not resolve hostname. \
                            If running in Docker, use the container name \
                            (e.g. `http://immich-server:2283`) instead of `localhost`.\n\n\
                            Raw error: {}",
                            msg
                        )
                    } else {
                        format!(
                            "Connection refused. Is Immich running at this address?\n\n\
                            Raw error: {}",
                            msg
                        )
                    }
                } else if e.is_timeout() {
                    format!(
                        "Connection timed out. \
                        Check that the URL is reachable from this container.\n\n\
                        Raw error: {}",
                        msg
                    )
                } else {
                    msg
                }
            }
            other => other.to_string(),
        }
    }
}

/// Convenience Result type.
pub type Result<T> = std::result::Result<T, Error>;
