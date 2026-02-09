//! Blur detection step.
//!
//! Detects blurry faces using gradient magnitude analysis within the face region:
//! 1. Convert image to grayscale
//! 2. Apply Sobel operator (Sobel-X and Sobel-Y) within the face bounding box
//! 3. Compute gradient magnitude for each pixel: sqrt(gx² + gy²)
//! 4. Calculate mean gradient magnitude
//! 5. Low gradient magnitude → blurry face (weak, spread-out edges)

use crate::config::Config;
use crate::pipeline::{
    computed_keys, draw_simple_text, ComputedValue, PipelineContext, ProcessingStep, StepOutcome,
};
use async_trait::async_trait;
use image::{DynamicImage, Rgb};

pub struct BlurStep;

impl BlurStep {
    /// Calculate the mean gradient magnitude of a specific region within an image.
    ///
    /// This method:
    /// 1. Converts the image to grayscale
    /// 2. Applies Sobel-X and Sobel-Y kernels within the specified region:
    ///    Sobel-X: [-1  0  1]    Sobel-Y: [-1 -2 -1]
    ///    [-2  0  2]             [ 0  0  0]
    ///    [-1  0  1]             [ 1  2  1]
    /// 3. Computes gradient magnitude for each pixel: sqrt(gx² + gy²)
    /// 4. Returns the mean gradient magnitude
    ///
    /// Higher gradient magnitude = sharper image (strong, concentrated edges)
    /// Lower gradient magnitude = blurrier image (weak, spread-out edges)
    ///
    /// # Arguments
    /// * `image` - The full image
    /// * `x1`, `y1`, `x2`, `y2` - Bounding box coordinates (pixels, clamped to image bounds)
    fn calculate_gradient_magnitude_in_region(
        image: &DynamicImage,
        x1: u32,
        y1: u32,
        x2: u32,
        y2: u32,
    ) -> f32 {
        let gray = image.to_luma8();
        let (img_width, img_height) = gray.dimensions();

        // Clamp coordinates to image bounds
        let x1 = x1.min(img_width.saturating_sub(1));
        let y1 = y1.min(img_height.saturating_sub(1));
        let x2 = x2.min(img_width);
        let y2 = y2.min(img_height);

        // Ensure valid region with space for 3x3 kernel
        if x2 <= x1 + 2 || y2 <= y1 + 2 {
            return 0.0;
        }

        // Apply Sobel filter within the region and compute gradient magnitudes
        let mut sum_magnitude = 0.0f32;
        let mut count = 0usize;

        for y in (y1 + 1)..(y2 - 1) {
            for x in (x1 + 1)..(x2 - 1) {
                // Get the 3x3 neighborhood
                let p00 = gray.get_pixel(x - 1, y - 1)[0] as i32;
                let p01 = gray.get_pixel(x, y - 1)[0] as i32;
                let p02 = gray.get_pixel(x + 1, y - 1)[0] as i32;
                let p10 = gray.get_pixel(x - 1, y)[0] as i32;
                let p12 = gray.get_pixel(x + 1, y)[0] as i32;
                let p20 = gray.get_pixel(x - 1, y + 1)[0] as i32;
                let p21 = gray.get_pixel(x, y + 1)[0] as i32;
                let p22 = gray.get_pixel(x + 1, y + 1)[0] as i32;

                // Apply Sobel-X kernel: [-1 0 1; -2 0 2; -1 0 1]
                let gx = -p00 + p02 - 2 * p10 + 2 * p12 - p20 + p22;

                // Apply Sobel-Y kernel: [-1 -2 -1; 0 0 0; 1 2 1]
                let gy = -p00 - 2 * p01 - p02 + p20 + 2 * p21 + p22;

                // Compute gradient magnitude: sqrt(gx² + gy²)
                let magnitude = ((gx * gx + gy * gy) as f32).sqrt();
                sum_magnitude += magnitude;
                count += 1;
            }
        }

        if count == 0 {
            return 0.0;
        }

        // Return mean gradient magnitude
        sum_magnitude / count as f32
    }
}

