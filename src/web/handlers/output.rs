//! Output folder and image management endpoints.

use crate::web::state::{validate_path_component, AppState, JobStatus, Progress};
use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::Json,
};
use serde::{Deserialize, Serialize};

use super::StartResponse;

/// Output folder information.
#[derive(Serialize)]
pub struct OutputFolderInfo {
    pub name: String,
    pub image_count: u32,
    pub size_bytes: u64,
    pub has_video: bool,
}

/// Image information for gallery view.
#[derive(Serialize)]
pub struct ImageInfo {
    pub filename: String,
    pub size_bytes: u64,
}

/// Response for listing images in a folder.
#[derive(Serialize)]
pub struct FolderImagesResponse {
    pub folder_name: String,
    pub images: Vec<ImageInfo>,
    pub total_count: u32,
    pub total_size_bytes: u64,
    pub video_exists: bool,
}

/// Request for bulk deleting images.
#[derive(Deserialize)]
pub struct BulkDeleteRequest {
    pub filenames: Vec<String>,
}

/// Response for bulk delete operations.
#[derive(Serialize)]
pub struct BulkDeleteResponse {
    pub deleted_count: u32,
    pub failed_count: u32,
    pub remaining_images: u32,
}

/// Check if a path exists asynchronously.
async fn path_exists(path: &std::path::Path) -> bool {
    tokio::fs::metadata(path).await.is_ok()
}

/// Check if a path is a directory asynchronously.
async fn path_is_dir(path: &std::path::Path) -> bool {
    tokio::fs::metadata(path)
        .await
        .map(|m| m.is_dir())
        .unwrap_or(false)
}

/// List all output folders with their stats.
pub async fn list_output_folders(
    State(state): State<AppState>,
) -> Result<Json<Vec<OutputFolderInfo>>, (StatusCode, String)> {
    let config = state.config.read().await;
    let output_dir = &config.output_dir;

    let mut folders = Vec::new();

    if path_exists(output_dir).await {
        let mut entries = tokio::fs::read_dir(output_dir).await.map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Failed to read output directory: {}", e),
            )
        })?;

        while let Some(entry) = entries.next_entry().await.map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Failed to read directory entry: {}", e),
            )
        })? {
            let path = entry.path();
            if path_is_dir(&path).await {
                let name = entry.file_name().to_string_lossy().to_string();

                // Count images in the images subfolder
                let images_dir = path.join("images");
                let mut image_count = 0u32;
                let mut size_bytes = 0u64;

                if path_exists(&images_dir).await {
                    if let Ok(mut img_entries) = tokio::fs::read_dir(&images_dir).await {
                        while let Ok(Some(img_entry)) = img_entries.next_entry().await {
                            let img_path = img_entry.path();
                            if img_path.extension().is_some_and(|ext| ext == "jpg") {
                                image_count += 1;
                                if let Ok(metadata) = tokio::fs::metadata(&img_path).await {
                                    size_bytes += metadata.len();
                                }
                            }
                        }
                    }
                }

                // Check for video file (named after the folder/person)
                let video_filename = format!("{}.mp4", name);
                let has_video = path_exists(&path.join(&video_filename)).await;

                folders.push(OutputFolderInfo {
                    name,
                    image_count,
                    size_bytes,
                    has_video,
                });
            }
        }
    }

    // Sort by name
    folders.sort_by(|a, b| a.name.cmp(&b.name));

    Ok(Json(folders))
}

