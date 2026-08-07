use serde_json::json;
use vda5050_demo::{
    LiveCapturedPublish, LsmartAction, LsmartActionBatch, LsmartActionKind, LsmartExecutionGate,
    LsmartPose, LsmartRobot, LsmartRunPlan, build_lsmart_capture_artifacts, build_lsmart_delivery,
    build_lsmart_order, build_lsmart_state, parse_lsmart_action_batch,
    validate_lsmart_delivered_order,
};

fn robot() -> LsmartRobot {
    LsmartRobot::new("fb0", "vda5050/v3/lsmart-lab/fb0").expect("synthetic robot")
}

fn batch() -> LsmartActionBatch {
    LsmartActionBatch::new(
        "lsmart-run-0001",
        "fb0-000007",
        "fb0",
        70,
        vec![
            LsmartAction::new(41, LsmartActionKind::Turn, [1.0, 2.0], [1.0, 2.0], 1, -1)
                .expect("turn"),
            LsmartAction::new(42, LsmartActionKind::Move, [1.0, 2.0], [1.0, 3.0], 1, -1)
                .expect("move"),
            LsmartAction::new(
                43,
                LsmartActionKind::Station,
                [1.0, 3.0],
                [1.0, 3.0],
                1,
                912,
            )
            .expect("station"),
        ],
    )
    .expect("batch")
}

#[test]
fn actual_lsmart_action_batch_becomes_a_released_vda_3_order() {
    let batch = batch();
    let order =
        build_lsmart_order(&robot(), &batch, 11, "2026-08-07T00:00:07.000Z").expect("valid order");

    assert_eq!(order.topic(), "vda5050/v3/lsmart-lab/fb0/order");
    assert_eq!(order.payload()["version"], "3.0.0");
    assert_eq!(order.payload()["manufacturer"], "lsmart-lab");
    assert_eq!(order.payload()["serialNumber"], "fb0");
    assert_eq!(order.payload()["orderId"], "lsmart-run-0001-fb0-000007");
    assert_eq!(order.payload()["orderUpdateId"], 0);
    assert_eq!(order.payload()["nodes"].as_array().map(Vec::len), Some(4));
    assert_eq!(order.payload()["edges"].as_array().map(Vec::len), Some(3));
    assert!(
        order.payload()["nodes"]
            .as_array()
            .expect("nodes")
            .iter()
            .all(|node| node["released"] == true)
    );
    assert_eq!(
        order.payload()["nodes"][3]["actions"][0],
        json!({
            "actionId": "lsmart-task-912",
            "actionType": "lsmartStation",
            "blockingType": "HARD",
            "actionParameters": [{"key": "taskId", "value": "912"}]
        })
    );
    assert!(!order.retain());
}

#[test]
fn execution_gate_stays_closed_until_the_matching_order_returns_from_mqtt() {
    let batch = batch();
    let order =
        build_lsmart_order(&robot(), &batch, 11, "2026-08-07T00:00:07.000Z").expect("valid order");
    let mut gate = LsmartExecutionGate::new(batch.clone()).expect("gate");

    assert!(!gate.execution_authorized());
    assert!(validate_lsmart_delivered_order(&batch, order.payload()).is_ok());
    gate.accept_mqtt_delivery(order.payload())
        .expect("delivery");
    assert!(gate.execution_authorized());
    assert!(gate.take_actions().is_some());
    assert!(gate.take_actions().is_none());
}

#[test]
fn execution_gate_rejects_a_different_batch_even_for_the_same_robot() {
    let batch = batch();
    let mut different = batch.clone();
    different.batch_id = "fb0-000008".to_owned();
    let wrong = build_lsmart_order(&robot(), &different, 12, "2026-08-07T00:00:08.000Z")
        .expect("valid order");
    let mut gate = LsmartExecutionGate::new(batch).expect("gate");

    assert!(gate.accept_mqtt_delivery(wrong.payload()).is_err());
    assert!(!gate.execution_authorized());
}

#[test]
fn state_uses_the_pose_observed_inside_argos() {
    let batch = batch();
    let pose = LsmartPose::new(
        "fb0",
        73,
        1.25,
        2.75,
        std::f64::consts::FRAC_PI_2,
        0.4,
        0.0,
        "DRIVING",
        Some(42),
        Some(batch.order_id()),
    )
    .expect("pose");
    let state = build_lsmart_state(
        &robot(),
        &pose,
        17,
        "2026-08-07T00:00:07.300Z",
        "lsmart-session-1",
    )
    .expect("state");

    assert_eq!(state.payload()["mobileRobotPosition"]["x"], 1.25);
    assert_eq!(state.payload()["mobileRobotPosition"]["y"], 2.75);
    assert_eq!(
        state.payload()["mobileRobotPosition"]["theta"],
        std::f64::consts::FRAC_PI_2
    );
    assert_eq!(state.payload()["velocity"]["vx"], 0.4);
    assert_eq!(state.payload()["lastNodeId"], "lsmart-node-42");
    assert_eq!(state.payload()["driving"], true);
}

