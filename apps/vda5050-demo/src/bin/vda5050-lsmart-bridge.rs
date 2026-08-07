use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::{Path, PathBuf},
    process::ExitCode,
    thread,
    time::{Duration, Instant},
};

use clap::Parser;
use serde::de::DeserializeOwned;
use serde_json::{Value, json};
use vda5050_demo::{
    LiveEvent, LiveEventWriter, LiveMqttClient, LiveSimSnapshot, LsmartActionBatch, LsmartPose,
    LsmartRobot, LsmartRunPlan, build_lsmart_connection, build_lsmart_delivery, build_lsmart_order,
    build_lsmart_state, build_lsmart_visualization, parse_lsmart_action_batch, write_new_artifact,
};

const MAX_PLAN_BYTES: u64 = 128 * 1_024;
const MAX_BATCH_BYTES: u64 = 256 * 1_024;
const MAX_POSE_BYTES: u64 = 16 * 1_024;

#[derive(Debug, Parser)]
#[command(name = "vda5050-lsmart-bridge")]
#[command(about = "Causal VDA 5050 bridge for the pinned official LSMART simulator")]
struct Cli {
    #[arg(long, default_value = "/artifacts/lsmart-run-manifest.json")]
    plan: PathBuf,
    #[arg(long, default_value = "/bridge")]
    bridge_dir: PathBuf,
    #[arg(long, default_value = "/artifacts")]
    output_dir: PathBuf,
}

struct RobotRuntime {
    robot: LsmartRobot,
    client: Option<LiveMqttClient>,
    connection_epoch: String,
    connection_header: u64,
    state_header: u64,
    visualization_header: u64,
    reference_state_header: u64,
    last_tick: Option<u64>,
    reconnect_at_tick: Option<u64>,
    fault_injected: bool,
}

#[derive(Debug)]
struct PendingBatch {
    batch: LsmartActionBatch,
    published: bool,
}

fn main() -> ExitCode {
    match run(&Cli::parse()) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("vda5050-lsmart-bridge: {error}");
            ExitCode::FAILURE
        }
    }
}

fn run(cli: &Cli) -> Result<(), Box<dyn std::error::Error>> {
    let plan: LsmartRunPlan = read_bounded_json(&cli.plan, MAX_PLAN_BYTES)?;
    plan.validate()?;
    validate_directory(&cli.bridge_dir)?;
    validate_directory(&cli.output_dir)?;
    for child in ["outbox", "inbox", "telemetry"] {
        validate_directory(&cli.bridge_dir.join(child))?;
    }

    let mut event_writer = LiveEventWriter::create_new(
        &cli.output_dir.join("lsmart-sim-events.jsonl"),
        usize::try_from(plan.simulation_ticks.saturating_mul(12).saturating_add(256))
            .unwrap_or(usize::MAX),
    )?;
    let fleet = LiveMqttClient::connect_isolated(
        &format!("vda5050-lsmart-fleet-{}", plan.run_id),
        &plan.broker_host,
        plan.broker_port,
        &[],
        None,
    )?;
    let mut runtimes = plan
        .robots
        .iter()
        .map(|robot| connect_robot(&plan, robot, "lsmart-session-1", true))
        .collect::<Result<Vec<_>, _>>()?;
    write_bridge_ready(&cli.output_dir, &plan.run_id, runtimes.len())?;

    let deadline = Instant::now() + Duration::from_secs(plan.duration_seconds);
    let mut processed_outbox = BTreeSet::new();
    let mut pending = BTreeMap::<String, PendingBatch>::new();
    let mut order_header = BTreeMap::<String, u64>::new();
    let mut event_sequence = 1_u64;
    let mut orders_published = 0_u64;
    let mut deliveries_released = 0_u64;
    let mut pose_samples = 0_u64;

    loop {
        if Instant::now() >= deadline {
            return Err(write_bridge_timeout(
                cli,
                &plan,
                &pending,
                &runtimes,
                orders_published,
                deliveries_released,
                pose_samples,
            )?
            .into());
        }
        receive_deliveries(
            &cli.bridge_dir.join("inbox"),
            &runtimes,
            &mut pending,
            &mut deliveries_released,
        )?;
        manage_faults(&plan, &mut runtimes, &pending)?;
        discover_batches(
            &plan,
            &cli.bridge_dir.join("outbox"),
            &mut processed_outbox,
            &mut pending,
        )?;
        publish_ready_batches(
            &plan,
            &fleet,
            &runtimes,
            &mut pending,
            &mut order_header,
            &mut orders_published,
        )?;
        publish_telemetry(
            &plan,
            &cli.bridge_dir.join("telemetry"),
            &mut runtimes,
            &mut event_writer,
            &mut event_sequence,
            &mut pose_samples,
        )?;

        if cli.bridge_dir.join("complete.json").is_file() && pending.is_empty() {
            fleet.publish_control(
                &plan.control_topic,
                &json!({
                    "run_id": plan.run_id,
                    "complete": true,
                    "orders_published": orders_published,
                    "deliveries_released": deliveries_released,
                    "pose_samples": pose_samples
                }),
            )?;
            break;
        }
        thread::sleep(Duration::from_millis(20));
    }

    finish_bridge(
        cli,
        &plan,
        runtimes,
        fleet,
        orders_published,
        deliveries_released,
        pose_samples,
    )
}

