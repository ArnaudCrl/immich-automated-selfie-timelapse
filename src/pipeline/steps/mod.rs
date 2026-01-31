//! Pipeline step implementations.
//!
//! Each step implements the `ProcessingStep` trait and performs a specific
//! operation in the image processing pipeline.

mod face_resolution;
mod decode;
mod brightness;
mod crop;
mod resize;

pub use face_resolution::FaceResolutionStep;
pub use decode::DecodeImageStep;
pub use brightness::BrightnessStep;
pub use crop::CropFaceStep;
pub use resize::ResizeStep;
