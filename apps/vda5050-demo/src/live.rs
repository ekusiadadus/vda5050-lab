use std::{
    collections::{BTreeMap, BTreeSet},
    fmt::Write as _,
    fs,
    path::PathBuf,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    thread,
    time::{Duration, Instant},
};

use rumqttc::{Client, Event, LastWill, MqttOptions, Packet, QoS};
use serde::Serialize;
use serde_json::Value;
use sha2::{Digest, Sha256};
use thiserror::Error;
use vda5050_core::{
    ActorRole, CaptureClosed, CaptureOpened, CapturePoint, CaptureRecord, ClockDescriptor,
    ClockWrapPolicy, MessageObserved, PayloadReference,
};
use vda5050_doctor::{CaptureCompleteness, EventEvidence, SyntheticEvidenceManifest};
use vda5050_import::{ImportConfig, ImportFormat, import_path};

use super::{
    BrokerTarget, DURATION_BUDGET_SECONDS, FleetLayout, FleetLayoutError, PROTOCOL_VERSION,
    Position, QosContract, RobotPlan, WireMessage, connection_message_for, message_budget,
    released_base_order_for, render_ascii_frame, state_message_for, visualization_message_for,
};

#[derive(Debug, Clone)]
pub struct RunOptions {
    pub broker: BrokerTarget,
    pub port: u16,
    pub output_dir: PathBuf,
    pub run_id: String,
    pub robot_count: usize,
    pub animate: bool,
    pub send_online_after_reconnect: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunArtifacts {
    pub trace: PathBuf,
    pub evidence_manifest: PathBuf,
    pub run_manifest: PathBuf,
}

#[derive(Debug, Error)]
pub enum RunError {
    #[error("broker port must be non-zero")]
    Port,
    #[error("run ID must contain only ASCII letters, digits, '-' or '_' and be 1..32 bytes")]
    RunId,
    #[error(transparent)]
    FleetLayout(#[from] FleetLayoutError),
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("MQTT error: {0}")]
    Mqtt(String),
    #[error("timed out waiting for {0}")]
    Timeout(&'static str),
    #[error("simulator did not receive its VDA order")]
    MissingOrder,
    #[error("fixed scenario contract is missing {0}")]
    Scenario(&'static str),
    #[error("message hard budget exceeded")]
    MessageBudget,
    #[error("duration hard budget exceeded")]
    DurationBudget,
    #[error("generated trace could not be imported: {0}")]
    Import(String),
}

#[derive(Debug, Serialize)]
#[allow(
    clippy::struct_excessive_bools,
    reason = "the resolved run manifest keeps independent safety assertions explicit"
)]
struct ResolvedRunManifest<'a> {
    schema: &'static str,
    run_id: &'a str,
    protocol_version: &'static str,
    scenario: &'static str,
    robot_count: usize,
    synthetic: bool,
    same_job_isolated: bool,
    target: TargetManifest<'a>,
    coordinate_system: CoordinateSystem,
    robots: Vec<RobotManifest>,
    client_ids: BTreeMap<String, String>,
    topic_allowlist: Vec<String>,
    retain_allowlist: Vec<String>,
    hard_limits: HardLimits,
    fault: FaultManifest,
    connect_is_side_effect: bool,
    physical_dut_authorized: bool,
}

#[derive(Debug, Serialize)]
struct TargetManifest<'a> {
    host: &'a str,
    port: u16,
    network_boundary: &'static str,
}

#[derive(Debug, Serialize)]
struct HardLimits {
    messages: usize,
    duration_seconds: u64,
    actors: usize,
    mqtt_connections: usize,
}

#[derive(Debug, Serialize)]
struct FaultManifest {
    id: &'static str,
    target_serial_number: &'static str,
    abrupt_disconnects: u8,
    omit_online_after_reconnect: bool,
    delay_ms: u64,
    duplicate_count: u8,
    drop_count: u8,
    reorder_window: u8,
}

#[derive(Debug, Clone)]
struct ResolvedClientIds {
    fleet: String,
    recorder: String,
    agvs: BTreeMap<String, String>,
}

