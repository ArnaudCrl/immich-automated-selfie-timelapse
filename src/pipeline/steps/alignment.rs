//! Eye-based face alignment step.
//!
//! Aligns faces based on eye positions to ensure consistent eye placement
//! across all images in the timelapse.
//!
//! Uses a single affine transformation matrix that combines rotation, scaling,
//! and translation to align eyes at the desired positions in one operation.

use crate::config::Config;
use crate::pipeline::{computed_keys, PipelineContext, Point, ProcessingStep, StepOutcome};
use async_trait::async_trait;
use image::{DynamicImage, Rgb, RgbImage};

/// Aligns faces based on eye positions.
///
/// This step:
/// 1. Retrieves landmarks from ctx.computed["landmarks"]
/// 2. Calculates a single affine transformation matrix that:
///    - Rotates to make eyes horizontal
///    - Scales to match desired inter-eye distance
///    - Translates to position eyes at configured target positions
/// 3. Applies the transformation using bilinear interpolation
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
        // Get landmarks from previous step (required)
        let landmarks: crate::pipeline::Landmarks = match ctx
            .get_computed(computed_keys::LANDMARKS)
            .and_then(|v| v.as_landmarks())
        {
            Some(l) => l.clone(),
            None => {
                // Landmarks should always be available since LandmarksStep is mandatory
                return StepOutcome::Error {
                    ctx,
                    error: "Landmarks not available for alignment - pipeline misconfigured"
                        .to_string(),
                };
            }
        };

        let image = match ctx.take_image("alignment") {
            Ok(img) => img,
            Err(e) => return StepOutcome::Error { ctx, error: e },
        };

        let output_size = config.processing.output.size;

        // Get eye centers
        let left_eye = landmarks.left_eye_center();
        let right_eye = landmarks.right_eye_center();

        // Guard against coincident eyes (would cause division by zero in transform)
        let eye_distance =
            ((right_eye.x - left_eye.x).powi(2) + (right_eye.y - left_eye.y).powi(2)).sqrt();
        if eye_distance < 1e-6 {
            return StepOutcome::Skip {
                ctx,
                reason: "alignment".to_string(),
                detail: Some("Eye positions are too close together for alignment".to_string()),
            };
        }

        // Calculate the transformation matrix
        let transform = calculate_eye_alignment_transform(
            left_eye,
            right_eye,
            output_size,
            &config.processing.alignment,
        );

        // Apply the transformation
        let aligned = apply_affine_transform(&image, &transform, output_size);

        ctx.image = Some(aligned);

        StepOutcome::Continue(ctx)
    }
}

/// 2x3 affine transformation matrix.
/// Represents the transformation: [x', y'] = [[a, b, c], [d, e, f]] * [x, y, 1]
#[derive(Debug, Clone, Copy)]
struct AffineMatrix {
    a: f32,
    b: f32,
    c: f32,
    d: f32,
    e: f32,
    f: f32,
}

impl AffineMatrix {
    /// Create a translation matrix.
    fn translation(tx: f32, ty: f32) -> Self {
        Self {
            a: 1.0,
            b: 0.0,
            c: tx,
            d: 0.0,
            e: 1.0,
            f: ty,
        }
    }

    /// Create a rotation matrix (angle in radians).
    fn rotation(angle: f32) -> Self {
        let cos_a = angle.cos();
        let sin_a = angle.sin();
        Self {
            a: cos_a,
            b: -sin_a,
            c: 0.0,
            d: sin_a,
            e: cos_a,
            f: 0.0,
        }
    }

    /// Create a scale matrix.
    fn scale(s: f32) -> Self {
        Self {
            a: s,
            b: 0.0,
            c: 0.0,
            d: 0.0,
            e: s,
            f: 0.0,
        }
    }

    /// Compose this transformation with another (self * other).
    /// This applies 'other' first, then 'self'.
    fn compose(&self, other: &AffineMatrix) -> AffineMatrix {
        AffineMatrix {
            a: self.a * other.a + self.b * other.d,
            b: self.a * other.b + self.b * other.e,
            c: self.a * other.c + self.b * other.f + self.c,
            d: self.d * other.a + self.e * other.d,
            e: self.d * other.b + self.e * other.e,
            f: self.d * other.c + self.e * other.f + self.f,
        }
    }

    /// Transform a point using this matrix.
    fn transform_point(&self, x: f32, y: f32) -> (f32, f32) {
        (
            self.a * x + self.b * y + self.c,
            self.d * x + self.e * y + self.f,
        )
    }
}

