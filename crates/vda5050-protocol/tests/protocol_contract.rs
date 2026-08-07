use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use serde_json::json;
use sha2::{Digest, Sha256};
use vda5050_protocol::{
    AdvisoryStatus, AnalysisBuildIdentity, ArtifactDigest, BundleError, BundleManifest,
    BundlePolicy, ComparatorError, ComparatorProfile, ErratumOverlay, ErratumStatus, FieldClass,
    SemanticRelation, VerifiedBundle, compare_order_bytes, compare_orders,
};

struct TempDir(PathBuf);

impl TempDir {
    fn new(label: &str) -> Self {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock should be after the epoch")
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "vda5050-protocol-{label}-{}-{nonce}",
            std::process::id()
        ));
        fs::create_dir_all(&path).expect("temporary directory should be created");
        Self(path)
    }

    fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn sha256(bytes: &[u8]) -> String {
    hex::encode(Sha256::digest(bytes))
}

fn manifest(reference: &str, digest: &str) -> BundleManifest {
    BundleManifest {
        bundle_id: "vda-3.0-minimal".into(),
        vda_version: "3.0.0".into(),
        evaluator_api_version: "1".into(),
        rule_implementation_digest: sha256(b"rules-v1"),
        artifacts: BTreeMap::from([(
            reference.into(),
            ArtifactDigest {
                sha256: digest.into(),
            },
        )]),
        errata: vec![],
        advisories: vec![],
    }
}

fn write_object(root: &Path, bytes: &[u8]) -> String {
    let digest = sha256(bytes);
    let objects = root.join("objects");
    fs::create_dir_all(&objects).expect("object directory should be created");
    fs::write(objects.join(&digest), bytes).expect("object should be written");
    digest
}

#[test]
fn verifies_and_resolves_only_manifest_listed_content_addressed_objects() {
    let temp = TempDir::new("valid-bundle");
    let digest = write_object(temp.path(), b"official source bytes");
    let bundle = VerifiedBundle::open(temp.path(), manifest("spec/vda-5050.pdf", &digest))
        .expect("valid bundle should verify");

    assert_eq!(
        bundle
            .resolve("spec/vda-5050.pdf")
            .expect("listed object should resolve"),
        b"official source bytes"
    );
    assert!(matches!(
        bundle.resolve("spec/not-listed.pdf"),
        Err(BundleError::UnlistedReference { .. })
    ));
}

#[test]
fn rejects_missing_and_digest_mismatched_objects() {
    let missing = TempDir::new("missing-object");
    fs::create_dir_all(missing.path().join("objects")).expect("objects directory");
    let missing_result = VerifiedBundle::open(
        missing.path(),
        manifest("spec/vda.pdf", &sha256(b"missing")),
    );
    assert!(matches!(
        missing_result,
        Err(BundleError::MissingObject { .. })
    ));

    let tampered = TempDir::new("tampered-object");
    let expected_digest = sha256(b"expected");
    fs::create_dir_all(tampered.path().join("objects")).expect("objects directory");
    fs::write(
        tampered.path().join("objects").join(&expected_digest),
        b"tampered",
    )
    .expect("tampered object should be written");
    let tampered_result =
        VerifiedBundle::open(tampered.path(), manifest("spec/vda.pdf", &expected_digest));
    assert!(matches!(
        tampered_result,
        Err(BundleError::DigestMismatch { .. })
    ));
}

#[test]
fn rejects_invalid_digests_non_regular_objects_and_oversized_artifacts() {
    let invalid_digest = TempDir::new("invalid-digest");
    fs::create_dir_all(invalid_digest.path().join("objects")).expect("objects directory");
    assert!(matches!(
        VerifiedBundle::open(
            invalid_digest.path(),
            manifest("spec/vda.pdf", "not-a-sha256")
        ),
        Err(BundleError::InvalidDigest { .. })
    ));

    let non_regular = TempDir::new("non-regular");
    let directory_digest = sha256(b"directory");
    fs::create_dir_all(non_regular.path().join("objects").join(&directory_digest))
        .expect("digest-named directory");
    assert!(matches!(
        VerifiedBundle::open(
            non_regular.path(),
            manifest("spec/vda.pdf", &directory_digest)
        ),
        Err(BundleError::NonRegularObject { .. })
    ));

    let oversized = TempDir::new("oversized");
    let oversized_digest = write_object(oversized.path(), b"five!");
    let policy = BundlePolicy {
        max_artifact_bytes: 4,
    };
    assert!(matches!(
        VerifiedBundle::open_with_policy(
            oversized.path(),
            manifest("spec/vda.pdf", &oversized_digest),
            policy,
        ),
        Err(BundleError::ArtifactTooLarge { .. })
    ));
}

