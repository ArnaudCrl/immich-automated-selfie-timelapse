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
    /// Create a new bounding box, ensuring x1 <= x2 and y1 <= y2.
    pub fn new(x1: f32, y1: f32, x2: f32, y2: f32) -> Self {
        Self {
            x1: x1.min(x2),
            y1: y1.min(y2),
            x2: x1.max(x2),
            y2: y1.max(y2),
        }
    }

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
///
/// This struct requires exactly 68 landmark points following the standard
/// dlib/iBUG 68-point face landmark format. Use `Landmarks::new()` to
/// construct with validation.
#[derive(Debug, Clone)]
pub struct Landmarks {
    /// All 68 landmark points (validated on construction).
    points: [Point; 68],
}

/// Expected number of landmark points.
pub const LANDMARK_COUNT: usize = 68;

impl Landmarks {
    /// Create a new Landmarks from a vector of points.
    ///
    /// Returns an error if the vector doesn't contain exactly 68 points.
    pub fn new(points: Vec<Point>) -> Result<Self, String> {
        let len = points.len();
        let points: [Point; LANDMARK_COUNT] = points.try_into().map_err(|_| {
            format!(
                "Expected exactly {} landmark points, got {}",
                LANDMARK_COUNT, len
            )
        })?;
        Ok(Self { points })
    }

    /// Get all landmark points.
    pub fn points(&self) -> &[Point; 68] {
        &self.points
    }

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

    /// Calculate Eye Aspect Ratio (EAR) for blink detection.
    ///
    /// EAR is computed as:
    /// EAR = (||p2-p6|| + ||p3-p5||) / (2 * ||p1-p4||)
    ///
    /// Where p1-p6 are the 6 eye landmark points.
    /// For left eye: points 36-41
    /// For right eye: points 42-47
    pub fn eye_aspect_ratio(&self) -> EyeAspectRatio {
        let left_ear = Self::compute_ear(&self.points[36..42]);
        let right_ear = Self::compute_ear(&self.points[42..48]);
        EyeAspectRatio {
            left: left_ear,
            right: right_ear,
        }
    }

    /// Compute EAR for a single eye given 6 landmark points.
    fn compute_ear(eye: &[Point]) -> f32 {
        if eye.len() != 6 {
            return 0.0;
        }

        // Vertical distances
        let v1 = Self::distance(&eye[1], &eye[5]); // p2-p6
        let v2 = Self::distance(&eye[2], &eye[4]); // p3-p5

        // Horizontal distance
        let h = Self::distance(&eye[0], &eye[3]); // p1-p4

        if h == 0.0 {
            return 0.0;
        }

        (v1 + v2) / (2.0 * h)
    }

    /// Euclidean distance between two points.
    fn distance(p1: &Point, p2: &Point) -> f32 {
        let dx = p2.x - p1.x;
        let dy = p2.y - p1.y;
        (dx * dx + dy * dy).sqrt()
    }

    /// Get the angle (in radians) to rotate the face so eyes are horizontal.
    pub fn eye_rotation_angle(&self) -> f32 {
        let left_eye = self.left_eye_center();
        let right_eye = self.right_eye_center();
        let dy = right_eye.y - left_eye.y;
        let dx = right_eye.x - left_eye.x;
        dy.atan2(dx)
    }

    /// Get the distance between eye centers.
    pub fn inter_eye_distance(&self) -> f32 {
        Self::distance(&self.left_eye_center(), &self.right_eye_center())
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
