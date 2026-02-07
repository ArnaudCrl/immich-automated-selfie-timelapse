//! Face cropping and resizing step.
//!
//! Crops the face region from the full image using the bounding box data,
//! then resizes it to the configured output size.

use crate::config::Config;
use crate::pipeline::crop_face_with_intermediate;
use crate::pipeline::{
    computed_keys, BoundingBox, ComputedValue, PipelineContext, ProcessingStep, StepOutcome,
};
use async_trait::async_trait;

/// Crops the face region from the full image and resizes it.
///
/// This transformer step extracts the face region using the bounding box
/// from Immich (with padding) and immediately resizes it to the configured
/// output size.
pub struct CropAndResizeStep;

#[async_trait]
impl ProcessingStep for CropAndResizeStep {
    fn id(&self) -> &'static str {
        "crop_and_resize"
    }

    fn name(&self) -> &'static str {
        "Crop & Resize"
    }

    async fn execute(&self, mut ctx: PipelineContext, config: &Config) -> StepOutcome {
        let image = match ctx.require_image("cropping and resizing") {
            Ok(img) => img,
            Err(e) => return StepOutcome::Error { ctx, error: e },
        };

        let output_size = config.processing.output.size;
        let eye_distance = config.processing.alignment.eye_distance;

        // Crop returns CropResult with cropped images and face rectangle in crop coordinates
        match crop_face_with_intermediate(image, &ctx.face_data, output_size, eye_distance) {
            Ok(crop_result) => {
                // Use the pre-resized image from the crop function
                let cropped_size = crop_result.cropped.width();
                ctx.image = Some(crop_result.resized);

                // Scale the face rectangle to match the resized image coordinates
                let scale = output_size as f32 / cropped_size as f32;
                let scaled_face_rect = BoundingBox {
                    x1: crop_result.face_rect.x1 * scale,
                    y1: crop_result.face_rect.y1 * scale,
                    x2: crop_result.face_rect.x2 * scale,
                    y2: crop_result.face_rect.y2 * scale,
                };

                // Store the scaled face rectangle for later steps
                ctx.set_computed(
                    computed_keys::FACE_RECT,
                    ComputedValue::FaceRect(scaled_face_rect),
                );

                StepOutcome::Continue(ctx)
            }
            Err(e) => StepOutcome::Skip {
                ctx,
                reason: "crop_failed".to_string(),
                detail: Some(e.to_string()),
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::immich_api::FaceData;
    use image::{DynamicImage, Rgb, RgbImage};

    fn make_ctx_with_image(image: DynamicImage) -> PipelineContext {
        // Face in the center of a 100x100 image
        let face_data = FaceData {
            bounding_box_x1: 30.0,
            bounding_box_y1: 30.0,
            bounding_box_x2: 70.0,
            bounding_box_y2: 70.0,
            image_width: 100,
            image_height: 100,
        };
        PipelineContext::new("test".to_string(), "2024-01-01".to_string(), face_data)
            .with_image(image)
    }

    fn create_test_image(width: u32, height: u32) -> DynamicImage {
        let img = RgbImage::from_fn(width, height, |x, y| {
            // Create a pattern so we can verify cropping
            Rgb([(x % 256) as u8, (y % 256) as u8, 128])
        });
        DynamicImage::ImageRgb8(img)
    }

    #[tokio::test]
    async fn test_crop_and_resize_success() {
        let step = CropAndResizeStep;
        let img = create_test_image(100, 100);
        let ctx = make_ctx_with_image(img);

        let mut config = Config::default();
        config.processing.output.size = 512;

        match step.execute(ctx, &config).await {
            StepOutcome::Continue(new_ctx) => {
                assert!(new_ctx.image.is_some());
                let resized = new_ctx.image.unwrap();
                // Should be resized to the configured output size
                assert_eq!(resized.width(), 512);
                assert_eq!(resized.height(), 512);
            }
            _ => panic!("Expected Continue"),
        }
    }

    #[tokio::test]
    async fn test_crop_and_resize_no_image() {
        let step = CropAndResizeStep;
        let face_data = FaceData {
            bounding_box_x1: 30.0,
            bounding_box_y1: 30.0,
            bounding_box_x2: 70.0,
            bounding_box_y2: 70.0,
            image_width: 100,
            image_height: 100,
        };
        let ctx = PipelineContext::new("test".to_string(), "2024-01-01".to_string(), face_data);
        let config = Config::default();

        match step.execute(ctx, &config).await {
            StepOutcome::Error { error, .. } => {
                assert!(error.contains("No image"));
            }
            _ => panic!("Expected Error"),
        }
    }

    #[tokio::test]
    async fn test_crop_and_resize_different_sizes() {
        let step = CropAndResizeStep;
        let img = create_test_image(200, 200);
        let ctx = make_ctx_with_image(img);

        let mut config = Config::default();
        config.processing.output.size = 256;

        match step.execute(ctx, &config).await {
            StepOutcome::Continue(new_ctx) => {
                let resized = new_ctx.image.unwrap();
                assert_eq!(resized.width(), 256);
                assert_eq!(resized.height(), 256);
            }
            _ => panic!("Expected Continue"),
        }
    }
}
