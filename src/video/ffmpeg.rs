//! FFmpeg wrapper for video compilation.

use crate::config::VideoConfig;
use crate::error::{Error, Result};
use std::path::Path;
use std::process::Stdio;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::Command;

/// Compile images into a timelapse video.
///
/// # Arguments
/// * `input_dir` - Directory containing JPEG images (sorted alphabetically for order)
/// * `output_path` - Path for the output video file
/// * `config` - Video configuration
/// * `progress_callback` - Called with (current_frame, total_frames)
pub async fn compile_timelapse<F>(
    input_dir: &Path,
    output_path: &Path,
    config: &VideoConfig,
    mut progress_callback: F,
) -> Result<()>
where
    F: FnMut(u32, u32),
{
    // Collect and sort image files for reliable ordering.
    // Filenames are timestamp-based, so alphabetical sort = chronological order.
    let mut image_files: Vec<_> = Vec::new();
    let mut entries = tokio::fs::read_dir(input_dir).await?;

    while let Some(entry) = entries.next_entry().await? {
        let path = entry.path();
        if path.extension().map(|e| e == "jpg").unwrap_or(false) {
            image_files.push(path);
        }
    }

    if image_files.is_empty() {
        return Err(Error::VideoCompilation(
            "No images found in input directory".to_string(),
        ));
    }

    // Sort alphabetically (timestamp-based names ensure chronological order)
    image_files.sort();
    let image_count = image_files.len() as u32;

    tracing::info!("Compiling {} images into video", image_count);

    // Create concat file for ffmpeg (more reliable than glob pattern).
    // Format: file '/path/to/image.jpg'\nduration 0.0667\n...
    let frame_duration = 1.0 / config.framerate as f64;
    let concat_path = input_dir.join(".concat.txt");

    let mut concat_content = String::new();
    for (i, path) in image_files.iter().enumerate() {
        // Use just the filename since concat file is in the same directory as images
        let filename = path
            .file_name()
            .map(|f| f.to_string_lossy())
            .unwrap_or_default()
            .replace('\'', "'\\''");
        concat_content.push_str(&format!("file '{}'\n", filename));

        // Don't add duration after the last frame
        if i < image_files.len() - 1 {
            concat_content.push_str(&format!("duration {:.6}\n", frame_duration));
        }
    }

    // Write concat file
    let mut file = tokio::fs::File::create(&concat_path).await?;
    file.write_all(concat_content.as_bytes()).await?;
    file.flush().await?;

    // Build ffmpeg command using concat demuxer
    let mut cmd = Command::new("ffmpeg");
    cmd.arg("-y") // Overwrite output
        .arg("-f")
        .arg("concat")
        .arg("-safe")
        .arg("0") // Allow absolute paths
        .arg("-i")
        .arg(&concat_path)
        .arg("-c:v")
        .arg(&config.codec)
        .arg("-crf")
        .arg(config.crf.to_string())
        .arg("-pix_fmt")
        .arg("yuv420p")
        .arg("-progress")
        .arg("pipe:1")
        .arg(output_path)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    let mut child = cmd.spawn().map_err(|e| {
        Error::FFmpeg(format!(
            "Failed to spawn ffmpeg (is it installed?): {}",
            e
        ))
    })?;

    // Capture stdout for progress and stderr for errors
    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| Error::FFmpeg("Failed to capture ffmpeg stdout".to_string()))?;
    let stderr = child
        .stderr
        .take()
        .ok_or_else(|| Error::FFmpeg("Failed to capture ffmpeg stderr".to_string()))?;

    // Read stderr in background to avoid blocking
    let stderr_handle = tokio::spawn(async move {
        let mut stderr_reader = BufReader::new(stderr).lines();
        let mut stderr_output = String::new();
        while let Ok(Some(line)) = stderr_reader.next_line().await {
            stderr_output.push_str(&line);
            stderr_output.push('\n');
        }
        stderr_output
    });

    // Parse progress from stdout
    let mut reader = BufReader::new(stdout).lines();
    while let Some(line) = reader.next_line().await? {
        if line.starts_with("frame=") {
            if let Some(frame_str) = line.strip_prefix("frame=") {
                if let Ok(frame) = frame_str.trim().parse::<u32>() {
                    // Clamp to image_count - ffmpeg can report higher values internally
                    let clamped_frame = frame.min(image_count);
                    progress_callback(clamped_frame, image_count);
                }
            }
        }
    }

    let status = child.wait().await?;
    let stderr_output = stderr_handle.await.unwrap_or_default();

    // Clean up concat file
    let _ = tokio::fs::remove_file(&concat_path).await;

    if !status.success() {
        // Get last few lines of stderr for the error message
        let error_lines: Vec<&str> = stderr_output.lines().rev().take(10).collect();
        let error_detail = error_lines.into_iter().rev().collect::<Vec<_>>().join("\n");

        return Err(Error::FFmpeg(format!(
            "ffmpeg exited with status {}\n{}",
            status,
            error_detail
        )));
    }

    progress_callback(image_count, image_count);
    tracing::info!("Video compilation complete: {:?}", output_path);

    Ok(())
}

/// Check if ffmpeg is available.
pub async fn check_ffmpeg() -> Result<String> {
    let output = Command::new("ffmpeg")
        .arg("-version")
        .output()
        .await
        .map_err(|e| Error::FFmpeg(format!("ffmpeg not found: {}", e)))?;

    if !output.status.success() {
        return Err(Error::FFmpeg("ffmpeg version check failed".to_string()));
    }

    let version_output = String::from_utf8_lossy(&output.stdout);
    let version = version_output
        .lines()
        .next()
        .unwrap_or("unknown")
        .to_string();

    Ok(version)
}
