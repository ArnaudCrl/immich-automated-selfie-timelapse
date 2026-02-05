//! WebSocket handler for real-time progress updates.

use crate::web::handlers::ProgressResponse;
use crate::web::state::{AppState, JobStatus};
use axum::{
    extract::{
        ws::{Message, WebSocket},
        State, WebSocketUpgrade,
    },
    response::IntoResponse,
};
use futures_util::{stream::StreamExt, SinkExt};

/// WebSocket upgrade handler.
pub async fn ws_handler(ws: WebSocketUpgrade, State(state): State<AppState>) -> impl IntoResponse {
    ws.on_upgrade(|socket| handle_socket(socket, state))
}

/// Handle an individual WebSocket connection.
async fn handle_socket(socket: WebSocket, state: AppState) {
    let (mut sender, mut receiver) = socket.split();

    // Subscribe to progress updates
    let mut progress_rx = state.progress_tx.subscribe();

    // Send current progress immediately on connect
    {
        let progress = state.progress.read().await;
        let response = progress_to_response(&progress);
        if let Ok(json) = serde_json::to_string(&response) {
            if sender.send(Message::Text(json.into())).await.is_err() {
                return;
            }
        }
    }

    // Spawn a task to handle incoming messages (ping/pong, close)
    let mut recv_task = tokio::spawn(async move {
        while let Some(msg) = receiver.next().await {
            match msg {
                Ok(Message::Close(_)) => break,
                Err(_) => break,
                _ => {} // Ignore other messages
            }
        }
    });

    // Send progress updates to the client
    let mut send_task = tokio::spawn(async move {
        while let Ok(progress) = progress_rx.recv().await {
            let response = progress_to_response(&progress);
            match serde_json::to_string(&response) {
                Ok(json) => {
                    if sender.send(Message::Text(json.into())).await.is_err() {
                        break;
                    }
                }
                Err(_) => break,
            }
        }
    });

    // Wait for either task to complete
    tokio::select! {
        _ = &mut recv_task => {
            send_task.abort();
        }
        _ = &mut send_task => {
            recv_task.abort();
        }
    }
}

/// Convert Progress to ProgressResponse for JSON serialization.
fn progress_to_response(progress: &crate::web::state::Progress) -> ProgressResponse {
    use crate::web::handlers::SkipStatsResponse;

    let status_str = match &progress.status {
        JobStatus::Idle => "idle",
        JobStatus::Running => "running",
        JobStatus::Cancelling => "cancelling",
        JobStatus::CompilingVideo => "compiling_video",
        JobStatus::Completed => "completed",
        JobStatus::Cancelled => "cancelled",
        JobStatus::Error(_) => "error",
    };

    ProgressResponse {
        status: status_str.to_string(),
        completed: progress.completed,
        total: progress.total,
        message: progress.message.clone(),
        skip_stats: SkipStatsResponse::from(&progress.skip_stats),
        person_id: progress.person_id.clone(),
        person_name: progress.person_name.clone(),
    }
}
