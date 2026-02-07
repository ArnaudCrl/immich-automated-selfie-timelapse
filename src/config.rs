//! Application configuration.
//!
//! Configuration can be loaded from:
//! 1. TOML file (config.toml)
//! 2. Environment variables (prefixed with IMMICH_)
//! 3. Programmatic overrides
//!
//! ## Step Configuration Pattern
//!
//! Each pipeline step has its own optional sub-config with an `enabled` flag:
//! ```ignore
//! processing:
//!   face_resolution:
//!     enabled: true
//!     min_size: 80
//!   brightness:
//!     enabled: false
//!     min_brightness: 0.1
//!     max_brightness: 0.95
//! ```

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

use crate::error::{Error, Result};

/// Main configuration struct.
///
/// This is the single source of truth for all configuration.
/// Pass this explicitly to functions that need it - no globals.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    /// Immich API configuration.
    pub api: ApiConfig,

    /// Face processing parameters.
    pub processing: ProcessingConfig,

    /// Video output settings.
    pub video: VideoConfig,

    /// Output directory for processed images and video.
    pub output_dir: PathBuf,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            api: ApiConfig::default(),
            processing: ProcessingConfig::default(),
            video: VideoConfig::default(),
            output_dir: PathBuf::from("output"),
        }
    }
}

/// Immich API connection settings.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiConfig {
    /// API key for authentication.
    pub api_key: String,

    /// Base URL of the Immich instance (e.g., "http://192.168.1.94:2283/api").
    pub base_url: String,

    /// Request timeout in seconds.
    #[serde(default = "default_timeout")]
    pub timeout_secs: u64,
}

fn default_timeout() -> u64 {
    30
}

impl Default for ApiConfig {
    fn default() -> Self {
        Self {
            api_key: String::new(),
            base_url: String::new(),
            timeout_secs: default_timeout(),
        }
    }
}

// ============================================================================
// Step Configurations
// ============================================================================

/// Face resolution validation configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FaceResolutionConfig {
    /// Whether face resolution validation is enabled.
    pub enabled: bool,

    /// Minimum face width/height in pixels.
    /// Faces smaller than this will be skipped.
    pub min_size: u32,
}

impl Default for FaceResolutionConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            min_size: 80,
        }
    }
}

impl FaceResolutionConfig {
    /// Validate the configuration values.
    pub fn validate(&self) -> Result<()> {
        if self.enabled && self.min_size == 0 {
            return Err(Error::Config(
                "Face resolution min_size must be greater than 0".to_string(),
            ));
        }
        Ok(())
    }
}

/// Brightness validation configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BrightnessConfig {
    /// Whether brightness validation is enabled.
    pub enabled: bool,

    /// Minimum acceptable brightness (0.0-1.0).
    /// Images darker than this will be skipped.
    pub min_brightness: f32,

    /// Maximum acceptable brightness (0.0-1.0).
    /// Images brighter than this will be skipped.
    pub max_brightness: f32,
}

impl Default for BrightnessConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            min_brightness: 0.1,
            max_brightness: 0.95,
        }
    }
}

impl BrightnessConfig {
    /// Validate the configuration values.
    pub fn validate(&self) -> Result<()> {
        if !self.enabled {
            return Ok(());
        }
        if self.min_brightness < 0.0 || self.min_brightness > 1.0 {
            return Err(Error::Config(
                "Brightness min_brightness must be between 0.0 and 1.0".to_string(),
            ));
        }
        if self.max_brightness < 0.0 || self.max_brightness > 1.0 {
            return Err(Error::Config(
                "Brightness max_brightness must be between 0.0 and 1.0".to_string(),
            ));
        }
        if self.min_brightness >= self.max_brightness {
            return Err(Error::Config(
                "Brightness min_brightness must be less than max_brightness".to_string(),
            ));
        }
        Ok(())
    }
}

/// Output/resize configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OutputConfig {
    /// Output image size (width and height in pixels).
    pub size: u32,

    /// Keep intermediate images (original, cropped) for inspection.
    pub keep_intermediates: bool,
}

impl Default for OutputConfig {
    fn default() -> Self {
        Self {
            size: 512,
            keep_intermediates: false,
        }
    }
}

impl OutputConfig {
    /// Validate the configuration values.
    pub fn validate(&self) -> Result<()> {
        if self.size < 64 {
            return Err(Error::Config(
                "Output size must be at least 64 pixels".to_string(),
            ));
        }
        if self.size > 4096 {
            return Err(Error::Config(
                "Output size must be at most 4096 pixels".to_string(),
            ));
        }
        Ok(())
    }
}

