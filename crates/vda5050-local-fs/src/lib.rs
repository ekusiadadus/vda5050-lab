//! Descriptor-bound, no-follow access to local regular files.
//!
//! This crate centralizes the filesystem boundary shared by trace imports and
//! protocol bundles. Untrusted paths below a trusted directory are traversed
//! one component at a time relative to already-open directory descriptors.

use std::{
    fs::{File, Metadata},
    io::{self, Read},
    path::{Component, Path, PathBuf},
};

use thiserror::Error;

#[derive(Debug, Error)]
pub enum LocalPathError {
    #[error("URI references are not accepted: {0}")]
    Uri(String),
    #[error("absolute path references are not accepted: {0}")]
    Absolute(String),
    #[error("empty or current-directory references are not accepted")]
    Empty,
    #[error("parent traversal is not accepted: {0}")]
    ParentTraversal(String),
    #[error("path contains a symlink: {0}")]
    Symlink(String),
    #[error("path is not a regular file: {0}")]
    NotRegular(String),
    #[error("cannot inspect local path {path}: {source}")]
    Io {
        path: String,
        #[source]
        source: io::Error,
    },
}

/// A trusted base directory bound to one already-open descriptor.
///
/// Renaming or replacing the path after construction does not redirect later
/// [`Self::open_file`] calls. Every untrusted component below this descriptor
/// is opened with no-follow semantics.
#[derive(Debug)]
pub struct TrustedDirectory {
    directory: File,
    path: PathBuf,
}

impl TrustedDirectory {
    /// Opens a caller-selected base directory without following its final
    /// component if that component is a symlink.
    ///
    /// # Errors
    ///
    /// Returns an error if the base cannot be opened as a real directory.
    pub fn open(path: &Path) -> Result<Self, LocalPathError> {
        let directory = open_ambient_dir_nofollow(path)?;
        Ok(Self {
            directory,
            path: path.to_path_buf(),
        })
    }

    #[must_use]
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Opens a relative, symlink-free regular file below this directory.
    ///
    /// The returned file's metadata and bytes come from the same descriptor.
    ///
    /// # Errors
    ///
    /// Returns an error for unsafe references, symlinks, non-regular final
    /// objects, missing components, or other I/O failures.
    pub fn open_file(&self, reference: &Path) -> Result<VerifiedLocalFile, LocalPathError> {
        validate_reference(reference)?;
        open_below_directory(&self.directory, &self.path, reference)
    }
}

/// A regular local file whose identity is bound to an already-open descriptor.
///
/// `path` is retained only for diagnostics. All reads and metadata queries use
/// the descriptor, so a later rename or path replacement cannot change the
/// object consumed by the caller.
#[derive(Debug)]
pub struct VerifiedLocalFile {
    file: File,
    path: PathBuf,
}

impl VerifiedLocalFile {
    #[must_use]
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Reads metadata from the verified descriptor, never by reopening
    /// [`Self::path`].
    ///
    /// # Errors
    ///
    /// Returns an I/O error if descriptor metadata cannot be queried.
    pub fn metadata(&self) -> io::Result<Metadata> {
        self.file.metadata()
    }
}

impl Read for VerifiedLocalFile {
    fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        self.file.read(buffer)
    }
}

/// Opens an untrusted local reference below a caller-selected trusted base.
///
/// # Errors
///
/// Returns an error when the base cannot be opened or the reference is not a
/// relative, symlink-free path to a regular file below that base.
pub fn validate_local_path(
    base: &Path,
    reference: &Path,
) -> Result<VerifiedLocalFile, LocalPathError> {
    TrustedDirectory::open(base)?.open_file(reference)
}

/// Opens a caller-selected path while binding final no-follow validation,
/// metadata, and subsequent reads to the same descriptor.
///
/// Parent components are caller-selected and may be followed. Code that needs
/// a containment guarantee must use [`TrustedDirectory::open_file`].
///
/// # Errors
///
/// Returns an error if the final component is a symlink, is not a regular file,
/// or cannot be opened.
pub fn open_path_no_follow(path: &Path) -> Result<VerifiedLocalFile, LocalPathError> {
    use cap_fs_ext::{OpenOptionsFollowExt, OpenOptionsSyncExt};
    use cap_primitives::{
        ambient_authority,
        fs::{FollowSymlinks, OpenOptions, open, open_ambient_dir},
    };

    let Some(file_name) = path.file_name() else {
        return Err(LocalPathError::NotRegular(path.display().to_string()));
    };
    let parent = ambient_parent(path);
    let directory = open_ambient_dir(parent, ambient_authority())
        .map_err(|source| local_io_error(parent, source))?;
    let mut options = OpenOptions::new();
    options.read(true).follow(FollowSymlinks::No).nonblock(true);
    let file = open(&directory, Path::new(file_name), &options)
        .map_err(|source| component_io_error(&directory, Path::new(file_name), path, source))?;
    verified_regular_file(file, path.to_path_buf())
}

/// Returns whether `value` starts with an RFC-style URI scheme.
#[must_use]
pub fn looks_like_uri(value: &str) -> bool {
    let Some(colon) = value.find(':') else {
        return false;
    };
    let scheme = &value[..colon];
    !scheme.is_empty()
        && scheme.bytes().enumerate().all(|(index, byte)| match byte {
            b'a'..=b'z' | b'A'..=b'Z' => true,
            b'0'..=b'9' | b'+' | b'-' | b'.' => index > 0,
            _ => false,
        })
}

