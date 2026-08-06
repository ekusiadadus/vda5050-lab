use std::{fs, io::Read, path::Path};

use serde_json::json;
use tempfile::tempdir;
use vda5050_core::{CaptureGap, CapturePoint, CaptureRecord, MessageObserved, PayloadReference};
use vda5050_import::{
    ImportConfig, ImportFormat, RecordKind, RejectReason, import_path, validate_local_path,
};

fn write(path: &Path, bytes: &[u8]) {
    fs::write(path, bytes).expect("write fixture");
}

#[test]
fn accepted_record_adapts_to_core_without_inventing_capture_facts() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("trace.jsonl");
    write(
        &path,
        br#"{"type":"message_observed","topic":"vda5050/v3/acme/r1/state","payload":{"headerId":1},"timestamp":"2026-08-06T00:00:00Z"}"#,
    );

    let report = import_path(&path, ImportFormat::MessageEnvelopeJsonl, config()).unwrap();
    let core = report.records[0]
        .to_core_message("synthetic-capture")
        .expect("accepted messages adapt to core");

    assert_eq!(core.event_id(), report.records[0].event_id);
    assert_eq!(core.capture_id(), "synthetic-capture");
    assert_eq!(core.capture_point(), CapturePoint::ImportedUnknown);
    assert_eq!(core.source_sequence(), Some(0));
    assert_eq!(core.observed_topic(), "vda5050/v3/acme/r1/state");
    assert_eq!(
        core.payload().inline_bytes(),
        Some(br#"{"headerId":1}"#.as_slice())
    );
}

fn config() -> ImportConfig {
    ImportConfig {
        max_file_bytes: 16 * 1024,
        max_records: 10,
        max_payload_bytes: 4 * 1024,
        max_json_depth: 16,
    }
}

#[test]
fn imports_canonical_jsonl_losslessly_with_offsets_digests_and_stable_ids() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("trace.jsonl");
    let original_message = MessageObserved::new(
        "source-event-1",
        "capture-1",
        CapturePoint::BrokerIngress,
        Some(1),
        "vda5050/v3/acme/r1/state",
        PayloadReference::inline(
            br#"{"headerId":1,"manufacturer":"acme","serialNumber":"r1"}"#.to_vec(),
        ),
    );
    let message: CaptureRecord = original_message.clone().into();
    let gap: CaptureRecord =
        CaptureGap::new("capture-1", "gap-1", Some(2), Some(4), "capture restarted").into();
    let first = serde_json::to_vec(&message).unwrap();
    let second = serde_json::to_vec(&gap).unwrap();
    let mut input = first.clone();
    input.push(b'\n');
    input.extend_from_slice(&second);
    input.push(b'\n');
    write(&path, &input);

    let report = import_path(&path, ImportFormat::CanonicalJsonl, config()).unwrap();

    assert_eq!(report.summary.imported, 1);
    assert_eq!(report.summary.gaps, 1);
    assert_eq!(report.summary.rejected, 0);
    assert_eq!(report.records.len(), 2);
    assert_eq!(report.records[0].location.byte_offset, 0);
    assert_eq!(report.records[0].location.line, Some(1));
    assert_eq!(report.records[0].raw.as_deref(), Some(first.as_slice()));
    assert_eq!(report.records[0].raw_digest.len(), 64);
    assert_eq!(report.records[0].event_digest.len(), 64);
    assert_eq!(report.records[0].event_id.len(), 64);
    assert!(matches!(report.records[0].kind, RecordKind::Message(_)));
    assert!(matches!(report.records[1].kind, RecordKind::Gap { .. }));
    let adapted = report.records[0].to_core_message("ignored").unwrap();
    assert_eq!(adapted.event_id(), report.records[0].event_id);
    assert_eq!(adapted.capture_id(), original_message.capture_id());
    assert_eq!(adapted.capture_point(), original_message.capture_point());
    assert_eq!(
        adapted.source_sequence(),
        original_message.source_sequence()
    );

    let repeated = import_path(&path, ImportFormat::CanonicalJsonl, config()).unwrap();
    assert_eq!(report.records[0].event_id, repeated.records[0].event_id);
    assert_eq!(report.records[0].raw_digest, repeated.records[0].raw_digest);
}