#[allow(clippy::too_many_arguments)]
fn finish_bridge(
    cli: &Cli,
    plan: &LsmartRunPlan,
    runtimes: Vec<RobotRuntime>,
    fleet: LiveMqttClient,
    orders_published: u64,
    deliveries_released: u64,
    pose_samples: u64,
) -> Result<(), Box<dyn std::error::Error>> {
    write_new_artifact(
        &cli.output_dir.join("lsmart-bridge-summary.json"),
        &serde_json::to_vec_pretty(&json!({
            "schema": "vda5050-lab.lsmart-bridge-summary/1",
            "run_id": plan.run_id,
            "causal_mqtt_gate": true,
            "orders_published": orders_published,
            "deliveries_released": deliveries_released,
            "pose_samples": pose_samples,
            "fault_injected": runtimes.iter().any(|runtime| runtime.fault_injected)
        }))?,
    )?;
    for runtime in runtimes {
        if let Some(client) = runtime.client {
            client.shutdown();
        }
    }
    fleet.shutdown();
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn write_bridge_timeout(
    cli: &Cli,
    plan: &LsmartRunPlan,
    pending: &BTreeMap<String, PendingBatch>,
    runtimes: &[RobotRuntime],
    orders_published: u64,
    deliveries_released: u64,
    pose_samples: u64,
) -> Result<String, Box<dyn std::error::Error>> {
    let pending_batch_ids = pending.keys().take(32).cloned().collect::<Vec<_>>();
    let robot_last_ticks = runtimes
        .iter()
        .map(|runtime| (runtime.robot.serial_number.clone(), runtime.last_tick))
        .collect::<BTreeMap<_, _>>();
    let diagnostic = bridge_timeout_report(
        &plan.run_id,
        plan.duration_seconds,
        cli.bridge_dir.join("complete.json").is_file(),
        pending.len(),
        &pending_batch_ids,
        &robot_last_ticks,
        orders_published,
        deliveries_released,
        pose_samples,
    );
    let diagnostic_path = cli.output_dir.join("lsmart-bridge-timeout.json");
    write_new_artifact(&diagnostic_path, &serde_json::to_vec_pretty(&diagnostic)?)?;
    Ok(format!(
        "official LSMART bridge duration hard limit exceeded; \
         complete_marker={}, pending_batches={}, orders_published={}, \
         deliveries_released={}, pose_samples={}; diagnostic={}",
        diagnostic["complete_marker_observed"],
        diagnostic["pending_batches"],
        diagnostic["orders_published"],
        diagnostic["deliveries_released"],
        diagnostic["pose_samples"],
        diagnostic_path.display()
    ))
}

#[allow(clippy::too_many_arguments)]
fn bridge_timeout_report(
    run_id: &str,
    duration_limit_seconds: u64,
    complete_marker_observed: bool,
    pending_batches: usize,
    pending_batch_ids_sample: &[String],
    robot_last_ticks: &BTreeMap<String, Option<u64>>,
    orders_published: u64,
    deliveries_released: u64,
    pose_samples: u64,
) -> Value {
    json!({
        "schema": "vda5050-lab.lsmart-bridge-timeout/1",
        "run_id": run_id,
        "reason": "DURATION_HARD_LIMIT_EXCEEDED",
        "duration_limit_seconds": duration_limit_seconds,
        "complete_marker_observed": complete_marker_observed,
        "pending_batches": pending_batches,
        "pending_batch_ids_sample": pending_batch_ids_sample,
        "robot_last_ticks": robot_last_ticks,
        "orders_published": orders_published,
        "deliveries_released": deliveries_released,
        "pose_samples": pose_samples
    })
}

fn write_bridge_ready(
    output_dir: &Path,
    run_id: &str,
    robot_adapters: usize,
) -> Result<(), Box<dyn std::error::Error>> {
    let ready = json!({
        "schema": "vda5050-lab.lsmart-bridge-ready/1",
        "run_id": run_id,
        "robot_adapters": robot_adapters,
        "subscriptions_ready": true
    });
    write_new_artifact(
        &output_dir.join("lsmart-bridge-ready.json"),
        &serde_json::to_vec_pretty(&ready)?,
    )?;
    Ok(())
}

fn connect_robot(
    plan: &LsmartRunPlan,
    robot: &LsmartRobot,
    epoch: &str,
    publish_online: bool,
) -> Result<RobotRuntime, Box<dyn std::error::Error>> {
    let broken = build_lsmart_connection(
        robot,
        2,
        "CONNECTION_BROKEN",
        "2026-08-07T00:00:15.000Z",
        epoch,
    );
    let subscriptions = vec![format!("{}/order", robot.topic_prefix)];
    let client = LiveMqttClient::connect_isolated(
        &format!(
            "vda5050-lsmart-robot-{}-{}-{epoch}",
            robot.serial_number, plan.run_id
        ),
        &plan.broker_host,
        plan.broker_port,
        &subscriptions,
        Some(&broken),
    )?;
    if publish_online {
        client.publish(&build_lsmart_connection(
            robot,
            1,
            "ONLINE",
            "2026-08-07T00:00:00.000Z",
            epoch,
        ))?;
    }
    Ok(RobotRuntime {
        robot: robot.clone(),
        client: Some(client),
        connection_epoch: epoch.to_owned(),
        connection_header: 2,
        state_header: 0,
        visualization_header: 0,
        reference_state_header: 0,
        last_tick: None,
        reconnect_at_tick: None,
        fault_injected: false,
    })
}

fn discover_batches(
    plan: &LsmartRunPlan,
    outbox: &Path,
    processed: &mut BTreeSet<PathBuf>,
    pending: &mut BTreeMap<String, PendingBatch>,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut paths = fs::read_dir(outbox)?
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| {
            path.extension()
                .is_some_and(|extension| extension == "json")
        })
        .collect::<Vec<_>>();
    paths.sort();
    for path in paths {
        if processed.contains(&path) {
            continue;
        }
        let bytes = read_bounded_complete_json(&path, MAX_BATCH_BYTES)?;
        let batch = parse_lsmart_action_batch(&bytes, &plan.run_id, 128)?;
        if plan.robot(&batch.robot_id).is_none() || pending.contains_key(&batch.batch_id) {
            return Err("LSMART outbox contains an unknown robot or duplicate batch".into());
        }
        processed.insert(path);
        pending.insert(
            batch.batch_id.clone(),
            PendingBatch {
                batch,
                published: false,
            },
        );
    }
    Ok(())
}

