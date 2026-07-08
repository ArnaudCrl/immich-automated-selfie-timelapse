//! Immich API client implementation.

use crate::config::ApiConfig;
use crate::error::{Error, Result};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::time::Duration;
use urlencoding::encode as urlencode;

/// Client for interacting with the Immich API.
#[derive(Clone)]
pub struct ImmichClient {
    client: Client,
    base_url: String,
    api_key: String,
}

/// Server information response.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ServerInfo {
    pub version: String,
}

/// Person data. Obtained from the /people Immich endpoint.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Person {
    pub id: String,
    pub name: Option<String>,
}

/// Album data. Obtained from the /albums Immich endpoint.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Album {
    pub id: String,
    pub album_name: String,
    pub asset_count: u32,
    pub album_thumbnail_asset_id: Option<String>,
}

/// Asset data from Immich. Obtained from /search/metadata Immich endpoint.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Asset {
    pub id: String,
    pub device_asset_id: Option<String>,
    pub original_file_name: Option<String>,
    pub file_created_at: Option<String>,
    pub local_date_time: Option<String>,
}

/// Face data from Immich.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FaceData {
    pub bounding_box_x1: f32,
    pub bounding_box_y1: f32,
    pub bounding_box_x2: f32,
    pub bounding_box_y2: f32,
    pub image_width: u32,
    pub image_height: u32,
}

/// A single detected face, as returned by the `/faces?id=<assetId>` endpoint.
/// One entry per person detected in the asset.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AssetFace {
    #[serde(flatten)]
    pub data: FaceData,
    pub person: Option<FacePerson>,
}

/// Minimal person reference embedded in a face record.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FacePerson {
    pub id: String,
}

/// Search response from Immich.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchResponse {
    pub assets: SearchAssets,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchAssets {
    pub items: Vec<Asset>,
    pub next_page: Option<String>,
}

/// Search parameters.
#[derive(Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchParams {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub person_ids: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub album_ids: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub taken_after: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub taken_before: Option<String>,
    #[serde(rename = "type")]
    pub asset_type: String,
    pub page: u32,
    pub size: u32,
}

impl ImmichClient {
    /// Create a new Immich client.
    pub fn new(config: &ApiConfig) -> Result<Self> {
        let client = Client::builder()
            .timeout(Duration::from_secs(config.timeout_secs))
            .build()?;

        Ok(Self {
            client,
            base_url: Self::sanitize_base_url(&config.base_url),
            api_key: config.api_key.clone(),
        })
    }

    /// Check an HTTP response for authentication/permission errors and return
    /// a descriptive error. Call this before checking `is_success()`.
    async fn check_response_error(
        response: reqwest::Response,
        operation: &str,
    ) -> std::result::Result<reqwest::Response, Error> {
        let status = response.status();
        if status.is_success() {
            return Ok(response);
        }

        let body = response.text().await.unwrap_or_default();

        match status {
            reqwest::StatusCode::UNAUTHORIZED => Err(Error::ImmichApi(format!(
                "{operation}: invalid or expired API key (401)"
            ))),
            reqwest::StatusCode::FORBIDDEN => Err(Error::ImmichApi(format!(
                "{operation}: missing permission (403). {body}"
            ))),
            _ => Err(Error::ImmichApi(format!(
                "{operation}: HTTP {status}. {body}"
            ))),
        }
    }

    /// Sanitize the base URL to ensure it ends with /api.
    fn sanitize_base_url(url: &str) -> String {
        let trimmed = url.trim_end_matches('/');

        if trimmed.ends_with("/api") {
            trimmed.to_string()
        } else {
            format!("{}/api", trimmed)
        }
    }

    /// Validate the connection to Immich.
    pub async fn validate_connection(&self) -> Result<ServerInfo> {
        let url = format!("{}/server/about", self.base_url);

        let response = self
            .client
            .get(&url)
            .header("x-api-key", &self.api_key)
            .send()
            .await?;

        let response = Self::check_response_error(response, "Validate connection").await?;

        let info: ServerInfo = response.json().await?;
        Ok(info)
    }