#[test]
fn stable_event_ids_are_scoped_to_the_source_trace_digest() {
    let dir = tempdir().unwrap();
    let first_path = dir.path().join("first.jsonl");
    let second_path = dir.path().join("second.jsonl");
    let shared = br#"{"topic":"uagv/v2/acme/r1/state","payload":{"headerId":1}}"#;
    write(&first_path, shared);
    let mut extended = shared.to_vec();
    extended
        .extend_from_slice(b"\n{\"topic\":\"uagv/v2/acme/r1/state\",\"payload\":{\"headerId\":2}}");
    write(&second_path, &extended);

    let first = import_path(&first_path, ImportFormat::MessageEnvelopeJsonl, config()).unwrap();
    let second = import_path(&second_path, ImportFormat::MessageEnvelopeJsonl, config()).unwrap();

    assert_ne!(first.source_digest, second.source_digest);
    assert_eq!(first.records[0].raw_digest, second.records[0].raw_digest);
    assert_ne!(first.records[0].event_id, second.records[0].event_id);
}

#[test]
fn canonical_inline_payload_digest_mismatch_is_rejected() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("tampered.jsonl");
    let message: CaptureRecord = MessageObserved::new(
        "source-event-1",
        "capture-1",
        CapturePoint::ImportedUnknown,
        Some(1),
        "vda5050/v3/acme/r1/state",
        PayloadReference::inline(br#"{"headerId":1}"#.to_vec()),
    )
    .into();
    let mut value = serde_json::to_value(message).unwrap();
    value["record"]["payload"]["sha256"] = json!("0".repeat(64));
    write(&path, serde_json::to_string(&value).unwrap().as_bytes());

    let report = import_path(&path, ImportFormat::CanonicalJsonl, config()).unwrap();
    assert!(matches!(
        report.records[0].kind,
        RecordKind::Rejected {
            reason: RejectReason::PayloadDigestMismatch
        }
    ));
}

#[test]
fn canonical_inline_payload_size_mismatch_is_rejected() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("wrong-size.jsonl");
    let message: CaptureRecord = MessageObserved::new(
        "source-event-1",
        "capture-1",
        CapturePoint::ImportedUnknown,
        Some(1),
        "vda5050/v3/acme/r1/state",
        PayloadReference::inline(br#"{"headerId":1}"#.to_vec()),
    )
    .into();
    let mut value = serde_json::to_value(message).unwrap();
    value["record"]["payload"]["size_bytes"] = json!(999);
    write(&path, serde_json::to_string(&value).unwrap().as_bytes());

    let report = import_path(&path, ImportFormat::CanonicalJsonl, config()).unwrap();
    assert!(matches!(
        report.records[0].kind,
        RecordKind::Rejected {
            reason: RejectReason::PayloadSizeMismatch
        }
    ));
}

#[test]
fn canonical_string_payload_is_validated_then_imported_without_rewriting_raw_record() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("string-payload.jsonl");
    let raw = br#"{"type":"message_observed","topic":"uagv/v2/acme/r1/state","payload":"{\"headerId\":1}"}"#;
    write(&path, raw);

    let report = import_path(&path, ImportFormat::MessageEnvelopeJsonl, config()).unwrap();

    assert_eq!(report.summary.imported, 1);
    assert_eq!(report.records[0].raw.as_deref(), Some(raw.as_slice()));
    let RecordKind::Message(message) = &report.records[0].kind else {
        panic!("expected imported message")
    };
    assert_eq!(message.payload, json!({"headerId": 1}));
}

#[test]
fn array_adapter_imports_message_envelopes_and_accounts_for_every_element() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("messages.json");
    let input = serde_json::to_vec(&json!([
        {"topic":"uagv/v2/acme/r1/state", "payload":{"headerId":1}, "timestamp":"2026-08-06T00:00:00Z"},
        {"topic":"uagv/v2/acme/r1/order", "payload":"{not json}"},
        {"note":"maintenance began"}
    ]))
    .unwrap();
    write(&path, &input);

    let report = import_path(&path, ImportFormat::MessageEnvelopeArray, config()).unwrap();

    assert_eq!(report.summary.imported, 1);
    assert_eq!(report.summary.rejected, 2);
    assert_eq!(report.summary.gaps, 0);
    assert_eq!(report.records.len(), 3);
    assert_eq!(report.records[0].location.record_index, 0);
    assert_eq!(report.records[0].location.byte_offset, 1);
    assert_eq!(report.records[0].location.line, Some(1));
    assert_eq!(report.records[1].location.record_index, 1);
    assert_eq!(report.records[2].location.record_index, 2);
    assert!(matches!(
        report.records[1].kind,
        RecordKind::Rejected {
            reason: RejectReason::PayloadMalformedJson
        }
    ));
    assert!(matches!(
        report.records[2].kind,
        RecordKind::Rejected {
            reason: RejectReason::MissingEnvelopeField { .. }
        }
    ));
}

