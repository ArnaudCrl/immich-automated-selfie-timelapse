//! Timestamp overlay step.
//!
//! Overlays date information on the processed image.

use crate::config::{Config, TextPosition};
use crate::pipeline::{draw_simple_text, PipelineContext, ProcessingStep, StepOutcome};
use async_trait::async_trait;
use image::{DynamicImage, Rgb};

/// Overlays timestamp on the image.
///
/// This step renders date information (year, month, day) on the final image
/// at the specified position.
pub struct TimestampStep;

impl TimestampStep {
    /// Parse a timestamp string and extract year, month, day.
    ///
    /// Supports formats like:
    /// - "2024-01-15" (ISO date)
    /// - "2024-01-15T12:34:56" (ISO datetime)
    /// - "2024-01-15T12:34:56.789Z" (ISO datetime with milliseconds)
    ///
    /// Returns (year, month, day) or None if parsing fails.
    fn parse_timestamp(timestamp: &str) -> Option<(String, String, String)> {
        // Try to extract date portion (before 'T' if present)
        let date_part = timestamp.split('T').next()?;

        // Split by '-' to get year, month, day
        let parts: Vec<&str> = date_part.split('-').collect();
        if parts.len() >= 3 {
            Some((
                parts[0].to_string(),
                parts[1].to_string(),
                parts[2].to_string(),
            ))
        } else {
            None
        }
    }

    /// Format the date string based on configuration.
    fn format_date(
        year: &str,
        month: &str,
        day: &str,
        config: &crate::config::TimestampConfig,
    ) -> String {
        let mut parts = Vec::new();

        if config.year {
            parts.push(year.to_string());
        }
        if config.month {
            parts.push(month.to_string());
        }
        if config.day {
            parts.push(day.to_string());
        }

        if parts.is_empty() {
            return String::new();
        }

        parts.join("-")
    }

    /// Calculate text position based on configuration.
    ///
    /// Returns (x, y) coordinates for the top-left corner of the text.
    fn calculate_position(
        position: TextPosition,
        text: &str,
        img_width: u32,
        img_height: u32,
    ) -> (u32, u32) {
        const CHAR_WIDTH: u32 = 6; // 5 pixels + 1 spacing
        const CHAR_HEIGHT: u32 = 7;
        const MARGIN: u32 = 8;

        let text_width = text.len() as u32 * CHAR_WIDTH;
        let text_height = CHAR_HEIGHT;

        match position {
            TextPosition::TopLeft => (MARGIN, MARGIN),
            TextPosition::TopRight => (img_width.saturating_sub(text_width + MARGIN), MARGIN),
            TextPosition::BottomLeft => (MARGIN, img_height.saturating_sub(text_height + MARGIN)),
            TextPosition::BottomRight => (
                img_width.saturating_sub(text_width + MARGIN),
                img_height.saturating_sub(text_height + MARGIN),
            ),
        }
    }
}

