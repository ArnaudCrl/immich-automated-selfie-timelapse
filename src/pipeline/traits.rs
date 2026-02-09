//! Pipeline traits and core types.
//!
//! Defines the `ProcessingStep` trait and associated types for building
//! extensible image processing pipelines.

use crate::config::Config;
use crate::immich_api::FaceData;
use crate::pipeline::types::{BoundingBox, HeadPose, Landmarks};
use async_trait::async_trait;
use bytes::Bytes;
use image::DynamicImage;
use std::collections::HashMap;
use std::fmt;

/// Keys for computed values stored in `PipelineContext`.
/// Use these constants to avoid typos and make dependencies explicit.
pub mod computed_keys {
    /// Brightness value (0.0 - 1.0) computed by BrightnessStep.
    pub const BRIGHTNESS: &str = "brightness";
    /// Blur metric (gradient magnitude) computed by BlurStep.
    pub const BLUR_METRIC: &str = "blur_metric";
    /// Face size in pixels computed by FaceResolutionStep.
    pub const FACE_SIZE: &str = "face_size";
    /// Eye Aspect Ratio computed by LandmarksStep.
    pub const EAR: &str = "ear";
    /// Facial landmarks (68 points) computed by LandmarksStep.
    pub const LANDMARKS: &str = "landmarks";
    /// Head pose (yaw, pitch, roll) computed by HeadPoseStep.
    pub const HEAD_POSE: &str = "head_pose";
    /// Face bounding box in current image coordinates computed by CropAndResizeStep.
    pub const FACE_RECT: &str = "face_rect";
}

/// Outcome of a pipeline step execution.
#[derive(Debug)]
pub enum StepOutcome {
    /// Continue to the next step with the updated context.
    Continue(PipelineContext),
    /// Skip this image with the given reason. Context is returned for debug visualization.
    Skip {
        ctx: PipelineContext,
        reason: String,
        detail: Option<String>,
    },
    /// An error occurred during processing. Context is preserved for debug visualization.
    Error { ctx: PipelineContext, error: String },
}

/// Values computed by pipeline steps that can be shared with subsequent steps.
#[derive(Debug, Clone)]
pub enum ComputedValue {
    /// A floating-point value (e.g., brightness, EAR).
    Float(f32),
    /// An integer value (e.g., face size in pixels).
    Int(i32),
    /// A boolean value (e.g., eyes_open).
    Bool(bool),
    /// A string value.
    String(String),
    /// Head pose estimation result (yaw, pitch, roll).
    HeadPose(HeadPose),
    /// Facial landmarks (68 points).
    Landmarks(Box<Landmarks>),
    /// Face bounding box in current image coordinates.
    FaceRect(BoundingBox),
}

impl ComputedValue {
    /// Get as f32 if this is a Float variant.
    pub fn as_float(&self) -> Option<f32> {
        match self {
            ComputedValue::Float(v) => Some(*v),
            _ => None,
        }
    }

    /// Get as i32 if this is an Int variant.
    pub fn as_int(&self) -> Option<i32> {
        match self {
            ComputedValue::Int(v) => Some(*v),
            _ => None,
        }
    }

    /// Get as bool if this is a Bool variant.
    pub fn as_bool(&self) -> Option<bool> {
        match self {
            ComputedValue::Bool(v) => Some(*v),
            _ => None,
        }
    }

    /// Get as &str if this is a String variant.
    pub fn as_str(&self) -> Option<&str> {
        match self {
            ComputedValue::String(v) => Some(v),
            _ => None,
        }
    }

    /// Get as HeadPose if this is a HeadPose variant.
    pub fn as_head_pose(&self) -> Option<&HeadPose> {
        match self {
            ComputedValue::HeadPose(v) => Some(v),
            _ => None,
        }
    }

    /// Get as Landmarks if this is a Landmarks variant.
    pub fn as_landmarks(&self) -> Option<&Landmarks> {
        match self {
            ComputedValue::Landmarks(v) => Some(v),
            _ => None,
        }
    }