/// Clean up all output folders.
pub async fn cleanup_all_output(
    State(state): State<AppState>,
) -> Result<Json<StartResponse>, (StatusCode, String)> {
    state
        .ensure_no_job_running()
        .await
        .map_err(|e| (StatusCode::CONFLICT, e.to_string()))?;

    let config = state.config.read().await;
    let output_dir = &config.output_dir;

    // Remove all contents of output directory
    if path_exists(output_dir).await {
        let mut entries = tokio::fs::read_dir(output_dir).await.map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Failed to read output directory: {}", e),
            )
        })?;

        let mut deleted_count = 0;
        while let Some(entry) = entries.next_entry().await.map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Failed to read directory entry: {}", e),
            )
        })? {
            let path = entry.path();
            if path_is_dir(&path).await {
                tokio::fs::remove_dir_all(&path).await.map_err(|e| {
                    (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        format!("Failed to remove directory {}: {}", path.display(), e),
                    )
                })?;
                deleted_count += 1;
            }
        }

        tracing::info!("Cleaned up {} output folders", deleted_count);

        Ok(Json(StartResponse {
            success: true,
            message: format!("Deleted {} output folders", deleted_count),
        }))
    } else {
        Ok(Json(StartResponse {
            success: true,
            message: "Output directory does not exist".to_string(),
        }))
    }
}

/// Clean up a specific output folder by name.
pub async fn cleanup_output_folder(
    State(state): State<AppState>,
    Path(folder_name): Path<String>,
) -> Result<Json<StartResponse>, (StatusCode, String)> {
    state
        .ensure_no_job_running()
        .await
        .map_err(|e| (StatusCode::CONFLICT, e.to_string()))?;

    validate_path_component(&folder_name, "folder name")
        .map_err(|e| (StatusCode::BAD_REQUEST, e.to_string()))?;

    let config = state.config.read().await;

    let folder_path = config.output_dir.join(&folder_name);

    let folder_meta = tokio::fs::metadata(&folder_path).await.map_err(|_| {
        (
            StatusCode::NOT_FOUND,
            format!("Folder '{}' not found", folder_name),
        )
    })?;

    if !folder_meta.is_dir() {
        return Err((
            StatusCode::BAD_REQUEST,
            format!("'{}' is not a directory", folder_name),
        ));
    }

    tokio::fs::remove_dir_all(&folder_path).await.map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Failed to remove folder: {}", e),
        )
    })?;

    tracing::info!("Cleaned up output folder: {}", folder_name);

    Ok(Json(StartResponse {
        success: true,
        message: format!("Deleted folder '{}'", folder_name),
    }))
}

/// List images in a specific output folder.
pub async fn list_folder_images(
    State(state): State<AppState>,
    Path(folder_name): Path<String>,
) -> Result<Json<FolderImagesResponse>, (StatusCode, String)> {
    validate_path_component(&folder_name, "folder name")
        .map_err(|e| (StatusCode::BAD_REQUEST, e.to_string()))?;

    let config = state.config.read().await;

    let images_dir = config.output_dir.join(&folder_name).join("images");

    if !path_exists(&images_dir).await {
        return Err((
            StatusCode::NOT_FOUND,
            format!("Folder '{}' not found or has no images", folder_name),
        ));
    }

    let mut images = Vec::new();
    let mut total_size_bytes = 0u64;

    let mut entries = tokio::fs::read_dir(&images_dir).await.map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Failed to read images directory: {}", e),
        )
    })?;

    while let Some(entry) = entries.next_entry().await.map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Failed to read directory entry: {}", e),
        )
    })? {
        let path = entry.path();
        if path.extension().is_some_and(|ext| ext == "jpg") {
            let filename = entry.file_name().to_string_lossy().to_string();
            let metadata = tokio::fs::metadata(&path).await.map_err(|e| {
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    format!("Failed to read file metadata: {}", e),
                )
            })?;
            let size_bytes = metadata.len();
            total_size_bytes += size_bytes;

            images.push(ImageInfo {
                filename,
                size_bytes,
            });
        }
    }

    // Sort by filename (chronological since filenames are timestamp-based)
    images.sort_by(|a, b| a.filename.cmp(&b.filename));

    let total_count = images.len() as u32;
    let video_filename = format!("{}.mp4", folder_name);
    let video_exists =
        path_exists(&config.output_dir.join(&folder_name).join(&video_filename)).await;

    Ok(Json(FolderImagesResponse {
        folder_name,
        images,
        total_count,
        total_size_bytes,
        video_exists,
    }))
}

