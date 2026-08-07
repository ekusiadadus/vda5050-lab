use serde_json::json;
use sha2::{Digest, Sha256};
use vda5050_core::ActorRole;
use vda5050_demo::{LiveCapturedPublish, LiveRunPlan, LiveScenario, build_live_capture_artifacts};

#[test]
fn sealed_live_capture_binds_every_message_and_connection_epoch() {
    let plan = LiveRunPlan::new("live-0001", LiveScenario::ReconnectMissingOnline).expect("plan");
    let prefix = &plan.robots()[0].topic_prefix;
    let captures = vec![
        capture(
            format!("{prefix}/connection"),
            &json!({"timestamp":"2026-08-07T00:00:00.000Z","connectionState":"ONLINE"}),
            1,
        ),
        capture(
            format!("{prefix}/order"),
            &json!({"timestamp":"2026-08-07T00:00:00.500Z","orderId":"live-reconnect-001"}),
            2,
        ),
        capture(
            format!("{prefix}/connection"),
            &json!({"timestamp":"2026-08-07T00:00:03.000Z","connectionState":"CONNECTION_BROKEN"}),
            3,
        ),
        capture(
            format!("{prefix}/state"),
            &json!({"timestamp":"2026-08-07T00:00:03.500Z","orderId":"live-reconnect-001"}),
            4,
        ),
    ];

    let artifacts = build_live_capture_artifacts(&plan, &captures).expect("artifacts");
    let records = std::str::from_utf8(&artifacts.trace_bytes)
        .expect("UTF-8 trace")
        .lines()
        .map(|line| serde_json::from_str::<serde_json::Value>(line).expect("record"))
        .collect::<Vec<_>>();
    assert_eq!(records.first().unwrap()["record_type"], "CAPTURE_OPENED");
    assert_eq!(records.last().unwrap()["record_type"], "CAPTURE_CLOSED");
    assert_eq!(records.len(), captures.len() + 2);
    assert_eq!(artifacts.evidence.events.len(), captures.len());
    assert_eq!(
        artifacts.evidence.source_trace_sha256,
        hex::encode(Sha256::digest(&artifacts.trace_bytes))
    );
    assert_eq!(
        artifacts.evidence.events[&2].actor_role,
        ActorRole::FleetControl
    );
    assert_eq!(
        artifacts.evidence.events[&3].participant_connection_epoch,
        "session-1"
    );
    assert_eq!(
        artifacts.evidence.events[&4].participant_connection_epoch,
        "session-2"
    );
}

#[test]
fn live_capture_rejects_control_topics_and_topics_outside_the_plan() {
    let plan = LiveRunPlan::new("live-0001", LiveScenario::ReconnectMissingOnline).expect("plan");
    let control = vec![capture(
        plan.control_topic().to_owned(),
        &json!({"complete":true}),
        1,
    )];
    assert!(build_live_capture_artifacts(&plan, &control).is_err());

    let foreign = vec![capture(
        "vda5050/v3/lab-demo/foreign/state".to_owned(),
        &json!({"timestamp":"2026-08-07T00:00:00.000Z"}),
        1,
    )];
    assert!(build_live_capture_artifacts(&plan, &foreign).is_err());
}

fn capture(topic: String, payload: &serde_json::Value, sequence: u64) -> LiveCapturedPublish {
    LiveCapturedPublish::new(
        topic,
        serde_json::to_vec(payload).unwrap(),
        0,
        false,
        sequence * 1_000_000,
    )
}
