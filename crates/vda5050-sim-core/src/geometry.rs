use serde::{Deserialize, Serialize};

use crate::{SimError, finite, positive};

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Pose2 {
    pub x: f64,
    pub y: f64,
    pub theta: f64,
}

impl Pose2 {
    /// Creates a finite planar pose.
    ///
    /// # Errors
    ///
    /// Returns an error if any component is NaN or infinite.
    pub fn new(x: f64, y: f64, theta: f64) -> Result<Self, SimError> {
        Ok(Self {
            x: finite(x, "pose.x")?,
            y: finite(y, "pose.y")?,
            theta: normalize_angle(finite(theta, "pose.theta")?),
        })
    }

    #[must_use]
    pub fn distance_to(self, x: f64, y: f64) -> f64 {
        (x - self.x).hypot(y - self.y)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct RobotGeometry {
    pub length: f64,
    pub width: f64,
}

impl RobotGeometry {
    /// Creates a positive rectangular footprint.
    ///
    /// # Errors
    ///
    /// Returns an error for non-finite or non-positive dimensions.
    pub fn new(length: f64, width: f64) -> Result<Self, SimError> {
        Ok(Self {
            length: positive(length, "geometry.length")?,
            width: positive(width, "geometry.width")?,
        })
    }

    pub(crate) fn collision_radius(self) -> f64 {
        0.5 * self.length.hypot(self.width)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Obstacle {
    min_x: f64,
    min_y: f64,
    max_x: f64,
    max_y: f64,
}

impl Obstacle {
    /// Creates an axis-aligned obstacle.
    ///
    /// # Errors
    ///
    /// Returns an error unless both extents are finite and strictly ordered.
    pub fn new(min_x: f64, min_y: f64, max_x: f64, max_y: f64) -> Result<Self, SimError> {
        let min_x = finite(min_x, "obstacle.min_x")?;
        let min_y = finite(min_y, "obstacle.min_y")?;
        let max_x = finite(max_x, "obstacle.max_x")?;
        let max_y = finite(max_y, "obstacle.max_y")?;
        if min_x >= max_x || min_y >= max_y {
            return Err(SimError::NonPositive("obstacle extent"));
        }
        Ok(Self {
            min_x,
            min_y,
            max_x,
            max_y,
        })
    }

    #[must_use]
    pub const fn min_x(self) -> f64 {
        self.min_x
    }

    pub(crate) fn intersects_circle(self, x: f64, y: f64, radius: f64) -> bool {
        let closest_x = x.clamp(self.min_x, self.max_x);
        let closest_y = y.clamp(self.min_y, self.max_y);
        (x - closest_x).hypot(y - closest_y) <= radius
    }
}

pub(crate) fn normalize_angle(angle: f64) -> f64 {
    let two_pi = 2.0 * std::f64::consts::PI;
    (angle + std::f64::consts::PI).rem_euclid(two_pi) - std::f64::consts::PI
}