#[test]
fn rejects_unsafe_logical_references_before_resolution() {
    let invalid_references = [
        "http://example.invalid/vda.pdf",
        "https://example.invalid/vda.pdf",
        "mqtt://broker.invalid/vda",
        "s3://bucket/vda.pdf",
        "file:///tmp/vda.pdf",
        "/absolute/vda.pdf",
        "../outside.pdf",
        "spec/../../outside.pdf",
    ];

    for reference in invalid_references {
        let temp = TempDir::new("unsafe-reference");
        let digest = write_object(temp.path(), b"object");
        let result = VerifiedBundle::open(temp.path(), manifest(reference, &digest));
        assert!(
            matches!(result, Err(BundleError::UnsafeReference { .. })),
            "reference should fail closed: {reference}"
        );
    }
}

#[cfg(unix)]
#[test]
fn rejects_content_object_symlink_escape() {
    use std::os::unix::fs::symlink;

    let temp = TempDir::new("symlink-root");
    let outside = TempDir::new("symlink-outside");
    let bytes = b"outside bytes";
    let digest = sha256(bytes);
    let outside_file = outside.path().join("outside.pdf");
    fs::write(&outside_file, bytes).expect("outside file should be written");
    fs::create_dir_all(temp.path().join("objects")).expect("objects directory");
    symlink(&outside_file, temp.path().join("objects").join(&digest))
        .expect("symlink should be created");

    let result = VerifiedBundle::open(temp.path(), manifest("spec/vda.pdf", &digest));
    assert!(matches!(result, Err(BundleError::SymlinkEscape { .. })));
}

#[cfg(unix)]
#[test]
fn rejects_symlinked_objects_directory_and_symlinked_bundle_root() {
    use std::os::unix::fs::symlink;

    let temp = TempDir::new("symlinked-objects-directory");
    let outside = TempDir::new("symlinked-objects-outside");
    let bytes = b"outside object";
    let digest = sha256(bytes);
    fs::write(outside.path().join(&digest), bytes).expect("outside object");
    symlink(outside.path(), temp.path().join("objects")).expect("objects-directory symlink");

    assert!(matches!(
        VerifiedBundle::open(temp.path(), manifest("spec/vda.pdf", &digest)),
        Err(BundleError::SymlinkEscape { .. })
    ));

    let root_container = TempDir::new("symlinked-bundle-root");
    let real_root = root_container.path().join("real-root");
    fs::create_dir(&real_root).expect("real bundle root");
    let root_digest = write_object(&real_root, b"real object");
    let root_link = root_container.path().join("root-link");
    symlink(&real_root, &root_link).expect("bundle-root symlink");

    assert!(matches!(
        VerifiedBundle::open(&root_link, manifest("spec/vda.pdf", &root_digest)),
        Err(BundleError::InvalidRoot { .. })
    ));
}

#[cfg(unix)]
#[test]
fn rejects_fifo_content_object_without_blocking() {
    use std::process::Command;

    let temp = TempDir::new("fifo-object");
    let digest = sha256(b"fifo object identity");
    let objects = temp.path().join("objects");
    fs::create_dir(&objects).expect("objects directory");
    let status = Command::new("mkfifo")
        .arg(objects.join(&digest))
        .status()
        .expect("POSIX mkfifo must be available on the Unix test runner");
    assert!(status.success(), "mkfifo must create the test FIFO");

    assert!(matches!(
        VerifiedBundle::open(temp.path(), manifest("spec/vda.pdf", &digest)),
        Err(BundleError::NonRegularObject { .. })
    ));
}

