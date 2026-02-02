//! Face cropping step.
//!
//! Crops the face region from the full image using the bounding box data.

use crate::config::Config;
use crate::face_processing::crop_face_with_intermediate;
use crate::face_processing::debug::draw_crop_debug;
use crate::pipeline::{ComputedValue, PipelineContext, ProcessingStep, StepOutcome};
use async_trait::async_trait;
use image::DynamicImage;

/// Crops the face region from the full image.
///
/// This transformer step extracts the face region using the bounding box
/// from Immich, adding padding around the face for context.
pub struct CropFaceStep;

#[async_trait]
impl ProcessingStep for CropFaceStep {
    fn id(&self) -> &'static str {
        "crop"
    }

    fn name(&self) -> &'static str {
        "Face Crop"
    }

    async fn execute(&self, mut ctx: PipelineContext, config: &Config) -> StepOutcome {
        let image = match &ctx.image {
            Some(img) => img,
            None => {
                return StepOutcome::Error("No image available for cropping".to_string());
            }
        };

        // Crop returns CropResult with cropped images and face rectangle in crop coordinates.
        // We use the full_res_crop here and let the resize step handle the final sizing.
        match crop_face_with_intermediate(image, &ctx.face_data, config.processing.output.size) {
            Ok(crop_result) => {
                // Use the full-resolution cropped image; resize step will handle final size
                ctx.image = Some(crop_result.cropped);
                // Store the face rectangle in crop coordinates for later steps
                ctx.set_computed("face_rect", ComputedValue::FaceRect(crop_result.face_rect));
                StepOutcome::Continue(ctx)
            }
            Err(e) => StepOutcome::Skip {
                ctx,
                reason: "crop_failed".to_string(),
                detail: Some(e.to_string()),
            },
        }
    }

    fn debug_visualize(&self, ctx: &PipelineContext) -> Option<DynamicImage> {
        // Draw the crop region on the original image
        // Note: This requires access to the original image before cropping,
        // which we don't have here. For now, we'll create the debug image
        // during execution if needed. This is a limitation of the current
        // design that could be addressed by storing the original image.

        // For now, return None and handle debug visualization in the pipeline
        // execution or via a separate mechanism
        ctx.raw_bytes.as_ref()?;

        // If we had the original image, we could do:
        // Some(draw_crop_debug(&original, &ctx.face_data))
        None
    }
}

/// Generates a debug visualization of the crop region.
///
/// This can be called separately before the crop step to visualize
/// what will be cropped.
#[allow(dead_code)]
pub fn generate_crop_debug(image: &DynamicImage, ctx: &PipelineContext) -> DynamicImage {
    draw_crop_debug(image, &ctx.face_data)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::immich_api::FaceData;
    use image::{DynamicImage, RgbImage, Rgb};

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
    async fn test_crop_success() {
        let step = CropFaceStep;
        let img = create_test_image(100, 100);
        let ctx = make_ctx_with_image(img);
        let config = Config::default();

        match step.execute(ctx, &config).await {
            StepOutcome::Continue(new_ctx) => {
                assert!(new_ctx.image.is_some());
                // The cropped image should be square
                let cropped = new_ctx.image.unwrap();
                assert_eq!(cropped.width(), cropped.height());
            }
            _ => panic!("Expected Continue"),
        }
    }

    #[tokio::test]
    async fn test_crop_no_image() {
        let step = CropFaceStep;
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
            StepOutcome::Error(msg) => {
                assert!(msg.contains("No image"));
            }
            _ => panic!("Expected Error"),
        }
    }
}
