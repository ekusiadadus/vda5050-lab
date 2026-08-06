use std::{
    io::{self, Read},
    mem,
    path::Path,
};

use serde_json::Value;
use sha2::{Digest, Sha256};
use thiserror::Error;
use vda5050_core::{CaptureRecord, PayloadReference};
use vda5050_local_fs::{LocalPathError, looks_like_uri, open_path_no_follow};

use crate::{
    model::{
        ImportConfig, ImportFormat, ImportRecord, ImportReport, ImportSummary, ImportedMessage,
        RecordKind, RecordLocation, RejectReason,
    },
    strict_json::{self, StrictJsonError},
};

const HARD_MAX_FILE_BYTES: u64 = 64 * 1024 * 1024;
const HARD_MAX_RECORDS: u64 = 2_000_000;
const HARD_MAX_PAYLOAD_BYTES: u64 = 16 * 1024 * 1024;
const HARD_MAX_JSON_DEPTH: u64 = 256;
const HARD_MAX_RETAINED_BYTES: u64 = 64 * 1024 * 1024;
const MIN_RETAINED_BYTES: u64 = 4 * 1024;
const RETAINED_LIMIT_MARKER_RESERVE: u64 = 2 * 1024;

#[derive(Debug, Error)]
pub enum ImportError {
    #[error("invalid import limit: {0} must be greater than zero")]
    InvalidLimit(&'static str),
    #[error("invalid import limit: {name} exceeds hard maximum {maximum}")]
    LimitTooHigh { name: &'static str, maximum: u64 },
    #[error("URI inputs are not accepted: {0}")]
    Uri(String),
    #[error("cannot access input {path}: {source}")]
    Io {
        path: String,
        #[source]
        source: io::Error,
    },
    #[error("file byte limit exceeded: actual {actual}, maximum {maximum}")]
    FileTooLarge { actual: u64, maximum: u64 },
    #[error(transparent)]
    SecureOpen(#[from] LocalPathError),
}

/// Imports one explicitly selected local trace format under hard limits.
///
/// # Errors
///
/// Returns an error before record parsing when limits are invalid, the input is
/// URI-like, the path is a symlink or non-regular object, I/O fails, or the
/// file exceeds `max_file_bytes`. Record-level failures are returned in the
/// report rather than as function errors.
pub fn import_path(
    path: &Path,
    format: ImportFormat,
    config: ImportConfig,
) -> Result<ImportReport, ImportError> {
    validate_config(config)?;
    let display = path.display().to_string();
    if looks_like_uri(&display) && !is_windows_absolute_path(path) {
        return Err(ImportError::Uri(display));
    }

    let opened = open_path_no_follow(path)?;
    let opened_metadata = opened.metadata().map_err(|source| ImportError::Io {
        path: display.clone(),
        source,
    })?;
    if opened_metadata.len() > config.max_file_bytes {
        return Err(ImportError::FileTooLarge {
            actual: opened_metadata.len(),
            maximum: config.max_file_bytes,
        });
    }

    let mut bytes = Vec::new();
    opened
        .take(config.max_file_bytes.saturating_add(1))
        .read_to_end(&mut bytes)
        .map_err(|source| ImportError::Io {
            path: path.display().to_string(),
            source,
        })?;
    let actual = u64::try_from(bytes.len()).unwrap_or(u64::MAX);
    if actual > config.max_file_bytes {
        return Err(ImportError::FileTooLarge {
            actual,
            maximum: config.max_file_bytes,
        });
    }

    Ok(match format {
        ImportFormat::CanonicalJsonl => import_jsonl(&bytes, config, JsonlSchema::Canonical),
        ImportFormat::MessageEnvelopeJsonl => {
            import_jsonl(&bytes, config, JsonlSchema::MessageEnvelope)
        }
        ImportFormat::MessageEnvelopeArray => import_envelope_array(&bytes, config),
    })
}

#[cfg(windows)]
fn is_windows_absolute_path(path: &Path) -> bool {
    path.is_absolute()
}

#[cfg(not(windows))]
const fn is_windows_absolute_path(_path: &Path) -> bool {
    false
}

fn validate_config(config: ImportConfig) -> Result<(), ImportError> {
    for (name, value, hard_maximum) in [
        ("max_file_bytes", config.max_file_bytes, HARD_MAX_FILE_BYTES),
        (
            "max_records",
            u64::try_from(config.max_records).unwrap_or(u64::MAX),
            HARD_MAX_RECORDS,
        ),
        (
            "max_payload_bytes",
            u64::try_from(config.max_payload_bytes).unwrap_or(u64::MAX),
            HARD_MAX_PAYLOAD_BYTES,
        ),
        (
            "max_json_depth",
            u64::try_from(config.max_json_depth).unwrap_or(u64::MAX),
            HARD_MAX_JSON_DEPTH,
        ),
    ] {
        if value == 0 {
            return Err(ImportError::InvalidLimit(name));
        }
        if value > hard_maximum {
            return Err(ImportError::LimitTooHigh {
                name,
                maximum: hard_maximum,
            });
        }
    }
    Ok(())
}

#[derive(Clone, Copy)]
enum JsonlSchema {
    Canonical,
    MessageEnvelope,
}

fn import_jsonl(bytes: &[u8], config: ImportConfig, schema: JsonlSchema) -> ImportReport {
    let mut report = new_report(bytes, config);
    let mut byte_offset = 0usize;
    let mut lines = bytes.split_inclusive(|byte| *byte == b'\n').enumerate();

    while let Some((index, terminated)) = lines.next() {
        let raw = terminated.strip_suffix(b"\n").unwrap_or(terminated);
        if index >= config.max_records {
            let remaining = 1 + lines.count();
            report.summary.truncated += remaining;
            let _ = push_record(
                &mut report,
                raw,
                RecordLocation {
                    record_index: index,
                    byte_offset: as_u64(byte_offset),
                    line: Some(index + 1),
                },
                RecordKind::Rejected {
                    reason: RejectReason::RecordLimitExceeded,
                },
            );
            break;
        }
        let kind = match schema {
            JsonlSchema::Canonical => classify_canonical(raw, config),
            JsonlSchema::MessageEnvelope => classify_envelope(raw, config),
        };
        if !push_record(
            &mut report,
            raw,
            RecordLocation {
                record_index: index,
                byte_offset: as_u64(byte_offset),
                line: Some(index + 1),
            },
            kind,
        ) {
            report.summary.truncated += lines.count();
            break;
        }
        byte_offset += terminated.len();
    }
    report
}

fn import_envelope_array(bytes: &[u8], config: ImportConfig) -> ImportReport {
    let mut report = new_report(bytes, config);

    if std::str::from_utf8(bytes).is_err() {
        let _ = push_record(
            &mut report,
            bytes,
            RecordLocation {
                record_index: 0,
                byte_offset: 0,
                line: Some(1),
            },
            RecordKind::Rejected {
                reason: RejectReason::InvalidUtf8,
            },
        );
        return report;
    }

    let (scan, structural_error) = match scan_array(bytes, config.max_records) {
        Ok(scan) => (scan, None),
        Err(failure) => {
            let ArrayScanFailure {
                scan,
                raw_start,
                line,
            } = failure;
            (scan, Some((raw_start, line)))
        }
    };

    let mut retained_budget_exhausted = false;
    for (index, span) in scan.spans.iter().take(config.max_records).enumerate() {
        let raw = &bytes[span.start..span.end];
        let kind = classify_envelope(raw, config);
        if !push_record(
            &mut report,
            raw,
            RecordLocation {
                record_index: index,
                byte_offset: as_u64(span.start),
                line: Some(span.line),
            },
            kind,
        ) {
            report.summary.truncated += scan.total.saturating_sub(index + 1);
            retained_budget_exhausted = true;
            break;
        }
    }

    if !retained_budget_exhausted && scan.total > config.max_records {
        let overflow_span = scan
            .spans
            .get(config.max_records)
            .expect("scanner retains the first over-limit element");
        report.summary.truncated = scan.total - config.max_records;
        let _ = push_record(
            &mut report,
            &bytes[overflow_span.start..overflow_span.end],
            RecordLocation {
                record_index: config.max_records,
                byte_offset: as_u64(overflow_span.start),
                line: Some(overflow_span.line),
            },
            RecordKind::Rejected {
                reason: RejectReason::RecordLimitExceeded,
            },
        );
    }

    if !retained_budget_exhausted && let Some((raw_start, line)) = structural_error {
        let _ = push_record(
            &mut report,
            bytes.get(raw_start..).unwrap_or(bytes),
            RecordLocation {
                record_index: scan.total,
                byte_offset: as_u64(raw_start),
                line: Some(line),
            },
            RecordKind::Rejected {
                reason: RejectReason::MalformedJson,
            },
        );
    }

    report
}

fn classify_canonical(raw: &[u8], config: ImportConfig) -> RecordKind {
    if raw.is_empty() {
        return RecordKind::Rejected {
            reason: RejectReason::EmptyRecord,
        };
    }
    let value = match strict_json::parse(raw, config.max_json_depth) {
        Ok(value) => value,
        Err(error) => return rejected_json(error, config.max_json_depth),
    };
    let Some(object) = value.as_object() else {
        return RecordKind::Rejected {
            reason: RejectReason::MissingCanonicalField {
                field: "record_type".to_owned(),
            },
        };
    };
    let Some(record_type) = object.get("record_type").and_then(Value::as_str) else {
        return RecordKind::Rejected {
            reason: RejectReason::MissingCanonicalField {
                field: "record_type".to_owned(),
            },
        };
    };

    match normalize_type(record_type).as_str() {
        "messageobserved" => canonical_message(&value, config),
        "capturegap" => {
            if serde_json::from_value::<CaptureRecord>(value.clone()).is_err() {
                return RecordKind::Rejected {
                    reason: RejectReason::MalformedCanonicalRecord,
                };
            }
            let Some(reason) = value
                .get("record")
                .and_then(|record| record.get("reason"))
                .and_then(Value::as_str)
            else {
                return RecordKind::Rejected {
                    reason: RejectReason::MissingCanonicalField {
                        field: "reason".to_owned(),
                    },
                };
            };
            RecordKind::Gap {
                reason: reason.to_owned(),
            }
        }
        "captureopened" | "captureclosed" | "adapterwarning" | "clockepochchanged" => {
            if serde_json::from_value::<CaptureRecord>(value.clone()).is_err() {
                return RecordKind::Rejected {
                    reason: RejectReason::MalformedCanonicalRecord,
                };
            }
            RecordKind::NonMessage {
                record_type: record_type.to_owned(),
                value,
            }
        }
        _ => RecordKind::Rejected {
            reason: RejectReason::UnknownRecordType {
                record_type: record_type.to_owned(),
            },
        },
    }
}

fn canonical_message(value: &Value, config: ImportConfig) -> RecordKind {
    let Ok(CaptureRecord::MessageObserved(message)) =
        serde_json::from_value::<CaptureRecord>(value.clone())
    else {
        return RecordKind::Rejected {
            reason: RejectReason::MalformedCanonicalRecord,
        };
    };
    let PayloadReference::Inline {
        bytes,
        sha256: asserted_digest,
        size_bytes: asserted_size,
    } = message.payload()
    else {
        return RecordKind::Rejected {
            reason: RejectReason::ExternalPayloadUnsupported,
        };
    };
    if bytes.len() > config.max_payload_bytes {
        return RecordKind::Rejected {
            reason: RejectReason::PayloadTooLarge {
                actual: bytes.len(),
                maximum: config.max_payload_bytes,
            },
        };
    }
    if u64::try_from(bytes.len()).unwrap_or(u64::MAX) != *asserted_size {
        return RecordKind::Rejected {
            reason: RejectReason::PayloadSizeMismatch,
        };
    }
    if sha256(bytes) != *asserted_digest {
        return RecordKind::Rejected {
            reason: RejectReason::PayloadDigestMismatch,
        };
    }
    let payload = match strict_json::parse(bytes, config.max_json_depth) {
        Ok(payload) if payload.is_object() => payload,
        Ok(_) => {
            return RecordKind::Rejected {
                reason: RejectReason::PayloadNotObject,
            };
        }
        Err(error) => return rejected_payload_json(error, config.max_json_depth),
    };
    let timestamp = value
        .get("record")
        .and_then(|record| record.get("observed_at"))
        .and_then(Value::as_str)
        .map(ToOwned::to_owned);
    RecordKind::Message(Box::new(ImportedMessage {
        topic: message.observed_topic().to_owned(),
        payload,
        timestamp,
        payload_bytes: bytes.clone(),
        core_message: Some(message),
    }))
}

fn classify_envelope(raw: &[u8], config: ImportConfig) -> RecordKind {
    let value = match strict_json::parse(raw, config.max_json_depth) {
        Ok(value) => value,
        Err(error) => return rejected_json(error, config.max_json_depth),
    };
    let Some(object) = value.as_object() else {
        return RecordKind::Rejected {
            reason: RejectReason::MissingEnvelopeField {
                field: "topic".to_owned(),
            },
        };
    };
    message_from_object(object, config)
}

fn message_from_object(
    object: &serde_json::Map<String, Value>,
    config: ImportConfig,
) -> RecordKind {
    let missing = |field: &str| RecordKind::Rejected {
        reason: RejectReason::MissingEnvelopeField {
            field: field.to_owned(),
        },
    };

    let Some(topic) = object.get("topic").and_then(Value::as_str) else {
        return missing("topic");
    };
    let Some(payload_value) = object.get("payload") else {
        return missing("payload");
    };
    let (payload, payload_bytes) = if let Some(encoded) = payload_value.as_str() {
        match strict_json::parse(encoded.as_bytes(), config.max_json_depth) {
            Ok(value) => (value, encoded.as_bytes().to_vec()),
            Err(error) => return rejected_payload_json(error, config.max_json_depth),
        }
    } else {
        let value = payload_value.clone();
        let bytes = value.to_string().into_bytes();
        (value, bytes)
    };
    if !payload.is_object() {
        return RecordKind::Rejected {
            reason: RejectReason::PayloadNotObject,
        };
    }
    let payload_size = payload_bytes.len();
    if payload_size > config.max_payload_bytes {
        return RecordKind::Rejected {
            reason: RejectReason::PayloadTooLarge {
                actual: payload_size,
                maximum: config.max_payload_bytes,
            },
        };
    }
    let timestamp = match object.get("timestamp") {
        None => None,
        Some(Value::String(timestamp)) => Some(timestamp.clone()),
        Some(_) => {
            return RecordKind::Rejected {
                reason: RejectReason::InvalidEnvelopeField {
                    field: "timestamp".to_owned(),
                },
            };
        }
    };
    RecordKind::Message(Box::new(ImportedMessage {
        topic: topic.to_owned(),
        payload,
        timestamp,
        payload_bytes,
        core_message: None,
    }))
}

fn rejected_json(error: StrictJsonError, maximum: usize) -> RecordKind {
    let reason = match error {
        StrictJsonError::InvalidUtf8 => RejectReason::InvalidUtf8,
        StrictJsonError::Malformed => RejectReason::MalformedJson,
        StrictJsonError::DuplicateKey(key) => RejectReason::DuplicateJsonKey { key },
        StrictJsonError::TooDeep => RejectReason::JsonTooDeep { maximum },
    };
    RecordKind::Rejected { reason }
}

fn rejected_payload_json(error: StrictJsonError, maximum: usize) -> RecordKind {
    match error {
        StrictJsonError::DuplicateKey(key) => RecordKind::Rejected {
            reason: RejectReason::DuplicateJsonKey { key },
        },
        StrictJsonError::TooDeep => RecordKind::Rejected {
            reason: RejectReason::JsonTooDeep { maximum },
        },
        StrictJsonError::InvalidUtf8 | StrictJsonError::Malformed => RecordKind::Rejected {
            reason: RejectReason::PayloadMalformedJson,
        },
    }
}

fn push_record(
    report: &mut ImportReport,
    raw: &[u8],
    location: RecordLocation,
    mut kind: RecordKind,
) -> bool {
    let raw_digest = sha256(raw);
    let event_id = stable_event_id(&report.source_digest, &raw_digest, &location);
    align_core_event_id(&mut kind, &event_id);
    let event_bytes = serde_json::to_vec(&kind)
        .expect("serializing an in-memory import outcome to a byte vector cannot fail");
    let retained = estimated_retained_bytes(raw.len(), &event_bytes, &kind);
    if report
        .summary
        .retained_bytes
        .saturating_add(retained)
        .saturating_add(RETAINED_LIMIT_MARKER_RESERVE)
        > report.summary.retained_bytes_limit
    {
        push_retained_limit_marker(report, raw_digest, event_id, location);
        return false;
    }
    let event_digest = sha256(&event_bytes);
    report.summary.observe(&kind);
    report.summary.retained_bytes += retained;
    report.records.push(ImportRecord {
        event_id,
        raw_digest,
        event_digest,
        raw: Some(raw.to_vec()),
        location,
        kind,
    });
    true
}

fn push_retained_limit_marker(
    report: &mut ImportReport,
    raw_digest: String,
    event_id: String,
    location: RecordLocation,
) {
    let kind = RecordKind::Rejected {
        reason: RejectReason::RetainedBytesLimitExceeded {
            maximum: report.summary.retained_bytes_limit,
        },
    };
    let event_bytes = serde_json::to_vec(&kind)
        .expect("serializing an in-memory import outcome to a byte vector cannot fail");
    let retained = estimated_retained_bytes(0, &event_bytes, &kind);
    if report.summary.retained_bytes.saturating_add(retained) <= report.summary.retained_bytes_limit
    {
        report.summary.observe(&kind);
        report.summary.retained_bytes += retained;
        report.records.push(ImportRecord {
            event_id,
            raw_digest,
            event_digest: sha256(&event_bytes),
            raw: None,
            location,
            kind,
        });
    } else {
        // The minimum budget reserves enough room for this marker in normal
        // builds. This defensive fallback still makes the report partial.
        report.summary.rejected += 1;
    }
}

fn estimated_retained_bytes(raw_len: usize, event_bytes: &[u8], kind: &RecordKind) -> u64 {
    let hidden_message_bytes = match kind {
        RecordKind::Message(message) => {
            let core_bytes = message.core_message.as_ref().map_or(0, |core| {
                serde_json::to_vec(core)
                    .expect("serializing an in-memory core message cannot fail")
                    .len()
            });
            message.payload_bytes.len().saturating_add(core_bytes)
        }
        RecordKind::Gap { .. } | RecordKind::NonMessage { .. } | RecordKind::Rejected { .. } => 0,
    };
    let variable = raw_len
        .saturating_add(event_bytes.len())
        .saturating_mul(4)
        .saturating_add(hidden_message_bytes);
    let fixed = mem::size_of::<ImportRecord>().saturating_add(3 * 64);
    u64::try_from(variable.saturating_add(fixed)).unwrap_or(u64::MAX)
}

fn new_report(bytes: &[u8], config: ImportConfig) -> ImportReport {
    ImportReport {
        source_digest: sha256(bytes),
        records: Vec::new(),
        summary: ImportSummary {
            retained_bytes_limit: retained_bytes_limit(config),
            ..ImportSummary::default()
        },
    }
}

fn retained_bytes_limit(config: ImportConfig) -> u64 {
    config
        .max_file_bytes
        .saturating_mul(2)
        .clamp(MIN_RETAINED_BYTES, HARD_MAX_RETAINED_BYTES)
}

fn align_core_event_id(kind: &mut RecordKind, event_id: &str) {
    let RecordKind::Message(message) = kind else {
        return;
    };
    let Some(core_message) = &message.core_message else {
        return;
    };
    let mut value = serde_json::to_value(core_message)
        .expect("serializing an in-memory core message cannot fail");
    value["event_id"] = Value::String(event_id.to_owned());
    message.core_message = Some(
        serde_json::from_value(value)
            .expect("changing only a core message event ID preserves its schema"),
    );
}

fn sha256(bytes: &[u8]) -> String {
    hex::encode(Sha256::digest(bytes))
}

fn stable_event_id(source_digest: &str, raw_digest: &str, location: &RecordLocation) -> String {
    let mut digest = Sha256::new();
    digest.update(b"vda5050-lab:event-id:v1\0");
    digest.update(source_digest.as_bytes());
    digest.update(raw_digest.as_bytes());
    digest.update(location.byte_offset.to_le_bytes());
    digest.update(as_u64(location.record_index).to_le_bytes());
    hex::encode(digest.finalize())
}

fn normalize_type(record_type: &str) -> String {
    record_type
        .chars()
        .filter(char::is_ascii_alphanumeric)
        .flat_map(char::to_lowercase)
        .collect()
}

fn as_u64(value: usize) -> u64 {
    u64::try_from(value).unwrap_or(u64::MAX)
}

#[derive(Clone, Copy, Debug)]
struct Span {
    start: usize,
    end: usize,
    line: usize,
}

#[derive(Debug)]
struct ArrayScan {
    spans: Vec<Span>,
    total: usize,
}

#[derive(Debug)]
struct ArrayScanFailure {
    scan: ArrayScan,
    raw_start: usize,
    line: usize,
}

fn scan_array(bytes: &[u8], retain_limit: usize) -> Result<ArrayScan, ArrayScanFailure> {
    let mut cursor = skip_whitespace(bytes, 0);
    let mut line_cursor = LineCursor::default();
    let mut spans = Vec::with_capacity(retain_limit.saturating_add(1).min(1024));
    let mut total = 0usize;
    if bytes.get(cursor) != Some(&b'[') {
        return Err(array_scan_failure(
            spans,
            total,
            cursor,
            line_cursor.at(bytes, cursor),
        ));
    }
    cursor += 1;

    loop {
        cursor = skip_whitespace(bytes, cursor);
        if bytes.get(cursor) == Some(&b']') {
            cursor = skip_whitespace(bytes, cursor + 1);
            return if cursor == bytes.len() {
                Ok(ArrayScan { spans, total })
            } else {
                let line = line_cursor.at(bytes, cursor);
                Err(array_scan_failure(spans, total, cursor, line))
            };
        }
        if cursor >= bytes.len() {
            let line = line_cursor.at(bytes, cursor);
            return Err(array_scan_failure(spans, total, cursor, line));
        }

        let start = cursor;
        let line = line_cursor.at(bytes, start);
        let Ok(end) = scan_element_end(bytes, start) else {
            return Err(array_scan_failure(spans, total, start, line));
        };
        if total <= retain_limit {
            spans.push(Span { start, end, line });
        }
        total += 1;
        cursor = skip_whitespace(bytes, end);
        match bytes.get(cursor) {
            Some(b',') => cursor += 1,
            Some(b']') => {
                cursor = skip_whitespace(bytes, cursor + 1);
                return if cursor == bytes.len() {
                    Ok(ArrayScan { spans, total })
                } else {
                    let line = line_cursor.at(bytes, cursor);
                    Err(array_scan_failure(spans, total, cursor, line))
                };
            }
            _ => {
                let line = line_cursor.at(bytes, cursor);
                return Err(array_scan_failure(spans, total, cursor, line));
            }
        }
    }
}

fn array_scan_failure(
    spans: Vec<Span>,
    total: usize,
    raw_start: usize,
    line: usize,
) -> ArrayScanFailure {
    ArrayScanFailure {
        scan: ArrayScan { spans, total },
        raw_start,
        line,
    }
}

#[derive(Debug)]
struct LineCursor {
    counted_to: usize,
    line: usize,
}

impl Default for LineCursor {
    fn default() -> Self {
        Self {
            counted_to: 0,
            line: 1,
        }
    }
}

impl LineCursor {
    fn at(&mut self, bytes: &[u8], target: usize) -> usize {
        debug_assert!(target >= self.counted_to);
        let bounded_target = target.min(bytes.len());
        for byte in &bytes[self.counted_to..bounded_target] {
            self.line += usize::from(*byte == b'\n');
        }
        self.counted_to = bounded_target;
        self.line
    }
}

fn scan_element_end(bytes: &[u8], start: usize) -> Result<usize, usize> {
    let mut cursor = start;
    let mut stack = Vec::new();
    let mut in_string = false;
    let mut escaped = false;

    while cursor < bytes.len() {
        let byte = bytes[cursor];
        if in_string {
            if escaped {
                escaped = false;
            } else if byte == b'\\' {
                escaped = true;
            } else if byte == b'"' {
                in_string = false;
            }
            cursor += 1;
            continue;
        }
        match byte {
            b'"' => in_string = true,
            b'{' => stack.push(b'}'),
            b'[' => stack.push(b']'),
            b'}' | b']' if stack.last() == Some(&byte) => {
                stack.pop();
            }
            b']' | b',' if stack.is_empty() => break,
            b'}' | b']' => return Err(cursor),
            _ => {}
        }
        cursor += 1;
    }
    if in_string || !stack.is_empty() {
        return Err(cursor);
    }
    let mut end = cursor;
    while end > start && bytes[end - 1].is_ascii_whitespace() {
        end -= 1;
    }
    if end == start {
        return Err(start);
    }
    Ok(end)
}

fn skip_whitespace(bytes: &[u8], mut cursor: usize) -> usize {
    while bytes.get(cursor).is_some_and(u8::is_ascii_whitespace) {
        cursor += 1;
    }
    cursor
}