fn publish_ready_batches(
    plan: &LsmartRunPlan,
    fleet: &LiveMqttClient,
    runtimes: &[RobotRuntime],
    pending: &mut BTreeMap<String, PendingBatch>,
    headers: &mut BTreeMap<String, u64>,
    published_count: &mut u64,
) -> Result<(), Box<dyn std::error::Error>> {
    for pending_batch in pending.values_mut().filter(|entry| !entry.published) {
        let Some(runtime) = runtimes
            .iter()
            .find(|runtime| runtime.robot.serial_number == pending_batch.batch.robot_id)
        else {
            return Err("pending batch has no robot runtime".into());
        };
        if runtime.client.is_none() {
            continue;
        }
        let header = headers
            .entry(runtime.robot.serial_number.clone())
            .and_modify(|value| *value = value.saturating_add(1))
            .or_insert(1);
        let timestamp = timestamp_for_tick(pending_batch.batch.simulation_tick);
        let order = build_lsmart_order(&runtime.robot, &pending_batch.batch, *header, &timestamp)?;
        fleet.publish(&order)?;
        pending_batch.published = true;
        *published_count = published_count.saturating_add(1);
    }
    if pending.len() > plan.message_limit {
        return Err("pending LSMART action batches exceed the message limit".into());
    }
    Ok(())
}

