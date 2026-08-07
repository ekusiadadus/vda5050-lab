# Tier 1 Multi-Robot Reconnect Demo

## Scope

This demo shows one evidence-bound VDA 5050 diagnosis at 1 through 100 virtual
mobile robots:

> A broker emitted `CONNECTION_BROKEN`; a new virtual mobile-robot session then
> emitted `state`, but did not publish the expected retained `ONLINE` message.

The published scale suite is `1, 2, 5, 10, 50, 100`. Every robot has a unique
VDA identity, topic prefix, MQTT ClientId, order ID, route, participant, and
connection epoch. `demo-001` is the sole reconnect-fault target; all remaining
robots are non-fault background controls.

It is not a conformance suite, fleet-capacity benchmark, or warehouse
simulator. It does not connect to a customer broker, external software DUT, or
physical robot. The generated actors, coordinates, trace, and evidence are
synthetic and belong to one isolated job.

The two executables have different authority:

- `vda5050-demo` is the Tier 1 MQTT producer and recorder. The recommended
  runner connects it only to the exact service `broker` on a Docker internal
  network with no host-published broker port.
- `vda5050-doctor` remains offline. It opens explicitly selected local files;
  it never connects to the broker or controls the demo.

## Safety boundary

Use a new disposable broker for each fault or control run. The demo publishes
retained connection messages. Reusing a broker can deliver state retained by a
previous run and invalidate the comparison.

The standalone binary defaults to `127.0.0.1:18884`. Its target validator
rejects unspecified addresses, routable IP addresses, and arbitrary hostnames.
The special combination `--broker-host broker --isolated-network` is an
operator assertion when used directly. The provided `run-tier1-demo.sh` adds
the missing enforcement: before CONNECT it inspects the fully resolved Compose
configuration, then verifies the live Docker network is internal and that the
broker has no host port binding. Doctor runs with `network_mode: none`.

Do not use this demo with:

- a shared, customer, staging, production, or otherwise persistent broker;
- a broker to which a real fleet-control or robot client can connect;
- a physical robot, external DUT, or site network; or
- an output directory containing data that must be preserved.

The maximum value, 100, means 100 virtual mobile-robot MQTT actors in the
isolated simulation. It does not mean 100 physical robots, 100 vendors, 100
tested hardware connections, or a production fleet-size limit.

MQTT CONNECT itself is a side effect. The run uses one run-derived ClientId per
virtual robot plus separate fleet-control and recorder ClientIds, and writes
the resolved identities before connecting. It is still the operator's
responsibility to ensure the disposable broker has no unrelated clients.

## Prerequisites

- Docker with Docker Compose;
- `jq`; and
- the repository checkout.

The Docker build uses digest-pinned Rust 1.97.1, Debian Bookworm, and Eclipse
Mosquitto 2.1.2 images. Mosquitto permits anonymous access only inside the
ephemeral internal network.

## 1. Run one fleet size

From the repository root:

```sh
artifact_dir="$(mktemp -d)"
VDA5050_DEMO_RUN_ID=fault-0001 \
VDA5050_DEMO_ROBOT_COUNT=10 \
VDA5050_DEMO_ARTIFACT_DIR="$artifact_dir" \
  bash scripts/run-tier1-demo.sh
```

`VDA5050_DEMO_ROBOT_COUNT` accepts an integer from 1 through 100. The direct
binary exposes the same boundary as `--robot-count N`. Omitting the value keeps
the one-robot compatibility case.

The run deliberately crashes the `demo-001` virtual robot session so that the
broker publishes its retained Last Will with `CONNECTION_BROKEN`. It reconnects
the same virtual participant in a new session, omits `ONLINE`, and publishes a
new `state` observation. Other robots remain connected as non-fault background
controls; they do not exercise the reconnect precondition.
The runner verifies the `demo-001` fault configuration, per-robot trace
identities, and a D4 FAIL with the matching same-job evidence. Doctor's public
investigation target is the role `MOBILE_ROBOT`, not a vendor or production
serial attribution. The runner stores `doctor-report.json` and removes the
containers and network even on failure.

The output directory contains:

| Artifact | Meaning |
| --- | --- |
| `run-manifest.json` | Robot count, coordinate system, routes, fault target, resolved target and identities, allowlists, and hard limits. |
| `trace.canonical.jsonl` | Broker-egress observations with capture-local sequence and monotonic time. |
| `synthetic-evidence.json` | Trace-digest-bound same-job assertions for actor role, participant identity, connection epoch, capture closure, and rule completeness. |
| `doctor-report.json` | Offline Doctor result generated in a network-disabled container. |

The run manifest records `robot_count`, all robot identities, routes, coordinate
system, sole fault target, and count-derived hard limits. The message ceiling is
`10 * robot_count + 16`; the current duration ceiling is 60 seconds. The actor
and MQTT-connection ceilings are each `robot_count + 2`: one per virtual robot,
one fleet-control actor, and one recorder. Budget exhaustion invalidates the run
instead of silently omitting data. Only robot-specific connection topics may be
retained. The run manifest explicitly records that CONNECT is a side effect and
that no physical DUT is authorized.

## 2. Run the fixed scale suite

Run all published counts into separate directories:

```sh
suite_dir="$(mktemp -d)"
VDA5050_DEMO_SUITE_ARTIFACT_DIR="$suite_dir" \
  bash scripts/run-tier1-fleet-suite.sh all
```

