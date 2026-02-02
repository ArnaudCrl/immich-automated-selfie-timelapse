//! Image resize step.
//!
//! Resizes the cropped face image to the configured output size.

use crate::config::Config;
use crate::pipeline::{PipelineContext, ProcessingStep, StepOutcome};
use async_trait::async_trait;
use image::imageops::FilterType;

/// Resizes the image to the configured output size.
///
/// This transformer step is typically the last step in the pipeline,
/// producing the final output image at the configured resolution.
pub struct ResizeStep;

#[async_trait]
impl ProcessingStep for ResizeStep {
    fn id(&self) -> &'static str {
        "resize"
    }

    fn name(&self) -> &'static str {
        "Resize"
    }

    async fn execute(&self, mut ctx: PipelineContext, config: &Config) -> StepOutcome {
        let image = match ctx.take_image("resizing") {
            Ok(img) => img,
            Err(e) => return StepOutcome::Error(e),
        };

        let output_size = config.processing.output.size;

        // Resize to square output
        let resized = image.resize_exact(output_size, output_size, FilterType::Lanczos3);

        ctx.image = Some(resized);
        StepOutcome::Continue(ctx)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::immich_api::FaceData;
    use image::{DynamicImage, RgbImage, Rgb};

    fn make_ctx_with_image(image: DynamicImage) -> PipelineContext {
        let face_data = FaceData {
            bounding_box_x1: 0.0,
            bounding_box_y1: 0.0,
            bounding_box_x2: 100.0,
            bounding_box_y2: 100.0,
            image_width: 100,
            image_height: 100,
        };
        PipelineContext::new("test".to_string(), "2024-01-01".to_string(), face_data)
            .with_image(image)
    }

    fn create_test_image(width: u32, height: u32) -> DynamicImage {
        let img = RgbImage::from_fn(width, height, |_, _| Rgb([128, 128, 128]));
        DynamicImage::ImageRgb8(img)
    }

    #[tokio::test]
    async fn test_resize_success() {
        let step = ResizeStep;
        let img = create_test_image(200, 200);
        let ctx = make_ctx_with_image(img);

        let mut config = Config::default();
        config.processing.output.size = 512;

        match step.execute(ctx, &config).await {
            StepOutcome::Continue(new_ctx) => {
                let resized = new_ctx.image.unwrap();
                assert_eq!(resized.width(), 512);
                assert_eq!(resized.height(), 512);
            }
            _ => panic!("Expected Continue"),
        }
    }

    #[tokio::test]
    async fn test_resize_no_image() {
        let step = ResizeStep;
        let face_data = FaceData {
            bounding_box_x1: 0.0,
            bounding_box_y1: 0.0,
            bounding_box_x2: 100.0,
            bounding_box_y2: 100.0,
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

    #[tokio::test]
    async fn test_resize_upscale() {
        let step = ResizeStep;
        let img = create_test_image(50, 50); // Small image
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