#[test]
fn envelope_adapter_rejects_present_but_invalid_timestamp_instead_of_dropping_it() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("invalid-timestamp.jsonl");
    write(
        &path,
        br#"{"topic":"uagv/v2/acme/r1/state","payload":{"headerId":1},"timestamp":42}"#,
    );

    let report = import_path(&path, ImportFormat::MessageEnvelopeJsonl, config()).unwrap();
    assert!(matches!(
        report.records[0].kind,
        RecordKind::Rejected {
            reason: RejectReason::InvalidEnvelopeField { .. }
        }
    ));
}

#[test]
fn malformed_json_and_invalid_utf8_are_rejected_records_not_silently_skipped() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("bad.jsonl");
    write(&path, b"{\"type\":\"message_observed\"}\n{nope}\n\xff\n\n");

    let report = import_path(&path, ImportFormat::CanonicalJsonl, config()).unwrap();

    assert_eq!(report.records.len(), 4);
    assert_eq!(report.summary.imported, 0);
    assert_eq!(report.summary.rejected, 4);
    assert!(matches!(
        report.records[1].kind,
        RecordKind::Rejected {
            reason: RejectReason::MalformedJson
        }
    ));
    assert!(matches!(
        report.records[2].kind,
        RecordKind::Rejected {
            reason: RejectReason::InvalidUtf8
        }
    ));
    assert!(matches!(
        report.records[3].kind,
        RecordKind::Rejected {
            reason: RejectReason::EmptyRecord
        }
    ));
}

#[test]
fn duplicate_json_keys_are_rejected_explicitly() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("duplicate.jsonl");
    write(
        &path,
        br#"{"type":"message_observed","topic":"a","topic":"b","payload":{}}"#,
    );

    let report = import_path(&path, ImportFormat::MessageEnvelopeJsonl, config()).unwrap();
    assert_eq!(report.summary.rejected, 1);
    assert!(matches!(
        report.records[0].kind,
        RecordKind::Rejected {
            reason: RejectReason::DuplicateJsonKey { .. }
        }
    ));
}

#[test]
fn nested_duplicate_json_keys_are_also_rejected() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("nested-duplicate.jsonl");
    write(
        &path,
        br#"{"type":"message_observed","topic":"a","payload":{"headerId":1,"headerId":2}}"#,
    );

    let report = import_path(&path, ImportFormat::MessageEnvelopeJsonl, config()).unwrap();
    assert!(matches!(
        report.records[0].kind,
        RecordKind::Rejected {
            reason: RejectReason::DuplicateJsonKey { .. }
        }
    ));
}

#[test]
fn unknown_and_non_message_canonical_records_are_never_silently_dropped() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("mixed.jsonl");
    write(
        &path,
        b"{\"record_type\":\"CAPTURE_OPENED\",\"record\":{\"capture_id\":\"c1\",\"opened_at\":\"2026-08-06T00:00:00Z\",\"clock\":null,\"synthetic\":true}}\n{\"record_type\":\"FUTURE_RECORD\",\"record\":{}}\n",
    );

    let report = import_path(&path, ImportFormat::CanonicalJsonl, config()).unwrap();
    assert_eq!(report.records.len(), 2);
    assert_eq!(report.summary.non_messages, 1);
    assert_eq!(report.summary.unknown, 1);
    assert_eq!(report.summary.rejected, 1);
    assert!(matches!(
        report.records[0].kind,
        RecordKind::NonMessage { .. }
    ));
    assert!(matches!(
        report.records[1].kind,
        RecordKind::Rejected {
            reason: RejectReason::UnknownRecordType { .. }
        }
    ));
}

