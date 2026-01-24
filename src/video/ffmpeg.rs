//! FFmpeg wrapper for video compilation.

use crate::config::VideoConfig;
use crate::error::{Error, Result};
use std::path::Path;
use std::process::Stdio;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;

/// Compile images into a timelapse video.
///
/// # Arguments
/// * `input_dir` - Directory containing numbered JPEG images
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
    // Count input images
    let mut image_count = 0u32;
    let mut entries = tokio::fs::read_dir(input_dir).await?;
    while let Some(entry) = entries.next_entry().await? {
        let path = entry.path();
        if path.extension().map(|e| e == "jpg").unwrap_or(false) {
            image_count += 1;
        }
    }

    if image_count == 0 {
        return Err(Error::VideoCompilation(
            "No images found in input directory".to_string(),
        ));
    }

    tracing::info!("Compiling {} images into video", image_count);

    // Build ffmpeg command
    let input_pattern = input_dir.join("*.jpg").to_string_lossy().to_string();

    let mut cmd = Command::new("ffmpeg");
    cmd.arg("-y") // Overwrite output
        .arg("-framerate")
        .arg(config.framerate.to_string())
        .arg("-pattern_type")
        .arg("glob")
        .arg("-i")
        .arg(&input_pattern)
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

    // Parse progress from stdout
    let stdout = child.stdout.take().unwrap();
    let mut reader = BufReader::new(stdout).lines();

    while let Some(line) = reader.next_line().await? {
        if line.starts_with("frame=") {
            if let Some(frame_str) = line.strip_prefix("frame=") {
                if let Ok(frame) = frame_str.trim().parse::<u32>() {
                    progress_callback(frame, image_count);
                }
            }
        }
    }

    let status = child.wait().await?;

    if !status.success() {
        return Err(Error::FFmpeg(format!(
            "ffmpeg exited with status {}",
            status
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
