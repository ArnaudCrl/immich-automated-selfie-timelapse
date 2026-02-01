//! ML model wrappers for face processing.
//!
//! This module provides wrappers for machine learning models used in the
//! face processing pipeline.

pub mod dlib_landmarks;
pub mod dmhead;

pub use dlib_landmarks::DlibLandmarks;
pub use dmhead::DMHeadModel;
