use serde_json::{Value, json};
use vda5050_core::{ActorRole, Applicability, CapturePoint, InvestigationTarget, Verdict};
use vda5050_doctor_engine::{
    CaptureCompleteness, ProtocolErrorLevel, TraceContext, TraceEvent, analyze,
};

fn event(id: &str, sequence: u64, topic: &str, payload: Value) -> TraceEvent {
    let actor_role = match topic.rsplit('/').next() {
        Some("order" | "instantActions") => Some(ActorRole::FleetControl),
        Some("state" | "connection") => Some(ActorRole::MobileRobot),
        _ => None,
    };
    TraceEvent {
        event_id: id.to_owned(),
        source_sequence: sequence,
        topic: topic.to_owned(),
        payload,
        capture_point: CapturePoint::PublisherAdapter,
        observed_monotonic_ns: Some(sequence * 1_000_000),
        clock_domain: Some("test-clock".to_owned()),
        clock_epoch: "epoch-1".to_owned(),
        actor_role,
        participant_id: Some("acme/r1".to_owned()),
        participant_connection_epoch: Some("session-1".to_owned()),
    }
}

fn passive_context(events: Vec<TraceEvent>) -> TraceContext {
    TraceContext {
        events,
        completeness: CaptureCompleteness::Unknown,
        capture_closed: true,
        role_attribution_complete: true,
    }
}

fn proved_context(events: Vec<TraceEvent>) -> TraceContext {
    TraceContext {
        events,
        completeness: CaptureCompleteness::ConfirmedForRule,
        capture_closed: true,
        role_attribution_complete: true,
    }
}

#[test]
fn changed_content_under_same_update_is_supported_with_both_evidence_events() {
    let first = event(
        "e1",
        1,
        "vda5050/v2/acme/r1/order",
        json!({
            "headerId": 10,
            "timestamp": "2026-08-06T00:00:00Z",
            "manufacturer": "acme",
            "serialNumber": "r1",
            "orderId": "o1",
            "orderUpdateId": 7,
            "nodes": [{"nodeId": "A", "sequenceId": 0, "released": true}]
        }),
    );
    let changed = event(
        "e2",
        2,
        "vda5050/v2/acme/r1/order",
        json!({
            "headerId": 11,
            "timestamp": "2026-08-06T00:00:01Z",
            "manufacturer": "acme",
            "serialNumber": "r1",
            "orderId": "o1",
            "orderUpdateId": 7,
            "nodes": [{"nodeId": "B", "sequenceId": 0, "released": true}]
        }),
    );

    let report = analyze(&passive_context(vec![first, changed]));
    let finding = report
        .findings
        .iter()
        .find(|finding| finding.rule_id == "LAB-D1-REPEATED-ORDER-CHANGED")
        .expect("changed repetition should be diagnosed");

    assert_eq!(
        finding.evaluation.applicability(),
        Applicability::Applicable
    );
    assert_eq!(finding.evaluation.verdict(), Verdict::Fail);
    assert_eq!(finding.evidence_event_ids, ["e1", "e2"]);
    assert_eq!(
        finding.investigation_target,
        InvestigationTarget::FleetControl
    );
    assert_eq!(
        finding
            .authority
            .as_ref()
            .map(|authority| authority.kind.as_str()),
        Some("PROJECT_SPECIFICATION"),
        "sender hygiene must not masquerade as a VDA obligation"
    );
}

#[test]
fn unknown_extension_difference_does_not_become_changed_content() {
    let first = event(
        "e1",
        1,
        "vda5050/v2/acme/r1/order",
        json!({
            "manufacturer": "acme",
            "serialNumber": "r1",
            "orderId": "o1",
            "orderUpdateId": 7,
            "nodes": [],
            "vendorExtension": {"mode": "a"}
        }),
    );
    let second = event(
        "e2",
        2,
        "vda5050/v2/acme/r1/order",
        json!({
            "manufacturer": "acme",
            "serialNumber": "r1",
            "orderId": "o1",
            "orderUpdateId": 7,
            "nodes": [],
            "vendorExtension": {"mode": "b"}
        }),
    );

    let report = analyze(&passive_context(vec![first, second]));
    let finding = report
        .findings
        .iter()
        .find(|finding| finding.rule_id == "LAB-D1-REPEATED-ORDER-UNKNOWN")
        .expect("unknown extension semantics require abstention");

    assert_eq!(
        finding.evaluation.applicability(),
        Applicability::Applicable
    );
    assert_eq!(finding.evaluation.verdict(), Verdict::Inconclusive);
    assert_eq!(
        finding.investigation_target,
        InvestigationTarget::Unresolved
    );
}

