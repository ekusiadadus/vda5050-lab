#![forbid(unsafe_code)]

//! Deterministic machine reports and bounded, terminal-safe projections.
//!
//! Raw captured payload bytes are intentionally absent from [`IncidentReport`].
//! Reports refer to source evidence by event ID, offset, and digest.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use thiserror::Error;
use vda5050_core::{
    Assertion, DiagnosticDomain, Evaluation, FindingSeverity, Inference, Observation,
    Recommendation, ReportValidity,
};

pub type CanonicalReport = IncidentReport;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IncidentReport {
    schema_version: String,
    report_id: String,
    validity: ReportValidity,
    domain: DiagnosticDomain,
    evaluation: Evaluation,
    finding_severity: FindingSeverity,
    summary: String,
    observations: Vec<Observation>,
    assertions: Vec<Assertion>,
    inferences: Vec<Inference>,
    recommendations: Vec<Recommendation>,
    specification_references: Vec<SpecificationReference>,
    evidence_references: Vec<EvidenceReference>,
    missing_evidence: Vec<String>,
}

impl IncidentReport {
    pub fn new(
        report_id: impl Into<String>,
        validity: ReportValidity,
        domain: DiagnosticDomain,
        evaluation: Evaluation,
        finding_severity: FindingSeverity,
        summary: impl Into<String>,
    ) -> Self {
        Self {
            schema_version: "vda5050-lab.report/1".to_owned(),
            report_id: report_id.into(),
            validity,
            domain,
            evaluation,
            finding_severity,
            summary: summary.into(),
            observations: Vec::new(),
            assertions: Vec::new(),
            inferences: Vec::new(),
            recommendations: Vec::new(),
            specification_references: Vec::new(),
            evidence_references: Vec::new(),
            missing_evidence: Vec::new(),
        }
    }

    #[must_use]
    pub fn with_evidence_layers(
        mut self,
        observations: Vec<Observation>,
        assertions: Vec<Assertion>,
        inferences: Vec<Inference>,
        recommendations: Vec<Recommendation>,
    ) -> Self {
        self.observations = observations;
        self.assertions = assertions;
        self.inferences = inferences;
        self.recommendations = recommendations;
        self
    }

    #[must_use]
    pub fn with_specification_references(
        mut self,
        references: Vec<SpecificationReference>,
    ) -> Self {
        self.specification_references = references;
        self
    }

    #[must_use]
    pub fn with_evidence_references(mut self, references: Vec<EvidenceReference>) -> Self {
        self.evidence_references = references;
        self
    }

    #[must_use]
    pub fn with_missing_evidence<I, S>(mut self, missing_evidence: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.missing_evidence = missing_evidence.into_iter().map(Into::into).collect();
        self
    }

    #[must_use]
    pub fn schema_version(&self) -> &str {
        &self.schema_version
    }

    #[must_use]
    pub fn report_id(&self) -> &str {
        &self.report_id
    }

    #[must_use]
    pub const fn validity(&self) -> ReportValidity {
        self.validity
    }

    #[must_use]
    pub const fn domain(&self) -> DiagnosticDomain {
        self.domain
    }

    #[must_use]
    pub const fn evaluation(&self) -> Evaluation {
        self.evaluation
    }

    #[must_use]
    pub const fn finding_severity(&self) -> FindingSeverity {
        self.finding_severity
    }

    #[must_use]
    pub fn summary(&self) -> &str {
        &self.summary
    }

    #[must_use]
    pub fn observations(&self) -> &[Observation] {
        &self.observations
    }

    #[must_use]
    pub fn assertions(&self) -> &[Assertion] {
        &self.assertions
    }

    #[must_use]
    pub fn inferences(&self) -> &[Inference] {
        &self.inferences
    }

    #[must_use]
    pub fn recommendations(&self) -> &[Recommendation] {
        &self.recommendations
    }

    #[must_use]
    pub fn specification_references(&self) -> &[SpecificationReference] {
        &self.specification_references
    }

    #[must_use]
    pub fn evidence_references(&self) -> &[EvidenceReference] {
        &self.evidence_references
    }