The suite writes each run below `robots-1/`, `robots-2/`, `robots-5/`,
`robots-10/`, `robots-50/`, or `robots-100/`. A single fixed scale can be run
through the same entrypoint. Each count receives a fresh internal broker and
network, preventing retained connection state from crossing scale boundaries:

```sh
bash scripts/run-tier1-fleet-suite.sh 10
```

Equivalent Make targets are:

```sh
make demo-fleet DEMO_ROBOTS=10
make demo-fleet-e2e
```

The first command runs one selected scale. The second exercises the full fixed
suite. A suite success is a bounded functional result on that host and build;
it is not a throughput, latency, CPU-sizing, or real-time result.

## 3. XY coordinate model

VDA 5050 positions are project-specific map coordinates. In this demo, X and Y
are meters on `mapId = "warehouse-demo"`; they are not pixels, cells,
robot-local odometry, or measurements from a real facility. The drawing uses a
synthetic lower-left origin.

For zero-based robot index `i`:

```text
column       = floor(i / 10)
row          = i mod 10
start        = (6 * column,     2 * row)
released_end = (6 * column + 4, 2 * row)
horizon_end  = (6 * column + 4, 2 * row + 1)
```

The first and last identities at the supported ceiling are therefore:

| Robot | Start (m) | Released end (m) | Horizon end (m) |
| --- | --- | --- | --- |
| `demo-001` | (0, 0) | (4, 0) | (4, 1) |
| `demo-100` | (54, 18) | (58, 18) | (58, 19) |

The spacing is chosen to make routes visually distinct. It does not establish
robot footprint clearance, allowed deviation, braking distance, collision
avoidance, or site safety. See
[ADR 0004](adr/0004-multi-agv-xy-demo.md) for every fixed scale and the VDA
profile rationale.

## 4. Diagnose the same trace without active assertions

First, give Doctor only the passive trace:

```sh
cargo run --locked --bin vda5050-doctor -- diagnose \
  "$artifact_dir/trace.canonical.jsonl" \
  --vda-version 3.0.0 \
  --input-format canonical-jsonl \
  --format terminal
```

Expected D4 result:

```text
Report: LAB-D4-RECONNECT-STATE
Verdict: INCONCLUSIVE
```

The exact rendering may contain additional evidence detail. The important
boundary is the verdict: observing `CONNECTION_BROKEN` and a later `state`
message does not by itself prove the publisher's identity, a new connection
epoch, or a rule-complete capture. Doctor must not turn an unobserved `ONLINE`
message into a passive FAIL. The underlying finding keeps the next
investigation target unresolved.

## 5. Add the matching same-job synthetic evidence

Now explicitly select the evidence manifest written by the same demo run:

```sh
cargo run --locked --bin vda5050-doctor -- diagnose \
  "$artifact_dir/trace.canonical.jsonl" \
  --vda-version 3.0.0 \
  --input-format canonical-jsonl \
  --synthetic-evidence-manifest \
    "$artifact_dir/synthetic-evidence.json" \
  --format terminal
```

Expected D4 result:

```text
evidence=TIER1_SYNTHETIC_SAME_JOB (not production proof)
Report: LAB-D4-RECONNECT-STATE
Verdict: FAIL
```

Doctor validates the evidence schema, its synthetic and same-job flags, the
source trace SHA-256, capture closure, record references, and complete event
attribution before applying the assertions. A manifest copied from another run
or attached to a modified trace fails closed.

This result means only that the self-contained synthetic scenario proved the
preconditions of this rule, so the underlying finding points to the synthetic
mobile-robot actor. It is not evidence that a production mobile robot, vendor
implementation, or real deployment has the same fault.

## 6. Run the control

The automated verdict matrix creates a fresh broker project for the control so
no retained connection state survives:

```sh
bash tests/tier1_demo_verdict_matrix.sh
```

The test proves all three intended outcomes: matching synthetic evidence yields
D4 FAIL/MOBILE_ROBOT, the same trace without the manifest yields
INCONCLUSIVE/UNRESOLVED, and the fresh-broker control has no D4 finding. The
control's observed connection-state sequence is also fixed to
`ONLINE,CONNECTION_BROKEN,ONLINE`.

Expected comparison: there is no `LAB-D4-RECONNECT-STATE` finding in the control
because the new synthetic session published `ONLINE`. If no other supported
incident rule fires, terminal output says:

```text
No supported incident finding was produced from the supplied observations.
```

No finding is not a PASS verdict. It does not certify the actor, protocol,
library, or VDA 5050 implementation, and it says nothing about scenarios the
trace and current rule catalog did not exercise.

## Interpretation summary

| Comparison | D4 outcome | Permitted conclusion |
| --- | --- | --- |
| Fault trace only | INCONCLUSIVE / UNRESOLVED | More evidence is required before attributing the missing publication. |
| Fault trace plus matching same-job manifest | FAIL / MOBILE_ROBOT | The synthetic virtual actor omitted `ONLINE` after a proved synthetic reconnect. Not production proof. |
| Control trace plus matching same-job manifest | No D4 finding | The synthetic control contains the expected `ONLINE`; this is not a general PASS. |

The difference between the first two rows is evidence quality, not different
MQTT observations. That distinction is the point of the demo.

## Out of scope

This demo does not establish broker identity outside the inspected Compose job,
isolation outside its internal Docker network, production capture completeness,
external participant identity, hardware safety, fault-bridge transparency,
cross-vendor interoperability, or production performance. The remaining design
and approval gates are in
[FUTURE_ACTIVE_TESTING.md](FUTURE_ACTIVE_TESTING.md).
