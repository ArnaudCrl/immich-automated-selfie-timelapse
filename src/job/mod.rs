//! Background job processing module.
//!
//! Handles the complete pipeline:
//! 1. Fetching assets from Immich for a specific person
//! 2. Downloading and processing images in parallel
//! 3. Processing images through the extensible pipeline
//! 4. Compiling processed images into a timelapse video

mod processing;

use crate::config::{PhotoLimitConfig, TimeRange};
use crate::error::{Error, Result};
use crate::immich_api::{Asset, FaceData, ImmichClient};
use crate::pipeline::Pipeline;
use crate::utils::sanitize_folder_name;
use crate::video::compile_timelapse;
use crate::web::{AppState, AtomicSkipStats, JobStatus, Progress, SkipStats};

use processing::{process_single_asset, AssetProcessResult, DebugDirs, OutputDirs};

use chrono::NaiveDate;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, RwLock};
use tokio::sync::Semaphore;
use tokio_util::sync::CancellationToken;

/// Tracks per-time-bucket photo counts for limiting.
///
/// Thread-safe: uses atomic counters with fetch_add + undo pattern
/// so concurrent workers can claim slots without races.
pub struct PhotoLimitTracker {
    max_photos: u32,
    time_range: TimeRange,
    buckets: RwLock<HashMap<String, Arc<AtomicU32>>>,
}

impl PhotoLimitTracker {
    /// Create a tracker if photo limiting is enabled, otherwise return None.
    pub fn new(config: &PhotoLimitConfig) -> Option<Self> {
        if !config.enabled {
            return None;
        }
        Some(Self {
            max_photos: config.max_photos,
            time_range: config.time_range,
            buckets: RwLock::new(HashMap::new()),
        })
    }

    /// Try to claim a slot for the given timestamp.
    /// Returns true if the slot was claimed, false if the bucket is full.
    pub fn try_claim(&self, timestamp: &str) -> bool {
        let key = self.bucket_key(timestamp);

        // Get or create the atomic counter for this bucket
        let counter = {
            // Try read lock first
            let buckets = self.buckets.read().unwrap();
            if let Some(counter) = buckets.get(&key) {
                counter.clone()
            } else {
                drop(buckets);
                let mut buckets = self.buckets.write().unwrap();
                buckets
                    .entry(key)
                    .or_insert_with(|| Arc::new(AtomicU32::new(0)))
                    .clone()
            }
        };

        // Atomically try to claim: increment, check, undo if over limit
        let prev = counter.fetch_add(1, Ordering::SeqCst);
        if prev < self.max_photos {
            true
        } else {
            counter.fetch_sub(1, Ordering::SeqCst);
            false
        }
    }

    /// Compute the bucket key from a timestamp string.
    fn bucket_key(&self, timestamp: &str) -> String {
        // Parse date from ISO 8601 timestamp (e.g. "2024-01-15" or "2024-01-15T12:34:56Z")
        let date_str = &timestamp[..timestamp.len().min(10)];
        let date = NaiveDate::parse_from_str(date_str, "%Y-%m-%d").unwrap_or_default();

        match self.time_range {
            TimeRange::Day => date.format("%Y-%m-%d").to_string(),
            TimeRange::Week => {
                use chrono::Datelike;
                format!("{}-W{:02}", date.iso_week().year(), date.iso_week().week())
            }
            TimeRange::Month => date.format("%Y-%m").to_string(),
        }
    }
}

/// Parameters for starting a processing job.
#[derive(Debug, Clone)]
pub struct JobParams {
    pub person_id: String,
    pub person_name: Option<String>,
    pub date_from: Option<String>,
    pub date_to: Option<String>,
}