#[derive(Debug, Serialize)]
struct CoordinateSystem {
    map_id: &'static str,
    unit: &'static str,
    origin: &'static str,
    x_axis: &'static str,
    y_axis: &'static str,
}

#[derive(Debug, Serialize)]
struct RobotManifest {
    serial_number: String,
    client_id: String,
    topic_prefix: String,
    order_id: String,
    route: RouteManifest,
}

#[derive(Debug, Serialize)]
struct RouteManifest {
    start: Position,
    released_end: Position,
    horizon_end: Position,
}

#[derive(Debug, Clone)]
struct CapturedPublish {
    topic: String,
    payload: Vec<u8>,
    qos: u8,
    duplicate: bool,
    monotonic_ns: u64,
}

#[derive(Debug)]
struct CaptureBuffer {
    publishes: Vec<CapturedPublish>,
    exceeded: bool,
    limit: usize,
}

impl CaptureBuffer {
    fn new(limit: usize) -> Self {
        Self {
            publishes: Vec::new(),
            exceeded: false,
            limit,
        }
    }

    fn push(&mut self, publish: CapturedPublish) {
        if self.publishes.len() >= self.limit {
            self.exceeded = true;
        } else {
            self.publishes.push(publish);
        }
    }
}

struct RunningClient {
    client: Option<Client>,
    stop: Arc<AtomicBool>,
    thread: Option<thread::JoinHandle<()>>,
}

struct AgvRuntime {
    robot: RobotPlan,
    client: Option<RunningClient>,
    order_rx: mpsc::Receiver<Value>,
}

struct ClientStart<'a> {
    client_id: &'a str,
    broker: &'a BrokerTarget,
    port: u16,
    subscription: Option<&'a str>,
    last_will: Option<&'a WireMessage>,
    captures: Option<Arc<Mutex<CaptureBuffer>>>,
    inbox: Option<mpsc::Sender<Value>>,
    clock_origin: Instant,
}

impl RunningClient {
    fn start(start: ClientStart<'_>) -> Result<Self, RunError> {
        let mut options = MqttOptions::new(start.client_id, start.broker.host(), start.port);
        options
            .set_keep_alive(Duration::from_secs(2))
            .set_clean_session(true);
        if let Some(will) = start.last_will {
            options.set_last_will(LastWill::new(
                will.topic(),
                serde_json::to_vec(will.payload())?,
                mqtt_qos(will.qos()),
                will.retain(),
            ));
        }
        let (client, mut connection) = Client::new(options, 32);
        if let Some(topic) = start.subscription {
            client
                .subscribe(topic, QoS::AtLeastOnce)
                .map_err(|error| RunError::Mqtt(error.to_string()))?;
        }
        let stop = Arc::new(AtomicBool::new(false));
        let thread_stop = Arc::clone(&stop);
        let (ready_tx, ready_rx) = mpsc::channel();
        let has_subscription = start.subscription.is_some();
        let captures = start.captures;
        let inbox = start.inbox;
        let clock_origin = start.clock_origin;
        let thread = thread::spawn(move || {
            let mut ready_sent = false;
            while !thread_stop.load(Ordering::Relaxed) {
                match connection.recv_timeout(Duration::from_millis(100)) {
                    Ok(Ok(Event::Incoming(Packet::ConnAck(_)))) if !has_subscription => {
                        if !ready_sent {
                            let _ = ready_tx.send(());
                            ready_sent = true;
                        }
                    }
                    Ok(Ok(Event::Incoming(Packet::SubAck(_)))) if has_subscription => {
                        if !ready_sent {
                            let _ = ready_tx.send(());
                            ready_sent = true;
                        }
                    }
                    Ok(Ok(Event::Incoming(Packet::Publish(publish)))) => {
                        let payload = publish.payload.to_vec();
                        if let Some(ref captured) = captures {
                            let item = CapturedPublish {
                                topic: publish.topic.clone(),
                                payload: payload.clone(),
                                qos: match publish.qos {
                                    QoS::AtMostOnce => 0,
                                    QoS::AtLeastOnce => 1,
                                    QoS::ExactlyOnce => 2,
                                },
                                duplicate: publish.dup,
                                monotonic_ns: u64::try_from(clock_origin.elapsed().as_nanos())
                                    .unwrap_or(u64::MAX),
                            };
                            if let Ok(mut guard) = captured.lock() {
                                guard.push(item);
                            }
                        }
                        if let Some(ref sender) = inbox
                            && let Ok(value) = serde_json::from_slice(&payload)
                        {
                            let _ = sender.send(value);
                        }
                    }
                    Ok(Ok(_)) | Err(rumqttc::RecvTimeoutError::Timeout) => {}
                    Ok(Err(_)) | Err(rumqttc::RecvTimeoutError::Disconnected) => break,
                }
            }
        });
        ready_rx
            .recv_timeout(Duration::from_secs(5))
            .map_err(|_| RunError::Timeout("MQTT CONNACK/SUBACK"))?;
        Ok(Self {
            client: Some(client),
            stop,
            thread: Some(thread),
        })
    }

