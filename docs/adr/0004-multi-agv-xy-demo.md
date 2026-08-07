# ADR 0004: Multi-Mobile-Robot XY Demo Scale Suite

- Status: Accepted
- Date: 2026-08-07
- Scope: 1, 2, 5, 10, 50, or 100 synthetic mobile robots on the isolated Tier 1 runner
- Supersedes: ADR 0003 only where this ADR explicitly extends the single-robot demo

## Context

ADR 0003 established one isolated virtual mobile robot as an evidence-bound
demonstration of the reconnect rule. That slice proves an important distinction
between passive observations and same-job assertions, but it does not show that
the trace, identity, coordinate, and evidence models remain separated when many
robots publish concurrently.

The extension must remain a protocol demonstration, not a warehouse or
hardware claim. VDA 5050 defines communication between fleet control and mobile
robots. It does not make this harness a physics simulator, collision checker,
traffic optimizer, fleet-capacity benchmark, or interoperability certificate.

## Decision

Add `--robot-count N` to the isolated demo, where `N` is in the closed range
1 through 100. The published scale suite is fixed to:

```text
1, 2, 5, 10, 50, 100
```

Arbitrary values in the valid range are useful for local diagnosis, but only
the six fixed counts form the reproducible suite. A result at one count does
not imply a result at a larger count.

Each virtual mobile robot has its own:

- `serialNumber` (`demo-001` through `demo-100`);
- robot-scoped topic prefix and mandatory terminal topic names;
- MQTT mobile-robot ClientId;
- participant identity and connection epoch;
- order ID; and
- route and observed position stream.

The fleet-control and recorder actors remain separate from every mobile-robot
actor. A message for one robot must never be attributed to another robot by
topic parsing alone; the same-job evidence mapping supplies the actor and
participant assertion.

`demo-001` is the sole reconnect-fault target at every scale. It emits a new
synthetic session after `CONNECTION_BROKEN` but omits the expected retained
`ONLINE`. All remaining robots are non-fault background controls and do not
exercise the reconnect precondition. This keeps the expected incident count
constant while the background population increases and makes cross-robot
attribution errors visible.

## VDA 5050 profile used by the demo

The demo targets VDA 5050 3.0.0. Section 4.2 describes the local-broker topic
layout as suggested rather than universally mandatory, while the terminal
topic names are mandatory. The demo therefore declares an explicit lab topic
profile:

```text
vda5050/v3/lab-demo/demo-NNN/{connection,order,state,visualization}
```

This layout is a demo profile, not a claim that every cloud or vendor system
must use it. The `manufacturer` and `serialNumber` values in message headers
must correspond to the selected virtual robot identity.

Connection publications use the connection-topic requirements of the existing
Tier 1 scenario, including retained `ONLINE` and the broker-published retained
`CONNECTION_BROKEN` Last Will. Other demo topics retain their existing QoS and
retain contract. Scaling the count does not relax those per-robot rules.

## Coordinate contract

VDA 5050 3.0.0 section 6.3 defines map coordinates in a project-specific,
right-handed coordinate system, and specifies X, Y, and Z in meters. The demo
uses:

- `mapId = "warehouse-demo"`;
- X and Y values in meters;
- a synthetic lower-left origin for presentation; and
- no claim that the coordinates were measured from a physical site.

“Lower-left” is a demo drawing convention, not an additional VDA 5050
requirement. The coordinate values remain the project-specific world
coordinates carried by VDA messages, not pixels, grid cells, odometry samples,
or robot-local coordinates.

For zero-based robot index `i`:

```text
column       = floor(i / 10)
row          = i mod 10
start        = (6 * column,     2 * row)
released_end = (6 * column + 4, 2 * row)
horizon_end  = (6 * column + 4, 2 * row + 1)
```

The released route therefore advances four meters along +X. The horizon adds a
one-meter +Y segment at its end. Neighboring rows are separated by two meters,
and neighboring columns begin six meters apart. This deterministic spacing
makes identities and paths visually distinguishable; it is not a calculation
of clearance, footprint, braking distance, allowed deviation, or safe
separation.

Representative terminal robots for each fixed scale are:

| Count | Last robot | Start (m) | Released end (m) | Horizon end (m) | Layout meaning |
| ---: | --- | --- | --- | --- | --- |
| 1 | `demo-001` | (0, 0) | (4, 0) | (4, 1) | Fault target only |
| 2 | `demo-002` | (0, 2) | (4, 2) | (4, 3) | Fault plus one control |
| 5 | `demo-005` | (0, 8) | (4, 8) | (4, 9) | Five visually reviewable lanes |
| 10 | `demo-010` | (0, 18) | (4, 18) | (4, 19) | One complete ten-lane column |
| 50 | `demo-050` | (24, 18) | (28, 18) | (28, 19) | Five columns of ten lanes |
| 100 | `demo-100` | (54, 18) | (58, 18) | (58, 19) | Ten-by-ten synthetic layout |

If `theta` is omitted from a node, the demo does not assert a required node
orientation. A rendered icon orientation is explanatory UI unless the emitted
VDA field explicitly carries that value. Likewise, a synthetic
`mobileRobotPosition.localized = true` describes the virtual actor's own state;
it is not evidence of a real localization system.

## Meaning of each scale