#[cfg(unix)]
#[test]
fn bundle_keeps_the_opened_root_when_its_path_is_renamed_and_replaced() {
    let temp = TempDir::new("bundle-root-rename-race");
    let original_root = temp.path().join("bundle");
    let moved_root = temp.path().join("bundle-original");
    fs::create_dir(&original_root).expect("original bundle root");
    let bytes = b"verified official source bytes";
    let digest = write_object(&original_root, bytes);
    let bundle = VerifiedBundle::open(&original_root, manifest("spec/vda.pdf", &digest))
        .expect("initial bundle should verify");

    fs::rename(&original_root, &moved_root).expect("rename the verified bundle root");
    fs::create_dir(&original_root).expect("replacement bundle root");
    fs::create_dir(original_root.join("objects")).expect("replacement object directory");
    fs::write(
        original_root.join("objects").join(&digest),
        b"attacker-controlled replacement",
    )
    .expect("replacement object");

    assert_eq!(
        bundle
            .resolve("spec/vda.pdf")
            .expect("resolution must stay below the originally opened root descriptor"),
        bytes
    );
}

#[test]
fn bundle_manifest_separates_rule_contract_from_actual_tool_build() {
    let manifest = manifest("spec/vda.pdf", &sha256(b"source"));
    let serialized = serde_json::to_value(&manifest).expect("manifest should serialize");

    assert_eq!(serialized["evaluator_api_version"], "1");
    assert!(serialized.get("rule_implementation_digest").is_some());
    assert!(serialized.get("tool_build_identity").is_none());

    let report_identity = AnalysisBuildIdentity {
        tool_version: "0.1.0".into(),
        build_digest: sha256(b"binary"),
        dependency_lock_digest: sha256(b"lock"),
    };
    assert_eq!(report_identity.tool_version, "0.1.0");
}

fn published_erratum() -> ErratumOverlay {
    ErratumOverlay {
        id: "VDA-3.0-E1".into(),
        status: ErratumStatus::FormallyPublished,
        affected_version: "3.0.0".into(),
        exact_clause: "6.1.4.5".into(),
        correction: "Replace the identified sentence with corrected text.".into(),
        immutable_source: "urn:sha256:vda-3.0-e1".into(),
        source_is_immutable: true,
        sha256: sha256(b"erratum"),
    }
}

#[test]
fn only_formally_published_complete_errata_can_override_the_exact_clause() {
    let erratum = published_erratum();
    assert!(erratum.is_normative_for("3.0.0", "6.1.4.5"));
    assert!(!erratum.is_normative_for("2.1.0", "6.1.4.5"));
    assert!(!erratum.is_normative_for("3.0.0", "6.1.4.6"));

    let invalid_cases = [
        ErratumOverlay {
            status: ErratumStatus::Acknowledged,
            ..erratum.clone()
        },
        ErratumOverlay {
            status: ErratumStatus::Proposed,
            ..erratum.clone()
        },
        ErratumOverlay {
            exact_clause: "*".into(),
            ..erratum.clone()
        },
        ErratumOverlay {
            correction: String::new(),
            ..erratum.clone()
        },
        ErratumOverlay {
            source_is_immutable: false,
            ..erratum.clone()
        },
        ErratumOverlay {
            sha256: "not-a-digest".into(),
            ..erratum
        },
    ];

    for invalid in invalid_cases {
        assert!(!invalid.is_normative_for("3.0.0", "6.1.4.5"));
    }

    assert!(!AdvisoryStatus::Acknowledged.is_normative());
    assert!(!AdvisoryStatus::Proposed.is_normative());
}

fn default_profile() -> ComparatorProfile {
    ComparatorProfile::default_vda()
}

#[test]
fn comparator_reports_byte_structural_and_semantic_equality_independently() {
    let left = br#"{"headerId":1,"timestamp":"2026-08-06T00:00:00Z","manufacturer":"A","serialNumber":"R1","orderId":"O1","orderUpdateId":1,"nodes":[],"edges":[]}"#;
    let reordered = br#"{ "edges": [], "nodes": [], "orderUpdateId": 1.0, "orderId": "O1", "serialNumber": "R1", "manufacturer": "A", "timestamp": "2026-08-06T00:00:00Z", "headerId": 1e0 }"#;

    let result = compare_order_bytes(left, reordered, &default_profile())
        .expect("valid orders should compare");

    assert_eq!(result.byte_equal, Some(false));
    assert!(result.structurally_equal);
    assert_eq!(result.semantic_relation, SemanticRelation::Equal);
    assert_eq!(result.metadata.comparator_id, "vda5050-order");
    assert!(!result.metadata.comparator_version.is_empty());
    assert_eq!(
        result.metadata.excluded_fields,
        vec!["/headerId".to_owned(), "/timestamp".to_owned()]
    );
    assert_eq!(result.metadata.config_digest.len(), 64);
}

