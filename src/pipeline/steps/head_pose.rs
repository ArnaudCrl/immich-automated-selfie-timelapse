//! Head pose estimation step.
//!
//! Uses the DMHead ONNX model to estimate head pose (yaw, pitch, roll) and
//! filter out non-front-facing faces.

use crate::config::Config;
use crate::models::DMHeadModel;
use crate::pipeline::{ComputedValue, PipelineContext, ProcessingStep, StepOutcome};
use async_trait::async_trait;
use image::{DynamicImage, GenericImageView, Rgb, RgbImage};

/// Estimates head pose and filters non-frontal faces.
///
/// This step:
/// 1. Runs DMHead inference on the cropped face image
/// 2. Stores the HeadPose result in ctx.computed["head_pose"]
/// 3. Skips if any angle exceeds configured thresholds
pub struct HeadPoseStep;

#[async_trait]
impl ProcessingStep for HeadPoseStep {
    fn id(&self) -> &'static str {
        "head_pose"
    }

    fn name(&self) -> &'static str {
        "Head Pose"
    }

    async fn execute(&self, mut ctx: PipelineContext, config: &Config) -> StepOutcome {
        // Skip if head pose filtering is disabled
        if !config.processing.head_pose.enabled {
            return StepOutcome::Continue(ctx);
        }

        let image = match ctx.require_image("head pose estimation") {
            Ok(img) => img,
            Err(e) => return StepOutcome::Error(e),
        };

        // Load the DMHead model
        let model = match DMHeadModel::global() {
            Ok(m) => m,
            Err(e) => {
                // If model isn't available, skip this step with a warning
                tracing::warn!("DMHead model not available, skipping head pose check: {}", e);
                return StepOutcome::Continue(ctx);
            }
        };

        // Extract a tighter face crop if we have the face rectangle
        // DMHead works better with tight face crops centered on the face
        let face_image: DynamicImage = if let Some(face_rect) = ctx
            .get_computed("face_rect")
            .and_then(|v| v.as_face_rect())
        {
            // Use the face rectangle to extract a tighter crop
            let (img_w, img_h) = image.dimensions();
            let x = (face_rect.x1 as u32).min(img_w.saturating_sub(1));
            let y = (face_rect.y1 as u32).min(img_h.saturating_sub(1));
            let w = ((face_rect.x2 - face_rect.x1) as u32).min(img_w - x);
            let h = ((face_rect.y2 - face_rect.y1) as u32).min(img_h - y);

            if w > 10 && h > 10 {
                // Add a small margin around the face for better model performance
                let margin = (w.max(h) / 4).min(20);
                let x = x.saturating_sub(margin);
                let y = y.saturating_sub(margin);
                let w = (w + margin * 2).min(img_w - x);
                let h = (h + margin * 2).min(img_h - y);

                image.crop_imm(x, y, w, h)
            } else {
                // Face rect too small, use full image
                image.clone()
            }
        } else {
            // No face rect available, use full image
            image.clone()
        };

        // Run inference on the face crop
        let pose = match model.estimate(&face_image) {
            Ok(p) => p,
            Err(e) => {
                return StepOutcome::Error(format!("Head pose estimation failed: {}", e));
            }
        };

        // Store pose in computed values
        ctx.set_computed("head_pose", ComputedValue::HeadPose(pose));

        // Check against thresholds
        let head_pose_config = &config.processing.head_pose;

        tracing::debug!(
            "Head pose detected: yaw={:.1}°, pitch={:.1}°, roll={:.1}°",
            pose.yaw,
            pose.pitch,
            pose.roll
        );

        if pose.yaw.abs() > head_pose_config.max_yaw {
            return StepOutcome::Skip {
                ctx,
                reason: "head_turned".to_string(),
                detail: Some(format!(
                    "Yaw {:.1}° exceeds threshold {:.1}°",
                    pose.yaw, head_pose_config.max_yaw
                )),
            };
        }

        if pose.pitch.abs() > head_pose_config.max_pitch {
            return StepOutcome::Skip {
                ctx,
                reason: "head_turned".to_string(),
                detail: Some(format!(
                    "Pitch {:.1}° exceeds threshold {:.1}°",
                    pose.pitch, head_pose_config.max_pitch
                )),
            };
        }

        if pose.roll.abs() > head_pose_config.max_roll {
            return StepOutcome::Skip {
                ctx,
                reason: "head_turned".to_string(),
                detail: Some(format!(
                    "Roll {:.1}° exceeds threshold {:.1}°",
                    pose.roll, head_pose_config.max_roll
                )),
            };
        }

        StepOutcome::Continue(ctx)
    }

    fn debug_visualize(&self, ctx: &PipelineContext) -> Option<DynamicImage> {
        // Get head pose from computed values
        let pose = ctx
            .get_computed("head_pose")
            .and_then(|v| v.as_head_pose())?;

        // Get the current image to draw on
        let image = ctx.image.as_ref()?;
        let rgb = image.to_rgb8();
        let (width, height) = (rgb.width(), rgb.height());

        // Create a copy for visualization
        let mut debug_img = rgb.clone();

        // Draw a center crosshair
        let cx = width / 2;
        let cy = height / 2;
        let crosshair_size = 20u32;

        // Horizontal line
        for x in cx.saturating_sub(crosshair_size)..=(cx + crosshair_size).min(width - 1) {
            debug_img.put_pixel(x, cy, Rgb([0, 255, 0]));
        }
        // Vertical line
        for y in cy.saturating_sub(crosshair_size)..=(cy + crosshair_size).min(height - 1) {
            debug_img.put_pixel(cx, y, Rgb([0, 255, 0]));
        }

        // Draw pose direction arrow from center
        // Yaw rotates left/right, pitch rotates up/down
        let arrow_len = 40.0_f32;
        let yaw_rad = pose.yaw.to_radians();
        let pitch_rad = pose.pitch.to_radians();

        // Arrow endpoint based on yaw and pitch
        let dx = (yaw_rad.sin() * arrow_len) as i32;
        let dy = (-pitch_rad.sin() * arrow_len) as i32; // Negative because y increases downward

        let ex = (cx as i32 + dx).clamp(0, width as i32 - 1) as u32;
        let ey = (cy as i32 + dy).clamp(0, height as i32 - 1) as u32;

        // Draw arrow line using Bresenham's algorithm
        draw_line(&mut debug_img, cx as i32, cy as i32, ex as i32, ey as i32, Rgb([255, 0, 0]));

        // Draw roll indicator as a tilted line through center
        let roll_rad = pose.roll.to_radians();
        let roll_len = 30.0_f32;
        let rx1 = (cx as f32 - roll_rad.cos() * roll_len) as u32;
        let ry1 = (cy as f32 - roll_rad.sin() * roll_len) as u32;
        let rx2 = (cx as f32 + roll_rad.cos() * roll_len) as u32;
        let ry2 = (cy as f32 + roll_rad.sin() * roll_len) as u32;
        draw_line(&mut debug_img, rx1 as i32, ry1 as i32, rx2 as i32, ry2 as i32, Rgb([0, 255, 255]));

        // Draw text background bar at bottom for pose values
        let bar_height = 20u32;
        for y in height.saturating_sub(bar_height)..height {
            for x in 0..width {
                debug_img.put_pixel(x, y, Rgb([0, 0, 0]));
            }
        }

        // Draw simple text representation of values using block characters
        // Format: Y:-20 P:+29 R:-21
        let text = format!(
            "Y:{:+.0} P:{:+.0} R:{:+.0}",
            pose.yaw, pose.pitch, pose.roll
        );
        draw_simple_text(&mut debug_img, 5, height - bar_height + 4, &text, Rgb([255, 255, 255]));

        Some(DynamicImage::ImageRgb8(debug_img))
    }
}

