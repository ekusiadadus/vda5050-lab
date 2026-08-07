use std::{
    fs,
    path::PathBuf,
    process::ExitCode,
    time::{Duration, Instant},
};

use clap::Parser;
use serde_json::{Value, json};
use vda5050_demo::{
    LiveCapturedPublish, LiveEvent, LiveEventWriter, LiveMqttClient, LsmartRunPlan,
    build_lsmart_capture_artifacts, write_new_artifact,
};

#[derive(Debug, Parser)]
#[command(name = "vda5050-lsmart-recorder")]
#[command(about = "Broker-egress recorder for the official LSMART demo")]
struct Cli {
    #[arg(long, default_value = "/artifacts/lsmart-run-manifest.json")]
    plan: PathBuf,
    #[arg(long, default_value = "/artifacts")]
    output_dir: PathBuf,
}

fn main() -> ExitCode {
    match run(&Cli::parse()) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("vda5050-lsmart-recorder: {error}");
            ExitCode::FAILURE
        }
    }
}

fn run(cli: &Cli) -> Result<(), Box<dyn std::error::Error>> {
    let metadata = fs::symlink_metadata(&cli.plan)?;
    if !metadata.is_file() || metadata.file_type().is_symlink() || metadata.len() > 128 * 1_024 {
        return Err("LSMART plan must be a bounded regular file".into());
    }
    let plan: LsmartRunPlan = serde_json::from_slice(&fs::read(&cli.plan)?)?;
    plan.validate()?;
    let subscriptions = vec![
        "vda5050/v3/lsmart-lab/+/+".to_owned(),
        plan.control_topic.clone(),
    ];
    let client = LiveMqttClient::connect_isolated(
        &format!("vda5050-lsmart-recorder-{}", plan.run_id),
        &plan.broker_host,
        plan.broker_port,
        &subscriptions,
        None,
    )?;
    let mut wire_writer = LiveEventWriter::create_new(
        &cli.output_dir.join("wire-events.jsonl"),
        plan.message_limit.saturating_add(16),
    )?;
    write_new_artifact(
        &cli.output_dir.join("recorder-ready.json"),
        &serde_json::to_vec_pretty(&json!({
            "run_id": plan.run_id,
            "subscription_ready": true,
            "source": "official-lsmart"
        }))?,
    )?;

    let deadline = Instant::now() + Duration::from_secs(plan.duration_seconds);
    let mut captures = Vec::new();
    let mut wire_sequence = 1_u64;
    loop {
        if Instant::now() >= deadline {
            return Err("LSMART recorder duration hard limit exceeded".into());
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
        wire_sequence = wire_sequence.saturating_add(1);
        if incoming.topic == plan.control_topic {
            if payload["complete"] == true && payload["run_id"] == plan.run_id {
                break;
            }
            return Err("LSMART recorder observed an invalid completion message".into());
        }
        if !plan.topic_allowed(&incoming.topic) {
            return Err(format!(
                "LSMART recorder observed unexpected topic: {}",
                incoming.topic
            )
            .into());
        }
        if captures.len() >= plan.message_limit {
            return Err("LSMART recorder message hard limit exceeded".into());
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
    let artifacts = build_lsmart_capture_artifacts(&plan, &captures)?;
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
            "run_id": plan.run_id,
            "capture_closed": true,
            "message_count": captures.len(),
            "source_trace_sha256": artifacts.evidence.source_trace_sha256,
            "source": "official-lsmart"
        }))?,
    )?;
    client.shutdown();
    Ok(())
}