    /// Search for assets containing a specific person, optionally filtered by albums.
    ///
    /// When multiple albums are provided, fetches each album separately and merges
    /// the results by asset ID.
    pub async fn get_assets_with_person(
        &self,
        person_id: &str,
        taken_after: Option<&str>,
        taken_before: Option<&str>,
        album_ids: &[String],
    ) -> Result<Vec<Asset>> {
        // Immich requires full ISO 8601 datetimes, but callers pass plain
        // `YYYY-MM-DD` dates (e.g. from an HTML date input), so widen them
        // to cover the whole day.
        let taken_after = taken_after.map(|d| format!("{d}T00:00:00.000Z"));
        let taken_before = taken_before.map(|d| format!("{d}T23:59:59.999Z"));
        let taken_after = taken_after.as_deref();
        let taken_before = taken_before.as_deref();

        if album_ids.len() <= 1 {
            // 0 or 1 album: single request
            let album_id = album_ids.first().map(|s| s.as_str());
            self.search_person_assets(person_id, taken_after, taken_before, album_id)
                .await
        } else {
            // Multiple albums: fetch per album and merge by asset ID
            use std::collections::HashSet;
            let mut all_assets: Vec<Asset> = Vec::new();
            let mut seen_ids: HashSet<String> = HashSet::new();

            for album_id in album_ids {
                let assets = self
                    .search_person_assets(
                        person_id,
                        taken_after,
                        taken_before,
                        Some(album_id.as_str()),
                    )
                    .await?;
                for asset in assets {
                    if seen_ids.insert(asset.id.clone()) {
                        all_assets.push(asset);
                    }
                }
            }

            tracing::debug!(
                "Found {} merged assets for person {} across {} albums",
                all_assets.len(),
                person_id,
                album_ids.len()
            );
            Ok(all_assets)
        }
    }

    /// Fetch all paginated assets for a person, with an optional single album filter.
    async fn search_person_assets(
        &self,
        person_id: &str,
        taken_after: Option<&str>,
        taken_before: Option<&str>,
        album_id: Option<&str>,
    ) -> Result<Vec<Asset>> {
        let mut all_assets = Vec::new();
        let mut page = 1u32;

        loop {
            let params = SearchParams {
                person_ids: Some(vec![person_id.to_string()]),
                album_ids: album_id.map(|id| vec![id.to_string()]),
                taken_after: taken_after.map(|s| s.to_string()),
                taken_before: taken_before.map(|s| s.to_string()),
                asset_type: "IMAGE".to_string(),
                page,
                size: 100,
            };

            let url = format!("{}/search/metadata", self.base_url);
            let response = self
                .client
                .post(&url)
                .header("x-api-key", &self.api_key)
                .json(&params)
                .send()
                .await?;

            let response = Self::check_response_error(response, "Search assets by person").await?;

            let search_response: SearchResponse = response.json().await?;
            all_assets.extend(search_response.assets.items);

            if search_response.assets.next_page.is_none() {
                break;
            }
            page += 1;
            if page > 1000 {
                tracing::warn!("Pagination limit reached (1000 pages), stopping asset fetch");
                break;
            }
        }

        tracing::debug!("Found {} assets for person {}", all_assets.len(), person_id);
        Ok(all_assets)
    }

    /// Maximum download size for a single asset (100 MB).
    const MAX_DOWNLOAD_SIZE: u64 = 100 * 1024 * 1024;

    /// Download an asset's original image.
    pub async fn download_asset(&self, asset_id: &str) -> Result<bytes::Bytes> {
        let encoded_id = urlencode(asset_id);
        let url = format!("{}/assets/{}/original", self.base_url, encoded_id);

        let response = self
            .client
            .get(&url)
            .header("x-api-key", &self.api_key)
            .send()
            .await?;

        let response = Self::check_response_error(response, "Download asset").await?;

        // Check Content-Length before downloading to reject unexpectedly large files
        if let Some(content_length) = response.content_length() {
            if content_length > Self::MAX_DOWNLOAD_SIZE {
                return Err(Error::ImmichApi(format!(
                    "Asset {} is too large ({} bytes, max {} bytes)",
                    asset_id,
                    content_length,
                    Self::MAX_DOWNLOAD_SIZE
                )));
            }
        }

        let bytes = response.bytes().await?;
        Ok(bytes)
    }