/// Draw a line using Bresenham's algorithm.
fn draw_line(img: &mut RgbImage, x0: i32, y0: i32, x1: i32, y1: i32, color: Rgb<u8>) {
    let (width, height) = (img.width() as i32, img.height() as i32);

    let dx = (x1 - x0).abs();
    let dy = -(y1 - y0).abs();
    let sx = if x0 < x1 { 1 } else { -1 };
    let sy = if y0 < y1 { 1 } else { -1 };
    let mut err = dx + dy;

    let mut x = x0;
    let mut y = y0;

    loop {
        if x >= 0 && x < width && y >= 0 && y < height {
            img.put_pixel(x as u32, y as u32, color);
        }

        if x == x1 && y == y1 {
            break;
        }

        let e2 = 2 * err;
        if e2 >= dy {
            err += dy;
            x += sx;
        }
        if e2 <= dx {
            err += dx;
            y += sy;
        }
    }
}

/// Draw simple text using a basic 5x7 pixel font.
/// Only supports basic ASCII characters needed for pose display.
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
        'Y' => [0b10001, 0b10001, 0b01010, 0b00100, 0b00100, 0b00100, 0b00100],
        'P' => [0b11110, 0b10001, 0b10001, 0b11110, 0b10000, 0b10000, 0b10000],
        'R' => [0b11110, 0b10001, 0b10001, 0b11110, 0b10100, 0b10010, 0b10001],
        ':' => [0b00000, 0b00100, 0b00000, 0b00000, 0b00100, 0b00000, 0b00000],
        '+' => [0b00000, 0b00100, 0b00100, 0b11111, 0b00100, 0b00100, 0b00000],
        '-' => [0b00000, 0b00000, 0b00000, 0b11111, 0b00000, 0b00000, 0b00000],
        ' ' => [0b00000, 0b00000, 0b00000, 0b00000, 0b00000, 0b00000, 0b00000],
        '.' => [0b00000, 0b00000, 0b00000, 0b00000, 0b00000, 0b00000, 0b00100],
        _ => [0b00000, 0b00000, 0b00000, 0b00000, 0b00000, 0b00000, 0b00000],
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::immich_api::FaceData;
    use image::RgbImage;

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
        let step = HeadPoseStep;
        let ctx = make_test_ctx();
        let mut config = Config::default();
        config.processing.head_pose.enabled = false;

        // Create a dummy image
        let img = DynamicImage::ImageRgb8(RgbImage::new(100, 100));
        let ctx = ctx.with_image(img);

        match step.execute(ctx, &config).await {
            StepOutcome::Continue(_) => {} // Expected
            other => panic!("Expected Continue when disabled, got {:?}", other),
        }
    }

    #[tokio::test]
    async fn test_no_image_error() {
        let step = HeadPoseStep;
        let ctx = make_test_ctx();
        let mut config = Config::default();
        config.processing.head_pose.enabled = true;

        match step.execute(ctx, &config).await {
            StepOutcome::Error(msg) => {
                assert!(msg.contains("No image"));
            }
            other => panic!("Expected Error, got {:?}", other),
        }
    }
}