/// Delete a single image from an output folder.
pub async fn delete_single_image(
    State(state): State<AppState>,
    Path((folder_name, filename)): Path<(String, String)>,
) -> Result<Json<StartResponse>, (StatusCode, String)> {
    state
        .ensure_no_job_running()
        .await
        .map_err(|e| (StatusCode::CONFLICT, e.to_string()))?;

    validate_path_component(&folder_name, "folder name")
        .map_err(|e| (StatusCode::BAD_REQUEST, e.to_string()))?;
    validate_path_component(&filename, "filename")
        .map_err(|e| (StatusCode::BAD_REQUEST, e.to_string()))?;

    let config = state.config.read().await;
    if !filename.ends_with(".jpg") {
        return Err((
            StatusCode::BAD_REQUEST,
            "Only .jpg files can be deleted".to_string(),
        ));
    }

    let file_path = config
        .output_dir
        .join(&folder_name)
        .join("images")
        .join(&filename);

    if !path_exists(&file_path).await {
        return Err((
            StatusCode::NOT_FOUND,
            format!("Image '{}' not found in folder '{}'", filename, folder_name),
        ));
    }

    tokio::fs::remove_file(&file_path).await.map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Failed to delete image: {}", e),
        )
    })?;

    tracing::info!("Deleted image: {}/{}", folder_name, filename);

    Ok(Json(StartResponse {
        success: true,
        message: format!("Deleted image '{}'", filename),
    }))
}

/// Bulk delete images from an output folder.
pub async fn delete_images_bulk(
    State(state): State<AppState>,
    Path(folder_name): Path<String>,
    Json(request): Json<BulkDeleteRequest>,
) -> Result<Json<BulkDeleteResponse>, (StatusCode, String)> {
    state
        .ensure_no_job_running()
        .await
        .map_err(|e| (StatusCode::CONFLICT, e.to_string()))?;

    validate_path_component(&folder_name, "folder name")
        .map_err(|e| (StatusCode::BAD_REQUEST, e.to_string()))?;

    let config = state.config.read().await;

    let images_dir = config.output_dir.join(&folder_name).join("images");

    if !path_exists(&images_dir).await {
        return Err((
            StatusCode::NOT_FOUND,
            format!("Folder '{}' not found or has no images", folder_name),
        ));
    }

    let mut deleted_count = 0u32;
    let mut failed_count = 0u32;

    for filename in &request.filenames {
        // Validate each filename
        if validate_path_component(filename, "filename").is_err() {
            failed_count += 1;
            continue;
        }
        if !filename.ends_with(".jpg") {
            failed_count += 1;
            continue;
        }

        let file_path = images_dir.join(filename);
        match tokio::fs::remove_file(&file_path).await {
            Ok(_) => deleted_count += 1,
            Err(_) => failed_count += 1,
        }
    }

    // Count remaining images
    let mut remaining_images = 0u32;
    if let Ok(mut entries) = tokio::fs::read_dir(&images_dir).await {
        while let Ok(Some(entry)) = entries.next_entry().await {
            if entry.path().extension().is_some_and(|ext| ext == "jpg") {
                remaining_images += 1;
            }
        }
    }

    tracing::info!(
        "Bulk deleted {} images from folder {} ({} failed)",
        deleted_count,
        folder_name,
        failed_count
    );

    Ok(Json(BulkDeleteResponse {
        deleted_count,
        failed_count,
        remaining_images,
    }))
}

