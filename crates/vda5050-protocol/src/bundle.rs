use std::collections::BTreeMap;
use std::io::Read;
use std::path::{Component, Path, PathBuf};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;
use vda5050_local_fs::{LocalPathError, TrustedDirectory};

use crate::{AdvisoryStatus, ErratumOverlay};

/// The digest recorded for one immutable protocol artifact.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ArtifactDigest {
    pub sha256: String,
}

/// Non-normative context that cannot override the published base authority.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NonNormativeAdvisory {
    pub id: String,
    pub status: AdvisoryStatus,
    pub source: String,
    pub sha256: String,
}

/// Immutable inputs that define protocol evaluation.
///
/// The running tool build deliberately does not belong to this structure. It
/// is recorded in the analysis report through [`AnalysisBuildIdentity`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BundleManifest {
    pub bundle_id: String,
    pub vda_version: String,
    pub evaluator_api_version: String,
    pub rule_implementation_digest: String,
    pub artifacts: BTreeMap<String, ArtifactDigest>,
    pub errata: Vec<ErratumOverlay>,
    pub advisories: Vec<NonNormativeAdvisory>,
}

/// Build provenance recorded in an analysis report, not a protocol bundle.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AnalysisBuildIdentity {
    pub tool_version: String,
    pub build_digest: String,
    pub dependency_lock_digest: String,
}

/// Resource limits applied while verifying local protocol artifacts.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BundlePolicy {
    pub max_artifact_bytes: u64,
}

impl Default for BundlePolicy {
    fn default() -> Self {
        Self {
            max_artifact_bytes: 64 * 1024 * 1024,
        }
    }
}

#[derive(Debug, Error)]
pub enum BundleError {
    #[error("bundle root cannot be resolved: {path}")]
    InvalidRoot {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("unsafe logical artifact reference: {reference}")]
    UnsafeReference { reference: String },

    #[error("artifact reference is not listed in the bundle: {reference}")]
    UnlistedReference { reference: String },

    #[error("invalid SHA-256 digest for {reference}: {digest}")]
    InvalidDigest { reference: String, digest: String },

    #[error("content-addressed object is missing for {reference}: {path}")]
    MissingObject { reference: String, path: PathBuf },

    #[error("content-addressed object escapes the bundle through a symlink: {path}")]
    SymlinkEscape { path: PathBuf },

    #[error("content-addressed object is not a regular file: {path}")]
    NonRegularObject { path: PathBuf },

    #[error(
        "content-addressed object exceeds the {limit}-byte limit: {path} ({observed} bytes observed)"
    )]
    ArtifactTooLarge {
        path: PathBuf,
        limit: u64,
        observed: u64,
    },

    #[error("failed reading content-addressed object: {path}")]
    ReadObject {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("digest mismatch for {reference}: expected {expected}, observed {observed}")]
    DigestMismatch {
        reference: String,
        expected: String,
        observed: String,
    },
}

/// A bundle whose complete manifest has been verified against local objects.
#[derive(Debug)]
pub struct VerifiedBundle {
    root: TrustedDirectory,
    manifest: BundleManifest,
    policy: BundlePolicy,
}

impl VerifiedBundle {
    /// Verifies every manifest-listed artifact before returning the bundle.
    ///
    /// # Errors
    ///
    /// Returns [`BundleError`] when the root, any logical reference, digest,
    /// object type, containment check, or object content fails verification.
    pub fn open(root: impl AsRef<Path>, manifest: BundleManifest) -> Result<Self, BundleError> {
        Self::open_with_policy(root, manifest, BundlePolicy::default())
    }

    /// Verifies a bundle with an explicit artifact resource policy.
    ///
    /// # Errors
    ///
    /// Returns [`BundleError`] under the same conditions as [`Self::open`], or
    /// when an artifact exceeds `policy.max_artifact_bytes`.
    pub fn open_with_policy(
        root: impl AsRef<Path>,
        manifest: BundleManifest,
        policy: BundlePolicy,
    ) -> Result<Self, BundleError> {
        let original_root = root.as_ref();
        let trusted_root = TrustedDirectory::open(original_root)
            .map_err(|error| invalid_root_error(original_root, error))?;

        validate_manifest_digest("rule implementation", &manifest.rule_implementation_digest)?;
        for (reference, artifact) in &manifest.artifacts {
            validate_reference(reference)?;
            validate_manifest_digest(reference, &artifact.sha256)?;
            verify_object(&trusted_root, reference, &artifact.sha256, policy)?;
        }

        Ok(Self {
            root: trusted_root,
            manifest,
            policy,
        })
    }