#[test]
fn default_semantics_exclude_only_republication_header_fields() {
    let original = json!({
        "headerId": 1,
        "timestamp": "2026-08-06T00:00:00Z",
        "manufacturer": "A",
        "serialNumber": "R1",
        "orderId": "O1",
        "orderUpdateId": 7,
        "zoneSetId": "zone-a",
        "nodes": [],
        "edges": []
    });
    let mut republished = original.clone();
    republished["headerId"] = json!(2);
    republished["timestamp"] = json!("2026-08-06T00:00:01Z");

    let excluded =
        compare_orders(&original, &republished, &default_profile()).expect("orders should compare");
    assert!(!excluded.structurally_equal);
    assert_eq!(excluded.semantic_relation, SemanticRelation::Equal);

    for field in ["manufacturer", "serialNumber", "orderId", "orderUpdateId"] {
        let mut changed = original.clone();
        changed[field] = if field == "orderUpdateId" {
            json!(8)
        } else {
            json!("different")
        };
        assert_eq!(
            compare_orders(&original, &changed, &default_profile())
                .expect("orders should compare")
                .semantic_relation,
            SemanticRelation::Changed,
            "{field} must remain significant"
        );
    }
}

#[test]
fn arrays_are_ordered_and_optional_omission_is_not_an_explicit_value() {
    let left = json!({
        "manufacturer": "A", "serialNumber": "R1", "orderId": "O1",
        "orderUpdateId": 1, "nodes": [{"nodeId":"n1"},{"nodeId":"n2"}], "edges": []
    });
    let reordered = json!({
        "manufacturer": "A", "serialNumber": "R1", "orderId": "O1",
        "orderUpdateId": 1, "nodes": [{"nodeId":"n2"},{"nodeId":"n1"}], "edges": []
    });
    assert_eq!(
        compare_orders(&left, &reordered, &default_profile())
            .expect("orders should compare")
            .semantic_relation,
        SemanticRelation::Changed
    );

    let explicit_optional = json!({
        "manufacturer": "A", "serialNumber": "R1", "orderId": "O1",
        "orderUpdateId": 1,
        "nodes": [
            {"nodeId":"n1","nodePosition":{"x":0,"y":0,"mapId":"demo"}},
            {"nodeId":"n2"}
        ],
        "edges": []
    });
    assert_eq!(
        compare_orders(&left, &explicit_optional, &default_profile())
            .expect("orders should compare")
            .semantic_relation,
        SemanticRelation::Changed
    );
}

#[test]
fn free_text_and_unknown_extension_differences_are_unknown_by_default() {
    let base = json!({
        "manufacturer": "A", "serialNumber": "R1", "orderId": "O1",
        "orderUpdateId": 1, "orderDescription": "first", "nodes": [], "edges": []
    });
    let mut description_changed = base.clone();
    description_changed["orderDescription"] = json!("second");
    assert_eq!(
        compare_orders(&base, &description_changed, &default_profile())
            .expect("orders should compare")
            .semantic_relation,
        SemanticRelation::Unknown
    );

    let mut extension_added = base.clone();
    extension_added["vendorMotionHint"] = json!({"mode":"fast"});
    assert_eq!(
        compare_orders(&base, &extension_added, &default_profile())
            .expect("orders should compare")
            .semantic_relation,
        SemanticRelation::Unknown
    );
}

#[test]
fn vda_300_descriptor_and_edge_limit_fields_are_classified_as_standard_content() {
    let profile = ComparatorProfile::default_vda();
    let first = json!({
        "headerId": 1,
        "timestamp": "2026-08-07T00:00:00Z",
        "version": "3.0.0",
        "manufacturer": "acme",
        "serialNumber": "r1",
        "orderId": "o1",
        "orderUpdateId": 1,
        "nodes": [
            {"nodeId":"A","sequenceId":0,"released":true,"nodeDescriptor":"pick","actions":[]},
            {"nodeId":"B","sequenceId":2,"released":true,"actions":[]}
        ],
        "edges": [
            {"edgeId":"A-B","sequenceId":1,"released":true,"edgeDescriptor":"aisle","maximumSpeed":1.0,"actions":[]}
        ]
    });
    let mut changed = first.clone();
    changed["edges"][0]["maximumSpeed"] = json!(0.5);

    let comparison = compare_orders(&first, &changed, &profile).unwrap();
    assert_eq!(comparison.semantic_relation, SemanticRelation::Changed);
}

