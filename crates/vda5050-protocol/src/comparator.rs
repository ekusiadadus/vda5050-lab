use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};
use serde_json::{Map, Number, Value};
use sha2::{Digest, Sha256};
use thiserror::Error;

const COMPARATOR_ID: &str = "vda5050-order";
const COMPARATOR_VERSION: &str = "1.0.0";

/// Conservative semantic result for two order payloads.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum SemanticRelation {
    Equal,
    Changed,
    Unknown,
}

/// Semantic treatment for a JSON-pointer field path.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum FieldClass {
    Significant,
    Excluded,
    Unknown,
}

/// Versioned field policy used by the order comparator.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ComparatorProfile {
    id: String,
    version: String,
    fields: BTreeMap<String, FieldClass>,
}

impl ComparatorProfile {
    /// Constructs a versioned profile with only the default VDA republication
    /// exclusions (`headerId` and `timestamp`).
    #[must_use]
    pub fn new(id: impl Into<String>, version: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            version: version.into(),
            fields: BTreeMap::from([
                ("/headerId".to_owned(), FieldClass::Excluded),
                ("/timestamp".to_owned(), FieldClass::Excluded),
            ]),
        }
    }

    /// Default, conservative VDA comparator policy.
    #[must_use]
    pub fn default_vda() -> Self {
        Self::new("vda-default", "1.0.0")
    }

    /// Classifies an exact JSON-pointer path for this versioned profile.
    pub fn classify(&mut self, path: impl Into<String>, class: FieldClass) {
        self.fields.insert(path.into(), class);
    }

    fn class_for(&self, path: &str, force_significant: bool) -> FieldDecision {
        if let Some(class) = self.fields.get(path) {
            return FieldDecision {
                class: *class,
                force_descendants: *class == FieldClass::Significant,
            };
        }
        if force_significant {
            FieldDecision {
                class: FieldClass::Significant,
                force_descendants: true,
            }
        } else if is_known_order_path(path) {
            FieldDecision {
                class: FieldClass::Significant,
                force_descendants: is_opaque_significant_path(path),
            }
        } else {
            FieldDecision {
                class: FieldClass::Unknown,
                force_descendants: false,
            }
        }
    }

    fn excluded_fields(&self) -> Vec<String> {
        self.fields
            .iter()
            .filter_map(|(path, class)| (*class == FieldClass::Excluded).then_some(path.clone()))
            .collect()
    }
}

impl Default for ComparatorProfile {
    fn default() -> Self {
        Self::default_vda()
    }
}

/// Reproducibility metadata attached to every comparison.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ComparatorMetadata {
    pub comparator_id: String,
    pub comparator_version: String,
    pub config_digest: String,
    pub excluded_fields: Vec<String>,
    pub profile_id: String,
    pub profile_version: String,
}

/// Independent byte, structural, and semantic comparison levels.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OrderComparison {
    /// `None` means the caller supplied parsed values, so original byte
    /// identity was not observable.
    pub byte_equal: Option<bool>,
    pub structurally_equal: bool,
    pub semantic_relation: SemanticRelation,
    pub metadata: ComparatorMetadata,
}

