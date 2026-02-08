//! Brightness validation step.
//!
//! Calculates average brightness within the face region and skips images that are too
//! dark or too bright. This step runs after cropping, analyzing the face region within
//! the cropped image.

use crate::config::Config;
use crate::pipeline::{
    computed_keys, draw_simple_text, ComputedValue, PipelineContext, ProcessingStep, StepOutcome,
};
use async_trait::async_trait;
use image::{DynamicImage, Rgb};

/// Validates image brightness and skips images outside acceptable range.
///
/// This is a validation step that computes the average luminance of the image
/// and skips if it falls below `min_brightness` or above `max_brightness`.
///
/// Brightness is normalized to 0.0-1.0 range.
pub struct BrightnessStep;

impl BrightnessStep {
    /// Calculate the average brightness of a specific region within an image.
    ///
    /// Returns a value between 0.0 (pure black) and 1.0 (pure white).
    ///
    /// # Arguments
    /// * `image` - The full image
    /// * `x1`, `y1`, `x2`, `y2` - Bounding box coordinates (pixels, clamped to image bounds)
    fn calculate_brightness_in_region(
        image: &DynamicImage,
        x1: u32,
        y1: u32,
        x2: u32,
        y2: u32,
    ) -> f32 {
        let rgb = image.to_rgb8();
        let (img_width, img_height) = (rgb.width(), rgb.height());

        // Clamp coordinates to image bounds
        let x1 = x1.min(img_width.saturating_sub(1));
        let y1 = y1.min(img_height.saturating_sub(1));
        let x2 = x2.min(img_width);
        let y2 = y2.min(img_height);

        // Ensure valid region
        if x2 <= x1 || y2 <= y1 {
            return 0.0;
        }

        let width = x2 - x1;
        let height = y2 - y1;
        let pixel_count = width as u64 * height as u64;

        if pixel_count == 0 {
            return 0.0;
        }

        // Calculate average luminance using standard RGB to luminance conversion
        // Y = 0.299*R + 0.587*G + 0.114*B
        let mut total_luminance: u64 = 0;
        for y in y1..y2 {
            for x in x1..x2 {
                let pixel = rgb.get_pixel(x, y);
                let [r, g, b] = pixel.0;
                total_luminance += (299 * r as u64 + 587 * g as u64 + 114 * b as u64) / 1000;
            }
        }

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

    fn provides(&self) -> Vec<&'static str> {
        vec![computed_keys::BRIGHTNESS]
    }

    async fn execute(&self, mut ctx: PipelineContext, config: &Config) -> StepOutcome {
        let step_config = &config.processing.brightness;

        let image = match ctx.require_image("brightness check") {
            Ok(img) => img,
            Err(e) => return StepOutcome::Error { ctx, error: e },
        };

        let img_width = image.width();
        let img_height = image.height();

        // Use FACE_RECT if available (face region within the cropped image),
        // otherwise analyze the entire cropped image
        let (x1, y1, x2, y2) = if let Some(face_rect) = ctx
            .get_computed(computed_keys::FACE_RECT)
            .and_then(|v| v.as_face_rect())
        {
            // Use the face rectangle within the cropped image
            let x1 = (face_rect.x1.max(0.0) as u32).min(img_width.saturating_sub(1));
            let y1 = (face_rect.y1.max(0.0) as u32).min(img_height.saturating_sub(1));
            let x2 = (face_rect.x2.max(0.0) as u32).min(img_width);
            let y2 = (face_rect.y2.max(0.0) as u32).min(img_height);
            (x1, y1, x2, y2)
        } else {
            // No face rect available, use the entire cropped image
            (0, 0, img_width, img_height)
        };

        let brightness = Self::calculate_brightness_in_region(image, x1, y1, x2, y2);

        // Store computed brightness for potential use by other steps
        ctx.set_computed(computed_keys::BRIGHTNESS, ComputedValue::Float(brightness));

        // Skip validation if disabled
        if !step_config.enabled {
            return StepOutcome::Continue(ctx);
        }

        if brightness < step_config.min_brightness {
            return StepOutcome::Skip {
                ctx,
                reason: "too_dark".to_string(),
                detail: Some(format!(
                    "{:.2} (min: {:.2})",
                    brightness, step_config.min_brightness
                )),
            };
        }

        if brightness > step_config.max_brightness {
            return StepOutcome::Skip {
                ctx,
                reason: "too_bright".to_string(),
                detail: Some(format!(
                    "{:.2} (max: {:.2})",
                    brightness, step_config.max_brightness
                )),
            };
        }

        StepOutcome::Continue(ctx)
    }