    /// Returns verified object bytes for a safe, manifest-listed reference.
    ///
    /// # Errors
    ///
    /// Returns [`BundleError`] when the reference is unsafe or unlisted, or
    /// when its local content-addressed object no longer verifies.
    pub fn resolve(&self, reference: &str) -> Result<Vec<u8>, BundleError> {
        validate_reference(reference)?;
        let artifact = self.manifest.artifacts.get(reference).ok_or_else(|| {
            BundleError::UnlistedReference {
                reference: reference.to_owned(),
            }
        })?;
        verify_object(&self.root, reference, &artifact.sha256, self.policy)
    }

    #[must_use]
    pub const fn manifest(&self) -> &BundleManifest {
        &self.manifest
    }
}

fn validate_manifest_digest(reference: &str, digest: &str) -> Result<(), BundleError> {
    if is_sha256(digest) {
        Ok(())
    } else {
        Err(BundleError::InvalidDigest {
            reference: reference.to_owned(),
            digest: digest.to_owned(),
        })
    }
}

fn is_sha256(digest: &str) -> bool {
    digest.len() == 64 && digest.bytes().all(|byte| byte.is_ascii_hexdigit())
}

fn validate_reference(reference: &str) -> Result<(), BundleError> {
    let normalized = reference.replace('\\', "/");
    let path = Path::new(&normalized);
    let has_uri_scheme = normalized
        .split_once(':')
        .is_some_and(|(scheme, _)| is_uri_scheme(scheme));
    let unsafe_component = path.components().any(|component| {
        matches!(
            component,
            Component::ParentDir | Component::RootDir | Component::Prefix(_)
        )
    });

    if normalized.is_empty() || has_uri_scheme || path.is_absolute() || unsafe_component {
        return Err(BundleError::UnsafeReference {
            reference: reference.to_owned(),
        });
    }
    Ok(())
}

fn is_uri_scheme(candidate: &str) -> bool {
    let mut chars = candidate.chars();
    chars
        .next()
        .is_some_and(|first| first.is_ascii_alphabetic())
        && chars.all(|character| {
            character.is_ascii_alphanumeric() || matches!(character, '+' | '-' | '.')
        })
}

fn verify_object(
    root: &TrustedDirectory,
    reference: &str,
    digest: &str,
    policy: BundlePolicy,
) -> Result<Vec<u8>, BundleError> {
    let object_reference = Path::new("objects").join(digest);
    let object_path = root.path().join(&object_reference);
    let file = root
        .open_file(&object_reference)
        .map_err(|error| object_open_error(reference, &object_path, error))?;
    let metadata = file.metadata().map_err(|source| BundleError::ReadObject {
        path: object_path.clone(),
        source,
    })?;

    if metadata.len() > policy.max_artifact_bytes {
        return Err(BundleError::ArtifactTooLarge {
            path: object_path,
            limit: policy.max_artifact_bytes,
            observed: metadata.len(),
        });
    }

    let mut bytes = Vec::new();
    file.take(policy.max_artifact_bytes.saturating_add(1))
        .read_to_end(&mut bytes)
        .map_err(|source| BundleError::ReadObject {
            path: object_path.clone(),
            source,
        })?;
    let observed = u64::try_from(bytes.len()).unwrap_or(u64::MAX);
    if observed > policy.max_artifact_bytes {
        return Err(BundleError::ArtifactTooLarge {
            path: object_path,
            limit: policy.max_artifact_bytes,
            observed,
        });
    }
    let observed = hex::encode(Sha256::digest(&bytes));
    if observed != digest.to_ascii_lowercase() {
        return Err(BundleError::DigestMismatch {
            reference: reference.to_owned(),
            expected: digest.to_owned(),
            observed,
        });
    }
    Ok(bytes)
}

fn invalid_root_error(path: &Path, error: LocalPathError) -> BundleError {
    let source = match error {
        LocalPathError::Io { source, .. } => source,
        other => std::io::Error::new(std::io::ErrorKind::InvalidInput, other),
    };
    BundleError::InvalidRoot {
        path: path.to_path_buf(),
        source,
    }
}

fn object_open_error(reference: &str, path: &Path, error: LocalPathError) -> BundleError {
    match error {
        LocalPathError::Symlink(_) => BundleError::SymlinkEscape {
            path: path.to_path_buf(),
        },
        LocalPathError::NotRegular(_) => BundleError::NonRegularObject {
            path: path.to_path_buf(),
        },
        LocalPathError::Io { source, .. } if source.kind() == std::io::ErrorKind::NotFound => {
            BundleError::MissingObject {
                reference: reference.to_owned(),
                path: path.to_path_buf(),
            }
        }
        LocalPathError::Io { source, .. } => BundleError::ReadObject {
            path: path.to_path_buf(),
            source,
        },
        other => BundleError::ReadObject {
            path: path.to_path_buf(),
            source: std::io::Error::new(std::io::ErrorKind::InvalidInput, other),
        },
    }
}