/// Head pose estimation configuration.
///
/// Uses DMHead ONNX model to estimate head pose angles and filter
/// non-front-facing faces.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HeadPoseConfig {
    /// Whether head pose filtering is enabled.
    pub enabled: bool,

    /// Maximum allowed yaw angle (left/right turn) in degrees.
    pub max_yaw: f32,

    /// Maximum allowed pitch angle (up/down tilt) in degrees.
    pub max_pitch: f32,

    /// Maximum allowed roll angle (head tilt) in degrees.
    pub max_roll: f32,
}

impl Default for HeadPoseConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            max_yaw: 35.0,
            max_pitch: 35.0,
            max_roll: 25.0,
        }
    }
}

impl HeadPoseConfig {
    /// Validate the configuration values.
    pub fn validate(&self) -> Result<()> {
        if self.enabled {
            if self.max_yaw < 0.0 || self.max_yaw > 90.0 {
                return Err(Error::Config(
                    "Head pose max_yaw must be between 0 and 90 degrees".to_string(),
                ));
            }
            if self.max_pitch < 0.0 || self.max_pitch > 90.0 {
                return Err(Error::Config(
                    "Head pose max_pitch must be between 0 and 90 degrees".to_string(),
                ));
            }
            if self.max_roll < 0.0 || self.max_roll > 90.0 {
                return Err(Error::Config(
                    "Head pose max_roll must be between 0 and 90 degrees".to_string(),
                ));
            }
        }
        Ok(())
    }
}

/// Eye filter configuration for blink detection.
///
/// Uses Eye Aspect Ratio (EAR) computed from facial landmarks to detect
/// closed eyes.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EyeFilterConfig {
    /// Whether eye filtering is enabled.
    pub enabled: bool,

    /// Minimum Eye Aspect Ratio (EAR) threshold.
    /// Eyes with EAR below this are considered closed.
    pub min_ear: f32,
}

impl Default for EyeFilterConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            min_ear: 0.2,
        }
    }
}

impl EyeFilterConfig {
    /// Validate the configuration values.
    pub fn validate(&self) -> Result<()> {
        if self.enabled && !(0.0..=0.5).contains(&self.min_ear) {
            return Err(Error::Config(
                "Eye filter min_ear must be between 0.0 and 0.5".to_string(),
            ));
        }
        Ok(())
    }
}

/// Face alignment configuration for landmark-based alignment.
///
/// Aligns faces based on eye positions detected from facial landmarks,
/// ensuring consistent eye placement across all images for smoother timelapses.
///
/// Note: Face alignment is always enabled and is a core part of the pipeline.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AlignmentConfig {
    /// Target Y position for left eye as percentage from top (0.0-1.0).
    /// Default 0.4 places left eye at 40% from the top.
    /// Both eyes are positioned at the same vertical level.
    pub left_eye_y_position: f32,

    /// Target X position for left eye as percentage from left (0.0-1.0).
    /// Default 0.35 places left eye at 35% from the left edge.
    /// Right eye will be placed at (1.0 - left_eye_x_position).
    pub left_eye_x_position: f32,
}

impl Default for AlignmentConfig {
    fn default() -> Self {
        Self {
            left_eye_y_position: 0.4,
            left_eye_x_position: 0.35,
        }
    }
}

impl AlignmentConfig {
    /// Validate the configuration values.
    pub fn validate(&self) -> Result<()> {
        if self.left_eye_y_position <= 0.0 || self.left_eye_y_position >= 1.0 {
            return Err(Error::Config(
                "Alignment left_eye_y_position must be between 0.0 and 1.0 (exclusive)".to_string(),
            ));
        }
        if self.left_eye_y_position < 0.2 || self.left_eye_y_position > 0.6 {
            return Err(Error::Config(
                "Alignment left_eye_y_position should be between 0.2 and 0.6 for best results"
                    .to_string(),
            ));
        }
        if self.left_eye_x_position <= 0.0 || self.left_eye_x_position >= 0.5 {
            return Err(Error::Config(
                "Alignment left_eye_x_position must be between 0.0 and 0.5 (left eye must be in left half)"
                    .to_string(),
            ));
        }
        if self.left_eye_x_position < 0.25 || self.left_eye_x_position > 0.45 {
            return Err(Error::Config(
                "Alignment left_eye_x_position should be between 0.25 and 0.45 for best results"
                    .to_string(),
            ));
        }
        Ok(())
    }

    /// Calculate the target inter-eye distance as a fraction of output width.
    /// Since left eye is at left_eye_x_position and right eye is at (1 - left_eye_x_position),
    /// the distance between them is: (1 - left_eye_x_position) - left_eye_x_position = 1 - 2*left_eye_x_position
    pub fn inter_eye_distance(&self) -> f32 {
        1.0 - 2.0 * self.left_eye_x_position
    }
}

// ============================================================================
// Main Processing Configuration
// ============================================================================