    #[must_use]
    pub fn missing_evidence(&self) -> &[String] {
        &self.missing_evidence
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SpecificationReference {
    source_id: String,
    section: String,
    source_digest: String,
}

impl SpecificationReference {
    pub fn new(
        source_id: impl Into<String>,
        section: impl Into<String>,
        source_digest: impl Into<String>,
    ) -> Self {
        Self {
            source_id: source_id.into(),
            section: section.into(),
            source_digest: source_digest.into(),
        }
    }

    #[must_use]
    pub fn source_id(&self) -> &str {
        &self.source_id
    }

    #[must_use]
    pub fn section(&self) -> &str {
        &self.section
    }

    #[must_use]
    pub fn source_digest(&self) -> &str {
        &self.source_digest
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EvidenceReference {
    event_id: String,
    source_offset: Option<u64>,
    payload_sha256: String,
    note: String,
}

impl EvidenceReference {
    pub fn new(
        event_id: impl Into<String>,
        source_offset: Option<u64>,
        payload_sha256: impl Into<String>,
        note: impl Into<String>,
    ) -> Self {
        Self {
            event_id: event_id.into(),
            source_offset,
            payload_sha256: payload_sha256.into(),
            note: note.into(),
        }
    }

    #[must_use]
    pub fn event_id(&self) -> &str {
        &self.event_id
    }

    #[must_use]
    pub const fn source_offset(&self) -> Option<u64> {
        self.source_offset
    }

    #[must_use]
    pub fn payload_sha256(&self) -> &str {
        &self.payload_sha256
    }

    #[must_use]
    pub fn note(&self) -> &str {
        &self.note
    }
}

/// Serializes a report after recursively sorting all object keys.
///
/// # Errors
///
/// Returns [`CanonicalJsonError`] if the typed report cannot be converted to
/// or encoded from its canonical JSON value.
pub fn to_canonical_json(report: &IncidentReport) -> Result<String, CanonicalJsonError> {
    to_canonical_json_value(report)
}

/// Serializes any report envelope after recursively sorting all object keys.
///
/// This is intended for CLI envelopes which contain an [`IncidentReport`] and
/// additional deterministic metadata. Raw trace payloads should not be passed
/// to this function because canonicalization is not redaction.
///
/// # Errors
///
/// Returns [`CanonicalJsonError`] if `value` cannot be converted to or encoded
/// from its canonical JSON representation.
pub fn to_canonical_json_value<T>(value: &T) -> Result<String, CanonicalJsonError>
where
    T: Serialize + ?Sized,
{
    let value = serde_json::to_value(value)?;
    Ok(serde_json::to_string(&sort_object_keys(value))?)
}

#[derive(Debug, Error)]
#[error("failed to serialize canonical report: {0}")]
pub struct CanonicalJsonError(#[from] serde_json::Error);

fn sort_object_keys(value: Value) -> Value {
    match value {
        Value::Array(values) => Value::Array(values.into_iter().map(sort_object_keys).collect()),
        Value::Object(values) => {
            let mut entries: Vec<_> = values.into_iter().collect();
            entries.sort_unstable_by(|left, right| left.0.cmp(&right.0));
            Value::Object(
                entries
                    .into_iter()
                    .map(|(key, value)| (key, sort_object_keys(value)))
                    .collect(),
            )
        }
        scalar => scalar,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RenderLimits {
    max_total_bytes: usize,
    max_field_bytes: usize,
}

impl RenderLimits {
    /// Creates validated per-field and whole-report terminal limits.
    ///
    /// # Errors
    ///
    /// Returns [`RenderLimitError`] for a total below the safe metadata floor,
    /// a zero field limit, or a field limit larger than the total limit.
    pub fn new(max_total_bytes: usize, max_field_bytes: usize) -> Result<Self, RenderLimitError> {
        if max_total_bytes < 128 {
            return Err(RenderLimitError::TotalTooSmall);
        }
        if max_field_bytes == 0 || max_field_bytes > max_total_bytes {
            return Err(RenderLimitError::InvalidFieldLimit);
        }
        Ok(Self {
            max_total_bytes,
            max_field_bytes,
        })
    }

    #[must_use]
    pub const fn max_total_bytes(self) -> usize {
        self.max_total_bytes
    }

    #[must_use]
    pub const fn max_field_bytes(self) -> usize {
        self.max_field_bytes
    }
}

impl Default for RenderLimits {
    fn default() -> Self {
        Self {
            max_total_bytes: 32 * 1_024,
            max_field_bytes: 4 * 1_024,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum RenderLimitError {
    #[error("terminal output limit must be at least 128 bytes")]
    TotalTooSmall,
    #[error("field limit must be nonzero and no greater than the total limit")]
    InvalidFieldLimit,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TruncationMetadata {
    field: String,
    reference: String,
    sha256: String,
    original_bytes: usize,
    rendered_bytes: usize,
}

impl TruncationMetadata {
    #[must_use]
    pub fn field(&self) -> &str {
        &self.field
    }

    #[must_use]
    pub fn reference(&self) -> &str {
        &self.reference
    }

    #[must_use]
    pub fn sha256(&self) -> &str {
        &self.sha256
    }

    #[must_use]
    pub const fn original_bytes(&self) -> usize {
        self.original_bytes
    }

    #[must_use]
    pub const fn rendered_bytes(&self) -> usize {
        self.rendered_bytes
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RenderedTerminal {
    text: String,
    truncations: Vec<TruncationMetadata>,
}

impl RenderedTerminal {
    #[must_use]
    pub fn text(&self) -> &str {
        &self.text
    }

    #[must_use]
    pub fn truncations(&self) -> &[TruncationMetadata] {
        &self.truncations
    }

    #[must_use]
    pub fn into_text(self) -> String {
        self.text
    }
}

/// Creates a human-readable projection which never includes raw capture bytes.
#[must_use]
pub fn render_terminal(report: &IncidentReport, limits: RenderLimits) -> RenderedTerminal {
    let mut text = String::new();
    let mut truncations = Vec::new();

    push_literal(&mut text, "VDA 5050 incident report\n");
    push_field(
        &mut text,
        &mut truncations,
        "Report",
        report.report_id(),
        "report://report_id",
        limits.max_field_bytes,
    );
    push_field(
        &mut text,
        &mut truncations,
        "Summary",
        report.summary(),
        "report://summary",
        limits.max_field_bytes,
    );
    push_literal(
        &mut text,
        &format!(
            "Validity: {}\nDomain: {}\nApplicability: {}\nVerdict: {}\nSeverity: {}\n",
            validity_label(report.validity()),
            domain_label(report.domain()),
            applicability_label(report.evaluation().applicability()),
            verdict_label(report.evaluation().verdict()),
            severity_label(report.finding_severity())
        ),
    );

    for observation in report.observations() {
        push_field(
            &mut text,
            &mut truncations,
            "Observation",
            observation.description(),
            &format!(
                "report://observations/{}",
                safe_path_segment(observation.id())
            ),
            limits.max_field_bytes,
        );
    }
    for assertion in report.assertions() {
        push_field(
            &mut text,
            &mut truncations,
            "Assertion",
            assertion.value(),
            &format!("report://assertions/{}", safe_path_segment(assertion.id())),
            limits.max_field_bytes,
        );
    }
    for inference in report.inferences() {
        push_field(
            &mut text,
            &mut truncations,
            "Inference",
            inference.conclusion(),
            &format!("report://inferences/{}", safe_path_segment(inference.id())),
            limits.max_field_bytes,
        );
    }
    for recommendation in report.recommendations() {
        push_field(
            &mut text,
            &mut truncations,
            "Next action",
            recommendation.action(),
            &format!(
                "report://recommendations/{}",
                safe_path_segment(recommendation.id())
            ),
            limits.max_field_bytes,
        );
    }
    for (index, missing) in report.missing_evidence().iter().enumerate() {
        push_field(
            &mut text,
            &mut truncations,
            "Missing evidence",
            missing,
            &format!("report://missing_evidence/{index}"),
            limits.max_field_bytes,
        );
    }

    if text.len() > limits.max_total_bytes {
        let (bounded, metadata) = truncate(
            "terminal",
            "report://terminal",
            &text,
            limits.max_total_bytes,
        );
        text = bounded;
        if let Some(metadata) = metadata {
            truncations.push(metadata);
        }
    }

    RenderedTerminal { text, truncations }
}

fn push_literal(output: &mut String, value: &str) {
    output.push_str(value);
}

fn push_field(
    output: &mut String,
    truncations: &mut Vec<TruncationMetadata>,
    label: &str,
    value: &str,
    reference: &str,
    limit: usize,
) {
    let safe = strip_terminal_controls(value);
    let (rendered, metadata) = truncate(label, reference, &safe, limit);
    output.push_str(label);
    output.push_str(": ");
    output.push_str(&rendered);
    output.push('\n');
    if let Some(metadata) = metadata {
        truncations.push(metadata);
    }
}

fn truncate(
    field: &str,
    reference: &str,
    value: &str,
    limit: usize,
) -> (String, Option<TruncationMetadata>) {
    if value.len() <= limit {
        return (value.to_owned(), None);
    }

    let digest = digest(value.as_bytes());
    let marker = format!(
        " [truncated ref={} sha256={}]",
        safe_path_segment(reference),
        &digest[..12]
    );
    let prefix_budget = limit.saturating_sub(marker.len());
    let prefix_end = floor_char_boundary(value, prefix_budget);
    let mut rendered = value[..prefix_end].to_owned();
    if marker.len() <= limit {
        rendered.push_str(&marker);
    } else {
        let fallback = "[truncated]";
        rendered.clear();
        rendered.push_str(&fallback[..fallback.len().min(limit)]);
    }

    let metadata = TruncationMetadata {
        field: field.to_owned(),
        reference: reference.to_owned(),
        sha256: digest,
        original_bytes: value.len(),
        rendered_bytes: rendered.len(),
    };
    (rendered, Some(metadata))
}

fn floor_char_boundary(value: &str, at: usize) -> usize {
    let mut index = at.min(value.len());
    while index > 0 && !value.is_char_boundary(index) {
        index -= 1;
    }
    index
}

fn strip_terminal_controls(input: &str) -> String {
    let mut output = String::with_capacity(input.len());
    let mut characters = input.chars().peekable();

    while let Some(character) = characters.next() {
        match character {
            '\u{001b}' => consume_escape_sequence(&mut characters),
            '\u{009b}' => consume_csi(&mut characters),
            '\u{009d}' => consume_control_string(&mut characters),
            '\u{200e}' | '\u{200f}' | '\u{202a}'..='\u{202e}' | '\u{2066}'..='\u{2069}' => {}
            control if control.is_control() => {}
            printable => output.push(printable),
        }
    }
    output
}

fn consume_escape_sequence<I>(characters: &mut std::iter::Peekable<I>)
where
    I: Iterator<Item = char>,
{
    match characters.next() {
        Some('[') => consume_csi(characters),
        Some(']' | 'P' | '^' | '_') => consume_control_string(characters),
        Some(_) | None => {}
    }
}

fn consume_csi<I>(characters: &mut std::iter::Peekable<I>)
where
    I: Iterator<Item = char>,
{
    for character in characters.by_ref() {
        if ('\u{0040}'..='\u{007e}').contains(&character) {
            break;
        }
    }
}

fn consume_control_string<I>(characters: &mut std::iter::Peekable<I>)
where
    I: Iterator<Item = char>,
{
    while let Some(character) = characters.next() {
        if character == '\u{0007}' || character == '\u{009c}' {
            break;
        }
        if character == '\u{001b}' && characters.next_if_eq(&'\\').is_some() {
            break;
        }
    }
}

fn safe_path_segment(value: &str) -> String {
    strip_terminal_controls(value)
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() || matches!(character, '/' | ':' | '_' | '-' | '.')
            {
                character
            } else {
                '_'
            }
        })
        .collect()
}

fn digest(bytes: &[u8]) -> String {
    hex::encode(Sha256::digest(bytes))
}

const fn validity_label(validity: ReportValidity) -> &'static str {
    match validity {
        ReportValidity::Valid => "VALID",
        ReportValidity::Partial => "PARTIAL",
        ReportValidity::Invalid => "INVALID",
    }
}

const fn domain_label(domain: DiagnosticDomain) -> &'static str {
    match domain {
        DiagnosticDomain::ProtocolConformance => "PROTOCOL_CONFORMANCE",
        DiagnosticDomain::TraceIntegrity => "TRACE_INTEGRITY",
        DiagnosticDomain::CaptureQuality => "CAPTURE_QUALITY",
        DiagnosticDomain::Migration => "MIGRATION",
        DiagnosticDomain::ToolRuntime => "TOOL_RUNTIME",
        DiagnosticDomain::Security => "SECURITY",
        DiagnosticDomain::SafetySignal => "SAFETY_SIGNAL",
    }
}

const fn applicability_label(applicability: vda5050_core::Applicability) -> &'static str {
    match applicability {
        vda5050_core::Applicability::Applicable => "APPLICABLE",
        vda5050_core::Applicability::NotApplicable => "NOT_APPLICABLE",
        vda5050_core::Applicability::Unknown => "UNKNOWN",
    }
}

const fn verdict_label(verdict: vda5050_core::Verdict) -> &'static str {
    match verdict {
        vda5050_core::Verdict::Pass => "PASS",
        vda5050_core::Verdict::Fail => "FAIL",
        vda5050_core::Verdict::Inconclusive => "INCONCLUSIVE",
        vda5050_core::Verdict::Unevaluated => "UNEVALUATED",
    }
}

const fn severity_label(severity: FindingSeverity) -> &'static str {
    match severity {
        FindingSeverity::Error => "ERROR",
        FindingSeverity::Warning => "WARNING",
        FindingSeverity::Info => "INFO",
    }
}