    /// Download an asset's preview image (1440p JPEG pre-generated by Immich).
    ///
    /// Much faster to download and decode than the original.
    /// Falls back to the original if the preview is unavailable.
    pub async fn download_asset_preview(&self, asset_id: &str) -> Result<bytes::Bytes> {
        let encoded_id = urlencode(asset_id);
        let url = format!(
            "{}/assets/{}/thumbnail?size=preview",
            self.base_url, encoded_id
        );

        let response = self
            .client
            .get(&url)
            .header("x-api-key", &self.api_key)
            .send()
            .await?;

        let response = Self::check_response_error(response, "Download asset preview").await?;

        let bytes = response.bytes().await?;
        Ok(bytes)
    }

    /// Get all detected faces for an asset (one entry per person present in the photo).
    pub async fn get_faces(&self, asset_id: &str) -> Result<Vec<AssetFace>> {
        let encoded_id = urlencode(asset_id);
        let url = format!("{}/faces?id={}", self.base_url, encoded_id);

        let response = self
            .client
            .get(&url)
            .header("x-api-key", &self.api_key)
            .send()
            .await?;

        let response = Self::check_response_error(response, "Get faces").await?;
        let faces: Vec<AssetFace> = response.json().await?;
        Ok(faces)
    }

    /// Get the face bounding box for a specific person on an asset, if present.
    pub async fn get_face_for_person(
        &self,
        asset_id: &str,
        person_id: &str,
    ) -> Result<Option<FaceData>> {
        let faces = self.get_faces(asset_id).await?;
        Ok(faces
            .into_iter()
            .find(|f| f.person.as_ref().is_some_and(|p| p.id == person_id))
            .map(|f| f.data))
    }

    /// Get all albums from Immich.
    pub async fn get_albums(&self) -> Result<Vec<Album>> {
        let url = format!("{}/albums", self.base_url);

        let response = self
            .client
            .get(&url)
            .header("x-api-key", &self.api_key)
            .send()
            .await?;

        let response = Self::check_response_error(response, "Get albums").await?;
        let albums: Vec<Album> = response.json().await?;
        Ok(albums)
    }

    /// Get an album thumbnail image by asset ID.
    /// Returns the image bytes and content-type.
    pub async fn get_album_thumbnail(&self, asset_id: &str) -> Result<(bytes::Bytes, String)> {
        let encoded_id = urlencode(asset_id);
        let url = format!(
            "{}/assets/{}/thumbnail?size=preview",
            self.base_url, encoded_id
        );

        let response = self
            .client
            .get(&url)
            .header("x-api-key", &self.api_key)
            .send()
            .await?;

        let response = Self::check_response_error(response, "Get album thumbnail").await?;

        let content_type = response
            .headers()
            .get("content-type")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("image/jpeg")
            .to_string();

        let bytes = response.bytes().await?;
        Ok((bytes, content_type))
    }

    /// Get all people from Immich.
    pub async fn get_people(&self) -> Result<Vec<Person>> {
        let url = format!("{}/people", self.base_url);

        let response = self
            .client
            .get(&url)
            .header("x-api-key", &self.api_key)
            .send()
            .await?;

        let response = Self::check_response_error(response, "Get people").await?;

        #[derive(Deserialize)]
        struct PeopleResponse {
            people: Vec<Person>,
        }

        let response: PeopleResponse = response.json().await?;
        Ok(response.people)
    }