#[test]
fn invalid_node_edge_stitching_is_reported_from_one_order() {
    let order = event(
        "order-1",
        1,
        "vda5050/v3/acme/r1/order",
        json!({
            "orderId": "o2",
            "orderUpdateId": 1,
            "nodes": [
                {"nodeId": "A", "sequenceId": 0, "released": true},
                {"nodeId": "B", "sequenceId": 4, "released": true}
            ],
            "edges": [
                {
                    "edgeId": "A-B",
                    "sequenceId": 1,
                    "released": true,
                    "startNodeId": "A",
                    "endNodeId": "B"
                }
            ]
        }),
    );

    let report = analyze(&passive_context(vec![order]));
    let finding = report
        .findings
        .iter()
        .find(|finding| finding.rule_id == "LAB-D2-GRAPH-STITCHING")
        .expect("broken sequence must be reported");

    assert_eq!(finding.evaluation.verdict(), Verdict::Fail);
    assert_eq!(finding.evidence_event_ids, ["order-1"]);
    assert_eq!(
        finding.investigation_target,
        InvestigationTarget::FleetControl
    );
}

#[test]
fn missing_response_to_new_base_request_abstains_for_passive_capture() {
    let state = event(
        "state-1",
        1,
        "vda5050/v2/acme/r1/state",
        json!({
            "orderId": "o3",
            "orderUpdateId": 2,
            "newBaseRequest": true
        }),
    );

    let report = analyze(&passive_context(vec![state]));
    let finding = report
        .findings
        .iter()
        .find(|finding| finding.rule_id == "LAB-D3-NEW-BASE-REQUEST")
        .expect("request must produce a missing-evidence diagnosis");

    assert_eq!(finding.evaluation.verdict(), Verdict::Inconclusive);
    assert_eq!(
        finding.investigation_target,
        InvestigationTarget::Unresolved
    );
    assert!(
        finding
            .missing_evidence
            .iter()
            .any(|item| item.contains("order"))
    );
}

#[test]
fn offline_without_proved_reconnect_never_becomes_a_failure() {
    let offline = event(
        "connection-1",
        1,
        "vda5050/v2/acme/r1/connection",
        json!({"connectionState": "OFFLINE"}),
    );
    let other = event(
        "state-1",
        2,
        "vda5050/v2/acme/r1/state",
        json!({"orderId": "o4", "orderUpdateId": 1}),
    );

    let passive = analyze(&passive_context(vec![offline.clone(), other.clone()]));
    let passive_finding = passive
        .findings
        .iter()
        .find(|finding| finding.rule_id == "LAB-D4-RECONNECT-STATE")
        .expect("passive capture still explains missing evidence");
    assert_eq!(passive_finding.evaluation.verdict(), Verdict::Inconclusive);

    let proved = analyze(&proved_context(vec![offline, other]));
    let proved_finding = proved
        .findings
        .iter()
        .find(|finding| finding.rule_id == "LAB-D4-RECONNECT-STATE")
        .expect("complete capture still explains the missing reconnect evidence");
    assert_eq!(proved_finding.evaluation.verdict(), Verdict::Inconclusive);
    assert_eq!(
        proved_finding.investigation_target,
        InvestigationTarget::Unresolved
    );
}