    fn publish(&self, message: &WireMessage) -> Result<(), RunError> {
        let client = self
            .client
            .as_ref()
            .ok_or_else(|| RunError::Mqtt("client is no longer running".to_owned()))?;
        client
            .publish(
                message.topic(),
                mqtt_qos(message.qos()),
                message.retain(),
                serde_json::to_vec(message.payload())?,
            )
            .map_err(|error| RunError::Mqtt(error.to_string()))
    }

    fn crash(mut self) {
        self.client.take();
        self.stop.store(true, Ordering::Relaxed);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }

    fn shutdown(mut self) {
        if let Some(client) = self.client.take() {
            let _ = client.disconnect();
        }
        thread::sleep(Duration::from_millis(100));
        self.stop.store(true, Ordering::Relaxed);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

/// Runs the fixed Tier 1 reconnect scenario and writes its evidence artifacts.
///
/// # Errors
///
/// Returns an error when safety options are invalid, MQTT setup or the fixed
/// scenario fails, a hard budget is exceeded, or an artifact cannot be written
/// and imported.
#[allow(
    clippy::too_many_lines,
    reason = "the safety-critical scenario remains linear so reviewers can audit every side effect"
)]
pub fn run_live(options: &RunOptions) -> Result<RunArtifacts, RunError> {
    validate_options(options)?;
    let layout = FleetLayout::new(options.robot_count)?;
    let capture_budget = message_budget(options.robot_count)?;
    fs::create_dir_all(&options.output_dir)?;
    let run_manifest_path = options.output_dir.join("run-manifest.json");
    let trace_path = options.output_dir.join("trace.canonical.jsonl");
    let evidence_path = options.output_dir.join("synthetic-evidence.json");

    let client_ids = resolved_client_ids(&layout, &options.run_id);
    let run_manifest = resolved_manifest(options, &layout, &client_ids, capture_budget);
    fs::write(
        &run_manifest_path,
        format!("{}\n", serde_json::to_string_pretty(&run_manifest)?),
    )?;

    let start = Instant::now();
    let captures = Arc::new(Mutex::new(CaptureBuffer::new(capture_budget)));
    let recorder_subscription = "vda5050/v3/lab-demo/#";
    let recorder = RunningClient::start(ClientStart {
        client_id: &client_ids.recorder,
        broker: &options.broker,
        port: options.port,
        subscription: Some(recorder_subscription),
        last_will: None,
        captures: Some(Arc::clone(&captures)),
        inbox: None,
        clock_origin: start,
    })?;

    let fleet = RunningClient::start(ClientStart {
        client_id: &client_ids.fleet,
        broker: &options.broker,
        port: options.port,
        subscription: None,
        last_will: None,
        captures: None,
        inbox: None,
        clock_origin: start,
    })?;

    let mut agvs = Vec::with_capacity(layout.robot_count());
    for robot in layout.robots() {
        let broken = connection_message_for(robot, 2, "CONNECTION_BROKEN", "session-1");
        let (order_tx, order_rx) = mpsc::channel();
        let order_subscription = format!("{}/order", robot.topic_prefix);
        let client_id = client_ids
            .agvs
            .get(&robot.serial_number)
            .ok_or(RunError::Scenario("resolved AGV client ID"))?;
        let client = RunningClient::start(ClientStart {
            client_id,
            broker: &options.broker,
            port: options.port,
            subscription: Some(&order_subscription),
            last_will: Some(&broken),
            captures: None,
            inbox: Some(order_tx),
            clock_origin: start,
        })?;
        agvs.push(AgvRuntime {
            robot: robot.clone(),
            client: Some(client),
            order_rx,
        });
    }

    for runtime in &agvs {
        runtime
            .client
            .as_ref()
            .ok_or(RunError::Scenario("AGV session"))?
            .publish(&connection_message_for(
                &runtime.robot,
                1,
                "ONLINE",
                "session-1",
            ))?;
        fleet.publish(&released_base_order_for(&runtime.robot))?;
    }
    for runtime in &agvs {
        let received = runtime
            .order_rx
            .recv_timeout(Duration::from_secs(5))
            .map_err(|_| RunError::MissingOrder)?;
        if received["orderId"].as_str() != Some(&runtime.robot.order_id) {
            return Err(RunError::MissingOrder);
        }
    }
    for runtime in &agvs {
        runtime
            .client
            .as_ref()
            .ok_or(RunError::Scenario("AGV session"))?
            .publish(&state_message_for(
                &runtime.robot,
                1,
                runtime.robot.start,
                true,
                false,
                &[],
                "session-1",
            ))?;
    }
    for step in 1_i32..=4 {
        for runtime in &agvs {
            let position = Position {
                x: runtime.robot.start.x + step,
                y: runtime.robot.start.y,
            };
            runtime
                .client
                .as_ref()
                .ok_or(RunError::Scenario("AGV session"))?
                .publish(&visualization_message_for(
                    &runtime.robot,
                    u64::try_from(step).unwrap_or(u64::MAX),
                    1,
                    position,
                    "session-1",
                ))?;
        }
        if options.animate {
            print!("\x1b[2J\x1b[H{}", render_fleet_frame(&layout, step, false));
        }
        thread::sleep(Duration::from_millis(150));
    }
    for runtime in &agvs {
        runtime
            .client
            .as_ref()
            .ok_or(RunError::Scenario("AGV session"))?
            .publish(&state_message_for(
                &runtime.robot,
                2,
                runtime.robot.released_end,
                false,
                false,
                &[],
                "session-1",
            ))?;
    }
    thread::sleep(Duration::from_millis(250));
    let victim = agvs
        .first_mut()
        .and_then(|runtime| runtime.client.take())
        .ok_or(RunError::Scenario("fault target demo-001"))?;
    victim.crash();
    wait_for_capture(&captures, "CONNECTION_BROKEN", Duration::from_secs(5))?;

    let victim_robot = layout
        .robots()
        .first()
        .ok_or(RunError::Scenario("fault target demo-001"))?;
    let broken = connection_message_for(victim_robot, 2, "CONNECTION_BROKEN", "session-2");
    let victim_client_id = client_ids
        .agvs
        .get(&victim_robot.serial_number)
        .ok_or(RunError::Scenario("fault target client ID"))?;
    let agv_session_2 = RunningClient::start(ClientStart {
        client_id: victim_client_id,
        broker: &options.broker,
        port: options.port,
        subscription: None,
        last_will: Some(&broken),
        captures: None,
        inbox: None,
        clock_origin: start,
    })?;
    if options.send_online_after_reconnect {
        let online_session_2 = connection_message_for(victim_robot, 3, "ONLINE", "session-2");
        agv_session_2.publish(&online_session_2)?;
    }
    let after_reconnect = state_message_for(
        victim_robot,
        3,
        victim_robot.released_end,
        false,
        true,
        &[],
        "session-2",
    );
    agv_session_2.publish(&after_reconnect)?;
    if options.animate {
        print!(
            "\x1b[2J\x1b[H{}",
            render_fleet_frame(&layout, 4, !options.send_online_after_reconnect)
        );
    }
    let expected_captures = options.robot_count * 8
        + if options.send_online_after_reconnect {
            3
        } else {
            2
        };
    wait_for_publish_count(&captures, expected_captures, Duration::from_secs(10))?;

    agv_session_2.shutdown();
    for runtime in agvs {
        if let Some(client) = runtime.client {
            client.shutdown();
        }
    }
    fleet.shutdown();
    thread::sleep(Duration::from_millis(250));
    recorder.shutdown();

    if start.elapsed() > Duration::from_secs(DURATION_BUDGET_SECONDS) {
        return Err(RunError::DurationBudget);
    }
    let capture_guard = captures
        .lock()
        .map_err(|_| RunError::Mqtt("capture mutex poisoned".to_owned()))?;
    if capture_guard.exceeded {
        return Err(RunError::MessageBudget);
    }
    let captured_publishes = capture_guard.publishes.clone();
    drop(capture_guard);
    let (trace_bytes, evidence_by_index) = canonical_trace(&options.run_id, &captured_publishes)?;
    fs::write(&trace_path, &trace_bytes)?;
    let imported = import_path(
        &trace_path,
        ImportFormat::CanonicalJsonl,
        ImportConfig::default(),
    )
    .map_err(|error| RunError::Import(error.to_string()))?;
    let evidence = SyntheticEvidenceManifest {
        schema: "vda5050-lab.synthetic-evidence/1".to_owned(),
        source_trace_sha256: imported.source_digest,
        synthetic: true,
        same_job_isolated: options.broker.isolated_namespace(),
        capture_closed: true,
        completeness: CaptureCompleteness::ConfirmedForRule,
        role_attribution_complete: true,
        events: evidence_by_index,
    };
    fs::write(
        &evidence_path,
        format!("{}\n", serde_json::to_string_pretty(&evidence)?),
    )?;

    Ok(RunArtifacts {
        trace: trace_path,
        evidence_manifest: evidence_path,
        run_manifest: run_manifest_path,
    })
}

