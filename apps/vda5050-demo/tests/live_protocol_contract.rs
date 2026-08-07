use vda5050_demo::{
    LiveRunPlan, LiveScenario, LiveSimSnapshot, QosContract, build_live_connection,
    build_live_order, build_live_state, build_live_visualization,
};

fn snapshot() -> LiveSimSnapshot {
    LiveSimSnapshot {
        robot_id: "demo-001".to_owned(),
        simulation_ns: 2_500_000_000,
        x: 2.5,
        y: 0.0,
        theta: 0.0,
        linear_velocity: 0.75,
        angular_velocity: 0.0,
        motion_state: "DRIVING".to_owned(),
        transport_connected: true,
        connection_epoch: "session-1".to_owned(),
    }
}

#[test]
fn live_messages_are_vda_300_and_preserve_mqtt_contract() {
    let plan = LiveRunPlan::new("live-0001", LiveScenario::ReconnectMissingOnline).expect("plan");
    let robot = &plan.robots()[0];
    let online = build_live_connection(robot, 1, "ONLINE", "session-1");
    let order = build_live_order(&plan, robot, 1);
    let state = build_live_state(&plan, robot, 2, &snapshot(), false);
    let visualization = build_live_visualization(&plan, robot, 3, 2, &snapshot());

    for message in [&online, &order, &state, &visualization] {
        assert_eq!(message.payload()["version"], "3.0.0");
        assert_eq!(message.payload()["manufacturer"], "lab-demo");
        assert_eq!(message.payload()["serialNumber"], "demo-001");
    }
    assert_eq!(online.qos(), QosContract::AtLeastOnce);
    assert!(online.retain());
    for message in [&order, &state, &visualization] {
        assert_eq!(message.qos(), QosContract::AtMostOnce);
        assert!(!message.retain());
    }
}

#[test]
fn state_and_visualization_share_the_exact_simulator_snapshot() {
    let plan = LiveRunPlan::new("live-0001", LiveScenario::ReconnectMissingOnline).expect("plan");
    let robot = &plan.robots()[0];
    let snapshot = snapshot();
    let state = build_live_state(&plan, robot, 2, &snapshot, false);
    let visualization = build_live_visualization(&plan, robot, 3, 2, &snapshot);

    assert_eq!(
        state.payload()["mobileRobotPosition"],
        visualization.payload()["mobileRobotPosition"]
    );
    assert_eq!(state.payload()["velocity"]["vx"], 0.75);
    assert_eq!(visualization.payload()["velocity"]["vx"], 0.75);
    assert_eq!(visualization.payload()["referenceStateHeaderId"], 2);
}

#[test]
fn arrived_state_advances_the_last_node_and_clears_route_states() {
    let plan = LiveRunPlan::new("live-0001", LiveScenario::ReconnectMissingOnline).expect("plan");
    let robot = &plan.robots()[0];
    let mut arrived = snapshot();
    arrived.x = robot.released_end.x;
    arrived.linear_velocity = 0.0;
    arrived.motion_state = "ARRIVED".to_owned();
    let state = build_live_state(&plan, robot, 9, &arrived, false);

    assert_eq!(state.payload()["lastNodeId"], "B-demo-001");
    assert_eq!(state.payload()["lastNodeSequenceId"], 2);
    assert_eq!(state.payload()["driving"], false);
    assert_eq!(state.payload()["nodeStates"].as_array().unwrap().len(), 0);
    assert_eq!(state.payload()["edgeStates"].as_array().unwrap().len(), 0);
}
