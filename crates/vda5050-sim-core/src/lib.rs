//! Deterministic, fixed-step mobile-robot kinematics for the isolated demo.

mod geometry;
mod motion;
mod robot;
mod route;
mod world;

pub use geometry::{Obstacle, Pose2, RobotGeometry};
pub use motion::{MotionLimits, Twist2};
pub use robot::{Robot, RobotMode, RobotSnapshot, StepResult};
pub use route::{Route, Waypoint};
pub use world::World;

use thiserror::Error;

#[derive(Debug, Error, Clone, PartialEq)]
pub enum SimError {
    #[error("{0} must be finite")]
    NonFinite(&'static str),
    #[error("{0} must be greater than zero")]
    NonPositive(&'static str),
    #[error("route must contain at least two finite, uniquely identified waypoints")]
    InvalidRoute,
    #[error("initial pose must coincide with the first route waypoint")]
    InitialPose,
    #[error("robot identity must not be empty")]
    EmptyRobotId,
    #[error("robot identity is already present: {0}")]
    DuplicateRobot(String),
}

pub(crate) fn finite(value: f64, name: &'static str) -> Result<f64, SimError> {
    if value.is_finite() {
        Ok(value)
    } else {
        Err(SimError::NonFinite(name))
    }
}

pub(crate) fn positive(value: f64, name: &'static str) -> Result<f64, SimError> {
    finite(value, name)?;
    if value > 0.0 {
        Ok(value)
    } else {
        Err(SimError::NonPositive(name))
    }
}