fn validate_options(options: &RunOptions) -> Result<(), RunError> {
    if options.port == 0 {
        return Err(RunError::Port);
    }
    if options.run_id.is_empty()
        || options.run_id.len() > 32
        || !options
            .run_id
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
    {
        return Err(RunError::RunId);
    }
    FleetLayout::new(options.robot_count)?;
    message_budget(options.robot_count)?;
    Ok(())
}

fn resolved_client_ids(layout: &FleetLayout, run_id: &str) -> ResolvedClientIds {
    ResolvedClientIds {
        fleet: format!("vda5050-demo-fleet-{run_id}"),
        recorder: format!("vda5050-demo-recorder-{run_id}"),
        agvs: layout
            .robots()
            .iter()
            .map(|robot| {
                (
                    robot.serial_number.clone(),
                    format!("vda5050-demo-{}-{run_id}", robot.serial_number),
                )
            })
            .collect(),
    }
}

fn resolved_manifest<'a>(
    options: &'a RunOptions,
    layout: &FleetLayout,
    client_ids: &ResolvedClientIds,
    capture_budget: usize,
) -> ResolvedRunManifest<'a> {
    let network_boundary = if options.broker.isolated_namespace() {
        "COMPOSE_INTERNAL"
    } else {
        "LOOPBACK_UNVERIFIED"
    };
    ResolvedRunManifest {
        schema: "vda5050-lab.tier1-run/2",
        run_id: &options.run_id,
        protocol_version: PROTOCOL_VERSION,
        scenario: "MULTI_AGV_RECONNECT_XY_V1",
        robot_count: layout.robot_count(),
        synthetic: true,
        same_job_isolated: options.broker.isolated_namespace(),
        target: TargetManifest {
            host: options.broker.host(),
            port: options.port,
            network_boundary,
        },
        coordinate_system: CoordinateSystem {
            map_id: "warehouse-demo",
            unit: "m",
            origin: "lower-left",
            x_axis: "right",
            y_axis: "up",
        },
        robots: layout
            .robots()
            .iter()
            .map(|robot| RobotManifest {
                serial_number: robot.serial_number.clone(),
                client_id: client_ids.agvs[&robot.serial_number].clone(),
                topic_prefix: robot.topic_prefix.clone(),
                order_id: robot.order_id.clone(),
                route: RouteManifest {
                    start: robot.start,
                    released_end: robot.released_end,
                    horizon_end: robot.horizon_end,
                },
            })
            .collect(),
        client_ids: std::iter::once(("fleet".to_owned(), client_ids.fleet.clone()))
            .chain(std::iter::once((
                "recorder".to_owned(),
                client_ids.recorder.clone(),
            )))
            .chain(
                client_ids
                    .agvs
                    .iter()
                    .map(|(serial, client)| (format!("agv.{serial}"), client.clone())),
            )
            .collect(),
        topic_allowlist: layout
            .robots()
            .iter()
            .flat_map(|robot| {
                ["connection", "order", "state", "visualization"]
                    .into_iter()
                    .map(move |topic| format!("{}/{topic}", robot.topic_prefix))
            })
            .collect(),
        retain_allowlist: layout
            .robots()
            .iter()
            .map(|robot| format!("{}/connection", robot.topic_prefix))
            .collect(),
        hard_limits: HardLimits {
            messages: capture_budget,
            duration_seconds: DURATION_BUDGET_SECONDS,
            actors: layout.robot_count() + 2,
            mqtt_connections: layout.robot_count() + 2,
        },
        fault: FaultManifest {
            id: "RECONNECT_WITHOUT_ONLINE",
            target_serial_number: "demo-001",
            abrupt_disconnects: 1,
            omit_online_after_reconnect: !options.send_online_after_reconnect,
            delay_ms: 0,
            duplicate_count: 0,
            drop_count: 0,
            reorder_window: 0,
        },
        connect_is_side_effect: true,
        physical_dut_authorized: false,
    }
}