    /// Get as BoundingBox if this is a FaceRect variant.
    pub fn as_face_rect(&self) -> Option<&BoundingBox> {
        match self {
            ComputedValue::FaceRect(v) => Some(v),
            _ => None,
        }
    }
}

/// A debug image with metadata about whether the step passed or failed.
#[derive(Debug, Clone)]
pub struct DebugImage {
    /// The debug visualization image.
    pub image: DynamicImage,
    /// Whether the step that generated this image passed (true) or failed/skipped (false).
    pub passed: bool,
}

impl DebugImage {
    /// Create a new debug image.
    pub fn new(image: DynamicImage, passed: bool) -> Self {
        Self { image, passed }
    }
}

/// Context passed through the pipeline, carrying data between steps.
#[derive(Debug)]
pub struct PipelineContext {
    /// The asset ID from Immich.
    pub asset_id: String,
    /// Timestamp for sorting/naming output files.
    pub timestamp: String,
    /// Raw image bytes (before decoding).
    pub raw_bytes: Option<Bytes>,
    /// Decoded image (after decode step).
    pub image: Option<DynamicImage>,
    /// Face bounding box and metadata from Immich.
    pub face_data: FaceData,
    /// Values computed by previous steps (e.g., brightness, landmarks).
    pub computed: HashMap<String, ComputedValue>,
    /// Debug images generated by steps (step_id -> debug image with pass/fail status).
    pub debug_images: HashMap<String, DebugImage>,
}

impl PipelineContext {
    /// Create a new pipeline context.
    pub fn new(asset_id: String, timestamp: String, face_data: FaceData) -> Self {
        Self {
            asset_id,
            timestamp,
            raw_bytes: None,
            image: None,
            face_data,
            computed: HashMap::new(),
            debug_images: HashMap::new(),
        }
    }

    /// Set the raw image bytes.
    pub fn with_bytes(mut self, bytes: Bytes) -> Self {
        self.raw_bytes = Some(bytes);
        self
    }

    /// Set the decoded image.
    pub fn with_image(mut self, image: DynamicImage) -> Self {
        self.image = Some(image);
        self
    }

    /// Add a computed value.
    pub fn set_computed(&mut self, key: impl Into<String>, value: ComputedValue) {
        self.computed.insert(key.into(), value);
    }

    /// Get a computed value.
    pub fn get_computed(&self, key: &str) -> Option<&ComputedValue> {
        self.computed.get(key)
    }

    /// Get a reference to the image, returning an error message if not available.
    ///
    /// Use this in pipeline steps that need to read the image without modifying it.
    ///
    /// # Example
    /// ```ignore
    /// let image = ctx.require_image("brightness check")?;
    /// ```
    pub fn require_image(&self, step_name: &str) -> Result<&DynamicImage, String> {
        self.image
            .as_ref()
            .ok_or_else(|| format!("No image available for {}", step_name))
    }

    /// Take ownership of the image, returning an error message if not available.
    ///
    /// Use this in pipeline steps that need to transform the image (alignment, resize).
    /// The step should set `ctx.image` to the transformed result before returning.
    ///
    /// # Example
    /// ```ignore
    /// let image = ctx.take_image("alignment")?;
    /// // ... transform image ...
    /// ctx.image = Some(transformed);
    /// ```
    pub fn take_image(&mut self, step_name: &str) -> Result<DynamicImage, String> {
        self.image
            .take()
            .ok_or_else(|| format!("No image available for {}", step_name))
    }

    /// Add a debug image for a step.
    ///
    /// # Arguments
    /// * `step_id` - The identifier of the step that generated this image
    /// * `image` - The debug visualization image
    /// * `passed` - Whether the step passed (true) or failed/skipped (false)
    pub fn add_debug_image(
        &mut self,
        step_id: impl Into<String>,
        image: DynamicImage,
        passed: bool,
    ) {
        self.debug_images
            .insert(step_id.into(), DebugImage::new(image, passed));
    }
}

