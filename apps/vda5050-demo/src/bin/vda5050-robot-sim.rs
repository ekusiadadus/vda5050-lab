use std::{
    path::PathBuf,
    process::ExitCode,
    thread,
    time::{Duration, Instant},
};

use clap::Parser;
use serde_json::Value;
use vda5050_demo::{
    LiveEvent, LiveEventWriter, LiveMqttClient, LivePhase, LiveRobotPlan, LiveRunPlan,
    LiveSimSnapshot, build_live_connection, build_live_state, build_live_visualization,
    read_live_plan,
};
use vda5050_sim_core::{MotionLimits, Pose2, Robot, RobotGeometry, RobotMode, Route, Waypoint};

#[derive(Debug, Parser)]
#[command(name = "vda5050-robot-sim")]
#[command(about = "Deterministic two-robot kinematic simulator for the isolated live demo")]
struct Cli {
    #[arg(long, default_value = "/artifacts/run-manifest.json")]
    plan: PathBuf,
    #[arg(long, default_value = "/artifacts")]
    output_dir: PathBuf,
    #[arg(long, default_value_t = 50)]
    tick_delay_ms: u64,
}

struct Runtime {
    plan: LiveRobotPlan,
    robot: Robot,
    client: Option<LiveMqttClient>,
    connection_epoch: String,
    state_header: u64,
    visualization_header: u64,
    reference_state_header: u64,
    fault_injected: bool,
    reconnect_at_tick: Option<u64>,
    final_state_sent: bool,
}

fn main() -> ExitCode {
    match run(&Cli::parse()) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("vda5050-robot-sim: {error}");
            ExitCode::FAILURE
        }
    }
}

fn run(cli: &Cli) -> Result<(), Box<dyn std::error::Error>> {
    let plan = read_live_plan(&cli.plan)?;
    let mut event_writer = LiveEventWriter::create_new(
        &cli.output_dir.join("sim-events.jsonl"),
        usize::try_from(plan.hard_limits().simulation_ticks * 2 + 32).unwrap_or(usize::MAX),
    )?;
    let mut event_sequence = 1_u64;
    event_writer.append(&LiveEvent::phase(event_sequence, LivePhase::Starting))?;
    event_sequence += 1;
    let mut runtimes = connect_robots(&plan)?;
    event_writer.append(&LiveEvent::phase(event_sequence, LivePhase::RobotsOnline))?;
    event_sequence += 1;
    wait_for_orders(&plan, &mut runtimes)?;
    event_writer.append(&LiveEvent::phase(event_sequence, LivePhase::OrderPublished))?;
    event_sequence += 1;
    simulate(cli, &plan, runtimes, event_writer, event_sequence)
}

fn connect_robots(plan: &LiveRunPlan) -> Result<Vec<Runtime>, Box<dyn std::error::Error>> {
    plan.robots()
        .iter()
        .map(|robot_plan| {
            let client_id = plan
                .client_ids()
                .get(&format!("agv.{}", robot_plan.serial_number))
                .ok_or("live plan has no robot ClientId")?;
            let broken = build_live_connection(robot_plan, 2, "CONNECTION_BROKEN", "session-1");
            let subscription = vec![format!("{}/order", robot_plan.topic_prefix)];
            let client = LiveMqttClient::connect(client_id, plan, &subscription, Some(&broken))?;
            client.publish(&build_live_connection(robot_plan, 1, "ONLINE", "session-1"))?;
            Ok(Runtime {
                plan: robot_plan.clone(),
                robot: new_robot(robot_plan)?,
                client: Some(client),
                connection_epoch: "session-1".to_owned(),
                state_header: 1,
                visualization_header: 1,
                reference_state_header: 1,
                fault_injected: false,
                reconnect_at_tick: None,
                final_state_sent: false,
            })
        })
        .collect()
}

fn wait_for_orders(
    plan: &LiveRunPlan,
    runtimes: &mut [Runtime],
) -> Result<(), Box<dyn std::error::Error>> {
    let order_deadline = Instant::now() + Duration::from_secs(10);
    for runtime in runtimes {
        wait_for_order(runtime, order_deadline)?;
        let snapshot = live_snapshot(runtime, 0, true);
        runtime
            .client
            .as_ref()
            .ok_or("robot MQTT client is missing")?
            .publish(&build_live_state(
                plan,
                &runtime.plan,
                runtime.state_header,
                &snapshot,
                false,
            ))?;
    }
    Ok(())
}

fn wait_for_order(runtime: &Runtime, deadline: Instant) -> Result<(), Box<dyn std::error::Error>> {
    loop {
        if Instant::now() >= deadline {
            return Err("timed out waiting for VDA order".into());
        }
        let Some(incoming) = runtime
            .client
            .as_ref()
            .ok_or("robot MQTT client is missing")?
            .recv_timeout(Duration::from_millis(100))?
        else {
            continue;
        };
        let payload: Value = serde_json::from_slice(&incoming.payload)?;
        if payload["orderId"] == runtime.plan.order_id {
            return Ok(());
        }
    }
}

