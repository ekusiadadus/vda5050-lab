use vda5050_core::{
    Applicability, Assertion, AssertionTrust, DiagnosticDomain, Evaluation, FindingSeverity,
    Inference, InferenceConfidence, InvestigationTarget, Observation, Recommendation,
    ReportValidity, Verdict,
};
use vda5050_report::{
    EvidenceReference, IncidentReport, RenderLimits, SpecificationReference, render_terminal,
    to_canonical_json, to_canonical_json_value,
};

fn report(summary: &str) -> IncidentReport {
    IncidentReport::new(
        "incident/repeated-order/001",
        ReportValidity::Partial,
        DiagnosticDomain::ProtocolConformance,
        Evaluation::new(Applicability::Unknown, Verdict::Unevaluated).unwrap(),
        FindingSeverity::Warning,
        summary,
    )
    .with_evidence_layers(
        vec![Observation::new(
            "obs-1",
            "two order messages were observed",
            ["event-1", "event-2"],
        )],
        vec![Assertion::new(
            "assert-1",
            "capture point is broker ingress",
            "capture manifest",
            AssertionTrust::Verified,
        )],
        vec![Inference::new(
            "infer-1",
            "the same update was resent",
            ["obs-1"],
            InferenceConfidence::Supported,
        )],
        vec![Recommendation::new(
            "rec-1",
            "capture the robot response",
            InvestigationTarget::MobileRobot,
            ["state topic at broker ingress"],
        )],
    )
    .with_specification_references(vec![SpecificationReference::new(
        "VDA5050-3.0.0",
        "6.1.4.5",
        "sha256:pdf-digest",
    )])
    .with_evidence_references(vec![EvidenceReference::new(
        "event-1",
        Some(17),
        "sha256:payload-digest",
        "order publication",
    )])
    .with_missing_evidence(["robot state after the second order"])
}

#[test]
fn canonical_json_is_byte_deterministic_and_has_sorted_object_keys() {
    let report = report("same input gives the same output");
    let first = to_canonical_json(&report).unwrap();
    let second = to_canonical_json(&report).unwrap();

    assert_eq!(first, second);
    assert!(first.starts_with("{\"assertions\":"));
    assert!(first.ends_with('}'));
    assert!(!first.ends_with("}\n"));

    let round_trip: IncidentReport = serde_json::from_str(&first).unwrap();
    assert_eq!(round_trip, report);
}

#[test]
fn arbitrary_serializable_wrappers_are_recursively_canonicalized() {
    let wrapper = serde_json::json!({
        "z": {"second": 2, "first": 1},
        "a": [{"right": false, "left": true}],
    });

    assert_eq!(
        to_canonical_json_value(&wrapper).unwrap(),
        r#"{"a":[{"left":true,"right":false}],"z":{"first":1,"second":2}}"#
    );
}

#[test]
fn canonical_report_contains_references_but_never_raw_payload_bytes() {
    let report = report("bounded evidence");
    let json = to_canonical_json(&report).unwrap();

    assert!(json.contains("payload_sha256"));
    assert!(json.contains("source_offset"));
    assert!(!json.contains("raw_payload"));
    assert!(!json.contains("payload_bytes"));

    assert_eq!(report.schema_version(), "vda5050-lab.report/1");
    assert_eq!(report.report_id(), "incident/repeated-order/001");
    assert_eq!(report.validity(), ReportValidity::Partial);
    assert_eq!(report.domain(), DiagnosticDomain::ProtocolConformance);
    assert_eq!(report.finding_severity(), FindingSeverity::Warning);
    assert_eq!(report.specification_references()[0].section(), "6.1.4.5");
    assert_eq!(report.evidence_references()[0].event_id(), "event-1");
    assert_eq!(report.evidence_references()[0].source_offset(), Some(17));
    assert_eq!(report.missing_evidence().len(), 1);
}

#[test]
fn terminal_rendering_strips_ansi_controls_and_bidi_overrides() {
    let hostile = "safe\u{1b}[31mRED\u{1b}[0m\0\u{0007}\u{202e}txt";
    let rendered = render_terminal(&report(hostile), RenderLimits::new(4_096, 512).unwrap());

    assert!(!rendered.text().contains('\u{1b}'));
    assert!(!rendered.text().contains("[31m"));
    assert!(!rendered.text().contains('\0'));
    assert!(!rendered.text().contains('\u{0007}'));
    assert!(!rendered.text().contains('\u{202e}'));
    assert!(rendered.text().contains("safeREDtxt"));
    assert!(
        rendered
            .text()
            .chars()
            .all(|character| { character == '\n' || character == '\t' || !character.is_control() })
    );
}

#[test]
fn terminal_rendering_is_bounded_and_records_each_truncation() {
    let long_summary = "warehouse-incident-".repeat(200);
    let limits = RenderLimits::new(640, 96).unwrap();
    let rendered = render_terminal(&report(&long_summary), limits);

    assert!(rendered.text().len() <= 640);
    assert!(!rendered.truncations().is_empty());
    for truncation in rendered.truncations() {
        assert!(!truncation.field().is_empty());
        assert_eq!(truncation.sha256().len(), 64);
        assert!(truncation.reference().starts_with("report://"));
        assert!(truncation.original_bytes() > truncation.rendered_bytes());
    }
    assert!(rendered.text().contains("truncated"));
    assert!(rendered.text().contains("sha256="));
    assert!(rendered.text().contains("ref=report://"));

    let text = rendered.into_text();
    assert!(text.len() <= limits.max_total_bytes());
    assert_eq!(limits.max_field_bytes(), 96);
}

#[test]
fn invalid_terminal_limits_fail_before_rendering() {
    assert!(RenderLimits::new(127, 64).is_err());
    assert!(RenderLimits::new(512, 0).is_err());
    assert!(RenderLimits::new(512, 513).is_err());
}
