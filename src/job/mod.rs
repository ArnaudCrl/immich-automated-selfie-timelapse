//! Background job processing module.
//!
//! Handles the complete pipeline:
//! 1. Fetching assets from Immich for a specific person
//! 2. Downloading and processing images in parallel
//! 3. Cropping faces using bounding boxes
//! 4. Compiling processed images into a timelapse video

use crate::config::Config;
use crate::error::{Error, Result};
use crate::face_processing::crop_face_with_intermediate;
use crate::face_processing::debug::draw_crop_debug;
use crate::face_processing::load_image_with_orientation;
use crate::face_processing::{AssetResult, ProcessedFace};
use crate::immich_api::{Asset, FaceData, ImmichClient};
use crate::video::compile_timelapse;
use crate::web::{AppState, JobStatus, Progress};

use image::ImageFormat;
use std::io::Cursor;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;
use tokio::sync::Semaphore;
use tokio_util::sync::CancellationToken;

/// Parameters for starting a processing job.
#[derive(Debug, Clone)]
pub struct JobParams {
    pub person_id: String,
    pub person_name: Option<String>,
    pub date_from: Option<String>,
    pub date_to: Option<String>,
}

/// Output directories for a processing job.
#[derive(Debug, Clone)]
struct OutputDirs {
    /// Directory for final processed images (used for video)
    images: PathBuf,
    /// Path for output video
    video: PathBuf,
    /// Optional debug directories for visualizing processing stages.
    debug: Option<DebugDirs>,
}

/// Debug output directories for visualizing each processing stage.
/// Each folder contains images with debug overlays showing what happened at that step.
///
/// Note: `landmarks` and `alignment` fields are placeholders for future features
/// (facial landmark detection and face alignment). They are intentionally unused
/// until those features are implemented.
#[derive(Debug, Clone, Default)]
struct DebugDirs {
    /// Original image with bounding box and crop region overlayed
    crop: Option<PathBuf>,
    /// Face with landmark points drawn (future: facial landmark detection)
    #[allow(dead_code)]
    landmarks: Option<PathBuf>,
    /// Before/after alignment visualization (future: face alignment)
    #[allow(dead_code)]
    alignment: Option<PathBuf>,
}

/// Create a safe folder name from person name or ID.
fn sanitize_folder_name(name: Option<&str>, id: &str) -> String {
    let base = name.filter(|n| !n.is_empty()).unwrap_or(id);

    // Replace unsafe characters with underscores
    let sanitized: String = base
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '-' || c == '_' || c == ' ' {
                c
            } else {
                '_'
            }
        })
        .collect();

    // Trim whitespace and limit length
    let trimmed = sanitized.trim();
    if trimmed.len() > 50 {
        trimmed[..50].to_string()
    } else {
        trimmed.to_string()
    }
}

