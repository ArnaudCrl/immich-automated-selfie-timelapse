//! Immich Selfie Timelapse - Create selfie timelapses from Immich.
//!
//! This library provides functionality to:
//! - Connect to an Immich server and fetch photos of a specific person
//! - Detect and align faces in images
//! - Compile aligned faces into a timelapse video

pub mod config;
pub mod error;
pub mod face_processing;
pub mod immich_api;
pub mod job;
pub mod video;
pub mod web;

pub use config::Config;
pub use error::{Error, Result};
