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

    /// Detect 68 facial landmarks from a cropped face image.
    ///
    /// # Arguments
    /// * `width` - Image width
    /// * `height` - Image height
    /// * `pixels` - Raw RGB pixel data (width * height * 3 bytes)
    ///
    /// # Returns
    /// Landmarks struct containing the 68 facial landmark points, or an error
    /// if landmarks could not be detected.
    pub fn detect_landmarks(
        &self,
        width: usize,
        height: usize,
        pixels: &[u8],
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

        // Since we have a cropped face, create a rectangle covering the whole image
        let margin = 5;
        let face_rect = Rectangle {
            left: margin,
            top: margin,
            right: (width as i64) - margin,
            bottom: (height as i64) - margin,
        };

        // Try to detect face in the cropped image first
        let faces = detector.face_locations(&matrix);

        // Use detected face if found, otherwise use the whole-image rectangle
        let rect = if !faces.is_empty() {
            faces[0].clone()
        } else {
            face_rect
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
