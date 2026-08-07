//! Offline conversion from imported evidence into the diagnostic engine context.

use std::{
    collections::{BTreeMap, BTreeSet},
    io::Read,
    path::Path,
};

use serde::{Deserialize, Serialize};
use thiserror::Error;
use vda5050_core::ActorRole;
pub use vda5050_doctor_engine::CaptureCompleteness;
use vda5050_doctor_engine::{TraceContext, TraceEvent};
use vda5050_import::{ImportReport, RecordKind};
use vda5050_local_fs::open_path_no_follow;

const MAX_EVIDENCE_MANIFEST_BYTES: u64 = 1024 * 1024;
const SYNTHETIC_EVIDENCE_SCHEMA: &str = "vda5050-lab.synthetic-evidence/1";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EventEvidence {
    pub actor_role: ActorRole,
    pub participant_id: String,
    pub participant_connection_epoch: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
#[allow(
    clippy::struct_excessive_bools,
    reason = "the signed evidence contract keeps each independent safety assertion explicit"
)]
pub struct SyntheticEvidenceManifest {
    pub schema: String,
    pub source_trace_sha256: String,
    pub synthetic: bool,
    pub same_job_isolated: bool,
    pub capture_closed: bool,
    pub completeness: CaptureCompleteness,
    pub role_attribution_complete: bool,
    pub events: BTreeMap<usize, EventEvidence>,
}

#[derive(Debug, Error)]
pub enum EvidenceError {
    #[error("cannot open synthetic evidence manifest: {0}")]
    Open(#[from] vda5050_local_fs::LocalPathError),
    #[error("cannot read synthetic evidence manifest: {0}")]
    Read(#[from] std::io::Error),
    #[error("synthetic evidence manifest exceeds the 1 MiB hard limit")]
    TooLarge,
    #[error("invalid synthetic evidence manifest JSON: {0}")]
    Json(#[from] serde_json::Error),
    #[error("unsupported synthetic evidence manifest schema")]
    Schema,
    #[error("synthetic evidence manifest does not match the trace digest")]
    TraceDigest,
    #[error("evidence manifest is not scoped to a same-job isolated synthetic run")]
    NotIsolatedSynthetic,
    #[error("role-complete evidence is missing for message record {0}")]
    MissingEvent(usize),
    #[error("evidence references non-message record {0}")]
    UnknownEvent(usize),
    #[error("canonical capture does not contain a closing record")]
    MissingCaptureClose,
    #[error("message record {0} could not be adapted to a canonical observation")]
    MessageAdaptation(usize),
}

/// Loads one explicitly selected, local, no-follow synthetic evidence manifest.
///
/// # Errors
///
/// Returns an error when the path is unsafe, the file exceeds the 1 MiB hard
/// limit, reading fails, or the JSON does not match the strict manifest schema.
pub fn load_synthetic_evidence_manifest(
    path: &Path,
) -> Result<SyntheticEvidenceManifest, EvidenceError> {
    let opened = open_path_no_follow(path)?;
    if opened.metadata()?.len() > MAX_EVIDENCE_MANIFEST_BYTES {
        return Err(EvidenceError::TooLarge);
    }
    let mut bytes = Vec::new();
    opened
        .take(MAX_EVIDENCE_MANIFEST_BYTES.saturating_add(1))
        .read_to_end(&mut bytes)?;
    if u64::try_from(bytes.len()).unwrap_or(u64::MAX) > MAX_EVIDENCE_MANIFEST_BYTES {
        return Err(EvidenceError::TooLarge);
    }
    Ok(serde_json::from_slice(&bytes)?)
}

/// Preserves canonical observation metadata and applies only explicit Tier 1
/// same-job synthetic assertions.
///
/// # Errors
///
/// Returns an error when the manifest is not digest-bound to this trace, does
/// not assert the required isolated synthetic scope, has incomplete or extra
/// event mappings, claims a missing capture close, or a message record cannot
/// be adapted to the canonical observation model.
pub fn trace_context_with_synthetic_evidence(
    imported: &ImportReport,
    manifest: Option<&SyntheticEvidenceManifest>,
) -> Result<TraceContext, EvidenceError> {
    let has_capture_close = imported.records.iter().any(|record| {
        matches!(
            &record.kind,
            RecordKind::NonMessage { record_type, .. }
                if record_type.eq_ignore_ascii_case("CAPTURE_CLOSED")
        )
    });

    if let Some(manifest) = manifest {
        validate_manifest(imported, manifest, has_capture_close)?;
    }

    let mut seen_evidence = BTreeSet::new();
    let mut events = Vec::new();
    for record in &imported.records {
        let RecordKind::Message(message) = &record.kind else {
            continue;
        };
        let Some(core) = record.to_core_message("imported-capture") else {
            return Err(EvidenceError::MessageAdaptation(
                record.location.record_index,
            ));
        };
        let evidence = manifest.and_then(|value| {
            let evidence = value.events.get(&record.location.record_index);
            if evidence.is_some() {
                seen_evidence.insert(record.location.record_index);
            }
            evidence
        });
        events.push(TraceEvent {
            event_id: record.event_id.clone(),
            source_sequence: core
                .source_sequence()
                .unwrap_or_else(|| u64::try_from(record.location.record_index).unwrap_or(u64::MAX)),
            topic: message.topic.clone(),
            payload: message.payload.clone(),
            capture_point: core.capture_point(),
            observed_monotonic_ns: core.observed_monotonic_ns(),
            clock_domain: core.clock_domain().map(ToOwned::to_owned),
            clock_epoch: core.clock_epoch().unwrap_or("IMPORTED_UNKNOWN").to_owned(),
            actor_role: evidence.map(|value| value.actor_role),
            participant_id: evidence.map(|value| value.participant_id.clone()),
            participant_connection_epoch: evidence
                .map(|value| value.participant_connection_epoch.clone()),
        });
    }

    if let Some(manifest) = manifest {
        for record_index in manifest.events.keys() {
            if !seen_evidence.contains(record_index) {
                return Err(EvidenceError::UnknownEvent(*record_index));
            }
        }
    }

    Ok(TraceContext {
        events,
        completeness: manifest.map_or(CaptureCompleteness::Unknown, |value| value.completeness),
        capture_closed: manifest.map_or(has_capture_close, |value| value.capture_closed),
        role_attribution_complete: manifest.is_some_and(|value| value.role_attribution_complete),
    })
}

fn validate_manifest(
    imported: &ImportReport,
    manifest: &SyntheticEvidenceManifest,
    has_capture_close: bool,
) -> Result<(), EvidenceError> {
    if manifest.schema != SYNTHETIC_EVIDENCE_SCHEMA {
        return Err(EvidenceError::Schema);
    }
    if manifest.source_trace_sha256 != imported.source_digest {
        return Err(EvidenceError::TraceDigest);
    }
    if !manifest.synthetic || !manifest.same_job_isolated {
        return Err(EvidenceError::NotIsolatedSynthetic);
    }
    if manifest.capture_closed && !has_capture_close {
        return Err(EvidenceError::MissingCaptureClose);
    }
    if manifest.role_attribution_complete {
        for record in &imported.records {
            if matches!(record.kind, RecordKind::Message(_))
                && !manifest.events.contains_key(&record.location.record_index)
            {
                return Err(EvidenceError::MissingEvent(record.location.record_index));
            }
        }
    }
    Ok(())
}
