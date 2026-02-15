//! Blur detection step.
//!
//! Detects blurry faces using Laplacian variance analysis within the face region:
//! 1. Convert image to grayscale
//! 2. Apply Laplacian operator within the face bounding box
//! 3. Compute variance of the Laplacian response across all pixels
//! 4. Low variance → blurry image (weak edge response, values clustered near zero)
//! 5. High variance → sharp image (strong edges mixed with smooth regions)

use crate::config::Config;
use crate::pipeline::{
    computed_keys, draw_simple_text, face_rect_pixels, ComputedValue, PipelineContext,
    ProcessingStep, StepOutcome,
};
use async_trait::async_trait;
use image::{DynamicImage, Rgb};

pub struct BlurStep;

impl BlurStep {
    /// Calculate the Laplacian variance of a specific region within an image.
    ///
    /// This method:
    /// 1. Converts the image to grayscale
    /// 2. Applies the discrete Laplacian kernel within the specified region:
    ///    [ 0  1  0]
    ///    [ 1 -4  1]
    ///    [ 0  1  0]
    /// 3. Computes the variance of the Laplacian response across all pixels
    ///
    /// Variance is used rather than mean because:
    /// - Blurry images: Laplacian values all near zero → low variance
    /// - Sharp images: strong responses at edges, near-zero on smooth skin → high variance
    /// - Mean can be near-zero for both (positive and negative Laplacian values cancel out)
    ///
    /// # Arguments
    /// * `image` - The full image
    /// * `x1`, `y1`, `x2`, `y2` - Bounding box coordinates (pixels, clamped to image bounds)
    fn calculate_laplacian_variance_in_region(
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

        // Apply Laplacian kernel and collect responses
        // Kernel: [0, 1, 0; 1, -4, 1; 0, 1, 0]
        let mut values = Vec::with_capacity(((x2 - x1) * (y2 - y1)) as usize);

        for y in (y1 + 1)..(y2 - 1) {
            for x in (x1 + 1)..(x2 - 1) {
                let center = gray.get_pixel(x, y)[0] as i32;
                let top = gray.get_pixel(x, y - 1)[0] as i32;
                let bottom = gray.get_pixel(x, y + 1)[0] as i32;
                let left = gray.get_pixel(x - 1, y)[0] as i32;
                let right = gray.get_pixel(x + 1, y)[0] as i32;

                let laplacian = (top + bottom + left + right - 4 * center) as f32;
                values.push(laplacian);
            }
        }

        if values.is_empty() {
            return 0.0;
        }

        // Compute variance: E[X²] - E[X]²
        let n = values.len() as f32;
        let mean = values.iter().sum::<f32>() / n;
        let variance = values.iter().map(|v| (v - mean).powi(2)).sum::<f32>() / n;
        variance
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

        let (img_width, img_height) = (image.width(), image.height());
        let (x1, y1, x2, y2) = face_rect_pixels(&ctx, img_width, img_height);

        let laplacian_variance =
            Self::calculate_laplacian_variance_in_region(image, x1, y1, x2, y2);

        // Normalize for upscaling only.
        // When the crop was upscaled to output_size (scale > 1), Lanczos3 smooths edges
        // and artificially reduces sharpness metrics. For variance, the correction is
        // scale² (variance of k·X = k²·variance(X)), so we multiply by scale².
        //
        // We do NOT correct for downscaling (scale < 1): downsampling doesn't
        // meaningfully inflate variance, and dividing by scale² < 1 would penalize
        // images where a large crop was taken, producing false blurry rejections.
        let crop_scale = ctx
            .get_computed(computed_keys::CROP_SCALE)
            .and_then(|v| v.as_float())
            .unwrap_or(1.0);
        let scale_factor = crop_scale.max(1.0);
        let normalized_variance = laplacian_variance * scale_factor * scale_factor;

        // Store computed variance for potential use by other steps
        ctx.set_computed(
            computed_keys::BLUR_METRIC,
            ComputedValue::Float(normalized_variance),
        );

        if normalized_variance < step_config.min_sharpness {
            return StepOutcome::Skip {
                ctx,
                reason: "too_blurry".to_string(),
                detail: Some(format!(
                    "laplacian_var: {:.1} (raw: {:.1}, scale: {:.2}, min: {:.1})",
                    normalized_variance, laplacian_variance, crop_scale, step_config.min_sharpness
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

        let (x1, y1, x2, y2) = face_rect_pixels(ctx, width, height);

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

        // Fill the bar based on Laplacian variance (log scale: 1–10000 maps to 0–100%)
        // Using log scale because variance spans several orders of magnitude.
        let log_variance = (gradient_mag + 1.0).ln();
        let log_max = (10000.0f32 + 1.0).ln();
        let normalized = (log_variance / log_max).clamp(0.0, 1.0);
        let fill_width = ((bar_width - 4) as f32 * normalized) as u32;
        let fill_color = variance_to_color(gradient_mag);
        for y in (outline_y + 2)..(outline_y + outline_height - 2) {
            for x in (bar_x + 2)..(bar_x + 2 + fill_width) {
                if x < width {
                    debug_img.put_pixel(x, y, fill_color);
                }
            }
        }

        // Draw Laplacian variance text value
        let text = format!("{:.0}", gradient_mag);
        draw_simple_text(&mut debug_img, 5, bar_y + 6, &text, Rgb([255, 255, 255]));

        Some(DynamicImage::ImageRgb8(debug_img))
    }
}

/// Convert Laplacian variance to a color (red for blurry, green for sharp).
/// Typical ranges: < 100 = blurry, 100–500 = borderline, > 500 = sharp.
fn variance_to_color(variance: f32) -> Rgb<u8> {
    if variance < 100.0 {
        Rgb([255, 80, 80]) // Red (blurry)
    } else if variance < 500.0 {
        Rgb([255, 200, 80]) // Yellow/orange (borderline)
    } else {
        Rgb([80, 255, 80]) // Green (sharp)
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
    fn test_laplacian_variance_uniform() {
        // Uniform image: Laplacian is zero everywhere → variance is zero
        let img = create_solid_image(128, 128, 128);
        let variance = BlurStep::calculate_laplacian_variance_in_region(&img, 0, 0, 100, 100);
        assert!(
            variance < 1.0,
            "Uniform image should have near-zero Laplacian variance, got {variance}"
        );
    }

    #[test]
    fn test_laplacian_variance_sharp() {
        // Checkerboard: strong Laplacian response at edges → high variance
        let img = create_checkerboard_image();
        let variance = BlurStep::calculate_laplacian_variance_in_region(&img, 0, 0, 100, 100);
        assert!(
            variance > 100.0,
            "Sharp checkerboard should have high Laplacian variance, got {variance}"
        );
    }

    #[tokio::test]
    async fn test_blur_too_blurry() {
        let step = BlurStep;
        let img = create_solid_image(128, 128, 128); // Zero Laplacian variance
        let ctx = make_ctx_with_image(img);

        let mut config = Config::default();
        config.processing.blur.enabled = true;
        config.processing.blur.min_sharpness = 50.0;

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
        let img = create_checkerboard_image(); // High Laplacian variance
        let ctx = make_ctx_with_image(img);

        let mut config = Config::default();
        config.processing.blur.enabled = true;
        config.processing.blur.min_sharpness = 50.0;

        match step.execute(ctx, &config).await {
            StepOutcome::Continue(_) => {} // Should pass
            _ => panic!("Expected Continue for sharp image"),
        }
    }
}
