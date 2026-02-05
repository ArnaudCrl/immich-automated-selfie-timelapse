//! Face cropping operations.
//!
//! Extracts and resizes face regions from images using bounding box data.

use crate::error::{Error, Result};
use crate::immich_api::FaceData;
use crate::pipeline::BoundingBox;
use image::imageops::FilterType;
use image::{DynamicImage, GenericImageView, RgbImage};

/// Result of cropping a face from an image.
pub struct CropResult {
    /// The cropped image at full resolution.
    pub cropped: DynamicImage,
    /// The cropped image resized to output size.
    pub resized: DynamicImage,
    /// The face bounding box in crop coordinates.
    pub face_rect: BoundingBox,
}

/// Crop and resize the face from an image using bounding box.
/// Returns CropResult containing cropped images and face rectangle in crop coordinates.
///
/// This is a simplified version that just uses the bounding box.
/// A full implementation would use facial landmarks for alignment.
pub fn crop_face_with_intermediate(
    img: &DynamicImage,
    face_data: &FaceData,
    output_size: u32,
) -> Result<CropResult> {
    let (img_width, img_height) = img.dimensions();

    // Scale bounding box from metadata dimensions to actual image dimensions.
    // Immich stores bounding box as pixel coordinates relative to image_width/image_height,
    // but the loaded image may have different dimensions (e.g., if downloaded at different resolution).
    let scale_x = img_width as f32 / face_data.image_width as f32;
    let scale_y = img_height as f32 / face_data.image_height as f32;

    let x1 = (face_data.bounding_box_x1 * scale_x) as u32;
    let y1 = (face_data.bounding_box_y1 * scale_y) as u32;
    let x2 = (face_data.bounding_box_x2 * scale_x) as u32;
    let y2 = (face_data.bounding_box_y2 * scale_y) as u32;

    let face_width = x2.saturating_sub(x1);
    let face_height = y2.saturating_sub(y1);

    if face_width == 0 || face_height == 0 {
        return Err(Error::ImageProcessing(
            "Invalid face bounding box".to_string(),
        ));
    }

    // Expand the crop area to include some context around the face
    // and make it square for consistent output
    let face_size = face_width.max(face_height);
    let padding = face_size / 2; // 50% padding on each side
    let crop_size = face_size + padding * 2;

    // Calculate center of face
    let center_x = (x1 + x2) / 2;
    let center_y = (y1 + y2) / 2;

    if crop_size < 10 {
        return Err(Error::ImageProcessing("Crop area too small".to_string()));
    }

    // Ideal crop region centered on the face (may extend outside image bounds)
    let ideal_x1 = center_x as i32 - crop_size as i32 / 2;
    let ideal_y1 = center_y as i32 - crop_size as i32 / 2;

    // Crop with replicate-fill: always keeps the face centered by extending
    // edge pixels at image borders instead of shifting the crop window.
    let cropped = crop_with_replicate_fill(img, ideal_x1, ideal_y1, crop_size);

    // Resize to output size
    let resized = cropped.resize_exact(output_size, output_size, FilterType::Lanczos3);

    // Face coordinates relative to the crop origin (face is always centered)
    let face_rect = BoundingBox {
        x1: (x1 as i32 - ideal_x1) as f32,
        y1: (y1 as i32 - ideal_y1) as f32,
        x2: (x2 as i32 - ideal_x1) as f32,
        y2: (y2 as i32 - ideal_y1) as f32,
    };

    Ok(CropResult {
        cropped,
        resized,
        face_rect,
    })
}

/// Crop a region from an image, filling out-of-bounds areas with replicated edge pixels.
///
/// When the crop region extends past the image borders, edge pixels are repeated
/// (e.g., column -1 uses column 0, column -2 uses column 0, etc.).
fn crop_with_replicate_fill(
    img: &DynamicImage,
    x_offset: i32,
    y_offset: i32,
    size: u32,
) -> DynamicImage {
    let rgb = img.to_rgb8();
    let (w, h) = (rgb.width() as i32, rgb.height() as i32);

    let out = RgbImage::from_fn(size, size, |px, py| {
        let sx = replicate_coord(x_offset + px as i32, w);
        let sy = replicate_coord(y_offset + py as i32, h);
        *rgb.get_pixel(sx as u32, sy as u32)
    });

    DynamicImage::ImageRgb8(out)
}

/// Clamp a coordinate into the valid range [0, len) using replicate/edge boundary.
///
/// For a dimension of length `len`:
/// - Coordinates in `[0, len)` map to themselves.
/// - Negative coordinates clamp to 0.
/// - Coordinates >= len clamp to len-1.
fn replicate_coord(c: i32, len: i32) -> i32 {
    if len <= 1 {
        return 0;
    }
    c.clamp(0, len - 1)
}
