#![forbid(unsafe_code)]

//! Shared, evidence-bounded value types for VDA 5050 diagnosis.
//!
//! The crate deliberately keeps captured facts, trusted assertions, derived
//! inferences, and recommended next actions in separate types. It contains no
//! network or broker code.

mod capture;
mod model;

pub use capture::{
    AdapterWarning, CaptureClosed, CaptureGap, CaptureLifecycle, CaptureLifecycleError,
    CaptureOpened, CapturePoint, CaptureRecord, ClockDescriptor, ClockEpochChanged,
    ClockWrapPolicy, FinalManifest, FinalManifestError, MessageObserved, PayloadReference,
};
pub use model::{
    ActorRole, Applicability, Assertion, AssertionTrust, DiagnosticDomain, Evaluation,
    EvaluationError, FindingSeverity, Inference, InferenceConfidence, InvestigationTarget,
    Observation, Recommendation, ReportValidity, Verdict,
};