    /// Get a person's thumbnail image.
    /// Returns the image bytes and content-type.
    pub async fn get_person_thumbnail(&self, person_id: &str) -> Result<(bytes::Bytes, String)> {
        let encoded_id = urlencode(person_id);
        let url = format!("{}/people/{}/thumbnail", self.base_url, encoded_id);
        // tracing::debug!("Fetching thumbnail from: {}", url);

        let response = self
            .client
            .get(&url)
            .header("x-api-key", &self.api_key)
            .send()
            .await?;

        let response = Self::check_response_error(response, "Get person thumbnail").await?;

        let content_type = response
            .headers()
            .get("content-type")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("image/jpeg")
            .to_string();

        let bytes = response.bytes().await?;
        Ok((bytes, content_type))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sanitize_base_url() {
        // URL already ends with /api
        assert_eq!(
            ImmichClient::sanitize_base_url("http://localhost:2283/api"),
            "http://localhost:2283/api"
        );

        // URL ends with /api/ (trailing slash)
        assert_eq!(
            ImmichClient::sanitize_base_url("http://localhost:2283/api/"),
            "http://localhost:2283/api"
        );

        // URL without /api
        assert_eq!(
            ImmichClient::sanitize_base_url("http://localhost:2283"),
            "http://localhost:2283/api"
        );

        // URL with trailing slash, no /api
        assert_eq!(
            ImmichClient::sanitize_base_url("http://localhost:2283/"),
            "http://localhost:2283/api"
        );

        // URL with path but no /api
        assert_eq!(
            ImmichClient::sanitize_base_url("http://example.com/immich"),
            "http://example.com/immich/api"
        );

        // URL with path and trailing slash
        assert_eq!(
            ImmichClient::sanitize_base_url("http://example.com/immich/"),
            "http://example.com/immich/api"
        );
    }

    #[test]
    fn test_client_creation() {
        let config = ApiConfig {
            api_key: "test-key".to_string(),
            base_url: "http://localhost:2283/api/".to_string(),
            timeout_secs: 30,
        };
        let client = ImmichClient::new(&config);
        assert!(client.is_ok());
    }

    #[test]
    fn test_client_creation_without_api_suffix() {
        // URL without /api should be sanitized automatically
        let config = ApiConfig {
            api_key: "test-key".to_string(),
            base_url: "http://localhost:2283".to_string(),
            timeout_secs: 30,
        };
        let client = ImmichClient::new(&config).unwrap();
        assert_eq!(client.base_url, "http://localhost:2283/api");
    }

    /// Helper to create a client for the demo server
    fn demo_client() -> ImmichClient {
        let config = ApiConfig {
            api_key: "1bpgd3LpG30Zr3IEPNV3sWhIEqMuUGmzK3jWNh59JU".to_string(), // Generate a new API key in the public Immich demo for debugging purposes.
            base_url: "https://demo.immich.app/api".to_string(),
            timeout_secs: 30,
        };
        ImmichClient::new(&config).expect("Failed to create client")
    }

    #[tokio::test]
    #[ignore] // Run with: cargo test -- --ignored
    async fn test_demo_validate_connection() {
        let client = demo_client();
        let result = client.validate_connection().await;

        match &result {
            Ok(info) => println!("Connected to Immich version: {}", info.version),
            Err(e) => println!("Connection failed: {:?}", e),
        }

        assert!(result.is_ok(), "Failed to connect: {:?}", result.err());
    }

    #[tokio::test]
    #[ignore]
    async fn test_demo_get_people() {
        let client = demo_client();
        let result = client.get_people().await;

        match &result {
            Ok(people) => {
                println!("Found {} people:", people.len());
                for person in people.iter().take(10) {
                    println!(
                        "  - {} ({})",
                        person.name.as_deref().unwrap_or("unnamed"),
                        person.id
                    );
                }
            }
            Err(e) => println!("Failed to get people: {:?}", e),
        }

        assert!(result.is_ok(), "Failed to get people: {:?}", result.err());
    }

    #[tokio::test]
    #[ignore]
    async fn test_demo_search_assets_with_person() {
        let client = demo_client();

        // First get a person ID
        let people = client.get_people().await.expect("Failed to get people");

        if people.is_empty() {
            println!("No people found, skipping asset search test");
            return;
        }

        let person = &people[0];
        println!(
            "Searching assets for: {} ({})",
            person.name.as_deref().unwrap_or("unnamed"),
            person.id
        );

        let result = client
            .get_assets_with_person(&person.id, None, None, &[])
            .await;

        match &result {
            Ok(assets) => {
                println!("Found {} assets for person", assets.len());
                for asset in assets.iter().take(5) {
                    println!("  - {} (created: {:?})", asset.id, asset.file_created_at);
                    if let Ok(faces) = client.get_faces(&asset.id).await {
                        println!("    Faces: {}", faces.len());
                    }
                }
            }
            Err(e) => println!("Failed to search assets: {:?}", e),
        }

        assert!(
            result.is_ok(),
            "Failed to search assets: {:?}",
            result.err()
        );
    }
}