| Count | Primary purpose | What it does not prove |
| ---: | --- | --- |
| 1 | Preserve the original reconnect evidence matrix and provide the easiest trace to inspect manually. | Multi-robot isolation or concurrency. |
| 2 | Show one fault target beside one healthy control and detect the simplest cross-attribution bug. | Fleet-scale behavior. |
| 5 | Keep every route and identity human-reviewable while exercising repeated generation. | Production commissioning readiness. |
| 10 | Exercise a complete ten-lane coordinate column and a modest concurrent population. | A ten-robot physical deployment. |
| 50 | Exercise larger manifest, trace, topic, and actor sets within the same isolated job. | Throughput, latency, or broker capacity. |
| 100 | Exercise the supported upper input boundary and a deterministic 10 by 10 synthetic layout. | One hundred physical robots, production scale, interoperability, or a performance SLA. |

The counts name virtual mobile-robot actors, not vendors, robot models,
warehouses, customer devices, Docker hosts, or independent fleet managers.

## Manifest and hard-limit contract

Before MQTT clients start, the resolved run manifest records at least:

- `robot_count`;
- every robot identity and ClientId;
- every robot-specific topic set and order ID;
- the coordinate-system declaration;
- every start, released-end, and horizon-end route;
- the sole `fault.target_serial_number` (`demo-001`);
- the resolved message, duration, and actor limits; and
- `synthetic`, `same_job_isolated`, `connect_is_side_effect`, and
  `physical_dut_authorized: false` boundaries.

Budgets scale from the resolved robot count and remain finite:

```text
message ceiling         = 10 * robot_count + 16
actor ceiling           = robot_count + 2
MQTT connection ceiling = robot_count + 2
duration ceiling        = 60 seconds
```

The two supporting actors are fleet control and the recorder. The manifest
value used by a run is authoritative for that run; documentation must not infer
an unrecorded budget. Budget exhaustion invalidates the run rather than
silently dropping a robot or message.

## Evidence and verdict boundary

The recorder observes broker egress. A subscriber observation does not prove
the original publisher ClientId, publish-side QoS, packet ID, or an unobserved
message's absence. Those facts remain unknown unless the same-job harness
asserts them in the trace-bound evidence manifest.

For each robot, the evidence manifest binds observed records to an explicit
actor role, participant identity, and connection epoch. It also binds capture
closure and rule completeness to the exact trace digest. Missing, duplicate,
cross-robot, extra, or cross-trace evidence invalidates the asserted result.

The recorder's `source_sequence` orders observations only at that broker-egress
capture point. Interleaving between robots is not a global clock, synchronized
motion, or proof that two publishers acted simultaneously. Animation frames
are explanatory projections of the deterministic script.

At every scale the intended D4 interpretation remains:

- fault trace without the synthetic manifest: INCONCLUSIVE / UNRESOLVED;
- fault trace plus matching same-job manifest: one D4 FAIL with investigation
  target MOBILE_ROBOT; the manifest and event evidence identify synthetic
  `demo-001` as the configured participant, but the public target remains
  role-level; and
- healthy controls: no D4 finding for those controls.

No D4 finding is not a PASS or certification. One correctly isolated fault
does not prove the absence of other faults outside the implemented rule set.

## Performance boundary

The scale suite is a bounded functional workload, not a benchmark. A run may
record requested robot count, started and completed actors, planned and
captured messages, budget consumption, elapsed wall time, and environment/build
identity for debugging. These numbers must not be marketed as:

- maximum supported fleet size;
- message throughput or latency capacity;
- real-time determinism;
- CPU or memory sizing guidance;
- broker or network capacity;
- robot-control frequency;
- production reliability; or
- performance relative to another VDA implementation.

A successful 100-robot run proves only that the exact synthetic workload
completed within the recorded limits on the recorded isolated environment. A
performance claim requires a separate benchmark protocol with warm-up,
repetition, host and container provenance, resource telemetry, percentile
latencies, failure criteria, and independent reproduction.

## Safety and proof boundary

All counts run only on the ADR 0003 internal Docker network with a disposable
broker, no broker host port, and no external or physical DUT. The Doctor remains
network-disabled. The 100-robot option is isolated simulation, not authority to
connect 100 real clients or any real robot.

The demo does not model vehicle dynamics, footprints, obstacle sensing,
localization uncertainty, braking, collision avoidance, path reservation,
deadlock handling, battery behavior, wireless loss, or site safety. Spatially
separated synthetic lines do not constitute a safety analysis.

## Consequences

The multi-robot suite improves demonstrations of identity separation,
coordinate generation, bounded capture, and evidence attribution. It also
increases the risk of overstating a visually impressive 100-actor animation.
README, demo instructions, release notes, screenshots, and videos must label
the run as isolated synthetic simulation and preserve the performance and
hardware disclaimers above.

This ADR extends only the fixed synthetic suite. External brokers, software
DUTs, physical robots, customer traces, vendor attribution, generalized fault
injection, and performance certification still require separate plans and
explicit approval.

## References

- [VDA 5050 3.0.0 release](https://github.com/VDA5050/VDA5050/releases/tag/3.0.0)
- [VDA 5050 3.0.0 specification source](https://github.com/VDA5050/VDA5050/blob/3.0.0/VDA5050_EN.md)
- [Project protocol-source precedence](../PROTOCOL_SOURCES.md)
- [ADR 0003: Isolated Tier 1 Reconnect Demo](0003-isolated-tier1-reconnect-demo.md)
