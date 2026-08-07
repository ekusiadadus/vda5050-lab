use serde::{Deserialize, Serialize};

use crate::{SimError, positive};

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Twist2 {
    pub linear: f64,
    pub angular: f64,
}

impl Twist2 {
    pub(crate) const STOPPED: Self = Self {
        linear: 0.0,
        angular: 0.0,
    };
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct MotionLimits {
    pub max_linear_speed: f64,
    pub max_angular_speed: f64,
    pub max_linear_acceleration: f64,
    pub max_angular_acceleration: f64,
    pub position_tolerance: f64,
    pub heading_tolerance: f64,
}

impl MotionLimits {
    /// Creates positive finite motion limits.
    ///
    /// # Errors
    ///
    /// Returns an error for a non-finite or non-positive value.
    pub fn new(
        max_linear_speed: f64,
        max_angular_speed: f64,
        max_linear_acceleration: f64,
        max_angular_acceleration: f64,
        position_tolerance: f64,
        heading_tolerance: f64,
    ) -> Result<Self, SimError> {
        Ok(Self {
            max_linear_speed: positive(max_linear_speed, "max linear speed")?,
            max_angular_speed: positive(max_angular_speed, "max angular speed")?,
            max_linear_acceleration: positive(max_linear_acceleration, "max linear acceleration")?,
            max_angular_acceleration: positive(
                max_angular_acceleration,
                "max angular acceleration",
            )?,
            position_tolerance: positive(position_tolerance, "position tolerance")?,
            heading_tolerance: positive(heading_tolerance, "heading tolerance")?,
        })
    }
}

pub(crate) fn approach(current: f64, target: f64, maximum_delta: f64) -> f64 {
    if current < target {
        (current + maximum_delta).min(target)
    } else {
        (current - maximum_delta).max(target)
    }
}
