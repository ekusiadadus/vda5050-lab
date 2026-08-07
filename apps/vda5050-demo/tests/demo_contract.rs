use serde_json::Value;
use vda5050_demo::{
    Actor, BrokerTarget, DemoScenario, Position, QosContract, render_ascii_frame,
    serialize_envelope_jsonl,
};

#[test]
fn broker_target_rejects_routable_hosts_without_isolation_proof() {
    assert!(BrokerTarget::new("broker.example.com", false).is_err());
    assert!(BrokerTarget::new("0.0.0.0", false).is_err());
    assert!(BrokerTarget::new("127.0.0.1", false).is_ok());
    assert!(BrokerTarget::new("::1", false).is_ok());
    assert!(BrokerTarget::new("broker", true).is_ok());
}

#[test]
fn changed_repeat_uses_real_vda_300_messages_and_distinct_actor_ids() {
    let scenario = DemoScenario::changed_repeated_order();
    assert_eq!(scenario.protocol_version(), "3.0.0");
    assert_ne!(
        scenario.client_id(Actor::FleetControl),
        scenario.client_id(Actor::MobileRobot)
    );
    assert_ne!(
        scenario.client_id(Actor::Recorder),
        scenario.client_id(Actor::MobileRobot)
    );

    let messages = scenario.scripted_messages();
    assert!(messages.len() <= scenario.message_budget());
    assert!(
        messages
            .iter()
            .all(|message| message.topic().starts_with("vda5050/v3/lab-demo/demo-001/"))
    );
    assert!(
        messages
            .iter()
            .all(|message| message.payload()["version"] == "3.0.0")
    );

    let orders: Vec<_> = messages
        .iter()
        .filter(|message| message.topic().ends_with("/order"))
        .collect();
    assert_eq!(orders.len(), 2);
    assert_eq!(
        orders[0].payload()["orderId"],
        orders[1].payload()["orderId"]
    );
    assert_eq!(
        orders[0].payload()["orderUpdateId"],
        orders[1].payload()["orderUpdateId"]
    );
    assert_ne!(orders[0].payload()["nodes"], orders[1].payload()["nodes"]);
}

#[test]
fn mqtt_contract_allows_retained_connection_only() {
    let scenario = DemoScenario::changed_repeated_order();
    for message in scenario.scripted_messages() {
        if message.topic().ends_with("/connection") {
            assert_eq!(message.qos(), QosContract::AtLeastOnce);
            assert!(message.retain());
        } else {
            assert_eq!(message.qos(), QosContract::AtMostOnce);
            assert!(!message.retain());
        }
    }
}

#[test]
fn captured_trace_is_an_explicit_envelope_jsonl_stream() {
    let scenario = DemoScenario::changed_repeated_order();
    let bytes = serialize_envelope_jsonl(&scenario.scripted_messages()).unwrap();
    let records: Vec<Value> = bytes
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect();
    assert_eq!(records.len(), scenario.scripted_messages().len());
    assert!(records.iter().all(|record| record["topic"].is_string()));
    assert!(records.iter().all(|record| record["timestamp"].is_string()));
    assert!(records.iter().all(|record| record["payload"].is_object()));
}

#[test]
fn ascii_frame_has_route_robot_and_incident_marker() {
    let frame = render_ascii_frame(Position { x: 4, y: 1 }, true);
    assert!(frame.contains('A'));
    assert!(frame.contains('B'));
    assert!(frame.contains('R'));
    assert!(frame.contains("SAME_ORDER_UPDATE_ID"));
}

#[test]
fn reconnect_incident_has_lwt_new_epoch_and_no_second_online() {
    let messages = DemoScenario::reconnect_missing_online().scripted_messages();
    let broken_index = messages
        .iter()
        .position(|message| message.payload()["connectionState"] == "CONNECTION_BROKEN")
        .expect("scenario must include the broker-published LWT");
    assert!(
        messages[..broken_index]
            .iter()
            .any(|message| message.payload()["connectionState"] == "ONLINE")
    );
    assert!(
        messages[broken_index + 1..]
            .iter()
            .all(|message| message.payload()["connectionState"] != "ONLINE")
    );
    assert!(
        messages[broken_index + 1..]
            .iter()
            .any(|message| message.connection_epoch() == "session-2")
    );
}
