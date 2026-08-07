use std::collections::BTreeSet;

use vda5050_demo::{
    LIVE_MESSAGE_LIMIT, LiveEvent, LivePhase, LiveRunPlan, LiveScenario, LiveSimSnapshot,
};

#[test]
fn live_plan_fixes_two_robots_identities_topics_rates_and_limits() {
    let plan =
        LiveRunPlan::new("live-0001", LiveScenario::ReconnectMissingOnline).expect("valid plan");

    assert_eq!(plan.schema(), "vda5050-lab.tier1-live-run/1");
    assert_eq!(plan.robots().len(), 2);
    assert_eq!(plan.fault_target(), "demo-001");
    assert_eq!(plan.tick_hz(), 20);
    assert_eq!(plan.visualization_hz(), 10);
    assert_eq!(plan.state_hz(), 2);
    assert_eq!(plan.hard_limits().messages, LIVE_MESSAGE_LIMIT);
    assert_eq!(plan.hard_limits().actors, 4);
    assert_eq!(plan.hard_limits().mqtt_connections, 4);
    assert!(plan.synthetic());
    assert!(plan.same_job_isolated());
    assert!(!plan.physical_dut_authorized());
    assert!(plan.omit_online_after_reconnect());

    let client_ids = plan.client_ids().values().collect::<BTreeSet<_>>();
    assert_eq!(client_ids.len(), 4);
    let topics = plan.topic_allowlist().iter().collect::<BTreeSet<_>>();
    assert_eq!(topics.len(), 8);
    assert_eq!(plan.retain_allowlist().len(), 2);
}

#[test]
fn control_plan_differs_only_in_the_fault_toggle() {
    let fault = LiveRunPlan::new("live-0001", LiveScenario::ReconnectMissingOnline).expect("fault");
    let control = LiveRunPlan::new("live-0001", LiveScenario::ReconnectControl).expect("control");

    assert!(fault.omit_online_after_reconnect());
    assert!(!control.omit_online_after_reconnect());
    assert_eq!(fault.robots(), control.robots());
    assert_eq!(fault.client_ids(), control.client_ids());
}

#[test]
fn live_event_schema_keeps_simulator_truth_explicitly_non_evidentiary() {
    let event = LiveEvent::sim_snapshot(
        7,
        LiveSimSnapshot {
            robot_id: "demo-001".to_owned(),
            simulation_ns: 350_000_000,
            x: 1.25,
            y: 0.0,
            theta: 0.0,
            linear_velocity: 0.8,
            angular_velocity: 0.0,
            motion_state: "DRIVING".to_owned(),
            transport_connected: false,
            connection_epoch: "session-1".to_owned(),
        },
    );
    let value = serde_json::to_value(event).expect("event JSON");

    assert_eq!(value["schema"], "vda5050-lab.live-event/1");
    assert_eq!(value["event_type"], "SIM_SNAPSHOT");
    assert_eq!(value["source"], "SIMULATOR_TRUTH");
    assert_eq!(value["evidence"], false);
    assert_eq!(value["source_sequence"], 7);
    assert_eq!(value["snapshot"]["transport_connected"], false);
}

#[test]
fn scenario_phase_is_a_bounded_explanatory_event() {
    let event = LiveEvent::phase(8, LivePhase::ReconnectedWithoutOnline);
    let value = serde_json::to_value(event).expect("event JSON");
    assert_eq!(value["event_type"], "SCENARIO_PHASE");
    assert_eq!(value["phase"], "RECONNECTED_WITHOUT_ONLINE");
    assert_eq!(value["evidence"], false);
}

#[test]
fn live_plan_rejects_unsafe_run_ids() {
    assert!(LiveRunPlan::new("", LiveScenario::ReconnectMissingOnline).is_err());
    assert!(LiveRunPlan::new("../../escape", LiveScenario::ReconnectMissingOnline).is_err());
    assert!(LiveRunPlan::new(&"x".repeat(33), LiveScenario::ReconnectMissingOnline).is_err());
}
