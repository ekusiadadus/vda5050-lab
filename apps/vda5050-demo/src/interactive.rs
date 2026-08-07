use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use serde_json::Value;
use thiserror::Error;

pub const LIVE_MESSAGE_LIMIT: usize = 384;
const LIVE_SCHEMA: &str = "vda5050-lab.tier1-live-run/1";
const LIVE_EVENT_SCHEMA: &str = "vda5050-lab.live-event/1";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum LiveScenario {
    ReconnectMissingOnline,
    ReconnectControl,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct LivePoint {
    pub x: f64,
    pub y: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LiveRobotPlan {
    pub serial_number: String,
    pub topic_prefix: String,
    pub order_id: String,
    pub start: LivePoint,
    pub released_end: LivePoint,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct LiveHardLimits {
    pub messages: usize,
    pub duration_seconds: u64,
    pub actors: usize,
    pub mqtt_connections: usize,
    pub simulation_ticks: u64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
// These booleans are intentionally explicit, machine-checkable proof-boundary
// assertions in the serialized run manifest.
#[allow(clippy::struct_excessive_bools)]
pub struct LiveRunPlan {
    schema: String,
    run_id: String,
    scenario: LiveScenario,
    protocol_version: String,
    synthetic: bool,
    same_job_isolated: bool,
    connect_is_side_effect: bool,
    physical_dut_authorized: bool,
    broker_host: String,
    broker_port: u16,
    map_id: String,
    tick_hz: u64,
    visualization_hz: u64,
    state_hz: u64,
    fault_target: String,
    disconnect_x: f64,
    robots: Vec<LiveRobotPlan>,
    client_ids: BTreeMap<String, String>,
    topic_allowlist: Vec<String>,
    retain_allowlist: Vec<String>,
    control_topic: String,
    hard_limits: LiveHardLimits,
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum LivePlanError {
    #[error("run ID must contain only ASCII letters, digits, '-' or '_' and be 1..32 bytes")]
    RunId,
}

impl LiveRunPlan {
    /// Builds the fixed, two-robot interactive Tier 1 plan.
    ///
    /// # Errors
    ///
    /// Returns an error unless `run_id` is a bounded filesystem-safe token.
    pub fn new(run_id: &str, scenario: LiveScenario) -> Result<Self, LivePlanError> {
        if run_id.is_empty()
            || run_id.len() > 32
            || !run_id
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
        {
            return Err(LivePlanError::RunId);
        }
        let robots = [0.0, 2.0]
            .into_iter()
            .enumerate()
            .map(|(index, y)| {
                let serial_number = format!("demo-{:03}", index + 1);
                LiveRobotPlan {
                    topic_prefix: format!("vda5050/v3/lab-demo/{serial_number}"),
                    order_id: format!("live-reconnect-{:03}", index + 1),
                    serial_number,
                    start: LivePoint { x: 0.0, y },
                    released_end: LivePoint { x: 6.0, y },
                }
            })
            .collect::<Vec<_>>();
        let client_ids =
            std::iter::once(("fleet".to_owned(), format!("vda5050-live-fleet-{run_id}")))
                .chain(std::iter::once((
                    "recorder".to_owned(),
                    format!("vda5050-live-recorder-{run_id}"),
                )))
                .chain(robots.iter().map(|robot| {
                    (
                        format!("agv.{}", robot.serial_number),
                        format!("vda5050-live-agv-{}-{run_id}", robot.serial_number),
                    )
                }))
                .collect();
        let topic_allowlist = robots
            .iter()
            .flat_map(|robot| {
                ["connection", "order", "state", "visualization"]
                    .into_iter()
                    .map(move |suffix| format!("{}/{suffix}", robot.topic_prefix))
            })
            .collect();
        let retain_allowlist = robots
            .iter()
            .map(|robot| format!("{}/connection", robot.topic_prefix))
            .collect();
        Ok(Self {
            schema: LIVE_SCHEMA.to_owned(),
            run_id: run_id.to_owned(),
            scenario,
            protocol_version: "3.0.0".to_owned(),
            synthetic: true,
            same_job_isolated: true,
            connect_is_side_effect: true,
            physical_dut_authorized: false,
            broker_host: "broker".to_owned(),
            broker_port: 1883,
            map_id: "warehouse-live".to_owned(),
            tick_hz: 20,
            visualization_hz: 10,
            state_hz: 2,
            fault_target: "demo-001".to_owned(),
            disconnect_x: 2.5,
            robots,
            client_ids,
            topic_allowlist,
            retain_allowlist,
            control_topic: format!("vda5050-lab/live/{run_id}/complete"),
            hard_limits: LiveHardLimits {
                messages: LIVE_MESSAGE_LIMIT,
                duration_seconds: 60,
                actors: 4,
                mqtt_connections: 4,
                simulation_ticks: 240,
            },
        })
    }

    #[must_use]
    pub fn schema(&self) -> &str {
        &self.schema
    }

    #[must_use]
    pub fn run_id(&self) -> &str {
        &self.run_id
    }

    #[must_use]
    pub const fn scenario(&self) -> LiveScenario {
        self.scenario
    }

    #[must_use]
    pub fn protocol_version(&self) -> &str {
        &self.protocol_version
    }

    #[must_use]
    pub const fn synthetic(&self) -> bool {
        self.synthetic
    }

    #[must_use]
    pub const fn same_job_isolated(&self) -> bool {
        self.same_job_isolated
    }

    #[must_use]
    pub const fn physical_dut_authorized(&self) -> bool {
        self.physical_dut_authorized
    }

    #[must_use]
    pub const fn tick_hz(&self) -> u64 {
        self.tick_hz
    }

    #[must_use]
    pub const fn visualization_hz(&self) -> u64 {
        self.visualization_hz
    }

    #[must_use]
    pub const fn state_hz(&self) -> u64 {
        self.state_hz
    }

    #[must_use]
    pub fn fault_target(&self) -> &str {
        &self.fault_target
    }

    #[must_use]
    pub fn robots(&self) -> &[LiveRobotPlan] {
        &self.robots
    }

    #[must_use]
    pub fn client_ids(&self) -> &BTreeMap<String, String> {
        &self.client_ids
    }

    #[must_use]
    pub fn topic_allowlist(&self) -> &[String] {
        &self.topic_allowlist
    }

    #[must_use]
    pub fn retain_allowlist(&self) -> &[String] {
        &self.retain_allowlist
    }

    #[must_use]
    pub const fn hard_limits(&self) -> LiveHardLimits {
        self.hard_limits
    }

    #[must_use]
    pub const fn omit_online_after_reconnect(&self) -> bool {
        matches!(self.scenario, LiveScenario::ReconnectMissingOnline)
    }

    #[must_use]
    pub fn broker_host(&self) -> &str {
        &self.broker_host
    }

    #[must_use]
    pub const fn broker_port(&self) -> u16 {
        self.broker_port
    }

    #[must_use]
    pub fn map_id(&self) -> &str {
        &self.map_id
    }

    #[must_use]
    pub const fn disconnect_x(&self) -> f64 {
        self.disconnect_x
    }

    #[must_use]
    pub fn control_topic(&self) -> &str {
        &self.control_topic
    }

    /// Revalidates a deserialized plan against the complete fixed plan
    /// generated from its run identity and scenario.
    ///
    /// # Errors
    ///
    /// Returns an error if any fixed safety field was modified.
    pub fn validate(&self) -> Result<(), LivePlanError> {
        let expected = Self::new(&self.run_id, self.scenario)?;
        if *self == expected {
            Ok(())
        } else {
            Err(LivePlanError::RunId)
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LiveSimSnapshot {
    pub robot_id: String,
    pub simulation_ns: u64,
    pub x: f64,
    pub y: f64,
    pub theta: f64,
    pub linear_velocity: f64,
    pub angular_velocity: f64,
    pub motion_state: String,
    pub transport_connected: bool,
    pub connection_epoch: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum LivePhase {
    Starting,
    RobotsOnline,
    OrderPublished,
    ConnectionBroken,
    ReconnectedWithoutOnline,
    ReconnectedOnline,
    RobotsArrived,
    CaptureSealed,
    Diagnosed,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "event_type", rename_all = "SCREAMING_SNAKE_CASE")]
pub enum LiveEvent {
    SimSnapshot {
        schema: String,
        source: String,
        evidence: bool,
        source_sequence: u64,
        snapshot: LiveSimSnapshot,
    },
    ScenarioPhase {
        schema: String,
        source: String,
        evidence: bool,
        source_sequence: u64,
        phase: LivePhase,
    },
    WireObserved {
        schema: String,
        source: String,
        evidence: bool,
        source_sequence: u64,
        monotonic_ns: u64,
        topic: String,
        payload: Value,
    },
}

impl LiveEvent {
    #[must_use]
    pub fn sim_snapshot(source_sequence: u64, snapshot: LiveSimSnapshot) -> Self {
        Self::SimSnapshot {
            schema: LIVE_EVENT_SCHEMA.to_owned(),
            source: "SIMULATOR_TRUTH".to_owned(),
            evidence: false,
            source_sequence,
            snapshot,
        }
    }

    #[must_use]
    pub fn phase(source_sequence: u64, phase: LivePhase) -> Self {
        Self::ScenarioPhase {
            schema: LIVE_EVENT_SCHEMA.to_owned(),
            source: "SCENARIO_ORCHESTRATOR".to_owned(),
            evidence: false,
            source_sequence,
            phase,
        }
    }

    #[must_use]
    pub fn wire_observed(
        source_sequence: u64,
        monotonic_ns: u64,
        topic: String,
        payload: Value,
    ) -> Self {
        Self::WireObserved {
            schema: LIVE_EVENT_SCHEMA.to_owned(),
            source: "BROKER_EGRESS_PROJECTION".to_owned(),
            evidence: false,
            source_sequence,
            monotonic_ns,
            topic,
            payload,
        }
    }
}