    fn debug_visualize(&self, ctx: &PipelineContext, _config: &Config) -> Option<DynamicImage> {
        // Get brightness from computed values
        let brightness = ctx
            .get_computed(computed_keys::BRIGHTNESS)
            .and_then(|v| v.as_float())?;

        // Get the current image to draw on
        let image = ctx.image.as_ref()?;
        let rgb = image.to_rgb8();
        let (width, height) = (rgb.width(), rgb.height());

        // Create a copy for visualization
        let mut debug_img = rgb.clone();

        // Draw the face bounding box (FACE_RECT if available, otherwise full image)
        let (x1, y1, x2, y2) = if let Some(face_rect) = ctx
            .get_computed(computed_keys::FACE_RECT)
            .and_then(|v| v.as_face_rect())
        {
            let x1 = (face_rect.x1.max(0.0) as u32).min(width.saturating_sub(1));
            let y1 = (face_rect.y1.max(0.0) as u32).min(height.saturating_sub(1));
            let x2 = (face_rect.x2.max(0.0) as u32).min(width);
            let y2 = (face_rect.y2.max(0.0) as u32).min(height);
            (x1, y1, x2, y2)
        } else {
            // No face rect, show that we analyzed the full cropped image
            (0, 0, width, height)
        };

        // Draw rectangle outline (cyan color for visibility)
        let rect_color = Rgb([0, 255, 255]);
        let thickness = 2;

        // Draw horizontal lines (top and bottom)
        for t in 0..thickness {
            for x in x1..x2 {
                if y1 + t < height {
                    debug_img.put_pixel(x, y1 + t, rect_color);
                }
                if y2 > t && y2 - t - 1 < height {
                    debug_img.put_pixel(x, y2 - t - 1, rect_color);
                }
            }
        }

        // Draw vertical lines (left and right)
        for t in 0..thickness {
            for y in y1..y2 {
                if x1 + t < width {
                    debug_img.put_pixel(x1 + t, y, rect_color);
                }
                if x2 > t && x2 - t - 1 < width {
                    debug_img.put_pixel(x2 - t - 1, y, rect_color);
                }
            }
        }

        // Draw a horizontal brightness bar at the bottom
        let bar_height = 20u32;
        let bar_y = height.saturating_sub(bar_height);
        let bar_width = (width as f32 * 0.8) as u32;
        let bar_x = (width - bar_width) / 2;

        // Draw background (dark gray)
        for y in bar_y..height {
            for x in 0..width {
                debug_img.put_pixel(x, y, Rgb([40, 40, 40]));
            }
        }

        // Draw bar outline (white)
        let outline_y = bar_y + 4;
        let outline_height = bar_height - 8;
        for x in bar_x..bar_x + bar_width {
            debug_img.put_pixel(x, outline_y, Rgb([200, 200, 200]));
            debug_img.put_pixel(x, outline_y + outline_height - 1, Rgb([200, 200, 200]));
        }
        for y in outline_y..outline_y + outline_height {
            debug_img.put_pixel(bar_x, y, Rgb([200, 200, 200]));
            debug_img.put_pixel(bar_x + bar_width - 1, y, Rgb([200, 200, 200]));
        }

        // Fill the bar based on brightness value
        let fill_width = ((bar_width - 4) as f32 * brightness.clamp(0.0, 1.0)) as u32;
        let fill_color = brightness_to_color(brightness);
        for y in (outline_y + 2)..(outline_y + outline_height - 2) {
            for x in (bar_x + 2)..(bar_x + 2 + fill_width) {
                if x < width {
                    debug_img.put_pixel(x, y, fill_color);
                }
            }
        }

        // Draw brightness text value
        let text = format!("B:{:.2}", brightness);
        draw_simple_text(&mut debug_img, 5, bar_y + 6, &text, Rgb([255, 255, 255]));

        Some(DynamicImage::ImageRgb8(debug_img))
    }
}

/// Convert brightness value to a color (red for dark/bright, green for good)
fn brightness_to_color(brightness: f32) -> Rgb<u8> {
    // Very dark or very bright = red, middle range = green
    if !(0.15..=0.85).contains(&brightness) {
        Rgb([255, 80, 80]) // Red
    } else if !(0.25..=0.75).contains(&brightness) {
        Rgb([255, 200, 80]) // Yellow/orange
    } else {
        Rgb([80, 255, 80]) // Green
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::immich_api::FaceData;
    use image::{DynamicImage, Rgb, RgbImage};

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
        let brightness = BrightnessStep::calculate_brightness_in_region(&img, 0, 0, 10, 10);
        assert!((brightness - 0.0).abs() < 0.01);
    }

    #[test]
    fn test_calculate_brightness_white() {
        let img = create_solid_image(255, 255, 255);
        let brightness = BrightnessStep::calculate_brightness_in_region(&img, 0, 0, 10, 10);
        assert!((brightness - 1.0).abs() < 0.01);
    }

    #[test]
    fn test_calculate_brightness_gray() {
        let img = create_solid_image(128, 128, 128);
        let brightness = BrightnessStep::calculate_brightness_in_region(&img, 0, 0, 10, 10);
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
                let brightness = new_ctx
                    .get_computed(computed_keys::BRIGHTNESS)
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
