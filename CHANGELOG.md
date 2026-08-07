# Changelog

All notable changes are documented here. This project follows Semantic
Versioning while it is pre-1.0; minor releases may still change unstable report
contracts with an explicit migration note.

## [Unreleased]

## [0.2.0] - 2026-08-07

First synthetic end-to-end demonstration of the evidence boundary behind
`vda5050-doctor`. This release adds an isolated Tier 1 MQTT scenario while
keeping diagnosis offline and making the difference between passive
INCONCLUSIVE evidence and same-job synthetic FAIL evidence visible.

### Added

- a bounded virtual fleet-control, mobile-robot, and recorder harness for the
  reconnect-without-ONLINE incident;
- a trace-digest-bound synthetic evidence manifest with explicit actor,
  participant, connection-epoch, capture-closure, and completeness assertions;
- a disposable, internal-only Docker Compose topology for the broker and demo,
  with a network-disabled Doctor container;
- fault and control runs that demonstrate the absence-sensitive D4 rule without
  making a production, hardware, or interoperability claim; and
- deterministic MP4 and GIF views for 1, 2, 5, 10, 50, and 100 robots, plus a
  six-scale overview, all rendered from broker-observed `x`/`y` values; every
  view keeps robot IDs, fleet density, the `demo-001` fault target, and
  fleet-state counts visible.

### Changed

- reconnect diagnosis recognizes VDA 5050 3.0.0 `CONNECTION_BROKEN` evidence;
- cancel diagnosis accepts terminal states from `instantActionStates` as well
  as legacy `actionStates` observations; and
- semantic order comparison covers additional VDA 5050 3.0.0 nested fields and
  keeps unknown extensions outside normative equality claims.

### Security

- release assembly now downloads named build artifacts individually, rejects
  every non-allowlisted entry, and publishes only twenty explicitly named
  assets;
- all fourteen fleet media files must be regular non-symlinks with fixed names;
  MP4 files require an ISO BMFF signature and a 100 MiB ceiling, while GIF files
  require a GIF87a/GIF89a signature and a 50 MiB ceiling; all are included in
  `SHA256SUMS` and SLSA provenance; and
- the binary-scoped CycloneDX attestation remains limited to the four Doctor
  archives and is not misapplied to the demo video.

### Known limitations

- the demo actors and evidence are synthetic and generated in one isolated job;
- MQTT CONNECT and retained connection publications are side effects, so the
  harness must not be pointed at a shared, customer, or production broker;
- the videos and GIFs are explanatory media, not protocol evidence,
  certification, or a byte-reproducibility claim for encoding; and
- the release still has no real customer-trace pilot, external DUT, or physical
  robot validation.

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

[Unreleased]: https://github.com/ekusiadadus/vda5050-lab/compare/v0.2.0...HEAD
[0.2.0]: https://github.com/ekusiadadus/vda5050-lab/releases/tag/v0.2.0
[0.1.2]: https://github.com/ekusiadadus/vda5050-lab/releases/tag/v0.1.2
[0.1.1]: https://github.com/ekusiadadus/vda5050-lab/tree/v0.1.1
[0.1.0]: https://github.com/ekusiadadus/vda5050-lab/tree/v0.1.0
