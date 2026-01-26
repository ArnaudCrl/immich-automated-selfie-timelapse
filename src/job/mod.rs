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
use image::{DynamicImage, GenericImageView, ImageFormat, Rgb};
use imageproc::drawing::{draw_hollow_rect_mut, draw_line_segment_mut};
use imageproc::rect::Rect;
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
#[derive(Debug, Clone, Default)]
#[allow(dead_code)]
struct DebugDirs {
    /// Original image with bounding box and crop region overlayed
    crop: Option<PathBuf>,
    /// Face with landmark points drawn (future)
    landmarks: Option<PathBuf>,
    /// Before/after alignment visualization (future)
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

        let permit = semaphore.clone().acquire_owned().await.unwrap();
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

        let state_clone = state.clone();

        compile_timelapse(
            &output_dirs.images,
            &output_dirs.video,
            &config.video,
            move |frame, total| {
                // Note: This callback is sync, so we can't easily update state here.
                // The FFmpeg wrapper already handles this internally.
                tracing::debug!("Video progress: {}/{}", frame, total);
                let _ = (&state_clone, frame, total); // Suppress unused warning
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

/// Crop and resize the face from an image using bounding box.
/// Returns (cropped_full_res, resized_final) for intermediate saving.
///
/// This is a simplified version that just uses the bounding box.
/// A full implementation would use facial landmarks for alignment.
fn crop_face_with_intermediate(
    img: &DynamicImage,
    face_data: &FaceData,
    output_size: u32,
) -> Result<(DynamicImage, DynamicImage)> {
    let (img_width, img_height) = img.dimensions();

    // Convert normalized bounding box to pixel coordinates
    let x1 = (face_data.bounding_box_x1 * img_width as f32) as u32;
    let y1 = (face_data.bounding_box_y1 * img_height as f32) as u32;
    let x2 = (face_data.bounding_box_x2 * img_width as f32) as u32;
    let y2 = (face_data.bounding_box_y2 * img_height as f32) as u32;

    let face_width = x2.saturating_sub(x1);
    let face_height = y2.saturating_sub(y1);

    if face_width == 0 || face_height == 0 {
        return Err(Error::ImageProcessing(
            "Invalid face bounding box".to_string(),
        ));
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
    let crop_x1 = center_x
        .saturating_sub(crop_size / 2)
        .min(img_width.saturating_sub(crop_size));
    let crop_y1 = center_y
        .saturating_sub(crop_size / 2)
        .min(img_height.saturating_sub(crop_size));

    // Ensure we don't exceed image bounds
    let actual_crop_size = crop_size.min(img_width - crop_x1).min(img_height - crop_y1);

    if actual_crop_size < 10 {
        return Err(Error::ImageProcessing("Crop area too small".to_string()));
    }

    // Crop the face region (full resolution)
    let cropped = img.crop_imm(crop_x1, crop_y1, actual_crop_size, actual_crop_size);

    // Resize to output size
    let resized = cropped.resize_exact(output_size, output_size, FilterType::Lanczos3);

    Ok((cropped, resized))
}

/// Draw debug visualization showing bounding box and crop region.
/// - Red rectangle: face bounding box from Immich
/// - Green rectangle: expanded crop region used for processing
fn draw_crop_debug(img: &DynamicImage, face_data: &FaceData) -> DynamicImage {
    let (img_width, img_height) = img.dimensions();
    let mut rgb_img = img.to_rgb8();

    // Face bounding box in pixel coordinates
    let bbox_x1 = (face_data.bounding_box_x1 * img_width as f32) as i32;
    let bbox_y1 = (face_data.bounding_box_y1 * img_height as f32) as i32;
    let bbox_x2 = (face_data.bounding_box_x2 * img_width as f32) as i32;
    let bbox_y2 = (face_data.bounding_box_y2 * img_height as f32) as i32;

    let face_width = (bbox_x2 - bbox_x1) as u32;
    let face_height = (bbox_y2 - bbox_y1) as u32;

    // Calculate crop region (same logic as crop_face_with_intermediate)
    let face_size = face_width.max(face_height);
    let padding = face_size / 2;
    let crop_size = face_size + padding * 2;

    let center_x = (bbox_x1 + bbox_x2) / 2;
    let center_y = (bbox_y1 + bbox_y2) / 2;

    let crop_x1 = (center_x - crop_size as i32 / 2).max(0) as u32;
    let crop_y1 = (center_y - crop_size as i32 / 2).max(0) as u32;
    let crop_x1 = crop_x1.min(img_width.saturating_sub(crop_size));
    let crop_y1 = crop_y1.min(img_height.saturating_sub(crop_size));
    let actual_crop_size = crop_size.min(img_width - crop_x1).min(img_height - crop_y1);

    // Colors
    let red = Rgb([255u8, 0, 0]);
    let green = Rgb([0u8, 255, 0]);

    // Draw face bounding box (red) - draw multiple times for thickness
    for offset in 0..3i32 {
        let rect = Rect::at(bbox_x1 - offset, bbox_y1 - offset)
            .of_size(face_width + offset as u32 * 2, face_height + offset as u32 * 2);
        draw_hollow_rect_mut(&mut rgb_img, rect, red);
    }

    // Draw crop region (green) - draw multiple times for thickness
    for offset in 0..3i32 {
        let rect = Rect::at(crop_x1 as i32 - offset, crop_y1 as i32 - offset)
            .of_size(actual_crop_size + offset as u32 * 2, actual_crop_size + offset as u32 * 2);
        draw_hollow_rect_mut(&mut rgb_img, rect, green);
    }

    // Draw crosshair at face center
    let cross_size = 20i32;
    draw_line_segment_mut(
        &mut rgb_img,
        ((center_x - cross_size) as f32, center_y as f32),
        ((center_x + cross_size) as f32, center_y as f32),
        red,
    );
    draw_line_segment_mut(
        &mut rgb_img,
        (center_x as f32, (center_y - cross_size) as f32),
        (center_x as f32, (center_y + cross_size) as f32),
        red,
    );

    DynamicImage::ImageRgb8(rgb_img)
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
