//! Eye-based face alignment step.
//!
//! Aligns faces based on eye positions to ensure consistent eye placement
//! across all images in the timelapse.

use crate::config::Config;
use crate::face_processing::types::Point;
use crate::pipeline::{PipelineContext, ProcessingStep, StepOutcome};
use async_trait::async_trait;
use image::{DynamicImage, GenericImageView, Rgb, RgbImage};
use imageproc::geometric_transformations::{rotate_about_center, Interpolation};

/// Aligns faces based on eye positions.
///
/// This step:
/// 1. Retrieves landmarks from ctx.computed["landmarks"]
/// 2. Calculates rotation angle from eye positions
/// 3. Applies affine transformation to align eyes horizontally
/// 4. Scales and crops to position eyes at configured positions
pub struct AlignmentStep;

#[async_trait]
impl ProcessingStep for AlignmentStep {
    fn id(&self) -> &'static str {
        "alignment"
    }

    fn name(&self) -> &'static str {
        "Alignment"
    }

    async fn execute(&self, mut ctx: PipelineContext, config: &Config) -> StepOutcome {
        // Skip if alignment is disabled
        if !config.processing.alignment.enabled {
            return StepOutcome::Continue(ctx);
        }

        // Get landmarks from previous step
        let landmarks: crate::face_processing::types::Landmarks = match ctx
            .get_computed("landmarks")
            .and_then(|v| v.as_landmarks())
        {
            Some(l) => l.clone(),
            None => {
                // If landmarks aren't available, skip alignment but continue
                tracing::warn!("Landmarks not available, skipping alignment");
                return StepOutcome::Continue(ctx);
            }
        };

        let image = match ctx.take_image("alignment") {
            Ok(img) => img,
            Err(e) => return StepOutcome::Error(e),
        };

        let (width, height) = image.dimensions();
        let output_size = config.processing.output.size;

        // Get eye centers
        let left_eye = landmarks.left_eye_center();
        let right_eye = landmarks.right_eye_center();

        // Calculate rotation angle to make eyes horizontal
        let angle = landmarks.eye_rotation_angle();

        // Calculate current inter-eye distance
        let current_eye_dist = landmarks.inter_eye_distance();

        // Target inter-eye distance based on config (as fraction of output width)
        let target_eye_dist = output_size as f32 * config.processing.alignment.inter_eye_distance;

        // Calculate scale factor
        let scale = target_eye_dist / current_eye_dist;

        // Target eye positions
        let target_eye_y = output_size as f32 * config.processing.alignment.eye_y_position;
        let target_left_eye_x = (output_size as f32 - target_eye_dist) / 2.0;
        let _target_right_eye_x = target_left_eye_x + target_eye_dist;

        // Eye center (midpoint between eyes)
        let eye_center = Point::new(
            (left_eye.x + right_eye.x) / 2.0,
            (left_eye.y + right_eye.y) / 2.0,
        );

        // First, rotate the image to make eyes horizontal
        let rgb = image.to_rgb8();
        let rotated = rotate_about_center(
            &rgb,
            -angle, // Negative because we want to counter-rotate
            Interpolation::Bilinear,
            Rgb([0, 0, 0]), // Black background for rotated areas
        );

        // After rotation, the eye center moves. Calculate new position.
        // For small angles, we can approximate that the center stays roughly the same
        // For more accuracy, we'd need to transform the point through the rotation

        // Calculate the new eye center after rotation
        let cos_a = angle.cos();
        let sin_a = angle.sin();
        let cx = width as f32 / 2.0;
        let cy = height as f32 / 2.0;

        // Rotate eye_center around image center
        let dx = eye_center.x - cx;
        let dy = eye_center.y - cy;
        let rotated_eye_center = Point::new(
            cx + dx * cos_a + dy * sin_a,
            cy - dx * sin_a + dy * cos_a,
        );

        // Now calculate crop region to achieve the desired scale and positioning
        // We want the eye center at (output_size/2, target_eye_y)
        let target_center_x = output_size as f32 / 2.0;
        let _target_center_y = target_eye_y;

        // Calculate crop region in the rotated image
        // The crop should be (output_size / scale) pixels, centered appropriately
        let crop_size = (output_size as f32 / scale) as u32;

        // Crop center in source image (accounting for where we want eyes to end up)
        let crop_center_x = rotated_eye_center.x - (target_center_x - output_size as f32 / 2.0) / scale;
        let crop_center_y = rotated_eye_center.y + (target_eye_y - output_size as f32 / 2.0) / scale;

        // Calculate crop bounds
        let crop_x = (crop_center_x - crop_size as f32 / 2.0).max(0.0) as u32;
        let crop_y = (crop_center_y - crop_size as f32 / 2.0).max(0.0) as u32;

        // Clamp to image bounds
        let (rot_width, rot_height) = (rotated.width(), rotated.height());
        let crop_x = crop_x.min(rot_width.saturating_sub(crop_size));
        let crop_y = crop_y.min(rot_height.saturating_sub(crop_size));
        let actual_crop_size = crop_size.min(rot_width - crop_x).min(rot_height - crop_y);

        // Crop and resize
        let rotated_dyn = DynamicImage::ImageRgb8(rotated);
        let cropped = rotated_dyn.crop_imm(crop_x, crop_y, actual_crop_size, actual_crop_size);
        let aligned = cropped.resize_exact(
            output_size,
            output_size,
            image::imageops::FilterType::Lanczos3,
        );

        ctx.image = Some(aligned);

        tracing::trace!(
            "Aligned: rotation={:.2}deg, scale={:.2}, crop={}x{} at ({},{})",
            angle.to_degrees(),
            scale,
            actual_crop_size,
            actual_crop_size,
            crop_x,
            crop_y
        );

        StepOutcome::Continue(ctx)
    }

    fn debug_visualize(&self, ctx: &PipelineContext) -> Option<DynamicImage> {
        // Get landmarks for visualization
        let landmarks: &crate::face_processing::types::Landmarks =
            ctx.get_computed("landmarks").and_then(|v| v.as_landmarks())?;
        let image = ctx.image.as_ref()?;

        let mut debug_img = image.to_rgb8();
        let (width, height) = (debug_img.width(), debug_img.height());

        // Draw target eye positions
        let output_size = width; // Assuming square output
        let eye_y = (output_size as f32 * 0.35) as u32; // Default eye_y_position

        // Draw horizontal line at target eye Y position
        for x in 0..width {
            if eye_y < height {
                debug_img.put_pixel(x, eye_y, Rgb([0, 255, 0]));
            }
        }

        // Draw vertical lines at target eye X positions (assuming 0.3 inter_eye_distance)
        let inter_eye = (output_size as f32 * 0.3) as u32;
        let left_x = (width - inter_eye) / 2;
        let right_x = left_x + inter_eye;

        for y in 0..height {
            if left_x < width {
                debug_img.put_pixel(left_x, y, Rgb([0, 255, 0]));
            }
            if right_x < width {
                debug_img.put_pixel(right_x, y, Rgb([0, 255, 0]));
            }
        }

        // Draw actual eye positions
        let left_eye = landmarks.left_eye_center();
        let right_eye = landmarks.right_eye_center();

        draw_marker(&mut debug_img, left_eye.x as u32, left_eye.y as u32, Rgb([255, 0, 0]));
        draw_marker(&mut debug_img, right_eye.x as u32, right_eye.y as u32, Rgb([255, 0, 0]));

        Some(DynamicImage::ImageRgb8(debug_img))
    }
}

