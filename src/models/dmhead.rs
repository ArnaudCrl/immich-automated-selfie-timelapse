//! DMHead head pose estimation model wrapper.
//!
//! DMHead is a lightweight head pose estimation model that predicts yaw, pitch,
//! and roll angles from a cropped face image.
//!
//! Model source: https://github.com/PINTO0309/DMHead
//! Input: 224x224 RGB image
//! Output: [yaw, pitch, roll] in degrees

use crate::error::{Error, Result};
use crate::pipeline::HeadPose;
use image::DynamicImage;
use ndarray::Array4;
use ort::session::{builder::GraphOptimizationLevel, Session};
use std::path::Path;
use std::sync::{Mutex, OnceLock};

/// Global model instance for DMHead.
/// Loaded lazily on first use.
static DMHEAD_MODEL: OnceLock<Result<DMHeadModel>> = OnceLock::new();

/// DMHead ONNX model for head pose estimation.
pub struct DMHeadModel {
    session: Mutex<Session>,
}

impl DMHeadModel {
    /// Model input size (width and height).
    pub const INPUT_SIZE: u32 = 224;

    /// Load the DMHead model from the given path.
    pub fn load(model_path: impl AsRef<Path>) -> Result<Self> {
        let session = Session::builder()
            .map_err(|e| Error::Model(format!("Failed to create ONNX session builder: {}", e)))?
            .with_optimization_level(GraphOptimizationLevel::Level3)
            .map_err(|e| Error::Model(format!("Failed to set optimization level: {}", e)))?
            .commit_from_file(model_path.as_ref())
            .map_err(|e| {
                Error::Model(format!(
                    "Failed to load DMHead model from {}: {}",
                    model_path.as_ref().display(),
                    e
                ))
            })?;

        Ok(Self {
            session: Mutex::new(session),
        })
    }

    /// Get or initialize the global DMHead model instance.
    ///
    /// The model is loaded from `models/dmhead_nomask_Nx3x224x224.onnx` relative
    /// to the current working directory.
    pub fn global() -> Result<&'static DMHeadModel> {
        DMHEAD_MODEL
            .get_or_init(|| {
                let model_path = Path::new("models/dmhead_nomask_Nx3x224x224.onnx");
                if !model_path.exists() {
                    return Err(Error::Model(format!(
                        "DMHead model not found at {}. \
                         Download from: https://github.com/PINTO0309/DMHead/releases",
                        model_path.display()
                    )));
                }
                DMHeadModel::load(model_path)
            })
            .as_ref()
            .map_err(|e| Error::Model(e.to_string()))
    }

    /// Estimate head pose from a face image.
    ///
    /// The image should be a cropped face. It will be resized to 224x224 if necessary.
    ///
    /// Returns (yaw, pitch, roll) in degrees where:
    /// - Yaw: left/right rotation (-90 to +90, positive = looking right)
    /// - Pitch: up/down rotation (-90 to +90, positive = looking up)
    /// - Roll: head tilt (-90 to +90, positive = tilting right)
    pub fn estimate(&self, image: &DynamicImage) -> Result<HeadPose> {
        // Resize image to model input size
        let resized = image.resize_exact(
            Self::INPUT_SIZE,
            Self::INPUT_SIZE,
            image::imageops::FilterType::Triangle,
        );

        // Convert to RGB
        let rgb = resized.to_rgb8();
        let (width, height) = rgb.dimensions();

        // Create input tensor: [1, 3, 224, 224] in NCHW format
        let mut input_data = Array4::<f32>::zeros((1, 3, height as usize, width as usize));

        for y in 0..height {
            for x in 0..width {
                let pixel = rgb.get_pixel(x, y);
                // No normalization - DMHead expects raw [0, 255] pixel values as floats
                // See: https://github.com/PINTO0309/DMHead/blob/main/demo_video.py
                input_data[[0, 0, y as usize, x as usize]] = pixel[0] as f32; // R
                input_data[[0, 1, y as usize, x as usize]] = pixel[1] as f32; // G
                input_data[[0, 2, y as usize, x as usize]] = pixel[2] as f32; // B
            }
        }

        // Flatten the array for ort input (ort 2.0 requires owned data)
        let shape = [1_usize, 3, height as usize, width as usize];
        let (input_vec, _offset) = input_data.into_raw_vec_and_offset();

        // Create input tensor
        let input_tensor = ort::value::Tensor::from_array((shape, input_vec))
            .map_err(|e| Error::Model(format!("Failed to create input tensor: {}", e)))?;

        // Run inference
        let mut session = self
            .session
            .lock()
            .map_err(|e| Error::Model(format!("Failed to lock session: {}", e)))?;

        // Get the input name from the model (don't assume it's "input")
        let input_name = session
            .inputs()
            .first()
            .map(|i| i.name().to_string())
            .unwrap_or_else(|| "input".to_string());

        let outputs = session
            .run(ort::inputs![input_name => input_tensor])
            .map_err(|e| Error::Model(format!("DMHead inference failed: {}", e)))?;

        // Extract output: [1, 3] containing [yaw, pitch, roll]
        // Get the first output (model may use different output names)
        let output_value = outputs
            .iter()
            .next()
            .map(|(_, v)| v)
            .ok_or_else(|| Error::Model("DMHead model returned no outputs".to_string()))?;

        let (_shape, output_data) = output_value
            .try_extract_tensor::<f32>()
            .map_err(|e| Error::Model(format!("Failed to extract output tensor: {}", e)))?;

        if output_data.len() < 3 {
            return Err(Error::Model(format!(
                "Expected 3 output values, got {}",
                output_data.len()
            )));
        }

        let yaw = output_data[0];
        let pitch = output_data[1];
        let roll = output_data[2];

        Ok(HeadPose { yaw, pitch, roll })
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_input_format() {
        // DMHead expects raw [0, 255] pixel values as floats, no normalization
        // See: https://github.com/PINTO0309/DMHead/blob/main/demo_video.py
        assert_eq!(0_u8 as f32, 0.0);
        assert_eq!(255_u8 as f32, 255.0);
        assert_eq!(128_u8 as f32, 128.0);
    }
}