#[async_trait]
impl ProcessingStep for BlurStep {
    fn id(&self) -> &'static str {
        "blur"
    }

    fn name(&self) -> &'static str {
        "Blur Detection"
    }

    fn provides(&self) -> Vec<&'static str> {
        vec![computed_keys::BLUR_METRIC]
    }

    async fn execute(&self, mut ctx: PipelineContext, config: &Config) -> StepOutcome {
        let step_config = &config.processing.blur;

        let image = match ctx.require_image("blur detection") {
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

        let gradient_magnitude =
            Self::calculate_gradient_magnitude_in_region(image, x1, y1, x2, y2);

        // Store computed gradient magnitude for potential use by other steps
        ctx.set_computed(
            computed_keys::BLUR_METRIC,
            ComputedValue::Float(gradient_magnitude),
        );

        if gradient_magnitude < step_config.min_sharpness {
            return StepOutcome::Skip {
                ctx,
                reason: "too_blurry".to_string(),
                detail: Some(format!(
                    "gradient: {:.1} (min: {:.1})",
                    gradient_magnitude, step_config.min_sharpness
                )),
            };
        }

        StepOutcome::Continue(ctx)
    }

    fn debug_visualize(&self, ctx: &PipelineContext, _config: &Config) -> Option<DynamicImage> {
        // Get blur metric (gradient magnitude) from computed values
        let gradient_mag = ctx
            .get_computed(computed_keys::BLUR_METRIC)
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

        // Draw a horizontal gradient magnitude bar at the bottom
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

        // Fill the bar based on gradient magnitude value (scale: 0-50 maps to 0-100% bar)
        let max_gradient = 50.0;
        let normalized = (gradient_mag / max_gradient).clamp(0.0, 1.0);
        let fill_width = ((bar_width - 4) as f32 * normalized) as u32;
        let fill_color = gradient_to_color(gradient_mag);
        for y in (outline_y + 2)..(outline_y + outline_height - 2) {
            for x in (bar_x + 2)..(bar_x + 2 + fill_width) {
                if x < width {
                    debug_img.put_pixel(x, y, fill_color);
                }
            }
        }

        // Draw gradient magnitude text value
        let text = format!("Grad:{:.1}", gradient_mag);
        draw_simple_text(&mut debug_img, 5, bar_y + 6, &text, Rgb([255, 255, 255]));

        Some(DynamicImage::ImageRgb8(debug_img))
    }
}

/// Convert gradient magnitude value to a color (red for blurry, green for sharp)
fn gradient_to_color(gradient_mag: f32) -> Rgb<u8> {
    // Very low gradient (< 10) = red (blurry)
    // Medium gradient (10-20) = yellow/orange
    // High gradient (> 20) = green (sharp)
    if gradient_mag < 10.0 {
        Rgb([255, 80, 80]) // Red
    } else if gradient_mag < 20.0 {
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
            bounding_box_x2: 100.0,
            bounding_box_y2: 100.0,
            image_width: 100,
            image_height: 100,
        };
        PipelineContext::new("test".to_string(), "2024-01-01".to_string(), face_data)
            .with_image(image)
    }

    fn create_solid_image(r: u8, g: u8, b: u8) -> DynamicImage {
        let img = RgbImage::from_fn(100, 100, |_, _| Rgb([r, g, b]));
        DynamicImage::ImageRgb8(img)
    }

    fn create_checkerboard_image() -> DynamicImage {
        // Create a sharp checkerboard pattern (high variance)
        let img = RgbImage::from_fn(100, 100, |x, y| {
            if (x / 10 + y / 10) % 2 == 0 {
                Rgb([255, 255, 255])
            } else {
                Rgb([0, 0, 0])
            }
        });
        DynamicImage::ImageRgb8(img)
    }

    #[test]
    fn test_calculate_gradient_magnitude_uniform() {
        // Uniform image should have very low gradient magnitude (no edges)
        let img = create_solid_image(128, 128, 128);
        let gradient_mag = BlurStep::calculate_gradient_magnitude_in_region(&img, 0, 0, 100, 100);
        assert!(
            gradient_mag < 1.0,
            "Uniform image should have near-zero gradient magnitude"
        );
    }

    #[test]
    fn test_calculate_gradient_magnitude_sharp() {
        // Checkerboard should have high gradient magnitude (many edges)
        let img = create_checkerboard_image();
        let gradient_mag = BlurStep::calculate_gradient_magnitude_in_region(&img, 0, 0, 100, 100);
        assert!(
            gradient_mag > 15.0,
            "Sharp checkerboard should have high gradient magnitude, got {}",
            gradient_mag
        );
    }

    #[tokio::test]
    async fn test_blur_too_blurry() {
        let step = BlurStep;
        let img = create_solid_image(128, 128, 128); // Very low gradient magnitude
        let ctx = make_ctx_with_image(img);

        let mut config = Config::default();
        config.processing.blur.enabled = true;
        config.processing.blur.min_sharpness = 15.0;

        match step.execute(ctx, &config).await {
            StepOutcome::Skip { reason, .. } => {
                assert_eq!(reason, "too_blurry");
            }
            _ => panic!("Expected Skip for blurry image"),
        }
    }

    #[tokio::test]
    async fn test_blur_sharp_image() {
        let step = BlurStep;
        let img = create_checkerboard_image(); // High gradient magnitude
        let ctx = make_ctx_with_image(img);

        let mut config = Config::default();
        config.processing.blur.enabled = true;
        config.processing.blur.min_sharpness = 15.0;

        match step.execute(ctx, &config).await {
            StepOutcome::Continue(_) => {} // Should pass
            _ => panic!("Expected Continue for sharp image"),
        }
    }
}
