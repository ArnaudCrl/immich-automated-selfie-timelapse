//! EXIF orientation handling.
//!
//! Reads EXIF orientation from image bytes and applies the correct transformation
//! to ensure images are displayed in their intended orientation.

use exif::{In, Reader, Tag};
use image::DynamicImage;
use std::io::Cursor;

/// EXIF orientation values.
/// See: https://exiftool.org/TagNames/EXIF.html#Orientation
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Orientation {
    /// Normal (no transformation needed)
    Normal,
    /// Flip horizontal
    FlipHorizontal,
    /// Rotate 180°
    Rotate180,
    /// Flip vertical
    FlipVertical,
    /// Rotate 90° CW then flip horizontal
    Rotate90CwFlipH,
    /// Rotate 90° clockwise (270° counter-clockwise)
    Rotate90Cw,
    /// Rotate 90° CCW then flip horizontal
    Rotate90CcwFlipH,
    /// Rotate 90° counter-clockwise (270° clockwise)
    Rotate90Ccw,
}

impl From<u32> for Orientation {
    fn from(value: u32) -> Self {
        match value {
            1 => Orientation::Normal,
            2 => Orientation::FlipHorizontal,
            3 => Orientation::Rotate180,
            4 => Orientation::FlipVertical,
            5 => Orientation::Rotate90CwFlipH,
            6 => Orientation::Rotate90Cw,
            7 => Orientation::Rotate90CcwFlipH,
            8 => Orientation::Rotate90Ccw,
            _ => Orientation::Normal,
        }
    }
}

/// Read EXIF orientation from image bytes.
/// Returns `Orientation::Normal` if no orientation tag is found or on error.
pub fn read_orientation(image_bytes: &[u8]) -> Orientation {
    let mut cursor = Cursor::new(image_bytes);

    let exif = match Reader::new().read_from_container(&mut cursor) {
        Ok(exif) => exif,
        Err(_) => return Orientation::Normal,
    };

    match exif.get_field(Tag::Orientation, In::PRIMARY) {
        Some(field) => match field.value.get_uint(0) {
            Some(value) => Orientation::from(value),
            None => Orientation::Normal,
        },
        None => Orientation::Normal,
    }
}

/// Apply EXIF orientation correction to an image.
/// This transforms the image so it displays correctly regardless of how the camera saved it.
pub fn apply_orientation(img: DynamicImage, orientation: Orientation) -> DynamicImage {
    match orientation {
        Orientation::Normal => img,
        Orientation::FlipHorizontal => img.fliph(),
        Orientation::Rotate180 => img.rotate180(),
        Orientation::FlipVertical => img.flipv(),
        Orientation::Rotate90CwFlipH => img.rotate90().fliph(),
        Orientation::Rotate90Cw => img.rotate90(),
        Orientation::Rotate90CcwFlipH => img.rotate270().fliph(),
        Orientation::Rotate90Ccw => img.rotate270(),
    }
}

/// Load image from bytes and apply EXIF orientation correction.
/// This is the main entry point for loading images that need correct orientation.
pub fn load_image_with_orientation(image_bytes: &[u8]) -> Result<DynamicImage, image::ImageError> {
    let orientation = read_orientation(image_bytes);
    let img = image::load_from_memory(image_bytes)?;
    Ok(apply_orientation(img, orientation))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_orientation_from_value() {
        assert_eq!(Orientation::from(1), Orientation::Normal);
        assert_eq!(Orientation::from(6), Orientation::Rotate90Cw);
        assert_eq!(Orientation::from(8), Orientation::Rotate90Ccw);
        assert_eq!(Orientation::from(99), Orientation::Normal); // Unknown defaults to Normal
    }
}