fn wait_for_capture(
    captures: &Arc<Mutex<CaptureBuffer>>,
    state: &'static str,
    timeout: Duration,
) -> Result<(), RunError> {
    let deadline = Instant::now() + timeout;
    while Instant::now() < deadline {
        let observed = captures
            .lock()
            .map_err(|_| RunError::Mqtt("capture mutex poisoned".to_owned()))?
            .publishes
            .iter()
            .any(|capture| {
                serde_json::from_slice::<Value>(&capture.payload)
                    .is_ok_and(|payload| payload["connectionState"].as_str() == Some(state))
            });
        if observed {
            return Ok(());
        }
        thread::sleep(Duration::from_millis(50));
    }
    Err(RunError::Timeout("broker-published LWT"))
}

fn wait_for_publish_count(
    captures: &Arc<Mutex<CaptureBuffer>>,
    expected: usize,
    timeout: Duration,
) -> Result<(), RunError> {
    let deadline = Instant::now() + timeout;
    while Instant::now() < deadline {
        let observed = captures
            .lock()
            .map_err(|_| RunError::Mqtt("capture mutex poisoned".to_owned()))?
            .publishes
            .len();
        if observed >= expected {
            return Ok(());
        }
        thread::sleep(Duration::from_millis(25));
    }
    Err(RunError::Timeout("all expected broker publications"))
}