/// Face processing parameters.
///
/// Each pipeline step has its own sub-configuration with an `enabled` flag.
/// This allows fine-grained control over which steps run and their parameters.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct ProcessingConfig {
    /// Number of parallel workers for processing.
    pub max_workers: usize,

    /// Face resolution validation settings.
    #[serde(default)]
    pub face_resolution: FaceResolutionConfig,

    /// Brightness validation settings.
    #[serde(default)]
    pub brightness: BrightnessConfig,

    /// Head pose estimation settings.
    #[serde(default)]
    pub head_pose: HeadPoseConfig,

    /// Eye filter settings (blink detection).
    #[serde(default)]
    pub eye_filter: EyeFilterConfig,

    /// Output image settings.
    #[serde(default)]
    pub output: OutputConfig,

    /// Face alignment settings (landmark-based).
    #[serde(default)]
    pub alignment: AlignmentConfig,
}

impl Default for ProcessingConfig {
    fn default() -> Self {
        Self {
            max_workers: num_cpus(),
            face_resolution: FaceResolutionConfig::default(),
            brightness: BrightnessConfig::default(),
            head_pose: HeadPoseConfig::default(),
            eye_filter: EyeFilterConfig::default(),
            output: OutputConfig::default(),
            alignment: AlignmentConfig::default(),
        }
    }
}

impl ProcessingConfig {
    /// Validate all step configurations.
    pub fn validate(&self) -> Result<()> {
        self.face_resolution.validate()?;
        self.brightness.validate()?;
        self.head_pose.validate()?;
        self.eye_filter.validate()?;
        self.output.validate()?;
        self.alignment.validate()?;
        Ok(())
    }
}

fn num_cpus() -> usize {
    std::thread::available_parallelism()
        .map(|p| p.get())
        .unwrap_or(4)
}

// ============================================================================
// Video Configuration
// ============================================================================

/// Video compilation settings.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct VideoConfig {
    /// Whether to compile video after processing.
    pub enabled: bool,

    /// Output video framerate.
    pub framerate: u32,

    /// Video codec (e.g., "libx264").
    pub codec: String,

    /// Constant Rate Factor for quality (0-51, lower = better, 18-28 recommended).
    pub crf: u32,
}

impl Default for VideoConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            framerate: 15,
            codec: "libx264".to_string(),
            crf: 23,
        }
    }
}

impl VideoConfig {
    /// Validate the configuration values.
    pub fn validate(&self) -> Result<()> {
        if self.framerate == 0 {
            return Err(Error::Config(
                "Video framerate must be greater than 0".to_string(),
            ));
        }
        if self.framerate > 120 {
            return Err(Error::Config(
                "Video framerate must be at most 120".to_string(),
            ));
        }
        if self.crf > 51 {
            return Err(Error::Config(
                "Video CRF must be between 0 and 51".to_string(),
            ));
        }
        Ok(())
    }
}

// ============================================================================
// Persistence
// ============================================================================

/// Persistable configuration (excludes sensitive API credentials).
///
/// This struct contains only the settings that can safely be written to disk.
/// API credentials should always come from environment variables.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PersistableConfig {
    pub processing: ProcessingConfig,
    pub video: VideoConfig,
}

impl From<&Config> for PersistableConfig {
    fn from(config: &Config) -> Self {
        Self {
            processing: config.processing.clone(),
            video: config.video.clone(),
        }
    }
}

// ============================================================================
// Config Loading and Validation
// ============================================================================

impl Config {
    /// Load configuration from a TOML file.
    pub fn from_file(path: impl AsRef<std::path::Path>) -> Result<Self> {
        let content = std::fs::read_to_string(path)?;
        let config: Config = toml::from_str(&content).map_err(|e| Error::Config(e.to_string()))?;
        Ok(config)
    }

    /// Load configuration from environment variables.
    ///
    /// Recognized variables:
    /// - IMMICH_API_KEY - API key for Immich
    /// - IMMICH_BASE_URL - Base URL of Immich instance
    /// - OUTPUT_DIR - Output directory for processed images/video
    /// - RESIZE_SIZE - Output image size in pixels
    /// - FACE_RESOLUTION_THRESHOLD - Minimum face size in pixels
    /// - MAX_WORKERS - Number of parallel processing workers
    /// - VIDEO_FRAMERATE - Output video framerate
    /// - VIDEO_ENABLED - Whether to compile video (true/false)
    /// - KEEP_INTERMEDIATES - Keep debug images (true/false)
    pub fn from_env() -> Self {
        Config::default().with_env()
    }

