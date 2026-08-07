use std::{
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    thread,
    time::{Duration, Instant},
};

use rumqttc::{Client, Event, LastWill, MqttOptions, Packet, QoS};
use serde_json::Value;
use thiserror::Error;

use crate::{BrokerTarget, LiveRunPlan, QosContract, WireMessage};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IncomingPublish {
    pub topic: String,
    pub payload: Vec<u8>,
    pub qos: u8,
    pub duplicate: bool,
    pub monotonic_ns: u64,
}

#[derive(Debug, Error)]
pub enum LiveMqttError {
    #[error("MQTT error: {0}")]
    Mqtt(String),
    #[error("MQTT client did not receive CONNACK/SUBACK within its hard timeout")]
    ReadyTimeout,
    #[error("incoming MQTT channel disconnected")]
    Disconnected,
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
}

pub struct LiveMqttClient {
    client: Option<Client>,
    inbox: mpsc::Receiver<IncomingPublish>,
    stop: Arc<AtomicBool>,
    thread: Option<thread::JoinHandle<()>>,
}

impl LiveMqttClient {
    /// Connects one fixed-plan MQTT participant and waits for all subscription
    /// acknowledgements before returning.
    ///
    /// # Errors
    ///
    /// Returns an error if CONNECT or subscription setup fails or readiness is
    /// not observed within five seconds.
    pub fn connect(
        client_id: &str,
        plan: &LiveRunPlan,
        subscriptions: &[String],
        last_will: Option<&WireMessage>,
    ) -> Result<Self, LiveMqttError> {
        plan.validate()
            .map_err(|error| LiveMqttError::Mqtt(error.to_string()))?;
        Self::connect_isolated(
            client_id,
            plan.broker_host(),
            plan.broker_port(),
            subscriptions,
            last_will,
        )
    }

    /// Connects to loopback or the exact Compose-internal `broker` service.
    ///
    /// # Errors
    ///
    /// Returns an error for any routable target, a non-default port, or when
    /// MQTT readiness is not observed within five seconds.
    pub fn connect_isolated(
        client_id: &str,
        broker_host: &str,
        broker_port: u16,
        subscriptions: &[String],
        last_will: Option<&WireMessage>,
    ) -> Result<Self, LiveMqttError> {
        let isolated_namespace = broker_host == "broker";
        BrokerTarget::new(broker_host, isolated_namespace)
            .map_err(|error| LiveMqttError::Mqtt(error.to_string()))?;
        if broker_port != 1883 {
            return Err(LiveMqttError::Mqtt(
                "isolated demo broker port must be 1883".to_owned(),
            ));
        }
        let mut options = MqttOptions::new(client_id, broker_host, broker_port);
        options
            .set_keep_alive(Duration::from_secs(2))
            .set_clean_session(true);
        if let Some(will) = last_will {
            options.set_last_will(LastWill::new(
                will.topic(),
                serde_json::to_vec(will.payload())?,
                mqtt_qos(will.qos()),
                will.retain(),
            ));
        }
        let (client, mut connection) = Client::new(options, 64);
        for subscription in subscriptions {
            client
                .subscribe(subscription, QoS::AtLeastOnce)
                .map_err(|error| LiveMqttError::Mqtt(error.to_string()))?;
        }
        let stop = Arc::new(AtomicBool::new(false));
        let thread_stop = Arc::clone(&stop);
        let (ready_tx, ready_rx) = mpsc::channel();
        let (inbox_tx, inbox) = mpsc::channel();
        let expected_subacks = subscriptions.len();
        let clock_origin = Instant::now();
        let thread = thread::spawn(move || {
            let mut subacks = 0_usize;
            let mut ready_sent = false;
            while !thread_stop.load(Ordering::Relaxed) {
                match connection.recv_timeout(Duration::from_millis(100)) {
                    Ok(Ok(Event::Incoming(Packet::ConnAck(_)))) if expected_subacks == 0 => {
                        if !ready_sent {
                            let _ = ready_tx.send(());
                            ready_sent = true;
                        }
                    }
                    Ok(Ok(Event::Incoming(Packet::SubAck(_)))) => {
                        subacks = subacks.saturating_add(1);
                        if subacks >= expected_subacks && !ready_sent {
                            let _ = ready_tx.send(());
                            ready_sent = true;
                        }
                    }
                    Ok(Ok(Event::Incoming(Packet::Publish(publish)))) => {
                        let _ = inbox_tx.send(IncomingPublish {
                            topic: publish.topic,
                            payload: publish.payload.to_vec(),
                            qos: match publish.qos {
                                QoS::AtMostOnce => 0,
                                QoS::AtLeastOnce => 1,
                                QoS::ExactlyOnce => 2,
                            },
                            duplicate: publish.dup,
                            monotonic_ns: u64::try_from(clock_origin.elapsed().as_nanos())
                                .unwrap_or(u64::MAX),
                        });
                    }
                    Ok(Ok(_)) | Err(rumqttc::RecvTimeoutError::Timeout) => {}
                    Ok(Err(_)) | Err(rumqttc::RecvTimeoutError::Disconnected) => break,
                }
            }
        });
        ready_rx
            .recv_timeout(Duration::from_secs(5))
            .map_err(|_| LiveMqttError::ReadyTimeout)?;
        Ok(Self {
            client: Some(client),
            inbox,
            stop,
            thread: Some(thread),
        })
    }