/// Compile video for a specific output folder.
pub async fn compile_folder_video(
    State(state): State<AppState>,
    Path(folder_name): Path<String>,
) -> Result<Json<StartResponse>, (StatusCode, String)> {
    validate_path_component(&folder_name, "folder name")
        .map_err(|e| (StatusCode::BAD_REQUEST, e.to_string()))?;

    let config = state.config.read().await;

    let folder_path = config.output_dir.join(&folder_name);
    let images_dir = folder_path.join("images");

    if !path_exists(&images_dir).await {
        return Err((
            StatusCode::NOT_FOUND,
            format!("Folder '{}' not found or has no images", folder_name),
        ));
    }

    // Count images to verify there are some
    let mut image_count = 0u32;
    if let Ok(mut entries) = tokio::fs::read_dir(&images_dir).await {
        while let Ok(Some(entry)) = entries.next_entry().await {
            if entry.path().extension().is_some_and(|ext| ext == "jpg") {
                image_count += 1;
            }
        }
    }

    if image_count == 0 {
        return Err((
            StatusCode::BAD_REQUEST,
            "No images found in folder to compile".to_string(),
        ));
    }

    // Atomically check-and-set: hold write lock across both check and status update
    // to prevent TOCTOU race conditions between concurrent requests.
    {
        let mut progress = state.progress.write().await;
        if progress.status == JobStatus::Running || progress.status == JobStatus::CompilingVideo {
            return Err((StatusCode::CONFLICT, "A job is already running".to_string()));
        }
        let new_progress = Progress {
            status: JobStatus::CompilingVideo,
            total: image_count,
            message: Some(format!("Compiling video for {}...", folder_name)),
            person_name: Some(folder_name.clone()),
            ..Progress::default()
        };
        *progress = new_progress.clone();
        let _ = state.progress_tx.send(new_progress);
    }

    // Create cancellation token
    let cancel_token = state.create_cancel_token().await;

    // Clone values needed for the async task
    let video_config = config.video.clone();
    let video_filename = format!("{}.mp4", folder_name);
    let output_path = folder_path.join(&video_filename);
    let job_state = state.clone();
    let folder_name_clone = folder_name.clone();

    // Spawn the compilation job in the background
    tokio::spawn(async move {
        let result = crate::video::compile_timelapse(
            &images_dir,
            &output_path,
            &video_config,
            Some(&cancel_token),
            |current, total| {
                // Check for cancellation
                if cancel_token.is_cancelled() {
                    return;
                }

                // Update progress (fire and forget since we're in sync callback)
                let state_clone = job_state.clone();
                let folder_clone = folder_name_clone.clone();
                tokio::spawn(async move {
                    state_clone
                        .update_progress(Progress {
                            status: JobStatus::CompilingVideo,
                            completed: current,
                            total,
                            message: Some(format!("Compiling video for {}...", folder_clone)),
                            person_name: Some(folder_clone),
                            ..Progress::default()
                        })
                        .await;
                });
            },
        )
        .await;

        // Update final status
        match result {
            Ok(_) => {
                job_state
                    .update_progress(Progress {
                        status: JobStatus::Completed,
                        completed: image_count,
                        total: image_count,
                        message: Some("Video compilation complete".to_string()),
                        person_name: Some(folder_name_clone),
                        ..Progress::default()
                    })
                    .await;
            }
            Err(e) => {
                if cancel_token.is_cancelled() {
                    job_state
                        .update_progress(Progress {
                            status: JobStatus::Cancelled,
                            total: image_count,
                            message: Some("Video compilation cancelled".to_string()),
                            person_name: Some(folder_name_clone),
                            ..Progress::default()
                        })
                        .await;
                } else {
                    tracing::error!("Video compilation failed: {}", e);
                    job_state
                        .update_progress(Progress {
                            status: JobStatus::Error(e.to_string()),
                            total: image_count,
                            message: Some(format!("Video compilation failed: {}", e)),
                            person_name: Some(folder_name_clone),
                            ..Progress::default()
                        })
                        .await;
                }
            }
        }

        job_state.clear_cancel_token().await;
    });

    Ok(Json(StartResponse {
        success: true,
        message: format!("Video compilation started for '{}'", folder_name),
    }))
}
