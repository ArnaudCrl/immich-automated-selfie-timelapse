//! Brightness validation step.
//!
//! Calculates average image brightness and skips images that are too dark or too bright.

use crate::config::Config;
use crate::pipeline::{ComputedValue, PipelineContext, ProcessingStep, StepOutcome};
use async_trait::async_trait;
use image::DynamicImage;

/// Validates image brightness and skips images outside acceptable range.
///
/// This is a validation step that computes the average luminance of the image
/// and skips if it falls below `min_brightness` or above `max_brightness`.
///
/// Brightness is normalized to 0.0-1.0 range.
pub struct BrightnessStep;

impl BrightnessStep {
    /// Calculate the average brightness of an image.
    ///
    /// Returns a value between 0.0 (pure black) and 1.0 (pure white).
    fn calculate_brightness(image: &DynamicImage) -> f32 {
        let rgb = image.to_rgb8();
        let pixels = rgb.pixels();
        let pixel_count = rgb.width() as u64 * rgb.height() as u64;

        if pixel_count == 0 {
            return 0.0;
        }

        // Calculate average luminance using standard RGB to luminance conversion
        // Y = 0.299*R + 0.587*G + 0.114*B
        let total_luminance: u64 = pixels
            .map(|p| {
                let [r, g, b] = p.0;
                // Use integer math for speed, then convert
                (299 * r as u64 + 587 * g as u64 + 114 * b as u64) / 1000
            })
            .sum();

        (total_luminance as f32 / pixel_count as f32) / 255.0
    }
}

#[async_trait]
impl ProcessingStep for BrightnessStep {
    fn id(&self) -> &'static str {
        "brightness"
    }

    fn name(&self) -> &'static str {
        "Brightness Check"
    }

    async fn execute(&self, mut ctx: PipelineContext, config: &Config) -> StepOutcome {
        let step_config = &config.processing.brightness;

        let image = match &ctx.image {
            Some(img) => img,
            None => {
                return StepOutcome::Error(
                    "No image available for brightness check".to_string(),
                );
            }
        };

        let brightness = Self::calculate_brightness(image);

        // Store computed brightness for potential use by other steps
        ctx.set_computed("brightness", ComputedValue::Float(brightness));

        // Skip validation if disabled
        if !step_config.enabled {
            return StepOutcome::Continue(ctx);
        }

        if brightness < step_config.min_brightness {
            return StepOutcome::Skip {
                reason: "too_dark".to_string(),
                detail: Some(format!(
                    "{:.2} (min: {:.2})",
                    brightness, step_config.min_brightness
                )),
            };
        }

        if brightness > step_config.max_brightness {
            return StepOutcome::Skip {
                reason: "too_bright".to_string(),
                detail: Some(format!(
                    "{:.2} (max: {:.2})",
                    brightness, step_config.max_brightness
                )),
            };
        }

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
            bounding_box_x2: 10.0,
            bounding_box_y2: 10.0,
            image_width: 10,
            image_height: 10,
        };
        PipelineContext::new("test".to_string(), "2024-01-01".to_string(), face_data)
            .with_image(image)
    }

    fn create_solid_image(r: u8, g: u8, b: u8) -> DynamicImage {
        let img = RgbImage::from_fn(10, 10, |_, _| Rgb([r, g, b]));
        DynamicImage::ImageRgb8(img)
    }

    #[test]
    fn test_calculate_brightness_black() {
        let img = create_solid_image(0, 0, 0);
        let brightness = BrightnessStep::calculate_brightness(&img);
        assert!((brightness - 0.0).abs() < 0.01);
    }

    #[test]
    fn test_calculate_brightness_white() {
        let img = create_solid_image(255, 255, 255);
        let brightness = BrightnessStep::calculate_brightness(&img);
        assert!((brightness - 1.0).abs() < 0.01);
    }

    #[test]
    fn test_calculate_brightness_gray() {
        let img = create_solid_image(128, 128, 128);
        let brightness = BrightnessStep::calculate_brightness(&img);
        // Should be approximately 0.5
        assert!(brightness > 0.45 && brightness < 0.55);
    }

    #[tokio::test]
    async fn test_brightness_disabled() {
        let step = BrightnessStep;
        let img = create_solid_image(128, 128, 128);
        let ctx = make_ctx_with_image(img);
        let config = Config::default(); // brightness.enabled = false by default

        match step.execute(ctx, &config).await {
            StepOutcome::Continue(new_ctx) => {
                // Should still compute brightness even when disabled
                let brightness = new_ctx.get_computed("brightness")
                    .and_then(|v| v.as_float())
                    .unwrap();
                assert!(brightness > 0.45 && brightness < 0.55);
            }
            _ => panic!("Expected Continue"),
        }
    }

    #[tokio::test]
    async fn test_brightness_too_dark() {
        let step = BrightnessStep;
        let img = create_solid_image(10, 10, 10); // Very dark
        let ctx = make_ctx_with_image(img);

        let mut config = Config::default();
        config.processing.brightness.enabled = true;
        config.processing.brightness.min_brightness = 0.2;
        config.processing.brightness.max_brightness = 0.9;

        match step.execute(ctx, &config).await {
            StepOutcome::Skip { reason, .. } => {
                assert_eq!(reason, "too_dark");
            }
            _ => panic!("Expected Skip"),
        }
    }

    #[tokio::test]
    async fn test_brightness_too_bright() {
        let step = BrightnessStep;
        let img = create_solid_image(250, 250, 250); // Very bright
        let ctx = make_ctx_with_image(img);

        let mut config = Config::default();
        config.processing.brightness.enabled = true;
        config.processing.brightness.min_brightness = 0.1;
        config.processing.brightness.max_brightness = 0.9;

        match step.execute(ctx, &config).await {
            StepOutcome::Skip { reason, .. } => {
                assert_eq!(reason, "too_bright");
            }
            _ => panic!("Expected Skip"),
        }
    }

    #[tokio::test]
    async fn test_brightness_within_range() {
        let step = BrightnessStep;
        let img = create_solid_image(128, 128, 128); // Mid-gray
        let ctx = make_ctx_with_image(img);

        let mut config = Config::default();
        config.processing.brightness.enabled = true;
        config.processing.brightness.min_brightness = 0.1;
        config.processing.brightness.max_brightness = 0.9;

        match step.execute(ctx, &config).await {
            StepOutcome::Continue(_) => {} // Should pass
            _ => panic!("Expected Continue for mid-range brightness"),
        }
    }
}
