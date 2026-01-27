//! Debug visualization functions for face processing.
//!
//! These functions create annotated images showing the processing steps,
//! useful for debugging and understanding the pipeline behavior.

use crate::immich_api::FaceData;
use image::{DynamicImage, GenericImageView, Rgb};
use imageproc::drawing::{draw_hollow_rect_mut, draw_line_segment_mut};
use imageproc::rect::Rect;

/// Draw debug visualization showing bounding box and crop region.
/// - Red rectangle: face bounding box from Immich
/// - Green rectangle: expanded crop region used for processing
/// - Red crosshair: center of the face bounding box
pub fn draw_crop_debug(img: &DynamicImage, face_data: &FaceData) -> DynamicImage {
    let (img_width, img_height) = img.dimensions();
    let mut rgb_img = img.to_rgb8();

    // Face bounding box in pixel coordinates
    let bbox_x1 = (face_data.bounding_box_x1 * img_width as f32) as i32;
    let bbox_y1 = (face_data.bounding_box_y1 * img_height as f32) as i32;
    let bbox_x2 = (face_data.bounding_box_x2 * img_width as f32) as i32;
    let bbox_y2 = (face_data.bounding_box_y2 * img_height as f32) as i32;

    let face_width = (bbox_x2 - bbox_x1) as u32;
    let face_height = (bbox_y2 - bbox_y1) as u32;

    // Calculate crop region (same logic as crop_face_with_intermediate in job/mod.rs)
    let face_size = face_width.max(face_height);
    let padding = face_size / 2;
    let crop_size = face_size + padding * 2;

    let center_x = (bbox_x1 + bbox_x2) / 2;
    let center_y = (bbox_y1 + bbox_y2) / 2;

    let crop_x1 = (center_x - crop_size as i32 / 2).max(0) as u32;
    let crop_y1 = (center_y - crop_size as i32 / 2).max(0) as u32;
    let crop_x1 = crop_x1.min(img_width.saturating_sub(crop_size));
    let crop_y1 = crop_y1.min(img_height.saturating_sub(crop_size));
    let actual_crop_size = crop_size.min(img_width - crop_x1).min(img_height - crop_y1);

    // Colors
    let red = Rgb([255u8, 0, 0]);
    let green = Rgb([0u8, 255, 0]);

    // Draw face bounding box (red) - draw multiple times for thickness
    for offset in 0..3i32 {
        let rect = Rect::at(bbox_x1 - offset, bbox_y1 - offset)
            .of_size(face_width + offset as u32 * 2, face_height + offset as u32 * 2);
        draw_hollow_rect_mut(&mut rgb_img, rect, red);
    }

    // Draw crop region (green) - draw multiple times for thickness
    for offset in 0..3i32 {
        let rect = Rect::at(crop_x1 as i32 - offset, crop_y1 as i32 - offset)
            .of_size(
                actual_crop_size + offset as u32 * 2,
                actual_crop_size + offset as u32 * 2,
            );
        draw_hollow_rect_mut(&mut rgb_img, rect, green);
    }

    // Draw crosshair at face center
    let cross_size = 20i32;
    draw_line_segment_mut(
        &mut rgb_img,
        ((center_x - cross_size) as f32, center_y as f32),
        ((center_x + cross_size) as f32, center_y as f32),
        red,
    );
    draw_line_segment_mut(
        &mut rgb_img,
        (center_x as f32, (center_y - cross_size) as f32),
        (center_x as f32, (center_y + cross_size) as f32),
        red,
    );

    DynamicImage::ImageRgb8(rgb_img)
}

// Future debug visualization functions:
// - draw_landmarks_debug: Face with 68-point landmarks drawn
// - draw_alignment_debug: Before/after alignment visualization
