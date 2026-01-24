//! Types for image processing.

use serde::{Deserialize, Serialize};

/// A 2D point.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct Point {
    pub x: f32,
    pub y: f32,
}

impl Point {
    pub fn new(x: f32, y: f32) -> Self {
        Self { x, y }
    }
}

/// Bounding box for a face.
#[derive(Debug, Clone, Copy)]
pub struct BoundingBox {
    pub x1: f32,
    pub y1: f32,
    pub x2: f32,
    pub y2: f32,
}

impl BoundingBox {
    pub fn width(&self) -> f32 {
        self.x2 - self.x1
    }

    pub fn height(&self) -> f32 {
        self.y2 - self.y1
    }

    pub fn center(&self) -> Point {
        Point::new((self.x1 + self.x2) / 2.0, (self.y1 + self.y2) / 2.0)
    }
}

/// Facial landmarks (68-point model).
#[derive(Debug, Clone)]
pub struct Landmarks {
    /// All 68 landmark points.
    pub points: Vec<Point>,
}

impl Landmarks {
    /// Left eye center (average of points 36-41).
    pub fn left_eye_center(&self) -> Point {
        let eye_points = &self.points[36..42];
        let x = eye_points.iter().map(|p| p.x).sum::<f32>() / 6.0;
        let y = eye_points.iter().map(|p| p.y).sum::<f32>() / 6.0;
        Point::new(x, y)
    }

    /// Right eye center (average of points 42-47).
    pub fn right_eye_center(&self) -> Point {
        let eye_points = &self.points[42..48];
        let x = eye_points.iter().map(|p| p.x).sum::<f32>() / 6.0;
        let y = eye_points.iter().map(|p| p.y).sum::<f32>() / 6.0;
        Point::new(x, y)
    }

    /// Nose tip (point 30).
    pub fn nose_tip(&self) -> Point {
        self.points[30]
    }

    /// Chin (point 8).
    pub fn chin(&self) -> Point {
        self.points[8]
    }

    /// Left mouth corner (point 48).
    pub fn left_mouth(&self) -> Point {
        self.points[48]
    }

    /// Right mouth corner (point 54).
    pub fn right_mouth(&self) -> Point {
        self.points[54]
    }
}

/// Head pose angles.
#[derive(Debug, Clone, Copy)]
pub struct HeadPose {
    /// Pitch (up/down tilt) in degrees.
    pub pitch: f32,
    /// Yaw (left/right turn) in degrees.
    pub yaw: f32,
    /// Roll (head tilt) in degrees.
    pub roll: f32,
}

impl HeadPose {
    /// Check if the pose is within acceptable thresholds.
    pub fn is_frontal(&self, yaw_threshold: f32) -> bool {
        self.yaw.abs() <= yaw_threshold
    }
}

/// Eye Aspect Ratio for blink detection.
#[derive(Debug, Clone, Copy)]
pub struct EyeAspectRatio {
    pub left: f32,
    pub right: f32,
}

impl EyeAspectRatio {
    /// Check if eyes are sufficiently open.
    pub fn eyes_open(&self, threshold: f32) -> bool {
        self.left >= threshold && self.right >= threshold
    }
}

/// Result of processing a single face.
#[derive(Debug)]
pub struct ProcessedFace {
    /// The aligned and cropped face image data.
    pub image_data: Vec<u8>,
    /// Original asset ID.
    pub asset_id: String,
    /// Timestamp for sorting/naming.
    pub timestamp: String,
}

/// Processing result for a single asset.
#[derive(Debug)]
pub enum AssetResult {
    /// Successfully processed.
    Success(ProcessedFace),
    /// Skipped (face too small, bad pose, etc.).
    Skipped { asset_id: String, reason: String },
    /// Error during processing.
    Error { asset_id: String, error: String },
}
