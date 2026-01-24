//! Application state for the web server.

use crate::config::Config;
use std::sync::Arc;
use tokio::sync::{broadcast, RwLock};

/// Job status.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum JobStatus {
    Idle,
    Running,
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

    /// Cancellation signal sender.
    pub cancel_tx: Arc<RwLock<Option<tokio::sync::oneshot::Sender<()>>>>,
}

impl AppState {
    pub fn new(config: Config) -> Self {
        let (progress_tx, _) = broadcast::channel(100);

        Self {
            config: Arc::new(RwLock::new(config)),
            progress: Arc::new(RwLock::new(Progress::default())),
            progress_tx,
            cancel_tx: Arc::new(RwLock::new(None)),
        }
    }

    /// Update progress and broadcast to all listeners.
    pub async fn update_progress(&self, progress: Progress) {
        *self.progress.write().await = progress.clone();
        let _ = self.progress_tx.send(progress);
    }

    /// Request cancellation of the current job.
    pub async fn request_cancel(&self) -> bool {
        let mut cancel_tx = self.cancel_tx.write().await;
        if let Some(tx) = cancel_tx.take() {
            let _ = tx.send(());
            true
        } else {
            false
        }
    }

    /// Set the cancellation sender for a new job.
    pub async fn set_cancel_sender(&self, tx: tokio::sync::oneshot::Sender<()>) {
        *self.cancel_tx.write().await = Some(tx);
    }
}
