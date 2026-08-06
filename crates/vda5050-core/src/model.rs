use serde::{Deserialize, Deserializer, Serialize};
use thiserror::Error;

/// Whether a diagnostic rule's preconditions are known to apply.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum Applicability {
    Applicable,
    NotApplicable,
    Unknown,
}

/// Result of evaluating a diagnostic rule.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum Verdict {
    Pass,
    Fail,
    Inconclusive,
    Unevaluated,
}

/// A validated applicability/verdict pair.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
pub struct Evaluation {
    applicability: Applicability,
    verdict: Verdict,
}

impl Evaluation {
    /// Constructs an evaluation only when the pair represents a legal state.
    ///
    /// # Errors
    ///
    /// Returns [`EvaluationError::IllegalCombination`] when the applicability
    /// and verdict do not form one of the five documented model states.
    pub fn new(applicability: Applicability, verdict: Verdict) -> Result<Self, EvaluationError> {
        let legal = matches!(
            (applicability, verdict),
            (
                Applicability::Applicable,
                Verdict::Pass | Verdict::Fail | Verdict::Inconclusive
            ) | (
                Applicability::NotApplicable | Applicability::Unknown,
                Verdict::Unevaluated
            )
        );

        if legal {
            Ok(Self {
                applicability,
                verdict,
            })
        } else {
            Err(EvaluationError::IllegalCombination {
                applicability,
                verdict,
            })
        }
    }

    #[must_use]
    pub const fn applicability(self) -> Applicability {
        self.applicability
    }

    #[must_use]
    pub const fn verdict(self) -> Verdict {
        self.verdict
    }
}

impl<'de> Deserialize<'de> for Evaluation {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        struct WireEvaluation {
            applicability: Applicability,
            verdict: Verdict,
        }

        let wire = WireEvaluation::deserialize(deserializer)?;
        Self::new(wire.applicability, wire.verdict).map_err(serde::de::Error::custom)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum EvaluationError {
    #[error("illegal evaluation state: applicability={applicability:?}, verdict={verdict:?}")]
    IllegalCombination {
        applicability: Applicability,
        verdict: Verdict,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ReportValidity {
    Valid,
    Partial,
    Invalid,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum DiagnosticDomain {
    ProtocolConformance,
    TraceIntegrity,
    CaptureQuality,
    Migration,
    ToolRuntime,
    Security,
    SafetySignal,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum FindingSeverity {
    Error,
    Warning,
    Info,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ActorRole {
    FleetControl,
    MobileRobot,
    Transport,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum InvestigationTarget {
    MobileRobot,
    FleetControl,
    Transport,
    Unresolved,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum AssertionTrust {
    Verified,
    Declared,
    Untrusted,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum InferenceConfidence {
    Supported,
    Tentative,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Observation {
    id: String,
    description: String,
    event_ids: Vec<String>,
}

impl Observation {
    pub fn new<I, S>(id: impl Into<String>, description: impl Into<String>, event_ids: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        Self {
            id: id.into(),
            description: description.into(),
            event_ids: event_ids.into_iter().map(Into::into).collect(),
        }
    }

    #[must_use]
    pub fn id(&self) -> &str {
        &self.id
    }

    #[must_use]
    pub fn description(&self) -> &str {
        &self.description
    }

    #[must_use]
    pub fn event_ids(&self) -> &[String] {
        &self.event_ids
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Assertion {
    id: String,
    value: String,
    source: String,
    trust: AssertionTrust,
}

impl Assertion {
    pub fn new(
        id: impl Into<String>,
        value: impl Into<String>,
        source: impl Into<String>,
        trust: AssertionTrust,
    ) -> Self {
        Self {
            id: id.into(),
            value: value.into(),
            source: source.into(),
            trust,
        }
    }

    #[must_use]
    pub fn id(&self) -> &str {
        &self.id
    }

    #[must_use]
    pub fn value(&self) -> &str {
        &self.value
    }

    #[must_use]
    pub fn source(&self) -> &str {
        &self.source
    }

    #[must_use]
    pub const fn trust(&self) -> AssertionTrust {
        self.trust
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Inference {
    id: String,
    conclusion: String,
    basis_ids: Vec<String>,
    confidence: InferenceConfidence,
}

impl Inference {
    pub fn new<I, S>(
        id: impl Into<String>,
        conclusion: impl Into<String>,
        basis_ids: I,
        confidence: InferenceConfidence,
    ) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        Self {
            id: id.into(),
            conclusion: conclusion.into(),
            basis_ids: basis_ids.into_iter().map(Into::into).collect(),
            confidence,
        }
    }

    #[must_use]
    pub fn id(&self) -> &str {
        &self.id
    }

    #[must_use]
    pub fn conclusion(&self) -> &str {
        &self.conclusion
    }

    #[must_use]
    pub fn basis_ids(&self) -> &[String] {
        &self.basis_ids
    }

    #[must_use]
    pub const fn confidence(&self) -> InferenceConfidence {
        self.confidence
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Recommendation {
    id: String,
    action: String,
    target: InvestigationTarget,
    evidence_needed: Vec<String>,
}

impl Recommendation {
    pub fn new<I, S>(
        id: impl Into<String>,
        action: impl Into<String>,
        target: InvestigationTarget,
        evidence_needed: I,
    ) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        Self {
            id: id.into(),
            action: action.into(),
            target,
            evidence_needed: evidence_needed.into_iter().map(Into::into).collect(),
        }
    }

    #[must_use]
    pub fn id(&self) -> &str {
        &self.id
    }

    #[must_use]
    pub fn action(&self) -> &str {
        &self.action
    }

    #[must_use]
    pub const fn target(&self) -> InvestigationTarget {
        self.target
    }

    #[must_use]
    pub fn evidence_needed(&self) -> &[String] {
        &self.evidence_needed
    }
}