/// A single step in the processing pipeline.
///
/// Steps can be:
/// - **Validators**: Skip the image if a condition fails (return `StepOutcome::Skip`)
/// - **Transformers**: Modify the image (update `ctx.image`)
/// - **Computers**: Calculate values for later steps (update `ctx.computed`)
#[async_trait]
pub trait ProcessingStep: Send + Sync {
    /// Unique identifier for this step (used in skip stats and debug output).
    fn id(&self) -> &'static str;

    /// Human-readable name for display.
    fn name(&self) -> &'static str;

    /// Computed value keys that this step depends on (must be present in `ctx.computed`).
    ///
    /// Return a list of keys from `computed_keys` that must be available for this step.
    /// The pipeline will verify these dependencies are satisfied before execution.
    fn dependencies(&self) -> Vec<&'static str> {
        vec![]
    }

    /// Computed value keys that this step provides (sets in `ctx.computed`).
    ///
    /// Return a list of keys from `computed_keys` that this step will set.
    /// This is used for documentation and dependency verification.
    fn provides(&self) -> Vec<&'static str> {
        vec![]
    }

    /// Execute this step on the given context.
    ///
    /// Returns:
    /// - `StepOutcome::Continue(ctx)` to proceed to the next step
    /// - `StepOutcome::Skip { reason, detail }` to skip this image
    /// - `StepOutcome::Error(msg)` if processing failed
    async fn execute(&self, ctx: PipelineContext, config: &Config) -> StepOutcome;

    /// Generate a debug visualization for this step (optional).
    ///
    /// Called after `execute()` if `keep_intermediates` is enabled and the step
    /// returned `Continue` or `Skip`. The returned image is saved to the debug folder.
    ///
    /// Only steps included in the pipeline (via `Pipeline::with_steps_from_config`)
    /// will have their debug images generated - disabled steps are not in the pipeline.
    fn debug_visualize(&self, _ctx: &PipelineContext, _config: &Config) -> Option<DynamicImage> {
        None
    }
}

impl fmt::Debug for dyn ProcessingStep {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ProcessingStep")
            .field("id", &self.id())
            .field("name", &self.name())
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_computed_value_conversions() {
        let float_val = ComputedValue::Float(0.5);
        assert_eq!(float_val.as_float(), Some(0.5));
        assert_eq!(float_val.as_int(), None);

        let int_val = ComputedValue::Int(100);
        assert_eq!(int_val.as_int(), Some(100));
        assert_eq!(int_val.as_float(), None);

        let bool_val = ComputedValue::Bool(true);
        assert_eq!(bool_val.as_bool(), Some(true));

        let str_val = ComputedValue::String("test".to_string());
        assert_eq!(str_val.as_str(), Some("test"));
    }

    #[test]
    fn test_pipeline_context_computed_values() {
        let face_data = FaceData {
            bounding_box_x1: 0.0,
            bounding_box_y1: 0.0,
            bounding_box_x2: 100.0,
            bounding_box_y2: 100.0,
            image_width: 1920,
            image_height: 1080,
        };

        let mut ctx =
            PipelineContext::new("asset123".to_string(), "2024-01-15".to_string(), face_data);

        ctx.set_computed(computed_keys::BRIGHTNESS, ComputedValue::Float(0.65));
        ctx.set_computed(computed_keys::FACE_SIZE, ComputedValue::Int(150));

        assert_eq!(
            ctx.get_computed(computed_keys::BRIGHTNESS)
                .and_then(|v| v.as_float()),
            Some(0.65)
        );
        assert_eq!(
            ctx.get_computed(computed_keys::FACE_SIZE)
                .and_then(|v| v.as_int()),
            Some(150)
        );
        assert!(ctx.get_computed("nonexistent").is_none());
    }
}
