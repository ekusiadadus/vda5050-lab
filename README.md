# vda5050-lab

[![CI](https://github.com/ekusiadadus/vda5050-lab/actions/workflows/ci.yml/badge.svg)](https://github.com/ekusiadadus/vda5050-lab/actions/workflows/ci.yml)
[![Release](https://img.shields.io/github/v/release/ekusiadadus/vda5050-lab?include_prereleases)](https://github.com/ekusiadadus/vda5050-lab/releases)
[![License](https://img.shields.io/badge/license-Apache--2.0-blue.svg)](LICENSE)

vda5050-lab is the home of **vda5050-doctor**, an offline incident-diagnosis
CLI for VDA 5050 traces.

Give it MQTT observations from a stalled or misbehaving integration. It should
explain, in less than ten minutes of user time:

1. what was observed;
2. which messages support that explanation;
3. which VDA 5050 version and clause are relevant;
4. what cannot be concluded from the capture;
5. which logs or observations to collect next; and
6. whether the next investigation should start with the mobile robot, fleet
   control, transport, or remain unresolved.

The product does not assign blame when the evidence cannot prove the responsible
protocol role.

## Status

`v0.1.1` Developer Preview. This is an offline implementation preview, not a
public-alpha or certification claim.

The Rust workspace now contains the bounded importer, evidence model,
versioned order comparator, eight initial rule IDs across five incident
families, terminal/JSON reporting, synthetic fixtures, and pinned CI. Local
formatting, strict Clippy, locked tests, coverage, and RustSec audit pass.

Product usefulness is still unproven: no real customer trace, design-partner
pilot, MQTT connection, or hardware execution has been performed. The CLI
deliberately accepts only VDA 5050 3.0.0 until a separate 2.1 rule profile and
source bundle exist.

## Why this project

VDA 5050 failures are commonly behavioral rather than syntactic. A message may
match its JSON Schema while the integration still mishandles an order update,
base/horizon transition, reconnect, cancel, or action lifecycle.

Existing products already make MQTT traffic easier to observe:

- [arculus describes](https://www.arculus.de/a-look-into-vda5050) the manual
  log-analysis workflow and its own visualizer and compliance-test suite.
- [Fletr](https://fletr.io/documentation) provides VDA-oriented MQTT
  monitoring, log import, timelines, and multi-version support.

This project is therefore not differentiated by being another viewer. Its
initial product boundary is evidence-bounded diagnosis:

> Fletr and similar tools help engineers inspect traffic. vda5050-doctor
> explains what the available traffic does and does not support, and recommends
> the next investigation step.

Reported implementation failures also show why a single-message validator is
insufficient:

- repeated newBaseRequest handling
  ([coatyio/vda-5050-lib.js#44](https://github.com/coatyio/vda-5050-lib.js/issues/44));
- connection state after reconnect
  ([#38](https://github.com/coatyio/vda-5050-lib.js/issues/38)); and
- cancel/action callback transitions
  ([#33](https://github.com/coatyio/vda-5050-lib.js/issues/33)).

Specification ambiguity remains visible rather than being converted into a
made-up universal answer. For example, the meaning of omitted theta has
produced an interoperability discussion between implementations
([VDA5050#613](https://github.com/VDA5050/VDA5050/issues/613)).

## First user

The first user is a system integrator or robot protocol engineer commissioning
a mixed-vendor VDA 5050 system. The first job is not certification. It is:

> Explain why this order stopped, and tell me what evidence to obtain next.

Operations dashboards, generic MQTT exploration, active conformance testing,
and physical robot testing are later or separate products.

## Initial product

The first executable is:

~~~text
vda5050-doctor diagnose trace.jsonl --vda-version 3.0.0
~~~

The local prototype accepts explicitly selected local trace formats only and
emits:

- a bounded terminal report;
- canonical JSON;
- incident hypotheses with confidence and evidence IDs;
- applicable specification sources;
- missing-evidence requirements; and
- a next-investigation target:
  MOBILE_ROBOT, FLEET_CONTROL, TRANSPORT, or UNRESOLVED.

The initial vertical slice covers five incident families:

1. repeated order IDs/update IDs with identical or changed semantic content;
2. base, horizon, and stitching inconsistencies;
3. newBaseRequest that remains unhandled;
4. connection state that remains inconsistent after reconnect; and
5. cancel and action lifecycle inconsistencies.

The future public alpha may expand to ten to fifteen evidence-backed diagnoses
and explicit VDA 5050 2.1-to-3.0 migration diagnostics only after the private
pilot gate passes.

## Install, verify, and run

The Developer Preview publishes archives for Linux x86-64/Arm64, macOS Arm64,
and Windows x86-64. Download an archive and `SHA256SUMS` from the
[GitHub release](https://github.com/ekusiadadus/vda5050-lab/releases/tag/v0.1.1),
then verify it:

```sh
sha256sum --check SHA256SUMS
gh attestation verify vda5050-doctor-v0.1.1-<target>.tar.gz \
  --repo ekusiadadus/vda5050-lab
```

Each archive contains the binary, license, changelog, rule catalog, source
manifests, and provenance documentation. The release also contains a CycloneDX
1.5 SBOM.

To build from source:

Prerequisites: Rust 1.97.1 through `rustup` and a local trace that you are
authorized to inspect.

```sh
make ci
make diagnose-example
```

Run against newline-delimited message envelopes:

```sh
cargo run --locked --bin vda5050-doctor -- \
  diagnose trace.jsonl \
  --vda-version 3.0.0 \
  --input-format envelope-jsonl \
  --format terminal
```

Supported input-format values are `canonical-jsonl`, `envelope-jsonl`, and
`envelope-array`. Output is `terminal` or deterministic `json`. Format
selection is explicit; the importer does not guess. Final-component symlinks,
symlink traversal below trusted bases, devices, URI inputs, duplicate JSON
keys, unsafe nesting, oversized records, and configured limits above compiled
safety ceilings fail closed or remain accounted for as rejected records. The
Developer Preview caps both input files and conservatively accounted retained
record data at 64 MiB; budget exhaustion is explicit in the report and
truncates further record retention.

`make ci` requires separately installed, pinned audit tools:

```sh
cargo install cargo-audit --version 0.22.2 --locked
cargo install cargo-deny --version 0.20.2 --locked
```

Release maintainers also install the Rust `llvm-tools-preview` component,
pinned `cargo-llvm-cov 0.8.7`, and `cargo-cyclonedx 0.5.9`, then run
`make release-check`. The release gate fails below 80% line coverage or 80%
region coverage.

```sh
rustup component add llvm-tools-preview --toolchain 1.97.1
cargo install cargo-llvm-cov --version 0.8.7 --locked
cargo install cargo-cyclonedx --version 0.5.9 --locked
make release-check
```

Synthetic examples and expected proof boundaries are documented in
[fixtures/synthetic/README.md](fixtures/synthetic/README.md).

## Result model

Applicability and verdict are separate internal dimensions:

~~~text
applicability = APPLICABLE | NOT_APPLICABLE | UNKNOWN
verdict       = PASS | FAIL | INCONCLUSIVE | UNEVALUATED
~~~

The canonical report retains both values. The human summary maps them to PASS,
FAIL, INCONCLUSIVE, or NOT_APPLICABLE without hiding unknown preconditions.

An incident explanation also distinguishes:

- **observation** — captured bytes, topic, sequence, and capture-local time;
- **assertion** — facts provided by a capture adapter or manifest;
- **inference** — a derived interpretation with its evidence basis; and
- **recommendation** — the next useful investigation action, not a normative
  verdict.

Missing messages, an unknown starting state, incomplete topics, timestamp
ambiguity, or an unsupported capture vantage can force INCONCLUSIVE.

## Product boundaries

The offline prototype does not:

- connect to an MQTT broker;
- publish a VDA 5050 message;
- control or simulate a robot;
- inject delay, loss, duplication, or reordering;
- diagnose LiDAR, motor, battery, localization, or other physical root causes;
- perform collision avoidance, routing, or traffic management;
- provide official VDA, cybersecurity, or functional-safety certification;
- claim an incomplete passive trace proves that an event never occurred;
- automatically translate a 2.1 deployment into a 3.0 deployment;
- provide a Web UI, JUnit, SARIF, or pseudonymized export; or
- upload customer traces or enable telemetry.

The report references the pinned VDA PDF digest, but the current `diagnose`
command does not yet load and verify a local protocol bundle. Source provenance
and the runtime-compatible manifest are present in
[docs/PROTOCOL_SOURCES.md](docs/PROTOCOL_SOURCES.md); runtime bundle integration
remains a pre-alpha blocker.
Active MQTT and physical-DUT work is deliberately outside the initial
implementation plan. Its retained safety design lives in
[docs/FUTURE_ACTIVE_TESTING.md](docs/FUTURE_ACTIVE_TESTING.md) and remains
unapproved.

## Validation before scale

The project will not call itself useful because its fixtures pass. Before a
public alpha decision it will evaluate at least twenty real incident traces from
at least three organizations, with an organization- or system-level holdout.

The proposed pilot gate requires:

- at least 70% actionable next-step coverage;
- zero observed unsupported participant attribution;
- median time to an accepted next action at most half of the manual baseline;
- correct abstention when evidence is insufficient; and
- at least three teams saying they would use it on another integration.

The complete study contract is in
[docs/VALIDATION_PROTOCOL.md](docs/VALIDATION_PROTOCOL.md).

## Implemented architecture

The first implementation is deliberately small:

- Rust for the offline importer, normalized trace model, diagnostic engine,
  protocol-source handling, reports, and CLI;
- local JSON/JSONL input;
- terminal and canonical JSON output;
- versioned rule contracts and minimized synthetic fixtures; and
- no network-capable dependency in the offline execution path.

See [docs/IMPLEMENTATION_PLAN.md](docs/IMPLEMENTATION_PLAN.md) for the
test-first phases and release gates, [rules/phase1.json](rules/phase1.json) for
the machine-readable rule contract, and
[docs/DEPENDENCIES.md](docs/DEPENDENCIES.md) for the reviewed supply-chain
snapshot.

## Repository contents

~~~text
.
├── .github/workflows/{ci,release}.yml
├── Cargo.lock
├── Cargo.toml
├── CHANGELOG.md
├── CONTRIBUTING.md
├── LICENSE
├── Makefile
├── README.md
├── SECURITY.md
├── apps/vda5050-doctor/
├── bundles/manifests/
├── crates/
│   ├── vda5050-core/
│   ├── vda5050-doctor/
│   ├── vda5050-import/
│   ├── vda5050-local-fs/
│   ├── vda5050-protocol/
│   └── vda5050-report/
├── fixtures/synthetic/
├── rules/phase1.json
└── docs/
    ├── DEPENDENCIES.md
    ├── FUTURE_ACTIVE_TESTING.md
    ├── IMPLEMENTATION_PLAN.md
    ├── PROTOCOL_SOURCES.md
    ├── RELEASING.md
    └── VALIDATION_PROTOCOL.md
~~~

## Primary references

- [VDA 5050 official repository](https://github.com/VDA5050/VDA5050)
- [VDA 5050 version 3.0.0 release](https://github.com/VDA5050/VDA5050/releases/tag/3.0.0)
- [VDA 5050 official documents](https://www.vda.de/en/topics/automotive-industry/vda-5050.)
- [MQTT 3.1.1 specification](https://docs.oasis-open.org/mqtt/mqtt/v3.1.1/mqtt-v3.1.1.html)

## License

Copyright 2026 Daisuke Kuriyama.

Project-authored code and documentation are licensed under Apache License 2.0.
Referenced VDA publications and schemas retain their own copyright and
redistribution terms. Their provenance and license must be reviewed before any
upstream artifact is committed.
