//! Isolated VDA 5050 demo harness.

use std::net::IpAddr;

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use thiserror::Error;

pub const PROTOCOL_VERSION: &str = "3.0.0";
pub const TOPIC_PREFIX: &str = "vda5050/v3/lab-demo/demo-001";
pub const DEMO_ROBOT_COUNTS: [usize; 6] = [1, 2, 5, 10, 50, 100];
pub const MAX_ROBOT_COUNT: usize = 100;
/// Compatibility budget for the original one-robot scripted scenario.
pub const MESSAGE_BUDGET: usize = 26;
pub const DURATION_BUDGET_SECONDS: u64 = 60;

mod live;

pub use live::{RunArtifacts, RunOptions, run_live};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum Actor {
    FleetControl,
    MobileRobot,
    Recorder,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QosContract {
    AtMostOnce,
    AtLeastOnce,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct Position {
    pub x: i32,
    pub y: i32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RobotPlan {
    pub serial_number: String,
    pub topic_prefix: String,
    pub order_id: String,
    pub start: Position,
    pub released_end: Position,
    pub horizon_end: Position,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FleetLayout {
    robots: Vec<RobotPlan>,
}

#[derive(Debug, Error, Clone, Copy, PartialEq, Eq)]
pub enum FleetLayoutError {
    #[error("robot count must be in the inclusive range 1..=100")]
    RobotCount,
}

impl FleetLayout {
    /// Builds the deterministic two-dimensional warehouse layout.
    ///
    /// # Errors
    ///
    /// Returns an error when `robot_count` is zero or greater than 100.
    pub fn new(robot_count: usize) -> Result<Self, FleetLayoutError> {
        if !(1..=MAX_ROBOT_COUNT).contains(&robot_count) {
            return Err(FleetLayoutError::RobotCount);
        }
        let robots = (0..robot_count)
            .map(|index| {
                let column = i32::try_from(index / 10).unwrap_or_default();
                let row = i32::try_from(index % 10).unwrap_or_default();
                let start = Position {
                    x: column * 6,
                    y: row * 2,
                };
                let serial_number = format!("demo-{:03}", index + 1);
                RobotPlan {
                    topic_prefix: format!("vda5050/v3/lab-demo/{serial_number}"),
                    order_id: format!("demo-reconnect-{:03}", index + 1),
                    serial_number,
                    start,
                    released_end: Position {
                        x: start.x + 4,
                        y: start.y,
                    },
                    horizon_end: Position {
                        x: start.x + 4,
                        y: start.y + 1,
                    },
                }
            })
            .collect();
        Ok(Self { robots })
    }

    #[must_use]
    pub fn robots(&self) -> &[RobotPlan] {
        &self.robots
    }

    #[must_use]
    pub fn robot_count(&self) -> usize {
        self.robots.len()
    }
}

/// Returns the fail-closed capture budget for a fleet size.
///
/// # Errors
///
/// Returns an error for a fleet size outside 1..=100.
pub const fn message_budget(robot_count: usize) -> Result<usize, FleetLayoutError> {
    if robot_count == 0 || robot_count > MAX_ROBOT_COUNT {
        Err(FleetLayoutError::RobotCount)
    } else {
        Ok(robot_count * 10 + 16)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct WireMessage {
    actor: Actor,
    topic: String,
    payload: Value,
    qos: QosContract,
    retain: bool,
    connection_epoch: String,
}

impl WireMessage {
    #[must_use]
    pub fn actor(&self) -> Actor {
        self.actor
    }

    #[must_use]
    pub fn topic(&self) -> &str {
        &self.topic
    }

    #[must_use]
    pub fn payload(&self) -> &Value {
        &self.payload
    }

    #[must_use]
    pub fn qos(&self) -> QosContract {
        self.qos
    }

    #[must_use]
    pub fn retain(&self) -> bool {
        self.retain
    }

    #[must_use]
    pub fn connection_epoch(&self) -> &str {
        &self.connection_epoch
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BrokerTarget {
    host: String,
    isolated_namespace: bool,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum BrokerTargetError {
    #[error("empty broker host")]
    Empty,
    #[error("unspecified broker addresses are forbidden")]
    Unspecified,
    #[error("routable broker host requires the exact isolated service name 'broker'")]
    Routable,
}

impl BrokerTarget {
    /// Creates a target constrained to loopback or the exact internal Compose
    /// broker identity.
    ///
    /// # Errors
    ///
    /// Returns an error for empty, wildcard, routable, or unapproved internal
    /// host names.
    pub fn new(host: &str, isolated_namespace: bool) -> Result<Self, BrokerTargetError> {
        let host = host.trim();
        if host.is_empty() {
            return Err(BrokerTargetError::Empty);
        }
        if let Ok(address) = host.parse::<IpAddr>() {
            if address.is_unspecified() {
                return Err(BrokerTargetError::Unspecified);
            }
            if !address.is_loopback() {
                return Err(BrokerTargetError::Routable);
            }
        } else if !(matches!(host, "localhost") || isolated_namespace && host == "broker") {
            return Err(BrokerTargetError::Routable);
        }
        Ok(Self {
            host: host.to_owned(),
            isolated_namespace,
        })
    }

    #[must_use]
    pub fn host(&self) -> &str {
        &self.host
    }

    #[must_use]
    pub fn isolated_namespace(&self) -> bool {
        self.isolated_namespace
    }
}

#[derive(Debug, Clone)]
pub struct DemoScenario {
    protocol_version: &'static str,
    client_ids: DemoClientIds,
    messages: Vec<WireMessage>,
}

#[derive(Debug, Clone)]
struct DemoClientIds {
    fleet_control: String,
    mobile_robot: String,
    recorder: String,
}

impl DemoScenario {
    #[must_use]
    pub fn changed_repeated_order() -> Self {
        let messages = vec![
            connection_message(1, "ONLINE", "session-1"),
            order_message(1, false),
            state_message(
                1,
                Position { x: 0, y: 0 },
                true,
                false,
                &[],
                "session-1",
                "demo-order",
            ),
            order_message(2, true),
            state_message(
                2,
                Position { x: 1, y: 0 },
                true,
                false,
                &[json!({
                    "errorType": "SAME_ORDER_UPDATE_ID",
                    "errorLevel": "WARNING",
                    "errorDescription": "Changed content under an existing order update identity"
                })],
                "session-1",
                "demo-order",
            ),
        ];
        Self::new(messages)
    }

    #[must_use]
    pub fn reconnect_missing_online() -> Self {
        let messages = vec![
            connection_message(1, "ONLINE", "session-1"),
            released_base_order(),
            state_message(
                1,
                Position { x: 0, y: 0 },
                true,
                false,
                &[],
                "session-1",
                "demo-reconnect",
            ),
            visualization_message(1, 1, Position { x: 2, y: 0 }, "session-1"),
            state_message(
                2,
                Position { x: 4, y: 0 },
                false,
                false,
                &[],
                "session-1",
                "demo-reconnect",
            ),
            connection_message(2, "CONNECTION_BROKEN", "session-1"),
            state_message(
                3,
                Position { x: 4, y: 0 },
                false,
                true,
                &[],
                "session-2",
                "demo-reconnect",
            ),
        ];
        Self::new(messages)
    }

    fn new(messages: Vec<WireMessage>) -> Self {
        Self {
            protocol_version: PROTOCOL_VERSION,
            client_ids: DemoClientIds {
                fleet_control: "vda5050-demo-fleet-run-0001".to_owned(),
                mobile_robot: "vda5050-demo-agv-run-0001".to_owned(),
                recorder: "vda5050-demo-recorder-run-0001".to_owned(),
            },
            messages,
        }
    }

    #[must_use]
    pub fn protocol_version(&self) -> &str {
        self.protocol_version
    }

    #[must_use]
    pub fn client_id(&self, actor: Actor) -> &str {
        match actor {
            Actor::FleetControl => &self.client_ids.fleet_control,
            Actor::MobileRobot => &self.client_ids.mobile_robot,
            Actor::Recorder => &self.client_ids.recorder,
        }
    }

    #[must_use]
    pub fn scripted_messages(&self) -> Vec<WireMessage> {
        self.messages.clone()
    }

    #[must_use]
    pub const fn message_budget(&self) -> usize {
        MESSAGE_BUDGET
    }
}

/// Serializes scripted messages into the explicit import envelope JSONL form.
///
/// # Errors
///
/// Returns an error if one of the in-memory JSON messages cannot be serialized.
pub fn serialize_envelope_jsonl(messages: &[WireMessage]) -> Result<String, serde_json::Error> {
    let mut output = String::new();
    for message in messages {
        let envelope = json!({
            "topic": message.topic,
            "timestamp": message.payload["timestamp"],
            "payload": message.payload,
        });
        output.push_str(&serde_json::to_string(&envelope)?);
        output.push('\n');
    }
    Ok(output)
}

#[must_use]
pub fn render_ascii_frame(position: Position, incident: bool) -> String {
    let mut grid = vec![vec![' '; 13]; 5];
    for x in 0..=8 {
        grid[2][x + 2] = '-';
    }
    for row in grid.iter_mut().take(5).skip(2) {
        row[6] = '|';
    }
    grid[2][2] = 'A';
    grid[2][6] = 'B';
    grid[4][6] = 'C';
    let draw_x = usize::try_from(position.x.clamp(0, 8)).unwrap_or(0) + 2;
    let draw_y = usize::try_from(position.y.clamp(0, 2)).unwrap_or(0) + 2;
    grid[draw_y][draw_x] = 'R';
    let mut output = String::from("VDA 5050 3.0 isolated warehouse simulation\n");
    for row in grid {
        output.extend(row);
        output.push('\n');
    }
    if incident {
        output.push_str("INCIDENT: SAME_ORDER_UPDATE_ID\n");
    } else {
        output.push_str("Robot follows the released base; horizon remains unreleased.\n");
    }
    output
}

fn header_for(header_id: u64, timestamp: &str, serial_number: &str) -> Value {
    json!({
        "headerId": header_id,
        "timestamp": timestamp,
        "version": PROTOCOL_VERSION,
        "manufacturer": "lab-demo",
        "serialNumber": serial_number
    })
}

fn merge_header_for(
    mut payload: Value,
    header_id: u64,
    timestamp: &str,
    serial_number: &str,
) -> Value {
    let header = header_for(header_id, timestamp, serial_number);
    let object = payload
        .as_object_mut()
        .expect("demo payload constructors always use objects");
    for (key, value) in header
        .as_object()
        .expect("header constructor always uses an object")
    {
        object.insert(key.clone(), value.clone());
    }
    payload
}

fn connection_message(header_id: u64, state: &str, epoch: &str) -> WireMessage {
    connection_message_for(&default_robot_plan(), header_id, state, epoch)
}

pub(crate) fn connection_message_for(
    robot: &RobotPlan,
    header_id: u64,
    state: &str,
    epoch: &str,
) -> WireMessage {
    WireMessage {
        actor: Actor::MobileRobot,
        topic: format!("{}/connection", robot.topic_prefix),
        payload: merge_header_for(
            json!({"connectionState": state}),
            header_id,
            if header_id == 1 {
                "2026-08-07T00:00:00.000Z"
            } else {
                "2026-08-07T00:00:05.000Z"
            },
            &robot.serial_number,
        ),
        qos: QosContract::AtLeastOnce,
        retain: true,
        connection_epoch: epoch.to_owned(),
    }
}

fn order_message(header_id: u64, changed: bool) -> WireMessage {
    let final_node = if changed { "X" } else { "B" };
    WireMessage {
        actor: Actor::FleetControl,
        topic: format!("{TOPIC_PREFIX}/order"),
        payload: merge_header_for(
            json!({
                "orderId": "demo-order",
                "orderUpdateId": 0,
                "nodes": [
                    {"nodeId":"A","sequenceId":0,"released":true,"nodePosition":{"x":0,"y":0,"mapId":"demo"},"actions":[]},
                    {"nodeId":final_node,"sequenceId":2,"released":true,"nodePosition":{"x":4,"y":0,"mapId":"demo"},"actions":[]}
                ],
                "edges": [
                    {"edgeId":"A-B","sequenceId":1,"released":true,"maximumSpeed":1.0,"actions":[]}
                ]
            }),
            header_id,
            if header_id == 1 {
                "2026-08-07T00:00:01.000Z"
            } else {
                "2026-08-07T00:00:03.000Z"
            },
            "demo-001",
        ),
        qos: QosContract::AtMostOnce,
        retain: false,
        connection_epoch: "fleet-session-1".to_owned(),
    }
}

fn released_base_order() -> WireMessage {
    released_base_order_for(&default_robot_plan())
}

pub(crate) fn released_base_order_for(robot: &RobotPlan) -> WireMessage {
    let start_node = format!("A-{}", robot.serial_number);
    let released_node = format!("B-{}", robot.serial_number);
    let horizon_node = format!("C-{}", robot.serial_number);
    WireMessage {
        actor: Actor::FleetControl,
        topic: format!("{}/order", robot.topic_prefix),
        payload: merge_header_for(
            json!({
                "orderId": robot.order_id,
                "orderUpdateId": 0,
                "nodes": [
                    {"nodeId":start_node,"sequenceId":0,"released":true,"nodePosition":{"x":robot.start.x,"y":robot.start.y,"mapId":"warehouse-demo"},"actions":[]},
                    {"nodeId":released_node,"sequenceId":2,"released":true,"nodePosition":{"x":robot.released_end.x,"y":robot.released_end.y,"mapId":"warehouse-demo"},"actions":[]},
                    {"nodeId":horizon_node,"sequenceId":4,"released":false,"nodePosition":{"x":robot.horizon_end.x,"y":robot.horizon_end.y,"mapId":"warehouse-demo"},"actions":[]}
                ],
                "edges": [
                    {"edgeId":format!("A-B-{}", robot.serial_number),"sequenceId":1,"released":true,"maximumSpeed":1.0,"actions":[]},
                    {"edgeId":format!("B-C-{}", robot.serial_number),"sequenceId":3,"released":false,"maximumSpeed":1.0,"actions":[]}
                ]
            }),
            1,
            "2026-08-07T00:00:01.000Z",
            &robot.serial_number,
        ),
        qos: QosContract::AtMostOnce,
        retain: false,
        connection_epoch: "fleet-session-1".to_owned(),
    }
}

fn state_message(
    header_id: u64,
    position: Position,
    driving: bool,
    new_base_request: bool,
    errors: &[Value],
    epoch: &str,
    order_id: &str,
) -> WireMessage {
    let mut robot = default_robot_plan();
    order_id.clone_into(&mut robot.order_id);
    state_message_for(
        &robot,
        header_id,
        position,
        driving,
        new_base_request,
        errors,
        epoch,
    )
}

#[allow(
    clippy::too_many_arguments,
    reason = "VDA state evidence is intentionally explicit"
)]
pub(crate) fn state_message_for(
    robot: &RobotPlan,
    header_id: u64,
    position: Position,
    driving: bool,
    new_base_request: bool,
    errors: &[Value],
    epoch: &str,
) -> WireMessage {
    let at_decision = position == robot.released_end;
    WireMessage {
        actor: Actor::MobileRobot,
        topic: format!("{}/state", robot.topic_prefix),
        payload: merge_header_for(
            json!({
                "orderId": robot.order_id,
                "orderUpdateId": 0,
                "lastNodeId": if at_decision {format!("B-{}", robot.serial_number)} else {format!("A-{}", robot.serial_number)},
                "lastNodeSequenceId": if at_decision {2} else {0},
                "nodeStates": [],
                "edgeStates": [],
                "mobileRobotPosition": {
                    "x": position.x,
                    "y": position.y,
                    "theta": 0,
                    "mapId": "warehouse-demo",
                    "localized": true
                },
                "driving": driving,
                "newBaseRequest": new_base_request,
                "actionStates": [],
                "instantActionStates": [],
                "powerSupply": {"stateOfCharge": 80, "charging": false},
                "operatingMode": "AUTOMATIC",
                "errors": errors,
                "safetyState": {"activeEmergencyStop": "NONE", "fieldViolation": false}
            }),
            header_id,
            &format!("2026-08-07T00:00:0{}.000Z", header_id.saturating_add(1)),
            &robot.serial_number,
        ),
        qos: QosContract::AtMostOnce,
        retain: false,
        connection_epoch: epoch.to_owned(),
    }
}

fn visualization_message(
    header_id: u64,
    reference_state_header_id: u64,
    position: Position,
    epoch: &str,
) -> WireMessage {
    visualization_message_for(
        &default_robot_plan(),
        header_id,
        reference_state_header_id,
        position,
        epoch,
    )
}

pub(crate) fn visualization_message_for(
    robot: &RobotPlan,
    header_id: u64,
    reference_state_header_id: u64,
    position: Position,
    epoch: &str,
) -> WireMessage {
    WireMessage {
        actor: Actor::MobileRobot,
        topic: format!("{}/visualization", robot.topic_prefix),
        payload: merge_header_for(
            json!({
                "referenceStateHeaderId": reference_state_header_id,
                "mobileRobotPosition": {
                    "x": position.x,
                    "y": position.y,
                    "theta": 0,
                    "mapId": "warehouse-demo",
                    "localized": true
                },
                "velocity": {"vx": 1.0, "vy": 0.0, "omega": 0.0}
            }),
            header_id,
            "2026-08-07T00:00:02.000Z",
            &robot.serial_number,
        ),
        qos: QosContract::AtMostOnce,
        retain: false,
        connection_epoch: epoch.to_owned(),
    }
}

fn default_robot_plan() -> RobotPlan {
    FleetLayout::new(1)
        .expect("one robot is always within the fixed demo bounds")
        .robots[0]
        .clone()
}