fn simulate(
    cli: &Cli,
    plan: &LiveRunPlan,
    mut runtimes: Vec<Runtime>,
    mut event_writer: LiveEventWriter,
    mut event_sequence: u64,
) -> Result<(), Box<dyn std::error::Error>> {
    let tick_hz = f64::from(u32::try_from(plan.tick_hz())?);

    let visualization_divisor = plan.tick_hz() / plan.visualization_hz();
    let state_divisor = plan.tick_hz() / plan.state_hz();
    for tick in 1..=plan.hard_limits().simulation_ticks {
        for runtime in &mut runtimes {
            if runtime.reconnect_at_tick == Some(tick) {
                reconnect(plan, runtime)?;
                event_writer.append(&LiveEvent::phase(
                    event_sequence,
                    if plan.omit_online_after_reconnect() {
                        LivePhase::ReconnectedWithoutOnline
                    } else {
                        LivePhase::ReconnectedOnline
                    },
                ))?;
                event_sequence += 1;
            }
            let step = runtime.robot.step(1.0 / tick_hz, &[])?;
            let connected = runtime.client.is_some();
            let snapshot = live_snapshot(runtime, tick, connected);

            if runtime.plan.serial_number == plan.fault_target()
                && !runtime.fault_injected
                && snapshot.x >= plan.disconnect_x()
            {
                runtime.fault_injected = true;
                runtime.reconnect_at_tick = Some(tick.saturating_add(10));
                if let Some(client) = runtime.client.take() {
                    client.crash();
                }
                event_writer.append(&LiveEvent::phase(
                    event_sequence,
                    LivePhase::ConnectionBroken,
                ))?;
                event_sequence += 1;
            }

            let connected = runtime.client.is_some();
            let snapshot = live_snapshot(runtime, tick, connected);
            event_writer.append(&LiveEvent::sim_snapshot(event_sequence, snapshot.clone()))?;
            event_sequence += 1;
            if let Some(client) = runtime.client.as_ref() {
                if tick % state_divisor == 0 {
                    runtime.state_header += 1;
                    runtime.reference_state_header = runtime.state_header;
                    client.publish(&build_live_state(
                        plan,
                        &runtime.plan,
                        runtime.state_header,
                        &snapshot,
                        false,
                    ))?;
                }
                if tick % visualization_divisor == 0 {
                    runtime.visualization_header += 1;
                    client.publish(&build_live_visualization(
                        plan,
                        &runtime.plan,
                        runtime.visualization_header,
                        runtime.reference_state_header,
                        &snapshot,
                    ))?;
                }
                if step.snapshot.mode == RobotMode::Arrived && !runtime.final_state_sent {
                    runtime.state_header += 1;
                    runtime.reference_state_header = runtime.state_header;
                    client.publish(&build_live_state(
                        plan,
                        &runtime.plan,
                        runtime.state_header,
                        &snapshot,
                        false,
                    ))?;
                    runtime.final_state_sent = true;
                }
            }
        }
        if runtimes.iter().all(|runtime| runtime.final_state_sent) {
            event_writer.append(&LiveEvent::phase(event_sequence, LivePhase::RobotsArrived))?;
            for runtime in runtimes {
                if let Some(client) = runtime.client {
                    client.shutdown();
                }
            }
            return Ok(());
        }
        if cli.tick_delay_ms > 0 {
            thread::sleep(Duration::from_millis(cli.tick_delay_ms));
        }
    }
    Err("simulation tick hard limit exceeded".into())
}

fn new_robot(plan: &LiveRobotPlan) -> Result<Robot, vda5050_sim_core::SimError> {
    let route = Route::new(vec![
        Waypoint::new("released-start", plan.start.x, plan.start.y),
        Waypoint::new("released-end", plan.released_end.x, plan.released_end.y),
    ])?;
    Robot::new(
        plan.serial_number.clone(),
        Pose2::new(plan.start.x, plan.start.y, 0.0)?,
        RobotGeometry::new(0.8, 0.5)?,
        MotionLimits::new(1.0, 1.5, 0.8, 2.0, 0.01, 0.01)?,
        route,
    )
}

fn reconnect(plan: &LiveRunPlan, runtime: &mut Runtime) -> Result<(), Box<dyn std::error::Error>> {
    "session-2".clone_into(&mut runtime.connection_epoch);
    let client_id = plan
        .client_ids()
        .get(&format!("agv.{}", runtime.plan.serial_number))
        .ok_or("live plan has no robot ClientId")?;
    let broken = build_live_connection(&runtime.plan, 4, "CONNECTION_BROKEN", "session-2");
    let client = LiveMqttClient::connect(client_id, plan, &[], Some(&broken))?;
    if !plan.omit_online_after_reconnect() {
        client.publish(&build_live_connection(
            &runtime.plan,
            3,
            "ONLINE",
            "session-2",
        ))?;
    }
    runtime.client = Some(client);
    Ok(())
}

fn live_snapshot(runtime: &Runtime, tick: u64, connected: bool) -> LiveSimSnapshot {
    let snapshot = runtime.robot.snapshot();
    LiveSimSnapshot {
        robot_id: runtime.plan.serial_number.clone(),
        simulation_ns: tick.saturating_mul(50_000_000),
        x: snapshot.pose.x,
        y: snapshot.pose.y,
        theta: snapshot.pose.theta,
        linear_velocity: snapshot.twist.linear,
        angular_velocity: snapshot.twist.angular,
        motion_state: format!("{:?}", snapshot.mode).to_ascii_uppercase(),
        transport_connected: connected,
        connection_epoch: runtime.connection_epoch.clone(),
    }
}