#[test]
fn standard_field_names_are_not_misclassified_outside_their_schema_context() {
    let base = json!({
        "manufacturer": "A", "serialNumber": "R1", "orderId": "O1",
        "orderUpdateId": 1, "nodes": [{"nodeId":"n1","sequenceId":0,"released":true}],
        "edges": []
    });
    for misleading_top_level_name in ["nodeId", "value", "actionType"] {
        let mut extended = base.clone();
        extended[misleading_top_level_name] = json!("vendor extension");
        assert_eq!(
            compare_orders(&base, &extended, &default_profile())
                .expect("orders should compare")
                .semantic_relation,
            SemanticRelation::Unknown,
            "a standard field name at an invalid schema path remains an unknown extension"
        );
    }

    let mut nested_extension = base.clone();
    nested_extension["nodes"][0]["value"] = json!("vendor extension");
    assert_eq!(
        compare_orders(&base, &nested_extension, &default_profile())
            .expect("orders should compare")
            .semantic_relation,
        SemanticRelation::Unknown
    );
}

#[test]
fn exact_decimal_math_handles_sign_zero_fraction_and_exponent() {
    let left = br#"{"manufacturer":"A","serialNumber":"R1","orderId":"O1","orderUpdateId":1,"nodes":[{"nodeId":"n1","sequenceId":0,"released":true,"nodePosition":{"x":1,"y":-0.0,"theta":100e-2,"mapId":"m"}}],"edges":[]}"#;
    let right = br#"{"manufacturer":"A","serialNumber":"R1","orderId":"O1","orderUpdateId":1.00,"nodes":[{"nodeId":"n1","sequenceId":0e5,"released":true,"nodePosition":{"x":1.0,"y":0,"theta":1,"mapId":"m"}}],"edges":[]}"#;
    let comparison = compare_order_bytes(left, right, &default_profile())
        .expect("supported exact decimals should compare");
    assert!(comparison.structurally_equal);
    assert_eq!(comparison.semantic_relation, SemanticRelation::Equal);
}

fn comprehensive_order() -> serde_json::Value {
    json!({
        "headerId": 1,
        "timestamp": "2026-08-06T00:00:00Z",
        "version": "3.0.0",
        "manufacturer": "A",
        "serialNumber": "R1",
        "orderId": "O1",
        "orderUpdateId": 1,
        "nodes": [{
            "nodeId": "n1",
            "sequenceId": 0,
            "released": true,
            "nodeDescriptor": "node",
            "nodePosition": {
                "x": 1, "y": 2, "theta": 0, "mapId": "map",
                "allowedDeviationXY": {"a": 0.1, "b": 0.1, "theta": 0},
                "allowedDeviationTheta": 0.2
            },
            "actions": [{
                "actionType": "pick", "actionId": "a1", "actionDescriptor": "pick",
                "blockingType": "HARD",
                "actionParameters": [{"key": "load", "value": {"id": "L1"}}]
            }]
        }],
        "edges": [{
            "edgeId": "e1", "sequenceId": 1, "released": true,
            "edgeDescriptor": "edge",
            "maximumSpeed": 1, "maximumMobileRobotHeight": 2,
            "minimumLoadHandlingDeviceHeight": 0,
            "orientation": 0, "orientationType": "GLOBAL", "direction": "FORWARD",
            "reachOrientationBeforeEntering": true, "maxRotationSpeed": 1, "length": 3,
            "corridor": {"leftWidth": 0.5, "rightWidth": 0.5},
            "trajectory": {
                "degree": 1, "knotVector": [0, 0, 1, 1],
                "controlPoints": [{"x": 1, "y": 2, "weight": 1}]
            },
            "actions": [{
                "actionType": "signal", "actionId": "a2", "actionDescriptor": "signal",
                "blockingType": "NONE", "actionParameters": [{"key": "color", "value": "blue"}]
            }]
        }]
    })
}

