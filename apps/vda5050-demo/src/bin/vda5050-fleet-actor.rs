use std::{
    collections::BTreeSet,
    path::PathBuf,
    process::ExitCode,
    time::{Duration, Instant},
};

use clap::Parser;
use serde_json::{Value, json};
use vda5050_demo::{LiveMqttClient, build_live_order, read_live_plan};

#[derive(Debug, Parser)]
#[command(name = "vda5050-fleet-actor")]
#[command(about = "Fixed Fleet Control actor for the isolated live demo")]
struct Cli {
    #[arg(long, default_value = "/artifacts/run-manifest.json")]
    plan: PathBuf,
}

fn main() -> ExitCode {
    match run(&Cli::parse()) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("vda5050-fleet-actor: {error}");
            ExitCode::FAILURE
        }
    }
}

fn run(cli: &Cli) -> Result<(), Box<dyn std::error::Error>> {
    let plan = read_live_plan(&cli.plan)?;
    let client_id = plan
        .client_ids()
        .get("fleet")
        .ok_or("live plan has no fleet ClientId")?;
    let subscriptions = vec![
        "vda5050/v3/lab-demo/+/connection".to_owned(),
        "vda5050/v3/lab-demo/+/state".to_owned(),
    ];
    let client = LiveMqttClient::connect(client_id, &plan, &subscriptions, None)?;
    let deadline = Instant::now() + Duration::from_secs(plan.hard_limits().duration_seconds);
    let expected = plan
        .robots()
        .iter()
        .map(|robot| robot.serial_number.clone())
        .collect::<BTreeSet<_>>();
    let mut online = BTreeSet::new();
    while online != expected {
        if Instant::now() >= deadline {
            return Err("timed out waiting for both robots to publish ONLINE".into());
        }
        if let Some(incoming) = client.recv_timeout(Duration::from_millis(100))? {
            let payload: Value = serde_json::from_slice(&incoming.payload)?;
            if payload["connectionState"] == "ONLINE"
                && let Some(serial) = payload["serialNumber"].as_str()
                && expected.contains(serial)
            {
                online.insert(serial.to_owned());
            }
        }
    }
    for robot in plan.robots() {
        client.publish(&build_live_order(&plan, robot, 1))?;
    }

    let mut arrived = BTreeSet::new();
    while arrived != expected {
        if Instant::now() >= deadline {
            return Err("timed out waiting for both robots to finish the released edge".into());
        }
        if let Some(incoming) = client.recv_timeout(Duration::from_millis(100))? {
            let payload: Value = serde_json::from_slice(&incoming.payload)?;
            if incoming.topic.ends_with("/state")
                && payload["driving"] == false
                && let Some(serial) = payload["serialNumber"].as_str()
                && payload["lastNodeId"] == format!("B-{serial}")
                && expected.contains(serial)
            {
                arrived.insert(serial.to_owned());
            }
        }
    }
    client.publish_control(
        plan.control_topic(),
        &json!({"run_id": plan.run_id(), "complete": true}),
    )?;
    std::thread::sleep(Duration::from_millis(250));
    client.shutdown();
    Ok(())
}
