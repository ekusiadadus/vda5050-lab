# Active VDA 5050 Testing Safety Design

- Status: Tier 1 synthetic slice implemented; Tiers 2 and 3 deferred and unapproved
- Design revision: 0.2
- Research snapshot: 2026-08-07
- Doctor dependency: None; `vda5050-doctor` remains offline
- Current implementation scope: one isolated reconnect scenario with 1..=100 virtual mobile robots

## 1. Purpose

This document records both the narrow Tier 1 safety contract now implemented by
`vda5050-demo` and the retained requirements for any later active harness. It
keeps MQTT code and authority outside the `vda5050-doctor` execution path.

The current authorization is limited to a disposable loopback broker, or the
exact service name `broker` inside the supplied runner's validated internal
network, with 1 through 100 virtual mobile robots, one fleet-control actor, one
recorder, and the built-in reconnect scenario. The implementation may CONNECT,
subscribe, publish, and deliberately crash the synthetic `demo-001` client only
inside that boundary.

This document does not authorize:

- a customer, shared, routable, or production broker;
- an external software DUT;
- a physical robot or hardware campaign;
- a generic fault bridge or user-defined active scenario;
- Tier 2 or Tier 3 execution; or
- treating same-job synthetic evidence as production evidence.

The offline Doctor must not depend on this design or include a network code
path. It consumes only explicitly selected local trace files and, optionally,
a matching local synthetic evidence manifest.

## 2. Current approval and future entry conditions

The current Tier 1 approval covers only the code and local execution described
in [DEMO.md](DEMO.md). It does not expand by analogy.

Any broader Tier 1 feature, Tier 2 work, or Tier 3 work may be proposed only
when:

1. users identify the active test as a higher-value next step than additional
   offline diagnosis or import support;
2. the exact target is stated: isolated virtual actor, external software DUT,
   or physical DUT;
3. an active threat and hazard assessment is reviewed;
4. the issuer and runner trust boundaries are selected;
5. the MQTT client and session contract is complete;
6. an implementation and test plan is separately approved; and
7. every external or physical execution receives its own approval.

Approval of the Tier 1 reconnect demo does not satisfy these conditions.

Unless a subsection explicitly describes the current Tier 1 reconnect demo,
the capability, preflight, bridge, export, Tier 2, and Tier 3 requirements below
are future gates, not claims about implemented behavior.

## 3. Safety principles

- MQTT CONNECT is a side effect, even when no publication follows.
- A duplicate ClientId can disconnect an existing client.
- A harness stop is not a robot emergency stop.
- cancelOrder, disconnect, broker shutdown, and Last Will are not emergency-stop
  mechanisms.
- Topic names and namespace heuristics cannot prove a target is safe.
- Active execution uses exact identities, exact topics, hard budgets, and
  fail-closed defaults.
- A cryptographic token proves authorization only when its issuer is outside the
  runner's trust domain.
- A subscriber observation does not prove original publisher properties.
- A fault bridge is not a transparent MQTT proxy.
- Physical testing always depends on an external safety system and site
  authority.

## 4. Test tiers

| Tier | Target | Network boundary | Execution authority |
| --- | --- | --- | --- |
| 0 | Offline trace | No network code path | Offline plan |
| 1 | Ephemeral broker and virtual actors | Proved loopback or isolated ephemeral network | Same-job isolated capability |
| 2 | External software DUT | Explicit test network, authenticated endpoint | Authorized operator or independent signer |
| 3 | Physical DUT | Site-approved test cell | Site authority plus operator and safety observer |

Plaintext MQTT is prohibited for Tiers 2 and 3. Tier 1 may use plaintext only
when the network is proved to be loopback-only or an ephemeral isolated
namespace owned by the same job.

No capability may be upgraded from one tier to another.

## 5. Issuer and runner trust boundary

### 5.1 Tier 1

The implemented slice does not issue a reusable capability. It resolves and
writes a run manifest before creating MQTT clients and accepts only:

- a loopback IP address;
- `localhost`; or
- the exact service name `broker` when `--isolated-network` is present.

Unspecified addresses, arbitrary hostnames, and routable IP addresses fail
before MQTT CONNECT. When the binary is invoked directly,
`--isolated-network` is only an operator assertion. The supported Compose runner
adds machine checks before CONNECT: it validates the fully resolved internal
bridge and fixed service command, then inspects the live network and verifies
that the broker has no host port binding. This proves the tested Docker
properties, not general broker identity or host egress isolation.

