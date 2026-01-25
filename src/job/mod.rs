//! Background job processing module.
//!
//! Handles the complete pipeline:
//! 1. Fetching assets from Immich for a specific person
//! 2. Downloading and processing images in parallel
//! 3. Cropping faces using bounding boxes
//! 4. Compiling processed images into a timelapse video

use crate::config::Config;
use crate::error::{Error, Result};
use crate::face_processing::{AssetResult, BoundingBox, ProcessedFace};
use crate::immich_api::{Asset, FaceData, ImmichClient};
use crate::video::compile_timelapse;
use crate::web::{AppState, JobStatus, Progress};

use image::imageops::FilterType;
use image::{DynamicImage, GenericImageView, ImageFormat};
use std::io::Cursor;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;
use tokio::sync::{oneshot, Semaphore};

/// Parameters for starting a processing job.
#[derive(Debug, Clone)]
pub struct JobParams {
    pub person_id: String,
    pub date_from: Option<String>,
    pub date_to: Option<String>,
}

/// Run the complete processing pipeline.
///
/// This is the main entry point for background job processing.
/// It handles progress reporting and cancellation.
pub async fn run_job(state: AppState, params: JobParams, cancel_rx: oneshot::Receiver<()>) {
    let result = run_job_inner(state.clone(), params, cancel_rx).await;

    match result {
        Ok(output_path) => {
            state
                .update_progress(Progress {
                    status: JobStatus::Completed,
                    completed: 0,
                    total: 0,
                    message: Some(format!("Video saved to: {}", output_path.display())),
                })
                .await;
        }
        Err(Error::Cancelled) => {
            state
                .update_progress(Progress {
                    status: JobStatus::Cancelled,
                    completed: 0,
                    total: 0,
                    message: Some("Job cancelled by user".to_string()),
                })
                .await;
        }
        Err(e) => {
            tracing::error!("Job failed: {}", e);
            state
                .update_progress(Progress {
                    status: JobStatus::Error(e.to_string()),
                    completed: 0,
                    total: 0,
                    message: Some(e.to_string()),
                })
                .await;
        }
    }
}

