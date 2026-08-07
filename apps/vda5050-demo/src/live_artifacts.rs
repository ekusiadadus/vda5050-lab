use std::{
    fs::{self, File, OpenOptions},
    io::{BufWriter, Read, Write},
    path::{Path, PathBuf},
};

use thiserror::Error;
use vda5050_local_fs::open_path_no_follow;

use crate::{LiveEvent, LiveRunPlan};

#[derive(Debug, Error)]
pub enum LiveArtifactError {
    #[error("artifact already exists: {0}")]
    AlreadyExists(String),
    #[error("event hard limit exceeded")]
    EventLimit,
    #[error("live run plan failed its fixed-field validation")]
    InvalidPlan,
    #[error("unsafe artifact path: {0}")]
    UnsafePath(String),
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
}

/// Writes the fixed live plan without replacing any existing artifact.
///
/// # Errors
///
/// Returns an error if the output directory is unsafe, the plan already
/// exists, or serialization or I/O fails.
pub fn write_live_plan(
    output_directory: &Path,
    plan: &LiveRunPlan,
) -> Result<PathBuf, LiveArtifactError> {
    plan.validate()
        .map_err(|_| LiveArtifactError::InvalidPlan)?;
    let metadata = fs::symlink_metadata(output_directory)?;
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return Err(LiveArtifactError::UnsafePath(
            output_directory.display().to_string(),
        ));
    }
    let path = output_directory.join("run-manifest.json");
    let mut file = create_new(&path)?;
    serde_json::to_writer_pretty(&mut file, plan)?;
    file.write_all(b"\n")?;
    file.sync_all()?;
    Ok(path)
}

/// Reads and completely revalidates a live plan selected by the caller.
///
/// # Errors
///
/// Returns an error for symlinks, non-regular files, invalid JSON, or any
/// modified fixed safety field.
pub fn read_live_plan(path: &Path) -> Result<LiveRunPlan, LiveArtifactError> {
    let mut file = open_path_no_follow(path)
        .map_err(|error| LiveArtifactError::UnsafePath(error.to_string()))?;
    let mut bytes = Vec::new();
    file.read_to_end(&mut bytes)?;
    let plan: LiveRunPlan = serde_json::from_slice(&bytes)?;
    plan.validate()
        .map_err(|_| LiveArtifactError::InvalidPlan)?;
    Ok(plan)
}

#[derive(Debug)]
pub struct LiveEventWriter {
    writer: BufWriter<File>,
    count: usize,
    limit: usize,
}

impl LiveEventWriter {
    /// Creates a new append-only JSONL projection with a finite event limit.
    ///
    /// # Errors
    ///
    /// Returns an error if the target already exists or the limit is zero.
    pub fn create_new(path: &Path, limit: usize) -> Result<Self, LiveArtifactError> {
        if limit == 0 {
            return Err(LiveArtifactError::EventLimit);
        }
        Ok(Self {
            writer: BufWriter::new(create_new(path)?),
            count: 0,
            limit,
        })
    }

    /// Appends and flushes one complete explanatory event.
    ///
    /// # Errors
    ///
    /// Returns an error without writing when the hard event limit is reached,
    /// or if serialization or I/O fails.
    pub fn append(&mut self, event: &LiveEvent) -> Result<(), LiveArtifactError> {
        if self.count >= self.limit {
            return Err(LiveArtifactError::EventLimit);
        }
        serde_json::to_writer(&mut self.writer, event)?;
        self.writer.write_all(b"\n")?;
        self.writer.flush()?;
        self.count += 1;
        Ok(())
    }

    #[must_use]
    pub const fn count(&self) -> usize {
        self.count
    }
}

fn create_new(path: &Path) -> Result<File, LiveArtifactError> {
    OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .map_err(|error| {
            if error.kind() == std::io::ErrorKind::AlreadyExists {
                LiveArtifactError::AlreadyExists(path.display().to_string())
            } else {
                LiveArtifactError::Io(error)
            }
        })
}

/// Writes exact bytes to a new artifact without replacement.
///
/// # Errors
///
/// Returns an error if the target exists or the write cannot be completed.
pub fn write_new_artifact(path: &Path, bytes: &[u8]) -> Result<(), LiveArtifactError> {
    let mut file = create_new(path)?;
    file.write_all(bytes)?;
    file.sync_all()?;
    Ok(())
}
