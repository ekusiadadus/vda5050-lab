use std::collections::{BTreeMap, BTreeSet};

use serde_json::Value;
use sha2::{Digest, Sha256};
use thiserror::Error;
use vda5050_core::{
    ActorRole, CaptureClosed, CaptureOpened, CapturePoint, CaptureRecord, ClockDescriptor,
    ClockWrapPolicy, MessageObserved, PayloadReference,
};
use vda5050_doctor::{CaptureCompleteness, EventEvidence, SyntheticEvidenceManifest};

use crate::{LiveRunPlan, LsmartRunPlan};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LiveCapturedPublish {
    pub topic: String,
    pub payload: Vec<u8>,
    pub qos: u8,
    pub duplicate: bool,
    pub monotonic_ns: u64,
}

impl LiveCapturedPublish {
    #[must_use]
    pub const fn new(
        topic: String,
        payload: Vec<u8>,
        qos: u8,
        duplicate: bool,
        monotonic_ns: u64,
    ) -> Self {
        Self {
            topic,
            payload,
            qos,
            duplicate,
            monotonic_ns,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LiveCaptureArtifacts {
    pub trace_bytes: Vec<u8>,
    pub evidence: SyntheticEvidenceManifest,
}

#[derive(Debug, Error)]
pub enum LiveCaptureBuildError {
    #[error("captured message hard limit exceeded")]
    MessageLimit,
    #[error("captured topic is outside the fixed run plan: {0}")]
    Topic(String),
    #[error("captured payload is not JSON: {0}")]
    Json(#[from] serde_json::Error),
}

/// Builds a closed canonical capture and its exact same-job evidence manifest.
///
/// # Errors
///
/// Returns an error when the message limit is exceeded, a topic is outside the
/// plan, or a payload is not JSON.
pub fn build_live_capture_artifacts(
    plan: &LiveRunPlan,
    captures: &[LiveCapturedPublish],
) -> Result<LiveCaptureArtifacts, LiveCaptureBuildError> {
    if captures.len() > plan.hard_limits().messages {
        return Err(LiveCaptureBuildError::MessageLimit);
    }
    let capture_id = format!("tier1-live-{}", plan.run_id());
    let clock_epoch = format!("clock-live-{}", plan.run_id());
    let opened: CaptureRecord = CaptureOpened::new(
        &capture_id,
        "2026-08-07T00:00:00.000Z",
        Some(ClockDescriptor::new(
            "live-recorder-monotonic",
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
    let mut events = BTreeMap::new();
    let mut broken_participants = BTreeSet::new();
    for (index, capture) in captures.iter().enumerate() {
        let record_index = index + 1;
        let (observed, evidence, broken_serial) = capture_record(
            plan,
            capture,
            record_index,
            &capture_id,
            &clock_epoch,
            &broken_participants,
        )?;
        records.push(observed.into());
        events.insert(record_index, evidence);
        if let Some(serial) = broken_serial {
            broken_participants.insert(serial);
        }
    }
    let open_stream = serde_json::to_vec(&records)?;
    let closed: CaptureRecord = CaptureClosed::new(
        &capture_id,
        "2026-08-07T00:00:30.000Z",
        u64::try_from(records.len() + 1).unwrap_or(u64::MAX),
        hex::encode(Sha256::digest(&open_stream)),
        true,
    )
    .into();
    records.push(closed);
    let mut trace_bytes = Vec::new();
    for record in records {
        trace_bytes.extend_from_slice(&serde_json::to_vec(&record)?);
        trace_bytes.push(b'\n');
    }
    let evidence = SyntheticEvidenceManifest {
        schema: "vda5050-lab.synthetic-evidence/1".to_owned(),
        source_trace_sha256: hex::encode(Sha256::digest(&trace_bytes)),
        synthetic: true,
        same_job_isolated: plan.same_job_isolated(),
        capture_closed: true,
        completeness: CaptureCompleteness::ConfirmedForRule,
        role_attribution_complete: true,
        events,
    };
    Ok(LiveCaptureArtifacts {
        trace_bytes,
        evidence,
    })
}

/// Builds a closed canonical capture from the official LSMART MQTT projection.
///
/// # Errors
///
/// Returns an error when the message limit is exceeded, a topic is outside the
/// sealed LSMART plan, or a payload is not JSON.
pub fn build_lsmart_capture_artifacts(
    plan: &LsmartRunPlan,
    captures: &[LiveCapturedPublish],
) -> Result<LiveCaptureArtifacts, LiveCaptureBuildError> {
    if captures.len() > plan.message_limit {
        return Err(LiveCaptureBuildError::MessageLimit);
    }
    let capture_id = format!("tier1-lsmart-{}", plan.run_id);
    let clock_epoch = format!("clock-lsmart-{}", plan.run_id);
    let opened: CaptureRecord = CaptureOpened::new(
        &capture_id,
        "2026-08-07T00:00:00.000Z",
        Some(ClockDescriptor::new(
            "lsmart-recorder-monotonic",
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
    let mut events = BTreeMap::new();
    let mut broken_participants = BTreeSet::new();
    for (index, capture) in captures.iter().enumerate() {
        let record_index = index + 1;
        let (observed, evidence, broken_serial) = lsmart_capture_record(
            plan,
            capture,
            record_index,
            &capture_id,
            &clock_epoch,
            &broken_participants,
        )?;
        records.push(observed.into());
        events.insert(record_index, evidence);
        if let Some(serial) = broken_serial {
            broken_participants.insert(serial);
        }
    }
    let open_stream = serde_json::to_vec(&records)?;
    let closed: CaptureRecord = CaptureClosed::new(
        &capture_id,
        "2026-08-07T00:01:00.000Z",
        u64::try_from(records.len() + 1).unwrap_or(u64::MAX),
        hex::encode(Sha256::digest(&open_stream)),
        true,
    )
    .into();
    records.push(closed);
    let mut trace_bytes = Vec::new();
    for record in records {
        trace_bytes.extend_from_slice(&serde_json::to_vec(&record)?);
        trace_bytes.push(b'\n');
    }
    let evidence = SyntheticEvidenceManifest {
        schema: "vda5050-lab.synthetic-evidence/1".to_owned(),
        source_trace_sha256: hex::encode(Sha256::digest(&trace_bytes)),
        synthetic: true,
        same_job_isolated: true,
        capture_closed: true,
        completeness: CaptureCompleteness::ConfirmedForRule,
        role_attribution_complete: true,
        events,
    };
    Ok(LiveCaptureArtifacts {
        trace_bytes,
        evidence,
    })
}

fn lsmart_capture_record(
    plan: &LsmartRunPlan,
    capture: &LiveCapturedPublish,
    record_index: usize,
    capture_id: &str,
    clock_epoch: &str,
    broken_participants: &BTreeSet<String>,
) -> Result<(MessageObserved, EventEvidence, Option<String>), LiveCaptureBuildError> {
    if !plan.topic_allowed(&capture.topic) {
        return Err(LiveCaptureBuildError::Topic(capture.topic.clone()));
    }
    let robot = plan
        .robots
        .iter()
        .find(|robot| {
            capture
                .topic
                .starts_with(&format!("{}/", robot.topic_prefix))
        })
        .ok_or_else(|| LiveCaptureBuildError::Topic(capture.topic.clone()))?;
    let payload: Value = serde_json::from_slice(&capture.payload)?;
    let actor_role = if capture.topic.ends_with("/order") {
        ActorRole::FleetControl
    } else {
        ActorRole::MobileRobot
    };
    let participant_id = if actor_role == ActorRole::FleetControl {
        "lsmart-lab/fleet".to_owned()
    } else {
        format!("lsmart-lab/{}", robot.serial_number)
    };
    let connection_epoch = if actor_role == ActorRole::FleetControl {
        "lsmart-fleet-session-1"
    } else if broken_participants.contains(&robot.serial_number) {
        "lsmart-session-2"
    } else {
        "lsmart-session-1"
    };
    let observed = MessageObserved::new(
        format!("lsmart-event-{record_index:05}"),
        capture_id,
        CapturePoint::BrokerEgress,
        Some(u64::try_from(record_index).unwrap_or(u64::MAX)),
        &capture.topic,
        PayloadReference::inline(capture.payload.clone()),
    )
    .with_observed_delivery(Some(capture.qos), Some(capture.duplicate))
    .with_time(
        payload["timestamp"].as_str().map(ToOwned::to_owned),
        Some(capture.monotonic_ns),
        Some("lsmart-recorder-monotonic".to_owned()),
        Some(clock_epoch.to_owned()),
    );
    let evidence = EventEvidence {
        actor_role,
        participant_id,
        participant_connection_epoch: connection_epoch.to_owned(),
    };
    let broken_serial =
        (payload["connectionState"] == "CONNECTION_BROKEN").then(|| robot.serial_number.clone());
    Ok((observed, evidence, broken_serial))
}

fn capture_record(
    plan: &LiveRunPlan,
    capture: &LiveCapturedPublish,
    record_index: usize,
    capture_id: &str,
    clock_epoch: &str,
    broken_participants: &BTreeSet<String>,
) -> Result<(MessageObserved, EventEvidence, Option<String>), LiveCaptureBuildError> {
    let robot = plan
        .robots()
        .iter()
        .find(|robot| {
            capture
                .topic
                .starts_with(&format!("{}/", robot.topic_prefix))
        })
        .ok_or_else(|| LiveCaptureBuildError::Topic(capture.topic.clone()))?;
    if !plan.topic_allowlist().contains(&capture.topic) {
        return Err(LiveCaptureBuildError::Topic(capture.topic.clone()));
    }
    let payload: Value = serde_json::from_slice(&capture.payload)?;
    let actor_role = if capture.topic.ends_with("/order") {
        ActorRole::FleetControl
    } else {
        ActorRole::MobileRobot
    };
    let participant_id = if actor_role == ActorRole::FleetControl {
        "lab-demo/fleet".to_owned()
    } else {
        format!("lab-demo/{}", robot.serial_number)
    };
    let connection_epoch = if actor_role == ActorRole::FleetControl {
        "fleet-session-1"
    } else if broken_participants.contains(&robot.serial_number) {
        "session-2"
    } else {
        "session-1"
    };
    let observed = MessageObserved::new(
        format!("event-{record_index:03}"),
        capture_id,
        CapturePoint::BrokerEgress,
        Some(u64::try_from(record_index).unwrap_or(u64::MAX)),
        &capture.topic,
        PayloadReference::inline(capture.payload.clone()),
    )
    .with_observed_delivery(Some(capture.qos), Some(capture.duplicate))
    .with_time(
        payload["timestamp"].as_str().map(ToOwned::to_owned),
        Some(capture.monotonic_ns),
        Some("live-recorder-monotonic".to_owned()),
        Some(clock_epoch.to_owned()),
    );
    let evidence = EventEvidence {
        actor_role,
        participant_id,
        participant_connection_epoch: connection_epoch.to_owned(),
    };
    let broken_serial =
        (payload["connectionState"] == "CONNECTION_BROKEN").then(|| robot.serial_number.clone());
    Ok((observed, evidence, broken_serial))
}