/// Inner implementation that returns Result for easier error handling.
async fn run_job_inner(
    state: AppState,
    params: JobParams,
    mut cancel_rx: oneshot::Receiver<()>,
) -> Result<PathBuf> {
    let config = state.config.read().await.clone();

    // Create output directories
    let images_dir = config.output_dir.join("images");
    tokio::fs::create_dir_all(&images_dir).await?;

    // Create Immich client
    let client = ImmichClient::new(&config.api)?;

    // Update progress: fetching assets
    state
        .update_progress(Progress {
            status: JobStatus::Running,
            completed: 0,
            total: 0,
            message: Some("Fetching assets from Immich...".to_string()),
        })
        .await;

    // Check for cancellation
    if cancel_rx.try_recv().is_ok() {
        return Err(Error::Cancelled);
    }

    // Fetch all assets for this person
    let assets = client
        .get_assets_with_person(
            &params.person_id,
            params.date_from.as_deref(),
            params.date_to.as_deref(),
        )
        .await?;

    if assets.is_empty() {
        return Err(Error::ImageProcessing(
            "No assets found for this person".to_string(),
        ));
    }

    tracing::info!("Found {} assets to process", assets.len());

    // Filter assets that have face data for the target person
    let assets_with_faces: Vec<(Asset, FaceData)> = assets
        .into_iter()
        .filter_map(|asset| {
            let face = find_face_for_person(&asset, &params.person_id)?;
            Some((asset, face))
        })
        .collect();

    if assets_with_faces.is_empty() {
        return Err(Error::ImageProcessing(
            "No assets with face data found".to_string(),
        ));
    }

    let total = assets_with_faces.len() as u32;
    tracing::info!("{} assets have face data", total);

    // Update progress with total count
    state
        .update_progress(Progress {
            status: JobStatus::Running,
            completed: 0,
            total,
            message: Some(format!("Processing {} images...", total)),
        })
        .await;

    // Process images in parallel with concurrency limit
    let completed = Arc::new(AtomicU32::new(0));
    let semaphore = Arc::new(Semaphore::new(config.processing.max_workers));
    let client = Arc::new(client);
    let config = Arc::new(config);
    let images_dir = Arc::new(images_dir);

    let mut handles = Vec::with_capacity(assets_with_faces.len());

    for (asset, face_data) in assets_with_faces {
        let permit = semaphore.clone().acquire_owned().await.unwrap();
        let client = client.clone();
        let config = config.clone();
        let images_dir = images_dir.clone();
        let completed = completed.clone();
        let state = state.clone();

        let handle = tokio::spawn(async move {
            let result = process_single_asset(&client, &config, &asset, &face_data, &images_dir).await;

            // Update progress
            let done = completed.fetch_add(1, Ordering::SeqCst) + 1;
            state
                .update_progress(Progress {
                    status: JobStatus::Running,
                    completed: done,
                    total,
                    message: Some(format!("Processing images... ({}/{})", done, total)),
                })
                .await;

            drop(permit);
            result
        });

        handles.push(handle);
    }

    // Check for cancellation periodically while waiting
    let mut results = Vec::with_capacity(handles.len());
    for handle in handles {
        tokio::select! {
            _ = &mut cancel_rx => {
                return Err(Error::Cancelled);
            }
            result = handle => {
                results.push(result.unwrap_or(AssetResult::Error {
                    asset_id: "unknown".to_string(),
                    error: "Task panicked".to_string(),
                }));
            }
        }
    }

    // Count results
    let successful = results
        .iter()
        .filter(|r| matches!(r, AssetResult::Success(_)))
        .count();
    let skipped = results
        .iter()
        .filter(|r| matches!(r, AssetResult::Skipped { .. }))
        .count();
    let errors = results
        .iter()
        .filter(|r| matches!(r, AssetResult::Error { .. }))
        .count();

    tracing::info!(
        "Processing complete: {} successful, {} skipped, {} errors",
        successful,
        skipped,
        errors
    );

    if successful == 0 {
        return Err(Error::ImageProcessing(
            "No images were successfully processed".to_string(),
        ));
    }

    // Check for cancellation before video compilation
    if cancel_rx.try_recv().is_ok() {
        return Err(Error::Cancelled);
    }

    // Compile video if enabled
    if config.video.enabled {
        state
            .update_progress(Progress {
                status: JobStatus::CompilingVideo,
                completed: 0,
                total: successful as u32,
                message: Some("Compiling video...".to_string()),
            })
            .await;

        let output_video = config.output_dir.join("timelapse.mp4");
        let state_clone = state.clone();

        compile_timelapse(&images_dir, &output_video, &config.video, move |frame, total| {
            // Note: This callback is sync, so we can't easily update state here.
            // The FFmpeg wrapper already handles this internally.
            tracing::debug!("Video progress: {}/{}", frame, total);
            let _ = (&state_clone, frame, total); // Suppress unused warning
        })
        .await?;

        Ok(output_video)
    } else {
        Ok(images_dir.to_path_buf())
    }
}

/// Find the face data for a specific person in an asset.
fn find_face_for_person(asset: &Asset, person_id: &str) -> Option<FaceData> {
    let people = asset.people.as_ref()?;

    for person in people {
        if person.id == person_id {
            if let Some(faces) = &person.faces {
                // Return the first face (usually there's only one per person per image)
                return faces.first().cloned();
            }
        }
    }

    None
}

/// Process a single asset: download, crop face, save.
async fn process_single_asset(
    client: &ImmichClient,
    config: &Config,
    asset: &Asset,
    face_data: &FaceData,
    output_dir: &Path,
) -> AssetResult {
    let asset_id = &asset.id;

    // Check face resolution
    let bbox = BoundingBox {
        x1: face_data.bounding_box_x1,
        y1: face_data.bounding_box_y1,
        x2: face_data.bounding_box_x2,
        y2: face_data.bounding_box_y2,
    };

    let face_width = bbox.width() * face_data.image_width as f32;
    let face_height = bbox.height() * face_data.image_height as f32;
    let face_size = face_width.min(face_height) as u32;

    if face_size < config.processing.face_resolution_threshold {
        return AssetResult::Skipped {
            asset_id: asset_id.clone(),
            reason: format!(
                "Face too small: {}px (threshold: {}px)",
                face_size, config.processing.face_resolution_threshold
            ),
        };
    }

    // Download image
    let image_bytes = match client.download_asset(asset_id).await {
        Ok(bytes) => bytes,
        Err(e) => {
            return AssetResult::Error {
                asset_id: asset_id.clone(),
                error: format!("Download failed: {}", e),
            };
        }
    };

    // Decode image
    let img = match image::load_from_memory(&image_bytes) {
        Ok(img) => img,
        Err(e) => {
            return AssetResult::Error {
                asset_id: asset_id.clone(),
                error: format!("Failed to decode image: {}", e),
            };
        }
    };

    // Crop and resize face
    let cropped = match crop_face(&img, face_data, config.processing.resize_size) {
        Ok(cropped) => cropped,
        Err(e) => {
            return AssetResult::Error {
                asset_id: asset_id.clone(),
                error: format!("Failed to crop face: {}", e),
            };
        }
    };

    // Generate timestamp-based filename for sorting
    let timestamp = asset
        .file_created_at
        .as_ref()
        .or(asset.local_date_time.as_ref())
        .cloned()
        .unwrap_or_else(|| asset_id.clone());

    // Sanitize timestamp for filename (replace colons and other invalid chars)
    let safe_timestamp: String = timestamp
        .chars()
        .map(|c| if c.is_alphanumeric() || c == '-' || c == '_' { c } else { '_' })
        .collect();

    let output_path = output_dir.join(format!("{}_{}.jpg", safe_timestamp, asset_id));

    // Encode and save
    let mut buffer = Cursor::new(Vec::new());
    if let Err(e) = cropped.write_to(&mut buffer, ImageFormat::Jpeg) {
        return AssetResult::Error {
            asset_id: asset_id.clone(),
            error: format!("Failed to encode image: {}", e),
        };
    }

    if let Err(e) = tokio::fs::write(&output_path, buffer.into_inner()).await {
        return AssetResult::Error {
            asset_id: asset_id.clone(),
            error: format!("Failed to save image: {}", e),
        };
    }

    AssetResult::Success(ProcessedFace {
        image_data: Vec::new(), // We saved to file, so don't keep in memory
        asset_id: asset_id.clone(),
        timestamp,
    })
}

