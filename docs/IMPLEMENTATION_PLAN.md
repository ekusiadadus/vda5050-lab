# vda5050-lab Implementation Plan

- Status: Phase 1 implemented; v0.1.2 corrective Developer Preview candidate
- Plan version: 0.7
- Supersedes: release-candidate revision 0.6
- Research snapshot: 2026-08-07
- Target repository: vda5050-lab
- Initial executable: vda5050-doctor
- Product implementation state: Offline vertical slice prepared for prerelease
- Initial operating mode: Offline only

## 1. Executive decision

vda5050-lab will not begin by implementing a general behavioral
interoperability platform.

Its first product will be vda5050-doctor: an offline CLI that explains a small
set of high-value VDA 5050 integration incidents from captured MQTT
observations.

The product contract is:

> Given a local trace from a stopped or misbehaving integration, produce an
> evidence-bounded explanation and the next useful investigation action in less
> than ten minutes of user time.

The first customer is a system integrator or robot protocol engineer during
commissioning. The first job is incident triage, not certification, generic
monitoring, active conformance testing, or physical diagnosis.

The project earns the right to expand only after it demonstrates value on real
incident traces. A technically complete platform without that evidence is not a
successful product validation. A Developer Preview may ship to recruit pilot
users only when its limitations remain explicit.

## 2. Why revision 0.5 changes the sequence

The problem is real:

- [arculus](https://www.arculus.de/a-look-into-vda5050) describes manual MQTT
  JSON investigation as time-consuming and error-prone, and built both a
  visualizer and automated compliance tests.
- Public library issues show cross-message failures involving
  [newBaseRequest](https://github.com/coatyio/vda-5050-lib.js/issues/44),
  [reconnect state](https://github.com/coatyio/vda-5050-lib.js/issues/38), and
  [cancel transitions](https://github.com/coatyio/vda-5050-lib.js/issues/33).
- Interoperability discussions such as
  [omitted theta semantics](https://github.com/VDA5050/VDA5050/issues/613)
  demonstrate that some cases require an explicit INCONCLUSIVE result rather
  than an invented universal interpretation.
- [VDA 5050 3.0.0](https://github.com/VDA5050/VDA5050/releases/tag/3.0.0)
  is the current release in this research snapshot, while
  [mixed 2.x and 3.x deployments](https://github.com/VDA5050/VDA5050/issues/618)
  create a migration and diagnosis problem.

The initial competitive wedge is narrow. [Fletr](https://fletr.io/documentation)
already provides VDA-oriented MQTT monitoring, log import, timelines, and
multi-version support. arculus already describes a visualizer and an active
compliance suite. Therefore these statements are not sufficient positioning:

- a viewer for VDA 5050;
- a JSON validator;
- a message timeline;
- a simple AGV simulator; or
- an active test runner by itself.

Revision 0.5 differentiates on five outputs:

1. an incident explanation;
2. message-level evidence;
3. the applicable source and normative subject;
4. an explicit account of missing evidence; and
5. the next investigation target without unsupported blame.

## 3. Review disposition

Revision 0.5 preserves the product correction from revision 0.3, records the
implemented proof boundaries found during test-first execution, and adds a
truthful Developer Preview release contract.

### 3.1 Product correction

The following work leaves the initial critical path:

- active MQTT actors;
- software or physical DUT execution;
- fault injection and fault bridges;
- complex arming capabilities;
- Web UI;
- JUnit and SARIF projections;
- pseudonymized export;
- general multi-capture partial-order reconstruction; and
- broad syntax coverage unrelated to the first incident families.

These designs are not silently discarded. Active work is retained in
[FUTURE_ACTIVE_TESTING.md](FUTURE_ACTIVE_TESTING.md), where it remains
unapproved.

### 3.2 Model and safety corrections

This revision also corrects the remaining review blockers:

| Review blocker | Revision 0.5 disposition |
| --- | --- |
| Applicability could not be unknown | Adds UNKNOWN applicability and UNEVALUATED verdict with legal-state constraints |
| Temporal deadlines lacked monotonic observations | Adds optional capture-local monotonic time, epoch, origin, resolution, wrap policy, and wall-clock uncertainty |
| Arming could be cryptographic self-approval | Defines separate issuer and runner trust domains for future Tier 2/3 work |
| MQTT ClientId and session side effects were absent | Future active manifest fixes every client ID, session, LWT, keepalive, subscription, and CONNECT approval |
| Same-order content equality was undefined | Adds a versioned three-level order comparator with conservative UNKNOWN handling |
| Errata could not correctly override a PDF | Makes the PDF the base authority and permits only formally published, clause-scoped errata overlays |
| Capture manifest was both immutable and mutable | Replaces it with CaptureOpened and CaptureClosed records plus a derived final manifest |
| Offline containment was not testable | Local content-addressed resolution only; network URI, file URI, traversal, and symlink escape fail closed |
| Fault parameters were hidden behind a digest | Future preflight expands the resolved schedule and every hard limit |
| Pseudonymization key handling was undefined | Removes export from the Developer Preview and defines HMAC key and public-digest rules for any future feature |
| Bundle and tool build were over-coupled | Bundle pins evaluator API and rule digest; analysis report pins the actual tool build |
| Finding severity and VDA error level were mixed | Uses finding_severity and expected_protocol_error_level as different fields |
| Release supply-chain controls were incomplete | Commits Cargo.lock, pins CI actions by SHA, uses minimum permissions, audits advisories, produces an SBOM, and records provenance |

## 4. Current repository state

The repository contains the Rust workspace, CLI, six library crates, locked
dependencies, rule and source manifests, synthetic fixtures, CI and release
workflows, community files, and release documentation. The `v0.1.2` release
contract produces four native archives, SHA-256 checksums, a CycloneDX SBOM,
and Sigstore-backed GitHub attestations.

Local tests, lint, audit, coverage, release build, CLI smoke test, source
manifest integrity, and SBOM generation are evidence recorded for the release
candidate. GitHub Actions and published-asset verification remain separate
remote proof and must pass before the release is called available.

No existing robot, simulator, planner, fleet manager, or customer trace is part
of this repository.

## 5. Initial user and job

### 5.1 Primary user

A system integrator or robot protocol engineer commissioning a VDA 5050
integration across a mobile robot and fleet-control implementation.

### 5.2 Trigger

The user has an incident such as:

- an order stopped progressing;
- a base update did not recover the system;
- connection status did not recover after reconnect;
- cancel or action state became inconsistent; or
- VDA 5050 2.1 and 3.0 participants disagree.

### 5.3 Desired outcome

Within ten minutes, the user can:

- identify the best-supported protocol-level explanation;
- open the exact supporting observations;
- distinguish normative findings from project diagnostics;
- see what the trace cannot prove;
- request the next useful log or capture vantage; and
- route the investigation without falsely blaming a vendor.

### 5.4 Initial buyer and contributor hypothesis

The likely economic user is an integration team for which one delayed
commissioning incident is expensive. The likely OSS contributor is a protocol
or library maintainer who can contribute a minimized incident fixture and a
source-backed rule.

Both hypotheses must be tested. They are not treated as established market
facts.

## 6. Product contract

### 6.1 Inputs

The Developer Preview accepts local, regular files:

- canonical vda5050-lab JSONL;
- a documented message-envelope JSON array; and
- explicitly selected adapters for a small number of common line-log formats.

There is no format auto-detection that silently skips unrecognized data.
Every input record produces one of:

- imported message;
- recognized non-message record;
- rejected record with offset and reason; or
- capture gap.

An import summary reports accepted, rejected, truncated, and unknown records
before any incident diagnosis runs.

### 6.2 Outputs

The Developer Preview emits:

- a bounded terminal summary; and
- canonical JSON.

Each incident report contains:

~~~yaml
incident_id: stable rule and case identifier
summary: bounded generated text
status: SUPPORTED | INCONCLUSIVE | NOT_APPLICABLE
applicability: APPLICABLE | NOT_APPLICABLE | UNKNOWN
verdict: PASS | FAIL | INCONCLUSIVE | UNEVALUATED
finding_severity: ERROR | WARNING | INFO
normative_subject: MOBILE_ROBOT | FLEET_CONTROL | UNRESOLVED
investigation_target: MOBILE_ROBOT | FLEET_CONTROL | TRANSPORT | UNRESOLVED
evidence_event_ids: []
counterevidence_event_ids: []
missing_evidence: []
next_actions: []
authority: {}
expected_protocol_error_level: ERROR | WARNING | NONE | UNKNOWN
confidence: HIGH | MEDIUM | LOW | UNKNOWN
~~~

finding_severity is an analyzer presentation concept.
expected_protocol_error_level is the VDA response expected from a protocol
participant. They are not interchangeable.

### 6.3 Ten-minute promise

Ten minutes is an end-to-end user outcome, not merely CPU runtime. Pilot timing
starts when the participant receives the accepted trace and task and stops when
the participant records an expert-accepted next action.

Import preparation performed only for the tool is included. Time spent
collecting an unavailable log is separately reported as missing-evidence time.

## 7. Explicit non-goals

The initial product does not:

- connect to a broker or external URL;
- publish MQTT or VDA 5050 messages;
- control, simulate, or fault-inject a robot;
- prove that an unobserved event did not occur;
- diagnose physical sensors, actuators, localization, or battery causes;
- perform traffic management, path planning, or collision avoidance;
- provide generic MQTT exploration;
- provide a Web UI;
- provide JUnit or SARIF output;
- export customer traces;
- automatically transform a 2.1 deployment into 3.0;
- certify official VDA conformance, cybersecurity, or functional safety; or
- declare an organization or vendor responsible when the normative subject
  cannot be proved.

## 8. Result and evidence model

### 8.1 Observations, assertions, inferences, and recommendations

An observation is a fact captured at a named vantage: payload bytes, observed
topic, delivery properties, source sequence, and capture-local time.

An assertion is supplied by a trusted adapter or manifest and records its
source and trust level. Examples include the claimed publisher role or broker
ingress identity.

An inference is derived from observations and assertions. Examples include
message correlation, application-level duplication, or a likely actor.

A recommendation identifies the next useful investigation. It is neither an
observation nor a normative verdict.

The canonical report keeps all four categories separate.

### 8.2 Applicability and verdict

Internal dimensions:

~~~text
applicability = APPLICABLE | NOT_APPLICABLE | UNKNOWN
verdict       = PASS | FAIL | INCONCLUSIVE | UNEVALUATED
~~~

Legal combinations:

| Applicability | Verdict | Meaning |
| --- | --- | --- |
| APPLICABLE | PASS | Preconditions and compliant behavior were adequately observed |
| APPLICABLE | FAIL | Preconditions and violating behavior were adequately observed |
| APPLICABLE | INCONCLUSIVE | Rule applies but evidence cannot decide behavior |
| NOT_APPLICABLE | UNEVALUATED | Preconditions are proved false |
| UNKNOWN | UNEVALUATED | Evidence cannot decide whether the rule applies |

Other combinations are invalid model states and fail internal validation.

The human output maps UNKNOWN plus UNEVALUATED to INCONCLUSIVE while preserving
both original values in JSON. Missing evidence never becomes PASS or
NOT_APPLICABLE.

### 8.3 Report validity

The report has a separate validity:

~~~text
VALID | PARTIAL | INVALID
~~~

A parse failure, resource-limit failure, or tool crash is not a protocol FAIL.

### 8.4 Diagnostic domains

~~~text
PROTOCOL_CONFORMANCE
TRACE_INTEGRITY
CAPTURE_QUALITY
MIGRATION
TOOL_RUNTIME
SECURITY
SAFETY_SIGNAL
~~~

Only PROTOCOL_CONFORMANCE requires an applicable VDA or selected integration
profile authority.

## 9. Trace and capture contract

### 9.1 Append-only lifecycle

A native trace is an append-only sequence:

1. CaptureOpened;
2. MessageObserved, CaptureGap, AdapterWarning, or ClockEpochChanged records;
3. CaptureClosed; and
4. a derived FinalManifest referencing the digests of all preceding records.

CaptureOpened is never edited to add an end time. CaptureClosed supplies the end
state. The final manifest is a new derived artifact, not an updated opening
record.

Imported logs receive a synthetic CaptureOpened and CaptureClosed pair. Every
synthetic or unknown field is marked as such.

### 9.2 Minimum message observation

~~~yaml
event_id: required
capture_id: required
capture_point: SUBSCRIBER_DELIVERY | BROKER_INGRESS | BROKER_EGRESS | PUBLISHER_ADAPTER | IMPORTED_UNKNOWN
source_sequence: required when the adapter can guarantee it
observed_topic: required
raw_payload: required or external bounded reference
raw_payload_digest: required
observed_wall_time: value or UNKNOWN
observed_monotonic_ns: value or UNKNOWN
clock_epoch: required
observed_delivery_qos: value or UNKNOWN
observed_dup: value or UNKNOWN
observed_retain: value or UNKNOWN
~~~

Publisher client ID, original DUP, original packet ID, original publish QoS, and
true publisher role are never fabricated by a subscriber import.

### 9.3 Monotonic clock contract

Every capture clock records:

- clock domain;
- monotonic origin description;
- resolution;
- wrap policy;
- clock epoch;
- mapping to wall clock, if available; and
- mapping uncertainty.

A capture-process restart creates a new epoch. Monotonic values in different
clock domains or epochs are never directly subtracted.

If an imported trace has wall-clock timestamps only, ordering may use a proved
source sequence, but deadline and duration claims are INCONCLUSIVE when clock
adjustment or uncertainty could change the result.

The Developer Preview evaluates one capture domain at a time. It does not attempt a general
multi-source partial-order merge. Explicit correlated events may be added in a
later offline release.

### 9.4 Gaps and completeness

Completeness is rule-scoped:

~~~text
CONFIRMED_FOR_RULE | PARTIAL | UNKNOWN
~~~

Every diagnostic contract declares required topics, start state, end condition,
observation window, capture vantage, and gap policy.

An absence-based conclusion is legal only when the selected vantage and
completeness evidence can prove the relevant absence. Ordinary passive QoS 0
observation usually cannot prove global non-publication.

## 10. Offline containment

The diagnose command has no network-capable resolver.

Protocol artifacts and schemas resolve only from a local content-addressed
store selected by digest. The resolver rejects:

- HTTP, HTTPS, MQTT, and other network URI schemes;
- file URI references;
- absolute paths;
- parent traversal;
- archive traversal;
- symlink escape;
- device files and named pipes; and
- references whose digest is not in the selected bundle manifest.

The Developer Preview accepts regular JSON and JSONL files, not archives. Archive
support requires a separate threat model and tests.

Network-access denial is tested at the process boundary in CI, not stated only
as documentation.

## 11. Protocol authority and bundle

### 11.1 Authority model

The published VDA PDF is the base normative authority for its version.
Released schemas supplement it within the scope assigned by the PDF. Tagged
source is an implementation aid.

An erratum overrides the base only when it is a formally published VDA artifact
that:

- identifies the affected VDA version;
- identifies the exact clause or artifact;
- states the replacement or correction;
- has an immutable or archived source and digest; and
- is valid for the analysis date or selected effective date.

A GitHub issue, maintainer comment, acknowledged ambiguity, proposed change, or
future milestone cannot change a normative verdict. It may appear as
non-normative context.

### 11.2 Bundle manifest

Each bundle records:

- VDA version;
- published PDF digest, publication date, retrieval date, and immutable source;
- Git tag and full commit SHA;
- every released schema digest;
- JSON Schema dialect and format-assertion setting;
- validator identity and version;
- formally applicable errata and their clause-scoped overlay records;
- non-normative advisories separately;
- evaluator API version;
- rule implementation digest; and
- selected profile digests.

The protocol bundle does not contain the current executable build identity.
The analysis report records the actual tool version, build digest, dependency
lock digest, bundle ID, and configuration digest.

Redistribution rights are reviewed before a VDA PDF, schema, or source excerpt
is committed. If rights are unclear, the repository contains a digest-verified
local import procedure rather than the artifact.

## 12. Diagnostic rule contract

Each rule declares:

- stable ID and version;
- diagnostic domain;
- applicable VDA versions;
- authority and obligation;
- normative subject;
- finding severity;
- expected protocol error type and level, if any;
- applicability preconditions;
- required capture vantage and topics;
- required starting state;
- antecedent observations;
- supported, contradicting, and missing evidence;
- time model and uncertainty tolerance;
- gap policy;
- PASS, FAIL, INCONCLUSIVE, NOT_APPLICABLE, and UNKNOWN-applicability behavior;
- next evidence to collect;
- allowed investigation targets; and
- fixtures for every meaningful outcome.

Project diagnostics and recommendations never masquerade as VDA SHALL
obligations.

## 13. Order-content equality

The same-order-update diagnosis uses a versioned comparator contract. It never
compares an entire re-sent JSON document as raw bytes and calls every difference
a semantic change.

The comparator reports three independent levels:

~~~text
BYTE_EQUAL
STRUCTURALLY_EQUAL
SEMANTICALLY_EQUAL | SEMANTICALLY_CHANGED | SEMANTICALLY_UNKNOWN
~~~

Rules:

- object key order does not affect structural equality;
- JSON number representations compare by exact mathematical value when that
  value is representable under the selected parser contract;
- array order remains significant;
- headerId and timestamp are excluded from the default semantic comparison
  because a re-publication may legitimately update them;
- manufacturer, serial number, orderId, and orderUpdateId remain significant;
- omitted optional fields and explicit values are not treated as equal merely
  because a schema contains a default annotation;
- differences in orderDescription or another non-control free-text field yield
  SEMANTICALLY_UNKNOWN unless the selected, versioned profile explicitly
  classifies that field;
- differences in unknown extension fields yield SEMANTICALLY_UNKNOWN unless an
  explicit profile defines their semantics; and
- every report records the comparator ID, version, configuration digest, and
  excluded fields.

A normative identical-content or changed-content verdict is prohibited when the
comparator returns SEMANTICALLY_UNKNOWN.

## 14. First diagnostic catalog

### D1. Repeated order and update ID

Questions:

- Was content semantically identical, changed, or unknown?
- Did the mobile robot keep and continue the previous order?
- Was the required SAME_ORDER_UPDATE_ID response observed when applicable?

The fleet-control sender-hygiene warning is a project diagnostic unless an
applicable normative source assigns that obligation. Mobile-robot response
rules use the exact source clause for the selected version.

### D2. Base, horizon, and stitching

Questions:

- Is graph continuity valid at the stitching boundary?
- Did released nodes or edges change illegally?
- Is the next base update compatible with the previously accepted state?

The diagnosis separates malformed graph structure from an unobservable robot
acceptance state.

### D3. Unhandled newBaseRequest

Questions:

- Was newBaseRequest observed as true?
- Did it remain asserted across later state messages?
- Was a responsive order update observable from the selected vantage?

Lack of an observed fleet-control response is INCONCLUSIVE when the capture
cannot prove that publication or delivery would have been observed.

### D4. Reconnect and connection state

Questions:

- Was a disconnection or new connection epoch observed?
- Was the connection topic observed at the required vantage?
- Did the reported state return to the version-required value?

Wall-clock-only timing cannot prove a deadline whose boundary overlaps clock
uncertainty.

### D5. Cancel and action lifecycle

Questions:

- Was cancellation requested and observable?
- Were order, node, edge, and action states transitioned consistently?
- Did a callback or state projection omit an observable transition?

The tool does not claim that cancelOrder physically stopped a robot.

### Public-alpha expansion candidates

After the vertical slice is useful on real traces, expand to:

- wait at the last released node;
- released-base mutation;
- node and edge progress stalls;
- action blocking-type and lifecycle consistency;
- order completion and failure inference;
- pause and resume;
- fleet-control restart and resynchronization;
- factsheet capability mismatch;
- map and position-initialization consistency; and
- duplicate, missing, delayed, and reordered application messages.

Frequency in the incident corpus, actionability, and source clarity determine
the order. The plan does not require all candidates.

## 15. VDA 5050 2.1-to-3.0 migration diagnosis

Migration is an explicit product feature, not an implicit version flag.
The Phase 1 CLI therefore rejects `--vda-version 2.1.0`; it must not apply the
3.0.0 catalog to a 2.1 trace. Support begins only after a separately pinned 2.1
source bundle, comparator profile, rule catalog, and cross-version fixtures are
implemented and reviewed.

For each relevant observation, the migration report classifies:

~~~text
UNCHANGED
RENAMED_OR_RESHAPED
REMOVED
ADDED
SEMANTICALLY_CHANGED
PROFILE_DEPENDENT
UNKNOWN
~~~

It distinguishes:

- syntax incompatible with the selected version;
- a known field or enum migration;
- a behavioral obligation that changed;
- a participant that appears to emit one version under another version label;
- a mixed-version boundary requiring an adapter; and
- insufficient evidence.

The planned public alpha may recommend migration work but will not generate or deploy an adapter.
Each migration rule cites both version sources and has 2.1, 3.0, mixed, and
unknown fixtures.

## 16. Small initial architecture

The implemented Rust workspace begins with only offline components:

~~~text
vda5050-lab/
├── Cargo.toml
├── Cargo.lock
├── rust-toolchain.toml
├── apps/
│   └── vda5050-doctor/
├── crates/
│   ├── vda5050-core/
│   ├── vda5050-local-fs/
│   ├── vda5050-import/
│   ├── vda5050-protocol/
│   ├── vda5050-doctor/
│   └── vda5050-report/
├── bundles/
│   └── manifests/
├── rules/
├── fixtures/
│   └── synthetic/
├── docs/
└── .github/workflows/ci.yml
~~~

Responsibilities:

- core: identifiers, observations, assertions, applicability, verdicts, and
  serialization;
- local-fs: descriptor-bound, no-follow local file and trusted-directory
  access shared by trace import and protocol bundles;
- import: bounded local formats, capture lifecycle, parse findings, and gaps;
- protocol: verified local bundles, authorities, schemas, and comparators;
- doctor: incident rules, state reconstruction, missing-evidence analysis, and
  next actions;
- report: canonical JSON and safe terminal projection; and
- CLI: configuration, orchestration, exit policy, and human workflow.

No mqtt, scenario, arm, bridge, viewer, or pseudonymization crate exists in the
initial workspace.

Crates may be merged if the first vertical slice shows the separation is
ceremony. A merge must preserve the tested contracts.

## 17. Phases and release gates

### Phase 0: evidence and contract setup

This phase may run in parallel with a disposable importer spike. It does not
require completing a general architecture.

Deliver:

- at least three candidate design partners spanning, where possible, robot,
  fleet-control, and integration perspectives;
- a trace-handling agreement and intake template;
- a first incident inventory;
- the canonical trace envelope;
- the five diagnostic contracts;
- the legal applicability/verdict state machine;
- minimal source bundles for only the clauses used by those diagnoses; and
- a synthetic example for each diagnosis and each evidence-shortage path.

Gate:

- at least one partner supplies an incident format that the importer design can
  represent without discarding records;
- every rule states what forces INCONCLUSIVE;
- the repository still contains no customer trace; and
- the implementation slice remains offline.

Failure to recruit three organizations does not prevent a disposable prototype.
It does prevent the public-alpha evidence claim.

### Phase 1: five-diagnosis vertical slice

Write tests first for:

- lossless accepted-record import;
- rejected-record accounting;
- CaptureOpened and CaptureClosed lifecycle;
- unknown applicability and illegal result states;
- wall-clock-only timing ambiguity;
- order semantic comparison;
- the five diagnostic outcome matrices;
- terminal and JSON injection;
- input size and nesting limits; and
- offline resolver containment.

Then implement:

- one canonical JSONL importer;
- one message-envelope adapter;
- minimal verified source bundles;
- five diagnostics;
- terminal and canonical JSON reports; and
- reproducible synthetic fixtures.

Gate:

- a clean checkout builds on macOS and Linux;
- the process performs no network access;
- every accepted raw record remains addressable by event ID, offset, and digest;
- every rejected record appears in the import summary;
- every diagnostic has supported, inconclusive, not-applicable, and
  unknown-applicability coverage where meaningful;
- no output assigns a participant when its role cannot be proved;
- core line and region coverage are each at least 80%, with branch coverage
  required when the selected LLVM toolchain exposes measurable Rust branch
  sites; and
- all rule evaluators link to 100% of their required fixtures.

Current local result on 2026-08-07:

- 86 locked workspace tests pass;
- strict workspace Clippy passes with warnings denied;
- LLVM workspace coverage is 86.66% lines, 86.93% regions, and 87.57%
  functions; CI and release verification fail below 80% lines or regions;
- the selected Rust/Xcode LLVM pipeline reports zero measurable branch sites,
  so no branch percentage is claimed;
- RustSec scanned 89 locked crate dependencies against 1,190 advisories and found
  no known vulnerability;
- the synthetic CLI examples pass on macOS;
- Linux CI and the multi-platform tag workflow are configured; their remote
  outcome is reported separately from local verification;
- runtime protocol-bundle loading is not connected to `diagnose`; and
- no real-incident pilot evidence exists.

The last two items prevent a public-alpha claim even though the local vertical
slice is operational.

### Phase 2: private incident pilot

Use the protocol in [VALIDATION_PROTOCOL.md](VALIDATION_PROTOCOL.md).

Deliver:

- twenty real incidents from at least three organizations;
- development and organization- or system-level holdout sets;
- independent expert labels;
- a manual investigation baseline;
- per-case tool reports; and
- a pilot evidence report.

Gate:

- actionable next-step coverage is at least 70%;
- unsupported participant attribution is zero in the observed pilot;
- abstention is judged appropriate when evidence is insufficient;
- median time to an accepted next action is at most 50% of baseline; and
- at least three teams state they would use it on another integration.

Failure means revise or stop. It does not automatically trigger more platform
implementation.

### Phase 3: offline public alpha

Add only diagnoses justified by pilot frequency and actionability, up to an
initial total of ten to fifteen.

Deliver:

- explicit VDA 5050 2.1-to-3.0 migration diagnostics;
- minimized synthetic or independently authorized public incident cases;
- stable rule IDs and report schema;
- contribution templates;
- advisory audit, SBOM, checksums, and build provenance; and
- documented limitations and pilot methodology.

Gate:

- no raw or reversible customer data is published;
- the holdout results remain above the pilot gate;
- hostile input and output tests pass;
- exact source and build provenance is present;
- installation works from a clean checkout; and
- the release makes no certification or physical-root-cause claim.

### Phase 4: expansion decision

After public-alpha usage evidence, choose one next investment:

- additional offline diagnoses;
- better import adapters;
- a local evidence viewer;
- CI report formats;
- privacy-preserving case export; or
- isolated active testing.

The choice requires usage evidence and a new approved plan. It is not inherited
from revision 0.5.

## 18. Validation strategy

Fixtures prove implementation behavior, not product usefulness.

Real-incident validation is defined in
[VALIDATION_PROTOCOL.md](VALIDATION_PROTOCOL.md). Its core safeguards are:

- development and holdout cases are separated by organization or system;
- tool output does not influence the initial ground-truth label;
- two experts independently score ambiguous cases;
- confirmed cause, accepted remediation, expert consensus, and unresolved
  outcome are distinct;
- actionable coverage and unsupported attribution are measured together;
- time comparison uses the same trace and task boundary; and
- public reports aggregate results without exposing customer traces.

Zero unsupported attributions in twenty cases is a pilot observation, not proof
of a low population error rate. With zero events in twenty independent cases,
the one-sided 95% upper bound is approximately 13.9%. Roughly fifty-nine
independent zero-event cases are needed before the corresponding upper bound is
below 5%.

## 19. Test strategy

Tests are written before each production slice.

### 19.1 Unit tests

- legal and illegal applicability/verdict states;
- observation, assertion, inference, and recommendation separation;
- capture lifecycle and final-manifest derivation;
- monotonic clock epochs and unknown time;
- bundle and errata precedence;
- order equality at all three levels;
- role attribution and unresolved fallback;
- migration classifications;
- serializers and output limits; and
- deterministic next-action generation.

### 19.2 Property and fuzz tests

- accepted serialization preserves raw bytes and digest;
- random malformed JSON never panics;
- duplicate JSON keys follow an explicit policy;
- nesting, payload, event, and string limits fail predictably;
- semantic comparison is invariant to object-key ordering;
- excluded re-publication header changes do not alter semantic equality;
- unknown extension differences cannot become a definitive equality verdict;
- terminal control and hostile Unicode never execute or corrupt framing; and
- offline results do not depend on current wall clock or thread scheduling.

### 19.3 Rule fixture matrix

Each diagnostic has, where meaningful:

- supported PASS or compliant behavior;
- supported FAIL or incident behavior;
- missing-event INCONCLUSIVE;
- mid-stream-start INCONCLUSIVE;
- capture-gap INCONCLUSIVE;
- clock-ambiguity INCONCLUSIVE;
- NOT_APPLICABLE;
- UNKNOWN applicability;
- unresolved actor;
- VDA 2.1 boundary;
- VDA 3.0 boundary; and
- source or profile conflict.

Golden-output changes require semantic review. Regeneration alone is not a
justification.

### 19.4 Integration and security tests

- CLI from local input through terminal and JSON;
- bundle verification and tamper rejection;
- network-denied process execution;
- network and file URI rejection;
- absolute path, traversal, and symlink escape rejection;
- regular-file and resource-limit enforcement;
- ANSI, control-character, bidirectional-text, and oversized-value handling;
- cancellation without corrupting the final report; and
- clean-checkout execution on macOS and Linux.

### 19.5 Coverage gates

- core crates: at least 80% line and branch coverage;
- diagnostic evaluators: 100% linked to required outcome fixtures;
- every public CLI subcommand: at least one end-to-end test; and
- unsafe Rust denied by default.

Coverage does not replace incident-pilot evidence.

## 20. Security and privacy

- Offline commands have no network code path.
- Traces, rules, schemas, profiles, and reports are untrusted input.
- YAML, if later used, is data only and cannot execute templates or commands.
- Raw payload rendering is bounded and escaped.
- Secrets never enter reports.
- Telemetry and automatic upload do not exist.
- Customer traces are stored outside the repository.
- A case cannot enter the public corpus without explicit case-level approval.
- Public fixtures are minimized synthetic reproductions by default.
- A digest of a private source trace is not included in a public fixture because
  it can enable confirmation by a party that possesses the original.

Pseudonymization is not a Developer Preview feature. If later approved, it must use a
high-entropy per-case HMAC key kept outside the export, define rotation and
destruction, fail closed for unknown fields and malformed payloads, and publish
neither the key nor an unkeyed source digest.

## 21. Dependency and release supply chain

Before the first dependency is accepted, record:

- exact version and source;
- license;
- maintenance and security status;
- network or telemetry behavior;
- replacement strategy; and
- whether it preserves unknown fields and raw payloads.

Repository policy:

- Cargo.lock is committed for binaries;
- the Rust toolchain is pinned;
- advisory and license checks run in CI;
- GitHub Actions references use full commit SHAs;
- workflow permissions default to read-only and jobs elevate only the minimum
  required permission;
- pull-request workflows do not expose release credentials;
- releases include checksums, an SBOM, build provenance, source commit, lockfile
  digest, bundle ID, and rule digest; and
- release signing or provenance credentials require a separate release
  approval.

## 22. Performance and usability

The pilot corpus determines final numeric performance thresholds. Before it
exists, synthetic throughput is not allowed to substitute for user value.

The Phase 1 benchmark records:

- event count and payload distribution;
- host and toolchain identity;
- maximum RSS;
- events per second;
- time to first incident candidate;
- total analysis time;
- report size; and
- cancellation latency.

The architecture streams import and bounded diagnostic state where semantics
permit. Any rule retaining history declares its bound.

Product success is measured primarily by time to an accepted next action, not
raw events per second.

## 23. Planned files

### Current documentation revision

- README.md
- docs/IMPLEMENTATION_PLAN.md
- docs/VALIDATION_PROTOCOL.md
- docs/FUTURE_ACTIVE_TESTING.md

### Phase 0

- docs/TRACE_FORMAT.md
- docs/RULE_CONTRACT.md
- docs/PROTOCOL_SOURCES.md
- docs/adr/0001-result-state-machine.md
- docs/adr/0002-order-semantic-equality.md

### Phase 1

- Rust workspace and lockfile;
- offline crates and CLI;
- five diagnostic contracts;
- minimal verified bundle manifests;
- synthetic, malformed, and golden fixtures;
- integration and security tests; and
- pinned CI workflows.

No active MQTT, Web, or physical-test file is planned before the Phase 4
expansion decision.

## 24. Risks and stop conditions

### Risk: the tool avoids blame by always abstaining

Mitigation: measure actionable coverage and correct abstention alongside
unsupported attribution.

Stop condition: coverage remains below 70% after the five highest-frequency
diagnoses are implemented and reviewed.

### Risk: twenty traces overfit the rules

Mitigation: hold out an organization or system, freeze rule versions before
evaluation, and report development and holdout results separately.

Stop condition: holdout performance does not meet the gate.

### Risk: ground truth is weak

Mitigation: separate confirmed cause, confirmed remediation, expert consensus,
and unresolved cases; require two independent reviews for ambiguity.

Stop condition: most cases cannot support an accepted next action or reliable
evidence score.

### Risk: private traces leak

Mitigation: keep them outside Git, minimize access, publish synthetic cases, and
require explicit case-level release approval.

Stop condition: a trace cannot be safely processed under the partner agreement.

### Risk: protocol ambiguity becomes a fake rule

Mitigation: formal source precedence, non-normative advisory separation,
UNKNOWN applicability, and INCONCLUSIVE output.

Stop condition: the rule cannot state a defensible authority and evidence
contract.

### Risk: viewer or active scope returns too early

Mitigation: no related crate or release gate exists in the Developer Preview architecture.

Stop condition: requested work does not improve validated offline incident
outcomes and has no separately approved plan.

## 25. Change control and rollback

- The release candidate is committed as one initial, auditable repository
  baseline because no earlier commit exists.
- Later changes use small reviewable commits and pull requests.
- Released rule IDs and bundle IDs are immutable.
- Corrected rules receive a new version and migration note.
- Customer traces never become rollback artifacts in Git.
- Generated reports and indexes are disposable; source evidence remains
  separately controlled.
- Public, networked, active, physical, or release actions always require their
  own approval.

## 26. Execution checkpoints

1. Completed: review and fundamentally reorganize the platform-first plan.
2. Completed: approve Phase 1 implementation without authorizing public or
   hardware operations.
3. Completed locally: implement the offline five-diagnosis vertical slice with
   test-first contracts and synthetic fixtures.
4. Completed locally: review code, source provenance, security tests, release
   workflow, and proof boundaries.
5. Current: publish and remotely verify the v0.1.2 Developer Preview after the
   immutable v0.1.0 and v0.1.1 tag release gates stopped before asset
   publication.
6. Pending: recruit design partners and finalize trace intake.
7. Pending: run the private incident pilot under the approved validation
   protocol.
8. Pending: decide whether the evidence justifies a public alpha.
9. Pending: choose one post-alpha expansion through a new plan.

Each checkpoint reports changed files, tests, exact results, unresolved
assumptions, customer-data boundaries, and which claims remain unverified.

## 27. Approval boundary

The user's 2026-08-06 release instruction authorizes completing the local
release candidate, creating the public GitHub repository, committing and
pushing the reviewed baseline, creating annotated tag `v0.1.0`, preserving it
after its release gate stopped, preserving the corrective `v0.1.1` tag after
its release gate also stopped, and publishing the annotated `v0.1.2` Developer
Preview with release artifacts and attestations.
It does not authorize customer outreach, customer trace transfer, broker
connection, message publication, external operational-system access,
simulation, fault injection, or hardware operation.

Phase 2 still requires explicit trace-handling approval and design-partner
coordination. Public alpha, active MQTT, and physical testing retain separate
approval gates.