fn canonical_trace(
    run_id: &str,
    captured: &[CapturedPublish],
) -> Result<(Vec<u8>, BTreeMap<usize, EventEvidence>), RunError> {
    let capture_id = format!("tier1-{run_id}");
    let clock_epoch = format!("clock-{run_id}");
    let opened: CaptureRecord = CaptureOpened::new(
        &capture_id,
        "2026-08-07T00:00:00.000Z",
        Some(ClockDescriptor::new(
            "demo-recorder-monotonic",
            "recorder process start",
            1,
            ClockWrapPolicy::DoesNotWrap,
            &clock_epoch,
            None,
        )),
        true,
    )
    .into();
    let mut records = vec![opened];
    let mut evidence = BTreeMap::new();
    let mut broken_participants = BTreeSet::new();
    for (index, capture) in captured.iter().enumerate() {
        let payload: Value = serde_json::from_slice(&capture.payload)?;
        let is_broken = payload["connectionState"] == "CONNECTION_BROKEN";
        let serial_number = serial_from_topic(&capture.topic)?;
        let actor_role = if capture.topic.ends_with("/order") {
            ActorRole::FleetControl
        } else {
            ActorRole::MobileRobot
        };
        let participant_id = if actor_role == ActorRole::FleetControl {
            "lab-demo/fleet".to_owned()
        } else {
            format!("lab-demo/{serial_number}")
        };
        let connection_epoch = if actor_role == ActorRole::FleetControl {
            "fleet-session-1"
        } else if broken_participants.contains(serial_number) {
            "session-2"
        } else {
            "session-1"
        };
        let record_index = index + 1;
        let observed = MessageObserved::new(
            format!("event-{record_index:03}"),
            &capture_id,
            CapturePoint::BrokerEgress,
            Some(u64::try_from(record_index).unwrap_or(u64::MAX)),
            &capture.topic,
            PayloadReference::inline(capture.payload.clone()),
        )
        .with_observed_delivery(Some(capture.qos), Some(capture.duplicate))
        .with_time(
            payload["timestamp"].as_str().map(ToOwned::to_owned),
            Some(capture.monotonic_ns),
            Some("demo-recorder-monotonic".to_owned()),
            Some(clock_epoch.clone()),
        );
        records.push(observed.into());
        evidence.insert(
            record_index,
            EventEvidence {
                actor_role,
                participant_id,
                participant_connection_epoch: connection_epoch.to_owned(),
            },
        );
        if is_broken {
            broken_participants.insert(serial_number.to_owned());
        }
    }
    let stream = serde_json::to_vec(&records)?;
    let closed: CaptureRecord = CaptureClosed::new(
        &capture_id,
        "2026-08-07T00:00:30.000Z",
        u64::try_from(records.len() + 1).unwrap_or(u64::MAX),
        hex::encode(Sha256::digest(&stream)),
        true,
    )
    .into();
    records.push(closed);
    let mut bytes = Vec::new();
    for record in records {
        bytes.extend_from_slice(&serde_json::to_vec(&record)?);
        bytes.push(b'\n');
    }
    Ok((bytes, evidence))
}

