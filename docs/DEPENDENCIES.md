# Dependency and supply-chain inventory

Snapshot date: 2026-08-06.

`Cargo.lock` is committed for reproducible application builds. CI uses
`--locked`, Rust 1.97.1, minimal GitHub permissions, and a full-SHA pin for
`actions/checkout`. A release must not be cut when the lockfile differs from
the reviewed dependency graph.

## Direct third-party crates

| Crate | Cargo requirement | Resolved in `Cargo.lock` | Purpose | Runtime |
| --- | --- | --- | --- | --- |
| `cap-fs-ext` | `4.0` | `4.0.2` | no-follow descriptor options | yes |
| `cap-primitives` | `4.0` | `4.0.2` | capability-based local file open | yes |
| `clap` | `4.5` | `4.6.5` | CLI argument parsing | yes |
| `hex` | `0.4` | `0.4.3` | SHA-256 encoding | yes |
| `serde` | `1.0` | `1.0.229` | typed wire contracts | yes |
| `serde_json` | `1.0` | `1.0.151` | bounded JSON parsing and output | yes |
| `sha2` | `0.10` | `0.10.9` | evidence and artifact digests | yes |
| `rustix` | `1.1` | `1.1.4` | Unix no-follow error classification | yes |
| `thiserror` | `2.0` | `2.0.19` | typed error definitions | yes |
| `tempfile` | `3.23` | `3.27.0` | importer tests only | no |

The locked normal dependency graph contains no HTTP, MQTT, socket, telemetry,
database, XML, or archive client. The current executable has no network path.
That statement must be rechecked whenever `Cargo.lock` changes.

## Verification

Run the local quality and advisory gates:

```sh
make ci
cargo install cargo-audit --version 0.22.2 --locked
cargo install cargo-deny --version 0.20.2 --locked
make audit
make deny
```

The 2026-08-06 audit used `cargo-audit 0.22.2`, loaded 1,190 RustSec
advisories, scanned 89 locked crate packages, and reported no known
vulnerability. This is a point-in-time result, not a guarantee about future
advisories.

The release policy also evaluates advisories, bans, licenses, and sources for
all four distributed targets with `cargo-deny 0.20.2`. Only Apache-2.0,
Apache-2.0 WITH LLVM-exception, MIT, and Unicode-3.0 were required by the
locked graph; unknown registries, Git dependencies, and wildcard requirements
are denied.

## Change policy

Every dependency change must include:

1. a reviewed `Cargo.toml` and `Cargo.lock` diff;
2. `cargo tree --workspace --edges normal --locked` review;
3. formatting, strict Clippy, and locked workspace tests;
4. a fresh RustSec audit;
5. confirmation that the offline executable gained no network-capable path;
6. license review for every new direct dependency; and
7. an update to this inventory when resolved direct versions change.

Tagged releases enforce line and region coverage floors with pinned
`cargo-llvm-cov 0.8.7`, then generate a binary-scoped CycloneDX 1.5 SBOM with
pinned `cargo-cyclonedx 0.5.9`. GitHub Actions creates Sigstore-backed SLSA build
provenance and SBOM attestations with the official `actions/attest` action
pinned to a full commit SHA. Users must still verify the downloaded archive,
checksum, and attestation rather than trusting the release page alone.
