use serde::{Deserialize, Serialize};

/// Publication status of a possible normative erratum.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ErratumStatus {
    FormallyPublished,
    Acknowledged,
    Proposed,
}

/// Status for context that is always non-normative.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum AdvisoryStatus {
    Acknowledged,
    Proposed,
}

impl AdvisoryStatus {
    /// Advisories never change a normative verdict.
    #[must_use]
    pub const fn is_normative(self) -> bool {
        false
    }
}

/// Clause-scoped correction layered over a published base specification.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ErratumOverlay {
    pub id: String,
    pub status: ErratumStatus,
    pub affected_version: String,
    pub exact_clause: String,
    pub correction: String,
    pub immutable_source: String,
    pub source_is_immutable: bool,
    pub sha256: String,
}

impl ErratumOverlay {
    /// Returns true only when this formally published correction can override
    /// the exact selected version and clause.
    #[must_use]
    pub fn is_normative_for(&self, version: &str, clause: &str) -> bool {
        self.status == ErratumStatus::FormallyPublished
            && self.affected_version == version
            && self.exact_clause == clause
            && is_exact_clause(&self.exact_clause)
            && !self.correction.trim().is_empty()
            && self.source_is_immutable
            && !self.immutable_source.trim().is_empty()
            && is_sha256(&self.sha256)
    }
}

fn is_exact_clause(clause: &str) -> bool {
    !clause.trim().is_empty()
        && !clause
            .chars()
            .any(|character| matches!(character, '*' | '?' | '[' | ']'))
}

fn is_sha256(digest: &str) -> bool {
    digest.len() == 64 && digest.bytes().all(|byte| byte.is_ascii_hexdigit())
}
