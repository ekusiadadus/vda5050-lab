use vda5050_core::{
    Applicability, Assertion, AssertionTrust, DiagnosticDomain, Evaluation, EvaluationError,
    Inference, InferenceConfidence, InvestigationTarget, Observation, Recommendation,
    ReportValidity, Verdict,
};

#[test]
fn evaluation_accepts_exactly_the_documented_legal_states() {
    let legal = [
        (Applicability::Applicable, Verdict::Pass),
        (Applicability::Applicable, Verdict::Fail),
        (Applicability::Applicable, Verdict::Inconclusive),
        (Applicability::NotApplicable, Verdict::Unevaluated),
        (Applicability::Unknown, Verdict::Unevaluated),
    ];

    for (applicability, verdict) in legal {
        assert!(Evaluation::new(applicability, verdict).is_ok());
    }

    for applicability in [
        Applicability::Applicable,
        Applicability::NotApplicable,
        Applicability::Unknown,
    ] {
        for verdict in [
            Verdict::Pass,
            Verdict::Fail,
            Verdict::Inconclusive,
            Verdict::Unevaluated,
        ] {
            let expected = legal.contains(&(applicability, verdict));
            assert_eq!(Evaluation::new(applicability, verdict).is_ok(), expected);
        }
    }
}

#[test]
fn evaluation_rejects_invalid_json_states() {
    let error =
        serde_json::from_str::<Evaluation>(r#"{"applicability":"UNKNOWN","verdict":"PASS"}"#)
            .expect_err("deserialization must preserve the invariant");

    assert!(error.to_string().contains("illegal evaluation state"));
    assert_eq!(
        Evaluation::new(Applicability::Unknown, Verdict::Pass),
        Err(EvaluationError::IllegalCombination {
            applicability: Applicability::Unknown,
            verdict: Verdict::Pass,
        })
    );
}

#[test]
fn evidence_categories_remain_distinct_in_json() {
    let observation = Observation::new("obs-1", "state message was delivered", ["event-7"]);
    let assertion = Assertion::new(
        "assert-1",
        "publisher role is ROBOT",
        "signed adapter manifest",
        AssertionTrust::Verified,
    );
    let inference = Inference::new(
        "infer-1",
        "state belongs to the robot",
        ["obs-1", "assert-1"],
        InferenceConfidence::Supported,
    );
    let recommendation = Recommendation::new(
        "rec-1",
        "capture the connection topic",
        InvestigationTarget::Transport,
        ["connection topic at broker ingress"],
    );

    let encoded = serde_json::json!({
        "observations": [observation],
        "assertions": [assertion],
        "inferences": [inference],
        "recommendations": [recommendation],
    });

    assert_eq!(encoded["observations"][0]["event_ids"][0], "event-7");
    assert_eq!(encoded["assertions"][0]["trust"], "VERIFIED");
    assert_eq!(encoded["inferences"][0]["basis_ids"][1], "assert-1");
    assert_eq!(encoded["recommendations"][0]["target"], "TRANSPORT");
    assert_ne!(
        serde_json::to_value(&encoded["observations"][0]).unwrap(),
        serde_json::to_value(&encoded["inferences"][0]).unwrap()
    );

    let observation = Observation::new("obs", "fact", ["event"]);
    assert_eq!(observation.id(), "obs");
    assert_eq!(observation.description(), "fact");
    assert_eq!(observation.event_ids(), ["event"]);

    let assertion = Assertion::new("assert", "value", "source", AssertionTrust::Declared);
    assert_eq!(assertion.id(), "assert");
    assert_eq!(assertion.value(), "value");
    assert_eq!(assertion.source(), "source");
    assert_eq!(assertion.trust(), AssertionTrust::Declared);

    let inference = Inference::new(
        "infer",
        "conclusion",
        ["obs"],
        InferenceConfidence::Tentative,
    );
    assert_eq!(inference.id(), "infer");
    assert_eq!(inference.conclusion(), "conclusion");
    assert_eq!(inference.basis_ids(), ["obs"]);
    assert_eq!(inference.confidence(), InferenceConfidence::Tentative);

    let recommendation = Recommendation::new(
        "rec",
        "inspect",
        InvestigationTarget::Unresolved,
        ["more evidence"],
    );
    assert_eq!(recommendation.id(), "rec");
    assert_eq!(recommendation.action(), "inspect");
    assert_eq!(recommendation.target(), InvestigationTarget::Unresolved);
    assert_eq!(recommendation.evidence_needed(), ["more evidence"]);
}

#[test]
fn public_taxonomies_have_stable_wire_names() {
    assert_eq!(
        serde_json::to_string(&ReportValidity::Partial).unwrap(),
        "\"PARTIAL\""
    );
    assert_eq!(
        serde_json::to_string(&DiagnosticDomain::ProtocolConformance).unwrap(),
        "\"PROTOCOL_CONFORMANCE\""
    );
    assert_eq!(
        serde_json::to_string(&InvestigationTarget::FleetControl).unwrap(),
        "\"FLEET_CONTROL\""
    );
    assert_eq!(
        serde_json::to_string(&InvestigationTarget::MobileRobot).unwrap(),
        "\"MOBILE_ROBOT\""
    );
}
