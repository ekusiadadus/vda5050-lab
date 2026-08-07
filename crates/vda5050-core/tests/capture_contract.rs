use vda5050_core::{
    AdapterWarning, CaptureClosed, CaptureGap, CaptureLifecycle, CaptureLifecycleError,
    CaptureOpened, CapturePoint, CaptureRecord, ClockDescriptor, ClockEpochChanged,
    ClockWrapPolicy, MessageObserved, PayloadReference,
};

fn opened() -> CaptureRecord {
    CaptureOpened::new(
        "capture-1",
        "2026-08-06T00:00:00Z",
        Some(ClockDescriptor::new(
            "process-1",
            "std::time::Instant",
            1,
            ClockWrapPolicy::DoesNotWrap,
            "epoch-1",
            Some(1_000_000),
        )),
        false,
    )
    .into()
}

fn message() -> CaptureRecord {
    MessageObserved::new(
        "event-1",
        "capture-1",
        CapturePoint::SubscriberDelivery,
        Some(1),
        "vda5050/v3/acme/r1/state",
        PayloadReference::inline(br#"{"headerId":1}"#.to_vec()),
    )
    .with_observed_delivery(Some(1), Some(false))
    .with_time(
        Some("2026-08-06T00:00:01Z".to_owned()),
        Some(1_000_000_000),
        Some("process-1".to_owned()),
        Some("epoch-1".to_owned()),
    )
    .into()
}

fn closed() -> CaptureRecord {
    CaptureClosed::new(
        "capture-1",
        "2026-08-06T00:00:02Z",
        2,
        "f".repeat(64),
        false,
    )
    .into()
}

#[test]
fn capture_records_round_trip_without_losing_inline_payload_bytes() {
    let record = message();
    let bytes = serde_json::to_vec(&record).unwrap();
    let decoded: CaptureRecord = serde_json::from_slice(&bytes).unwrap();

    assert_eq!(decoded, record);
    let CaptureRecord::MessageObserved(decoded_message) = decoded else {
        panic!("expected message record")
    };
    assert_eq!(
        decoded_message.payload().inline_bytes(),
        Some(br#"{"headerId":1}"#.as_slice())
    );
}

#[test]
fn lifecycle_validates_order_identity_and_terminal_close() {
    let lifecycle = CaptureLifecycle::validate(vec![opened(), message(), closed()]).unwrap();
    assert_eq!(lifecycle.capture_id(), "capture-1");
    assert_eq!(lifecycle.records().len(), 3);

    let wrong_capture = MessageObserved::new(
        "event-x",
        "capture-2",
        CapturePoint::ImportedUnknown,
        None,
        "vda5050/v3/acme/r2/state",
        PayloadReference::inline(vec![]),
    );
    assert!(matches!(
        CaptureLifecycle::validate(vec![opened(), wrong_capture.into(), closed()]),
        Err(CaptureLifecycleError::CaptureIdMismatch { .. })
    ));

    assert!(matches!(
        CaptureLifecycle::validate(vec![opened(), closed(), message()]),
        Err(CaptureLifecycleError::RecordAfterClose { .. })
    ));
}

#[test]
fn lifecycle_requires_exactly_one_open_and_close() {
    assert_eq!(
        CaptureLifecycle::validate(vec![]).unwrap_err(),
        CaptureLifecycleError::MissingOpen
    );
    assert!(matches!(
        CaptureLifecycle::validate(vec![message(), closed()]),
        Err(CaptureLifecycleError::FirstRecordNotOpen)
    ));
    assert_eq!(
        CaptureLifecycle::validate(vec![opened(), message()]).unwrap_err(),
        CaptureLifecycleError::MissingClose
    );
}

#[test]
fn all_append_only_record_kinds_are_serde_ready() {
    let records = vec![
        opened(),
        CaptureGap::new(
            "capture-1",
            "gap-1",
            Some(2),
            Some(4),
            "subscriber overflow",
        )
        .into(),
        AdapterWarning::new("capture-1", "warning-1", "unknown line", Some(42)).into(),
        ClockEpochChanged::new(
            "capture-1",
            "process-1",
            "epoch-1",
            "epoch-2",
            "capture process restarted",
        )
        .into(),
        closed(),
    ];

    let encoded = serde_json::to_string(&records).unwrap();
    let decoded: Vec<CaptureRecord> = serde_json::from_str(&encoded).unwrap();
    assert_eq!(decoded, records);
    assert!(encoded.contains("CAPTURE_GAP"));
    assert!(encoded.contains("CLOCK_EPOCH_CHANGED"));
}

#[test]
fn final_manifest_is_deterministic_and_references_the_immutable_records() {
    let first = CaptureLifecycle::validate(vec![opened(), message(), closed()]).unwrap();
    let second = CaptureLifecycle::validate(vec![opened(), message(), closed()]).unwrap();
    let manifest_a = first.derive_final_manifest().unwrap();
    let manifest_b = second.derive_final_manifest().unwrap();

    assert_eq!(manifest_a, manifest_b);
    assert_eq!(manifest_a.capture_id(), "capture-1");
    assert_eq!(manifest_a.record_count(), 3);
    assert_eq!(manifest_a.records_sha256().len(), 64);
}

#[test]
fn capture_metadata_and_external_payload_are_available_without_mutation() {
    let clock = ClockDescriptor::new(
        "clock-domain",
        "monotonic origin",
        10,
        ClockWrapPolicy::Unknown,
        "epoch-a",
        None,
    );
    assert_eq!(clock.domain(), "clock-domain");
    assert_eq!(clock.epoch(), "epoch-a");

    let opened = CaptureOpened::new("capture-x", "wall-time", Some(clock), true);
    assert_eq!(opened.capture_id(), "capture-x");
    assert_eq!(opened.opened_at(), "wall-time");
    assert_eq!(opened.clock().unwrap().epoch(), "epoch-a");
    assert!(opened.synthetic());

    let external = PayloadReference::external("trace://payload/7", "a".repeat(64), 42);
    assert!(external.inline_bytes().is_none());
    assert_eq!(external.sha256(), "a".repeat(64));

    let observed = MessageObserved::new(
        "event-x",
        "capture-x",
        CapturePoint::BrokerIngress,
        Some(9),
        "vda5050/v3/acme/r1/order",
        external,
    )
    .with_time(
        Some("2026-08-07T00:00:00Z".to_owned()),
        Some(42),
        Some("broker-clock".to_owned()),
        Some("clock-epoch-1".to_owned()),
    );
    assert_eq!(observed.event_id(), "event-x");
    assert_eq!(observed.capture_id(), "capture-x");
    assert_eq!(observed.capture_point(), CapturePoint::BrokerIngress);
    assert_eq!(observed.source_sequence(), Some(9));
    assert_eq!(observed.observed_topic(), "vda5050/v3/acme/r1/order");
    assert_eq!(observed.observed_monotonic_ns(), Some(42));
    assert_eq!(observed.clock_epoch(), Some("clock-epoch-1"));
    assert!(observed.payload().inline_bytes().is_none());
}

#[test]
fn a_second_open_record_is_rejected() {
    assert!(matches!(
        CaptureLifecycle::validate(vec![opened(), opened(), closed()]),
        Err(CaptureLifecycleError::DuplicateOpen { index: 1 })
    ));
}
