//! Image processing module.
//!
//! This module handles face detection, landmark detection,
//! alignment, and image transformation.

mod crop;
pub mod debug;
mod orientation;
mod types;

pub use crop::*;
pub use orientation::load_image_with_orientation;
pub use types::*;

// TODO: Implement these modules
// mod landmarks;
// mod alignment;
// mod filters;