The current run manifest records the robot count, coordinate system, every
robot identity and route, exact ClientIds, exact topic and retain allowlists,
the sole `demo-001` fault target, and these count-derived ceilings:

```text
messages         = 10 * robot_count + 16
duration_seconds = 60
actors           = robot_count + 2
mqtt_connections = robot_count + 2
```

The message cap is enforced during capture. Fixed scenario steps have finite
timeouts, and a run exceeding 60 seconds is rejected before evidence artifacts
are finalized; the duration check is a validity gate, not an asynchronous
emergency stop. Connection messages use QoS 1 and retained publication; order,
state, and visualization messages use QoS 0 and are not retained.

The published counts are 1, 2, 5, 10, 50, and 100. The 100-robot case is an
isolated synthetic workload, not proof of physical fleet size, performance,
interoperability, broker capacity, or production readiness.

The ideal same-job capability described elsewhere in this document remains a
future hardening requirement before Tier 1 can claim broker nonce, host-egress,
general ownership, and reusable capability proof.

### 5.2 Tier 2

The runner cannot issue its own capability.

Required separation:

- an authorized operator authenticates to a separate issuer;
- the issuer owns the signing key;
- the runner has only a public verification key;
- the issuer displays the fully resolved manifest, not only its digest;
- the operator approves the exact target, clients, subscriptions, publications,
  scenarios, faults, and budgets;
- the issuer records operator identity and approval time; and
- CI cannot issue a Tier 2 capability using repository credentials.

An interactive yes prompt in the runner is insufficient by itself.

### 5.3 Tier 3

Tier 3 requires:

- a signed site approval artifact;
- named run operator;
- named independent safety observer;
- named site authority;
- exact physical inventory;
- verified external emergency stop;
- approved test area and exclusion procedure; and
- dual confirmation immediately before the run.

The runner cannot create, sign, or broaden the site approval artifact.

### 5.4 Capability scope

The signed capability binds:

- manifest digest;
- run ID and nonce;
- tier;
- issuer and operator identities;
- issue and expiry times;
- executable build digest;
- scenario digest;
- resolved broker identity;
- exact MQTT client manifests;
- DUT identities;
- publish and subscribe allowlists;
- message types;
- fault schedule;
- hard budgets;
- retain and Last Will policies; and
- physical approval digest when applicable.

A mismatch aborts. There is no partial use or interactive widening.

### 5.5 Revocation and consumption

- Issuers maintain a revocation list or short-lived online verification method
  selected by the security plan.
- Capability lifetime is minimized and cannot cross an approved maintenance
  window.
- The runner holds an exclusive run lock.
- Nonce consumption uses a durable transactional store with a unique nonce
  constraint.
- Consumption commits and is synchronized to durable storage before the first
  MQTT CONNECT.
- A crash after consumption requires a new capability.
- Store unavailability, lock failure, duplicate nonce, or uncertain commit
  aborts.

The exact transactional technology is selected and tested in the future
implementation plan.

## 6. Preflight without MQTT side effects

Preflight resolves configuration and produces a proposed manifest.

By default it may perform:

- local configuration validation;
- DNS resolution;
- TCP reachability check only when approved for the tier;
- TLS handshake and certificate-chain validation;
- SPKI fingerprint comparison; and
- local route and network-namespace inspection.

It does not perform MQTT CONNECT by default.

If a broker cannot be identified without MQTT CONNECT, the probe is a separate
approved active operation. Its exact ClientId, session settings, credentials,
keepalive, Last Will, and zero-publication policy appear in the manifest.

“Preflight publishes nothing” is not sufficient safety language because CONNECT
can disconnect an existing client with the same ClientId.

## 7. MQTT client and session contract

Every harness client has an explicit manifest:

~~~yaml
client_role: CAPTURE | ROBOT_ACTOR | FLEET_ACTOR | BRIDGE_IN | BRIDGE_OUT
client_id: exact string
client_id_generation_basis: fixed or reviewed derivation
dut_client_id_inventory_digest: required
non_collision_proof: required
protocol_version: MQTT_3_1_1 or selected supported version
clean_session: exact boolean
session_expiry: exact value where supported
session_present_expected: exact value
existing_session_policy: ABORT | EXPLICITLY_CLEAR
keepalive_seconds: positive bounded value
connect_timeout_seconds: positive bounded value
credentials_reference: protected runtime reference
tls_server_name: exact value
spki_sha256: exact value for Tier 2 and 3
lwt:
  enabled: false by default
  topic: none
  qos: none
  retain: none
