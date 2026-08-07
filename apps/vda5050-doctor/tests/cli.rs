use std::fs;
use std::ops::Deref;
use std::path::{Path, PathBuf};
use std::process::Command;
use tempfile::{Builder, TempDir};

struct TestFile {
    _directory: TempDir,
    path: PathBuf,
}

impl Deref for TestFile {
    type Target = Path;

    fn deref(&self) -> &Self::Target {
        &self.path
    }
}

impl AsRef<Path> for TestFile {
    fn as_ref(&self) -> &Path {
        &self.path
    }
}

fn temp_file(name: &str, contents: &str) -> TestFile {
    let directory = Builder::new()
        .prefix("vda5050-doctor-test-")
        .tempdir()
        .expect("atomically unique temporary directory should be created");
    let path = directory.path().join(name);
    fs::write(&path, contents).expect("fixture should be written");
    TestFile {
        _directory: directory,
        path,
    }
}

fn binary() -> Command {
    Command::new(env!("CARGO_BIN_EXE_vda5050-doctor"))
}

#[test]
fn diagnose_jsonl_emits_canonical_json_with_changed_order_finding() {
    let input = temp_file(
        "trace.jsonl",
        concat!(
            "{\"topic\":\"vda5050/v3/acme/r1/order\",\"payload\":",
            "{\"headerId\":1,\"timestamp\":\"2026-08-06T00:00:00Z\",",
            "\"manufacturer\":\"acme\",\"serialNumber\":\"r1\",",
            "\"orderId\":\"o1\",\"orderUpdateId\":1,",
            "\"nodes\":[{\"nodeId\":\"A\",\"sequenceId\":0,\"released\":true}]}}\n",
            "{\"topic\":\"vda5050/v3/acme/r1/order\",\"payload\":",
            "{\"headerId\":2,\"timestamp\":\"2026-08-06T00:00:01Z\",",
            "\"manufacturer\":\"acme\",\"serialNumber\":\"r1\",",
            "\"orderId\":\"o1\",\"orderUpdateId\":1,",
            "\"nodes\":[{\"nodeId\":\"B\",\"sequenceId\":0,\"released\":true}]}}\n",
        ),
    );

    let output = binary()
        .args([
            "diagnose",
            input.to_str().expect("utf-8 path"),
            "--vda-version",
            "3.0.0",
            "--format",
            "json",
        ])
        .output()
        .expect("binary should run");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let report: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("stdout should be JSON");
    assert_eq!(report["tool"], "vda5050-doctor");
    assert_eq!(report["tool_version"], "0.2.0");
    assert!(
        report["build"]["dependency_lock_sha256"]
            .as_str()
            .is_some_and(|digest| digest.len() == 64)
    );
    assert!(
        report["build"]["rule_catalog_sha256"]
            .as_str()
            .is_some_and(|digest| digest.len() == 64)
    );
    assert_eq!(
        report["build"]["protocol_bundle_id"],
        "vda5050-3.0.0-phase1@2026-08-06"
    );
    assert_eq!(report["vda_version"], "3.0.0");
    assert!(report["import_summary"]["retained_bytes"].as_u64().unwrap() > 0);
    assert!(
        report["import_summary"]["retained_bytes"].as_u64().unwrap()
            <= report["import_summary"]["retained_bytes_limit"]
                .as_u64()
                .unwrap()
    );
    assert_eq!(report["import_summary"]["retained_budget_exhausted"], false);
    assert!(
        report["findings"]
            .as_array()
            .expect("findings array")
            .iter()
            .any(|finding| finding["rule_id"] == "LAB-D1-REPEATED-ORDER-CHANGED")
    );
}

#[test]
fn version_reports_the_release_semver() {
    let output = binary()
        .arg("--version")
        .output()
        .expect("binary should run");

    assert!(output.status.success());
    assert_eq!(
        String::from_utf8_lossy(&output.stdout).trim(),
        "vda5050-doctor 0.2.0"
    );
    assert!(output.stderr.is_empty());
}

#[test]
fn malformed_record_is_reported_and_makes_report_partial() {
    let input = temp_file(
        "malformed.jsonl",
        "{\"topic\":\"vda5050/v3/acme/r1/state\",\"payload\":{}}\nnot-json\n",
    );

    let output = binary()
        .args([
            "diagnose",
            input.to_str().expect("utf-8 path"),
            "--vda-version",
            "3.0.0",
            "--format",
            "json",
        ])
        .output()
        .expect("binary should run");

    assert!(output.status.success());
    let report: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("stdout should be JSON");
    assert_eq!(report["report_validity"], "PARTIAL");
    assert_eq!(report["import_summary"]["rejected_records"], 1);
}

#[test]
fn unknown_version_fails_without_diagnosing() {
    let input = temp_file(
        "trace.jsonl",
        "{\"topic\":\"vda5050/v3/acme/r1/state\",\"payload\":{}}\n",
    );

    let output = binary()
        .args([
            "diagnose",
            input.to_str().expect("utf-8 path"),
            "--vda-version",
            "9.9.9",
        ])
        .output()
        .expect("binary should run");

    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("unsupported VDA version"));
    assert!(output.stdout.is_empty());
}

#[test]
fn vda_2_1_is_rejected_until_its_rule_profile_is_implemented() {
    let input = temp_file(
        "trace.jsonl",
        "{\"topic\":\"vda5050/v2/acme/r1/state\",\"payload\":{}}\n",
    );

    let output = binary()
        .args([
            "diagnose",
            input.to_str().expect("utf-8 path"),
            "--vda-version",
            "2.1.0",
        ])
        .output()
        .expect("binary should run");

    assert!(!output.status.success());
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("2.1.0 rule profile is not implemented")
    );
    assert!(output.stdout.is_empty());
}

#[cfg(unix)]
#[test]
fn symlink_input_is_rejected() {
    use std::os::unix::fs::symlink;

    let target = temp_file(
        "trace.jsonl",
        "{\"topic\":\"vda5050/v3/acme/r1/state\",\"payload\":{}}\n",
    );
    let link = target.with_file_name("trace-link.jsonl");
    symlink(&target, &link).expect("symlink should be created");

    let output = binary()
        .args([
            "diagnose",
            link.to_str().expect("utf-8 path"),
            "--vda-version",
            "3.0.0",
        ])
        .output()
        .expect("binary should run");

    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("symlink"));
}
