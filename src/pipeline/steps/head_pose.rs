//! Head pose estimation step.
//!
//! Uses the DMHead ONNX model to estimate head pose (yaw, pitch, roll) and
//! filter out non-front-facing faces.

use crate::config::Config;
use crate::models::DMHeadModel;
use crate::pipeline::{
    computed_keys, draw_simple_text, ComputedValue, PipelineContext, ProcessingStep, StepOutcome,
};
use async_trait::async_trait;
use image::{DynamicImage, GenericImageView, Rgb, RgbImage};

/// 3D point for cube vertices
#[derive(Clone, Copy)]
struct Point3D {
    x: f32,
    y: f32,
    z: f32,
}

impl Point3D {
    fn new(x: f32, y: f32, z: f32) -> Self {
        Self { x, y, z }
    }

    /// Rotate around Y axis (yaw)
    fn rotate_y(self, angle_deg: f32) -> Self {
        let rad = angle_deg.to_radians();
        let cos = rad.cos();
        let sin = rad.sin();
        Self {
            x: self.x * cos + self.z * sin,
            y: self.y,
            z: -self.x * sin + self.z * cos,
        }
    }

    /// Rotate around X axis (pitch)
    fn rotate_x(self, angle_deg: f32) -> Self {
        let rad = angle_deg.to_radians();
        let cos = rad.cos();
        let sin = rad.sin();
        Self {
            x: self.x,
            y: self.y * cos - self.z * sin,
            z: self.y * sin + self.z * cos,
        }
    }

    /// Rotate around Z axis (roll)
    fn rotate_z(self, angle_deg: f32) -> Self {
        let rad = angle_deg.to_radians();
        let cos = rad.cos();
        let sin = rad.sin();
        Self {
            x: self.x * cos - self.y * sin,
            y: self.x * sin + self.y * cos,
            z: self.z,
        }
    }

    /// Project 3D point to 2D using perspective projection
    fn project(self, cx: f32, cy: f32, focal_length: f32) -> (i32, i32) {
        let scale = focal_length / (focal_length + self.z);
        let x2d = cx + self.x * scale;
        let y2d = cy + self.y * scale;
        (x2d as i32, y2d as i32)
    }
}

/// Draw 3D pose axes (RGB = XYZ) from face center, rotated by head pose.
/// This is the standard visualization for head pose estimation.
fn draw_pose_axes(
    img: &mut RgbImage,
    cx: f32,
    cy: f32,
    axis_length: f32,
    yaw: f32,
    pitch: f32,
    roll: f32,
) {
    // Define axis endpoints (origin at 0,0,0)
    // X axis (red) - points right
    // Y axis (green) - points down (image coordinates)
    // Z axis (blue) - points out of screen (towards camera)
    let axes = [
        (Point3D::new(axis_length, 0.0, 0.0), Rgb([255, 0, 0])), // X - red
        (Point3D::new(0.0, axis_length, 0.0), Rgb([0, 255, 0])), // Y - green
        (Point3D::new(0.0, 0.0, -axis_length), Rgb([0, 0, 255])), // Z - blue (negative = towards camera)
    ];

    // Negate roll and pitch to convert from model convention to image coordinates
    // (image Y-axis points down, model assumes Y-axis points up)
    let focal_length = axis_length * 2.0;
    let origin = Point3D::new(0.0, 0.0, 0.0)
        .rotate_y(yaw)
        .rotate_x(-pitch)
        .rotate_z(-roll)
        .project(cx, cy, focal_length);

    for (endpoint, color) in axes {
        let rotated = endpoint
            .rotate_y(yaw)
            .rotate_x(-pitch)
            .rotate_z(-roll)
            .project(cx, cy, focal_length);

        draw_line(img, origin.0, origin.1, rotated.0, rotated.1, color);
    }
}