fn receive_deliveries(
    inbox_dir: &Path,
    runtimes: &[RobotRuntime],
    pending: &mut BTreeMap<String, PendingBatch>,
    delivery_count: &mut u64,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut completed = Vec::new();
    for runtime in runtimes {
        let Some(client) = runtime.client.as_ref() else {
            continue;
        };
        while let Some(incoming) = client.recv_timeout(Duration::ZERO)? {
            if incoming.topic != format!("{}/order", runtime.robot.topic_prefix) {
                return Err("LSMART robot adapter received an unexpected topic".into());
            }
            let order: Value = serde_json::from_slice(&incoming.payload)?;
            let order_id = order["orderId"]
                .as_str()
                .ok_or("delivered LSMART order has no orderId")?;
            let Some((batch_id, entry)) = pending.iter().find(|(_, entry)| {
                entry.batch.robot_id == runtime.robot.serial_number
                    && entry.batch.order_id() == order_id
            }) else {
                return Err("MQTT delivered an order without a pending LSMART batch".into());
            };
            let delivery = build_lsmart_delivery(&entry.batch, &order)?;
            let path = inbox_dir.join(format!("{batch_id}.json"));
            write_new_artifact(&path, &serde_json::to_vec_pretty(&delivery)?)?;
            completed.push(batch_id.clone());
            *delivery_count = delivery_count.saturating_add(1);
        }
    }
    for batch_id in completed {
        pending.remove(&batch_id);
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn publish_telemetry(
    plan: &LsmartRunPlan,
    telemetry_dir: &Path,
    runtimes: &mut [RobotRuntime],
    event_writer: &mut LiveEventWriter,
    event_sequence: &mut u64,
    pose_samples: &mut u64,
) -> Result<(), Box<dyn std::error::Error>> {
    for runtime in runtimes {
        let path = telemetry_dir.join(format!("{}.json", runtime.robot.serial_number));
        if !path.is_file() {
            continue;
        }
        let pose: LsmartPose = read_bounded_json(&path, MAX_POSE_BYTES)?;
        if pose.robot_id != runtime.robot.serial_number
            || runtime
                .last_tick
                .is_some_and(|last_tick| pose.simulation_tick <= last_tick)
        {
            continue;
        }
        runtime.last_tick = Some(pose.simulation_tick);
        *pose_samples = pose_samples.saturating_add(1);
        event_writer.append(&LiveEvent::sim_snapshot(
            *event_sequence,
            LiveSimSnapshot {
                robot_id: pose.robot_id.clone(),
                simulation_ns: pose.simulation_tick.saturating_mul(100_000_000),
                x: pose.x,
                y: pose.y,
                theta: pose.theta,
                linear_velocity: pose.linear_velocity,
                angular_velocity: pose.angular_velocity,
                motion_state: pose.motion_state.clone(),
                transport_connected: runtime.client.is_some(),
                connection_epoch: runtime.connection_epoch.clone(),
            },
        ))?;
        *event_sequence = event_sequence.saturating_add(1);

        let Some(client) = runtime.client.as_ref() else {
            continue;
        };
        let timestamp = timestamp_for_tick(pose.simulation_tick);
        runtime.visualization_header = runtime.visualization_header.saturating_add(1);
        client.publish(&build_lsmart_visualization(
            &runtime.robot,
            &pose,
            runtime.visualization_header,
            runtime.reference_state_header,
            &timestamp,
            &runtime.connection_epoch,
        )?)?;
        if pose.simulation_tick.is_multiple_of(5) {
            runtime.state_header = runtime.state_header.saturating_add(1);
            runtime.reference_state_header = runtime.state_header;
            client.publish(&build_lsmart_state(
                &runtime.robot,
                &pose,
                runtime.state_header,
                &timestamp,
                &runtime.connection_epoch,
            )?)?;
        }
    }
    if *pose_samples > u64::try_from(plan.message_limit).unwrap_or(u64::MAX) {
        return Err("LSMART pose samples exceed the hard limit".into());
    }
    Ok(())
}

fn manage_faults(
    plan: &LsmartRunPlan,
    runtimes: &mut [RobotRuntime],
    pending: &BTreeMap<String, PendingBatch>,
) -> Result<(), Box<dyn std::error::Error>> {
    let Some(runtime) = runtimes
        .iter_mut()
        .find(|runtime| runtime.robot.serial_number == plan.fault_target)
    else {
        return Err("LSMART fault target is absent".into());
    };
    let tick = runtime.last_tick.unwrap_or(0);
    let target_has_pending_batch = pending
        .values()
        .any(|entry| entry.batch.robot_id == runtime.robot.serial_number);
    if should_inject_fault(runtime.fault_injected, tick, target_has_pending_batch) {
        runtime.fault_injected = true;
        runtime.reconnect_at_tick = Some(tick.saturating_add(10));
        if let Some(client) = runtime.client.take() {
            client.crash();
        }
    }
    if runtime.client.is_none()
        && runtime
            .reconnect_at_tick
            .is_some_and(|reconnect_at| tick >= reconnect_at)
    {
        "lsmart-session-2".clone_into(&mut runtime.connection_epoch);
        runtime.connection_header = runtime.connection_header.saturating_add(1);
        let broken = build_lsmart_connection(
            &runtime.robot,
            runtime.connection_header.saturating_add(1),
            "CONNECTION_BROKEN",
            &timestamp_for_tick(tick.saturating_add(150)),
            &runtime.connection_epoch,
        );
        let subscriptions = vec![format!("{}/order", runtime.robot.topic_prefix)];
        let client = LiveMqttClient::connect_isolated(
            &format!(
                "vda5050-lsmart-robot-{}-{}-session-2",
                runtime.robot.serial_number, plan.run_id
            ),
            &plan.broker_host,
            plan.broker_port,
            &subscriptions,
            Some(&broken),
        )?;
        if !plan.inject_reconnect_fault {
            runtime.connection_header = runtime.connection_header.saturating_add(1);
            client.publish(&build_lsmart_connection(
                &runtime.robot,
                runtime.connection_header,
                "ONLINE",
                &timestamp_for_tick(tick),
                &runtime.connection_epoch,
            ))?;
        }
        runtime.client = Some(client);
        runtime.reconnect_at_tick = None;
    }
    Ok(())
}

fn should_inject_fault(fault_injected: bool, tick: u64, target_has_pending_batch: bool) -> bool {
    !fault_injected && tick >= 150 && !target_has_pending_batch
}

fn read_bounded_json<T: DeserializeOwned>(
    path: &Path,
    max_bytes: u64,
) -> Result<T, Box<dyn std::error::Error>> {
    Ok(serde_json::from_slice(&read_bounded_complete_json(
        path, max_bytes,
    )?)?)
}

fn read_bounded_complete_json(
    path: &Path,
    max_bytes: u64,
) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    let mut last_error = None;
    for attempt in 0..10 {
        let bytes = read_bounded(path, max_bytes)?;
        match serde_json::from_slice::<Value>(&bytes) {
            Ok(_) => return Ok(bytes),
            Err(error) => last_error = Some(error),
        }
        if attempt < 9 {
            thread::sleep(Duration::from_millis(2));
        }
    }
    Err(last_error
        .expect("the retry loop always attempts JSON parsing")
        .into())
}

fn read_bounded(path: &Path, max_bytes: u64) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    let metadata = fs::symlink_metadata(path)?;
    if !metadata.is_file() || metadata.file_type().is_symlink() || metadata.len() > max_bytes {
        return Err("bridge input must be a bounded regular file".into());
    }
    Ok(fs::read(path)?)
}

