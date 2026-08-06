//! Bounded, offline VDA 5050 trace import.
//!
//! This crate deliberately accepts only caller-selected local files. It has no
//! network resolver, performs no format auto-detection, and accounts for every
//! record it examines.

mod model;
mod reader;
mod strict_json;

pub use model::{
    ImportConfig, ImportFormat, ImportRecord, ImportReport, ImportSummary, ImportedMessage,
    RecordKind, RecordLocation, RejectReason,
};
pub use reader::ImportError;
pub use reader::import_path;
pub use vda5050_local_fs::{LocalPathError, VerifiedLocalFile, validate_local_path};
