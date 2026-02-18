//! Album-related endpoints (list, thumbnails).

use crate::immich_api::ImmichClient;
use crate::web::state::AppState;
use axum::{
    body::Body,
    extract::{Path, State},
    http::{header, StatusCode},
    response::{Json, Response},
};
use serde::Serialize;

/// Basic album info for listing.
#[derive(Serialize)]
pub struct AlbumInfo {
    pub id: String,
    pub name: String,
    pub asset_count: u32,
    pub thumbnail_asset_id: Option<String>,
}

/// Get list of albums from Immich.
pub async fn get_albums(
    State(state): State<AppState>,
) -> Result<Json<Vec<AlbumInfo>>, (StatusCode, String)> {
    let config = state.config.read().await;
    let client = ImmichClient::new(&config.api).map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Failed to create client: {}", e),
        )
    })?;

    let albums = client
        .get_albums()
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let album_info: Vec<AlbumInfo> = albums
        .into_iter()
        .map(|a| AlbumInfo {
            id: a.id,
            name: a.album_name,
            asset_count: a.asset_count,
            thumbnail_asset_id: a.album_thumbnail_asset_id,
        })
        .collect();

    Ok(Json(album_info))
}

/// Get an album's thumbnail image by its thumbnail asset ID.
pub async fn get_album_thumbnail(
    State(state): State<AppState>,
    Path(thumbnail_asset_id): Path<String>,
) -> Result<Response<Body>, (StatusCode, String)> {
    let config = state.config.read().await;
    let client = ImmichClient::new(&config.api).map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Failed to create client: {}", e),
        )
    })?;

    let (bytes, content_type) = client
        .get_album_thumbnail(&thumbnail_asset_id)
        .await
        .map_err(|e| {
            tracing::error!(
                "Album thumbnail fetch failed for asset {}: {}",
                thumbnail_asset_id,
                e
            );
            (
                StatusCode::NOT_FOUND,
                format!("Failed to get thumbnail: {}", e),
            )
        })?;

    Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, content_type)
        .header(header::CACHE_CONTROL, "public, max-age=3600")
        .body(Body::from(bytes))
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Failed to build response: {}", e),
            )
        })
}
