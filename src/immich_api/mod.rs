//! Immich API client implementation.

use crate::config::ApiConfig;
use crate::error::{Error, Result};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::time::Duration;

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

/// Asset data from Immich. May contain one or more person inside. Obtained from /search/metadata Immich endpoint.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Asset {
    pub id: String,
    pub device_asset_id: Option<String>,
    pub original_file_name: Option<String>,
    pub file_created_at: Option<String>,
    pub local_date_time: Option<String>,
    pub people: Option<Vec<PersonWithFaces>>,
}

/// Person with face data.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PersonWithFaces {
    pub id: String,
    pub name: Option<String>,
    pub faces: Option<Vec<FaceData>>,
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
    pub taken_after: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub taken_before: Option<String>,
    #[serde(rename = "type")]
    pub asset_type: String,
    pub page: u32,
    pub size: u32,
    pub with_people: bool,
}

impl ImmichClient {
    /// Create a new Immich client.
    pub fn new(config: &ApiConfig) -> Result<Self> {
        let client = Client::builder()
            .timeout(Duration::from_secs(config.timeout_secs))
            .build()?;

        Ok(Self {
            client,
            base_url: config.base_url.trim_end_matches('/').to_string(),
            api_key: config.api_key.clone(),
        })
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

        if response.status() == reqwest::StatusCode::UNAUTHORIZED {
            return Err(Error::ImmichApi("Invalid API key".to_string()));
        }

        if !response.status().is_success() {
            return Err(Error::ImmichApi(format!(
                "Server returned status {}",
                response.status()
            )));
        }

        let info: ServerInfo = response.json().await?;
        Ok(info)
    }

    /// Search for assets containing a specific person.
    pub async fn get_assets_with_person(
        &self,
        person_id: &str,
        taken_after: Option<&str>,
        taken_before: Option<&str>,
    ) -> Result<Vec<Asset>> {
        let mut all_assets = Vec::new();
        let mut page = 1u32;

        loop {
            let params: SearchParams = SearchParams {
                person_ids: Some(vec![person_id.to_string()]),
                taken_after: taken_after.map(|s| s.to_string()),
                taken_before: taken_before.map(|s| s.to_string()),
                asset_type: "IMAGE".to_string(),
                page,
                size: 100,
                with_people: true,
            };

            let url = format!("{}/search/metadata", self.base_url);
            let response = self
                .client
                .post(&url)
                .header("x-api-key", &self.api_key)
                .json(&params)
                .send()
                .await?;

            if !response.status().is_success() {
                let status = response.status();
                let body = response.text().await.unwrap_or_default();
                return Err(Error::ImmichApi(format!(
                    "Search failed with status {}: {}",
                    status, body
                )));
            }

            let search_response: SearchResponse = response.json().await?;
            all_assets.extend(search_response.assets.items);

            if search_response.assets.next_page.is_none() {
                break;
            }
            page += 1;
        }

        tracing::info!("Found {} assets for person {}", all_assets.len(), person_id);
        Ok(all_assets)
    }

    /// Download an asset's original image.
    pub async fn download_asset(&self, asset_id: &str) -> Result<bytes::Bytes> {
        let url = format!("{}/assets/{}/original", self.base_url, asset_id);

        let response = self
            .client
            .get(&url)
            .header("x-api-key", &self.api_key)
            .send()
            .await?;

        if !response.status().is_success() {
            return Err(Error::ImmichApi(format!(
                "Failed to download asset {}: {}",
                asset_id,
                response.status()
            )));
        }

        let bytes = response.bytes().await?;
        Ok(bytes)
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

        if !response.status().is_success() {
            return Err(Error::ImmichApi(format!(
                "Failed to get people: {}",
                response.status()
            )));
        }

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
        let url = format!("{}/people/{}/thumbnail", self.base_url, person_id);
        // tracing::debug!("Fetching thumbnail from: {}", url);

        let response = self
            .client
            .get(&url)
            .header("x-api-key", &self.api_key)
            .send()
            .await?;

        if !response.status().is_success() {
            return Err(Error::ImmichApi(format!(
                "Failed to get thumbnail for person {}: {}",
                person_id,
                response.status()
            )));
        }

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
    fn test_client_creation() {
        let config = ApiConfig {
            api_key: "test-key".to_string(),
            base_url: "http://localhost:2283/api/".to_string(),
            timeout_secs: 30,
        };
        let client = ImmichClient::new(&config);
        assert!(client.is_ok());
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

        let result = client.get_assets_with_person(&person.id, None, None).await;

        match &result {
            Ok(assets) => {
                println!("Found {} assets for person", assets.len());
                for asset in assets.iter().take(5) {
                    println!("  - {} (created: {:?})", asset.id, asset.file_created_at);
                    if let Some(people) = &asset.people {
                        for p in people {
                            if let Some(faces) = &p.faces {
                                println!("    Faces: {}", faces.len());
                            }
                        }
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
