use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum CapturePoint {
    SubscriberDelivery,
    BrokerIngress,
    BrokerEgress,
    PublisherAdapter,
    ImportedUnknown,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ClockWrapPolicy {
    DoesNotWrap,
    WrapsAt { monotonic_ns: u64 },
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClockDescriptor {
    domain: String,
    monotonic_origin: String,
    resolution_ns: u64,
    wrap_policy: ClockWrapPolicy,
    epoch: String,
    wall_clock_uncertainty_ns: Option<u64>,
}

impl ClockDescriptor {
    pub fn new(
        domain: impl Into<String>,
        monotonic_origin: impl Into<String>,
        resolution_ns: u64,
        wrap_policy: ClockWrapPolicy,
        epoch: impl Into<String>,
        wall_clock_uncertainty_ns: Option<u64>,
    ) -> Self {
        Self {
            domain: domain.into(),
            monotonic_origin: monotonic_origin.into(),
            resolution_ns,
            wrap_policy,
            epoch: epoch.into(),
            wall_clock_uncertainty_ns,
        }
    }

    #[must_use]
    pub fn domain(&self) -> &str {
        &self.domain
    }

    #[must_use]
    pub fn epoch(&self) -> &str {
        &self.epoch
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CaptureOpened {
    capture_id: String,
    opened_at: String,
    clock: Option<ClockDescriptor>,
    synthetic: bool,
}

impl CaptureOpened {
    pub fn new(
        capture_id: impl Into<String>,
        opened_at: impl Into<String>,
        clock: Option<ClockDescriptor>,
        synthetic: bool,
    ) -> Self {
        Self {
            capture_id: capture_id.into(),
            opened_at: opened_at.into(),
            clock,
            synthetic,
        }
    }

    #[must_use]
    pub fn capture_id(&self) -> &str {
        &self.capture_id
    }

    #[must_use]
    pub fn opened_at(&self) -> &str {
        &self.opened_at
    }

    #[must_use]
    pub fn clock(&self) -> Option<&ClockDescriptor> {
        self.clock.as_ref()
    }

    #[must_use]
    pub const fn synthetic(&self) -> bool {
        self.synthetic
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CaptureClosed {
    capture_id: String,
    closed_at: String,
    record_count: u64,
    stream_sha256: String,
    synthetic: bool,
}

impl CaptureClosed {
    pub fn new(
        capture_id: impl Into<String>,
        closed_at: impl Into<String>,
        record_count: u64,
        stream_sha256: impl Into<String>,
        synthetic: bool,
    ) -> Self {
        Self {
            capture_id: capture_id.into(),
            closed_at: closed_at.into(),
            record_count,
            stream_sha256: stream_sha256.into(),
            synthetic,
        }
    }

    #[must_use]
    pub fn capture_id(&self) -> &str {
        &self.capture_id
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "storage", rename_all = "SCREAMING_SNAKE_CASE")]
pub enum PayloadReference {
    Inline {
        bytes: Vec<u8>,
        sha256: String,
        size_bytes: u64,
    },
    External {
        reference: String,
        sha256: String,
        size_bytes: u64,
    },
}

impl PayloadReference {
    #[must_use]
    pub fn inline(bytes: Vec<u8>) -> Self {
        let sha256 = digest(&bytes);
        let size_bytes = u64::try_from(bytes.len()).unwrap_or(u64::MAX);
        Self::Inline {
            bytes,
            sha256,
            size_bytes,
        }
    }

    #[must_use]
    pub fn external(
        reference: impl Into<String>,
        sha256: impl Into<String>,
        size_bytes: u64,
    ) -> Self {
        Self::External {
            reference: reference.into(),
            sha256: sha256.into(),
            size_bytes,
        }
    }

    #[must_use]
    pub fn inline_bytes(&self) -> Option<&[u8]> {
        match self {
            Self::Inline { bytes, .. } => Some(bytes),
            Self::External { .. } => None,
        }
    }

    #[must_use]
    pub fn sha256(&self) -> &str {
        match self {
            Self::Inline { sha256, .. } | Self::External { sha256, .. } => sha256,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MessageObserved {
    event_id: String,
    capture_id: String,
    capture_point: CapturePoint,
    source_sequence: Option<u64>,
    observed_topic: String,
    payload: PayloadReference,
    observed_delivery_qos: Option<u8>,
    observed_dup: Option<bool>,
    observed_at: Option<String>,
    observed_monotonic_ns: Option<u64>,
    clock_domain: Option<String>,
    clock_epoch: Option<String>,
}

impl MessageObserved {
    pub fn new(
        event_id: impl Into<String>,
        capture_id: impl Into<String>,
        capture_point: CapturePoint,
        source_sequence: Option<u64>,
        observed_topic: impl Into<String>,
        payload: PayloadReference,
    ) -> Self {
        Self {
            event_id: event_id.into(),
            capture_id: capture_id.into(),
            capture_point,
            source_sequence,
            observed_topic: observed_topic.into(),
            payload,
            observed_delivery_qos: None,
            observed_dup: None,
            observed_at: None,
            observed_monotonic_ns: None,
            clock_domain: None,
            clock_epoch: None,
        }
    }

    #[must_use]
    pub fn with_observed_delivery(mut self, qos: Option<u8>, duplicate: Option<bool>) -> Self {
        self.observed_delivery_qos = qos;
        self.observed_dup = duplicate;
        self
    }

    #[must_use]
    pub fn with_time(
        mut self,
        observed_at: Option<String>,
        observed_monotonic_ns: Option<u64>,
        clock_domain: Option<String>,
        clock_epoch: Option<String>,
    ) -> Self {
        self.observed_at = observed_at;
        self.observed_monotonic_ns = observed_monotonic_ns;
        self.clock_domain = clock_domain;
        self.clock_epoch = clock_epoch;
        self
    }

    #[must_use]
    pub fn event_id(&self) -> &str {
        &self.event_id
    }

    #[must_use]
    pub fn capture_id(&self) -> &str {
        &self.capture_id
    }

    #[must_use]
    pub fn observed_topic(&self) -> &str {
        &self.observed_topic
    }

    #[must_use]
    pub const fn capture_point(&self) -> CapturePoint {
        self.capture_point
    }

    #[must_use]
    pub const fn source_sequence(&self) -> Option<u64> {
        self.source_sequence
    }

    #[must_use]
    pub const fn observed_monotonic_ns(&self) -> Option<u64> {
        self.observed_monotonic_ns
    }

    #[must_use]
    pub fn clock_domain(&self) -> Option<&str> {
        self.clock_domain.as_deref()
    }

    #[must_use]
    pub fn clock_epoch(&self) -> Option<&str> {
        self.clock_epoch.as_deref()
    }

    #[must_use]
    pub fn payload(&self) -> &PayloadReference {
        &self.payload
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CaptureGap {
    capture_id: String,
    gap_id: String,
    first_missing_sequence: Option<u64>,
    last_missing_sequence: Option<u64>,
    reason: String,
}

impl CaptureGap {
    pub fn new(
        capture_id: impl Into<String>,
        gap_id: impl Into<String>,
        first_missing_sequence: Option<u64>,
        last_missing_sequence: Option<u64>,
        reason: impl Into<String>,
    ) -> Self {
        Self {
            capture_id: capture_id.into(),
            gap_id: gap_id.into(),
            first_missing_sequence,
            last_missing_sequence,
            reason: reason.into(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AdapterWarning {
    capture_id: String,
    warning_id: String,
    message: String,
    source_offset: Option<u64>,
}

impl AdapterWarning {
    pub fn new(
        capture_id: impl Into<String>,
        warning_id: impl Into<String>,
        message: impl Into<String>,
        source_offset: Option<u64>,
    ) -> Self {
        Self {
            capture_id: capture_id.into(),
            warning_id: warning_id.into(),
            message: message.into(),
            source_offset,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClockEpochChanged {
    capture_id: String,
    clock_domain: String,
    previous_epoch: String,
    new_epoch: String,
    reason: String,
}

impl ClockEpochChanged {
    pub fn new(
        capture_id: impl Into<String>,
        clock_domain: impl Into<String>,
        previous_epoch: impl Into<String>,
        new_epoch: impl Into<String>,
        reason: impl Into<String>,
    ) -> Self {
        Self {
            capture_id: capture_id.into(),
            clock_domain: clock_domain.into(),
            previous_epoch: previous_epoch.into(),
            new_epoch: new_epoch.into(),
            reason: reason.into(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    tag = "record_type",
    content = "record",
    rename_all = "SCREAMING_SNAKE_CASE"
)]
pub enum CaptureRecord {
    CaptureOpened(CaptureOpened),
    MessageObserved(MessageObserved),
    CaptureGap(CaptureGap),
    AdapterWarning(AdapterWarning),
    ClockEpochChanged(ClockEpochChanged),
    CaptureClosed(CaptureClosed),
}

macro_rules! record_conversion {
    ($source:ty, $variant:ident) => {
        impl From<$source> for CaptureRecord {
            fn from(record: $source) -> Self {
                Self::$variant(record)
            }
        }
    };
}

record_conversion!(CaptureOpened, CaptureOpened);
record_conversion!(MessageObserved, MessageObserved);
record_conversion!(CaptureGap, CaptureGap);
record_conversion!(AdapterWarning, AdapterWarning);
record_conversion!(ClockEpochChanged, ClockEpochChanged);
record_conversion!(CaptureClosed, CaptureClosed);

impl CaptureRecord {
    fn capture_id(&self) -> &str {
        match self {
            Self::CaptureOpened(record) => &record.capture_id,
            Self::MessageObserved(record) => &record.capture_id,
            Self::CaptureGap(record) => &record.capture_id,
            Self::AdapterWarning(record) => &record.capture_id,
            Self::ClockEpochChanged(record) => &record.capture_id,
            Self::CaptureClosed(record) => &record.capture_id,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CaptureLifecycle {
    capture_id: String,
    records: Vec<CaptureRecord>,
}

impl CaptureLifecycle {
    /// Validates an entire append-only capture record sequence.
    ///
    /// # Errors
    ///
    /// Returns [`CaptureLifecycleError`] when the sequence does not open first,
    /// close last, preserve capture identity, or contains records after close.
    pub fn validate(records: Vec<CaptureRecord>) -> Result<Self, CaptureLifecycleError> {
        let Some(CaptureRecord::CaptureOpened(opened)) = records.first() else {
            return Err(if records.is_empty() {
                CaptureLifecycleError::MissingOpen
            } else {
                CaptureLifecycleError::FirstRecordNotOpen
            });
        };
        let capture_id = opened.capture_id.clone();
        let mut closed_at = None;

        for (index, record) in records.iter().enumerate() {
            if let Some(close_index) = closed_at {
                return Err(CaptureLifecycleError::RecordAfterClose { close_index, index });
            }
            if index > 0 && matches!(record, CaptureRecord::CaptureOpened(_)) {
                return Err(CaptureLifecycleError::DuplicateOpen { index });
            }
            if record.capture_id() != capture_id {
                return Err(CaptureLifecycleError::CaptureIdMismatch {
                    index,
                    expected: capture_id,
                    actual: record.capture_id().to_owned(),
                });
            }
            if matches!(record, CaptureRecord::CaptureClosed(_)) {
                closed_at = Some(index);
            }
        }

        if closed_at.is_none() {
            return Err(CaptureLifecycleError::MissingClose);
        }

        Ok(Self {
            capture_id,
            records,
        })
    }

    #[must_use]
    pub fn capture_id(&self) -> &str {
        &self.capture_id
    }

    #[must_use]
    pub fn records(&self) -> &[CaptureRecord] {
        &self.records
    }

    /// Derives stable digests which reference the validated immutable records.
    ///
    /// # Errors
    ///
    /// Returns [`FinalManifestError::Serialization`] if a record cannot be
    /// serialized. The invariant error is defensive and cannot be produced by
    /// [`CaptureLifecycle::validate`].
    pub fn derive_final_manifest(&self) -> Result<FinalManifest, FinalManifestError> {
        let encoded = serde_json::to_vec(&self.records)?;
        let opened = serde_json::to_vec(
            self.records
                .first()
                .ok_or(FinalManifestError::InvariantViolation)?,
        )?;
        let closed = serde_json::to_vec(
            self.records
                .last()
                .ok_or(FinalManifestError::InvariantViolation)?,
        )?;
        Ok(FinalManifest {
            capture_id: self.capture_id.clone(),
            record_count: u64::try_from(self.records.len()).unwrap_or(u64::MAX),
            records_sha256: digest(&encoded),
            opened_record_sha256: digest(&opened),
            closed_record_sha256: digest(&closed),
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum CaptureLifecycleError {
    #[error("capture has no opening record")]
    MissingOpen,
    #[error("the first capture record is not CAPTURE_OPENED")]
    FirstRecordNotOpen,
    #[error("capture has no closing record")]
    MissingClose,
    #[error("capture contains another opening record at index {index}")]
    DuplicateOpen { index: usize },
    #[error("capture record at index {index} follows close at index {close_index}")]
    RecordAfterClose { close_index: usize, index: usize },
    #[error("capture ID mismatch at index {index}: expected {expected}, got {actual}")]
    CaptureIdMismatch {
        index: usize,
        expected: String,
        actual: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FinalManifest {
    capture_id: String,
    record_count: u64,
    records_sha256: String,
    opened_record_sha256: String,
    closed_record_sha256: String,
}

impl FinalManifest {
    #[must_use]
    pub fn capture_id(&self) -> &str {
        &self.capture_id
    }

    #[must_use]
    pub const fn record_count(&self) -> u64 {
        self.record_count
    }

    #[must_use]
    pub fn records_sha256(&self) -> &str {
        &self.records_sha256
    }
}

#[derive(Debug, Error)]
pub enum FinalManifestError {
    #[error("failed to serialize capture records for the final manifest: {0}")]
    Serialization(#[from] serde_json::Error),
    #[error("validated capture lifecycle unexpectedly contains no records")]
    InvariantViolation,
}

fn digest(bytes: &[u8]) -> String {
    hex::encode(Sha256::digest(bytes))
}
