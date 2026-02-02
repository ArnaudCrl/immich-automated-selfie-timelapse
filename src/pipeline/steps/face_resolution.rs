//! Face resolution validation step.
//!
//! Skips images where the detected face is too small.

use crate::config::Config;
use crate::pipeline::{ComputedValue, PipelineContext, ProcessingStep, StepOutcome};
use async_trait::async_trait;

/// Validates that the face bounding box meets minimum size requirements.
///
/// This is a validation step that runs early in the pipeline (before downloading)
/// to quickly skip low-resolution faces.
pub struct FaceResolutionStep;

#[async_trait]
impl ProcessingStep for FaceResolutionStep {
    fn id(&self) -> &'static str {
        "face_resolution"
    }

    fn name(&self) -> &'static str {
        "Face Resolution Check"
    }

    async fn execute(&self, mut ctx: PipelineContext, config: &Config) -> StepOutcome {
        let step_config = &config.processing.face_resolution;

        // Skip this step if disabled
        if !step_config.enabled {
            return StepOutcome::Continue(ctx);
        }

        let face_data = &ctx.face_data;

        // Calculate face size in pixels from bounding box
        let face_width = face_data.bounding_box_x2 - face_data.bounding_box_x1;
        let face_height = face_data.bounding_box_y2 - face_data.bounding_box_y1;
        let face_size = face_width.min(face_height) as i32;

        // Store computed value for later steps or debugging
        ctx.set_computed("face_size", ComputedValue::Int(face_size));

        let threshold = step_config.min_size as i32;

        if face_size < threshold {
            return StepOutcome::Skip {
                ctx,
                reason: "face_too_small".to_string(),
                detail: Some(format!("{}px (threshold: {}px)", face_size, threshold)),
            };
        }

        StepOutcome::Continue(ctx)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::immich_api::FaceData;

    fn make_ctx(face_size: f32) -> PipelineContext {
        let face_data = FaceData {
            bounding_box_x1: 0.0,
            bounding_box_y1: 0.0,
            bounding_box_x2: face_size,
            bounding_box_y2: face_size,
            image_width: 1920,
            image_height: 1080,
        };
        PipelineContext::new("test".to_string(), "2024-01-01".to_string(), face_data)
    }

    #[tokio::test]
    async fn test_face_too_small() {
        let step = FaceResolutionStep;
        let ctx = make_ctx(50.0); // 50px face
        let config = Config::default(); // threshold is 80px

        match step.execute(ctx, &config).await {
            StepOutcome::Skip { reason, detail, .. } => {
                assert_eq!(reason, "face_too_small");
                assert!(detail.unwrap().contains("50px"));
            }
            _ => panic!("Expected Skip"),
        }
    }

    #[tokio::test]
    async fn test_face_large_enough() {
        let step = FaceResolutionStep;
        let ctx = make_ctx(100.0); // 100px face
        let config = Config::default(); // threshold is 80px

        match step.execute(ctx, &config).await {
            StepOutcome::Continue(new_ctx) => {
                // Should have stored face_size
                assert_eq!(
                    new_ctx.get_computed("face_size").and_then(|v| v.as_int()),
                    Some(100)
                );
            }
            _ => panic!("Expected Continue"),
        }
    }

    #[tokio::test]
    async fn test_step_disabled() {
        let step = FaceResolutionStep;
        let ctx = make_ctx(50.0); // Would normally be too small

        let mut config = Config::default();
        config.processing.face_resolution.enabled = false;

        match step.execute(ctx, &config).await {
            StepOutcome::Continue(_) => {} // Should pass through when disabled
            _ => panic!("Expected Continue when disabled"),
        }
    }
}
