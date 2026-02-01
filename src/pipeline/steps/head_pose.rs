//! Head pose estimation step.
//!
//! Uses the DMHead ONNX model to estimate head pose (yaw, pitch, roll) and
//! filter out non-front-facing faces.

use crate::config::Config;
use crate::models::DMHeadModel;
use crate::pipeline::{ComputedValue, PipelineContext, ProcessingStep, StepOutcome};
use async_trait::async_trait;
use image::{DynamicImage, Rgb};

/// Estimates head pose and filters non-frontal faces.
///
/// This step:
/// 1. Runs DMHead inference on the cropped face image
/// 2. Stores the HeadPose result in ctx.computed["head_pose"]
/// 3. Skips if any angle exceeds configured thresholds
pub struct HeadPoseStep;

#[async_trait]
impl ProcessingStep for HeadPoseStep {
    fn id(&self) -> &'static str {
        "head_pose"
    }

    fn name(&self) -> &'static str {
        "Head Pose"
    }

    async fn execute(&self, mut ctx: PipelineContext, config: &Config) -> StepOutcome {
        // Skip if head pose filtering is disabled
        if !config.processing.head_pose.enabled {
            return StepOutcome::Continue(ctx);
        }

        let image = match &ctx.image {
            Some(img) => img,
            None => {
                return StepOutcome::Error("No image available for head pose estimation".to_string());
            }
        };

        // Load the DMHead model
        let model = match DMHeadModel::global() {
            Ok(m) => m,
            Err(e) => {
                // If model isn't available, skip this step with a warning
                tracing::warn!("DMHead model not available, skipping head pose check: {}", e);
                return StepOutcome::Continue(ctx);
            }
        };

        // Run inference
        let pose = match model.estimate(image) {
            Ok(p) => p,
            Err(e) => {
                return StepOutcome::Error(format!("Head pose estimation failed: {}", e));
            }
        };

        // Store pose in computed values
        ctx.set_computed("head_pose", ComputedValue::HeadPose(pose));

        // Check against thresholds
        let head_pose_config = &config.processing.head_pose;

        tracing::debug!(
            "Head pose detected: yaw={:.1}°, pitch={:.1}°, roll={:.1}°",
            pose.yaw,
            pose.pitch,
            pose.roll
        );

        if pose.yaw.abs() > head_pose_config.max_yaw {
            return StepOutcome::Skip {
                reason: "head_turned".to_string(),
                detail: Some(format!(
                    "Yaw {:.1}° exceeds threshold {:.1}°",
                    pose.yaw, head_pose_config.max_yaw
                )),
            };
        }

        if pose.pitch.abs() > head_pose_config.max_pitch {
            return StepOutcome::Skip {
                reason: "head_turned".to_string(),
                detail: Some(format!(
                    "Pitch {:.1}° exceeds threshold {:.1}°",
                    pose.pitch, head_pose_config.max_pitch
                )),
            };
        }

        if pose.roll.abs() > head_pose_config.max_roll {
            return StepOutcome::Skip {
                reason: "head_turned".to_string(),
                detail: Some(format!(
                    "Roll {:.1}° exceeds threshold {:.1}°",
                    pose.roll, head_pose_config.max_roll
                )),
            };
        }

        StepOutcome::Continue(ctx)
    }

    fn debug_visualize(&self, ctx: &PipelineContext) -> Option<DynamicImage> {
        // Get head pose from computed values
        let pose = ctx
            .get_computed("head_pose")
            .and_then(|v| v.as_head_pose())?;

        // Get the current image to draw on
        let image = ctx.image.as_ref()?;
        let rgb = image.to_rgb8();
        let (width, height) = (rgb.width(), rgb.height());

        // Create a copy for visualization
        let mut debug_img = rgb.clone();

        // Draw pose info as text overlay
        // For simplicity, we'll draw colored bars indicating pose angles
        // Green = within range, Red = out of range

        // Draw yaw indicator (horizontal bar at top)
        let yaw_pos = ((pose.yaw / 90.0 + 1.0) / 2.0 * width as f32) as u32;
        let yaw_pos = yaw_pos.min(width - 1);
        for x in 0..width {
            let color = if x == yaw_pos {
                Rgb([255, 255, 0]) // Yellow marker
            } else if x == width / 2 {
                Rgb([0, 255, 0]) // Green center
            } else {
                Rgb([50, 50, 50]) // Dark background
            };
            for y in 0..5 {
                if y < height {
                    debug_img.put_pixel(x, y, color);
                }
            }
        }

        // Draw pitch indicator (vertical bar on left)
        let pitch_pos = ((pose.pitch / 90.0 + 1.0) / 2.0 * height as f32) as u32;
        let pitch_pos = pitch_pos.min(height - 1);
        for y in 0..height {
            let color = if y == pitch_pos {
                Rgb([255, 255, 0]) // Yellow marker
            } else if y == height / 2 {
                Rgb([0, 255, 0]) // Green center
            } else {
                Rgb([50, 50, 50]) // Dark background
            };
            for x in 0..5 {
                debug_img.put_pixel(x, y, color);
            }
        }

        Some(DynamicImage::ImageRgb8(debug_img))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::immich_api::FaceData;
    use image::RgbImage;

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
        let step = HeadPoseStep;
        let ctx = make_test_ctx();
        let mut config = Config::default();
        config.processing.head_pose.enabled = false;

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
        let step = HeadPoseStep;
        let ctx = make_test_ctx();
        let mut config = Config::default();
        config.processing.head_pose.enabled = true;

        match step.execute(ctx, &config).await {
            StepOutcome::Error(msg) => {
                assert!(msg.contains("No image"));
            }
            other => panic!("Expected Error, got {:?}", other),
        }
    }
}
