//! Face cropping operations.
//!
//! Extracts and resizes face regions from images using bounding box data.

use crate::error::{Error, Result};
use crate::immich_api::FaceData;
use image::imageops::FilterType;
use image::{DynamicImage, GenericImageView};

/// Crop and resize the face from an image using bounding box.
/// Returns (cropped_full_res, resized_final) for intermediate saving.
///
/// This is a simplified version that just uses the bounding box.
/// A full implementation would use facial landmarks for alignment.
pub fn crop_face_with_intermediate(
    img: &DynamicImage,
    face_data: &FaceData,
    output_size: u32,
) -> Result<(DynamicImage, DynamicImage)> {
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

    // Calculate crop bounds, clamped to image dimensions
    let crop_x1 = center_x
        .saturating_sub(crop_size / 2)
        .min(img_width.saturating_sub(crop_size));
    let crop_y1 = center_y
        .saturating_sub(crop_size / 2)
        .min(img_height.saturating_sub(crop_size));

    // Ensure we don't exceed image bounds
    let actual_crop_size = crop_size.min(img_width - crop_x1).min(img_height - crop_y1);

    if actual_crop_size < 10 {
        return Err(Error::ImageProcessing("Crop area too small".to_string()));
    }

    // Crop the face region (full resolution)
    let cropped = img.crop_imm(crop_x1, crop_y1, actual_crop_size, actual_crop_size);

    // Resize to output size
    let resized = cropped.resize_exact(output_size, output_size, FilterType::Lanczos3);

    Ok((cropped, resized))
}
