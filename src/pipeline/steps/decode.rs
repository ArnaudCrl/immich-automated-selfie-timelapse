//! Image decode step.
//!
//! Decodes raw image bytes and applies EXIF orientation correction.

use crate::config::Config;
use crate::pipeline::load_image_with_orientation;
use crate::pipeline::{PipelineContext, ProcessingStep, StepOutcome};
use async_trait::async_trait;

/// Decodes image bytes and corrects EXIF orientation.
///
/// This step expects `ctx.raw_bytes` to be set (from a previous download step
/// or passed in directly). It sets `ctx.image` with the decoded image.
pub struct DecodeImageStep;

#[async_trait]
impl ProcessingStep for DecodeImageStep {
    fn id(&self) -> &'static str {
        "decode"
    }

    fn name(&self) -> &'static str {
        "Image Decode"
    }

    async fn execute(&self, mut ctx: PipelineContext, _config: &Config) -> StepOutcome {
        let raw_bytes = match &ctx.raw_bytes {
            Some(bytes) => bytes,
            None => {
                return StepOutcome::Error {
                    ctx,
                    error: "No raw bytes available for decoding".to_string(),
                };
            }
        };

        // Decode image with EXIF orientation correction
        match load_image_with_orientation(raw_bytes) {
            Ok(image) => {
                ctx.image = Some(image);
                StepOutcome::Continue(ctx)
            }
            Err(e) => StepOutcome::Skip {
                ctx,
                reason: "decode_failed".to_string(),
                detail: Some(e.to_string()),
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bytes::Bytes;
    use crate::immich_api::FaceData;
    use image::{DynamicImage, RgbImage};
    use std::io::Cursor;

    fn make_test_jpeg() -> Bytes {
        // Create a small test image
        let img = RgbImage::from_fn(10, 10, |_, _| image::Rgb([128, 128, 128]));
        let dynamic = DynamicImage::ImageRgb8(img);

        let mut buffer = Cursor::new(Vec::new());
        dynamic.write_to(&mut buffer, image::ImageFormat::Jpeg).unwrap();

        Bytes::from(buffer.into_inner())
    }

    fn make_ctx_with_bytes(bytes: Bytes) -> PipelineContext {
        let face_data = FaceData {
            bounding_box_x1: 0.0,
            bounding_box_y1: 0.0,
            bounding_box_x2: 10.0,
            bounding_box_y2: 10.0,
            image_width: 10,
            image_height: 10,
        };
        PipelineContext::new("test".to_string(), "2024-01-01".to_string(), face_data)
            .with_bytes(bytes)
    }

    #[tokio::test]
    async fn test_decode_valid_jpeg() {
        let step = DecodeImageStep;
        let bytes = make_test_jpeg();
        let ctx = make_ctx_with_bytes(bytes);
        let config = Config::default();

        match step.execute(ctx, &config).await {
            StepOutcome::Continue(new_ctx) => {
                assert!(new_ctx.image.is_some());
            }
            _ => panic!("Expected Continue"),
        }
    }

    #[tokio::test]
    async fn test_decode_invalid_bytes() {
        let step = DecodeImageStep;
        let bytes = Bytes::from_static(b"not an image");
        let ctx = make_ctx_with_bytes(bytes);
        let config = Config::default();

        match step.execute(ctx, &config).await {
            StepOutcome::Skip { reason, .. } => {
                assert_eq!(reason, "decode_failed");
            }
            _ => panic!("Expected Skip"),
        }
    }

    #[tokio::test]
    async fn test_decode_no_bytes() {
        let step = DecodeImageStep;
        let face_data = FaceData {
            bounding_box_x1: 0.0,
            bounding_box_y1: 0.0,
            bounding_box_x2: 10.0,
            bounding_box_y2: 10.0,
            image_width: 10,
            image_height: 10,
        };
        let ctx = PipelineContext::new("test".to_string(), "2024-01-01".to_string(), face_data);
        let config = Config::default();

        match step.execute(ctx, &config).await {
            StepOutcome::Error { error, .. } => {
                assert!(error.contains("No raw bytes"));
            }
            _ => panic!("Expected Error"),
        }
    }
}