#[test]
fn enforces_file_record_payload_and_depth_limits() {
    let dir = tempdir().unwrap();

    let too_large = dir.path().join("large.jsonl");
    write(&too_large, b"{}\n{}\n");
    let mut limits = config();
    limits.max_file_bytes = 3;
    let err = import_path(&too_large, ImportFormat::CanonicalJsonl, limits).unwrap_err();
    assert!(err.to_string().contains("file byte limit"));

    let too_many = dir.path().join("many.jsonl");
    write(&too_many, b"{}\n{}\n{}\n");
    let mut limits = config();
    limits.max_records = 2;
    let report = import_path(&too_many, ImportFormat::CanonicalJsonl, limits).unwrap();
    assert_eq!(report.records.len(), 3);
    assert_eq!(report.summary.rejected, 3);
    assert_eq!(report.summary.truncated, 1);
    assert!(matches!(
        report.records[2].kind,
        RecordKind::Rejected {
            reason: RejectReason::RecordLimitExceeded
        }
    ));

    let payload = dir.path().join("payload.jsonl");
    write(
        &payload,
        br#"{"type":"message_observed","topic":"a","payload":{"long":"1234567890"}}"#,
    );
    let mut limits = config();
    limits.max_payload_bytes = 4;
    let report = import_path(&payload, ImportFormat::MessageEnvelopeJsonl, limits).unwrap();
    assert!(matches!(
        report.records[0].kind,
        RecordKind::Rejected {
            reason: RejectReason::PayloadTooLarge { .. }
        }
    ));

    let deep = dir.path().join("deep.jsonl");
    write(
        &deep,
        br#"{"type":"message_observed","topic":"a","payload":{"a":{"b":{"c":1}}}}"#,
    );
    let mut limits = config();
    limits.max_json_depth = 2;
    let report = import_path(&deep, ImportFormat::MessageEnvelopeJsonl, limits).unwrap();
    assert!(matches!(
        report.records[0].kind,
        RecordKind::Rejected {
            reason: RejectReason::JsonTooDeep { .. }
        }
    ));
}

#[test]
fn configured_limits_cannot_disable_compiled_resource_ceilings() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("small.jsonl");
    write(&path, br#"{"topic":"a","payload":{}}"#);

    for limits in [
        ImportConfig {
            max_file_bytes: u64::MAX,
            ..ImportConfig::default()
        },
        ImportConfig {
            max_records: usize::MAX,
            ..ImportConfig::default()
        },
        ImportConfig {
            max_payload_bytes: usize::MAX,
            ..ImportConfig::default()
        },
        ImportConfig {
            max_json_depth: usize::MAX,
            ..ImportConfig::default()
        },
    ] {
        let error = import_path(&path, ImportFormat::MessageEnvelopeJsonl, limits)
            .expect_err("unsafe limit override must fail before reading");
        assert!(error.to_string().contains("hard maximum"));
    }
}

#[test]
fn local_path_validation_rejects_uris_absolute_traversal_and_non_regular_files() {
    let dir = tempdir().unwrap();
    let base = dir.path();
    let regular = base.join("trace.jsonl");
    write(&regular, b"{}\n");

    let mut opened = validate_local_path(base, Path::new("trace.jsonl")).unwrap();
    assert_eq!(opened.path(), regular);
    let mut contents = Vec::new();
    opened.read_to_end(&mut contents).unwrap();
    assert_eq!(contents, b"{}\n");
    assert!(validate_local_path(base, Path::new("https://example.test/x")).is_err());
    assert!(validate_local_path(base, Path::new("file:///tmp/x")).is_err());
    assert!(validate_local_path(base, Path::new("../escape.jsonl")).is_err());
    assert!(validate_local_path(base, &regular).is_err());
    assert!(validate_local_path(base, Path::new(".")).is_err());
    assert!(validate_local_path(base, Path::new("mqtt:broker/topic")).is_err());
    assert!(validate_local_path(base, Path::new("C:\\trace.jsonl")).is_err());
}

#[cfg(unix)]
#[test]
fn validated_local_path_is_bound_to_the_opened_descriptor() {
    let dir = tempdir().unwrap();
    let original = dir.path().join("trace.jsonl");
    let moved = dir.path().join("original.jsonl");
    write(&original, b"original\n");

    let mut opened = validate_local_path(dir.path(), Path::new("trace.jsonl")).unwrap();
    fs::rename(&original, &moved).unwrap();
    write(&original, b"replacement\n");

    let mut contents = String::new();
    opened.read_to_string(&mut contents).unwrap();
    assert_eq!(contents, "original\n");
}

#[cfg(unix)]
#[test]
fn rejects_symlinks_and_device_files() {
    use std::os::unix::fs::symlink;

    let dir = tempdir().unwrap();
    let target = dir.path().join("target.jsonl");
    let link = dir.path().join("link.jsonl");
    write(&target, b"{}\n");
    symlink(&target, &link).unwrap();

    assert!(import_path(&link, ImportFormat::CanonicalJsonl, config()).is_err());
    assert!(
        import_path(
            Path::new("/dev/null"),
            ImportFormat::CanonicalJsonl,
            config()
        )
        .is_err()
    );

    let nested = dir.path().join("nested");
    fs::create_dir(&nested).unwrap();
    let nested_file = nested.join("trace.jsonl");
    write(&nested_file, b"{}\n");
    let nested_link = dir.path().join("nested-link");
    symlink(&nested, &nested_link).unwrap();
    assert!(validate_local_path(dir.path(), Path::new("nested-link/trace.jsonl")).is_err());

    let base_link = dir.path().join("base-link");
    symlink(&nested, &base_link).unwrap();
    assert!(validate_local_path(&base_link, Path::new("trace.jsonl")).is_err());
}