#[test]
fn proved_new_connection_epoch_without_online_is_a_failure() {
    let offline = event(
        "connection-1",
        1,
        "vda5050/v3/acme/r1/connection",
        json!({"connectionState": "OFFLINE"}),
    );
    let mut after_reconnect = event(
        "state-1",
        2,
        "vda5050/v3/acme/r1/state",
        json!({"orderId": "o4", "orderUpdateId": 1}),
    );
    after_reconnect.participant_connection_epoch = Some("session-2".to_owned());

    let report = analyze(&proved_context(vec![offline, after_reconnect]));
    let finding = report
        .findings
        .iter()
        .find(|finding| finding.rule_id == "LAB-D4-RECONNECT-STATE")
        .expect("a proved reconnect without ONLINE violates the publication obligation");

    assert_eq!(finding.evaluation.verdict(), Verdict::Fail);
    assert_eq!(
        finding.investigation_target,
        InvestigationTarget::MobileRobot
    );
}

#[test]
fn connection_broken_then_new_epoch_without_online_is_a_failure() {
    let broken = event(
        "connection-1",
        1,
        "vda5050/v3/acme/r1/connection",
        json!({"connectionState": "CONNECTION_BROKEN"}),
    );
    let mut after_reconnect = event(
        "state-1",
        2,
        "vda5050/v3/acme/r1/state",
        json!({"orderId": "o4", "orderUpdateId": 1}),
    );
    after_reconnect.participant_connection_epoch = Some("session-2".to_owned());

    let report = analyze(&proved_context(vec![broken, after_reconnect]));
    let finding = report
        .findings
        .iter()
        .find(|finding| finding.rule_id == "LAB-D4-RECONNECT-STATE")
        .expect("a proved reconnect after LWT without ONLINE violates the publication obligation");

    assert_eq!(finding.evaluation.verdict(), Verdict::Fail);
    assert_eq!(
        finding.investigation_target,
        InvestigationTarget::MobileRobot
    );
}

#[test]
fn cancel_without_action_lifecycle_is_inconclusive_without_absence_proof() {
    let cancel = event(
        "instant-1",
        1,
        "vda5050/v2/acme/r1/instantActions",
        json!({
            "actions": [{
                "actionId": "cancel-1",
                "actionType": "cancelOrder"
            }]
        }),
    );

    let report = analyze(&passive_context(vec![cancel]));
    let finding = report
        .findings
        .iter()
        .find(|finding| finding.rule_id == "LAB-D5-CANCEL-LIFECYCLE")
        .expect("cancel must produce a lifecycle diagnosis");

    assert_eq!(finding.evaluation.verdict(), Verdict::Inconclusive);
    assert!(
        finding
            .missing_evidence
            .iter()
            .any(|item| item.contains("action"))
    );
}

#[test]
fn cancel_terminal_state_is_read_from_vda_300_instant_action_states() {
    let cancel = event(
        "instant-1",
        1,
        "vda5050/v3/acme/r1/instantActions",
        json!({
            "actions": [{
                "actionId": "cancel-1",
                "actionType": "cancelOrder"
            }]
        }),
    );
    let finished = event(
        "state-1",
        2,
        "vda5050/v3/acme/r1/state",
        json!({
            "instantActionStates": [{
                "actionId": "cancel-1",
                "actionStatus": "FINISHED"
            }]
        }),
    );

    let report = analyze(&proved_context(vec![cancel, finished]));
    assert!(
        report
            .findings
            .iter()
            .all(|finding| finding.rule_id != "LAB-D5-CANCEL-LIFECYCLE")
    );
}

#[test]
fn findings_are_deterministically_sorted_by_sequence_then_rule() {
    let reconnect = event(
        "connection-1",
        3,
        "vda5050/v2/acme/r1/connection",
        json!({"connectionState": "OFFLINE"}),
    );
    let request = event(
        "state-1",
        1,
        "vda5050/v2/acme/r1/state",
        json!({"newBaseRequest": true}),
    );

    let report = analyze(&passive_context(vec![reconnect, request]));
    let ids: Vec<_> = report
        .findings
        .iter()
        .map(|finding| finding.rule_id.as_str())
        .collect();

    assert_eq!(
        ids,
        vec!["LAB-D3-NEW-BASE-REQUEST", "LAB-D4-RECONNECT-STATE"]
    );
}

