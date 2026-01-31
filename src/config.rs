//! Application configuration.
//!
//! Configuration can be loaded from:
//! 1. TOML file (config.toml)
//! 2. Environment variables (prefixed with IMMICH_)
//! 3. Programmatic overrides

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

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

/// Face processing parameters.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct ProcessingConfig {
    /// Output image size (width and height in pixels).
    pub resize_size: u32,

    /// Minimum face width/height in pixels.
    pub face_resolution_threshold: u32,

    /// Maximum allowed head yaw angle in degrees.
    pub pose_threshold: f32,

    /// Eye Aspect Ratio threshold for eye visibility.
    pub ear_threshold: f32,

    /// Target position for left eye as (x%, y%).
    pub left_eye_pos: (f32, f32),

    /// Target position for right eye as (x%, y%).
    pub right_eye_pos: (f32, f32),

    /// Number of parallel workers for processing.
    pub max_workers: usize,

    /// Keep intermediate images (original, cropped) for inspection.
    pub keep_intermediates: bool,
}

impl Default for ProcessingConfig {
    fn default() -> Self {
        Self {
            resize_size: 512,
            face_resolution_threshold: 80,
            pose_threshold: 25.0,
            ear_threshold: 0.2,
            left_eye_pos: (0.35, 0.4),
            right_eye_pos: (0.65, 0.4),
            max_workers: num_cpus(),
            keep_intermediates: false,
        }
    }
}

fn num_cpus() -> usize {
    std::thread::available_parallelism()
        .map(|p| p.get())
        .unwrap_or(4)
}

/// Video compilation settings.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct VideoConfig {
    /// Output video framerate.
    pub framerate: u32,

    /// Whether to compile video after processing.
    pub enabled: bool,

    /// Video codec (e.g., "libx264").
    pub codec: String,

    /// Constant Rate Factor for quality (lower = better, 18-28 recommended).
    pub crf: u32,
}

impl Default for VideoConfig {
    fn default() -> Self {
        Self {
            framerate: 15,
            enabled: true,
            codec: "libx264".to_string(),
            crf: 23,
        }
    }
}

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

impl Config {
    /// Load configuration from a TOML file.
    pub fn from_file(path: impl AsRef<std::path::Path>) -> crate::error::Result<Self> {
        let content = std::fs::read_to_string(path)?;
        let config: Config =
            toml::from_str(&content).map_err(|e| crate::error::Error::Config(e.to_string()))?;
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
                self.processing.resize_size = size;
            }
        }
        if let Ok(val) = std::env::var("FACE_RESOLUTION_THRESHOLD") {
            if let Ok(threshold) = val.parse() {
                self.processing.face_resolution_threshold = threshold;
            }
        }
        if let Ok(val) = std::env::var("MAX_WORKERS") {
            if let Ok(workers) = val.parse() {
                self.processing.max_workers = workers;
            }
        }
        if let Ok(val) = std::env::var("KEEP_INTERMEDIATES") {
            self.processing.keep_intermediates = val.eq_ignore_ascii_case("true") || val == "1";
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

    /// Validate the configuration.
    pub fn validate(&self) -> crate::error::Result<()> {
        if self.api.api_key.is_empty() {
            return Err(crate::error::Error::Config(
                "API key is required".to_string(),
            ));
        }
        if self.api.base_url.is_empty() {
            return Err(crate::error::Error::Config(
                "Base URL is required".to_string(),
            ));
        }
        if self.processing.resize_size == 0 {
            return Err(crate::error::Error::Config(
                "Resize size must be greater than 0".to_string(),
            ));
        }
        Ok(())
    }

    /// Save processing and video configuration to a TOML file.
    ///
    /// Only saves `processing` and `video` sections - API credentials are
    /// intentionally excluded as they should come from environment variables.
    pub fn save_to_file(&self, path: impl AsRef<std::path::Path>) -> crate::error::Result<()> {
        let persistable = PersistableConfig::from(self);
        let content = toml::to_string_pretty(&persistable)
            .map_err(|e| crate::error::Error::Config(format!("Failed to serialize config: {}", e)))?;
        std::fs::write(path, content)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = Config::default();
        assert_eq!(config.processing.resize_size, 512);
        assert_eq!(config.processing.face_resolution_threshold, 80);
        assert_eq!(config.video.framerate, 15);
    }

    #[test]
    fn test_validation_missing_api_key() {
        let config = Config::default();
        let result = config.validate();
        assert!(result.is_err());
    }

    #[test]
    fn test_save_and_load_config() {
        // Create a config with custom values
        let mut config = Config::default();
        config.processing.resize_size = 256;
        config.processing.face_resolution_threshold = 100;
        config.video.framerate = 30;
        config.video.crf = 18;

        // Save to a temp file
        let temp_dir = std::env::temp_dir();
        let temp_path = temp_dir.join("test_config.toml");

        config.save_to_file(&temp_path).expect("Failed to save config");

        // Verify file was created and contains expected content
        let content = std::fs::read_to_string(&temp_path).expect("Failed to read config");
        assert!(content.contains("resize_size = 256"));
        assert!(content.contains("face_resolution_threshold = 100"));
        assert!(content.contains("framerate = 30"));
        assert!(content.contains("crf = 18"));

        // Verify API credentials are NOT in the file
        assert!(!content.contains("api_key"));
        assert!(!content.contains("base_url"));

        // Clean up
        let _ = std::fs::remove_file(&temp_path);
    }
}