/// Run the complete processing pipeline.
///
/// This is the main entry point for background job processing.
/// It handles progress reporting and cancellation.
pub async fn run_job(state: AppState, params: JobParams, cancel_token: CancellationToken) {
    let result = run_job_inner(state.clone(), params, cancel_token).await;

    // Clear the cancellation token when done
    state.clear_cancel_token().await;

    // Get final state from progress (skip stats and person info)
    let final_progress = state.progress.read().await.clone();

    match result {
        Ok(output_path) => {
            state
                .update_progress(Progress {
                    status: JobStatus::Completed,
                    completed: final_progress.completed,
                    total: final_progress.total,
                    message: Some(format!("Video saved to: {}", output_path.display())),
                    skip_stats: final_progress.skip_stats,
                    person_id: final_progress.person_id,
                    person_name: final_progress.person_name,
                })
                .await;
        }
        Err(Error::Cancelled) => {
            state
                .update_progress(Progress {
                    status: JobStatus::Cancelled,
                    completed: final_progress.completed,
                    total: final_progress.total,
                    message: Some("Job cancelled by user".to_string()),
                    skip_stats: final_progress.skip_stats,
                    person_id: final_progress.person_id,
                    person_name: final_progress.person_name,
                })
                .await;
        }
        Err(e) => {
            tracing::error!("Job failed: {}", e);
            state
                .update_progress(Progress {
                    status: JobStatus::Error(e.to_string()),
                    completed: final_progress.completed,
                    total: final_progress.total,
                    message: Some(e.to_string()),
                    skip_stats: final_progress.skip_stats,
                    person_id: final_progress.person_id,
                    person_name: final_progress.person_name,
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

    // If the folder exists, delete all its contents to start fresh
    if person_dir.exists() {
        tracing::info!("Removing existing folder: {}", person_dir.display());
        tokio::fs::remove_dir_all(&person_dir).await?;
    }

    // Create output directories
    let images_dir = person_dir.join("images");
    tokio::fs::create_dir_all(&images_dir).await?;

    // Create debug directories if enabled
    let debug = if config.processing.output.keep_intermediates {
        let debug_base = person_dir.join("debug");
        tokio::fs::create_dir_all(&debug_base).await?;
        Some(DebugDirs { base: debug_base })
    } else {
        None
    };

    let video_filename = format!("{}.mp4", folder_name);
    let output_dirs = OutputDirs {
        images: images_dir.clone(),
        video: person_dir.join(&video_filename),
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
            skip_stats: SkipStats::default(),
            person_id: Some(params.person_id.clone()),
            person_name: params.person_name.clone(),
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
            skip_stats: SkipStats::default(),
            person_id: Some(params.person_id.clone()),
            person_name: params.person_name.clone(),
        })
        .await;

    // Create the processing pipeline based on config
    let pipeline = Arc::new(Pipeline::with_steps_from_config(&config));
    tracing::debug!("Pipeline steps: {:?}", pipeline.step_ids());

    // Create photo limit tracker if enabled
    let photo_limit_tracker = PhotoLimitTracker::new(&config.processing.photo_limit).map(Arc::new);

    // Process images in parallel with concurrency limit
    let completed = Arc::new(AtomicU32::new(0));
    let semaphore = Arc::new(Semaphore::new(config.processing.max_workers));
    let client = Arc::new(client);
    let config = Arc::new(config);
    let output_dirs = Arc::new(output_dirs);

    // Atomic counters for real-time skip statistics (consolidated into single struct)
    let skip_stats = Arc::new(AtomicSkipStats::new());

    let mut handles = Vec::with_capacity(assets_with_faces.len());

    // Clone person info for use in spawned tasks
    let task_person_id = params.person_id.clone();
    let task_person_name = params.person_name.clone();

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
        let pipeline = pipeline.clone();

        // Clone skip stats for this task
        let skip_stats = skip_stats.clone();

        // Clone photo limit tracker for this task
        let photo_limit_tracker = photo_limit_tracker.clone();

        // Clone person info for this task
        let person_id = task_person_id.clone();
        let person_name = task_person_name.clone();

        let handle = tokio::spawn(async move {
            // Check cancellation at start of task
            if task_cancel_token.is_cancelled() {
                drop(permit);
                return AssetProcessResult::Cancelled {
                    asset_id: asset.id.clone(),
                };
            }

            let result = process_single_asset(
                &client,
                &config,
                &asset,
                &face_data,
                &output_dirs,
                &task_cancel_token,
                &skip_stats,
                &pipeline,
                photo_limit_tracker.as_ref(),
            )
            .await;

            // Update progress with current skip stats (only if not cancelled)
            if !task_cancel_token.is_cancelled() {
                let done = completed.fetch_add(1, Ordering::SeqCst) + 1;
                let current_skip_stats = skip_stats.snapshot();
                state
                    .update_progress(Progress {
                        status: JobStatus::Running,
                        completed: done,
                        total,
                        message: Some(format!("Processing images... ({}/{})", done, total)),
                        skip_stats: current_skip_stats,
                        person_id: Some(person_id.clone()),
                        person_name: person_name.clone(),
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

    // Get final skip statistics from atomic counters
    let final_skip_stats = skip_stats.snapshot();

    // Count results
    let successful = results
        .iter()
        .filter(|r| matches!(r, AssetProcessResult::Success { .. }))
        .count();
    let errors = results
        .iter()
        .filter(|r| matches!(r, AssetProcessResult::Error { .. }))
        .count();
    let skipped = final_skip_stats.total();

    tracing::info!(
        "Processing complete: {} successful, {} skipped, {} errors",
        successful,
        skipped,
        errors
    );

    // Update progress with final skip stats
    state
        .update_progress(Progress {
            status: JobStatus::Running,
            completed: total,
            total,
            message: Some("Processing complete, preparing video...".to_string()),
            skip_stats: final_skip_stats.clone(),
            person_id: Some(params.person_id.clone()),
            person_name: params.person_name.clone(),
        })
        .await;

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
                skip_stats: final_skip_stats.clone(),
                person_id: Some(params.person_id.clone()),
                person_name: params.person_name.clone(),
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
