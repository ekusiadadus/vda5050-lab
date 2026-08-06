use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use serde_json::Value;
use vda5050_core::{
    ActorRole, Applicability, DiagnosticDomain, Evaluation, FindingSeverity, InvestigationTarget,
    Verdict,
};
use vda5050_protocol::{ComparatorProfile, SemanticRelation, compare_orders};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum CaptureCompleteness {
    ConfirmedForRule,
    Partial,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TraceEvent {
    pub event_id: String,
    pub source_sequence: u64,
    pub topic: String,
    pub payload: Value,
    pub observed_monotonic_ns: Option<u64>,
    pub clock_epoch: String,
    pub actor_role: Option<ActorRole>,
    pub participant_id: Option<String>,
    pub participant_connection_epoch: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TraceContext {
    pub events: Vec<TraceEvent>,
    pub completeness: CaptureCompleteness,
    pub capture_closed: bool,
    pub role_attribution_complete: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AuthorityReference {
    pub kind: String,
    pub version: String,
    pub section: String,
    pub artifact_digest: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ProtocolErrorLevel {
    Error,
    Warning,
    None,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExpectedProtocolError {
    pub error_type: String,
    pub level: ProtocolErrorLevel,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Finding {
    pub rule_id: String,
    pub summary: String,
    pub diagnostic_domain: DiagnosticDomain,
    pub evaluation: Evaluation,
    pub finding_severity: FindingSeverity,
    pub normative_subject: ActorRole,
    pub investigation_target: InvestigationTarget,
    pub evidence_event_ids: Vec<String>,
    pub counterevidence_event_ids: Vec<String>,
    pub missing_evidence: Vec<String>,
    pub next_actions: Vec<String>,
    pub authority: Option<AuthorityReference>,
    pub expected_protocol_error: Option<ExpectedProtocolError>,
    #[serde(skip)]
    first_sequence: u64,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct DiagnosticReport {
    pub findings: Vec<Finding>,
}

struct FindingDraft<'a> {
    rule_id: &'a str,
    summary: &'a str,
    verdict: Verdict,
    severity: FindingSeverity,
    target: InvestigationTarget,
    evidence_event_ids: Vec<String>,
    missing_evidence: Vec<String>,
    next_actions: Vec<String>,
    first_sequence: u64,
}

struct AbsenceFindingDraft<'a> {
    rule_id: &'a str,
    summary: &'a str,
    proved_target: InvestigationTarget,
    event: &'a TraceEvent,
    missing: &'a str,
    next_action: &'a str,
    precondition_proven: bool,
    missing_precondition: &'a str,
}

#[must_use]
pub fn analyze(context: &TraceContext) -> DiagnosticReport {
    let mut events: Vec<&TraceEvent> = context.events.iter().collect();
    events.sort_by(|left, right| {
        left.source_sequence
            .cmp(&right.source_sequence)
            .then_with(|| left.event_id.cmp(&right.event_id))
    });

    let mut findings = Vec::new();
    analyze_repeated_orders(context, &events, &mut findings);
    analyze_graphs(context, &events, &mut findings);
    analyze_new_base_requests(context, &events, &mut findings);
    analyze_reconnect(context, &events, &mut findings);
    analyze_cancel_lifecycle(context, &events, &mut findings);

    findings.sort_by(|left, right| {
        left.first_sequence
            .cmp(&right.first_sequence)
            .then_with(|| left.rule_id.cmp(&right.rule_id))
            .then_with(|| left.evidence_event_ids.cmp(&right.evidence_event_ids))
    });

    DiagnosticReport { findings }
}

fn analyze_repeated_orders(
    context: &TraceContext,
    events: &[&TraceEvent],
    findings: &mut Vec<Finding>,
) {
    let mut previous_by_update: BTreeMap<(String, String), &TraceEvent> = BTreeMap::new();
    let mut previous_by_order: BTreeMap<String, &TraceEvent> = BTreeMap::new();
    let profile = ComparatorProfile::default_vda();

    for event in events
        .iter()
        .copied()
        .filter(|event| terminal_topic(&event.topic) == "order")
    {
        let Some(order_id) = scalar_key(event.payload.get("orderId")) else {
            continue;
        };
        let Some(update_id) = scalar_key(event.payload.get("orderUpdateId")) else {
            if let Some(previous) = previous_by_order.get(&order_id) {
                findings.push(unknown_applicability_finding(
                    "LAB-D1-REPEATED-ORDER-APPLICABILITY",
                    "A repeated orderId was observed, but orderUpdateId is missing or unsupported.",
                    vec![previous.event_id.clone(), event.event_id.clone()],
                    vec!["A parseable orderUpdateId on both order messages.".to_owned()],
                    vec!["Capture the original order payloads without field filtering.".to_owned()],
                    previous.source_sequence,
                ));
            }
            previous_by_order.insert(order_id, event);
            continue;
        };
        previous_by_order.insert(order_id.clone(), event);
        let key = (order_id, update_id);

        if let Some(previous) = previous_by_update.insert(key, event) {
            let relation = compare_orders(&previous.payload, &event.payload, &profile)
                .map_or(SemanticRelation::Unknown, |result| result.semantic_relation);
            match relation {
                SemanticRelation::Changed => {
                    let sender_is_proven = context.role_attribution_complete
                        && event.actor_role == Some(ActorRole::FleetControl);
                    findings.push(project_finding(FindingDraft {
                        rule_id: "LAB-D1-REPEATED-ORDER-CHANGED",
                        summary: "The same orderId and orderUpdateId were observed with changed semantic content.",
                        verdict: if sender_is_proven {
                            Verdict::Fail
                        } else {
                            Verdict::Inconclusive
                        },
                        severity: FindingSeverity::Error,
                        target: if sender_is_proven {
                            InvestigationTarget::FleetControl
                        } else {
                            InvestigationTarget::Unresolved
                        },
                        evidence_event_ids: vec![previous.event_id.clone(), event.event_id.clone()],
                        missing_evidence: if sender_is_proven {
                            Vec::new()
                        } else {
                            vec!["A trusted actor mapping proving which participant published the order.".to_owned()]
                        },
                        next_actions: vec![
                            "Compare the sender's update-ID allocation with the selected VDA version."
                                .to_owned(),
                            "Capture the mobile-robot state after the second order.".to_owned(),
                        ],
                        first_sequence: previous.source_sequence,
                    }));
                    evaluate_changed_robot_response(
                        context,
                        events,
                        previous,
                        event,
                        findings,
                    );
                }
                SemanticRelation::Unknown => findings.push(project_finding(FindingDraft {
                    rule_id: "LAB-D1-REPEATED-ORDER-UNKNOWN",
                    summary: "The same update was repeated, but free-text or extension semantics prevent a content decision.",
                    verdict: Verdict::Inconclusive,
                    severity: FindingSeverity::Warning,
                    target: InvestigationTarget::Unresolved,
                    evidence_event_ids: vec![previous.event_id.clone(), event.event_id.clone()],
                    missing_evidence: vec![
                        "A versioned comparator profile for the differing extension or free-text field."
                            .to_owned(),
                    ],
                    next_actions: vec![
                        "Ask both participants which differing fields affect logical order processing."
                            .to_owned(),
                    ],
                    first_sequence: previous.source_sequence,
                })),
                SemanticRelation::Equal => {}
            }
        }
    }
}

fn analyze_graphs(context: &TraceContext, events: &[&TraceEvent], findings: &mut Vec<Finding>) {
    for event in events
        .iter()
        .copied()
        .filter(|event| terminal_topic(&event.topic) == "order")
    {
        if graph_is_inconsistent(&event.payload) {
            let actor_is_proven = context.role_attribution_complete
                && event.actor_role == Some(ActorRole::FleetControl);
            findings.push(project_finding(FindingDraft {
                rule_id: "LAB-D2-GRAPH-STITCHING",
                summary: "The observed order graph does not form a contiguous node-edge-node sequence.",
                verdict: if actor_is_proven {
                    Verdict::Fail
                } else {
                    Verdict::Inconclusive
                },
                severity: FindingSeverity::Error,
                target: if actor_is_proven {
                    InvestigationTarget::FleetControl
                } else {
                    InvestigationTarget::Unresolved
                },
                evidence_event_ids: vec![event.event_id.clone()],
                missing_evidence: if actor_is_proven {
                    Vec::new()
                } else {
                    vec![
                        "A trusted actor mapping proving which participant published the order."
                            .to_owned(),
                    ]
                },
                next_actions: vec![
                    "Inspect node and edge sequence IDs at the base/horizon stitching boundary."
                        .to_owned(),
                ],
                first_sequence: event.source_sequence,
            }));
        }
    }
}

fn analyze_new_base_requests(
    context: &TraceContext,
    events: &[&TraceEvent],
    findings: &mut Vec<Finding>,
) {
    for request in events.iter().copied().filter(|event| {
        terminal_topic(&event.topic) == "state"
            && event.payload.get("newBaseRequest").and_then(Value::as_bool) == Some(true)
    }) {
        let request_order = request.payload.get("orderId");
        let response = events.iter().copied().any(|candidate| {
            candidate.source_sequence > request.source_sequence
                && terminal_topic(&candidate.topic) == "order"
                && identifiers_compatible(request_order, candidate.payload.get("orderId"))
        });
        if response {
            continue;
        }

        findings.push(absence_sensitive_finding(
            context,
            "LAB-D3-NEW-BASE-REQUEST",
            "newBaseRequest was observed without a later matching order update in this capture.",
            InvestigationTarget::FleetControl,
            request,
            "A capture vantage that can prove fleet-control order publication after newBaseRequest.",
            "Capture the order topic at broker ingress from the request until the response deadline.",
        ));
    }
}

fn analyze_reconnect(context: &TraceContext, events: &[&TraceEvent], findings: &mut Vec<Finding>) {
    for offline in events.iter().copied().filter(|event| {
        terminal_topic(&event.topic) == "connection"
            && event
                .payload
                .get("connectionState")
                .and_then(Value::as_str)
                .is_some_and(|state| state.eq_ignore_ascii_case("OFFLINE"))
    }) {
        let online = events.iter().copied().any(|candidate| {
            candidate.source_sequence > offline.source_sequence
                && terminal_topic(&candidate.topic) == "connection"
                && same_known_participant(offline, candidate)
                && candidate
                    .payload
                    .get("connectionState")
                    .and_then(Value::as_str)
                    .is_some_and(|state| state.eq_ignore_ascii_case("ONLINE"))
        });
        if online {
            continue;
        }

        let reconnect_proven = events.iter().copied().any(|candidate| {
            candidate.source_sequence > offline.source_sequence
                && context.role_attribution_complete
                && candidate.actor_role == Some(ActorRole::MobileRobot)
                && same_known_participant(offline, candidate)
                && different_known_connection_epoch(offline, candidate)
        });
        findings.push(gated_absence_sensitive_finding(
            context,
            &AbsenceFindingDraft {
                rule_id: "LAB-D4-RECONNECT-STATE",
                summary: "An OFFLINE connection state was not followed by ONLINE after a proved reconnect.",
                proved_target: InvestigationTarget::MobileRobot,
                event: offline,
                missing: "A rule-complete connection-topic capture covering the reconnect window.",
                next_action: "Capture connection publications and connection epochs at broker ingress across disconnect and reconnect.",
                precondition_proven: reconnect_proven,
                missing_precondition: "Independent evidence of a new participant connection epoch after OFFLINE.",
            },
        ));
    }
}

fn analyze_cancel_lifecycle(
    context: &TraceContext,
    events: &[&TraceEvent],
    findings: &mut Vec<Finding>,
) {
    for request in events.iter().copied().filter(|event| {
        terminal_topic(&event.topic) == "instantActions"
            && cancel_action_id(&event.payload).is_some()
    }) {
        let action_id = cancel_action_id(&request.payload).expect("filter established action ID");
        let terminal_state = events.iter().copied().any(|candidate| {
            candidate.source_sequence > request.source_sequence
                && terminal_topic(&candidate.topic) == "state"
                && has_terminal_action_state(&candidate.payload, &action_id)
        });
        if terminal_state {
            continue;
        }

        findings.push(absence_sensitive_finding(
            context,
            "LAB-D5-CANCEL-LIFECYCLE",
            "A cancelOrder action was observed without a terminal action lifecycle state.",
            InvestigationTarget::MobileRobot,
            request,
            "State action lifecycle observations after the cancelOrder action.",
            "Capture actionStates and order state until cancel completion or failure.",
        ));
    }
}

fn project_finding(draft: FindingDraft<'_>) -> Finding {
    Finding {
        rule_id: draft.rule_id.to_owned(),
        summary: draft.summary.to_owned(),
        diagnostic_domain: DiagnosticDomain::ProtocolConformance,
        evaluation: Evaluation::new(Applicability::Applicable, draft.verdict)
            .expect("finding constructors use legal states"),
        finding_severity: draft.severity,
        normative_subject: rule_metadata(draft.rule_id).0,
        investigation_target: draft.target,
        evidence_event_ids: draft.evidence_event_ids,
        counterevidence_event_ids: Vec::new(),
        missing_evidence: draft.missing_evidence,
        next_actions: draft.next_actions,
        authority: rule_metadata(draft.rule_id).1,
        expected_protocol_error: None,
        first_sequence: draft.first_sequence,
    }
}

fn absence_sensitive_finding(
    context: &TraceContext,
    rule_id: &str,
    summary: &str,
    proved_target: InvestigationTarget,
    event: &TraceEvent,
    missing: &str,
    next_action: &str,
) -> Finding {
    gated_absence_sensitive_finding(
        context,
        &AbsenceFindingDraft {
            rule_id,
            summary,
            proved_target,
            event,
            missing,
            next_action,
            precondition_proven: true,
            missing_precondition: "",
        },
    )
}

fn gated_absence_sensitive_finding(
    context: &TraceContext,
    draft: &AbsenceFindingDraft<'_>,
) -> Finding {
    let time_is_provable = draft.event.observed_monotonic_ns.is_some();
    let complete = context.capture_closed
        && context.completeness == CaptureCompleteness::ConfirmedForRule
        && context.role_attribution_complete;
    let decisive = complete && time_is_provable && draft.precondition_proven;
    let verdict = if decisive {
        Verdict::Fail
    } else {
        Verdict::Inconclusive
    };
    let target = if decisive {
        draft.proved_target
    } else {
        InvestigationTarget::Unresolved
    };

    project_finding(FindingDraft {
        rule_id: draft.rule_id,
        summary: draft.summary,
        verdict,
        severity: if decisive {
            FindingSeverity::Error
        } else {
            FindingSeverity::Warning
        },
        target,
        evidence_event_ids: vec![draft.event.event_id.clone()],
        missing_evidence: if decisive {
            Vec::new()
        } else if !time_is_provable {
            vec![
                "A capture-local monotonic timestamp and clock epoch for the observation window."
                    .to_owned(),
            ]
        } else if !draft.precondition_proven {
            vec![draft.missing_precondition.to_owned()]
        } else {
            vec![draft.missing.to_owned()]
        },
        next_actions: vec![draft.next_action.to_owned()],
        first_sequence: draft.event.source_sequence,
    })
}

fn unknown_applicability_finding(
    rule_id: &str,
    summary: &str,
    evidence_event_ids: Vec<String>,
    missing_evidence: Vec<String>,
    next_actions: Vec<String>,
    first_sequence: u64,
) -> Finding {
    Finding {
        rule_id: rule_id.to_owned(),
        summary: summary.to_owned(),
        diagnostic_domain: DiagnosticDomain::ProtocolConformance,
        evaluation: Evaluation::new(Applicability::Unknown, Verdict::Unevaluated)
            .expect("unknown applicability uses UNEVALUATED"),
        finding_severity: FindingSeverity::Warning,
        normative_subject: rule_metadata(rule_id).0,
        investigation_target: InvestigationTarget::Unresolved,
        evidence_event_ids,
        counterevidence_event_ids: Vec::new(),
        missing_evidence,
        next_actions,
        authority: rule_metadata(rule_id).1,
        expected_protocol_error: None,
        first_sequence,
    }
}

fn evaluate_changed_robot_response(
    context: &TraceContext,
    events: &[&TraceEvent],
    previous: &TraceEvent,
    repeated: &TraceEvent,
    findings: &mut Vec<Finding>,
) {
    if !current_order_is_proven(context, events, previous, repeated) {
        findings.push(unknown_applicability_finding(
            "VDA300-D1-MOBILE-ROBOT-CHANGED-RESPONSE",
            "Changed repeated content was observed, but the mobile robot's current order at receipt is not proven.",
            vec![previous.event_id.clone(), repeated.event_id.clone()],
            vec![
                "A trusted mobile-robot state before the repeat proving the same current orderId and orderUpdateId."
                    .to_owned(),
            ],
            vec![
                "Capture the state topic before and after retransmitting the changed order."
                    .to_owned(),
            ],
            previous.source_sequence,
        ));
        return;
    }

    let response = events.iter().copied().find(|candidate| {
        candidate.source_sequence > repeated.source_sequence
            && terminal_topic(&candidate.topic) == "state"
            && identifiers_compatible(
                repeated.payload.get("orderId"),
                candidate.payload.get("orderId"),
            )
    });
    let has_warning = response.is_some_and(|state| response_has_expected_warning(context, state));
    let time_is_provable = repeated.observed_monotonic_ns.is_some();
    let absence_is_provable = context.capture_closed
        && context.completeness == CaptureCompleteness::ConfirmedForRule
        && context.role_attribution_complete
        && time_is_provable;
    let (verdict, target, missing) = if has_warning {
        (Verdict::Pass, InvestigationTarget::Unresolved, Vec::new())
    } else if absence_is_provable {
        (Verdict::Fail, InvestigationTarget::MobileRobot, Vec::new())
    } else {
        (
            Verdict::Inconclusive,
            InvestigationTarget::Unresolved,
            vec![
                if time_is_provable {
                    "A rule-complete state capture after the changed repeated order."
                } else {
                    "A capture-local monotonic timestamp for the response window."
                }
                .to_owned(),
            ],
        )
    };
    let mut evidence = vec![previous.event_id.clone(), repeated.event_id.clone()];
    if let Some(state) = response {
        evidence.push(state.event_id.clone());
    }

    findings.push(Finding {
        rule_id: "VDA300-D1-MOBILE-ROBOT-CHANGED-RESPONSE".to_owned(),
        summary: if has_warning {
            "The mobile robot reported SAME_ORDER_UPDATE_ID at WARNING after changed repeated content."
                .to_owned()
        } else {
            "The required mobile-robot response to changed content under the same update ID could not be confirmed."
                .to_owned()
        },
        diagnostic_domain: DiagnosticDomain::ProtocolConformance,
        evaluation: Evaluation::new(Applicability::Applicable, verdict)
            .expect("response rule uses a legal evaluation"),
        finding_severity: if verdict == Verdict::Fail {
            FindingSeverity::Error
        } else {
            FindingSeverity::Warning
        },
        normative_subject: ActorRole::MobileRobot,
        investigation_target: target,
        evidence_event_ids: evidence,
        counterevidence_event_ids: Vec::new(),
        missing_evidence: missing,
        next_actions: if has_warning {
            Vec::new()
        } else {
            vec![
                "Capture the state topic until SAME_ORDER_UPDATE_ID is reported or a new order is accepted."
                    .to_owned(),
            ]
        },
        authority: Some(vda_pdf_authority("6.1.4.5")),
        expected_protocol_error: Some(ExpectedProtocolError {
            error_type: "SAME_ORDER_UPDATE_ID".to_owned(),
            level: ProtocolErrorLevel::Warning,
        }),
        first_sequence: previous.source_sequence,
    });
}

fn current_order_is_proven(
    context: &TraceContext,
    events: &[&TraceEvent],
    previous: &TraceEvent,
    repeated: &TraceEvent,
) -> bool {
    events.iter().copied().any(|candidate| {
        candidate.source_sequence < repeated.source_sequence
            && candidate.source_sequence >= previous.source_sequence
            && terminal_topic(&candidate.topic) == "state"
            && context.role_attribution_complete
            && candidate.actor_role == Some(ActorRole::MobileRobot)
            && same_known_participant(repeated, candidate)
            && identifiers_equal(
                repeated.payload.get("orderId"),
                candidate.payload.get("orderId"),
            )
            && identifiers_equal(
                repeated.payload.get("orderUpdateId"),
                candidate.payload.get("orderUpdateId"),
            )
    })
}

fn response_has_expected_warning(context: &TraceContext, state: &TraceEvent) -> bool {
    context.role_attribution_complete
        && state.actor_role == Some(ActorRole::MobileRobot)
        && state
            .payload
            .get("errors")
            .and_then(Value::as_array)
            .is_some_and(|errors| {
                errors.iter().any(|error| {
                    error.get("errorType").and_then(Value::as_str) == Some("SAME_ORDER_UPDATE_ID")
                        && error
                            .get("errorLevel")
                            .and_then(Value::as_str)
                            .is_some_and(|level| level.eq_ignore_ascii_case("WARNING"))
                })
            })
}

fn rule_metadata(rule_id: &str) -> (ActorRole, Option<AuthorityReference>) {
    match rule_id {
        "LAB-D2-GRAPH-STITCHING" => (ActorRole::FleetControl, Some(vda_pdf_authority("6.1.1"))),
        "LAB-D4-RECONNECT-STATE" => (ActorRole::MobileRobot, Some(vda_pdf_authority("6.5"))),
        "LAB-D5-CANCEL-LIFECYCLE" => (ActorRole::MobileRobot, Some(vda_pdf_authority("6.1.3"))),
        "VDA300-D1-MOBILE-ROBOT-CHANGED-RESPONSE" => {
            (ActorRole::MobileRobot, Some(vda_pdf_authority("6.1.4.5")))
        }
        "LAB-D1-REPEATED-ORDER-CHANGED"
        | "LAB-D1-REPEATED-ORDER-UNKNOWN"
        | "LAB-D1-REPEATED-ORDER-APPLICABILITY"
        | "LAB-D3-NEW-BASE-REQUEST" => (
            ActorRole::Unknown,
            Some(AuthorityReference {
                kind: "PROJECT_SPECIFICATION".to_owned(),
                version: "vda5050-lab.rule-contract/1".to_owned(),
                section: rule_id.to_owned(),
                artifact_digest: "UNRELEASED-RULE-CATALOG".to_owned(),
            }),
        ),
        _ => (ActorRole::Unknown, None),
    }
}

fn vda_pdf_authority(section: &str) -> AuthorityReference {
    AuthorityReference {
        kind: "VDA_PUBLISHED_PDF".to_owned(),
        version: "3.0.0".to_owned(),
        section: section.to_owned(),
        artifact_digest: "becf70e4c4a97db4058464d333ef9f7492a32007577dda70fdc433de045dab20"
            .to_owned(),
    }
}

fn graph_is_inconsistent(payload: &Value) -> bool {
    let (Some(nodes), Some(edges)) = (
        payload.get("nodes").and_then(Value::as_array),
        payload.get("edges").and_then(Value::as_array),
    ) else {
        return false;
    };

    if nodes.len() < 2 {
        return !edges.is_empty();
    }
    if edges.len() + 1 != nodes.len() {
        return true;
    }

    for (index, pair) in nodes.windows(2).enumerate() {
        let Some(start_sequence) = pair[0].get("sequenceId").and_then(Value::as_u64) else {
            return true;
        };
        let Some(end_sequence) = pair[1].get("sequenceId").and_then(Value::as_u64) else {
            return true;
        };
        let edge = &edges[index];
        let Some(edge_sequence) = edge.get("sequenceId").and_then(Value::as_u64) else {
            return true;
        };
        if end_sequence != start_sequence.saturating_add(2)
            || edge_sequence != start_sequence.saturating_add(1)
        {
            return true;
        }
        if let (Some(start_id), Some(edge_start)) = (
            pair[0].get("nodeId").and_then(Value::as_str),
            edge.get("startNodeId").and_then(Value::as_str),
        ) && start_id != edge_start
        {
            return true;
        }
        if let (Some(end_id), Some(edge_end)) = (
            pair[1].get("nodeId").and_then(Value::as_str),
            edge.get("endNodeId").and_then(Value::as_str),
        ) && end_id != edge_end
        {
            return true;
        }
    }
    false
}

fn cancel_action_id(payload: &Value) -> Option<String> {
    payload
        .get("actions")
        .or_else(|| payload.get("instantActions"))
        .and_then(Value::as_array)?
        .iter()
        .find(|action| {
            action
                .get("actionType")
                .and_then(Value::as_str)
                .is_some_and(|action_type| action_type == "cancelOrder")
        })
        .and_then(|action| action.get("actionId"))
        .and_then(Value::as_str)
        .map(str::to_owned)
}

fn has_terminal_action_state(payload: &Value, action_id: &str) -> bool {
    payload
        .get("actionStates")
        .and_then(Value::as_array)
        .is_some_and(|states| {
            states.iter().any(|state| {
                state.get("actionId").and_then(Value::as_str) == Some(action_id)
                    && state
                        .get("actionStatus")
                        .and_then(Value::as_str)
                        .is_some_and(|status| matches!(status, "FINISHED" | "FAILED"))
            })
        })
}

fn terminal_topic(topic: &str) -> &str {
    topic.rsplit('/').next().unwrap_or(topic)
}

fn identifiers_compatible(left: Option<&Value>, right: Option<&Value>) -> bool {
    match (left, right) {
        (Some(left), Some(right)) => left == right,
        _ => true,
    }
}

fn identifiers_equal(left: Option<&Value>, right: Option<&Value>) -> bool {
    matches!((left, right), (Some(left), Some(right)) if left == right)
}

fn same_known_participant(left: &TraceEvent, right: &TraceEvent) -> bool {
    matches!(
        (&left.participant_id, &right.participant_id),
        (Some(left), Some(right)) if left == right
    )
}

fn different_known_connection_epoch(left: &TraceEvent, right: &TraceEvent) -> bool {
    matches!(
        (
            &left.participant_connection_epoch,
            &right.participant_connection_epoch,
        ),
        (Some(left), Some(right)) if left != right
    )
}

fn scalar_key(value: Option<&Value>) -> Option<String> {
    match value? {
        Value::String(value) => Some(value.clone()),
        Value::Number(value) => Some(value.to_string()),
        _ => None,
    }
}