/// Draw an axis-aligned bounding box (no rotation applied).
fn draw_axis_aligned_rect(img: &mut RgbImage, x1: i32, y1: i32, x2: i32, y2: i32, color: Rgb<u8>) {
    draw_line(img, x1, y1, x2, y1, color); // top
    draw_line(img, x2, y1, x2, y2, color); // right
    draw_line(img, x2, y2, x1, y2, color); // bottom
    draw_line(img, x1, y2, x1, y1, color); // left
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
        let image: &DynamicImage = match ctx.require_image("head pose estimation") {
            Ok(img) => img,
            Err(e) => return StepOutcome::Error { ctx, error: e },
        };

        // Load the DMHead model
        let model = match DMHeadModel::global() {
            Ok(m) => m,
            Err(e) => {
                // If model isn't available, skip this step with a warning
                tracing::warn!(
                    "DMHead model not available, skipping head pose check: {}",
                    e
                );
                return StepOutcome::Continue(ctx);
            }
        };

        // Extract a square face crop if we have the face rectangle
        // Square crop prevents aspect ratio distortion when DMHead resizes to 224x224
        let face_image: DynamicImage = if let Some(face_rect) = ctx
            .get_computed(computed_keys::FACE_RECT)
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
                let x_with_margin = x.saturating_sub(margin);
                let y_with_margin = y.saturating_sub(margin);
                let w_with_margin = (w + margin * 2).min(img_w - x_with_margin);
                let h_with_margin = (h + margin * 2).min(img_h - y_with_margin);

                // Make the crop square by using the larger dimension
                let size = w_with_margin.max(h_with_margin);

                // Center the square crop around the face
                let center_x = x_with_margin + w_with_margin / 2;
                let center_y = y_with_margin + h_with_margin / 2;

                let square_x = center_x.saturating_sub(size / 2);
                let square_y = center_y.saturating_sub(size / 2);

                // Ensure the square crop doesn't go out of bounds
                let final_x = square_x.min(img_w.saturating_sub(size));
                let final_y = square_y.min(img_h.saturating_sub(size));
                let final_size = size.min(img_w - final_x).min(img_h - final_y);

                image.crop_imm(final_x, final_y, final_size, final_size)
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
                return StepOutcome::Error {
                    ctx,
                    error: format!("Head pose estimation failed: {}", e),
                };
            }
        };

        // Store pose in computed values
        ctx.set_computed(computed_keys::HEAD_POSE, ComputedValue::HeadPose(pose));

        // Check against thresholds
        let head_pose_config = &config.processing.head_pose;

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

        // Note: Roll is not checked for pass/fail - only used for visualization
        // Roll (head tilt) is less important for timelapse alignment since
        // the alignment step can handle rotated faces

        StepOutcome::Continue(ctx)
    }

    fn debug_visualize(&self, ctx: &PipelineContext, _config: &Config) -> Option<DynamicImage> {
        // Get head pose from computed values
        let pose = ctx
            .get_computed(computed_keys::HEAD_POSE)
            .and_then(|v| v.as_head_pose())?;

        // Get the current image to draw on
        let image = ctx.image.as_ref()?;
        let rgb = image.to_rgb8();
        let (width, height) = (rgb.width(), rgb.height());

        // Create a copy for visualization
        let mut debug_img = rgb.clone();

        // Get face rectangle if available
        let face_rect = ctx
            .get_computed(computed_keys::FACE_RECT)
            .and_then(|v| v.as_face_rect());

        // Draw axis-aligned face bounding box and 3D pose axes
        if let Some(rect) = face_rect {
            let x1 = rect.x1 as i32;
            let y1 = rect.y1 as i32;
            let x2 = rect.x2 as i32;
            let y2 = rect.y2 as i32;

            // Draw axis-aligned bounding box in cyan
            draw_axis_aligned_rect(&mut debug_img, x1, y1, x2, y2, Rgb([0, 255, 255]));

            // Draw 3D pose axes from face center
            let cx = (x1 + x2) as f32 / 2.0;
            let cy = (y1 + y2) as f32 / 2.0;
            let axis_length = ((x2 - x1).max(y2 - y1) as f32) * 0.6;
            draw_pose_axes(
                &mut debug_img,
                cx,
                cy,
                axis_length,
                pose.yaw,
                pose.pitch,
                pose.roll,
            );
        }

        // Draw text background bar at bottom for pose values
        let bar_height = 20u32;
        for y in height.saturating_sub(bar_height)..height {
            for x in 0..width {
                debug_img.put_pixel(x, y, Rgb([0, 0, 0]));
            }
        }

        // Draw simple text representation of values
        let text = format!(
            "Y:{:+.0} P:{:+.0} R:{:+.0}",
            pose.yaw, pose.pitch, pose.roll
        );
        draw_simple_text(
            &mut debug_img,
            5,
            height - bar_height + 4,
            &text,
            Rgb([255, 255, 255]),
        );

        Some(DynamicImage::ImageRgb8(debug_img))
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
    async fn test_no_image_error() {
        let step = HeadPoseStep;
        let ctx = make_test_ctx();
        let mut config = Config::default();
        config.processing.head_pose.enabled = true;

        match step.execute(ctx, &config).await {
            StepOutcome::Error { error, .. } => {
                assert!(error.contains("No image"));
            }
            other => panic!("Expected Error, got {:?}", other),
        }
    }
}
