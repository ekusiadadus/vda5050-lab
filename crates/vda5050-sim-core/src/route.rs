use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};

use crate::SimError;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Waypoint {
    pub id: String,
    pub x: f64,
    pub y: f64,
}

impl Waypoint {
    #[must_use]
    pub fn new(id: impl Into<String>, x: f64, y: f64) -> Self {
        Self {
            id: id.into(),
            x,
            y,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Route {
    waypoints: Vec<Waypoint>,
}

impl Route {
    /// Creates a route with at least one traversable edge.
    ///
    /// # Errors
    ///
    /// Returns an error for fewer than two waypoints, empty or duplicate IDs,
    /// or non-finite coordinates.
    pub fn new(waypoints: Vec<Waypoint>) -> Result<Self, SimError> {
        let ids = waypoints
            .iter()
            .map(|waypoint| waypoint.id.as_str())
            .collect::<BTreeSet<_>>();
        let valid = waypoints.len() >= 2
            && ids.len() == waypoints.len()
            && waypoints.iter().all(|waypoint| {
                !waypoint.id.trim().is_empty() && waypoint.x.is_finite() && waypoint.y.is_finite()
            });
        if !valid {
            return Err(SimError::InvalidRoute);
        }
        Ok(Self { waypoints })
    }

    #[must_use]
    pub fn waypoints(&self) -> &[Waypoint] {
        &self.waypoints
    }
}
