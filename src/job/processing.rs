//! Single asset processing logic.
//!
//! Contains the logic for processing individual images through the pipeline,
//! including downloading, running the pipeline, and saving results.

use crate::config::Config;
use crate::immich_api::{Asset, FaceData, ImmichClient};
use crate::pipeline::{Pipeline, PipelineContext, PipelineResult};
use crate::web::AtomicSkipStats;

use bytes::Bytes;
use image::ImageFormat;
use std::io::Cursor;
use std::path::PathBuf;
use std::sync::Arc;
use tokio_util::sync::CancellationToken;

/// Output directories for a processing job.
#[derive(Debug, Clone)]
pub struct OutputDirs {
    /// Directory for final processed images (used for video)
    pub images: PathBuf,
    /// Path for output video
    pub video: PathBuf,
    /// Optional debug directories for visualizing processing stages.
    pub debug: Option<DebugDirs>,
}

/// Debug output base directory for visualizing processing stages.
/// Subdirectories are created on-demand by the pipeline for each step:
/// - `{step_id}/passed/` - Images that passed the step
/// - `{step_id}/failed/` - Images that failed/skipped at the step
#[derive(Debug, Clone)]
pub struct DebugDirs {
    /// Base directory for debug output (e.g., `output/PersonName/debug`)
    pub base: PathBuf,
}

/// Result of processing a single asset.
///
/// Fields are kept for debugging/logging purposes even if not currently read.
#[derive(Debug)]
#[allow(dead_code)]
pub enum AssetProcessResult {
    /// Successfully processed.
    Success { asset_id: String },
    /// Skipped for some reason.
    Skipped { asset_id: String, reason: String },
    /// Error during processing.
    Error { asset_id: String, error: String },
    /// Cancelled by user.
    Cancelled { asset_id: String },
}

/// Process a single asset using the pipeline.
///
/// This function handles:
/// 1. Downloading the image from Immich
/// 2. Running it through the processing pipeline
/// 3. Saving the result to disk
#[allow(clippy::too_many_arguments)]
pub async fn process_single_asset(
    client: &ImmichClient,
    config: &Config,
    asset: &Asset,
    face_data: &FaceData,
    output_dirs: &OutputDirs,
    cancel_token: &CancellationToken,
    skip_stats: &Arc<AtomicSkipStats>,
    pipeline: &Pipeline,
) -> AssetProcessResult {
    let asset_id = &asset.id;

    // Generate timestamp-based filename for sorting
    let timestamp = asset
        .file_created_at
        .as_ref()
        .or(asset.local_date_time.as_ref())
        .cloned()
        .unwrap_or_else(|| asset_id.clone());

    // Create pipeline context
    let mut ctx = PipelineContext::new(asset_id.clone(), timestamp.clone(), face_data.clone());

    // Check before download (potentially slow)
    if cancel_token.is_cancelled() {
        return AssetProcessResult::Cancelled {
            asset_id: asset_id.clone(),
        };
    }

    // Download image
    let image_bytes: Bytes = match client.download_asset(asset_id).await {
        Ok(bytes) => bytes,
        Err(e) => {
            skip_stats.increment("download_failed");
            return AssetProcessResult::Error {
                asset_id: asset_id.clone(),
                error: format!("Download failed: {}", e),
            };
        }
    };

    // Set raw bytes on context
    ctx = ctx.with_bytes(image_bytes);

    // Determine debug directory
    let debug_dir = output_dirs.debug.as_ref().map(|d| d.base.clone());

    // Execute the pipeline
    let result = pipeline
        .execute(ctx, config, cancel_token, skip_stats, debug_dir.as_ref())
        .await;

    match result {
        PipelineResult::Success {
            image,
            asset_id,
            timestamp,
            ..
        } => {
            // Sanitize timestamp for filename
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
            let output_path = output_dirs.images.join(&filename);

            // Encode and save final image
            let mut buffer = Cursor::new(Vec::new());
            if let Err(e) = image.write_to(&mut buffer, ImageFormat::Jpeg) {
                return AssetProcessResult::Error {
                    asset_id,
                    error: format!("Failed to encode image: {}", e),
                };
            }

            if let Err(e) = tokio::fs::write(&output_path, buffer.into_inner()).await {
                return AssetProcessResult::Error {
                    asset_id,
                    error: format!("Failed to save image: {}", e),
                };
            }

            AssetProcessResult::Success { asset_id }
        }
        PipelineResult::Skipped {
            asset_id, reason, ..
        } => AssetProcessResult::Skipped { asset_id, reason },
        PipelineResult::Error { asset_id, error } => AssetProcessResult::Error { asset_id, error },
        PipelineResult::Cancelled { asset_id } => AssetProcessResult::Cancelled { asset_id },
    }
}