#[async_trait]
impl ProcessingStep for TimestampStep {
    fn id(&self) -> &'static str {
        "timestamp"
    }

    fn name(&self) -> &'static str {
        "Timestamp Overlay"
    }

    async fn execute(&self, mut ctx: PipelineContext, config: &Config) -> StepOutcome {
        let step_config = &config.processing.timestamp;

        let image = match ctx.take_image("timestamp overlay") {
            Ok(img) => img,
            Err(e) => return StepOutcome::Error { ctx, error: e },
        };

        // Parse the timestamp
        let (year, month, day) = match Self::parse_timestamp(&ctx.timestamp) {
            Some((y, m, d)) => (y, m, d),
            None => {
                // Failed to parse timestamp - continue without overlay
                ctx.image = Some(image);
                return StepOutcome::Continue(ctx);
            }
        };

        // Format the date string
        let date_str = Self::format_date(&year, &month, &day, step_config);

        // If no components are enabled, skip overlay
        if date_str.is_empty() {
            ctx.image = Some(image);
            return StepOutcome::Continue(ctx);
        }

        // Convert to RGB8 for drawing
        let mut rgb = image.to_rgb8();
        let (img_width, img_height) = (rgb.width(), rgb.height());

        // Calculate text position
        let (x, y) =
            Self::calculate_position(step_config.position, &date_str, img_width, img_height);

        // Draw the text (white with black shadow for visibility)
        let black = Rgb([0, 0, 0]);
        let white = Rgb([255, 255, 255]);

        // Draw shadow (offset by 1 pixel)
        draw_simple_text(&mut rgb, x + 1, y + 1, &date_str, black);
        // Draw text
        draw_simple_text(&mut rgb, x, y, &date_str, white);

        // Update context with modified image
        ctx.image = Some(DynamicImage::ImageRgb8(rgb));

        StepOutcome::Continue(ctx)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{Config, TextPosition, TimestampConfig};
    use crate::immich_api::FaceData;
    use image::{DynamicImage, RgbImage};

    fn make_ctx_with_image(timestamp: &str) -> PipelineContext {
        let face_data = FaceData {
            bounding_box_x1: 0.0,
            bounding_box_y1: 0.0,
            bounding_box_x2: 10.0,
            bounding_box_y2: 10.0,
            image_width: 10,
            image_height: 10,
        };
        let img = RgbImage::new(512, 512);
        PipelineContext::new("test".to_string(), timestamp.to_string(), face_data)
            .with_image(DynamicImage::ImageRgb8(img))
    }

    #[test]
    fn test_parse_timestamp_iso_date() {
        let result = TimestampStep::parse_timestamp("2024-01-15");
        assert_eq!(
            result,
            Some(("2024".to_string(), "01".to_string(), "15".to_string()))
        );
    }

    #[test]
    fn test_parse_timestamp_iso_datetime() {
        let result = TimestampStep::parse_timestamp("2024-01-15T12:34:56");
        assert_eq!(
            result,
            Some(("2024".to_string(), "01".to_string(), "15".to_string()))
        );
    }

    #[test]
    fn test_parse_timestamp_iso_datetime_with_millis() {
        let result = TimestampStep::parse_timestamp("2024-01-15T12:34:56.789Z");
        assert_eq!(
            result,
            Some(("2024".to_string(), "01".to_string(), "15".to_string()))
        );
    }

    #[test]
    fn test_parse_timestamp_invalid() {
        let result = TimestampStep::parse_timestamp("invalid");
        assert_eq!(result, None);
    }

    #[test]
    fn test_format_date_all() {
        let config = TimestampConfig {
            enabled: true,
            position: TextPosition::TopLeft,
            year: true,
            month: true,
            day: true,
        };
        let result = TimestampStep::format_date("2024", "01", "15", &config);
        assert_eq!(result, "2024-01-15");
    }

    #[test]
    fn test_format_date_year_month() {
        let config = TimestampConfig {
            enabled: true,
            position: TextPosition::TopLeft,
            year: true,
            month: true,
            day: false,
        };
        let result = TimestampStep::format_date("2024", "01", "15", &config);
        assert_eq!(result, "2024-01");
    }

    #[test]
    fn test_format_date_month_day() {
        let config = TimestampConfig {
            enabled: true,
            position: TextPosition::TopLeft,
            year: false,
            month: true,
            day: true,
        };
        let result = TimestampStep::format_date("2024", "01", "15", &config);
        assert_eq!(result, "01-15");
    }

    #[test]
    fn test_format_date_none() {
        let config = TimestampConfig {
            enabled: true,
            position: TextPosition::TopLeft,
            year: false,
            month: false,
            day: false,
        };
        let result = TimestampStep::format_date("2024", "01", "15", &config);
        assert_eq!(result, "");
    }

    #[tokio::test]
    async fn test_timestamp_overlay() {
        let step = TimestampStep;
        let ctx = make_ctx_with_image("2024-01-15");

        let mut config = Config::default();
        config.processing.timestamp.enabled = true;
        config.processing.timestamp.year = true;
        config.processing.timestamp.month = true;
        config.processing.timestamp.day = true;

        match step.execute(ctx, &config).await {
            StepOutcome::Continue(ctx) => {
                assert!(ctx.image.is_some());
            }
            _ => panic!("Expected Continue"),
        }
    }

    #[tokio::test]
    async fn test_timestamp_overlay_no_components() {
        let step = TimestampStep;
        let ctx = make_ctx_with_image("2024-01-15");

        let mut config = Config::default();
        config.processing.timestamp.enabled = true;
        config.processing.timestamp.year = false;
        config.processing.timestamp.month = false;
        config.processing.timestamp.day = false;

        match step.execute(ctx, &config).await {
            StepOutcome::Continue(ctx) => {
                // Should still have image, just no overlay
                assert!(ctx.image.is_some());
            }
            _ => panic!("Expected Continue"),
        }
    }

    #[test]
    fn test_calculate_position_top_left() {
        let (x, y) =
            TimestampStep::calculate_position(TextPosition::TopLeft, "2024-01-15", 512, 512);
        assert_eq!(x, 8);
        assert_eq!(y, 8);
    }

    #[test]
    fn test_calculate_position_bottom_right() {
        let (x, y) =
            TimestampStep::calculate_position(TextPosition::BottomRight, "2024-01-15", 512, 512);
        // Text is 10 chars * 6px = 60px wide, 7px tall
        // x = 512 - 60 - 8 = 444
        // y = 512 - 7 - 8 = 497
        assert_eq!(x, 444);
        assert_eq!(y, 497);
    }
}