/// Crop and resize the face from an image using bounding box.
///
/// This is a simplified version that just uses the bounding box.
/// A full implementation would use facial landmarks for alignment.
fn crop_face(img: &DynamicImage, face_data: &FaceData, output_size: u32) -> Result<DynamicImage> {
    let (img_width, img_height) = img.dimensions();

    // Convert normalized bounding box to pixel coordinates
    let x1 = (face_data.bounding_box_x1 * img_width as f32) as u32;
    let y1 = (face_data.bounding_box_y1 * img_height as f32) as u32;
    let x2 = (face_data.bounding_box_x2 * img_width as f32) as u32;
    let y2 = (face_data.bounding_box_y2 * img_height as f32) as u32;

    let face_width = x2.saturating_sub(x1);
    let face_height = y2.saturating_sub(y1);

    if face_width == 0 || face_height == 0 {
        return Err(Error::ImageProcessing("Invalid face bounding box".to_string()));
    }

    // Expand the crop area to include some context around the face
    // and make it square for consistent output
    let face_size = face_width.max(face_height);
    let padding = face_size / 2; // 50% padding on each side
    let crop_size = face_size + padding * 2;

    // Calculate center of face
    let center_x = (x1 + x2) / 2;
    let center_y = (y1 + y2) / 2;

    // Calculate crop bounds, clamped to image dimensions
    let crop_x1 = center_x.saturating_sub(crop_size / 2).min(img_width.saturating_sub(crop_size));
    let crop_y1 = center_y.saturating_sub(crop_size / 2).min(img_height.saturating_sub(crop_size));

    // Ensure we don't exceed image bounds
    let actual_crop_size = crop_size.min(img_width - crop_x1).min(img_height - crop_y1);

    if actual_crop_size < 10 {
        return Err(Error::ImageProcessing("Crop area too small".to_string()));
    }

    // Crop the face region
    let cropped = img.crop_imm(crop_x1, crop_y1, actual_crop_size, actual_crop_size);

    // Resize to output size
    let resized = cropped.resize_exact(output_size, output_size, FilterType::Lanczos3);

    Ok(resized)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_find_face_for_person() {
        use crate::immich_api::PersonWithFaces;

        let asset = Asset {
            id: "asset1".to_string(),
            device_asset_id: None,
            original_file_name: None,
            file_created_at: Some("2024-01-15T10:30:00Z".to_string()),
            local_date_time: None,
            people: Some(vec![PersonWithFaces {
                id: "person1".to_string(),
                name: Some("Test Person".to_string()),
                faces: Some(vec![FaceData {
                    bounding_box_x1: 0.2,
                    bounding_box_y1: 0.1,
                    bounding_box_x2: 0.5,
                    bounding_box_y2: 0.6,
                    image_width: 1920,
                    image_height: 1080,
                }]),
            }]),
        };

        let face = find_face_for_person(&asset, "person1");
        assert!(face.is_some());

        let face = find_face_for_person(&asset, "person2");
        assert!(face.is_none());
    }
}
