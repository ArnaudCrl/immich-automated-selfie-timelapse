//! Face cropping and resizing step.
//!
//! Crops the face region from the full image using the bounding box data,
//! then resizes it to the configured output size.

use crate::config::Config;
use crate::pipeline::crop_face_with_intermediate;
use crate::pipeline::debug_utils::draw_simple_text;
use crate::pipeline::{
    computed_keys, BoundingBox, ComputedValue, PipelineContext, ProcessingStep, StepOutcome,
};
use async_trait::async_trait;
use image::{DynamicImage, Rgb};

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
                // Store padding info for debug visualization
                ctx.set_computed(
                    computed_keys::PADDING_EDGES,
                    ComputedValue::PaddingEdges(crop_result.padding_edges),
                );

                // Check if too much of the crop falls outside the image
                let crop_config = &config.processing.crop;
                if crop_config.enabled {
                    let padding_pct = crop_result.padding_fraction * 100.0;
                    if padding_pct > crop_config.max_padding_percent {
                        // Set image so debug_visualize can use it
                        ctx.image = Some(crop_result.resized);
                        return StepOutcome::Skip {
                            ctx,
                            reason: "excessive_padding".to_string(),
                            detail: Some(format!(
                                "padding {:.1}% exceeds max {:.1}%",
                                padding_pct, crop_config.max_padding_percent
                            )),
                        };
                    }
                }

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

    fn debug_visualize(&self, ctx: &PipelineContext, config: &Config) -> Option<DynamicImage> {
        let edges = ctx
            .get_computed(computed_keys::PADDING_EDGES)
            .and_then(|v| v.as_padding_edges())?;
        let image = ctx.image.as_ref()?;
        let rgb = image.to_rgb8();
        let (width, height) = (rgb.width(), rgb.height());

        let mut debug_img = rgb.clone();

        // Tint padded regions with a semi-transparent red overlay
        let tint = |pixel: &Rgb<u8>| -> Rgb<u8> {
            Rgb([
                (pixel[0] as u16 / 2 + 127).min(255) as u8,
                pixel[1] / 2,
                pixel[2] / 2,
            ])
        };

        let left_px = (edges.left * width as f32).round() as u32;
        let right_px = (edges.right * width as f32).round() as u32;
        let top_px = (edges.top * height as f32).round() as u32;
        let bottom_px = (edges.bottom * height as f32).round() as u32;

        // Tint left edge
        for y in 0..height {
            for x in 0..left_px.min(width) {
                debug_img.put_pixel(x, y, tint(debug_img.get_pixel(x, y)));
            }
        }
        // Tint right edge
        for y in 0..height {
            for x in width.saturating_sub(right_px)..width {
                debug_img.put_pixel(x, y, tint(debug_img.get_pixel(x, y)));
            }
        }
        // Tint top edge (only the non-corner part to avoid double-tinting)
        for y in 0..top_px.min(height) {
            for x in left_px.min(width)..width.saturating_sub(right_px) {
                debug_img.put_pixel(x, y, tint(debug_img.get_pixel(x, y)));
            }
        }
        // Tint bottom edge (only the non-corner part)
        for y in height.saturating_sub(bottom_px)..height {
            for x in left_px.min(width)..width.saturating_sub(right_px) {
                debug_img.put_pixel(x, y, tint(debug_img.get_pixel(x, y)));
            }
        }

        // Draw padding percentage bar at the bottom
        let total_pct = edges.total_fraction() * 100.0;
        let max_pct = config.processing.crop.max_padding_percent;

        let bar_height = 20u32;
        let bar_y = height.saturating_sub(bar_height);
        let bar_width = (width as f32 * 0.8) as u32;
        let bar_x = (width - bar_width) / 2;

        // Background
        for y in bar_y..height {
            for x in 0..width {
                debug_img.put_pixel(x, y, Rgb([40, 40, 40]));
            }
        }

        // Bar outline
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

        // Fill bar (scale: 0-50% maps to full bar)
        let max_scale = 50.0_f32;
        let normalized = (total_pct / max_scale).clamp(0.0, 1.0);
        let fill_width = ((bar_width - 4) as f32 * normalized) as u32;
        let fill_color = if total_pct > max_pct {
            Rgb([255, 80, 80]) // Red - exceeds threshold
        } else if total_pct > max_pct * 0.7 {
            Rgb([255, 200, 80]) // Yellow - approaching threshold
        } else {
            Rgb([80, 255, 80]) // Green - well within threshold
        };
        for y in (outline_y + 2)..(outline_y + outline_height - 2) {
            for x in (bar_x + 2)..(bar_x + 2 + fill_width) {
                if x < width {
                    debug_img.put_pixel(x, y, fill_color);
                }
            }
        }

        // Draw threshold marker on the bar
        let threshold_x =
            bar_x + 2 + ((bar_width - 4) as f32 * (max_pct / max_scale).clamp(0.0, 1.0)) as u32;
        if threshold_x < bar_x + bar_width {
            for y in outline_y..(outline_y + outline_height) {
                debug_img.put_pixel(threshold_x, y, Rgb([255, 255, 255]));
            }
        }

        // Text label
        let text = format!("{:.1}%", total_pct);
        draw_simple_text(&mut debug_img, 5, bar_y + 6, &text, Rgb([255, 255, 255]));

        Some(DynamicImage::ImageRgb8(debug_img))
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