    /// Merge environment variables into an existing config.
    pub fn with_env(mut self) -> Self {
        // API settings
        if let Ok(key) = std::env::var("IMMICH_API_KEY") {
            self.api.api_key = key;
        }
        if let Ok(url) = std::env::var("IMMICH_BASE_URL") {
            self.api.base_url = url;
        }

        // Output directory
        if let Ok(dir) = std::env::var("OUTPUT_DIR") {
            self.output_dir = PathBuf::from(dir);
        }

        // Processing settings
        if let Ok(val) = std::env::var("RESIZE_SIZE") {
            if let Ok(size) = val.parse() {
                self.processing.output.size = size;
            }
        }
        if let Ok(val) = std::env::var("FACE_RESOLUTION_THRESHOLD") {
            if let Ok(threshold) = val.parse() {
                self.processing.face_resolution.min_size = threshold;
            }
        }
        if let Ok(val) = std::env::var("MAX_WORKERS") {
            if let Ok(workers) = val.parse() {
                self.processing.max_workers = workers;
            }
        }
        if let Ok(val) = std::env::var("KEEP_INTERMEDIATES") {
            self.processing.output.keep_intermediates =
                val.eq_ignore_ascii_case("true") || val == "1";
        }

        // Video settings
        if let Ok(val) = std::env::var("VIDEO_FRAMERATE") {
            if let Ok(fps) = val.parse() {
                self.video.framerate = fps;
            }
        }
        if let Ok(val) = std::env::var("VIDEO_ENABLED") {
            self.video.enabled = val.eq_ignore_ascii_case("true") || val == "1";
        }
        if let Ok(val) = std::env::var("VIDEO_CRF") {
            if let Ok(crf) = val.parse() {
                self.video.crf = crf;
            }
        }

        self
    }

    /// Validate the entire configuration.
    pub fn validate(&self) -> Result<()> {
        // API validation
        if self.api.api_key.is_empty() {
            return Err(Error::Config("API key is required".to_string()));
        }
        if self.api.base_url.is_empty() {
            return Err(Error::Config("Base URL is required".to_string()));
        }

        // Delegate to sub-config validation
        self.processing.validate()?;
        self.video.validate()?;

        Ok(())
    }

    /// Save processing and video configuration to a TOML file.
    ///
    /// Only saves `processing` and `video` sections - API credentials are
    /// intentionally excluded as they should come from environment variables.
    pub fn save_to_file(&self, path: impl AsRef<std::path::Path>) -> Result<()> {
        let persistable = PersistableConfig::from(self);
        let content = toml::to_string_pretty(&persistable)
            .map_err(|e| Error::Config(format!("Failed to serialize config: {}", e)))?;
        std::fs::write(path, content)?;
        Ok(())
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = Config::default();
        assert_eq!(config.processing.output.size, 512);
        assert_eq!(config.processing.face_resolution.min_size, 80);
        assert_eq!(config.video.framerate, 15);
    }

    #[test]
    fn test_validation_missing_api_key() {
        let config = Config::default();
        let result = config.validate();
        assert!(result.is_err());
    }

    #[test]
    fn test_brightness_validation() {
        let mut config = BrightnessConfig {
            enabled: true,
            min_brightness: 0.1,
            max_brightness: 0.9,
        };

        // Valid config
        assert!(config.validate().is_ok());

        // Invalid: min >= max
        config.min_brightness = 0.9;
        config.max_brightness = 0.1;
        assert!(config.validate().is_err());

        // Invalid: out of range
        config.min_brightness = -0.1;
        config.max_brightness = 0.9;
        assert!(config.validate().is_err());

        // Disabled config skips validation
        config.enabled = false;
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_video_validation() {
        let mut config = VideoConfig::default();

        // Valid
        assert!(config.validate().is_ok());

        // Invalid CRF
        config.crf = 52;
        assert!(config.validate().is_err());

        // Invalid framerate
        config.crf = 23;
        config.framerate = 0;
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_output_validation() {
        let mut config = OutputConfig::default();

        // Valid
        assert!(config.validate().is_ok());

        // Too small
        config.size = 32;
        assert!(config.validate().is_err());

        // Too large
        config.size = 8192;
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_save_and_load_config() {
        // Create a config with custom values
        let mut config = Config::default();
        config.processing.output.size = 256;
        config.processing.face_resolution.min_size = 100;
        config.video.framerate = 30;
        config.video.crf = 18;

        // Save to a temp file
        let temp_dir = std::env::temp_dir();
        let temp_path = temp_dir.join("test_config_new.toml");

        config
            .save_to_file(&temp_path)
            .expect("Failed to save config");

        // Verify file was created and contains expected content
        let content = std::fs::read_to_string(&temp_path).expect("Failed to read config");
        assert!(content.contains("size = 256"));
        assert!(content.contains("min_size = 100"));
        assert!(content.contains("framerate = 30"));
        assert!(content.contains("crf = 18"));

        // Verify API credentials are NOT in the file
        assert!(!content.contains("api_key"));
        assert!(!content.contains("base_url"));

        // Clean up
        let _ = std::fs::remove_file(&temp_path);
    }
}