subscriptions: []
publish_topics: []
~~~

Requirements:

- Every client ID is listed before approval.
- Wildcards or random IDs not bound into the manifest are forbidden.
- The client ID inventory must prove non-collision with known DUT clients.
- If non-collision cannot be proved, CONNECT is prohibited.
- MQTT 3.1.1 harness clients use CleanSession true by default.
- MQTT 5 harness clients use session expiry zero by default.
- A returned Session Present value contrary to the manifest aborts.
- Existing-session clearing is a separate approved side effect.
- Client IDs are not reused across concurrent or expired runs.
- Keepalive and reconnect policy are finite and explicit.
- Automatic reconnect is disabled unless its count and timing are budgeted.
- LWT is disabled by default.
- Subscriptions and requested QoS are exact.
- Broker ACLs should independently restrict the same allowlists.

The manifest records every CONNECT, CONNACK, disconnect, session-present result,
and unexpected broker action.

## 8. Resolved preflight manifest

The human-readable manifest expands effective values:

~~~yaml
run:
  id: required
  nonce: required
  tier: 1 | 2 | 3
  expires_at: required
  tool_build_digest: required
  scenario_digest: required
broker:
  scheme: mqtt | mqtts
  hostname: exact configured value
  resolved_addresses: exact list
  port: exact value
  tls_server_name: exact value
  spki_sha256: required for Tier 2 and 3
  instance_identity: required
dut:
  identities: exact list
  inventory_digest: required
clients:
  - complete MQTT client manifest
topics:
  publish_allowlist: exact topics only
  subscribe_allowlist: exact topics or reviewed filters
message_types: exact list
budgets:
  max_connects: positive bounded value
  max_reconnects: bounded value
  max_messages: positive bounded value
  max_publish_rate_per_second: positive bounded value
  max_duration_seconds: positive bounded value
  max_concurrent_actors: positive bounded value
  max_payload_bytes: positive bounded value
  max_queue_messages: positive bounded value
  max_queue_bytes: positive bounded value
retain_policy:
  default: DENY
  exceptions: fully expanded list
lwt_policy:
  default: DENY
  exceptions: fully expanded list
fault_schedule:
  seed: exact value
  drop_count: bounded value
  duplicate_factor: bounded value
  maximum_delay_ms: bounded value
  reorder_window_messages: bounded value
  queue_limits: fully expanded
  resolved_operations: human-readable summary
physical:
  enabled: exact boolean
  approval_digest: required for Tier 3
  estop_evidence_digest: required for Tier 3
~~~

Zero, missing, negative, overflowed, or “unlimited” budgets are invalid.

The operator sees resolved fault behavior and hard limits. A scenario digest
alone is insufficient approval evidence.

## 9. Publish, retain, and Last Will policy

- Publish topics use an exact allowlist.
- Plus and hash MQTT wildcards are forbidden in publish entries.
- Message types and VDA identities are independently allowlisted.
- Retained publication is denied by default.
- Last Will is denied by default.
- A retained or Last Will test requires its exact topic, payload class, QoS,
  retain value, trigger, cleanup, and rationale in the capability.
- Cleanup consumes the same hard budgets.
- Cleanup failure invalidates the active run and remains in the report.
- The harness never promises it can remove an unknown retained value safely.

## 10. Execution budgets

Budgets are enforced independently at:

- scenario scheduling;
- actor publication;
- bridge forwarding;
- reconnect behavior;
- queue admission; and
- total run control.

The most restrictive applicable limit wins.

Counters use checked arithmetic. A counter overflow, time-source failure, or
uncertain persisted state aborts.

No budget may increase after capability issuance. A lower emergency runtime
limit may be applied locally and is recorded.

## 11. Fault bridge

The possible future bridge is an application-level MQTT bridge, not a
transparent packet proxy.

Supported topology:

- two brokers with explicit direction; or
- one broker with mathematically disjoint input and output namespaces.

Every direction has:

- exact input filters;
- exact output mapping;
- separate MQTT clients;
- queue bounds;
- schedule;
- forwarding configuration; and
- internal event correlation.

