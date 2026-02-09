//! Eye aspect ratio (EAR) filter step.
//!
//! Computes and filters images based on eye openness using the Eye Aspect Ratio
//! from facial landmarks.

use crate::config::Config;
use crate::pipeline::{
    computed_keys, draw_simple_text, ComputedValue, Landmarks, PipelineContext, ProcessingStep,
    StepOutcome,
};
use async_trait::async_trait;
use image::{DynamicImage, Rgb, RgbImage};

/// Filters images where eyes appear closed based on Eye Aspect Ratio.
///
/// This validator step computes the EAR from landmarks and skips images
/// where the average EAR is below the configured threshold.
///
/// Must run after LandmarksStep.
pub struct EyeFilterStep;

#[async_trait]
impl ProcessingStep for EyeFilterStep {
    fn id(&self) -> &'static str {
        "eye_filter"
    }

    fn name(&self) -> &'static str {
        "Eye Filter"
    }

    async fn execute(&self, mut ctx: PipelineContext, config: &Config) -> StepOutcome {
        // Get landmarks from computed values (set by LandmarksStep)
        let landmarks = match ctx
            .get_computed(computed_keys::LANDMARKS)
            .and_then(|v| v.as_landmarks())
        {
            Some(lm) => lm,
            None => {
                // No landmarks available - landmarks step must have been skipped
                tracing::warn!("Landmarks not available for eye filter, skipping check");
                return StepOutcome::Continue(ctx);
            }
        };

        // Compute EAR from landmarks
        let ear = landmarks.eye_aspect_ratio();
        let avg_ear = (ear.left + ear.right) / 2.0;

        // Store EAR for potential use by other steps or debug visualization
        ctx.set_computed(computed_keys::EAR, ComputedValue::Float(avg_ear));

        let min_ear = config.processing.eye_filter.min_ear;

        if avg_ear < min_ear {
            return StepOutcome::Skip {
                ctx,
                reason: "eyes_closed".to_string(),
                detail: Some(format!("EAR {:.3} below threshold {:.3}", avg_ear, min_ear)),
            };
        }

        tracing::trace!("Eye filter passed: EAR {:.3} >= {:.3}", avg_ear, min_ear);

        StepOutcome::Continue(ctx)
    }

    fn debug_visualize(&self, ctx: &PipelineContext, _config: &Config) -> Option<DynamicImage> {
        // Get landmarks for eye visualization
        let landmarks: &Landmarks = ctx
            .get_computed(computed_keys::LANDMARKS)
            .and_then(|v| v.as_landmarks())?;

        // Get EAR values (computed during execute)
        let ear = landmarks.eye_aspect_ratio();
        let avg_ear = ctx
            .get_computed(computed_keys::EAR)
            .and_then(|v| v.as_float())
            .unwrap_or_else(|| (ear.left + ear.right) / 2.0);

        // Get the current image to draw on
        let image = ctx.image.as_ref()?;
        let rgb = image.to_rgb8();
        let (width, height) = (rgb.width(), rgb.height());

        // Create a copy for visualization
        let mut debug_img = rgb.clone();

        // Draw eye landmarks (points 36-47)
        let points = landmarks.points();

        // Left eye (points 36-41) - yellow or red depending on EAR
        let left_color = if ear.left >= 0.2 {
            Rgb([0, 255, 0]) // Green - open
        } else {
            Rgb([255, 0, 0]) // Red - closed
        };

        for point in points.iter().skip(36).take(6) {
            draw_cross(&mut debug_img, point.x as u32, point.y as u32, left_color);
        }

        // Right eye (points 42-47)
        let right_color = if ear.right >= 0.2 {
            Rgb([0, 255, 0]) // Green - open
        } else {
            Rgb([255, 0, 0]) // Red - closed
        };

        for point in points.iter().skip(42).take(6) {
            draw_cross(&mut debug_img, point.x as u32, point.y as u32, right_color);
        }

        // Draw eye centers
        let left_eye = landmarks.left_eye_center();
        let right_eye = landmarks.right_eye_center();
        draw_marker(
            &mut debug_img,
            left_eye.x as u32,
            left_eye.y as u32,
            Rgb([0, 255, 255]),
        );
        draw_marker(
            &mut debug_img,
            right_eye.x as u32,
            right_eye.y as u32,
            Rgb([0, 255, 255]),
        );

        // Draw info bar at bottom
        let bar_height = 20u32;
        let bar_y = height.saturating_sub(bar_height);

        // Draw background
        for y in bar_y..height {
            for x in 0..width {
                debug_img.put_pixel(x, y, Rgb([0, 0, 0]));
            }
        }

        // Draw EAR values
        let text = format!("L:{:.2} R:{:.2} Avg:{:.2}", ear.left, ear.right, avg_ear);
        draw_simple_text(&mut debug_img, 5, bar_y + 6, &text, Rgb([255, 255, 255]));

        Some(DynamicImage::ImageRgb8(debug_img))
    }
}

/// Draw a small cross at the given position.
fn draw_cross(img: &mut RgbImage, x: u32, y: u32, color: Rgb<u8>) {
    let (width, height) = (img.width(), img.height());
    let size: i32 = 2;

    // Check base coordinates are in bounds
    if x >= width || y >= height {
        return;
    }

    for dx in 0..=size * 2 {
        let px = (x as i32 + dx - size) as u32;
        if px < width && y < height {
            img.put_pixel(px, y, color);
        }
    }
    for dy in 0..=size * 2 {
        let py = (y as i32 + dy - size) as u32;
        if x < width && py < height {
            img.put_pixel(x, py, color);
        }
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
    async fn test_no_landmarks_continues() {
        let step = EyeFilterStep;
        let ctx = make_test_ctx(); // No landmarks set
        let mut config = Config::default();
        config.processing.eye_filter.enabled = true;

        match step.execute(ctx, &config).await {
            StepOutcome::Continue(_) => {} // Expected - gracefully handles missing landmarks
            other => panic!("Expected Continue when no landmarks, got {:?}", other),
        }
    }

    // Note: Tests for EAR filtering would require creating mock Landmarks objects,
    // which is complex due to the dlib integration. The filtering logic is tested
    // through integration tests instead.
}