/// Draw a marker (small filled square) at the given position.
fn draw_marker(img: &mut RgbImage, x: u32, y: u32, color: Rgb<u8>) {
    let (width, height) = (img.width(), img.height());
    let size: i32 = 3;

    for dy in 0..=size * 2 {
        for dx in 0..=size * 2 {
            let px = (x as i32 + dx - size) as u32;
            let py = (y as i32 + dy - size) as u32;
            if px < width && py < height {
                img.put_pixel(px, py, color);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::immich_api::FaceData;

    fn make_test_ctx() -> PipelineContext {
        let face_data = FaceData {
            bounding_box_x1: 0.0,
            bounding_box_y1: 0.0,
            bounding_box_x2: 100.0,
            bounding_box_y2: 100.0,
            image_width: 100,
            image_height: 100,
        };
        PipelineContext::new("test".to_string(), "2024-01-01".to_string(), face_data)
    }

    #[tokio::test]
    async fn test_disabled_skips_alignment() {
        let step = AlignmentStep;
        let ctx = make_test_ctx();
        let mut config = Config::default();
        config.processing.alignment.enabled = false;

        // Create a dummy image
        let img = DynamicImage::ImageRgb8(RgbImage::new(100, 100));
        let ctx = ctx.with_image(img);

        match step.execute(ctx, &config).await {
            StepOutcome::Continue(new_ctx) => {
                assert!(new_ctx.image.is_some());
            }
            other => panic!("Expected Continue when disabled, got {:?}", other),
        }
    }

    #[tokio::test]
    async fn test_no_landmarks_continues() {
        let step = AlignmentStep;
        let ctx = make_test_ctx();
        let mut config = Config::default();
        config.processing.alignment.enabled = true;

        // Create a dummy image but no landmarks
        let img = DynamicImage::ImageRgb8(RgbImage::new(100, 100));
        let ctx = ctx.with_image(img);

        match step.execute(ctx, &config).await {
            StepOutcome::Continue(_) => {} // Expected - continues without alignment
            other => panic!("Expected Continue without landmarks, got {:?}", other),
        }
    }
}
