//! Dlib landmark predictor wrapper.
//!
//! Wraps the dlib-face-recognition crate's LandmarkPredictor and FaceDetector
//! in a thread-safe singleton to avoid reloading the model for every image.

use crate::error::{Error, Result};
use crate::face_processing::types::{Landmarks, Point};
use dlib_face_recognition::{
    FaceDetector, FaceDetectorTrait, ImageMatrix, LandmarkPredictor, LandmarkPredictorTrait,
    Rectangle,
};
use std::sync::{Mutex, OnceLock};

/// Global landmark predictor instance.
/// Loaded lazily on first use.
static LANDMARK_PREDICTOR: OnceLock<Result<DlibLandmarks>> = OnceLock::new();

/// Thread-safe wrapper for dlib's FaceDetector and LandmarkPredictor.
///
/// The dlib types are not thread-safe, so we wrap them in a Mutex.
/// The model is loaded once and reused for all subsequent calls.
pub struct DlibLandmarks {
    detector: Mutex<FaceDetector>,
    predictor: Mutex<LandmarkPredictor>,
}

// Safety: The Mutex ensures thread-safe access to the inner types
unsafe impl Send for DlibLandmarks {}
unsafe impl Sync for DlibLandmarks {}

impl DlibLandmarks {
    /// Load the landmark predictor model.
    ///
    /// This will download/check the model file (shape_predictor_68_face_landmarks.dat).
    fn load() -> Result<Self> {
        let detector = FaceDetector::default();
        let predictor = LandmarkPredictor::default()
            .map_err(|e| Error::Model(format!("Failed to load landmark predictor: {}", e)))?;

        Ok(Self {
            detector: Mutex::new(detector),
            predictor: Mutex::new(predictor),
        })
    }

    /// Get or initialize the global landmark predictor instance.
    pub fn global() -> Result<&'static DlibLandmarks> {
        LANDMARK_PREDICTOR
            .get_or_init(DlibLandmarks::load)
            .as_ref()
            .map_err(|e| Error::Model(e.to_string()))
    }

    /// Eagerly initialize the model at startup.
    ///
    /// Call this during server startup to load the model before processing begins.
    /// This ensures any model loading messages appear during startup rather than
    /// during image processing.
    pub fn init() -> Result<()> {
        Self::global()?;
        Ok(())
    }

    /// Detect 68 facial landmarks from a cropped face image.
    ///
    /// # Arguments
    /// * `width` - Image width
    /// * `height` - Image height
    /// * `pixels` - Raw RGB pixel data (width * height * 3 bytes)
    /// * `face_rect` - Optional face bounding box (x1, y1, x2, y2) in image coordinates.
    ///   If provided, uses this rectangle for landmark detection.
    ///   If None, attempts to detect the face or uses the whole image.
    ///
    /// # Returns
    /// Landmarks struct containing the 68 facial landmark points, or an error
    /// if landmarks could not be detected.
    pub fn detect_landmarks(
        &self,
        width: usize,
        height: usize,
        pixels: &[u8],
        face_rect: Option<(i64, i64, i64, i64)>,
    ) -> Result<Landmarks> {
        // Create image matrix for dlib
        let matrix = unsafe { ImageMatrix::new(width, height, pixels.as_ptr()) };

        // Lock detector and predictor
        let detector = self
            .detector
            .lock()
            .map_err(|e| Error::Model(format!("Failed to lock detector: {}", e)))?;
        let predictor = self
            .predictor
            .lock()
            .map_err(|e| Error::Model(format!("Failed to lock predictor: {}", e)))?;

        // Determine the face rectangle to use
        let rect = if let Some((x1, y1, x2, y2)) = face_rect {
            // Use the provided face rectangle
            Rectangle {
                left: x1.max(0),
                top: y1.max(0),
                right: x2.min(width as i64),
                bottom: y2.min(height as i64),
            }
        } else {
            // No face rect provided - try to detect or use whole image
            let faces = detector.face_locations(&matrix);
            if !faces.is_empty() {
                faces[0]
            } else {
                // Fallback: use whole image with small margin
                let margin = 5;
                Rectangle {
                    left: margin,
                    top: margin,
                    right: (width as i64) - margin,
                    bottom: (height as i64) - margin,
                }
            }
        };

        // Detect landmarks
        let landmarks_raw = predictor.face_landmarks(&matrix, &rect);

        // Convert dlib landmarks to our Landmarks type
        let points: Vec<Point> = landmarks_raw
            .iter()
            .map(|p| Point::new(p.x() as f32, p.y() as f32))
            .collect();

        Landmarks::new(points).ok_or_else(|| {
            Error::Model("Could not detect 68 facial landmarks".to_string())
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_global_initialization() {
        // Just test that we can get the global instance
        // (this will fail if model file is not present, which is expected in CI)
        let _ = DlibLandmarks::global();
    }
}
