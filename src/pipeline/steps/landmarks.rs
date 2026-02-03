//! Facial landmark detection step.
//!
//! Uses dlib to detect 68 facial landmarks for alignment and eye filtering.

use crate::config::Config;
use crate::models::DlibLandmarks;
use crate::pipeline::{ComputedValue, Landmarks, PipelineContext, ProcessingStep, StepOutcome};
use async_trait::async_trait;
use tokio::task;

/// Detects facial landmarks using dlib.
///
/// This step:
/// 1. Uses dlib to detect faces and 68 landmarks
/// 2. Stores Landmarks in ctx.computed["landmarks"]
/// 3. Computes EAR and stores in ctx.computed["ear"]
///
/// Eye filtering (skipping closed eyes) is handled by EyeFilterStep.
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

        let image = match ctx.require_image("landmark detection") {
            Ok(img) => img,
            Err(e) => return StepOutcome::Error(e),
        };

        // Get the global landmark predictor (loaded once, reused for all images)
        let dlib = match DlibLandmarks::global() {
            Ok(d) => d,
            Err(e) => {
                // If model isn't available, skip this step with a warning
                tracing::warn!("Dlib landmarks model not available: {}", e);
                return StepOutcome::Skip {
                    ctx,
                    reason: "landmarks_failed".to_string(),
                    detail: Some(e.to_string()),
                };
            }
        };

        // Convert to RGB for dlib
        let rgb = image.to_rgb8();
        let (width, height) = (rgb.width() as usize, rgb.height() as usize);
        let pixels = rgb.into_raw();

        // Get the face rectangle if available
        let face_rect: Option<(i64, i64, i64, i64)> = ctx
            .get_computed("face_rect")
            .and_then(|v| v.as_face_rect())
            .map(|r| (r.x1 as i64, r.y1 as i64, r.x2 as i64, r.y2 as i64));

        // Run dlib operations in a blocking thread to avoid dropping in async context
        let landmarks_result = task::spawn_blocking(move || -> Result<Landmarks, String> {
            dlib.detect_landmarks(width, height, &pixels, face_rect)
                .map_err(|e| e.to_string())
        })
        .await;

        let landmarks = match landmarks_result {
            Ok(Ok(l)) => l,
            Ok(Err(e)) => {
                return StepOutcome::Skip {
                    ctx,
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

        tracing::trace!(
            "Landmarks detected: EAR left={:.3}, right={:.3}, avg={:.3}",
            ear.left,
            ear.right,
            avg_ear
        );

        StepOutcome::Continue(ctx)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::immich_api::FaceData;
    use image::{DynamicImage, RgbImage};

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