Loop prevention:

- resolved inputs and outputs must be disjoint;
- bidirectional mappings must prove neither output rematches either input;
- every delivery receives an internal bridge event ID;
- correlation stays outside the VDA payload;
- vendor extension fields are not inserted for loop control; and
- ambiguity aborts before CONNECT.

The bridge does not claim to preserve:

- publisher ClientId;
- packet ID;
- inbound DUP;
- original publish QoS when not observed;
- broker-internal ordering; or
- transparent retained-delivery semantics.

Determinism means the same normalized inputs, bridge configuration, and seed
produce the same intended fault decisions and forwarding order. It does not
promise byte-identical timing or identical DUT responses. Timing assertions use
tolerances and capture-local monotonic clocks.

## 12. Capture and time

Active capture uses the same observation, assertion, inference, and
recommendation separation as the offline product.

Every process records:

- capture point;
- source sequence;
- observed wall time;
- observed monotonic nanoseconds;
- monotonic origin and resolution;
- clock epoch;
- restart events;
- mapping uncertainty; and
- known gaps.

A process restart creates a new epoch. Different clock domains are not directly
subtracted. Harness-created correlation may establish partial-order edges
without inventing a global clock.

Publisher properties are assertions only when captured at an adequate publisher
or broker vantage.

## 13. Kill switch and emergency stop

The harness kill switch:

- stops new scheduled publications;
- stops bridge forwarding;
- disconnects harness clients;
- records skipped operations;
- records disconnect attempts and outcomes; and
- writes an append-only termination record.

It does not guarantee a robot stops moving.

Tier 3 requires an independently verified external emergency stop whose safety
function does not depend on:

- the harness;
- the broker;
- VDA 5050;
- cancelOrder;
- Wi-Fi;
- the same computer; or
- the same power path when the site procedure requires independence.

The observer can trigger the emergency stop without operator software.

## 14. Physical campaign contract

Every Tier 3 campaign separately records:

- site and approved area;
- robot inventory and serial numbers;
- firmware and software versions;
- payload and fixture state;
- map and route constraints;
- broker, network, and RF topology;
- certificate fingerprints;
- DUT and harness ClientIds;
- safety observer and run authority;
- external emergency-stop test evidence;
- pre-run inspection;
- scenario and capability digests;
- all hard budgets;
- permitted initial states;
- abort conditions;
- communication-loss behavior;
- post-run inspection;
- artifact retention; and
- incident procedure.

Approval for a simulation, software DUT, smaller fleet, or earlier date cannot
authorize a later physical campaign.

Results from simulation, software DUTs, and physical robots are reported as
different evidence classes.

## 15. Evidence export if later approved

No export feature is inherited from the offline Doctor.

If active evidence export is proposed:

- call it pseudonymized, never anonymous;
- use HMAC-SHA-256 or a reviewed equivalent with a fresh high-entropy per-case
  secret;
- generate the key with an operating-system cryptographic source;
- store it outside the export and outside Git;
- define access, backup, rotation, and destruction;
- fail closed for unknown fields, free text, malformed payloads, and binary
  content;
- remove, quantize, or synthetically replace geometry;
- publish no unkeyed digest of the private source trace;
- use a keyed commitment only when linkability is explicitly required; and
- include a residual-risk manifest.

Translation or rotation of coordinates is not anonymization because facility
shape remains recognizable.

## 16. Security and supply chain

- Active code is in the separate `vda5050-demo` workspace package.
- The `vda5050-doctor` dependency graph excludes the MQTT client dependency.
- Credentials use protected runtime references and never enter manifests or
  reports.
- TLS verification is enabled for Tier 2 and 3.
- Test CAs and private keys are generated ephemerally with restrictive
  permissions.
- No private-key fixture is committed.
- Dependencies and licenses are inventoried.
- Cargo.lock is committed.
- Advisory audits run continuously.
- GitHub Actions use full commit SHAs and minimum permissions.
- Releases include an SBOM and provenance.
- Release credentials are unavailable to untrusted pull-request workflows.

## 17. Required tests before any active release

### Trust and capability

- unauthorized issuer;
- issuer and runner key separation;
- wrong operator or tier;
- expired, revoked, altered, and replayed capability;
- concurrent nonce consumption;
- crash before and after durable consumption;
- store or lock failure;
- manifest drift; and
- attempted scope widening.

