# ADR 0005: Deterministic Live Simulator and Read-Only Cockpit

- Status: Accepted
- Date: 2026-08-07
- Scope: one two-robot isolated Tier 1 demonstration
- Extends: ADR 0003 and ADR 0004 without broadening external-DUT authority

## Context

The existing Tier 1 demo produces real MQTT traffic, but its movement is a
small scripted projection rendered after capture. That is sufficient for the
Doctor evidence contract and weak as a local, hands-on explanation of what an
integrator sees when a robot keeps moving after its transport session changes.

The new demonstration must show continuous robot motion, independent MQTT
participants, wire capture, and offline diagnosis without turning Doctor into
a network client or implying physical fidelity.

## Decision

Add a second, explicitly bounded live demonstration with these independent
processes:

```text
Fleet Control actor ──order──▶ internal Mosquitto ──▶ Robot simulator
                                      │
                                      ▼
                               Trace recorder
                                      │
                         canonical trace + evidence
                                      │
                         network-disabled Doctor
                                      │
                         read-only local Web cockpit
```

`vda5050-doctor` remains unchanged as an offline analyzer. The simulator,
Fleet Control actor, recorder, and Web gateway are separate executables. The
Web gateway has no MQTT dependency, no Docker socket, no path parameters, and
no write endpoint. It reads a fixed artifact allowlist and is published only
on `127.0.0.1`.

The MQTT participants share only the internal `tier1` network. The cockpit is
on a different bridge so Docker can publish its localhost port; it cannot join
the MQTT network. Although that bridge is not marked `internal`, the gateway
binary implements no outbound client behavior and receives only a read-only
artifact mount. This is a smaller claim than host-egress isolation.

## Deterministic motion profile

The new `vda5050-sim-core` crate implements fixed-step planar motion. The live
scenario fixes:

- two robots, `demo-001` and `demo-002`;
- released routes `(0, 0) → (6, 0)` and `(0, 2) → (6, 2)` in meters;
- 20 Hz simulation, 10 Hz visualization, and 2 Hz state publication;
- bounded linear/angular speed and acceleration;
- rotate-before-translate differential-drive-like control;
- rectangular robot geometry represented conservatively by a collision circle;
- optional static axis-aligned obstacle collision; and
- 240 simulation ticks and a 60-second job deadline.

This is a deterministic kinematic software simulator. It is not a dynamics,
sensor, localization, wireless, Nav2, Gazebo, traffic-management, or safety
model. The live scenario deliberately contains no obstacle or collision claim.

## Scenario

The default fixed scenario is `reconnect-missing-online`:

1. both robot clients publish retained `ONLINE`;
2. Fleet Control publishes one VDA 5050 3.0.0 order per robot;
3. both robots move continuously on their released edges;
4. at `x >= 2.5 m`, `demo-001` loses its MQTT connection and Mosquitto emits
   its retained `CONNECTION_BROKEN` Last Will;
5. ten simulation ticks later, `demo-001` reconnects with the same fixed
   ClientId in a new synthetic connection epoch;
6. the fault variant omits `ONLINE`; the control variant publishes it;
7. movement and VDA state/visualization publications continue; and
8. the recorder seals the trace before Doctor runs twice.

The active processes use actual MQTT packets on a disposable broker. The
kinematics, identities, clock, connection epochs, and fault are synthetic.

## Artifact and evidence boundary

The run writes new files only into an initially empty, caller-selected
directory:

- `run-manifest.json` — complete pre-CONNECT plan and hard limits;
- `sim-events.jsonl` — simulator truth and scenario phases;
- `wire-events.jsonl` — a live broker-egress UI projection;
- `trace.canonical.jsonl` — closed Doctor input;
- `synthetic-evidence.json` — trace-digest-bound same-job assertions;
- `doctor-passive.json` — passive analysis; and
- `doctor-evidence.json` — analysis with the matching synthetic manifest.

Simulator and UI events always carry `evidence: false`. They explain the run
but are never silently promoted into Doctor evidence. Only the canonical
capture and the explicitly supplied matching manifest affect diagnosis.

For the default fault, the intended comparison is:

| Doctor input | D4 result | Meaning |
| --- | --- | --- |
| trace only | `INCONCLUSIVE / UNRESOLVED` | Subscriber observations cannot prove reconnect actor, epoch, or completeness. |
| trace plus matching manifest | `FAIL / MOBILE_ROBOT` | The isolated same-job harness supplied those assertions for the synthetic actor. |

Neither result proves vendor responsibility, hardware behavior, production
interoperability, or certification.

## Web and rendering boundary

The cockpit renders the latest deterministic simulator snapshot, broker-egress
projection, scenario phases, and both Doctor reports. It uses text-only DOM
insertion and fixed SVG construction; raw MQTT payloads are not inserted as
HTML. File reads are descriptor-bound, symlink-resistant, size-capped, and
limited to fixed routes. Responses set a restrictive content security policy,
disable caching, deny framing, and prevent MIME sniffing.

The UI is an explanatory replay surface. It is not an operations dashboard,
live broker monitor, commissioning control panel, evidence editor, or customer
trace uploader.

## Verification

The repository gates this decision with:

- deterministic sim-core unit contracts;
- VDA state/visualization equality contracts;
- manifest, artifact, topic, and evidence-boundary tests;
- strict Rust Clippy and workspace tests;
- Vitest with 80% statement, branch, function, and line floors for the UI
  projection model;
- a resolved Compose security contract;
- real Mosquitto headless E2E; and
- an optional Playwright browser contract in its own image/profile.

## Consequences

The local demo is materially easier to understand and supplies stronger
technical evidence than fixture replay: independent processes generated and
captured actual MQTT traffic while a continuous simulator ran. It remains a
small product demonstration. External brokers, customer systems, third-party
simulators, software DUTs, physical robots, generic scenarios, and safety or
performance claims still require separate approval.

## References

- [ADR 0003: Isolated Tier 1 Reconnect Demo](0003-isolated-tier1-reconnect-demo.md)
- [ADR 0004: Multi-Mobile-Robot XY Demo Scale Suite](0004-multi-agv-xy-demo.md)
- [Live demo guide](../LIVE_DEMO.md)
- [Future active testing boundary](../FUTURE_ACTIVE_TESTING.md)
