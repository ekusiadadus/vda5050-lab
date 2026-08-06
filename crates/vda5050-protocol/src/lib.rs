//! Offline protocol sources and versioned VDA 5050 order comparison.

mod bundle;
mod comparator;
mod errata;

pub use bundle::{
    AnalysisBuildIdentity, ArtifactDigest, BundleError, BundleManifest, BundlePolicy,
    NonNormativeAdvisory, VerifiedBundle,
};
pub use comparator::{
    ComparatorError, ComparatorMetadata, ComparatorProfile, FieldClass, OrderComparison,
    SemanticRelation, compare_order_bytes, compare_orders,
};
pub use errata::{AdvisoryStatus, ErratumOverlay, ErratumStatus};
