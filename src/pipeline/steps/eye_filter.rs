//! Eye aspect ratio (EAR) filter step.
//!
//! Filters images based on eye openness using the Eye Aspect Ratio computed
//! from facial landmarks.

use crate::config::Config;
use crate::pipeline::{Landmarks, PipelineContext, ProcessingStep, StepOutcome};
use async_trait::async_trait;
use image::{DynamicImage, Rgb, RgbImage};

/// Filters images where eyes appear closed based on Eye Aspect Ratio.
///
/// This validator step reads the EAR value computed by LandmarksStep
/// and skips images where the average EAR is below the configured threshold.
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

    async fn execute(&self, ctx: PipelineContext, config: &Config) -> StepOutcome {
        // Skip if eye filtering is disabled
        if !config.processing.eye_filter.enabled {
            return StepOutcome::Continue(ctx);
        }

        // Get EAR from computed values (set by LandmarksStep)
        let avg_ear = match ctx.get_computed("ear").and_then(|v| v.as_float()) {
            Some(ear) => ear,
            None => {
                // No EAR available - landmarks step must have been skipped
                tracing::warn!("EAR not available for eye filter, skipping check");
                return StepOutcome::Continue(ctx);
            }
        };

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
            .get_computed("landmarks")
            .and_then(|v| v.as_landmarks())?;

        // Get EAR values
        let ear = landmarks.eye_aspect_ratio();
        let avg_ear = (ear.left + ear.right) / 2.0;

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

        for i in 36..42 {
            let point = &points[i];
            draw_cross(&mut debug_img, point.x as u32, point.y as u32, left_color);
        }

        // Right eye (points 42-47)
        let right_color = if ear.right >= 0.2 {
            Rgb([0, 255, 0]) // Green - open
        } else {
            Rgb([255, 0, 0]) // Red - closed
        };

        for i in 42..48 {
            let point = &points[i];
            draw_cross(&mut debug_img, point.x as u32, point.y as u32, right_color);
        }

        // Draw eye centers
        let left_eye = landmarks.left_eye_center();
        let right_eye = landmarks.right_eye_center();
        draw_marker(&mut debug_img, left_eye.x as u32, left_eye.y as u32, Rgb([0, 255, 255]));
        draw_marker(&mut debug_img, right_eye.x as u32, right_eye.y as u32, Rgb([0, 255, 255]));

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
        let text = format!(
            "L:{:.2} R:{:.2} Avg:{:.2}",
            ear.left, ear.right, avg_ear
        );
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

/// Draw simple text using a basic 5x7 pixel font.
fn draw_simple_text(img: &mut RgbImage, x: u32, y: u32, text: &str, color: Rgb<u8>) {
    let (width, height) = (img.width(), img.height());
    let mut cursor_x = x;

    for ch in text.chars() {
        let pattern = get_char_pattern(ch);
        for (row_idx, row) in pattern.iter().enumerate() {
            for col in 0..5 {
                if (row >> (4 - col)) & 1 == 1 {
                    let px = cursor_x + col;
                    let py = y + row_idx as u32;
                    if px < width && py < height {
                        img.put_pixel(px, py, color);
                    }
                }
            }
        }
        cursor_x += 6; // 5 pixels wide + 1 pixel spacing
    }
}

/// Get a 5x7 pixel pattern for a character.
fn get_char_pattern(ch: char) -> [u8; 7] {
    match ch {
        '0' => [0b01110, 0b10001, 0b10011, 0b10101, 0b11001, 0b10001, 0b01110],
        '1' => [0b00100, 0b01100, 0b00100, 0b00100, 0b00100, 0b00100, 0b01110],
        '2' => [0b01110, 0b10001, 0b00001, 0b00110, 0b01000, 0b10000, 0b11111],
        '3' => [0b01110, 0b10001, 0b00001, 0b00110, 0b00001, 0b10001, 0b01110],
        '4' => [0b00010, 0b00110, 0b01010, 0b10010, 0b11111, 0b00010, 0b00010],
        '5' => [0b11111, 0b10000, 0b11110, 0b00001, 0b00001, 0b10001, 0b01110],
        '6' => [0b00110, 0b01000, 0b10000, 0b11110, 0b10001, 0b10001, 0b01110],
        '7' => [0b11111, 0b00001, 0b00010, 0b00100, 0b01000, 0b01000, 0b01000],
        '8' => [0b01110, 0b10001, 0b10001, 0b01110, 0b10001, 0b10001, 0b01110],
        '9' => [0b01110, 0b10001, 0b10001, 0b01111, 0b00001, 0b00010, 0b01100],
        'L' => [0b10000, 0b10000, 0b10000, 0b10000, 0b10000, 0b10000, 0b11111],
        'R' => [0b11110, 0b10001, 0b10001, 0b11110, 0b10100, 0b10010, 0b10001],
        'A' => [0b01110, 0b10001, 0b10001, 0b11111, 0b10001, 0b10001, 0b10001],
        'v' => [0b00000, 0b00000, 0b10001, 0b10001, 0b10001, 0b01010, 0b00100],
        'g' => [0b00000, 0b00000, 0b01111, 0b10001, 0b01111, 0b00001, 0b01110],
        ':' => [0b00000, 0b00100, 0b00000, 0b00000, 0b00100, 0b00000, 0b00000],
        '.' => [0b00000, 0b00000, 0b00000, 0b00000, 0b00000, 0b00000, 0b00100],
        ' ' => [0b00000, 0b00000, 0b00000, 0b00000, 0b00000, 0b00000, 0b00000],
        _ => [0b00000, 0b00000, 0b00000, 0b00000, 0b00000, 0b00000, 0b00000],
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::immich_api::FaceData;
    use crate::pipeline::ComputedValue;

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
    async fn test_disabled_skips_check() {
        let step = EyeFilterStep;
        let mut ctx = make_test_ctx();
        ctx.set_computed("ear", ComputedValue::Float(0.1)); // Below threshold
        let mut config = Config::default();
        config.processing.eye_filter.enabled = false;

        match step.execute(ctx, &config).await {
            StepOutcome::Continue(_) => {} // Expected
            other => panic!("Expected Continue when disabled, got {:?}", other),
        }
    }

    #[tokio::test]
    async fn test_below_threshold_skips() {
        let step = EyeFilterStep;
        let mut ctx = make_test_ctx();
        ctx.set_computed("ear", ComputedValue::Float(0.1)); // Below default 0.2 threshold
        let mut config = Config::default();
        config.processing.eye_filter.enabled = true;
        config.processing.eye_filter.min_ear = 0.2;

        match step.execute(ctx, &config).await {
            StepOutcome::Skip { reason, .. } => {
                assert_eq!(reason, "eyes_closed");
            }
            other => panic!("Expected Skip, got {:?}", other),
        }
    }

    #[tokio::test]
    async fn test_above_threshold_continues() {
        let step = EyeFilterStep;
        let mut ctx = make_test_ctx();
        ctx.set_computed("ear", ComputedValue::Float(0.3)); // Above threshold
        let mut config = Config::default();
        config.processing.eye_filter.enabled = true;
        config.processing.eye_filter.min_ear = 0.2;

        match step.execute(ctx, &config).await {
            StepOutcome::Continue(_) => {} // Expected
            other => panic!("Expected Continue, got {:?}", other),
        }
    }

    #[tokio::test]
    async fn test_no_ear_continues() {
        let step = EyeFilterStep;
        let ctx = make_test_ctx(); // No EAR set
        let mut config = Config::default();
        config.processing.eye_filter.enabled = true;

        match step.execute(ctx, &config).await {
            StepOutcome::Continue(_) => {} // Expected - gracefully handles missing EAR
            other => panic!("Expected Continue when no EAR, got {:?}", other),
        }
    }
}
