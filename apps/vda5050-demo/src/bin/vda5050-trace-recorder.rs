use std::{
    collections::BTreeSet,
    path::PathBuf,
    process::ExitCode,
    time::{Duration, Instant},
};

use clap::Parser;
use serde_json::{Value, json};
use vda5050_demo::{
    LiveCapturedPublish, LiveEvent, LiveEventWriter, LiveMqttClient, build_live_capture_artifacts,
    read_live_plan, write_new_artifact,
};

#[derive(Debug, Parser)]
#[command(name = "vda5050-trace-recorder")]
#[command(about = "Broker-egress recorder for the isolated live demo")]
struct Cli {
    #[arg(long, default_value = "/artifacts/run-manifest.json")]
    plan: PathBuf,
    #[arg(long, default_value = "/artifacts")]
    output_dir: PathBuf,
}

fn main() -> ExitCode {
    match run(&Cli::parse()) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("vda5050-trace-recorder: {error}");
            ExitCode::FAILURE
        }
    }
}

fn run(cli: &Cli) -> Result<(), Box<dyn std::error::Error>> {
    let plan = read_live_plan(&cli.plan)?;
    let client_id = plan
        .client_ids()
        .get("recorder")
        .ok_or("live plan has no recorder ClientId")?;
    let subscriptions = vec![
        "vda5050/v3/lab-demo/+/+".to_owned(),
        plan.control_topic().to_owned(),
    ];
    let client = LiveMqttClient::connect(client_id, &plan, &subscriptions, None)?;
    let mut wire_writer = LiveEventWriter::create_new(
        &cli.output_dir.join("wire-events.jsonl"),
        plan.hard_limits().messages + 16,
    )?;
    write_new_artifact(
        &cli.output_dir.join("recorder-ready.json"),
        &serde_json::to_vec_pretty(&json!({
            "run_id": plan.run_id(),
            "subscription_ready": true
        }))?,
    )?;

    let deadline = Instant::now() + Duration::from_secs(plan.hard_limits().duration_seconds);
    let expected = plan
        .robots()
        .iter()
        .map(|robot| robot.serial_number.clone())
        .collect::<BTreeSet<_>>();
    let mut captures = Vec::new();
    let mut final_states = BTreeSet::new();
    let mut completion_observed = false;
    let mut wire_sequence = 1_u64;
    while !(completion_observed && final_states == expected) {
        if Instant::now() >= deadline {
            return Err("recorder duration hard limit exceeded".into());
        }
        let Some(incoming) = client.recv_timeout(Duration::from_millis(100))? else {
            continue;
        };
        let payload: Value = serde_json::from_slice(&incoming.payload)?;
        wire_writer.append(&LiveEvent::wire_observed(
            wire_sequence,
            incoming.monotonic_ns,
            incoming.topic.clone(),
            payload.clone(),
        ))?;
        wire_sequence += 1;
        if incoming.topic == plan.control_topic() {
            completion_observed =
                payload["complete"] == true && payload["run_id"].as_str() == Some(plan.run_id());
            continue;
        }
        if !plan.topic_allowlist().contains(&incoming.topic) {
            return Err(format!("recorder observed unexpected topic: {}", incoming.topic).into());
        }
        if captures.len() >= plan.hard_limits().messages {
            return Err("recorder message hard limit exceeded".into());
        }
        if incoming.topic.ends_with("/state")
            && payload["driving"] == false
            && let Some(serial) = payload["serialNumber"].as_str()
            && payload["lastNodeId"] == format!("B-{serial}")
            && expected.contains(serial)
        {
            final_states.insert(serial.to_owned());
        }
        captures.push(LiveCapturedPublish::new(
            incoming.topic,
            incoming.payload,
            incoming.qos,
            incoming.duplicate,
            incoming.monotonic_ns,
        ));
    }
    std::thread::sleep(Duration::from_millis(250));
    let artifacts = build_live_capture_artifacts(&plan, &captures)?;
    write_new_artifact(
        &cli.output_dir.join("trace.canonical.jsonl"),
        &artifacts.trace_bytes,
    )?;
    let mut evidence = serde_json::to_vec_pretty(&artifacts.evidence)?;
    evidence.push(b'\n');
    write_new_artifact(&cli.output_dir.join("synthetic-evidence.json"), &evidence)?;
    write_new_artifact(
        &cli.output_dir.join("capture-sealed.json"),
        &serde_json::to_vec_pretty(&json!({
            "run_id": plan.run_id(),
            "capture_closed": true,
            "message_count": captures.len(),
            "source_trace_sha256": artifacts.evidence.source_trace_sha256
        }))?,
    )?;
    client.shutdown();
    Ok(())
}