fn validate_reference(reference: &Path) -> Result<(), LocalPathError> {
    let display = reference.display().to_string();
    if reference.is_absolute() || looks_like_windows_absolute(&display) {
        return Err(LocalPathError::Absolute(display));
    }
    if looks_like_uri(&display) {
        return Err(LocalPathError::Uri(display));
    }

    let components: Vec<_> = reference.components().collect();
    if components.is_empty()
        || components
            .iter()
            .all(|component| matches!(component, Component::CurDir))
    {
        return Err(LocalPathError::Empty);
    }
    if components
        .iter()
        .any(|component| matches!(component, Component::ParentDir))
    {
        return Err(LocalPathError::ParentTraversal(display));
    }
    if components
        .iter()
        .any(|component| !matches!(component, Component::Normal(_) | Component::CurDir))
    {
        return Err(LocalPathError::Absolute(display));
    }
    Ok(())
}

fn open_below_directory(
    trusted: &File,
    base_path: &Path,
    reference: &Path,
) -> Result<VerifiedLocalFile, LocalPathError> {
    use cap_fs_ext::{OpenOptionsFollowExt, OpenOptionsSyncExt};
    use cap_primitives::fs::{FollowSymlinks, OpenOptions, open, open_dir_nofollow};

    let display = base_path.join(reference);
    let mut directory = trusted
        .try_clone()
        .map_err(|source| local_io_error(base_path, source))?;
    let components: Vec<_> = reference
        .components()
        .filter_map(|component| match component {
            Component::Normal(name) => Some(name),
            Component::CurDir => None,
            _ => unreachable!("reference was validated before descriptor traversal"),
        })
        .collect();

    for (index, component) in components.iter().enumerate() {
        if index + 1 == components.len() {
            let mut options = OpenOptions::new();
            options.read(true).follow(FollowSymlinks::No).nonblock(true);
            let file = open(&directory, Path::new(component), &options)
                .map_err(|source| local_io_error(&display, source))?;
            return verified_regular_file(file, display);
        }
        directory = open_dir_nofollow(&directory, Path::new(component)).map_err(|source| {
            component_io_error(&directory, Path::new(component), &display, source)
        })?;
    }

    Err(LocalPathError::Empty)
}

fn verified_regular_file(
    file: File,
    display_path: PathBuf,
) -> Result<VerifiedLocalFile, LocalPathError> {
    let metadata = file.metadata().map_err(|source| LocalPathError::Io {
        path: display_path.display().to_string(),
        source,
    })?;
    if !metadata.is_file() {
        return Err(LocalPathError::NotRegular(
            display_path.display().to_string(),
        ));
    }
    Ok(VerifiedLocalFile {
        file,
        path: display_path,
    })
}

fn open_ambient_dir_nofollow(path: &Path) -> Result<File, LocalPathError> {
    use cap_primitives::{
        ambient_authority,
        fs::{open_ambient_dir, open_dir_nofollow},
    };

    let Some(name) = path.file_name() else {
        return open_ambient_dir(path, ambient_authority())
            .map_err(|source| local_io_error(path, source));
    };
    let parent = ambient_parent(path);
    let parent_file = open_ambient_dir(parent, ambient_authority())
        .map_err(|source| local_io_error(parent, source))?;
    open_dir_nofollow(&parent_file, Path::new(name))
        .map_err(|source| component_io_error(&parent_file, Path::new(name), path, source))
}

fn ambient_parent(path: &Path) -> &Path {
    path.parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."))
}

fn local_io_error(path: &Path, source: io::Error) -> LocalPathError {
    if is_symlink_open_error(&source) {
        LocalPathError::Symlink(path.display().to_string())
    } else {
        LocalPathError::Io {
            path: path.display().to_string(),
            source,
        }
    }
}

fn component_io_error(
    directory: &File,
    component: &Path,
    display_path: &Path,
    source: io::Error,
) -> LocalPathError {
    use cap_primitives::fs::{FollowSymlinks, stat};

    let is_symlink = is_symlink_open_error(&source)
        || stat(directory, component, FollowSymlinks::No)
            .is_ok_and(|metadata| metadata.is_symlink());
    if is_symlink {
        LocalPathError::Symlink(display_path.display().to_string())
    } else {
        LocalPathError::Io {
            path: display_path.display().to_string(),
            source,
        }
    }
}

#[cfg(windows)]
fn is_symlink_open_error(source: &io::Error) -> bool {
    // ERROR_STOPPED_ON_SYMLINK from CreateFile with no-follow semantics.
    source.raw_os_error() == Some(681)
}

#[cfg(unix)]
fn is_symlink_open_error(source: &io::Error) -> bool {
    source
        .raw_os_error()
        .is_some_and(|raw| rustix::io::Errno::from_raw_os_error(raw) == rustix::io::Errno::LOOP)
}

#[cfg(not(any(unix, windows)))]
const fn is_symlink_open_error(_source: &io::Error) -> bool {
    false
}

fn looks_like_windows_absolute(value: &str) -> bool {
    let bytes = value.as_bytes();
    let drive_absolute = bytes.len() >= 3
        && bytes[0].is_ascii_alphabetic()
        && bytes[1] == b':'
        && matches!(bytes[2], b'/' | b'\\');
    drive_absolute || value.starts_with("\\\\")
}
