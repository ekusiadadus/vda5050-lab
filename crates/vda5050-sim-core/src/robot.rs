use serde::{Deserialize, Serialize};

use crate::{
    MotionLimits, Obstacle, Pose2, RobotGeometry, Route, SimError, Twist2,
    geometry::normalize_angle, motion::approach, positive,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum RobotMode {
    Turning,
    Driving,
    Arrived,
    Blocked,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RobotSnapshot {
    pub robot_id: String,
    pub tick: u64,
    pub pose: Pose2,
    pub twist: Twist2,
    pub mode: RobotMode,
    pub target_node_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StepResult {
    pub snapshot: RobotSnapshot,
    pub reached_nodes: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct Robot {
    id: String,
    geometry: RobotGeometry,
    limits: MotionLimits,
    route: Route,
    target_index: usize,
    snapshot: RobotSnapshot,
}

impl Robot {
    /// Creates a robot positioned on the first route node.
    ///
    /// # Errors
    ///
    /// Returns an error for an empty identity or when the initial pose does not
    /// coincide with the first waypoint within the configured tolerance.
    pub fn new(
        robot_id: impl Into<String>,
        initial_pose: Pose2,
        geometry: RobotGeometry,
        limits: MotionLimits,
        route: Route,
    ) -> Result<Self, SimError> {
        let robot_id = robot_id.into();
        if robot_id.trim().is_empty() {
            return Err(SimError::EmptyRobotId);
        }
        let start = &route.waypoints()[0];
        if initial_pose.distance_to(start.x, start.y) > limits.position_tolerance {
            return Err(SimError::InitialPose);
        }
        let snapshot = RobotSnapshot {
            robot_id: robot_id.clone(),
            tick: 0,
            pose: initial_pose,
            twist: Twist2::STOPPED,
            mode: RobotMode::Turning,
            target_node_id: Some(route.waypoints()[1].id.clone()),
        };
        Ok(Self {
            id: robot_id,
            geometry,
            limits,
            route,
            target_index: 1,
            snapshot,
        })
    }

    #[must_use]
    pub const fn mode(&self) -> RobotMode {
        self.snapshot.mode
    }

    #[must_use]
    pub const fn snapshot(&self) -> &RobotSnapshot {
        &self.snapshot
    }

    /// Advances the robot by one fixed simulation step.
    ///
    /// # Errors
    ///
    /// Returns an error if `dt_seconds` is not finite and positive.
    pub fn step(
        &mut self,
        dt_seconds: f64,
        obstacles: &[Obstacle],
    ) -> Result<StepResult, SimError> {
        let dt = positive(dt_seconds, "simulation step")?;
        self.snapshot.tick = self.snapshot.tick.saturating_add(1);
        if matches!(self.snapshot.mode, RobotMode::Arrived | RobotMode::Blocked) {
            return Ok(self.result(Vec::new()));
        }

        let target = &self.route.waypoints()[self.target_index];
        let delta_x = target.x - self.snapshot.pose.x;
        let delta_y = target.y - self.snapshot.pose.y;
        let distance = delta_x.hypot(delta_y);
        if distance <= self.limits.position_tolerance {
            return Ok(self.reach_target());
        }

        let desired_heading = delta_y.atan2(delta_x);
        let heading_error = normalize_angle(desired_heading - self.snapshot.pose.theta);
        if heading_error.abs() > self.limits.heading_tolerance {
            self.turn_towards(desired_heading, heading_error, dt);
            return Ok(self.result(Vec::new()));
        }

        self.snapshot.pose.theta = desired_heading;
        self.snapshot.twist.angular = approach(
            self.snapshot.twist.angular,
            0.0,
            self.limits.max_angular_acceleration * dt,
        );
        let stopping_speed = (2.0 * self.limits.max_linear_acceleration * distance).sqrt();
        let target_speed = stopping_speed.min(self.limits.max_linear_speed);
        self.snapshot.twist.linear = approach(
            self.snapshot.twist.linear,
            target_speed,
            self.limits.max_linear_acceleration * dt,
        );
        let travel = (self.snapshot.twist.linear * dt).min(distance);
        let candidate_x = self.snapshot.pose.x + travel * desired_heading.cos();
        let candidate_y = self.snapshot.pose.y + travel * desired_heading.sin();
        let radius = self.geometry.collision_radius();
        if obstacles
            .iter()
            .any(|obstacle| obstacle.intersects_circle(candidate_x, candidate_y, radius))
        {
            self.snapshot.mode = RobotMode::Blocked;
            self.snapshot.twist = Twist2::STOPPED;
            return Ok(self.result(Vec::new()));
        }

        self.snapshot.mode = RobotMode::Driving;
        self.snapshot.pose.x = candidate_x;
        self.snapshot.pose.y = candidate_y;
        if travel >= distance
            || self.snapshot.pose.distance_to(target.x, target.y) <= self.limits.position_tolerance
        {
            Ok(self.reach_target())
        } else {
            Ok(self.result(Vec::new()))
        }
    }

    fn turn_towards(&mut self, desired_heading: f64, heading_error: f64, dt: f64) {
        self.snapshot.mode = RobotMode::Turning;
        self.snapshot.twist.linear = approach(
            self.snapshot.twist.linear,
            0.0,
            self.limits.max_linear_acceleration * dt,
        );
        let target_angular = heading_error.signum() * self.limits.max_angular_speed;
        self.snapshot.twist.angular = approach(
            self.snapshot.twist.angular,
            target_angular,
            self.limits.max_angular_acceleration * dt,
        );
        let rotation = self.snapshot.twist.angular * dt;
        if rotation.abs() >= heading_error.abs() {
            self.snapshot.pose.theta = desired_heading;
            self.snapshot.twist.angular = 0.0;
        } else {
            self.snapshot.pose.theta = normalize_angle(self.snapshot.pose.theta + rotation);
        }
    }

    fn reach_target(&mut self) -> StepResult {
        let reached = self.route.waypoints()[self.target_index].clone();
        self.snapshot.pose.x = reached.x;
        self.snapshot.pose.y = reached.y;
        self.snapshot.twist = Twist2::STOPPED;
        self.target_index += 1;
        if self.target_index == self.route.waypoints().len() {
            self.snapshot.mode = RobotMode::Arrived;
            self.snapshot.target_node_id = None;
        } else {
            self.snapshot.mode = RobotMode::Turning;
            self.snapshot.target_node_id =
                Some(self.route.waypoints()[self.target_index].id.clone());
        }
        self.result(vec![reached.id])
    }

    fn result(&self, reached_nodes: Vec<String>) -> StepResult {
        StepResult {
            snapshot: self.snapshot.clone(),
            reached_nodes,
        }
    }

    #[must_use]
    pub fn robot_id(&self) -> &str {
        &self.id
    }
}
