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

/// Progress information for a running job.
#[derive(Debug, Clone)]
pub struct Progress {
    pub status: JobStatus,
    pub completed: u32,
    pub total: u32,
    pub message: Option<String>,
}

impl Default for Progress {
    fn default() -> Self {
        Self {
            status: JobStatus::Idle,
            completed: 0,
            total: 0,
            message: None,
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
    pub async fn update_progress(&self, progress: Progress) {
        *self.progress.write().await = progress.clone();
        let _ = self.progress_tx.send(progress);
    }

    /// Request cancellation of the current job.
    pub async fn request_cancel(&self) -> bool {
        let cancel_token = self.cancel_token.read().await;
        if let Some(token) = cancel_token.as_ref() {
            token.cancel();
            // Update progress to show cancelling state immediately
            let mut progress = self.progress.write().await;
            if progress.status == JobStatus::Running || progress.status == JobStatus::CompilingVideo
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