/// Calculate the affine transformation matrix to align eyes at desired positions.
///
/// This implements the same algorithm as the Python example:
/// 1. Calculate target eye positions based on config
/// 2. Compute rotation angle to make eyes horizontal
/// 3. Compute scale to match desired inter-eye distance
/// 4. Combine translation, rotation, scale, and final translation into one matrix
fn calculate_eye_alignment_transform(
    left_eye: Point,
    right_eye: Point,
    output_size: u32,
    alignment_config: &crate::config::AlignmentConfig,
) -> AffineMatrix {
    let output_size_f = output_size as f32;

    // Calculate target eye positions from the single eye_distance setting
    let left_eye_x = alignment_config.left_eye_x();
    let left_eye_y = alignment_config.left_eye_y();

    let left_eye_target = Point::new(output_size_f * left_eye_x, output_size_f * left_eye_y);
    let right_eye_target = Point::new(
        output_size_f * (1.0 - left_eye_x),
        output_size_f * left_eye_y,
    );

    // Calculate angles
    let current_angle = (right_eye.y - left_eye.y).atan2(right_eye.x - left_eye.x);
    let target_angle =
        (right_eye_target.y - left_eye_target.y).atan2(right_eye_target.x - left_eye_target.x);
    let rotation_angle = target_angle - current_angle;

    // Calculate scale
    let current_eye_distance =
        ((right_eye.x - left_eye.x).powi(2) + (right_eye.y - left_eye.y).powi(2)).sqrt();
    let target_eye_distance = ((right_eye_target.x - left_eye_target.x).powi(2)
        + (right_eye_target.y - left_eye_target.y).powi(2))
    .sqrt();
    let scale = target_eye_distance / current_eye_distance;

    // Eye centers
    let center = Point::new(
        (left_eye.x + right_eye.x) / 2.0,
        (left_eye.y + right_eye.y) / 2.0,
    );
    let target_center = Point::new(
        (left_eye_target.x + right_eye_target.x) / 2.0,
        (left_eye_target.y + right_eye_target.y) / 2.0,
    );

    // Build the transformation matrix by composing:
    // 1. Translate to origin (center of eyes)
    // 2. Rotate
    // 3. Scale
    // 4. Translate to target position
    let m1 = AffineMatrix::translation(-center.x, -center.y);
    let m2 = AffineMatrix::rotation(rotation_angle);
    let m3 = AffineMatrix::scale(scale);
    let m4 = AffineMatrix::translation(target_center.x, target_center.y);

    // Compose: M = M4 * M3 * M2 * M1
    // This means we apply M1 first, then M2, then M3, then M4
    m4.compose(&m3.compose(&m2.compose(&m1)))
}

/// Apply an affine transformation to an image.
///
/// Uses inverse mapping with bilinear interpolation to avoid holes in the output.
fn apply_affine_transform(
    image: &DynamicImage,
    transform: &AffineMatrix,
    output_size: u32,
) -> DynamicImage {
    let rgb = image.to_rgb8();
    let (width, height) = rgb.dimensions();

    // We need the inverse transform to do inverse mapping
    // For an affine transform, the inverse can be computed analytically
    let det = transform.a * transform.e - transform.b * transform.d;
    if det.abs() < 1e-10 {
        // Degenerate transform, return black image
        return DynamicImage::ImageRgb8(RgbImage::new(output_size, output_size));
    }

    let inv_det = 1.0 / det;
    let inv_transform = AffineMatrix {
        a: transform.e * inv_det,
        b: -transform.b * inv_det,
        c: (transform.b * transform.f - transform.e * transform.c) * inv_det,
        d: -transform.d * inv_det,
        e: transform.a * inv_det,
        f: (transform.d * transform.c - transform.a * transform.f) * inv_det,
    };

    // Create output image using inverse mapping
    let output = RgbImage::from_fn(output_size, output_size, |x, y| {
        // Map output pixel to source pixel
        let (src_x, src_y) = inv_transform.transform_point(x as f32, y as f32);

        // Clamp coordinates to valid range (replicate edge pixels)
        let clamped_x = src_x.clamp(0.0, (width - 1) as f32);
        let clamped_y = src_y.clamp(0.0, (height - 1) as f32);

        // Bilinear interpolation with clamped coordinates
        let x0 = clamped_x.floor() as u32;
        let y0 = clamped_y.floor() as u32;
        let x1 = (x0 + 1).min(width - 1);
        let y1 = (y0 + 1).min(height - 1);

        let dx = clamped_x - x0 as f32;
        let dy = clamped_y - y0 as f32;

        let p00 = rgb.get_pixel(x0, y0);
        let p10 = rgb.get_pixel(x1, y0);
        let p01 = rgb.get_pixel(x0, y1);
        let p11 = rgb.get_pixel(x1, y1);

        let r = (p00[0] as f32 * (1.0 - dx) * (1.0 - dy)
            + p10[0] as f32 * dx * (1.0 - dy)
            + p01[0] as f32 * (1.0 - dx) * dy
            + p11[0] as f32 * dx * dy) as u8;
        let g = (p00[1] as f32 * (1.0 - dx) * (1.0 - dy)
            + p10[1] as f32 * dx * (1.0 - dy)
            + p01[1] as f32 * (1.0 - dx) * dy
            + p11[1] as f32 * dx * dy) as u8;
        let b = (p00[2] as f32 * (1.0 - dx) * (1.0 - dy)
            + p10[2] as f32 * dx * (1.0 - dy)
            + p01[2] as f32 * (1.0 - dx) * dy
            + p11[2] as f32 * dx * dy) as u8;

        Rgb([r, g, b])
    });

    DynamicImage::ImageRgb8(output)
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
    async fn test_no_landmarks_errors() {
        let step = AlignmentStep;
        let ctx = make_test_ctx();
        let config = Config::default();

        // Create a dummy image but no landmarks
        let img = DynamicImage::ImageRgb8(RgbImage::new(100, 100));
        let ctx = ctx.with_image(img);

        match step.execute(ctx, &config).await {
            StepOutcome::Error { error, .. } => {
                assert!(error.contains("Landmarks not available"));
            }
            other => panic!("Expected Error without landmarks, got {:?}", other),
        }
    }
}