#[test]
fn repeated_order_precondition_with_missing_update_id_is_unknown_not_ignored() {
    let first = event(
        "e1",
        1,
        "vda5050/v3/acme/r1/order",
        json!({"orderId": "o1", "orderUpdateId": 1, "nodes": [], "edges": []}),
    );
    let second = event(
        "e2",
        2,
        "vda5050/v3/acme/r1/order",
        json!({"orderId": "o1", "nodes": [], "edges": []}),
    );

    let report = analyze(&passive_context(vec![first, second]));
    let finding = report
        .findings
        .iter()
        .find(|finding| finding.rule_id == "LAB-D1-REPEATED-ORDER-APPLICABILITY")
        .expect("unknown precondition must remain visible");

    assert_eq!(finding.evaluation.applicability(), Applicability::Unknown);
    assert_eq!(finding.evaluation.verdict(), Verdict::Unevaluated);
    assert_eq!(
        finding.investigation_target,
        InvestigationTarget::Unresolved
    );
}

#[test]
fn complete_capture_without_monotonic_time_cannot_fail_a_deadline_rule() {
    let mut offline = event(
        "connection-1",
        1,
        "vda5050/v3/acme/r1/connection",
        json!({"connectionState": "OFFLINE"}),
    );
    offline.observed_monotonic_ns = None;

    let report = analyze(&proved_context(vec![offline]));
    let finding = report
        .findings
        .iter()
        .find(|finding| finding.rule_id == "LAB-D4-RECONNECT-STATE")
        .expect("missing clock evidence must be explained");

    assert_eq!(finding.evaluation.verdict(), Verdict::Inconclusive);
    assert!(
        finding
            .missing_evidence
            .iter()
            .any(|item| item.contains("monotonic"))
    );
}

#[test]
fn observed_online_transition_satisfies_reconnect_diagnosis() {
    let offline = event(
        "connection-1",
        1,
        "vda5050/v3/acme/r1/connection",
        json!({"connectionState": "OFFLINE"}),
    );
    let online = event(
        "connection-2",
        2,
        "vda5050/v3/acme/r1/connection",
        json!({"connectionState": "ONLINE"}),
    );

    let report = analyze(&proved_context(vec![offline, online]));
    assert!(
        report
            .findings
            .iter()
            .all(|finding| finding.rule_id != "LAB-D4-RECONNECT-STATE")
    );
}

#[test]
fn terminal_cancel_action_state_satisfies_lifecycle_diagnosis() {
    let cancel = event(
        "instant-1",
        1,
        "vda5050/v3/acme/r1/instantActions",
        json!({
            "actions": [{"actionId": "cancel-1", "actionType": "cancelOrder"}]
        }),
    );
    let finished = event(
        "state-1",
        2,
        "vda5050/v3/acme/r1/state",
        json!({
            "actionStates": [{"actionId": "cancel-1", "actionStatus": "FINISHED"}]
        }),
    );

    let report = analyze(&proved_context(vec![cancel, finished]));
    assert!(
        report
            .findings
            .iter()
            .all(|finding| finding.rule_id != "LAB-D5-CANCEL-LIFECYCLE")
    );
}