#[derive(Debug, Error)]
pub enum ComparatorError {
    #[error("left order is not valid JSON: {0}")]
    InvalidLeftJson(#[source] serde_json::Error),
    #[error("right order is not valid JSON: {0}")]
    InvalidRightJson(#[source] serde_json::Error),
    #[error("JSON number is outside the supported exact-decimal contract: {0}")]
    UnsupportedNumber(String),
    #[error("comparator profile cannot be serialized: {0}")]
    InvalidProfile(#[source] serde_json::Error),
}

/// Compares original JSON bytes, preserving observable byte identity.
///
/// # Errors
///
/// Returns [`ComparatorError`] for malformed JSON, an unsupported exact
/// number representation, or a profile that cannot be serialized.
pub fn compare_order_bytes(
    left: &[u8],
    right: &[u8],
    profile: &ComparatorProfile,
) -> Result<OrderComparison, ComparatorError> {
    let left_value = serde_json::from_slice(left).map_err(ComparatorError::InvalidLeftJson)?;
    let right_value = serde_json::from_slice(right).map_err(ComparatorError::InvalidRightJson)?;
    compare_values(&left_value, &right_value, profile, Some(left == right))
}

/// Compares parsed orders without inventing a claim about their original bytes.
///
/// # Errors
///
/// Returns [`ComparatorError`] for an unsupported exact number representation
/// or a profile that cannot be serialized.
pub fn compare_orders(
    left: &Value,
    right: &Value,
    profile: &ComparatorProfile,
) -> Result<OrderComparison, ComparatorError> {
    compare_values(left, right, profile, None)
}

fn compare_values(
    left: &Value,
    right: &Value,
    profile: &ComparatorProfile,
    byte_equal: Option<bool>,
) -> Result<OrderComparison, ComparatorError> {
    let structurally_equal = structural_equal(left, right)?;
    let semantic_relation = semantic_relation(left, right, profile)?;
    let profile_bytes = serde_json::to_vec(profile).map_err(ComparatorError::InvalidProfile)?;
    let mut config_hasher = Sha256::new();
    config_hasher.update(COMPARATOR_ID.as_bytes());
    config_hasher.update([0]);
    config_hasher.update(COMPARATOR_VERSION.as_bytes());
    config_hasher.update([0]);
    config_hasher.update(profile_bytes);

    Ok(OrderComparison {
        byte_equal,
        structurally_equal,
        semantic_relation,
        metadata: ComparatorMetadata {
            comparator_id: COMPARATOR_ID.to_owned(),
            comparator_version: COMPARATOR_VERSION.to_owned(),
            config_digest: hex::encode(config_hasher.finalize()),
            excluded_fields: profile.excluded_fields(),
            profile_id: profile.id.clone(),
            profile_version: profile.version.clone(),
        },
    })
}

fn structural_equal(left: &Value, right: &Value) -> Result<bool, ComparatorError> {
    match (left, right) {
        (Value::Null, Value::Null) => Ok(true),
        (Value::Bool(left), Value::Bool(right)) => Ok(left == right),
        (Value::Number(left), Value::Number(right)) => numeric_equal(left, right),
        (Value::String(left), Value::String(right)) => Ok(left == right),
        (Value::Array(left), Value::Array(right)) => {
            if left.len() != right.len() {
                return Ok(false);
            }
            for (left, right) in left.iter().zip(right) {
                if !structural_equal(left, right)? {
                    return Ok(false);
                }
            }
            Ok(true)
        }
        (Value::Object(left), Value::Object(right)) => object_equal(left, right),
        _ => Ok(false),
    }
}

fn object_equal(
    left: &Map<String, Value>,
    right: &Map<String, Value>,
) -> Result<bool, ComparatorError> {
    if left.len() != right.len() {
        return Ok(false);
    }
    for (key, left_value) in left {
        let Some(right_value) = right.get(key) else {
            return Ok(false);
        };
        if !structural_equal(left_value, right_value)? {
            return Ok(false);
        }
    }
    Ok(true)
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct DecimalKey {
    negative: bool,
    coefficient: String,
    exponent: i64,
}

fn numeric_equal(left: &Number, right: &Number) -> Result<bool, ComparatorError> {
    Ok(decimal_key(&left.to_string())? == decimal_key(&right.to_string())?)
}

fn decimal_key(raw: &str) -> Result<DecimalKey, ComparatorError> {
    let (negative, unsigned) = raw
        .strip_prefix('-')
        .map_or((false, raw), |value| (true, value));
    let (mantissa, exponent) = unsigned
        .split_once(['e', 'E'])
        .map_or((unsigned, "0"), |parts| parts);
    let explicit_exponent = exponent
        .parse::<i64>()
        .map_err(|_| ComparatorError::UnsupportedNumber(raw.to_owned()))?;
    let (integer, fraction) = mantissa
        .split_once('.')
        .map_or((mantissa, ""), |parts| parts);
    let fraction_len = i64::try_from(fraction.len())
        .map_err(|_| ComparatorError::UnsupportedNumber(raw.to_owned()))?;
    let mut coefficient = format!("{integer}{fraction}")
        .trim_start_matches('0')
        .to_owned();

    if coefficient.is_empty() {
        return Ok(DecimalKey {
            negative: false,
            coefficient: "0".to_owned(),
            exponent: 0,
        });
    }

    let trailing_zeroes = coefficient.len() - coefficient.trim_end_matches('0').len();
    coefficient.truncate(coefficient.len() - trailing_zeroes);
    let trailing_zeroes = i64::try_from(trailing_zeroes)
        .map_err(|_| ComparatorError::UnsupportedNumber(raw.to_owned()))?;
    let exponent = explicit_exponent
        .checked_sub(fraction_len)
        .and_then(|value| value.checked_add(trailing_zeroes))
        .ok_or_else(|| ComparatorError::UnsupportedNumber(raw.to_owned()))?;

    Ok(DecimalKey {
        negative,
        coefficient,
        exponent,
    })
}

#[derive(Debug, Clone, Copy, Default)]
struct SemanticSummary {
    changed: bool,
    unknown: bool,
}

#[derive(Debug, Clone, Copy)]
struct FieldDecision {
    class: FieldClass,
    force_descendants: bool,
}

impl SemanticSummary {
    fn merge(&mut self, other: Self) {
        self.changed |= other.changed;
        self.unknown |= other.unknown;
    }

    const fn relation(self) -> SemanticRelation {
        if self.changed {
            SemanticRelation::Changed
        } else if self.unknown {
            SemanticRelation::Unknown
        } else {
            SemanticRelation::Equal
        }
    }
}

fn semantic_relation(
    left: &Value,
    right: &Value,
    profile: &ComparatorProfile,
) -> Result<SemanticRelation, ComparatorError> {
    Ok(compare_semantic(left, right, "", profile, false, false)?.relation())
}

fn compare_semantic(
    left: &Value,
    right: &Value,
    path: &str,
    profile: &ComparatorProfile,
    difference_is_significant: bool,
    force_descendants: bool,
) -> Result<SemanticSummary, ComparatorError> {
    if structural_equal(left, right)? {
        return Ok(SemanticSummary::default());
    }

    match (left, right) {
        (Value::Object(left), Value::Object(right)) => {
            compare_semantic_objects(left, right, path, profile, force_descendants)
        }
        (Value::Array(left), Value::Array(right)) => {
            if left.len() != right.len() {
                return Ok(summary_for_difference(difference_is_significant));
            }
            let mut summary = SemanticSummary::default();
            for (index, (left, right)) in left.iter().zip(right).enumerate() {
                let child_path = format!("{path}/{index}");
                summary.merge(compare_semantic(
                    left,
                    right,
                    &child_path,
                    profile,
                    difference_is_significant,
                    force_descendants,
                )?);
            }
            Ok(summary)
        }
        _ => Ok(summary_for_difference(difference_is_significant)),
    }
}

fn compare_semantic_objects(
    left: &Map<String, Value>,
    right: &Map<String, Value>,
    path: &str,
    profile: &ComparatorProfile,
    force_descendants: bool,
) -> Result<SemanticSummary, ComparatorError> {
    let keys: BTreeSet<&str> = left
        .keys()
        .chain(right.keys())
        .map(String::as_str)
        .collect();
    let mut summary = SemanticSummary::default();

    for key in keys {
        let child_path = format!("{path}/{}", escape_json_pointer(key));
        let decision = profile.class_for(&child_path, force_descendants);
        if decision.class == FieldClass::Excluded {
            continue;
        }
        match (left.get(key), right.get(key)) {
            (Some(left), Some(right)) => {
                if structural_equal(left, right)? {
                    continue;
                }
                match decision.class {
                    FieldClass::Significant => {
                        summary.merge(compare_semantic(
                            left,
                            right,
                            &child_path,
                            profile,
                            true,
                            decision.force_descendants,
                        )?);
                    }
                    FieldClass::Unknown => summary.unknown = true,
                    FieldClass::Excluded => {}
                }
            }
            (Some(_), None) | (None, Some(_)) => match decision.class {
                FieldClass::Significant => summary.changed = true,
                FieldClass::Unknown => summary.unknown = true,
                FieldClass::Excluded => {}
            },
            (None, None) => unreachable!("key originated from at least one object"),
        }
    }
    Ok(summary)
}

const fn summary_for_difference(significant: bool) -> SemanticSummary {
    if significant {
        SemanticSummary {
            changed: true,
            unknown: false,
        }
    } else {
        SemanticSummary {
            changed: false,
            unknown: true,
        }
    }
}

fn escape_json_pointer(key: &str) -> String {
    key.replace('~', "~0").replace('/', "~1")
}

fn is_known_order_path(path: &str) -> bool {
    let parts: Vec<&str> = path.split('/').skip(1).collect();
    match parts.as_slice() {
        [field] => is_order_field(field),
        ["nodes", index, field] if is_index(index) => is_node_field(field),
        ["nodes", index, "nodePosition", field] if is_index(index) => is_node_position_field(field),
        ["nodes", index, "nodePosition", "allowedDeviationXY", field] if is_index(index) => {
            matches!(*field, "a" | "b" | "theta")
        }
        ["nodes", node_index, "actions", action_index, field]
            if is_index(node_index) && is_index(action_index) =>
        {
            is_action_field(field)
        }
        [
            "nodes",
            node_index,
            "actions",
            action_index,
            "actionParameters",
            parameter_index,
            field,
        ] if is_index(node_index) && is_index(action_index) && is_index(parameter_index) => {
            is_action_parameter_field(field)
        }
        ["edges", index, field] if is_index(index) => is_edge_field(field),
        ["edges", index, "trajectory", field] if is_index(index) => is_trajectory_field(field),
        ["edges", index, "corridor", field] if is_index(index) => is_corridor_field(field),
        [
            "edges",
            edge_index,
            "trajectory",
            "controlPoints",
            point_index,
            field,
        ] if is_index(edge_index) && is_index(point_index) => is_control_point_field(field),
        ["edges", edge_index, "actions", action_index, field]
            if is_index(edge_index) && is_index(action_index) =>
        {
            is_action_field(field)
        }
        [
            "edges",
            edge_index,
            "actions",
            action_index,
            "actionParameters",
            parameter_index,
            field,
        ] if is_index(edge_index) && is_index(action_index) && is_index(parameter_index) => {
            is_action_parameter_field(field)
        }
        _ => false,
    }
}

fn is_opaque_significant_path(path: &str) -> bool {
    let parts: Vec<&str> = path.split('/').skip(1).collect();
    matches!(
        parts.as_slice(),
        [
            "nodes" | "edges",
            _,
            "actions",
            _,
            "actionParameters",
            _,
            "value"
        ]
    )
}

fn is_index(value: &str) -> bool {
    !value.is_empty() && value.bytes().all(|byte| byte.is_ascii_digit())
}

fn is_order_field(field: &str) -> bool {
    matches!(
        field,
        "headerId"
            | "timestamp"
            | "version"
            | "manufacturer"
            | "serialNumber"
            | "orderId"
            | "orderUpdateId"
            | "nodes"
            | "edges"
    )
}

fn is_node_field(field: &str) -> bool {
    matches!(
        field,
        "nodeId" | "sequenceId" | "released" | "nodeDescriptor" | "nodePosition" | "actions"
    )
}

fn is_node_position_field(field: &str) -> bool {
    matches!(
        field,
        "x" | "y" | "theta" | "mapId" | "allowedDeviationXY" | "allowedDeviationTheta"
    )
}

fn is_edge_field(field: &str) -> bool {
    matches!(
        field,
        "edgeId"
            | "sequenceId"
            | "released"
            | "edgeDescriptor"
            | "maximumSpeed"
            | "maximumMobileRobotHeight"
            | "minimumLoadHandlingDeviceHeight"
            | "orientation"
            | "orientationType"
            | "direction"
            | "reachOrientationBeforeEntering"
            | "maxRotationSpeed"
            | "length"
            | "trajectory"
            | "corridor"
            | "actions"
    )
}

fn is_trajectory_field(field: &str) -> bool {
    matches!(field, "degree" | "knotVector" | "controlPoints")
}

fn is_control_point_field(field: &str) -> bool {
    matches!(field, "x" | "y" | "weight")
}

fn is_corridor_field(field: &str) -> bool {
    matches!(
        field,
        "leftWidth"
            | "rightWidth"
            | "corridorReferencePoint"
            | "releaseRequired"
            | "releaseLossBehavior"
    )
}

fn is_action_field(field: &str) -> bool {
    matches!(
        field,
        "actionType"
            | "actionId"
            | "actionDescriptor"
            | "blockingType"
            | "actionParameters"
            | "retriable"
    )
}

fn is_action_parameter_field(field: &str) -> bool {
    matches!(field, "key" | "value")
}