#[test]
fn schema_context_covers_node_edge_trajectory_action_and_parameter_fields() {
    let original = comprehensive_order();
    let changed_paths = [
        "/nodes/0/nodePosition/x",
        "/nodes/0/actions/0/actionId",
        "/nodes/0/actions/0/actionParameters/0/key",
        "/nodes/0/actions/0/actionParameters/0/value/id",
        "/edges/0/edgeId",
        "/edges/0/trajectory/degree",
        "/edges/0/trajectory/controlPoints/0/x",
        "/edges/0/actions/0/actionId",
        "/edges/0/actions/0/actionParameters/0/key",
    ];

    for pointer in changed_paths {
        let mut changed = original.clone();
        *changed
            .pointer_mut(pointer)
            .expect("test pointer should identify a field") = json!("changed");
        assert_eq!(
            compare_orders(&original, &changed, &ComparatorProfile::default())
                .expect("orders should compare")
                .semantic_relation,
            SemanticRelation::Changed,
            "known field at {pointer} must be significant"
        );
    }

    let mut extra_node = original.clone();
    extra_node["nodes"]
        .as_array_mut()
        .expect("nodes should be an array")
        .push(json!({"nodeId":"n2","sequenceId":2,"released":true}));
    assert_eq!(
        compare_orders(&original, &extra_node, &default_profile())
            .expect("orders should compare")
            .semantic_relation,
        SemanticRelation::Changed
    );

    let mut profile = ComparatorProfile::new("opaque-extension", "1");
    profile.classify("/vendorExtension", FieldClass::Significant);
    let mut with_extension = original.clone();
    with_extension["vendorExtension"] = json!({"nested": 1});
    let mut changed_extension = with_extension.clone();
    changed_extension["vendorExtension"]["nested"] = json!(2);
    assert_eq!(
        compare_orders(&with_extension, &changed_extension, &profile)
            .expect("orders should compare")
            .semantic_relation,
        SemanticRelation::Changed
    );
}

#[test]
fn malformed_order_bytes_identify_the_failing_side() {
    let valid = br#"{"orderId":"O1"}"#;
    assert!(matches!(
        compare_order_bytes(b"{", valid, &default_profile()),
        Err(ComparatorError::InvalidLeftJson(_))
    ));
    assert!(matches!(
        compare_order_bytes(valid, b"{", &default_profile()),
        Err(ComparatorError::InvalidRightJson(_))
    ));
}

#[test]
fn versioned_profile_can_classify_free_text_and_extensions() {
    let left = json!({
        "manufacturer": "A", "serialNumber": "R1", "orderId": "O1",
        "orderUpdateId": 1, "orderDescription":"before", "vendorMode":"a",
        "nodes": [], "edges": []
    });
    let mut right = left.clone();
    right["orderDescription"] = json!("after");
    right["vendorMode"] = json!("b");

    let mut profile = ComparatorProfile::new("integrator-a", "2026-08-06");
    profile.classify("/orderDescription", FieldClass::Significant);
    profile.classify("/vendorMode", FieldClass::Significant);
    let comparison = compare_orders(&left, &right, &profile).expect("orders should compare");
    assert_eq!(comparison.semantic_relation, SemanticRelation::Changed);
    assert_eq!(comparison.metadata.profile_id, "integrator-a");
    assert_eq!(comparison.metadata.profile_version, "2026-08-06");

    let mut ignored = ComparatorProfile::new("integrator-b", "1");
    ignored.classify("/orderDescription", FieldClass::Excluded);
    ignored.classify("/vendorMode", FieldClass::Excluded);
    assert_eq!(
        compare_orders(&left, &right, &ignored)
            .expect("orders should compare")
            .semantic_relation,
        SemanticRelation::Equal
    );
}

#[test]
fn parsed_values_do_not_claim_unobservable_byte_equality() {
    let order = json!({
        "manufacturer":"A", "serialNumber":"R1", "orderId":"O1",
        "orderUpdateId":1, "nodes":[], "edges":[]
    });
    let comparison =
        compare_orders(&order, &order, &default_profile()).expect("orders should compare");
    assert_eq!(comparison.byte_equal, None);
    assert!(comparison.structurally_equal);
    assert_eq!(comparison.semantic_relation, SemanticRelation::Equal);
}
