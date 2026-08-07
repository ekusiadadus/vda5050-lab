# Changelog

All notable changes are documented here. This project follows Semantic
Versioning while it is pre-1.0; minor releases may still change unstable report
contracts with an explicit migration note.

## [Unreleased]

## [0.1.2] - 2026-08-07

First downloadable Developer Preview. The immutable `v0.1.1` tag completed
the release contract, all four native builds, checksums, and SLSA provenance,
but stopped before publishing because the generated CycloneDX document omitted
the `serialNumber` required by the pinned attestation action.

### Fixed

- add a deterministic RFC 4122 UUIDv5 `serialNumber`, bound to the canonical
  SBOM body, source commit, tag, and repository, before attestation;
- validate the normalized CycloneDX identity, structure, and serial-number
  contract before upload; and
- cover deterministic normalization, malformed input, unsafe tag, symlink,
  and whitespace-containing path behavior with release-contract tests.

## [0.1.1] - 2026-08-07

Unpublished corrective release attempt. The immutable `v0.1.0` tag did not
produce a GitHub release because the release gate stopped after
`actions/checkout` combined the tag ref with its peeled commit SHA and rewrote
the local annotated-tag ref to a commit ref. The `v0.1.1` gate later stopped at
CycloneDX attestation, so it also has no GitHub release or release assets.

### Fixed

- pass the tag ref explicitly to every checkout so its annotated object is
  preserved and can be validated before publication;
- compare every release build directly with the tag event commit SHA; and
- normalize Windows directory opens so non-regular selected inputs reach the
  same fail-closed `NotRegular` classification as Unix.

## [0.1.0] - 2026-08-06

First Developer Preview of the offline `vda5050-doctor` CLI.

### Added

- bounded canonical JSONL, envelope JSONL, and envelope-array import;
- append-only evidence and capture lifecycle contracts;
- explicit applicability and verdict state machines;
- eight rule IDs across five incident families;
- versioned semantic comparison for repeated VDA 5050 orders;
- safe terminal and deterministic JSON reports;
- content-addressed protocol bundle verification;
- VDA 5050 3.0.0 source and rule-catalog provenance;
- synthetic examples, strict CI, dependency audit, and release automation;
- embedded tool, commit, lockfile, rule-catalog, and bundle identities; and
- automation for multi-platform release archives, checksums, a CycloneDX SBOM,
  and GitHub artifact attestations. The `v0.1.0` release attempt stopped before
  these were published.

### Security

- symlink, device, URI, traversal, duplicate-key, malformed UTF-8, deep JSON,
  oversized input, and terminal-control handling fail closed;
- compiled resource ceilings cannot be disabled by CLI overrides;
- input and retained record data have independent 64 MiB hard ceilings, with
  explicit truncation when the retained-data budget is exhausted;
- missing actor, current-order, reconnect-epoch, timing, or completeness
  evidence cannot produce a role-specific normative failure; and
- VDA 5050 2.1.0 is rejected until its own source bundle and rule profile are
  implemented.

### Known limitations

- only VDA 5050 3.0.0 is accepted;
- `diagnose` records the pinned source identity but does not yet load a local
  protocol bundle at runtime;
- passive envelope imports cannot prove publisher identity or event absence;
- rules D2 and D5 cover documented subsets of their full behavioral clauses;
- no real customer trace, broker, simulator, or physical robot was used; and
- this release is not certification, functional-safety evidence, or proof of
  product usefulness.

[Unreleased]: https://github.com/ekusiadadus/vda5050-lab/compare/v0.1.2...HEAD
[0.1.2]: https://github.com/ekusiadadus/vda5050-lab/releases/tag/v0.1.2
[0.1.1]: https://github.com/ekusiadadus/vda5050-lab/tree/v0.1.1
[0.1.0]: https://github.com/ekusiadadus/vda5050-lab/tree/v0.1.0
