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

/// Blur detection configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlurConfig {
    /// Whether blur detection is enabled.
    pub enabled: bool,

    /// Minimum gradient magnitude threshold.
    /// Images with gradient magnitude below this are considered blurry.
    /// Uses Sobel operator for robust edge detection.
    /// Typical values: 10-20 for strict filtering, 5-10 for moderate filtering.
    pub min_sharpness: f32,
}

impl Default for BlurConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            min_sharpness: 15.0,
        }
    }
}

impl BlurConfig {
    /// Validate the configuration values.
    pub fn validate(&self) -> Result<()> {
        if self.enabled && self.min_sharpness < 0.0 {
            return Err(Error::Config(
                "Blur min_sharpness must be non-negative".to_string(),
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
/// Uses a single `eye_distance` parameter to control framing. Eye positions
/// are derived by estimating the visual face center (approximately at the nose
/// tip) from standard face proportions, then placing that point at the image
/// center with a small downward offset to show more neck.
///
/// Note: Face alignment is always enabled and is a core part of the pipeline.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AlignmentConfig {
    /// Distance between left and right eye centers as a fraction of output
    /// image width (0.0-1.0). Default 0.3 means the eyes span 30% of the
    /// output width. Larger values zoom in (bigger face), smaller values
    /// zoom out (more background).
    pub eye_distance: f32,
}

impl Default for AlignmentConfig {
    fn default() -> Self {
        Self { eye_distance: 0.3 }
    }
}

impl AlignmentConfig {
    /// Offset from the eye line to the approximate visual face center
    /// (with a slight neck bias), expressed as a multiple of the inter-eye
    /// distance. The visible face center (hairline-to-chin midpoint) is
    /// about 0.4× IPD below the eyes; adding a small neck offset brings
    /// this to 0.5× IPD. This matches the standard used by dlib and OpenCV
    /// face alignment (desiredLeftEye = (0.35, 0.35) with IPD = 0.3).
    /// This point is placed at the image center (0.5, 0.5).
    const FACE_CENTER_BELOW_EYES: f32 = 0.5;

    /// Validate the configuration values.
    pub fn validate(&self) -> Result<()> {
        if self.eye_distance <= 0.0 || self.eye_distance >= 1.0 {
            return Err(Error::Config(
                "Alignment eye_distance must be between 0.0 and 1.0 (exclusive)".to_string(),
            ));
        }
        if self.eye_distance < 0.05 || self.eye_distance > 0.4 {
            return Err(Error::Config(
                "Alignment eye_distance should be between 0.05 and 0.4 for best results".to_string(),
            ));
        }
        Ok(())
    }

    /// Target X position for the left eye as a fraction of output width.
    /// Eyes are centered horizontally: `left_eye_x = (1 - eye_distance) / 2`.
    pub fn left_eye_x(&self) -> f32 {
        (1.0 - self.eye_distance) / 2.0
    }

    /// Target Y position for both eyes as a fraction of output height.
    /// Derived by placing the estimated face center at the image center:
    /// `eye_y = 0.5 - eye_distance * FACE_CENTER_BELOW_EYES`.
    pub fn left_eye_y(&self) -> f32 {
        0.5 - self.eye_distance * Self::FACE_CENTER_BELOW_EYES
    }
}

/// Position for timestamp text overlay.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum TextPosition {
    #[default]
    TopLeft,
    TopRight,
    BottomLeft,
    BottomRight,
}

/// Timestamp overlay configuration.
///
/// Overlays date information on the processed image.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimestampConfig {
    /// Whether timestamp overlay is enabled.
    pub enabled: bool,

    /// Position of the timestamp text.
    pub position: TextPosition,

    /// Whether to display the year.
    pub year: bool,

    /// Whether to display the month.
    pub month: bool,

    /// Whether to display the day.
    pub day: bool,
}

impl Default for TimestampConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            position: TextPosition::BottomLeft,
            year: true,
            month: false,
            day: false,
        }
    }
}

impl TimestampConfig {
    /// Validate the configuration values.
    pub fn validate(&self) -> Result<()> {
        // No validation needed for booleans and enum
        Ok(())
    }
}

/// Time range granularity for photo limiting.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum TimeRange {
    #[default]
    Day,
    Week,
    Month,
}

/// Time interval configuration.
///
/// Limits the number of successfully processed photos per time range
/// (day/week/month) to avoid over-representation of busy periods.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimeIntervalConfig {
    /// Whether photo limiting is enabled.
    pub enabled: bool,

    /// Maximum number of photos per time range.
    pub max_photos: u32,

    /// Time range granularity.
    pub time_range: TimeRange,
}

impl Default for TimeIntervalConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            max_photos: 1,
            time_range: TimeRange::Day,
        }
    }
}

impl TimeIntervalConfig {
    /// Validate the configuration values.
    pub fn validate(&self) -> Result<()> {
        if self.enabled && self.max_photos == 0 {
            return Err(Error::Config(
                "Time interval max_photos must be greater than 0".to_string(),
            ));
        }
        if self.enabled && self.max_photos > 100 {
            return Err(Error::Config(
                "Time interval max_photos must be at most 100".to_string(),
            ));
        }
        Ok(())
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

    /// Blur detection settings.
    #[serde(default)]
    pub blur: BlurConfig,

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

    /// Timestamp overlay settings.
    #[serde(default)]
    pub timestamp: TimestampConfig,

    /// Time interval settings.
    #[serde(default)]
    pub time_interval: TimeIntervalConfig,
}

impl Default for ProcessingConfig {
    fn default() -> Self {
        Self {
            max_workers: num_cpus(),
            face_resolution: FaceResolutionConfig::default(),
            blur: BlurConfig::default(),
            brightness: BrightnessConfig::default(),
            head_pose: HeadPoseConfig::default(),
            eye_filter: EyeFilterConfig::default(),
            output: OutputConfig::default(),
            alignment: AlignmentConfig::default(),
            timestamp: TimestampConfig::default(),
            time_interval: TimeIntervalConfig::default(),
        }
    }
}

impl ProcessingConfig {
    /// Validate all step configurations.
    pub fn validate(&self) -> Result<()> {
        self.face_resolution.validate()?;
        self.blur.validate()?;
        self.brightness.validate()?;
        self.head_pose.validate()?;
        self.eye_filter.validate()?;
        self.output.validate()?;
        self.alignment.validate()?;
        self.timestamp.validate()?;
        self.time_interval.validate()?;
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
