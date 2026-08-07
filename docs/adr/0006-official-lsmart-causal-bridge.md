# ADR 0006: Official LSMART as a causal Tier 1 trace producer

- Status: Accepted
- Date: 2026-08-07
- Scope: one pinned ten-robot isolated demonstration
- Extends: ADR 0005 without changing Doctor's offline boundary

## Context

The deterministic two-robot simulator clearly demonstrates the Doctor evidence
boundary, but its two fixed lanes do not look or behave like lifelong warehouse
traffic. The local demonstration needs real multi-agent planning, continuous
task assignment, controller execution, and robot motion without making the
Doctor an MQTT client or pretending that a custom animation is LSMART.

## Decision

Use the official `smart-mapf/lifelong-smart` repository at commit
`ce0a020d8da10a806b23ffa3ccb56b1affff57d1`, RHCR with PBS, PIBT fail policy,
the windowed task assigner, the official ADG/controller, ARGoS beta59 at commit
`3ef43eb857810a2a461a51f2574a9913cf56702f`, seed 42, ten robots, and the
`kiva_large_w_mode` map.

Add a narrow execution-boundary overlay. Its server-side line serializes the
documented `Action.task_id` through the existing ADG RPC instead of reading the
unused `task_ptr`; this preserves station identity without changing plans,
dependencies, queueing, or task accounting. For each action batch returned by
the official ADG, the controller-side portion writes a bounded outbox envelope
and does not queue those actions. A separate Fleet bridge projects that
envelope into a VDA 5050 3.0.0 order and publishes it to the isolated broker.
The per-robot adapter must receive the order from MQTT, revalidate its complete
action semantics, and write the matching delivery envelope before the overlay
queues it. This makes the MQTT/VDA path causal to robot execution.

Actual ARGoS poses are projected into VDA state and visualization messages.
An independent broker subscriber builds the canonical capture and same-job
evidence. Doctor runs only after capture closes and with networking disabled.

## Isolation and distribution

The broker has no host port and only joins the internal Tier 1 network. The
official LSMART container has no network namespace beyond loopback; the VDA
bridge is the only process joining MQTT and the controller exchange. The Web
gateway joins a separate cockpit network, exposes only a localhost port, and
reads fixed artifacts.

The pinned LSMART source has no root license file. It is therefore an external
clean checkout, not vendored source. The repository records its exact commit,
the local patch, and provenance. The derived image is local-only and must not
be pushed or redistributed until upstream licensing is clarified.

## Verification

The integration must fail closed unless:

- source origin, commit, cleanliness, overlay applicability, and map digest
  match the sealed inputs;
- the resolved broker network is internal and has no host port;
- all ten MQTT robot subscriptions are ready before LSMART starts;
- every outbox action batch has exactly one matching MQTT-returned inbox
  delivery;
- bridge order and delivery counts match and actual ARGoS poses exist for all
  ten robots;
- official LSMART reports at least one finished task; and
- Doctor preserves the passive/same-job evidence distinction.

## Consequences

The cockpit can now show genuine lifelong warehouse traffic while the recorded
VDA messages remain causally relevant to execution. The demo is heavier,
requires amd64 emulation on Apple Silicon, and depends on an externally obtained
source checkout. It remains synthetic evidence and makes no certification,
third-party interoperability, production performance, physical fidelity, or
hardware claim.
