use serde::{Deserialize, Serialize};
use serde_json::Value;
use vda5050_core::{CapturePoint, MessageObserved, PayloadReference};

/// Hard limits applied before or during parsing.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ImportConfig {
    pub max_file_bytes: u64,
    pub max_records: usize,
    pub max_payload_bytes: usize,
    pub max_json_depth: usize,
}

impl Default for ImportConfig {
    fn default() -> Self {
        Self {
            max_file_bytes: 64 * 1024 * 1024,
            max_records: 1_000_000,
            max_payload_bytes: 1024 * 1024,
            max_json_depth: 64,
        }
    }
}

/// Input format is always selected explicitly by the caller.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ImportFormat {
    CanonicalJsonl,
    MessageEnvelopeJsonl,
    MessageEnvelopeArray,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize)]
pub struct ImportSummary {
    pub imported: usize,
    pub rejected: usize,
    pub gaps: usize,
    pub non_messages: usize,
    pub unknown: usize,
    pub truncated: usize,
    /// Conservative bytes retained by record outcomes and evidence copies.
    pub retained_bytes: u64,
    /// Hard aggregate ceiling applied to retained record data.
    pub retained_bytes_limit: u64,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct ImportReport {
    pub source_digest: String,
    pub records: Vec<ImportRecord>,
    pub summary: ImportSummary,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct RecordLocation {
    pub record_index: usize,
    pub byte_offset: u64,
    pub line: Option<usize>,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct ImportRecord {
    pub event_id: String,
    pub raw_digest: String,
    pub event_digest: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub raw: Option<Vec<u8>>,
    pub location: RecordLocation,
    pub kind: RecordKind,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(tag = "outcome", rename_all = "SCREAMING_SNAKE_CASE")]
pub enum RecordKind {
    Message(Box<ImportedMessage>),
    Gap { reason: String },
    NonMessage { record_type: String, value: Value },
    Rejected { reason: RejectReason },
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct ImportedMessage {
    pub topic: String,
    pub payload: Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timestamp: Option<String>,
    #[serde(skip)]
    pub(crate) payload_bytes: Vec<u8>,
    #[serde(skip)]
    pub(crate) core_message: Option<MessageObserved>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "code", rename_all = "SCREAMING_SNAKE_CASE")]
pub enum RejectReason {
    InvalidUtf8,
    EmptyRecord,
    MalformedJson,
    DuplicateJsonKey { key: String },
    JsonTooDeep { maximum: usize },
    MissingCanonicalField { field: String },
    MissingEnvelopeField { field: String },
    InvalidEnvelopeField { field: String },
    UnknownRecordType { record_type: String },
    PayloadMalformedJson,
    PayloadNotObject,
    PayloadTooLarge { actual: usize, maximum: usize },
    PayloadSizeMismatch,
    PayloadDigestMismatch,
    ExternalPayloadUnsupported,
    MalformedCanonicalRecord,
    RecordLimitExceeded,
    RetainedBytesLimitExceeded { maximum: u64 },
}

impl ImportSummary {
    pub(crate) fn observe(&mut self, kind: &RecordKind) {
        match kind {
            RecordKind::Message(_) => self.imported += 1,
            RecordKind::Gap { .. } => self.gaps += 1,
            RecordKind::NonMessage { .. } => self.non_messages += 1,
            RecordKind::Rejected { reason } => {
                self.rejected += 1;
                if matches!(reason, RejectReason::UnknownRecordType { .. }) {
                    self.unknown += 1;
                }
            }
        }
    }
}

impl ImportRecord {
    /// Converts an accepted imported message into the shared evidence model.
    ///
    /// The adapter asserts only capture-local ordering. Publisher identity,
    /// delivery `QoS`, `DUP`, monotonic time and clock identity stay unknown.
    #[must_use]
    pub fn to_core_message(&self, capture_id: impl Into<String>) -> Option<MessageObserved> {
        let RecordKind::Message(message) = &self.kind else {
            return None;
        };
        if let Some(core_message) = &message.core_message {
            return Some(core_message.clone());
        }
        let observation = MessageObserved::new(
            self.event_id.clone(),
            capture_id,
            CapturePoint::ImportedUnknown,
            Some(u64::try_from(self.location.record_index).unwrap_or(u64::MAX)),
            message.topic.clone(),
            PayloadReference::inline(message.payload_bytes.clone()),
        )
        .with_time(message.timestamp.clone(), None, None, None);
        Some(observation)
    }
}

impl ImportedMessage {
    #[must_use]
    pub fn payload_bytes(&self) -> &[u8] {
        &self.payload_bytes
    }
}
