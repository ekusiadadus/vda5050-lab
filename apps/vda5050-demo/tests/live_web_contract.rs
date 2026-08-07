use std::fs;

use tempfile::tempdir;
use vda5050_demo::{LiveWebStore, WebBody};

#[test]
fn gateway_serves_only_fixed_get_routes_with_security_headers() {
    let root = tempdir().expect("temp root");
    let artifacts = root.path().join("artifacts");
    let static_dir = root.path().join("static");
    fs::create_dir_all(&artifacts).expect("artifact dir");
    fs::create_dir_all(&static_dir).expect("static dir");
    fs::write(
        static_dir.join("index.html"),
        b"<!doctype html><title>lab</title>",
    )
    .expect("index");
    fs::write(static_dir.join("app.js"), b"export {};").expect("app");
    fs::write(static_dir.join("model.js"), b"export {};").expect("model");
    fs::write(static_dir.join("styles.css"), b"body{}").expect("styles");
    fs::write(
        artifacts.join("run-manifest.json"),
        br#"{"synthetic":true}"#,
    )
    .expect("plan");
    let store = LiveWebStore::new(&artifacts, &static_dir, 1_024).expect("store");

    let index = store.resolve("GET", "/");
    assert_eq!(index.status, 200);
    assert_eq!(index.content_type, "text/html; charset=utf-8");
    assert_eq!(index.headers["X-Content-Type-Options"], "nosniff");
    assert!(matches!(index.body, WebBody::Bytes(_)));

    let plan = store.resolve("GET", "/api/plan");
    assert_eq!(plan.status, 200);
    assert_eq!(plan.content_type, "application/json; charset=utf-8");

    assert_eq!(store.resolve("POST", "/api/plan").status, 405);
    assert_eq!(store.resolve("GET", "/../Cargo.toml").status, 404);
    assert_eq!(store.resolve("GET", "/api/unknown").status, 404);
}

#[test]
fn gateway_reports_pending_artifacts_and_rejects_oversized_files() {
    let root = tempdir().expect("temp root");
    let artifacts = root.path().join("artifacts");
    let static_dir = root.path().join("static");
    fs::create_dir_all(&artifacts).expect("artifact dir");
    fs::create_dir_all(&static_dir).expect("static dir");
    for name in ["index.html", "app.js", "model.js", "styles.css"] {
        fs::write(static_dir.join(name), b"ok").expect("static asset");
    }
    fs::write(artifacts.join("sim-events.jsonl"), vec![b'x'; 65]).expect("events");
    let store = LiveWebStore::new(&artifacts, &static_dir, 64).expect("store");

    let status = store.resolve("GET", "/api/status");
    assert_eq!(status.status, 200);
    let WebBody::Bytes(body) = status.body;
    let status_json: serde_json::Value = serde_json::from_slice(&body).expect("status JSON");
    assert_eq!(status_json["artifacts"]["sim_events"], true);
    assert_eq!(status_json["artifacts"]["evidence_report"], false);

    assert_eq!(store.resolve("GET", "/api/sim-events").status, 413);
    assert_eq!(store.resolve("GET", "/api/report/evidence").status, 404);
}

#[test]
fn gateway_serves_official_lsmart_artifact_aliases_and_map() {
    let root = tempdir().expect("temp root");
    let artifacts = root.path().join("artifacts");
    let static_dir = root.path().join("static");
    fs::create_dir_all(&artifacts).expect("artifact dir");
    fs::create_dir_all(&static_dir).expect("static dir");
    for name in ["index.html", "app.js", "model.js", "styles.css"] {
        fs::write(static_dir.join(name), b"ok").expect("static asset");
    }
    fs::write(
        artifacts.join("lsmart-run-manifest.json"),
        br#"{"schema":"vda5050-lab.lsmart-run/1"}"#,
    )
    .expect("LSMART plan");
    fs::write(
        artifacts.join("lsmart-sim-events.jsonl"),
        br#"{"event_type":"SIM_SNAPSHOT"}"#,
    )
    .expect("LSMART events");
    fs::write(
        artifacts.join("lsmart-map.json"),
        br#"{"n_row":1,"n_col":2,"layout":[".."]}"#,
    )
    .expect("LSMART map");
    let store = LiveWebStore::new(&artifacts, &static_dir, 1_024).expect("store");

    assert_eq!(store.resolve("GET", "/api/plan").status, 200);
    assert_eq!(store.resolve("GET", "/api/sim-events").status, 200);
    assert_eq!(store.resolve("GET", "/api/map").status, 200);

    let status = store.resolve("GET", "/api/status");
    let WebBody::Bytes(body) = status.body;
    let status_json: serde_json::Value = serde_json::from_slice(&body).expect("status JSON");
    assert_eq!(status_json["artifacts"]["plan"], true);
    assert_eq!(status_json["artifacts"]["sim_events"], true);
    assert_eq!(status_json["artifacts"]["map"], true);
}