### MQTT side effects

- preflight performs no MQTT CONNECT by default;
- approved TLS-only probe;
- exact ClientId inventory;
- known ClientId collision rejection;
- unknown non-collision proof rejection;
- unexpected Session Present;
- CleanSession and session-expiry enforcement;
- reconnect-budget enforcement;
- LWT deny by default;
- exact subscription and QoS; and
- CONNECT audit records.

### Budgets and publication

- exact topic and identity allowlists;
- wildcard publication rejection;
- message, rate, duration, actor, payload, queue, and connect limits;
- checked counter overflow;
- retain and Last Will exceptions;
- cleanup budget and failure;
- kill-switch termination; and
- no budget increase after arming.

### Bridge and faults

- dual-broker topology;
- disjoint namespace proof;
- all rematch loop cases;
- original and forwarded event correlation;
- queue overflow;
- resolved fault-summary accuracy;
- seeded schedule reproduction; and
- tolerance-aware timing.

### Physical boundary

- missing site approval;
- inventory drift;
- expired approval;
- missing or failed external emergency-stop evidence;
- absent safety observer;
- changed test area;
- incomplete pre-run inspection; and
- attempt to reuse earlier campaign authority.

Safety-control deny paths require full decision and boundary coverage plus
mutation testing where the toolchain supports it.

## 18. Acceptance gates

### Current Tier 1 reconnect demo

- The only scenario uses 1..=100 virtual mobile robots. `demo-001` receives
  `CONNECTION_BROKEN` and enters a new synthetic session that omits `ONLINE`;
  other robots remain connected as non-fault background controls. The explicit
  control variant publishes `ONLINE` for `demo-001` after reconnect.
- The fixed scale suite is 1, 2, 5, 10, 50, and 100.
- The broker target validator accepts loopback by default and the exact service
  name `broker` only with an explicit isolated-network assertion.
- The run emits a resolved run manifest, canonical trace, and trace-bound
  synthetic evidence manifest.
- Doctor without the manifest remains passive and returns D4 INCONCLUSIVE for
  the fault trace.
- Doctor with the matching same-job manifest can return D4 FAIL for the fault
  trace and no D4 finding for the control trace.
- Every result is labeled synthetic. It is not customer, production, external
  DUT, physical, interoperability, or performance evidence.

The current implementation does not yet satisfy the reusable capability,
broker-nonce, and host-egress proofs required by the following future release
gate.

### Isolated active alpha

- Tier 1 only.
- The job owns every broker, actor, and network namespace.
- No external route exists.
- Exact MQTT clients and budgets are recorded.
- Capability and deny-path tests pass.
- Active capture reproduces supported findings offline.

### External software DUT alpha

- Separate approval.
- Tier 2 issuer and runner separation is verified.
- TLS and SPKI pinning pass.
- ClientId non-collision is proved.
- Every CONNECT is authorized.
- Endpoint, identity, subscription, publication, build, scenario, or budget
  drift aborts.
- Fault topology and loop tests pass.

### Physical preview

- Separate campaign approval.
- Tier 3 trust and dual-control requirements pass.
- External emergency stop is tested.
- Physical inventory and boundaries match.
- Hardware results are not generalized to untested configurations.

## 19. Open decisions

The future implementation plan must select and justify:

- capability signature and issuer deployment;
- operator identity provider;
- revocation design;
- durable nonce store and synchronization semantics;
- broker identity method beyond endpoint and SPKI;
- ClientId inventory authority;
- MQTT protocol versions;
- credential injection mechanism;
- isolated-network proof;
- fault schedule language;
- clock synchronization requirements; and
- audit-record retention.

None may be silently chosen during coding.

## 20. Approval boundary

The implemented approval boundary ends at the Tier 1 reconnect demo described
in [DEMO.md](DEMO.md). Within it, `vda5050-demo` may use the declared virtual
actors, up to 100 virtual mobile robots, and MQTT messages against a disposable
loopback broker or the supplied Compose runner's validated internal `broker`
service.

It authorizes no external broker, external system, customer environment,
software DUT, physical operation, generic fault injection, or Tier 2/3
capability. Direct use of `--isolated-network` is not independent proof of
isolation, and the generated evidence manifest is not production proof.

Create a new implementation plan and request explicit approval before crossing
that boundary.
