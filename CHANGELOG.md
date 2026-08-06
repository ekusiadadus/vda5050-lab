# Changelog

All notable changes are documented here. This project follows Semantic
Versioning while it is pre-1.0; minor releases may still change unstable report
contracts with an explicit migration note.

## [Unreleased]

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
- multi-platform release archives, checksums, CycloneDX SBOM, and GitHub
  artifact attestations.

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

[Unreleased]: https://github.com/ekusiadadus/vda5050-lab/compare/v0.1.0...HEAD
[0.1.0]: https://github.com/ekusiadadus/vda5050-lab/releases/tag/v0.1.0