fn validate_directory(path: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let metadata = fs::symlink_metadata(path)?;
    if !metadata.is_dir() || metadata.file_type().is_symlink() {
        return Err("bridge path must be a real directory".into());
    }
    Ok(())
}

fn timestamp_for_tick(tick: u64) -> String {
    let seconds = tick / 10;
    let millis = (tick % 10) * 100;
    format!(
        "2026-08-07T00:{:02}:{:02}.{millis:03}Z",
        seconds / 60,
        seconds % 60
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn timeout_report_preserves_the_exact_bridge_progress() {
        let report = bridge_timeout_report(
            "lsmart-run-0001",
            180,
            true,
            2,
            &["batch-7".to_owned(), "batch-8".to_owned()],
            &BTreeMap::from([("fb0".to_owned(), Some(599)), ("fb1".to_owned(), None)]),
            41,
            39,
            712,
        );

        assert_eq!(report["schema"], "vda5050-lab.lsmart-bridge-timeout/1");
        assert_eq!(report["run_id"], "lsmart-run-0001");
        assert_eq!(report["duration_limit_seconds"], 180);
        assert_eq!(report["complete_marker_observed"], true);
        assert_eq!(report["pending_batches"], 2);
        assert_eq!(
            report["pending_batch_ids_sample"],
            json!(["batch-7", "batch-8"])
        );
        assert_eq!(report["robot_last_ticks"]["fb0"], 599);
        assert!(report["robot_last_ticks"]["fb1"].is_null());
        assert_eq!(report["orders_published"], 41);
        assert_eq!(report["deliveries_released"], 39);
        assert_eq!(report["pose_samples"], 712);
    }

    #[test]
    fn reconnect_fault_waits_until_the_target_has_no_in_flight_order() {
        assert!(!should_inject_fault(false, 150, true));
        assert!(should_inject_fault(false, 150, false));
        assert!(!should_inject_fault(false, 149, false));
        assert!(!should_inject_fault(true, 150, false));
    }

    #[test]
    fn retries_a_cross_mount_json_update_until_it_is_complete() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let path = directory.path().join("pose.json");
        fs::write(&path, br#"{"robot_id":"0""#).expect("partial pose");
        let writer_path = path.clone();
        let writer = thread::spawn(move || {
            thread::sleep(Duration::from_millis(4));
            fs::write(&writer_path, br#"{"robot_id":"0","simulation_tick":1}"#)
                .expect("complete pose");
        });

        let value: Value = read_bounded_json(&path, 1_024).expect("eventually complete JSON");
        writer.join().expect("writer thread");
        assert_eq!(value["simulation_tick"], 1);
    }

    #[test]
    fn rejects_json_that_remains_malformed() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let path = directory.path().join("pose.json");
        fs::write(&path, br#"{"robot_id":"0""#).expect("malformed pose");

        assert!(read_bounded_json::<Value>(&path, 1_024).is_err());
    }
}
