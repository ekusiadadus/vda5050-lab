# ADR 0003: Isolated Tier 1 Reconnect Demo

- Status: Superseded in fleet scale and budgets by ADR 0004; evidence contract retained
- Date: 2026-08-07
- Scope: Synthetic virtual actors only

## Context

The offline Doctor needs a reproducible public demonstration of its central
evidence boundary. A passive subscriber trace cannot prove publisher identity,
connection epochs, or the absence of a publication at broker ingress. The
previous active-testing design therefore deferred every MQTT actor until after
a private pilot.

That general deferral remains correct for external software and physical DUTs.
It is unnecessarily broad for one self-contained demonstration in which the
same job owns the broker, all virtual actors, recorder, artifacts, and teardown.
The demo is a release and contributor-onboarding artifact; it is not evidence
that the product has solved a customer incident.

## Decision

Implement one Tier 1 scenario in the separate `vda5050-demo` binary:

1. a virtual fleet-control actor publishes a released base;
2. a virtual mobile robot publishes retained `ONLINE` and traverses A to B;
3. its MQTT connection ends abnormally and Mosquitto publishes the retained
   `CONNECTION_BROKEN` Last Will;
4. the same virtual participant reconnects in a new synthetic connection epoch
   but the fault variant omits retained `ONLINE`;
5. the recorder creates a canonical broker-egress trace; and
6. Doctor diagnoses the saved files with no network access.

The control variant publishes retained `ONLINE` after reconnect. The same fault
trace is also analyzed without its evidence manifest. The required matrix is:

| Input | D4 outcome |
| --- | --- |
| Fault trace plus matching same-job manifest | FAIL / MOBILE_ROBOT |
| The same fault trace without the manifest | INCONCLUSIVE / UNRESOLVED |
| Fresh-broker control plus matching manifest | no D4 finding |

## Safety boundary

The supported runner uses a digest-pinned Mosquitto image on a Docker internal
bridge. The broker has no host-published port. The runner inspects both the
resolved Compose configuration and the live network/port bindings before the
demo connects. Doctor uses `network_mode: none`.

The original resolved run manifest was written before any MQTT client was
created and fixed the broker identity, three run-derived ClientIds, four exact
topics, the connection-only retain allowlist, one abnormal disconnect, 64
captured messages, 30 seconds, and three actors. ADR 0004 replaces those
single-robot budget values with a fully expanded per-fleet manifest and bounded
`N`-dependent limits. `physical_dut_authorized` remains false.
Runtime containers are read-only, non-root, capability-free, protected by
`no-new-privileges`, and have CPU, memory, process, and temporary-filesystem
limits. Cleanup runs on success, failure, and interruption.

The standalone `--isolated-network` flag is only an operator assertion. It does
not independently prove namespace ownership. Only the supplied runner performs
the additional Docker checks. Neither path is authorized for a shared,
customer, staging, production, routable, or persistent broker.

## Evidence contract

The recorder observes broker egress and does not claim publisher-side MQTT
packet fields. Canonical records retain capture point, capture-local monotonic
time, clock domain and epoch, source sequence, delivery QoS, and DUP observation.

The separate synthetic manifest is digest-bound to one canonical trace and
contains explicit actor, participant, and connection-epoch assertions. Doctor
accepts it only when it declares same-job isolated synthetic scope, capture
closure, rule completeness, and complete mappings for every message record.
Missing, extra, modified, or cross-trace mappings fail closed.

This evidence may support a finding about the virtual actor only. It is not
production capture completeness, vendor attribution, interoperability proof,
hardware evidence, or certification.

## Alternatives considered

- A viewer-only demo was rejected because it would not demonstrate the
  evidence-dependent verdict change.
- A third-party VDA 5050 simulator was rejected for the first slice because the
  available projects did not provide this VDA 5050 3.0 reconnect/evidence
  contract and would enlarge the trusted dependency boundary.
- A host-published Mosquitto port was rejected because it would weaken the
  same-job isolation claim.
- A transparent MQTT fault proxy was deferred because publisher identity, DUP,
  QoS, packet IDs, and retained behavior cannot be preserved by a simple
  subscribe-and-republish bridge.
- External software and physical DUT runs remain deferred because they require
  separate identity, authorization, TLS, emergency-stop, and campaign controls.

## Consequences

The workspace now contains a network-capable demo dependency, but Doctor's
distributed dependency graph and executable remain offline. Unit coverage
floors continue to apply to Doctor and its libraries; the live orchestration is
gated by real Docker/Mosquitto incident, passive, control, and pre-CONNECT
failure tests. Release media is generated from a successful real demo run and
is explanatory, not protocol evidence.

This ADR narrowly replaces the earlier “no Tier 1 before private pilot” entry
condition for this one synthetic scenario. It grants no precedent or authority
for more scenarios, user-selected brokers, generic faults, external DUTs, or
physical robots. Crossing any of those boundaries requires a new ADR,
implementation plan, and explicit approval.