#[test]
fn overlay_batch_is_revalidated_before_it_can_reach_mqtt() {
    let expected = batch();
    let encoded = serde_json::to_vec(&expected).expect("encode");
    let parsed = parse_lsmart_action_batch(&encoded, "lsmart-run-0001", 64).expect("parse");
    assert_eq!(parsed, expected);

    let mut wrong_schema = serde_json::to_value(&expected).expect("value");
    wrong_schema["schema"] = json!("untrusted/9");
    assert!(
        parse_lsmart_action_batch(
            &serde_json::to_vec(&wrong_schema).expect("encode"),
            "lsmart-run-0001",
            64,
        )
        .is_err()
    );
    assert!(parse_lsmart_action_batch(&encoded, "different-run", 64).is_err());
    assert!(parse_lsmart_action_batch(&encoded, "lsmart-run-0001", 2).is_err());
}

#[test]
fn delivered_envelope_returns_only_the_batch_observed_on_mqtt() {
    let batch = batch();
    let order =
        build_lsmart_order(&robot(), &batch, 11, "2026-08-07T00:00:07.000Z").expect("order");
    let delivery = build_lsmart_delivery(&batch, order.payload()).expect("delivery");

    assert_eq!(delivery["schema"], "vda5050-lab.lsmart-delivery/1");
    assert_eq!(delivery["status"], "mqtt-delivered");
    assert_eq!(delivery["run_id"], batch.run_id);
    assert_eq!(delivery["batch_id"], batch.batch_id);
    assert_eq!(delivery["robot_id"], batch.robot_id);
    assert_eq!(delivery["order_id"], batch.order_id());
    assert_eq!(delivery["actions"], json!(batch.actions));
}

#[test]
fn run_plan_seals_the_official_lsmart_source_before_any_connection() {
    let plan = LsmartRunPlan::new("lsmart-run-0001", true).expect("plan");

    assert_eq!(plan.robots.len(), 10);
    assert_eq!(plan.map, "maps/kiva_large_w_mode.json");
    assert_eq!(plan.planner, "RHCR");
    assert_eq!(plan.fail_policy, "PIBT");
    assert_eq!(plan.simulation_ticks, 600);
    assert_eq!(
        plan.source_commit,
        "ce0a020d8da10a806b23ffa3ccb56b1affff57d1"
    );
    assert_eq!(
        plan.source_url,
        "https://github.com/smart-mapf/lifelong-smart.git"
    );
    assert!(plan.inject_reconnect_fault);
    assert!(plan.validate().is_ok());
}

#[test]
fn actual_lsmart_mqtt_capture_is_closed_with_same_job_role_evidence() {
    let plan = LsmartRunPlan::new("lsmart-run-0001", true).expect("plan");
    let messages = [
        (
            "connection",
            json!({
                "headerId": 1, "timestamp": "2026-08-07T00:00:00.000Z",
                "version": "3.0.0", "manufacturer": "lsmart-lab",
                "serialNumber": "0", "connectionState": "ONLINE"
            }),
        ),
        (
            "connection",
            json!({
                "headerId": 2, "timestamp": "2026-08-07T00:00:15.000Z",
                "version": "3.0.0", "manufacturer": "lsmart-lab",
                "serialNumber": "0", "connectionState": "CONNECTION_BROKEN"
            }),
        ),
        (
            "state",
            json!({
                "headerId": 3, "timestamp": "2026-08-07T00:00:16.000Z",
                "version": "3.0.0", "manufacturer": "lsmart-lab",
                "serialNumber": "0", "driving": true
            }),
        ),
    ]
    .into_iter()
    .enumerate()
    .map(|(index, (suffix, payload))| {
        LiveCapturedPublish::new(
            format!("vda5050/v3/lsmart-lab/0/{suffix}"),
            serde_json::to_vec(&payload).expect("payload"),
            0,
            false,
            u64::try_from(index + 1).expect("index") * 1_000,
        )
    })
    .collect::<Vec<_>>();

    let artifacts = build_lsmart_capture_artifacts(&plan, &messages).expect("capture");
    let lines = artifacts.trace_bytes.split(|byte| *byte == b'\n').count() - 1;
    assert_eq!(lines, messages.len() + 2);
    assert!(artifacts.evidence.capture_closed);
    assert!(artifacts.evidence.role_attribution_complete);
    assert_eq!(artifacts.evidence.events.len(), messages.len());
}