#[cfg(windows)]
#[test]
fn rejects_windows_file_and_directory_reparse_points_and_device_names() {
    use std::os::windows::fs::{symlink_dir, symlink_file};

    let dir = tempdir().unwrap();
    let target = dir.path().join("target.jsonl");
    let link = dir.path().join("link.jsonl");
    write(&target, b"{}\n");
    let direct = import_path(&target, ImportFormat::CanonicalJsonl, config())
        .expect("ordinary Windows trace files must remain importable");
    assert_eq!(direct.records.len(), 1);
    symlink_file(&target, &link)
        .expect("Windows release runner must permit creating a test file symlink");

    assert!(import_path(&link, ImportFormat::CanonicalJsonl, config()).is_err());

    let nested = dir.path().join("nested");
    fs::create_dir(&nested).unwrap();
    write(&nested.join("trace.jsonl"), b"{}\n");
    let nested_link = dir.path().join("nested-link");
    symlink_dir(&nested, &nested_link)
        .expect("Windows release runner must permit creating a test directory symlink");

    assert!(validate_local_path(dir.path(), Path::new("nested-link/trace.jsonl")).is_err());
    assert!(import_path(Path::new("NUL"), ImportFormat::CanonicalJsonl, config()).is_err());
}

#[test]
fn malformed_array_input_becomes_an_explicit_rejected_record() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("broken.json");
    write(&path, b"[{\"topic\":\"a\",\"payload\":{}}, nope]");

    let report = import_path(&path, ImportFormat::MessageEnvelopeArray, config()).unwrap();

    assert!(report.summary.rejected >= 1);
    assert!(report.records.iter().any(|record| matches!(
        record.kind,
        RecordKind::Rejected {
            reason: RejectReason::MalformedJson
        }
    )));
}

#[test]
fn structurally_broken_array_preserves_preceding_record_accounting() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("truncated.json");
    write(
        &path,
        b"[{\"topic\":\"a\",\"payload\":{}},{\"topic\":\"b\",\"payload\":{",
    );

    let report = import_path(&path, ImportFormat::MessageEnvelopeArray, config()).unwrap();

    assert_eq!(report.summary.imported, 1);
    assert_eq!(report.summary.rejected, 1);
    assert_eq!(report.records.len(), 2);
    assert_eq!(report.records[0].location.record_index, 0);
    assert_eq!(report.records[1].location.record_index, 1);
    assert!(matches!(
        report.records[1].kind,
        RecordKind::Rejected {
            reason: RejectReason::MalformedJson
        }
    ));
}

#[test]
fn array_locations_track_lines_incrementally_across_multiline_elements() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("multiline.json");
    write(
        &path,
        br#"[
  {
    "topic": "a",
    "payload": {}
  },

  {"topic":"b","payload":{}}
]"#,
    );

    let report = import_path(&path, ImportFormat::MessageEnvelopeArray, config()).unwrap();

    assert_eq!(report.records.len(), 2);
    assert_eq!(report.records[0].location.line, Some(2));
    assert_eq!(report.records[1].location.line, Some(7));
}

#[test]
fn aggregate_retained_byte_budget_fails_closed_before_record_metadata_exhaustion() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("many-small-records.jsonl");
    let input = "{}\n".repeat(100);
    write(&path, input.as_bytes());
    let limits = ImportConfig {
        max_file_bytes: u64::try_from(input.len()).unwrap(),
        max_records: 100,
        max_payload_bytes: 128,
        max_json_depth: 8,
    };

    let report = import_path(&path, ImportFormat::CanonicalJsonl, limits).unwrap();

    assert!(report.summary.retained_bytes <= report.summary.retained_bytes_limit);
    assert!(report.summary.truncated > 0);
    assert!(report.records.iter().any(|record| matches!(
        record.kind,
        RecordKind::Rejected {
            reason: RejectReason::RetainedBytesLimitExceeded { .. }
        }
    )));
}