fn serial_from_topic(topic: &str) -> Result<&str, RunError> {
    let mut levels = topic.split('/');
    match (
        levels.next(),
        levels.next(),
        levels.next(),
        levels.next(),
        levels.next(),
        levels.next(),
        levels.next(),
    ) {
        (Some("vda5050"), Some("v3"), Some("lab-demo"), Some(serial), Some(_), None, None)
            if serial.starts_with("demo-") =>
        {
            Ok(serial)
        }
        _ => Err(RunError::Scenario("canonical VDA topic identity")),
    }
}

fn render_fleet_frame(layout: &FleetLayout, step: i32, incident: bool) -> String {
    if layout.robot_count() == 1 {
        let robot = &layout.robots()[0];
        let position = Position {
            x: robot.start.x + step,
            y: robot.start.y,
        };
        let mut frame = render_ascii_frame(position, incident);
        writeln!(
            frame,
            "robot=demo-001 x={}m y={}m fleet=1",
            position.x, position.y
        )
        .expect("writing to a String cannot fail");
        return frame;
    }

    let mut output = format!(
        "VDA 5050 3.0 isolated 2D fleet · robots={} · map=warehouse-demo · unit=m\n",
        layout.robot_count()
    );
    output.push_str("rows are y=18..0; columns are x-lane groups\n");
    for row in (0..10).rev() {
        let y = row * 2;
        write!(output, "y={y:02} ").expect("writing to a String cannot fail");
        for column in 0..10 {
            let index = column * 10 + row;
            if index < layout.robot_count() {
                let marker = if incident && index == 0 { '!' } else { 'R' };
                output.push(marker);
                output.push(' ');
            } else {
                output.push_str("· ");
            }
        }
        output.push('\n');
    }
    writeln!(
        output,
        "step={step} · demo-001 x={}m y=0m{}",
        step,
        if incident {
            " · reconnect missing ONLINE"
        } else {
            ""
        }
    )
    .expect("writing to a String cannot fail");
    output
}

