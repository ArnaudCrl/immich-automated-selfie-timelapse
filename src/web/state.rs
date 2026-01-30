//! Application state for the web server.

use crate::config::Config;
use std::sync::Arc;
use tokio::sync::{broadcast, RwLock};
use tokio_util::sync::CancellationToken;

/// Job status.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum JobStatus {
    Idle,
    Running,
    Cancelling,
    CompilingVideo,
    Completed,
    Cancelled,
    Error(String),
}

impl JobStatus {
    /// Returns true if this is a terminal status (job has finished).
    /// Terminal statuses should not be overwritten by non-terminal statuses.
    pub fn is_terminal(&self) -> bool {
        matches!(self, JobStatus::Completed | JobStatus::Cancelled | JobStatus::Error(_))
    }
}

/// Detailed statistics for skipped images.
#[derive(Debug, Clone, Default)]
pub struct SkipStats {
    pub face_too_small: u32,
    pub eyes_closed: u32,
    pub head_turned: u32,
    pub too_dark: u32,
    pub too_bright: u32,
    pub no_face_detected: u32,
    pub download_failed: u32,
    pub decode_failed: u32,
    pub crop_failed: u32,
}

impl SkipStats {
    /// Total number of skipped images.
    pub fn total(&self) -> u32 {
        self.face_too_small
            + self.eyes_closed
            + self.head_turned
            + self.too_dark
            + self.too_bright
            + self.no_face_detected
            + self.download_failed
            + self.decode_failed
            + self.crop_failed
    }
}

/// Progress information for a running job.
#[derive(Debug, Clone)]
pub struct Progress {
    pub status: JobStatus,
    pub completed: u32,
    pub total: u32,
    pub message: Option<String>,
    pub skip_stats: SkipStats,
    /// The person being processed (for display when resuming)
    pub person_id: Option<String>,
    pub person_name: Option<String>,
}

impl Default for Progress {
    fn default() -> Self {
        Self {
            status: JobStatus::Idle,
            completed: 0,
            total: 0,
            message: None,
            skip_stats: SkipStats::default(),
            person_id: None,
            person_name: None,
        }
    }
}

/// Shared application state.
#[derive(Clone)]
pub struct AppState {
    /// Current configuration.
    pub config: Arc<RwLock<Config>>,

    /// Current job progress.
    pub progress: Arc<RwLock<Progress>>,

    /// Channel for broadcasting progress updates.
    pub progress_tx: broadcast::Sender<Progress>,

    /// Cancellation token for the current job.
    pub cancel_token: Arc<RwLock<Option<CancellationToken>>>,
}

impl AppState {
    pub fn new(config: Config) -> Self {
        let (progress_tx, _) = broadcast::channel(100);

        Self {
            config: Arc::new(RwLock::new(config)),
            progress: Arc::new(RwLock::new(Progress::default())),
            progress_tx,
            cancel_token: Arc::new(RwLock::new(None)),
        }
    }

    /// Update progress and broadcast to all listeners.
    ///
    /// Note: This method prevents non-terminal statuses (Running, CompilingVideo)
    /// from overwriting terminal statuses (Completed, Cancelled, Error). This
    /// avoids race conditions where fire-and-forget progress updates arrive after
    /// the job has finished.
    pub async fn update_progress(&self, progress: Progress) {
        let mut current = self.progress.write().await;

        // Don't allow non-terminal statuses to overwrite terminal statuses.
        // This prevents race conditions from fire-and-forget progress callbacks.
        if current.status.is_terminal() && !progress.status.is_terminal() {
            tracing::debug!(
                "Ignoring progress update: current status {:?} is terminal, incoming {:?} is not",
                current.status,
                progress.status
            );
            return;
        }

        *current = progress.clone();
        let _ = self.progress_tx.send(progress);
    }

    /// Request cancellation of the current job.
    pub async fn request_cancel(&self) -> bool {
        let cancel_token = self.cancel_token.read().await;
        if let Some(token) = cancel_token.as_ref() {
            token.cancel();
            // Update progress to show cancelling state immediately.
            // Only change to Cancelling if in an active (non-terminal) state.
            let mut progress = self.progress.write().await;
            if progress.status == JobStatus::Running
                || progress.status == JobStatus::CompilingVideo
            {
                progress.status = JobStatus::Cancelling;
                progress.message = Some("Cancelling...".to_string());
            }
            true
        } else {
            false
        }
    }

    /// Create a new cancellation token for a job.
    pub async fn create_cancel_token(&self) -> CancellationToken {
        let token = CancellationToken::new();
        *self.cancel_token.write().await = Some(token.clone());
        token
    }

    /// Clear the cancellation token when job completes.
    pub async fn clear_cancel_token(&self) {
        *self.cancel_token.write().await = None;
    }
}
