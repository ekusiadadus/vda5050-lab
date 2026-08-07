use serde_json::{Value, json};

use crate::{
    Actor, LiveRobotPlan, LiveRunPlan, LiveSimSnapshot, PROTOCOL_VERSION, QosContract, WireMessage,
};

#[must_use]
pub fn build_live_connection(
    robot: &LiveRobotPlan,
    header_id: u64,
    state: &str,
    epoch: &str,
) -> WireMessage {
    WireMessage {
        actor: Actor::MobileRobot,
        topic: format!("{}/connection", robot.topic_prefix),
        payload: with_header(
            json!({"connectionState": state}),
            header_id,
            &timestamp_for(header_id.saturating_mul(50_000_000)),
            &robot.serial_number,
        ),
        qos: QosContract::AtLeastOnce,
        retain: true,
        connection_epoch: epoch.to_owned(),
    }
}

#[must_use]
pub fn build_live_order(plan: &LiveRunPlan, robot: &LiveRobotPlan, header_id: u64) -> WireMessage {
    let start_id = format!("A-{}", robot.serial_number);
    let end_id = format!("B-{}", robot.serial_number);
    WireMessage {
        actor: Actor::FleetControl,
        topic: format!("{}/order", robot.topic_prefix),
        payload: with_header(
            json!({
                "orderId": robot.order_id,
                "orderUpdateId": 0,
                "nodes": [
                    {
                        "nodeId": start_id,
                        "sequenceId": 0,
                        "released": true,
                        "nodePosition": {
                            "x": robot.start.x,
                            "y": robot.start.y,
                            "theta": 0.0,
                            "mapId": plan.map_id()
                        },
                        "actions": []
                    },
                    {
                        "nodeId": end_id,
                        "sequenceId": 2,
                        "released": true,
                        "nodePosition": {
                            "x": robot.released_end.x,
                            "y": robot.released_end.y,
                            "theta": 0.0,
                            "mapId": plan.map_id()
                        },
                        "actions": []
                    }
                ],
                "edges": [{
                    "edgeId": format!("A-B-{}", robot.serial_number),
                    "sequenceId": 1,
                    "startNodeId": format!("A-{}", robot.serial_number),
                    "endNodeId": format!("B-{}", robot.serial_number),
                    "released": true,
                    "maximumSpeed": 1.0,
                    "actions": []
                }]
            }),
            header_id,
            &timestamp_for(500_000_000),
            &robot.serial_number,
        ),
        qos: QosContract::AtMostOnce,
        retain: false,
        connection_epoch: "fleet-session-1".to_owned(),
    }
}

#[must_use]
pub fn build_live_state(
    plan: &LiveRunPlan,
    robot: &LiveRobotPlan,
    header_id: u64,
    snapshot: &LiveSimSnapshot,
    new_base_request: bool,
) -> WireMessage {
    let arrived = snapshot.motion_state == "ARRIVED";
    let node_states = if arrived {
        Vec::new()
    } else {
        vec![json!({
            "nodeId": format!("B-{}", robot.serial_number),
            "sequenceId": 2,
            "released": true,
            "nodePosition": {
                "x": robot.released_end.x,
                "y": robot.released_end.y,
                "theta": 0.0,
                "mapId": plan.map_id()
            }
        })]
    };
    let edge_states = if arrived {
        Vec::new()
    } else {
        vec![json!({
            "edgeId": format!("A-B-{}", robot.serial_number),
            "sequenceId": 1,
            "released": true
        })]
    };
    WireMessage {
        actor: Actor::MobileRobot,
        topic: format!("{}/state", robot.topic_prefix),
        payload: with_header(
            json!({
                "orderId": robot.order_id,
                "orderUpdateId": 0,
                "lastNodeId": if arrived {
                    format!("B-{}", robot.serial_number)
                } else {
                    format!("A-{}", robot.serial_number)
                },
                "lastNodeSequenceId": if arrived { 2 } else { 0 },
                "nodeStates": node_states,
                "edgeStates": edge_states,
                "mobileRobotPosition": position(snapshot, plan.map_id()),
                "velocity": velocity(snapshot),
                "driving": snapshot.motion_state == "DRIVING",
                "newBaseRequest": new_base_request,
                "actionStates": [],
                "instantActionStates": [],
                "powerSupply": {"stateOfCharge": 80, "charging": false},
                "operatingMode": "AUTOMATIC",
                "errors": [],
                "safetyState": {"activeEmergencyStop": "NONE", "fieldViolation": false}
            }),
            header_id,
            &timestamp_for(snapshot.simulation_ns),
            &robot.serial_number,
        ),
        qos: QosContract::AtMostOnce,
        retain: false,
        connection_epoch: snapshot.connection_epoch.clone(),
    }
}

#[must_use]
pub fn build_live_visualization(
    plan: &LiveRunPlan,
    robot: &LiveRobotPlan,
    header_id: u64,
    reference_state_header_id: u64,
    snapshot: &LiveSimSnapshot,
) -> WireMessage {
    WireMessage {
        actor: Actor::MobileRobot,
        topic: format!("{}/visualization", robot.topic_prefix),
        payload: with_header(
            json!({
                "referenceStateHeaderId": reference_state_header_id,
                "mobileRobotPosition": position(snapshot, plan.map_id()),
                "velocity": velocity(snapshot)
            }),
            header_id,
            &timestamp_for(snapshot.simulation_ns),
            &robot.serial_number,
        ),
        qos: QosContract::AtMostOnce,
        retain: false,
        connection_epoch: snapshot.connection_epoch.clone(),
    }
}

fn position(snapshot: &LiveSimSnapshot, map_id: &str) -> Value {
    json!({
        "x": snapshot.x,
        "y": snapshot.y,
        "theta": snapshot.theta,
        "mapId": map_id,
        "localized": true
    })
}

fn velocity(snapshot: &LiveSimSnapshot) -> Value {
    json!({
        "vx": snapshot.linear_velocity,
        "vy": 0.0,
        "omega": snapshot.angular_velocity
    })
}

fn with_header(mut payload: Value, header_id: u64, timestamp: &str, serial_number: &str) -> Value {
    let object = payload
        .as_object_mut()
        .expect("live protocol payloads are always JSON objects");
    object.insert("headerId".to_owned(), json!(header_id));
    object.insert("timestamp".to_owned(), json!(timestamp));
    object.insert("version".to_owned(), json!(PROTOCOL_VERSION));
    object.insert("manufacturer".to_owned(), json!("lab-demo"));
    object.insert("serialNumber".to_owned(), json!(serial_number));
    payload
}

fn timestamp_for(simulation_ns: u64) -> String {
    let total_millis = simulation_ns / 1_000_000;
    let seconds = total_millis / 1_000;
    let millis = total_millis % 1_000;
    format!("2026-08-07T00:00:{seconds:02}.{millis:03}Z")
}