#[test]
fn changed_repeat_with_required_warning_passes_mobile_robot_response_rule() {
    let first = event(
        "order-1",
        1,
        "vda5050/v3/acme/r1/order",
        json!({
            "headerId": 1,
            "timestamp": "2026-08-06T00:00:00Z",
            "manufacturer": "acme",
            "serialNumber": "r1",
            "orderId": "o1",
            "orderUpdateId": 4,
            "nodes": [{"nodeId":"A","sequenceId":0,"released":true}],
            "edges": []
        }),
    );
    let current_state = event(
        "state-current",
        2,
        "vda5050/v3/acme/r1/state",
        json!({
            "orderId": "o1",
            "orderUpdateId": 4
        }),
    );
    let changed = event(
        "order-2",
        3,
        "vda5050/v3/acme/r1/order",
        json!({
            "headerId": 2,
            "timestamp": "2026-08-06T00:00:01Z",
            "manufacturer": "acme",
            "serialNumber": "r1",
            "orderId": "o1",
            "orderUpdateId": 4,
            "nodes": [{"nodeId":"B","sequenceId":0,"released":true}],
            "edges": []
        }),
    );
    let state = event(
        "state-1",
        4,
        "vda5050/v3/acme/r1/state",
        json!({
            "orderId": "o1",
            "orderUpdateId": 4,
            "errors": [{
                "errorType": "SAME_ORDER_UPDATE_ID",
                "errorLevel": "WARNING"
            }]
        }),
    );

    let report = analyze(&proved_context(vec![first, current_state, changed, state]));
    let finding = report
        .findings
        .iter()
        .find(|finding| finding.rule_id == "VDA300-D1-MOBILE-ROBOT-CHANGED-RESPONSE")
        .expect("mobile robot obligation must be evaluated separately");

    assert_eq!(finding.evaluation.verdict(), Verdict::Pass);
    assert_eq!(finding.normative_subject, ActorRole::MobileRobot);
    assert_eq!(
        finding
            .authority
            .as_ref()
            .map(|authority| authority.section.as_str()),
        Some("6.1.4.5")
    );
    assert_eq!(
        finding
            .expected_protocol_error
            .as_ref()
            .map(|expectation| expectation.level),
        Some(ProtocolErrorLevel::Warning)
    );
}

#[test]
fn changed_repeat_without_current_order_evidence_leaves_response_rule_unevaluated() {
    let first = event(
        "order-1",
        1,
        "vda5050/v3/acme/r1/order",
        json!({
            "manufacturer":"acme","serialNumber":"r1",
            "orderId":"o1","orderUpdateId":4,
            "nodes":[{"nodeId":"A","sequenceId":0,"released":true}],"edges":[]
        }),
    );
    let changed = event(
        "order-2",
        2,
        "vda5050/v3/acme/r1/order",
        json!({
            "manufacturer":"acme","serialNumber":"r1",
            "orderId":"o1","orderUpdateId":4,
            "nodes":[{"nodeId":"B","sequenceId":0,"released":true}],"edges":[]
        }),
    );

    let report = analyze(&passive_context(vec![first, changed]));
    let finding = report
        .findings
        .iter()
        .find(|finding| finding.rule_id == "VDA300-D1-MOBILE-ROBOT-CHANGED-RESPONSE")
        .expect("missing response evidence must remain visible");

    assert_eq!(finding.evaluation.applicability(), Applicability::Unknown);
    assert_eq!(finding.evaluation.verdict(), Verdict::Unevaluated);
    assert_eq!(
        finding.investigation_target,
        InvestigationTarget::Unresolved
    );
    assert!(
        finding
            .missing_evidence
            .iter()
            .any(|evidence| evidence.contains("state"))
    );
}

#[test]
fn unproved_actor_cannot_receive_role_specific_graph_failure() {
    let mut order = event(
        "order-1",
        1,
        "custom/cloud/order",
        json!({
            "orderId":"o1","orderUpdateId":1,
            "nodes":[
                {"nodeId":"A","sequenceId":0,"released":true},
                {"nodeId":"B","sequenceId":4,"released":true}
            ],
            "edges":[
                {"edgeId":"A-B","sequenceId":1,"startNodeId":"A","endNodeId":"B","released":true}
            ]
        }),
    );
    order.actor_role = None;
    let mut context = proved_context(vec![order]);
    context.role_attribution_complete = false;

    let report = analyze(&context);
    let finding = report
        .findings
        .iter()
        .find(|finding| finding.rule_id == "LAB-D2-GRAPH-STITCHING")
        .expect("structural evidence remains useful");

    assert_eq!(finding.evaluation.verdict(), Verdict::Inconclusive);
    assert_eq!(
        finding.investigation_target,
        InvestigationTarget::Unresolved
    );
    assert_eq!(finding.normative_subject, ActorRole::FleetControl);
    assert!(
        finding
            .missing_evidence
            .iter()
            .any(|evidence| evidence.contains("actor"))
    );
}
