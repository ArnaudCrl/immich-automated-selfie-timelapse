//! Facial landmark detection step.
//!
//! Uses dlib to detect 68 facial landmarks for alignment and eye filtering.

use crate::config::Config;
use crate::face_processing::types::Landmarks;
use crate::models::DlibLandmarks;
use crate::pipeline::{ComputedValue, PipelineContext, ProcessingStep, StepOutcome};
use async_trait::async_trait;
use image::{DynamicImage, Rgb, RgbImage};
use tokio::task;

/// Detects facial landmarks and optionally filters based on eye aspect ratio.
///
/// This step:
/// 1. Uses dlib to detect faces and 68 landmarks
/// 2. Stores Landmarks in ctx.computed["landmarks"]
/// 3. Computes EAR and stores in ctx.computed["ear"]
/// 4. Optionally skips if EAR is below threshold (eyes closed)
pub struct LandmarksStep;

#[async_trait]
impl ProcessingStep for LandmarksStep {
    fn id(&self) -> &'static str {
        "landmarks"
    }

    fn name(&self) -> &'static str {
        "Landmarks"
    }

    async fn execute(&self, mut ctx: PipelineContext, config: &Config) -> StepOutcome {
        // We always need landmarks if alignment is enabled, even if eye filter is disabled
        let need_landmarks =
            config.processing.alignment.enabled || config.processing.eye_filter.enabled;

        if !need_landmarks {
            return StepOutcome::Continue(ctx);
        }

        let image = match &ctx.image {
            Some(img) => img,
            None => {
                return StepOutcome::Error(
                    "No image available for landmark detection".to_string(),
                );
            }
        };

        // Get the global landmark predictor (loaded once, reused for all images)
        let dlib = match DlibLandmarks::global() {
            Ok(d) => d,
            Err(e) => {
                // If model isn't available, skip this step with a warning
                tracing::warn!("Dlib landmarks model not available: {}", e);
                return StepOutcome::Skip {
                    reason: "landmarks_failed".to_string(),
                    detail: Some(e.to_string()),
                };
            }
        };

        // Convert to RGB for dlib
        let rgb = image.to_rgb8();
        let (width, height) = (rgb.width() as usize, rgb.height() as usize);
        let pixels = rgb.into_raw();

        // Run dlib operations in a blocking thread to avoid dropping in async context
        let landmarks_result = task::spawn_blocking(move || -> Result<Landmarks, String> {
            dlib.detect_landmarks(width, height, &pixels)
                .map_err(|e| e.to_string())
        })
        .await;

        let landmarks = match landmarks_result {
            Ok(Ok(l)) => l,
            Ok(Err(e)) => {
                return StepOutcome::Skip {
                    reason: "landmarks_failed".to_string(),
                    detail: Some(e),
                };
            }
            Err(e) => {
                return StepOutcome::Error(format!("Landmark detection task failed: {}", e));
            }
        };

        // Compute and store EAR
        let ear = landmarks.eye_aspect_ratio();
        let avg_ear = (ear.left + ear.right) / 2.0;
        ctx.set_computed("ear", ComputedValue::Float(avg_ear));

        // Store landmarks
        ctx.set_computed("landmarks", ComputedValue::Landmarks(Box::new(landmarks)));

        // Check eye filter if enabled
        if config.processing.eye_filter.enabled {
            let min_ear = config.processing.eye_filter.min_ear;
            if avg_ear < min_ear {
                return StepOutcome::Skip {
                    reason: "eyes_closed".to_string(),
                    detail: Some(format!("EAR {:.3} below threshold {:.3}", avg_ear, min_ear)),
                };
            }
        }

        tracing::trace!(
            "Landmarks detected: EAR left={:.3}, right={:.3}, avg={:.3}",
            ear.left,
            ear.right,
            avg_ear
        );

        StepOutcome::Continue(ctx)
    }

    fn debug_visualize(&self, ctx: &PipelineContext) -> Option<DynamicImage> {
        // Get landmarks from computed values
        let landmarks: &Landmarks = ctx
            .get_computed("landmarks")
            .and_then(|v| v.as_landmarks())?;

        // Get the current image to draw on
        let image = ctx.image.as_ref()?;
        let mut debug_img = image.to_rgb8();

        // Draw all 68 landmark points
        let points = landmarks.points();
        for (i, point) in points.iter().enumerate() {
            let x = point.x as u32;
            let y = point.y as u32;

            // Color-code different facial regions
            let color = match i {
                0..=16 => Rgb([255, 0, 0]),    // Jaw (red)
                17..=21 => Rgb([0, 255, 0]),   // Left eyebrow (green)
                22..=26 => Rgb([0, 255, 0]),   // Right eyebrow (green)
                27..=35 => Rgb([0, 0, 255]),   // Nose (blue)
                36..=41 => Rgb([255, 255, 0]), // Left eye (yellow)
                42..=47 => Rgb([255, 255, 0]), // Right eye (yellow)
                48..=67 => Rgb([255, 0, 255]), // Mouth (magenta)
                _ => Rgb([255, 255, 255]),     // Other (white)
            };

            // Draw a small cross at each point
            draw_cross(&mut debug_img, x, y, color);
        }

        // Draw eye centers
        let left_eye = landmarks.left_eye_center();
        let right_eye = landmarks.right_eye_center();
        draw_cross(
            &mut debug_img,
            left_eye.x as u32,
            left_eye.y as u32,
            Rgb([0, 255, 255]),
        );
        draw_cross(
            &mut debug_img,
            right_eye.x as u32,
            right_eye.y as u32,
            Rgb([0, 255, 255]),
        );

        Some(DynamicImage::ImageRgb8(debug_img))
    }
}

/// Draw a small cross at the given position.
fn draw_cross(img: &mut RgbImage, x: u32, y: u32, color: Rgb<u8>) {
    let (width, height) = (img.width(), img.height());
    let size = 2;

    for dx in 0..=size * 2 {
        let px = (x as i32 + dx as i32 - size as i32) as u32;
        if px < width {
            img.put_pixel(px, y, color);
        }
    }
    for dy in 0..=size * 2 {
        let py = (y as i32 + dy as i32 - size as i32) as u32;
        if py < height {
            img.put_pixel(x, py, color);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::immich_api::FaceData;

    fn make_test_ctx() -> PipelineContext {
        let face_data = FaceData {
            bounding_box_x1: 0.0,
            bounding_box_y1: 0.0,
            bounding_box_x2: 100.0,
            bounding_box_y2: 100.0,
            image_width: 100,
            image_height: 100,
        };
        PipelineContext::new("test".to_string(), "2024-01-01".to_string(), face_data)
    }

    #[tokio::test]
    async fn test_disabled_skips_check() {
        let step = LandmarksStep;
        let ctx = make_test_ctx();
        let mut config = Config::default();
        config.processing.alignment.enabled = false;
        config.processing.eye_filter.enabled = false;

        // Create a dummy image
        let img = DynamicImage::ImageRgb8(RgbImage::new(100, 100));
        let ctx = ctx.with_image(img);

        match step.execute(ctx, &config).await {
            StepOutcome::Continue(_) => {} // Expected
            other => panic!("Expected Continue when disabled, got {:?}", other),
        }
    }

    #[tokio::test]
    async fn test_no_image_error() {
        let step = LandmarksStep;
        let ctx = make_test_ctx();
        let mut config = Config::default();
        config.processing.alignment.enabled = true;

        match step.execute(ctx, &config).await {
            StepOutcome::Error(msg) => {
                assert!(msg.contains("No image"));
            }
            other => panic!("Expected Error, got {:?}", other),
        }
    }
}
