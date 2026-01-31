//! Health check and connection status endpoints.

use crate::immich_api::ImmichClient;
use crate::web::state::AppState;
use axum::{extract::State, response::Json};
use serde::Serialize;

/// Health check endpoint.
pub async fn health_check() -> &'static str {
    "OK"
}

/// Connection status response.
#[derive(Serialize)]
pub struct ConnectionStatus {
    pub connected: bool,
    pub version: Option<String>,
    pub error: Option<String>,
}

/// Check connection to Immich.
pub async fn check_connection(State(state): State<AppState>) -> Json<ConnectionStatus> {
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
