use std::{env, fs, path::PathBuf};

use sha2::{Digest, Sha256};

fn main() {
    let manifest_dir = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").expect("manifest dir"));
    let workspace = manifest_dir.join("../..");
    emit_digest("VDA5050_LOCK_SHA256", &workspace.join("Cargo.lock"));
    emit_digest(
        "VDA5050_RULE_CATALOG_SHA256",
        &workspace.join("rules/phase1.json"),
    );

    let commit = env::var("VDA5050_BUILD_COMMIT").unwrap_or_else(|_| "UNVERIFIED".to_owned());
    println!("cargo:rustc-env=VDA5050_BUILD_COMMIT={commit}");
    println!("cargo:rerun-if-env-changed=VDA5050_BUILD_COMMIT");
}

fn emit_digest(name: &str, path: &PathBuf) {
    let bytes = fs::read(path).unwrap_or_else(|error| {
        panic!(
            "failed to read release provenance input {}: {error}",
            path.display()
        )
    });
    println!("cargo:rustc-env={name}={:x}", Sha256::digest(bytes));
    println!("cargo:rerun-if-changed={}", path.display());
}