    /// Publishes one fixed VDA message.
    ///
    /// # Errors
    ///
    /// Returns an error after shutdown or when serialization or MQTT queuing
    /// fails.
    pub fn publish(&self, message: &WireMessage) -> Result<(), LiveMqttError> {
        self.publish_bytes(
            message.topic(),
            message.qos(),
            message.retain(),
            &serde_json::to_vec(message.payload())?,
        )
    }

    /// Publishes a lab-control JSON object on an exact plan-owned topic.
    ///
    /// # Errors
    ///
    /// Returns an error after shutdown or when serialization or MQTT queuing
    /// fails.
    pub fn publish_control(&self, topic: &str, payload: &Value) -> Result<(), LiveMqttError> {
        self.publish_bytes(
            topic,
            QosContract::AtLeastOnce,
            false,
            &serde_json::to_vec(payload)?,
        )
    }

    fn publish_bytes(
        &self,
        topic: &str,
        qos: QosContract,
        retain: bool,
        payload: &[u8],
    ) -> Result<(), LiveMqttError> {
        self.client
            .as_ref()
            .ok_or_else(|| LiveMqttError::Mqtt("client is no longer running".to_owned()))?
            .publish(topic, mqtt_qos(qos), retain, payload)
            .map_err(|error| LiveMqttError::Mqtt(error.to_string()))
    }

    /// Receives one broker-egress publication without waiting past `timeout`.
    ///
    /// # Errors
    ///
    /// Returns an error if the connection worker has stopped unexpectedly.
    pub fn recv_timeout(
        &self,
        timeout: Duration,
    ) -> Result<Option<IncomingPublish>, LiveMqttError> {
        match self.inbox.recv_timeout(timeout) {
            Ok(publish) => Ok(Some(publish)),
            Err(mpsc::RecvTimeoutError::Timeout) => Ok(None),
            Err(mpsc::RecvTimeoutError::Disconnected) => Err(LiveMqttError::Disconnected),
        }
    }

    pub fn crash(mut self) {
        self.client.take();
        self.stop.store(true, Ordering::Relaxed);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }

    pub fn shutdown(mut self) {
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

fn mqtt_qos(qos: QosContract) -> QoS {
    match qos {
        QosContract::AtMostOnce => QoS::AtMostOnce,
        QosContract::AtLeastOnce => QoS::AtLeastOnce,
    }
}
