//! Background job processing module.
//!
//! Handles the complete pipeline:
//! 1. Fetching assets from Immich for a specific person
//! 2. Downloading and processing images in parallel
//! 3. Processing images through the extensible pipeline
//! 4. Compiling processed images into a timelapse video

mod processing;

use crate::config::{TimeIntervalConfig, TimeRange};
use crate::error::{Error, Result};
use crate::immich_api::{Asset, FaceData, ImmichClient};
use crate::pipeline::Pipeline;
use crate::utils::sanitize_folder_name;
use crate::video::compile_timelapse;
use crate::web::{AppState, AtomicSkipStats, JobStatus, Progress};

use processing::{process_single_asset, AssetProcessResult, DebugDirs, OutputDirs};

use chrono::NaiveDate;
use futures_util::stream::{self, StreamExt, TryStreamExt};
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
pub struct TimeIntervalTracker {
    max_photos: u32,
    time_range: TimeRange,
    buckets: RwLock<HashMap<String, Arc<AtomicU32>>>,
}

impl TimeIntervalTracker {
    /// Create a tracker if photo limiting is enabled, otherwise return None.
    pub fn new(config: &TimeIntervalConfig) -> Option<Self> {
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
            let buckets = self.buckets.read().expect("bucket lock poisoned");
            if let Some(counter) = buckets.get(&key) {
                counter.clone()
            } else {
                drop(buckets);
                let mut buckets = self.buckets.write().expect("bucket lock poisoned");
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

    /// Check if the bucket for the given timestamp is already full.
    /// This is a non-mutating check used to skip processing early.
    pub fn is_full(&self, timestamp: &str) -> bool {
        let key = self.bucket_key(timestamp);
        let buckets = self.buckets.read().expect("bucket lock poisoned");
        if let Some(counter) = buckets.get(&key) {
            counter.load(Ordering::SeqCst) >= self.max_photos
        } else {
            false
        }
    }

    /// Compute the bucket key from a timestamp string.
    fn bucket_key(&self, timestamp: &str) -> String {
        // Parse date from ISO 8601 timestamp (e.g. "2024-01-15" or "2024-01-15T12:34:56Z")
        let date_str = timestamp.get(..10).unwrap_or(timestamp);
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
    pub album_ids: Vec<String>,
    pub album_names: Vec<String>,
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
                    message: Some(format!(
                        "Video saved to: {}\
                        <br><i>Please note that for the best \
                        looking results, you should do a manual pass in the gallery view \
                        (button available in the section below)</i>",
                        output_path.display()
                    )),
                    ..final_progress
                })
                .await;
        }
        Err(Error::Cancelled) => {
            state
                .update_progress(Progress {
                    status: JobStatus::Cancelled,
                    message: Some("Job cancelled by user".to_string()),
                    ..final_progress
                })
                .await;
        }
        Err(e) => {
            tracing::error!("Job failed: {}", e);
            let friendly = e.user_message();
            state
                .update_progress(Progress {
                    status: JobStatus::Error(friendly.clone()),
                    message: Some(friendly),
                    ..final_progress
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
    tokio::fs::create_dir_all(&images_dir)
        .await
        .map_err(|e| permission_aware_io_error(e, &images_dir))?;

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

    // Base progress carrying person/album identity, reused for all updates in this job
    let params_base = Progress {
        person_id: Some(params.person_id.clone()),
        person_name: params.person_name.clone(),
        album_ids: params.album_ids.clone(),
        album_names: params.album_names.clone(),
        ..Progress::default()
    };

    // Update progress: fetching assets
    state
        .update_progress(Progress {
            status: JobStatus::Running,
            message: Some("Fetching assets from Immich...".to_string()),
            ..params_base.clone()
        })
        .await;

    // Check for cancellation
    if cancel_token.is_cancelled() {
        return Err(Error::Cancelled);
    }

    // Fetch all assets for this person (optionally filtered by albums)
    let assets = client
        .get_assets_with_person(
            &params.person_id,
            params.date_from.as_deref(),
            params.date_to.as_deref(),
            &params.album_ids,
        )
        .await?;

    if assets.is_empty() {
        let msg = if params.album_ids.is_empty() {
            "No assets found for this person".to_string()
        } else {
            let names = if params.album_names.is_empty() {
                "selected albums".to_string()
            } else {
                params.album_names.join(", ")
            };
            format!("No assets found for this person in: {}", names)
        };
        return Err(Error::ImageProcessing(msg));
    }

    tracing::info!("Found {} assets to process", assets.len());

    // Fetch face bounding box data for the target person on each asset.
    let face_concurrency = config.processing.max_workers.max(1);
    let assets_with_faces: Vec<(Asset, FaceData)> = stream::iter(assets)
        .map(|asset| {
            let client = &client;
            let person_id = &params.person_id;
            async move {
                let face = client.get_face_for_person(&asset.id, person_id).await?;
                Ok::<_, Error>((asset, face))
            }
        })
        .buffer_unordered(face_concurrency)
        .try_filter_map(|(asset, face)| async move { Ok(face.map(|f| (asset, f))) })
        .try_collect()
        .await?;

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
            total,
            message: Some(format!("Processing {} images...", total)),
            ..params_base.clone()
        })
        .await;

    // Create the processing pipeline based on config
    let pipeline = Arc::new(Pipeline::with_steps_from_config(&config));
    tracing::debug!("Pipeline steps: {:?}", pipeline.step_ids());

    // Create photo limit tracker if enabled
    let time_interval_tracker =
        TimeIntervalTracker::new(&config.processing.time_interval).map(Arc::new);

    // Process images in parallel with concurrency limit
    let completed = Arc::new(AtomicU32::new(0));
    let semaphore = Arc::new(Semaphore::new(config.processing.max_workers));
    let client = Arc::new(client);
    let config = Arc::new(config);
    let output_dirs = Arc::new(output_dirs);

    // Atomic counters for real-time skip statistics (consolidated into single struct)
    let skip_stats = Arc::new(AtomicSkipStats::new());

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
        let pipeline = pipeline.clone();

        // Clone skip stats for this task
        let skip_stats = skip_stats.clone();

        // Clone photo limit tracker for this task
        let time_interval_tracker = time_interval_tracker.clone();

        let task_base = params_base.clone();

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
                time_interval_tracker.as_ref(),
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
                        ..task_base
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

    // Count results and log errors (deduplicated)
    let mut successful = 0usize;
    let mut errors = 0usize;
    let mut first_error: Option<String> = None;
    let mut all_same_error = true;
    for result in &results {
        match result {
            AssetProcessResult::Success { .. } => successful += 1,
            AssetProcessResult::Error { error, .. } => {
                errors += 1;
                match &first_error {
                    None => first_error = Some(error.clone()),
                    Some(first) if first != error => all_same_error = false,
                    _ => {}
                }
            }
            _ => {}
        }
    }
    if let Some(ref err) = first_error {
        if all_same_error {
            tracing::error!("{} assets failed with: {}", errors, err);
        } else {
            // Log distinct errors individually (up to 3)
            let mut logged = 0;
            for result in &results {
                if let AssetProcessResult::Error { asset_id, error } = result {
                    if logged < 3 {
                        tracing::error!("Asset {} failed: {}", asset_id, error);
                        logged += 1;
                    }
                }
            }
            if errors > 3 {
                tracing::error!("... and {} more errors", errors - 3);
            }
        }
    }
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
            ..params_base.clone()
        })
        .await;

    if successful == 0 {
        let detail = first_error
            .map(|e| format!(" First error: {}", e))
            .unwrap_or_default();
        return Err(Error::ImageProcessing(format!(
            "No images were successfully processed ({} errors).{}",
            errors, detail
        )));
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
                total: successful as u32,
                message: Some("Compiling video...".to_string()),
                skip_stats: final_skip_stats.clone(),
                ..params_base
            })
            .await;

        compile_timelapse(
            &output_dirs.images,
            &output_dirs.video,
            &config.video,
            Some(&cancel_token),
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

/// Convert a permission-denied I/O error into a Config error with Docker fix instructions.
fn permission_aware_io_error(e: std::io::Error, path: &std::path::Path) -> Error {
    if e.kind() == std::io::ErrorKind::PermissionDenied {
        Error::Config(format!(
            "Permission denied for '{}'. {}",
            path.display(),
            crate::error::PERMISSION_HINT
        ))
    } else {
        Error::Io(e)
    }
}
