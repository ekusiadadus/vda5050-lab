use std::{fs, io::Read, path::Path};

use tempfile::tempdir;
use vda5050_local_fs::{
    LocalPathError, TrustedDirectory, open_path_no_follow, validate_local_path,
};

#[test]
fn rejects_uri_absolute_parent_empty_and_windows_absolute_references() {
    let dir = tempdir().expect("temporary directory");
    let absolute = dir.path().join("trace.jsonl");
    fs::write(&absolute, b"trace").expect("trace file");

    assert!(matches!(
        validate_local_path(dir.path(), Path::new("https://example.test/trace")),
        Err(LocalPathError::Uri(_))
    ));
    assert!(matches!(
        validate_local_path(dir.path(), Path::new("file:///tmp/trace")),
        Err(LocalPathError::Uri(_))
    ));
    assert!(matches!(
        validate_local_path(dir.path(), &absolute),
        Err(LocalPathError::Absolute(_))
    ));
    assert!(matches!(
        validate_local_path(dir.path(), Path::new("../trace")),
        Err(LocalPathError::ParentTraversal(_))
    ));
    assert!(matches!(
        validate_local_path(dir.path(), Path::new(".")),
        Err(LocalPathError::Empty)
    ));
    assert!(matches!(
        validate_local_path(dir.path(), Path::new("C:\\trace.jsonl")),
        Err(LocalPathError::Absolute(_))
    ));
    assert!(matches!(
        validate_local_path(dir.path(), Path::new("\\\\server\\share\\trace.jsonl")),
        Err(LocalPathError::Absolute(_))
    ));
}

#[test]
fn opens_nested_regular_file_and_uses_the_same_handle_for_metadata_and_reads() {
    let dir = tempdir().expect("temporary directory");
    fs::create_dir(dir.path().join("nested")).expect("nested directory");
    fs::write(dir.path().join("nested/trace.jsonl"), b"original").expect("nested trace file");

    let trusted = TrustedDirectory::open(dir.path()).expect("trusted base");
    let mut opened = trusted
        .open_file(Path::new("nested/trace.jsonl"))
        .expect("verified nested file");

    assert_eq!(opened.path(), dir.path().join("nested/trace.jsonl"));
    assert_eq!(opened.metadata().expect("descriptor metadata").len(), 8);
    let mut bytes = Vec::new();
    opened.read_to_end(&mut bytes).expect("descriptor read");
    assert_eq!(bytes, b"original");
}

#[test]
fn missing_file_preserves_not_found_io_kind() {
    let dir = tempdir().expect("temporary directory");
    let error =
        validate_local_path(dir.path(), Path::new("missing")).expect_err("missing file must fail");

    let LocalPathError::Io { source, .. } = error else {
        panic!("missing object should remain an I/O error")
    };
    assert_eq!(source.kind(), std::io::ErrorKind::NotFound);
}

#[test]
fn selected_path_open_rejects_non_regular_objects() {
    let dir = tempdir().expect("temporary directory");
    assert!(matches!(
        open_path_no_follow(dir.path()),
        Err(LocalPathError::NotRegular(_))
    ));

    #[cfg(unix)]
    assert!(matches!(
        open_path_no_follow(Path::new("/dev/null")),
        Err(LocalPathError::NotRegular(_))
    ));
}

#[cfg(unix)]
#[test]
fn opened_file_is_bound_to_its_descriptor_across_rename_and_replacement() {
    let dir = tempdir().expect("temporary directory");
    let original = dir.path().join("trace.jsonl");
    let moved = dir.path().join("trace-original.jsonl");
    fs::write(&original, b"original").expect("original file");

    let mut opened =
        validate_local_path(dir.path(), Path::new("trace.jsonl")).expect("verified original file");
    fs::rename(&original, &moved).expect("rename original file");
    fs::write(&original, b"replacement").expect("replacement file");

    let mut bytes = Vec::new();
    opened.read_to_end(&mut bytes).expect("read opened file");
    assert_eq!(bytes, b"original");
}

#[cfg(unix)]
#[test]
fn trusted_directory_is_bound_across_root_rename_and_replacement() {
    let outer = tempdir().expect("temporary directory");
    let root = outer.path().join("root");
    let moved = outer.path().join("root-original");
    fs::create_dir(&root).expect("trusted root");
    fs::write(root.join("object"), b"original").expect("original object");
    let trusted = TrustedDirectory::open(&root).expect("opened trusted root");

    fs::rename(&root, &moved).expect("rename trusted root");
    fs::create_dir(&root).expect("replacement root");
    fs::write(root.join("object"), b"replacement").expect("replacement object");

    let mut opened = trusted
        .open_file(Path::new("object"))
        .expect("object below original descriptor");
    let mut bytes = Vec::new();
    opened
        .read_to_end(&mut bytes)
        .expect("read original object");
    assert_eq!(bytes, b"original");
}

#[cfg(unix)]
#[test]
fn rejects_final_nested_and_base_symlinks() {
    use std::os::unix::fs::symlink;

    let dir = tempdir().expect("temporary directory");
    let outside = tempdir().expect("outside directory");
    fs::write(outside.path().join("target"), b"outside").expect("outside target");
    fs::create_dir(dir.path().join("nested")).expect("nested directory");
    fs::write(dir.path().join("nested/target"), b"inside").expect("inside target");

    symlink(outside.path().join("target"), dir.path().join("final-link")).expect("final symlink");
    assert!(matches!(
        validate_local_path(dir.path(), Path::new("final-link")),
        Err(LocalPathError::Symlink(_))
    ));

    symlink(dir.path().join("nested"), dir.path().join("nested-link")).expect("directory symlink");
    assert!(matches!(
        validate_local_path(dir.path(), Path::new("nested-link/target")),
        Err(LocalPathError::Symlink(_))
    ));

    let base_container = tempdir().expect("base-link container");
    let base_link = base_container.path().join("base-link");
    symlink(dir.path(), &base_link).expect("base symlink");
    assert!(matches!(
        validate_local_path(&base_link, Path::new("nested/target")),
        Err(LocalPathError::Symlink(_))
    ));
}

#[cfg(windows)]
#[test]
fn rejects_windows_file_and_directory_reparse_points() {
    use std::os::windows::fs::{symlink_dir, symlink_file};

    let dir = tempdir().expect("temporary directory");
    fs::write(dir.path().join("target"), b"target").expect("target file");
    fs::create_dir(dir.path().join("nested")).expect("nested directory");
    fs::write(dir.path().join("nested/target"), b"nested target").expect("nested target");

    symlink_file(dir.path().join("target"), dir.path().join("final-link"))
        .expect("test runner must allow file symlink creation");
    assert!(matches!(
        validate_local_path(dir.path(), Path::new("final-link")),
        Err(LocalPathError::Symlink(_))
    ));

    symlink_dir(dir.path().join("nested"), dir.path().join("nested-link"))
        .expect("test runner must allow directory symlink creation");
    assert!(matches!(
        validate_local_path(dir.path(), Path::new("nested-link/target")),
        Err(LocalPathError::Symlink(_))
    ));
}
