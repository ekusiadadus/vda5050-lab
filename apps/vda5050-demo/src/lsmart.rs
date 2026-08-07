use std::f64::consts::{FRAC_PI_2, PI};

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use thiserror::Error;

use crate::{Actor, PROTOCOL_VERSION, QosContract, WireMessage};

const MAP_ID: &str = "lsmart-kiva-large";

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum LsmartError {
    #[error("LSMART identifier must contain only ASCII letters, digits, '-' or '_'")]
    Identifier,
    #[error("LSMART topic prefix does not match the synthetic robot identity")]
    Topic,
    #[error("LSMART action batch must contain at least one action")]
    EmptyBatch,
    #[error("LSMART action contains a non-finite coordinate")]
    Coordinate,
    #[error("LSMART orientation must be in 0..=3")]
    Orientation,
    #[error("LSMART station action requires a non-negative task ID")]
    TaskId,
    #[error("delivered VDA order does not match the pending LSMART action batch")]
    DeliveryMismatch,
    #[error("LSMART execution actions have already been consumed")]
    AlreadyConsumed,
    #[error("invalid LSMART bridge JSON: {0}")]
    Json(String),
    #[error("LSMART bridge schema or run identity does not match the sealed plan")]
    Envelope,
    #[error("LSMART action batch exceeds its hard action limit")]
    ActionLimit,
    #[error("LSMART run plan differs from the fixed official integration contract")]
    PlanMismatch,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum LsmartActionKind {
    Move,
    Turn,
    Station,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LsmartAction {
    pub node_id: i64,
    pub kind: LsmartActionKind,
    pub start: [f64; 2],
    pub end: [f64; 2],
    pub orientation: u8,
    pub task_id: i64,
}

impl LsmartAction {
    /// Creates one action obtained from the official LSMART ADG.
    ///
    /// # Errors
    ///
    /// Returns an error for invalid coordinates, orientation, or station task.
    pub fn new(
        node_id: i64,
        kind: LsmartActionKind,
        start: [f64; 2],
        end: [f64; 2],
        orientation: u8,
        task_id: i64,
    ) -> Result<Self, LsmartError> {
        if !start.into_iter().chain(end).all(f64::is_finite) {
            return Err(LsmartError::Coordinate);
        }
        if orientation > 3 {
            return Err(LsmartError::Orientation);
        }
        if kind == LsmartActionKind::Station && task_id < 0 {
            return Err(LsmartError::TaskId);
        }
        Ok(Self {
            node_id,
            kind,
            start,
            end,
            orientation,
            task_id,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LsmartActionBatch {
    pub schema: String,
    pub run_id: String,
    pub batch_id: String,
    pub robot_id: String,
    pub simulation_tick: u64,
    pub actions: Vec<LsmartAction>,
}

impl LsmartActionBatch {
    /// Creates a bounded action batch emitted by the official LSMART overlay.
    ///
    /// # Errors
    ///
    /// Returns an error for invalid identities or an empty action list.
    pub fn new(
        run_id: impl Into<String>,
        batch_id: impl Into<String>,
        robot_id: impl Into<String>,
        simulation_tick: u64,
        actions: Vec<LsmartAction>,
    ) -> Result<Self, LsmartError> {
        let run_id = run_id.into();
        let batch_id = batch_id.into();
        let robot_id = robot_id.into();
        if !valid_identifier(&run_id)
            || !valid_identifier(&batch_id)
            || !valid_identifier(&robot_id)
        {
            return Err(LsmartError::Identifier);
        }
        if actions.is_empty() {
            return Err(LsmartError::EmptyBatch);
        }
        Ok(Self {
            schema: "vda5050-lab.lsmart-action-batch/1".to_owned(),
            run_id,
            batch_id,
            robot_id,
            simulation_tick,
            actions,
        })
    }

    #[must_use]
    pub fn order_id(&self) -> String {
        format!("{}-{}", self.run_id, self.batch_id)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LsmartRobot {
    pub serial_number: String,
    pub topic_prefix: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[allow(clippy::struct_excessive_bools)]
pub struct LsmartRunPlan {
    pub schema: String,
    pub run_id: String,
    pub synthetic: bool,
    pub source_url: String,
    pub source_commit: String,
    pub source_license_status: String,
    pub redistribute_source: bool,
    pub redistribute_image: bool,
    pub map: String,
    pub planner: String,
    pub fail_policy: String,
    pub task_assigner: String,
    pub planner_invoke_policy: String,
    pub planning_window: u64,
    pub simulation_ticks: u64,
    pub ticks_per_second: u64,
    pub broker_host: String,
    pub broker_port: u16,
    pub inject_reconnect_fault: bool,
    pub fault_target: String,
    pub message_limit: usize,
    pub duration_seconds: u64,
    pub control_topic: String,
    pub robots: Vec<LsmartRobot>,
}

impl LsmartRunPlan {
    /// Seals the only approved official LSMART demo topology before CONNECT.
    ///
    /// # Errors
    ///
    /// Returns an error when the run identity is not safe for artifacts and MQTT.
    pub fn new(run_id: &str, inject_reconnect_fault: bool) -> Result<Self, LsmartError> {
        if !valid_identifier(run_id) {
            return Err(LsmartError::Identifier);
        }
        let robots = (0..10)
            .map(|index| {
                let serial = index.to_string();
                LsmartRobot::new(&serial, format!("vda5050/v3/lsmart-lab/{serial}"))
            })
            .collect::<Result<Vec<_>, _>>()?;
        Ok(Self {
            schema: "vda5050-lab.lsmart-run/1".to_owned(),
            run_id: run_id.to_owned(),
            synthetic: true,
            source_url: "https://github.com/smart-mapf/lifelong-smart.git".to_owned(),
            source_commit: "ce0a020d8da10a806b23ffa3ccb56b1affff57d1".to_owned(),
            source_license_status: "NO_ROOT_LICENSE_FILE_OBSERVED".to_owned(),
            redistribute_source: false,
            redistribute_image: false,
            map: "maps/kiva_large_w_mode.json".to_owned(),
            planner: "RHCR".to_owned(),
            fail_policy: "PIBT".to_owned(),
            task_assigner: "windowed".to_owned(),
            planner_invoke_policy: "default".to_owned(),
            planning_window: 10,
            simulation_ticks: 600,
            ticks_per_second: 10,
            broker_host: "broker".to_owned(),
            broker_port: 1883,
            inject_reconnect_fault,
            fault_target: "0".to_owned(),
            message_limit: 8_192,
            duration_seconds: 180,
            control_topic: format!("vda5050-lab/lsmart/{run_id}/complete"),
            robots,
        })
    }

    /// Reconstructs and compares the complete fixed run contract.
    ///
    /// # Errors
    ///
    /// Returns an error when a deserialized field was modified.
    pub fn validate(&self) -> Result<(), LsmartError> {
        let expected = Self::new(&self.run_id, self.inject_reconnect_fault)?;
        if self == &expected {
            Ok(())
        } else {
            Err(LsmartError::PlanMismatch)
        }
    }

    #[must_use]
    pub fn robot(&self, serial_number: &str) -> Option<&LsmartRobot> {
        self.robots
            .iter()
            .find(|robot| robot.serial_number == serial_number)
    }

    #[must_use]
    pub fn topic_allowed(&self, topic: &str) -> bool {
        self.robots.iter().any(|robot| {
            ["connection", "order", "state", "visualization"]
                .into_iter()
                .any(|suffix| topic == format!("{}/{suffix}", robot.topic_prefix))
        })
    }
}

impl LsmartRobot {
    /// Creates a synthetic VDA identity for one LSMART foot-bot.
    ///
    /// # Errors
    ///
    /// Returns an error when the identity or exact topic prefix is invalid.
    pub fn new(
        serial_number: impl Into<String>,
        topic_prefix: impl Into<String>,
    ) -> Result<Self, LsmartError> {
        let serial_number = serial_number.into();
        let topic_prefix = topic_prefix.into();
        if !valid_identifier(&serial_number) {
            return Err(LsmartError::Identifier);
        }
        if topic_prefix != format!("vda5050/v3/lsmart-lab/{serial_number}") {
            return Err(LsmartError::Topic);
        }
        Ok(Self {
            serial_number,
            topic_prefix,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LsmartPose {
    pub schema: String,
    pub robot_id: String,
    pub simulation_tick: u64,
    pub x: f64,
    pub y: f64,
    pub theta: f64,
    pub linear_velocity: f64,
    pub angular_velocity: f64,
    pub motion_state: String,
    pub last_node_id: Option<i64>,
    pub order_id: Option<String>,
}

impl LsmartPose {
    /// Creates one pose sampled inside the `ARGoS` controller.
    ///
    /// # Errors
    ///
    /// Returns an error for an invalid robot identity or non-finite motion data.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        robot_id: impl Into<String>,
        simulation_tick: u64,
        x: f64,
        y: f64,
        theta: f64,
        linear_velocity: f64,
        angular_velocity: f64,
        motion_state: impl Into<String>,
        last_node_id: Option<i64>,
        order_id: Option<String>,
    ) -> Result<Self, LsmartError> {
        let robot_id = robot_id.into();
        if !valid_identifier(&robot_id) {
            return Err(LsmartError::Identifier);
        }
        if ![x, y, theta, linear_velocity, angular_velocity]
            .into_iter()
            .all(f64::is_finite)
        {
            return Err(LsmartError::Coordinate);
        }
        Ok(Self {
            schema: "vda5050-lab.lsmart-pose/1".to_owned(),
            robot_id,
            simulation_tick,
            x,
            y,
            theta,
            linear_velocity,
            angular_velocity,
            motion_state: motion_state.into(),
            last_node_id,
            order_id,
        })
    }
}

#[derive(Debug, Clone)]
pub struct LsmartExecutionGate {
    batch: Option<LsmartActionBatch>,
    authorized: bool,
}

impl LsmartExecutionGate {
    /// Creates a closed execution gate for one pending ADG action batch.
    ///
    /// # Errors
    ///
    /// Returns an error if the supplied batch is structurally invalid.
    pub fn new(batch: LsmartActionBatch) -> Result<Self, LsmartError> {
        if batch.actions.is_empty() {
            return Err(LsmartError::EmptyBatch);
        }
        Ok(Self {
            batch: Some(batch),
            authorized: false,
        })
    }

    #[must_use]
    pub const fn execution_authorized(&self) -> bool {
        self.authorized
    }

    /// Opens the gate only for the matching VDA order observed on MQTT.
    ///
    /// # Errors
    ///
    /// Returns an error when the order differs from the pending action batch.
    pub fn accept_mqtt_delivery(&mut self, order: &Value) -> Result<(), LsmartError> {
        let batch = self.batch.as_ref().ok_or(LsmartError::AlreadyConsumed)?;
        validate_lsmart_delivered_order(batch, order)?;
        self.authorized = true;
        Ok(())
    }

    pub fn take_actions(&mut self) -> Option<Vec<LsmartAction>> {
        if !self.authorized {
            return None;
        }
        self.authorized = false;
        self.batch.take().map(|batch| batch.actions)
    }
}

/// Converts an actual LSMART action batch into a released VDA 5050 order.
///
/// # Errors
///
/// Returns an error when the robot identity differs from the batch producer.
pub fn build_lsmart_order(
    robot: &LsmartRobot,
    batch: &LsmartActionBatch,
    header_id: u64,
    timestamp: &str,
) -> Result<WireMessage, LsmartError> {
    if robot.serial_number != batch.robot_id {
        return Err(LsmartError::DeliveryMismatch);
    }
    let mut nodes = Vec::with_capacity(batch.actions.len() + 1);
    let mut edges = Vec::with_capacity(batch.actions.len());
    let start = batch.actions.first().ok_or(LsmartError::EmptyBatch)?.start;
    let start_id = format!("lsmart-start-{}", batch.batch_id);
    nodes.push(node(&start_id, 0, start, 0.0, &[]));
    let mut previous_id = start_id;
    for (index, action) in batch.actions.iter().enumerate() {
        let sequence = u64::try_from(index + 1).unwrap_or(u64::MAX) * 2;
        let node_id = format!("lsmart-node-{}", action.node_id);
        let actions = if action.kind == LsmartActionKind::Station {
            vec![json!({
                "actionId": format!("lsmart-task-{}", action.task_id),
                "actionType": "lsmartStation",
                "blockingType": "HARD",
                "actionParameters": [{
                    "key": "taskId",
                    "value": action.task_id.to_string()
                }]
            })]
        } else {
            Vec::new()
        };
        nodes.push(node(
            &node_id,
            sequence,
            action.end,
            orientation_theta(action.orientation),
            &actions,
        ));
        edges.push(json!({
            "edgeId": format!("lsmart-edge-{}-{}", batch.batch_id, index + 1),
            "sequenceId": sequence - 1,
            "startNodeId": previous_id,
            "endNodeId": node_id,
            "released": true,
            "maximumSpeed": 2.0,
            "actions": []
        }));
        previous_id = node_id;
    }
    let payload = with_header(
        json!({
            "orderId": batch.order_id(),
            "orderUpdateId": 0,
            "nodes": nodes,
            "edges": edges
        }),
        header_id,
        timestamp,
        &robot.serial_number,
    );
    Ok(WireMessage {
        actor: Actor::FleetControl,
        topic: format!("{}/order", robot.topic_prefix),
        payload,
        qos: QosContract::AtMostOnce,
        retain: false,
        connection_epoch: "lsmart-fleet-session-1".to_owned(),
    })
}

#[must_use]
pub fn build_lsmart_connection(
    robot: &LsmartRobot,
    header_id: u64,
    state: &str,
    timestamp: &str,
    connection_epoch: &str,
) -> WireMessage {
    WireMessage {
        actor: Actor::MobileRobot,
        topic: format!("{}/connection", robot.topic_prefix),
        payload: with_header(
            json!({"connectionState": state}),
            header_id,
            timestamp,
            &robot.serial_number,
        ),
        qos: QosContract::AtLeastOnce,
        retain: true,
        connection_epoch: connection_epoch.to_owned(),
    }
}

/// Checks that an order returned by MQTT is the exact projection of a batch.
///
/// # Errors
///
/// Returns an error when identity, order ID, node count, or node IDs differ.
pub fn validate_lsmart_delivered_order(
    batch: &LsmartActionBatch,
    order: &Value,
) -> Result<(), LsmartError> {
    let nodes = order["nodes"]
        .as_array()
        .ok_or(LsmartError::DeliveryMismatch)?;
    let identities_match = order["version"] == PROTOCOL_VERSION
        && order["manufacturer"] == "lsmart-lab"
        && order["serialNumber"] == batch.robot_id
        && order["orderId"] == batch.order_id()
        && order["orderUpdateId"] == 0
        && nodes.len() == batch.actions.len() + 1;
    if !identities_match {
        return Err(LsmartError::DeliveryMismatch);
    }
    for (node, action) in nodes.iter().skip(1).zip(&batch.actions) {
        if node["nodeId"] != format!("lsmart-node-{}", action.node_id)
            || node["nodePosition"]["x"] != action.end[0]
            || node["nodePosition"]["y"] != action.end[1]
            || node["released"] != true
        {
            return Err(LsmartError::DeliveryMismatch);
        }
    }
    Ok(())
}

/// Revalidates an untrusted action-batch file produced by the LSMART overlay.
///
/// # Errors
///
/// Returns an error for malformed JSON, a different run, invalid actions, or
/// an action count outside the fixed caller-provided bound.
pub fn parse_lsmart_action_batch(
    bytes: &[u8],
    expected_run_id: &str,
    max_actions: usize,
) -> Result<LsmartActionBatch, LsmartError> {
    if max_actions == 0 {
        return Err(LsmartError::ActionLimit);
    }
    let batch: LsmartActionBatch =
        serde_json::from_slice(bytes).map_err(|error| LsmartError::Json(error.to_string()))?;
    if batch.schema != "vda5050-lab.lsmart-action-batch/1"
        || batch.run_id != expected_run_id
        || !valid_identifier(&batch.batch_id)
        || !valid_identifier(&batch.robot_id)
    {
        return Err(LsmartError::Envelope);
    }
    if batch.actions.is_empty() || batch.actions.len() > max_actions {
        return Err(LsmartError::ActionLimit);
    }
    for action in &batch.actions {
        LsmartAction::new(
            action.node_id,
            action.kind,
            action.start,
            action.end,
            action.orientation,
            action.task_id,
        )?;
    }
    Ok(batch)
}

/// Creates the inbox envelope only after the matching order was observed on MQTT.
///
/// # Errors
///
/// Returns an error when the delivered order differs from the pending batch.
pub fn build_lsmart_delivery(
    batch: &LsmartActionBatch,
    delivered_order: &Value,
) -> Result<Value, LsmartError> {
    validate_lsmart_delivered_order(batch, delivered_order)?;
    Ok(json!({
        "schema": "vda5050-lab.lsmart-delivery/1",
        "status": "mqtt-delivered",
        "run_id": batch.run_id,
        "batch_id": batch.batch_id,
        "robot_id": batch.robot_id,
        "order_id": batch.order_id(),
        "actions": batch.actions
    }))
}

/// Builds a VDA state from a pose sampled by the `ARGoS` robot controller.
///
/// # Errors
///
/// Returns an error if the pose belongs to a different robot.
pub fn build_lsmart_state(
    robot: &LsmartRobot,
    pose: &LsmartPose,
    header_id: u64,
    timestamp: &str,
    connection_epoch: &str,
) -> Result<WireMessage, LsmartError> {
    if robot.serial_number != pose.robot_id {
        return Err(LsmartError::DeliveryMismatch);
    }
    let last_node_id = pose.last_node_id.map_or_else(
        || "lsmart-initial".to_owned(),
        |id| format!("lsmart-node-{id}"),
    );
    let payload = with_header(
        json!({
            "orderId": pose.order_id.clone().unwrap_or_default(),
            "orderUpdateId": 0,
            "lastNodeId": last_node_id,
            "lastNodeSequenceId": 0,
            "nodeStates": [],
            "edgeStates": [],
            "mobileRobotPosition": {
                "x": pose.x,
                "y": pose.y,
                "theta": pose.theta,
                "mapId": MAP_ID,
                "localized": true
            },
            "velocity": {
                "vx": pose.linear_velocity,
                "vy": 0.0,
                "omega": pose.angular_velocity
            },
            "driving": pose.motion_state == "DRIVING",
            "newBaseRequest": false,
            "actionStates": [],
            "instantActionStates": [],
            "powerSupply": {"stateOfCharge": 80, "charging": false},
            "operatingMode": "AUTOMATIC",
            "errors": [],
            "safetyState": {"activeEmergencyStop": "NONE", "fieldViolation": false}
        }),
        header_id,
        timestamp,
        &robot.serial_number,
    );
    Ok(WireMessage {
        actor: Actor::MobileRobot,
        topic: format!("{}/state", robot.topic_prefix),
        payload,
        qos: QosContract::AtMostOnce,
        retain: false,
        connection_epoch: connection_epoch.to_owned(),
    })
}

/// Builds a VDA visualization from one pose sampled in `ARGoS`.
///
/// # Errors
///
/// Returns an error if the pose belongs to a different robot.
pub fn build_lsmart_visualization(
    robot: &LsmartRobot,
    pose: &LsmartPose,
    header_id: u64,
    reference_state_header_id: u64,
    timestamp: &str,
    connection_epoch: &str,
) -> Result<WireMessage, LsmartError> {
    if robot.serial_number != pose.robot_id {
        return Err(LsmartError::DeliveryMismatch);
    }
    let payload = with_header(
        json!({
            "referenceStateHeaderId": reference_state_header_id,
            "mobileRobotPosition": {
                "x": pose.x,
                "y": pose.y,
                "theta": pose.theta,
                "mapId": MAP_ID,
                "localized": true
            },
            "velocity": {
                "vx": pose.linear_velocity,
                "vy": 0.0,
                "omega": pose.angular_velocity
            }
        }),
        header_id,
        timestamp,
        &robot.serial_number,
    );
    Ok(WireMessage {
        actor: Actor::MobileRobot,
        topic: format!("{}/visualization", robot.topic_prefix),
        payload,
        qos: QosContract::AtMostOnce,
        retain: false,
        connection_epoch: connection_epoch.to_owned(),
    })
}

fn node(id: &str, sequence_id: u64, position: [f64; 2], theta: f64, actions: &[Value]) -> Value {
    json!({
        "nodeId": id,
        "sequenceId": sequence_id,
        "released": true,
        "nodePosition": {
            "x": position[0],
            "y": position[1],
            "theta": theta,
            "mapId": MAP_ID
        },
        "actions": actions
    })
}

fn orientation_theta(orientation: u8) -> f64 {
    match orientation {
        0 => 0.0,
        1 => -FRAC_PI_2,
        2 => PI,
        3 => FRAC_PI_2,
        _ => unreachable!("validated LSMART orientation"),
    }
}

fn with_header(mut payload: Value, header_id: u64, timestamp: &str, serial_number: &str) -> Value {
    let object = payload
        .as_object_mut()
        .expect("LSMART protocol payloads are JSON objects");
    object.insert("headerId".to_owned(), json!(header_id));
    object.insert("timestamp".to_owned(), json!(timestamp));
    object.insert("version".to_owned(), json!(PROTOCOL_VERSION));
    object.insert("manufacturer".to_owned(), json!("lsmart-lab"));
    object.insert("serialNumber".to_owned(), json!(serial_number));
    payload
}

fn valid_identifier(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
}
