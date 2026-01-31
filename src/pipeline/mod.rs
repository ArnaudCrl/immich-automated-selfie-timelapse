//! Extensible image processing pipeline.
//!
//! This module provides a trait-based pipeline architecture for processing images.
//! Steps can be validators (skip if condition fails), transformers (modify image),
//! or computers (calculate values for later steps).
//!
//! # Example
//!
//! ```ignore
//! use crate::pipeline::{Pipeline, PipelineContext};
//!
//! let pipeline = Pipeline::default();
//! let result = pipeline.execute(ctx, &config, &cancel_token, &skip_stats).await;
//! ```

mod traits;
pub mod steps;

pub use traits::*;

use crate::config::Config;
use crate::web::AtomicSkipStats;
use image::{DynamicImage, ImageFormat};
use std::io::Cursor;
use std::path::PathBuf;
use std::sync::Arc;
use tokio_util::sync::CancellationToken;

/// Result of pipeline execution.
#[derive(Debug)]
pub enum PipelineResult {
    /// Successfully processed the image.
    Success {
        /// The final processed image.
        image: DynamicImage,
        /// Asset ID for reference.
        asset_id: String,
        /// Timestamp for file naming.
        timestamp: String,
        /// Debug images if keep_intermediates was enabled.
        debug_images: Vec<(String, DynamicImage)>,
    },
    /// Image was skipped.
    Skipped {
        asset_id: String,
        reason: String,
        detail: Option<String>,
    },
    /// Processing error.
    Error {
        asset_id: String,
        error: String,
    },
    /// Cancelled by user.
    Cancelled {
        asset_id: String,
    },
}

/// Processing pipeline that executes steps sequentially.
pub struct Pipeline {
    steps: Vec<Box<dyn ProcessingStep>>,
}

impl std::fmt::Debug for Pipeline {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Pipeline")
            .field("steps", &self.steps.iter().map(|s| s.id()).collect::<Vec<_>>())
            .finish()
    }
}

impl Default for Pipeline {
    fn default() -> Self {
        Self::new()
    }
}

impl Pipeline {
    /// Create a new empty pipeline.
    pub fn new() -> Self {
        Self { steps: Vec::new() }
    }

    /// Create the default processing pipeline with standard steps.
    pub fn with_default_steps() -> Self {
        use steps::*;

        let mut pipeline = Self::new();
        pipeline.add_step(Box::new(FaceResolutionStep));
        pipeline.add_step(Box::new(DecodeImageStep));
        pipeline.add_step(Box::new(BrightnessStep));
        pipeline.add_step(Box::new(CropFaceStep));
        pipeline.add_step(Box::new(ResizeStep));
        pipeline
    }

    /// Add a step to the pipeline.
    pub fn add_step(&mut self, step: Box<dyn ProcessingStep>) {
        self.steps.push(step);
    }

    /// Get the list of step IDs in this pipeline.
    pub fn step_ids(&self) -> Vec<&'static str> {
        self.steps.iter().map(|s| s.id()).collect()
    }

    /// Execute the pipeline on the given context.
    ///
    /// Runs each step sequentially, checking for cancellation between steps.
    /// Updates skip statistics for any skipped images.
    pub async fn execute(
        &self,
        mut ctx: PipelineContext,
        config: &Config,
        cancel_token: &CancellationToken,
        skip_stats: &Arc<AtomicSkipStats>,
        debug_dir: Option<&PathBuf>,
    ) -> PipelineResult {
        let asset_id = ctx.asset_id.clone();
        let timestamp = ctx.timestamp.clone();

        for step in &self.steps {
            // Check for cancellation before each step
            if cancel_token.is_cancelled() {
                return PipelineResult::Cancelled { asset_id };
            }

            tracing::trace!("Executing step '{}' for asset {}", step.id(), asset_id);

            match step.execute(ctx, config).await {
                StepOutcome::Continue(new_ctx) => {
                    ctx = new_ctx;

                    // Generate debug visualization if enabled
                    if config.processing.output.keep_intermediates {
                        if let Some(debug_img) = step.debug_visualize(&ctx) {
                            ctx.add_debug_image(step.id(), debug_img);
                        }
                    }
                }
                StepOutcome::Skip { reason, detail } => {
                    // Update skip stats with the step ID as reason
                    skip_stats.increment(&reason);

                    tracing::debug!(
                        "Asset {} skipped at step '{}': {} ({})",
                        asset_id,
                        step.id(),
                        reason,
                        detail.as_deref().unwrap_or("no detail")
                    );

                    return PipelineResult::Skipped {
                        asset_id,
                        reason,
                        detail,
                    };
                }
                StepOutcome::Error(error) => {
                    tracing::warn!(
                        "Asset {} error at step '{}': {}",
                        asset_id,
                        step.id(),
                        error
                    );

                    return PipelineResult::Error { asset_id, error };
                }
            }
        }

        // All steps completed successfully
        let final_image = match ctx.image {
            Some(img) => img,
            None => {
                return PipelineResult::Error {
                    asset_id,
                    error: "Pipeline completed but no image was produced".to_string(),
                };
            }
        };

        // Save debug images if enabled
        let debug_images: Vec<(String, DynamicImage)> = ctx.debug_images.into_iter().collect();

        if let Some(debug_base) = debug_dir {
            for (step_id, debug_img) in &debug_images {
                let step_dir = debug_base.join(step_id);
                if let Err(e) = tokio::fs::create_dir_all(&step_dir).await {
                    tracing::warn!("Failed to create debug dir {}: {}", step_dir.display(), e);
                    continue;
                }

                let filename = format!("{}_{}.jpg", timestamp, asset_id)
                    .chars()
                    .map(|c| if c.is_alphanumeric() || c == '-' || c == '_' || c == '.' { c } else { '_' })
                    .collect::<String>();

                let debug_path = step_dir.join(&filename);
                let mut buffer = Cursor::new(Vec::new());

                if let Err(e) = debug_img.write_to(&mut buffer, ImageFormat::Jpeg) {
                    tracing::warn!("Failed to encode debug image: {}", e);
                    continue;
                }

                if let Err(e) = tokio::fs::write(&debug_path, buffer.into_inner()).await {
                    tracing::warn!("Failed to save debug image: {}", e);
                }
            }
        }

        PipelineResult::Success {
            image: final_image,
            asset_id,
            timestamp,
            debug_images,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pipeline_step_ids() {
        let pipeline = Pipeline::with_default_steps();
        let ids = pipeline.step_ids();

        assert!(ids.contains(&"face_resolution"));
        assert!(ids.contains(&"decode"));
        assert!(ids.contains(&"brightness"));
        assert!(ids.contains(&"crop"));
        assert!(ids.contains(&"resize"));
    }
}