const fn mqtt_qos(qos: QosContract) -> QoS {
    match qos {
        QosContract::AtMostOnce => QoS::AtMostOnce,
        QosContract::AtLeastOnce => QoS::AtLeastOnce,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn options(host: &str, isolated: bool) -> RunOptions {
        RunOptions {
            broker: BrokerTarget::new(host, isolated).expect("test broker target is valid"),
            port: 1883,
            output_dir: PathBuf::from("unused"),
            run_id: "safety-test".to_owned(),
            robot_count: 1,
            animate: false,
            send_online_after_reconnect: false,
        }
    }

    #[test]
    fn same_job_evidence_is_never_claimed_for_loopback_developer_mode() {
        let options = options("127.0.0.1", false);
        let layout = FleetLayout::new(options.robot_count).expect("valid layout");
        let clients = resolved_client_ids(&layout, &options.run_id);
        let manifest = resolved_manifest(
            &options,
            &layout,
            &clients,
            message_budget(options.robot_count).expect("valid budget"),
        );

        assert!(!manifest.same_job_isolated);
        assert_eq!(manifest.target.network_boundary, "LOOPBACK_UNVERIFIED");
    }

    #[test]
    fn compose_internal_target_can_claim_same_job_isolation() {
        let options = options("broker", true);
        let layout = FleetLayout::new(options.robot_count).expect("valid layout");
        let clients = resolved_client_ids(&layout, &options.run_id);
        let manifest = resolved_manifest(
            &options,
            &layout,
            &clients,
            message_budget(options.robot_count).expect("valid budget"),
        );

        assert!(manifest.same_job_isolated);
        assert_eq!(manifest.target.network_boundary, "COMPOSE_INTERNAL");
    }

    #[test]
    fn capture_buffer_fails_closed_instead_of_silently_truncating() {
        let limit = message_budget(1).expect("valid budget");
        let mut buffer = CaptureBuffer::new(limit);
        for sequence in 0..=limit {
            buffer.push(CapturedPublish {
                topic: format!("topic/{sequence}"),
                payload: Vec::new(),
                qos: 0,
                duplicate: false,
                monotonic_ns: u64::try_from(sequence).unwrap_or(u64::MAX),
            });
        }

        assert_eq!(buffer.publishes.len(), limit);
        assert!(buffer.exceeded);
    }

    #[test]
    fn one_hundred_robot_manifest_expands_every_identity_and_xy_route() {
        let mut options = options("broker", true);
        options.robot_count = 100;
        let layout = FleetLayout::new(100).expect("valid layout");
        let clients = resolved_client_ids(&layout, &options.run_id);
        let manifest = resolved_manifest(
            &options,
            &layout,
            &clients,
            message_budget(100).expect("valid budget"),
        );

        assert_eq!(manifest.schema, "vda5050-lab.tier1-run/2");
        assert_eq!(manifest.robot_count, 100);
        assert_eq!(manifest.robots.len(), 100);
        assert_eq!(manifest.topic_allowlist.len(), 400);
        assert_eq!(manifest.hard_limits.messages, 1_016);
        assert_eq!(manifest.hard_limits.actors, 102);
        assert_eq!(manifest.hard_limits.mqtt_connections, 102);
        assert_eq!(manifest.robots[99].route.released_end.x, 58);
        assert_eq!(manifest.robots[99].route.released_end.y, 18);
    }
}