/// Run the complete processing pipeline.
///
/// This is the main entry point for background job processing.
/// It handles progress reporting and cancellation.
pub async fn run_job(state: AppState, params: JobParams, cancel_token: CancellationToken) {
    let result = run_job_inner(state.clone(), params, cancel_token).await;

    // Clear the cancellation token when done
    state.clear_cancel_token().await;

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
    cancel_token: CancellationToken,
) -> Result<PathBuf> {
    let config = state.config.read().await.clone();

    // Create person-specific output directory
    let folder_name = sanitize_folder_name(params.person_name.as_deref(), &params.person_id);
    let person_dir = config.output_dir.join(&folder_name);

    // Create output directories
    let images_dir = person_dir.join("images");
    tokio::fs::create_dir_all(&images_dir).await?;

    // Create debug directories if enabled
    let debug = if config.processing.keep_intermediates {
        let debug_base = person_dir.join("debug");

        let crop_dir = debug_base.join("crop");
        tokio::fs::create_dir_all(&crop_dir).await?;

        // Future directories (created on-demand when those features are implemented):
        // - debug_base.join("landmarks") - face with landmark points
        // - debug_base.join("alignment") - before/after alignment

        Some(DebugDirs {
            crop: Some(crop_dir),
            landmarks: None,
            alignment: None,
        })
    } else {
        None
    };

    let output_dirs = OutputDirs {
        images: images_dir.clone(),
        video: person_dir.join("timelapse.mp4"),
        debug,
    };

    tracing::info!("Output directory: {}", person_dir.display());

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
    if cancel_token.is_cancelled() {
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
    let output_dirs = Arc::new(output_dirs);

    let mut handles = Vec::with_capacity(assets_with_faces.len());

    for (asset, face_data) in assets_with_faces {
        // Check for cancellation before spawning more tasks
        if cancel_token.is_cancelled() {
            break;
        }

        let permit = match semaphore.clone().acquire_owned().await {
            Ok(permit) => permit,
            Err(_) => continue, // Semaphore closed, skip remaining tasks
        };
        let client = client.clone();
        let config = config.clone();
        let output_dirs = output_dirs.clone();
        let completed = completed.clone();
        let state = state.clone();
        let task_cancel_token = cancel_token.clone();

        let handle = tokio::spawn(async move {
            // Check cancellation at start of task
            if task_cancel_token.is_cancelled() {
                drop(permit);
                return AssetResult::Skipped {
                    asset_id: asset.id.clone(),
                    reason: "Cancelled".to_string(),
                };
            }

            let result = process_single_asset(
                &client,
                &config,
                &asset,
                &face_data,
                &output_dirs,
                &task_cancel_token,
            )
            .await;

            // Update progress (only if not cancelled)
            if !task_cancel_token.is_cancelled() {
                let done = completed.fetch_add(1, Ordering::SeqCst) + 1;
                state
                    .update_progress(Progress {
                        status: JobStatus::Running,
                        completed: done,
                        total,
                        message: Some(format!("Processing images... ({}/{})", done, total)),
                    })
                    .await;
            }

            drop(permit);
            result
        });

        handles.push(handle);
    }

    // Wait for all tasks to complete
    let mut results = Vec::with_capacity(handles.len());
    for handle in handles {
        match handle.await {
            Ok(result) => results.push(result),
            Err(e) => {
                tracing::error!("Task panicked: {:?}", e);
            }
        }
    }

    // Check if we were cancelled
    if cancel_token.is_cancelled() {
        return Err(Error::Cancelled);
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
    if cancel_token.is_cancelled() {
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

        compile_timelapse(
            &output_dirs.images,
            &output_dirs.video,
            &config.video,
            |frame, total| {
                // Sync callback - just log progress. State updates would require
                // async context or channels, which adds complexity for minimal benefit
                // since video compilation is typically fast.
                tracing::debug!("Video progress: {}/{}", frame, total);
            },
        )
        .await?;

        Ok(output_dirs.video.clone())
    } else {
        Ok(output_dirs.images.clone())
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
    output_dirs: &OutputDirs,
    cancel_token: &CancellationToken,
) -> AssetResult {
    let asset_id = &asset.id;

    // Check face resolution (bounding box coordinates are already in pixels)
    let face_width = face_data.bounding_box_x2 - face_data.bounding_box_x1;
    let face_height = face_data.bounding_box_y2 - face_data.bounding_box_y1;
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

    // Check before download (potentially slow)
    if cancel_token.is_cancelled() {
        return AssetResult::Skipped {
            asset_id: asset_id.clone(),
            reason: "Cancelled".to_string(),
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
        .map(|c| {
            if c.is_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect();

    let filename = format!("{}_{}.jpg", safe_timestamp, asset_id);

    // Check after download, before CPU-intensive processing
    if cancel_token.is_cancelled() {
        return AssetResult::Skipped {
            asset_id: asset_id.clone(),
            reason: "Cancelled".to_string(),
        };
    }

    // Decode image and apply EXIF orientation correction.
    // This ensures bounding box coordinates from Immich align with the image pixels.
    let img = match load_image_with_orientation(&image_bytes) {
        Ok(img) => img,
        Err(e) => {
            return AssetResult::Error {
                asset_id: asset_id.clone(),
                error: format!("Failed to decode image: {}", e),
            };
        }
    };

    // Crop face
    let (_cropped_full, final_image) =
        match crop_face_with_intermediate(&img, face_data, config.processing.resize_size) {
            Ok(result) => result,
            Err(e) => {
                return AssetResult::Error {
                    asset_id: asset_id.clone(),
                    error: format!("Failed to crop face: {}", e),
                };
            }
        };

    // Save debug visualization if enabled
    if let Some(ref debug) = output_dirs.debug {
        if let Some(ref crop_dir) = debug.crop {
            let debug_img = draw_crop_debug(&img, face_data);
            let debug_path = crop_dir.join(&filename);
            let mut buffer = Cursor::new(Vec::new());
            if let Err(e) = debug_img.write_to(&mut buffer, ImageFormat::Jpeg) {
                tracing::warn!("Failed to encode debug image: {}", e);
            } else if let Err(e) = tokio::fs::write(&debug_path, buffer.into_inner()).await {
                tracing::warn!("Failed to save debug image: {}", e);
            }
        }
    }

    // Check before file I/O
    if cancel_token.is_cancelled() {
        return AssetResult::Skipped {
            asset_id: asset_id.clone(),
            reason: "Cancelled".to_string(),
        };
    }

    let output_path = output_dirs.images.join(&filename);

    // Encode and save final image
    let mut buffer = Cursor::new(Vec::new());
    if let Err(e) = final_image.write_to(&mut buffer, ImageFormat::Jpeg) {
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
