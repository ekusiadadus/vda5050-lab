use thiserror::Error;
use vda5050_sim_core::{
    MotionLimits, Pose2, Robot, RobotGeometry, RobotMode, RobotSnapshot, Route, SimError, Waypoint,
};

use crate::Position;

const SIMULATION_DT_SECONDS: f64 = 0.05;
const MAX_SIMULATION_TICKS: usize = 400;

#[derive(Debug, Error)]
pub enum DemoSimulationError {
    #[error(transparent)]
    Simulation(#[from] SimError),
    #[error("the compatibility demo route must be a four-meter horizontal released edge")]
    CompatibilityRoute,
    #[error("simulation did not reach the released node within its fixed tick budget")]
    TickBudget,
}

/// Runs the compatibility route through the same fixed-step controller used by
/// the interactive simulator.
///
/// # Errors
///
/// Returns an error for an unsupported compatibility route or invalid motion
/// configuration, or if the fixed tick budget is exhausted.
pub fn simulate_robot_route(
    start: Position,
    released_end: Position,
) -> Result<Vec<RobotSnapshot>, DemoSimulationError> {
    if released_end.x - start.x != 4 || released_end.y != start.y {
        return Err(DemoSimulationError::CompatibilityRoute);
    }
    let route = Route::new(vec![
        Waypoint::new("released-start", f64::from(start.x), f64::from(start.y)),
        Waypoint::new(
            "released-end",
            f64::from(released_end.x),
            f64::from(released_end.y),
        ),
    ])?;
    let mut robot = Robot::new(
        "compatibility-demo",
        Pose2::new(f64::from(start.x), f64::from(start.y), 0.0)?,
        RobotGeometry::new(0.8, 0.5)?,
        MotionLimits::new(1.0, 1.5, 0.8, 2.0, 0.01, 0.01)?,
        route,
    )?;
    let mut snapshots = vec![robot.snapshot().clone()];
    for _ in 0..MAX_SIMULATION_TICKS {
        let step = robot.step(SIMULATION_DT_SECONDS, &[])?;
        snapshots.push(step.snapshot);
        if robot.mode() == RobotMode::Arrived {
            return Ok(snapshots);
        }
    }
    Err(DemoSimulationError::TickBudget)
}

/// Reduces continuous compatibility motion to the four historic wire samples.
///
/// # Errors
///
/// Returns an error unless the snapshots begin at an integral position and
/// cross each of the four one-meter observation boundaries.
pub fn sample_wire_positions(
    snapshots: &[RobotSnapshot],
) -> Result<Vec<Position>, DemoSimulationError> {
    let Some(first) = snapshots.first() else {
        return Err(DemoSimulationError::CompatibilityRoute);
    };
    let start_x = exact_i32(first.pose.x)?;
    let start_y = exact_i32(first.pose.y)?;
    let mut samples = Vec::with_capacity(4);
    for offset in 1..=4 {
        let threshold = f64::from(start_x + offset);
        if snapshots.iter().any(|snapshot| {
            snapshot.pose.x >= threshold && (snapshot.pose.y - f64::from(start_y)).abs() <= 0.01
        }) {
            samples.push(Position {
                x: start_x + offset,
                y: start_y,
            });
        } else {
            return Err(DemoSimulationError::CompatibilityRoute);
        }
    }
    Ok(samples)
}

fn exact_i32(value: f64) -> Result<i32, DemoSimulationError> {
    if !value.is_finite() || value.fract() != 0.0 {
        return Err(DemoSimulationError::CompatibilityRoute);
    }
    value
        .to_string()
        .parse::<i32>()
        .map_err(|_| DemoSimulationError::CompatibilityRoute)
}
