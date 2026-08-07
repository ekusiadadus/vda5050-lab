use std::collections::{BTreeMap, btree_map::Entry};

use crate::{Obstacle, Robot, SimError, StepResult};

#[derive(Debug, Default)]
pub struct World {
    robots: BTreeMap<String, Robot>,
    obstacles: Vec<Obstacle>,
}

impl World {
    #[must_use]
    pub const fn new() -> Self {
        Self {
            robots: BTreeMap::new(),
            obstacles: Vec::new(),
        }
    }

    /// Adds a uniquely identified robot.
    ///
    /// # Errors
    ///
    /// Returns an error when a robot with the same identity already exists.
    pub fn add_robot(&mut self, robot: Robot) -> Result<(), SimError> {
        let id = robot.robot_id().to_owned();
        match self.robots.entry(id) {
            Entry::Vacant(entry) => {
                entry.insert(robot);
                Ok(())
            }
            Entry::Occupied(entry) => Err(SimError::DuplicateRobot(entry.key().clone())),
        }
    }

    pub fn add_obstacle(&mut self, obstacle: Obstacle) {
        self.obstacles.push(obstacle);
    }

    /// Advances every robot in stable identity order.
    ///
    /// # Errors
    ///
    /// Returns an error if the fixed step is invalid.
    pub fn step(&mut self, dt_seconds: f64) -> Result<Vec<StepResult>, SimError> {
        self.robots
            .values_mut()
            .map(|robot| robot.step(dt_seconds, &self.obstacles))
            .collect()
    }
}
