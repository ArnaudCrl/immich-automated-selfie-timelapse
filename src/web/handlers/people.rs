//! People-related endpoints (list, thumbnails, asset counts).

use crate::immich_api::ImmichClient;
use crate::web::state::AppState;
use axum::{
    body::Body,
    extract::{Path, State},
    http::{header, StatusCode},
    response::{Json, Response},
};
use serde::Serialize;

/// Basic person info for listing.
#[derive(Serialize)]
pub struct PersonInfo {
    pub id: String,
    pub name: Option<String>,
}

/// Get list of people from Immich.
pub async fn get_people(
    State(state): State<AppState>,
) -> Result<Json<Vec<PersonInfo>>, (StatusCode, String)> {
    let config = state.config.read().await;
    let client = ImmichClient::new(&config.api).map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Failed to create client: {}", e),
        )
    })?;

    let people = client.get_people().await.map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Failed to get people: {}", e),
        )
    })?;

    let people_info: Vec<PersonInfo> = people
        .into_iter()
        .map(|p| PersonInfo {
            id: p.id,
            name: p.name,
        })
        .collect();

    Ok(Json(people_info))
}

/// Get a person's thumbnail image.
pub async fn get_person_thumbnail(
    State(state): State<AppState>,
    Path(person_id): Path<String>,
) -> Result<Response<Body>, (StatusCode, String)> {
    let config = state.config.read().await;
    let client = ImmichClient::new(&config.api).map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Failed to create client: {}", e),
        )
    })?;

    let (bytes, content_type) = client.get_person_thumbnail(&person_id).await.map_err(|e| {
        tracing::error!("Thumbnail fetch failed for {}: {}", person_id, e);
        (
            StatusCode::NOT_FOUND,
            format!("Failed to get thumbnail: {}", e),
        )
    })?;

    // Return the image with appropriate headers
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

/// Asset count response for a person.
#[derive(Serialize)]
pub struct AssetCountResponse {
    pub total_assets: u32,
    pub assets_with_faces: u32,
}

/// Get the count of assets for a person.
pub async fn get_person_asset_count(
    State(state): State<AppState>,
    Path(person_id): Path<String>,
) -> Result<Json<AssetCountResponse>, (StatusCode, String)> {
    let config = state.config.read().await;
    let client = ImmichClient::new(&config.api).map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Failed to create client: {}", e),
        )
    })?;

    // Fetch assets for this person
    let assets = client
        .get_assets_with_person(&person_id, None, None)
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Failed to get assets: {}", e),
            )
        })?;

    let total_assets = assets.len() as u32;

    // Count assets that have face data for the target person
    let assets_with_faces = assets
        .iter()
        .filter(|asset| {
            asset.people.as_ref().is_some_and(|people| {
                people.iter().any(|p| {
                    p.id == person_id && p.faces.as_ref().is_some_and(|faces| !faces.is_empty())
                })
            })
        })
        .count() as u32;

    Ok(Json(AssetCountResponse {
        total_assets,
        assets_with_faces,
    }))
}
