use std::{fs, io::Read};

use tempfile::tempdir;
use vda5050_demo::{
    LiveArtifactError, LiveEvent, LiveEventWriter, LivePhase, LiveRunPlan, LiveScenario,
    read_live_plan, write_live_plan,
};

#[test]
fn plan_round_trip_is_new_file_only_and_revalidates_all_fixed_fields() {
    let dir = tempdir().expect("tempdir");
    let plan = LiveRunPlan::new("live-0001", LiveScenario::ReconnectMissingOnline).expect("plan");
    let path = write_live_plan(dir.path(), &plan).expect("write plan");
    assert_eq!(read_live_plan(&path).expect("read plan"), plan);
    assert!(matches!(
        write_live_plan(dir.path(), &plan),
        Err(LiveArtifactError::AlreadyExists(_))
    ));

    let mut value: serde_json::Value =
        serde_json::from_slice(&fs::read(&path).expect("plan bytes")).expect("plan JSON");
    value["broker_host"] = "broker.example.com".into();
    let tampered = dir.path().join("tampered.json");
    fs::write(&tampered, serde_json::to_vec(&value).unwrap()).expect("tampered plan");
    assert!(matches!(
        read_live_plan(&tampered),
        Err(LiveArtifactError::InvalidPlan)
    ));
}

#[test]
fn event_writer_appends_complete_json_lines_and_fails_closed_at_its_limit() {
    let dir = tempdir().expect("tempdir");
    let path = dir.path().join("events.jsonl");
    let mut writer = LiveEventWriter::create_new(&path, 2).expect("writer");
    writer
        .append(&LiveEvent::phase(1, LivePhase::Starting))
        .expect("first");
    writer
        .append(&LiveEvent::phase(2, LivePhase::RobotsOnline))
        .expect("second");
    assert!(matches!(
        writer.append(&LiveEvent::phase(3, LivePhase::OrderPublished)),
        Err(LiveArtifactError::EventLimit)
    ));
    drop(writer);

    let mut text = String::new();
    fs::File::open(path)
        .expect("event file")
        .read_to_string(&mut text)
        .expect("read events");
    let lines = text.lines().collect::<Vec<_>>();
    assert_eq!(lines.len(), 2);
    assert!(
        lines
            .iter()
            .all(|line| serde_json::from_str::<LiveEvent>(line).is_ok())
    );
}

#[cfg(unix)]
#[test]
fn event_writer_rejects_symlink_targets() {
    use std::os::unix::fs::symlink;

    let dir = tempdir().expect("tempdir");
    let target = dir.path().join("target");
    fs::write(&target, b"preserve").expect("target");
    let link = dir.path().join("events.jsonl");
    symlink(&target, &link).expect("symlink");
    assert!(LiveEventWriter::create_new(&link, 1).is_err());
    assert_eq!(fs::read(target).expect("preserved"), b"preserve");
}
