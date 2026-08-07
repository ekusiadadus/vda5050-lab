use std::collections::BTreeMap;

use serde_json::json;
use tempfile::tempdir;
use vda5050_core::{
    ActorRole, CaptureClosed, CaptureOpened, CapturePoint, CaptureRecord, ClockDescriptor,
    ClockWrapPolicy, MessageObserved, PayloadReference,
};
use vda5050_doctor::{
    EventEvidence, SyntheticEvidenceManifest, trace_context_with_synthetic_evidence,
};
use vda5050_doctor_engine::CaptureCompleteness;
use vda5050_import::{ImportConfig, ImportFormat, ImportSummary, import_path};

#[test]
#[allow(
    clippy::too_many_lines,
    reason = "the test keeps the complete canonical evidence lifecycle visible in one contract"
)]
fn canonical_observation_and_explicit_synthetic_assertions_reach_trace_context() {
    let dir = tempdir().unwrap();
    let trace_path = dir.path().join("trace.jsonl");
    let opened: CaptureRecord = CaptureOpened::new(
        "demo-capture",
        "2026-08-07T00:00:00Z",
        Some(ClockDescriptor::new(
            "broker-clock",
            "scenario start",
            1,
            ClockWrapPolicy::DoesNotWrap,
            "clock-epoch-1",
            Some(1_000_000),
        )),
        true,
    )
    .into();
    let broken: CaptureRecord = MessageObserved::new(
        "broken",
        "demo-capture",
        CapturePoint::BrokerIngress,
        Some(1),
        "vda5050/v3/lab-demo/demo-001/connection",
        PayloadReference::inline(
            serde_json::to_vec(&json!({
                "headerId": 1,
                "timestamp": "2026-08-07T00:00:01Z",
                "version": "3.0.0",
                "manufacturer": "lab-demo",
                "serialNumber": "demo-001",
                "connectionState": "CONNECTION_BROKEN"
            }))
            .unwrap(),
        ),
    )
    .with_time(
        Some("2026-08-07T00:00:01Z".to_owned()),
        Some(1_000_000_000),
        Some("broker-clock".to_owned()),
        Some("clock-epoch-1".to_owned()),
    )
    .into();
    let state: CaptureRecord = MessageObserved::new(
        "state-after-reconnect",
        "demo-capture",
        CapturePoint::BrokerIngress,
        Some(2),
        "vda5050/v3/lab-demo/demo-001/state",
        PayloadReference::inline(
            serde_json::to_vec(&json!({
                "headerId": 2,
                "timestamp": "2026-08-07T00:00:02Z",
                "version": "3.0.0",
                "manufacturer": "lab-demo",
                "serialNumber": "demo-001",
                "orderId": "demo-order",
                "orderUpdateId": 0
            }))
            .unwrap(),
        ),
    )
    .with_time(
        Some("2026-08-07T00:00:02Z".to_owned()),
        Some(2_000_000_000),
        Some("broker-clock".to_owned()),
        Some("clock-epoch-1".to_owned()),
    )
    .into();
    let closed: CaptureRecord = CaptureClosed::new(
        "demo-capture",
        "2026-08-07T00:00:03Z",
        4,
        "0".repeat(64),
        true,
    )
    .into();
    let bytes = [opened, broken, state, closed]
        .into_iter()
        .map(|record| serde_json::to_string(&record).unwrap())
        .collect::<Vec<_>>()
        .join("\n");
    std::fs::write(&trace_path, format!("{bytes}\n")).unwrap();

    let imported = import_path(
        &trace_path,
        ImportFormat::CanonicalJsonl,
        ImportConfig::default(),
    )
    .unwrap();
    let message_indexes: Vec<_> = imported
        .records
        .iter()
        .filter(|record| record.to_core_message("ignored").is_some())
        .map(|record| record.location.record_index)
        .collect();
    assert_eq!(message_indexes, vec![1, 2]);

    let manifest = SyntheticEvidenceManifest {
        schema: "vda5050-lab.synthetic-evidence/1".to_owned(),
        source_trace_sha256: imported.source_digest.clone(),
        synthetic: true,
        same_job_isolated: true,
        capture_closed: true,
        completeness: CaptureCompleteness::ConfirmedForRule,
        role_attribution_complete: true,
        events: BTreeMap::from([
            (
                1,
                EventEvidence {
                    actor_role: ActorRole::MobileRobot,
                    participant_id: "lab-demo/demo-001".to_owned(),
                    participant_connection_epoch: "session-1".to_owned(),
                },
            ),
            (
                2,
                EventEvidence {
                    actor_role: ActorRole::MobileRobot,
                    participant_id: "lab-demo/demo-001".to_owned(),
                    participant_connection_epoch: "session-2".to_owned(),
                },
            ),
        ]),
    };

    let context = trace_context_with_synthetic_evidence(&imported, Some(&manifest)).unwrap();
    assert!(context.capture_closed);
    assert_eq!(context.completeness, CaptureCompleteness::ConfirmedForRule);
    assert!(context.role_attribution_complete);
    assert_eq!(context.events[0].observed_monotonic_ns, Some(1_000_000_000));
    assert_eq!(context.events[0].clock_epoch, "clock-epoch-1");
    assert_eq!(context.events[0].actor_role, Some(ActorRole::MobileRobot));
    assert_eq!(
        context.events[1].participant_connection_epoch.as_deref(),
        Some("session-2")
    );
}

#[test]
fn evidence_manifest_must_match_trace_and_be_explicitly_synthetic_and_isolated() {
    let report = vda5050_import::ImportReport {
        source_digest: "a".repeat(64),
        records: Vec::new(),
        summary: ImportSummary::default(),
    };
    let mut manifest = SyntheticEvidenceManifest {
        schema: "vda5050-lab.synthetic-evidence/1".to_owned(),
        source_trace_sha256: "b".repeat(64),
        synthetic: true,
        same_job_isolated: true,
        capture_closed: true,
        completeness: CaptureCompleteness::ConfirmedForRule,
        role_attribution_complete: true,
        events: BTreeMap::new(),
    };
    assert!(trace_context_with_synthetic_evidence(&report, Some(&manifest)).is_err());
    manifest.source_trace_sha256 = report.source_digest.clone();
    manifest.same_job_isolated = false;
    assert!(trace_context_with_synthetic_evidence(&report, Some(&manifest)).is_err());
}
