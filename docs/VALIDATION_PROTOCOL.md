# vda5050-doctor Incident Validation Protocol

- Status: Proposed
- Protocol version: 0.1
- Research snapshot: 2026-08-06
- Applies to: Private pilot and public-alpha evidence
- Minimum planned corpus: 20 real incidents from at least 3 organizations

## 1. Purpose

This protocol determines whether vda5050-doctor helps a commissioning engineer
reach a defensible next action faster than manual trace investigation.

It does not test whether the software merely:

- parses JSON;
- passes synthetic fixtures;
- finds a known pattern in the same trace used to author a rule; or
- produces a plausible-looking explanation.

The study evaluates evidence quality, actionability, abstention, attribution,
and investigation time on real incidents.

## 2. Primary questions

The pilot answers:

1. Can the tool produce an expert-accepted next action for at least 70% of
   incidents?
2. Does it avoid unsupported attribution to a mobile robot or fleet-control
   participant?
3. Does it abstain for the correct reason when the capture is insufficient?
4. Does it reduce median time to an accepted next action by at least half?
5. Do at least three participating teams want to use it again?

Secondary questions:

- Which incident families create the most value?
- Which missing observations prevent diagnosis most often?
- Which import formats deserve maintained adapters?
- How often is a 2.1-to-3.0 mismatch relevant?
- Are source citations and rule explanations understandable?

## 3. Definitions

### 3.1 Incident

A bounded operational or commissioning episode with:

- an observed unwanted outcome;
- a time or sequence window;
- available trace evidence;
- a human investigation question; and
- a known resolution state.

Twenty extracts from one underlying failure count as one incident.

### 3.2 Actionable next step

A concrete action that an experienced engineer judges likely to reduce
uncertainty or resolve the incident. Examples include:

- capture a named topic from a specified vantage;
- compare the order content under a named comparator;
- inspect a particular participant state transition;
- reproduce with a specified version boundary; or
- correct a supported protocol behavior.

“Check the logs,” “investigate the robot,” and unsupported vendor blame are not
actionable.

### 3.3 Unsupported participant attribution

Any output that identifies MOBILE_ROBOT or FLEET_CONTROL as the normative
violator or responsible investigation target when the supplied evidence and
authority do not support that role-specific conclusion.

TRANSPORT may be selected only when the evidence supports a transport or
capture-path problem. Otherwise the required value is UNRESOLVED.

### 3.4 Appropriate abstention

An INCONCLUSIVE or UNKNOWN-applicability result whose stated missing evidence
matches the expert review and whose next collection action is useful.

An unexplained abstention is not counted as correct.

### 3.5 Accepted next action

An action accepted independently by two qualified reviewers, or by one reviewer
and the incident owner when a second independent reviewer is unavailable.

The report identifies which standard was used.

## 4. Partner and corpus composition

Recruit at least three organizations. Where possible, include:

- one mobile-robot implementation perspective;
- one fleet-control implementation perspective; and
- one system-integrator or commissioning perspective.

The three organizations must not all supply incidents from the same software
stack.

The twenty-incident minimum should include:

- VDA 5050 2.1 incidents;
- VDA 5050 3.0 incidents;
- at least one mixed-version boundary;
- incomplete captures;
- incidents with confirmed causes;
- incidents whose final cause remained unresolved; and
- at least three of the initial five diagnostic families.

No individual organization should contribute more than 60% of the pilot
corpus. If that is unavoidable, results are reported as an imbalanced pilot and
cannot satisfy the public-alpha evidence gate without additional data.

## 5. Data agreement and authority

Before receiving a trace, record:

- organization and authorized contact;
- ownership or authority to share;
- permitted storage location;
- permitted processors and reviewers;
- retention and deletion date;
- whether derived rules may be published;
- whether minimized synthetic cases may be published;
- whether direct excerpts may be shown privately;
- incident identifiers that must be removed;
- facility-geometry handling;
- disclosure and incident-response contacts; and
- whether the trace may be used only for product development or also for
  evaluation.

No assumption of open-source publication follows from permission to analyze a
trace.

Raw traces stay outside the Git repository. Access is minimum necessary and
logged where the storage system supports it.

## 6. Incident intake record

Each incident receives an append-only intake record:

~~~yaml
case_id: pseudonymous internal ID
organization_id: pseudonymous internal ID
system_family_id: pseudonymous internal ID
received_at: timestamp
usage_class: DEVELOPMENT | HOLDOUT
vda_versions_claimed: []
participants:
  mobile_robot: known metadata or UNKNOWN
  fleet_control: known metadata or UNKNOWN
capture:
  formats: []
  vantages: []
  topics_claimed: []
  start_condition: known value or UNKNOWN
  end_condition: known value or UNKNOWN
  source_sequence: description or UNKNOWN
  monotonic_clock: description or UNKNOWN
  wall_clock: description or UNKNOWN
  known_gaps: []
incident_question: bounded text
resolution_state: CONFIRMED_CAUSE | CONFIRMED_REMEDIATION | EXPERT_CONSENSUS | UNRESOLVED
publication_permission: NONE | SYNTHETIC_DERIVATIVE | MINIMIZED_EXCERPT
retention_deadline: timestamp
~~~

The intake process does not silently normalize away invalid records. Rejected
or truncated data remains part of capture-quality ground truth.

## 7. Ground-truth levels

Cases use one of four levels:

### CONFIRMED_CAUSE

The cause was reproduced, directly observed, or verified by a corrective change
that isolates the causal mechanism.

### CONFIRMED_REMEDIATION

A remediation fixed the incident, but the available evidence does not uniquely
prove the deeper cause.

### EXPERT_CONSENSUS

Two qualified reviewers agree on the best explanation and next action, but no
independent reproduction or isolating remediation exists.

### UNRESOLVED

The incident cannot support a cause claim. It may still support correct
abstention and a useful collection recommendation.

Reports never merge these levels into a single “ground truth” number.

## 8. Label creation

### 8.1 Independent first pass

Before seeing vda5050-doctor output, reviewers record:

- incident summary;
- supported observations;
- suspected protocol behavior;
- applicable VDA version and source;
- normative subject, if provable;
- missing evidence;
- next action;
- confidence; and
- resolution level.

### 8.2 Disagreement

Reviewers compare labels only after independent submission.

Disagreement is resolved by:

1. identifying whether the dispute is factual, normative, or interpretive;
2. checking the pinned source artifacts;
3. requesting additional incident-owner evidence when allowed;
4. recording both original labels; and
5. choosing consensus or UNRESOLVED.

An unresolved specification ambiguity cannot be converted into a definitive
normative rule.

### 8.3 Qualification

A qualified reviewer has direct experience implementing, integrating, or
debugging VDA 5050 or a comparable robot-fleet protocol. Each report states the
reviewer roles and conflicts of interest without publishing personal identities
when confidentiality prohibits it.

## 9. Development and holdout split

Rules may be designed and tuned using development cases.

At least five incidents are held out. The holdout must include:

- at least one organization or system family not represented in the development
  cases, when the available corpus permits;
- at least two incident families; and
- at least one incomplete or unresolved capture.

Before holdout execution, freeze:

- executable build digest;
- protocol bundle IDs;
- rule implementation digest;
- comparator versions;
- configuration;
- scoring implementation; and
- case eligibility list.

A rule change after viewing a holdout result invalidates that holdout run for
the release decision. The case may enter a later development set, but a new
unseen holdout is required.

Development and holdout metrics are always reported separately.

## 10. Manual baseline

### 10.1 Task boundary

The human baseline and tool-assisted run receive:

- the same trace files;
- the same incident question;
- the same known version and system metadata;
- the same time boundary; and
- the same permission to request unavailable evidence.

The human baseline does not receive tool-generated hints.

### 10.2 Timing

Record:

- time to first hypothesis;
- time to first actionable next step;
- time to accepted next step;
- time spent importing or reformatting;
- time spent looking up specification sources; and
- whether the participant timed out.

The primary time metric is time to accepted next step.

### 10.3 Study design

Use different qualified participants for manual and tool-assisted review when
possible. If the same participant must review both, counterbalance the order
across cases and apply a washout interval. Cases seen during rule development
cannot be used for that developer's unbiased time comparison.

## 11. Tool-assisted run

For every case, preserve:

- input digests;
- import summary;
- capture-quality findings;
- executable and dependency-lock digests;
- protocol bundle and rule digests;
- complete canonical JSON output;
- terminal output;
- start and completion times;
- user actions needed before diagnosis;
- tool errors or crashes; and
- the participant's final selected next action.

A human may navigate evidence and source links. A human may not edit the
canonical tool output before scoring.

## 12. Scoring

### 12.1 Per-incident rubric

Each dimension is scored independently:

| Dimension | Values |
| --- | --- |
| Incident detection | CORRECT, PARTIAL, INCORRECT, ABSTAINED |
| Evidence selection | CORRECT, PARTIAL, INCORRECT |
| Source citation | CORRECT, WRONG_CLAUSE, WRONG_VERSION, MISSING, NOT_REQUIRED |
| Normative subject | CORRECT, UNSUPPORTED, UNRESOLVED_CORRECTLY, NOT_APPLICABLE |
| Missing-evidence explanation | CORRECT, PARTIAL, INCORRECT, NOT_REQUIRED |
| Next action | ACCEPTED, PARTIAL, REJECTED, NONE |
| Abstention | APPROPRIATE, INAPPROPRIATE, NOT_APPLICABLE |
| Migration classification | CORRECT, PARTIAL, INCORRECT, NOT_APPLICABLE |

Evidence correctness is based on exact event IDs and raw-record reachability,
not just a plausible prose summary.

### 12.2 Actionable coverage

~~~text
actionable coverage =
  incidents with ACCEPTED next action
  divided by eligible incidents
~~~

Eligibility includes unresolved cases when a specific evidence-collection
action could be useful.

### 12.3 Unsupported attribution rate

~~~text
unsupported attribution rate =
  incidents with one or more unsupported role-specific attributions
  divided by all evaluated incidents
~~~

The primary pilot gate requires zero observed incidents, not merely a low
average number of findings.

### 12.4 Appropriate abstention rate

~~~text
appropriate abstention rate =
  appropriate abstentions
  divided by all expert-labeled insufficient-evidence opportunities
~~~

Also report false abstentions: cases where the supplied evidence was sufficient
for an accepted next action but the tool returned only an unexplained
INCONCLUSIVE result.

### 12.5 Time ratio

~~~text
time ratio =
  median tool-assisted time to accepted next action
  divided by median manual time to accepted next action
~~~

Report paired per-case results where possible. Do not substitute runtime
benchmarks for investigation time.

## 13. Release gates

### Discovery gate

- At least three candidate organizations interviewed.
- At least five incident descriptions collected.
- At least one real trace format is representable without silent loss.
- Initial diagnostic priority is supported by incident frequency or severity.

### Vertical-slice gate

- Five diagnoses run end to end.
- Every unsupported input record is reported.
- Experts accept at least one tool-recommended next action on a real development
  case.
- Incomplete evidence produces an actionable INCONCLUSIVE result.
- No customer data is committed.

### Private-pilot gate

- At least twenty eligible incidents.
- At least three organizations.
- At least five holdout incidents.
- At least 70% actionable coverage.
- Zero observed unsupported participant attribution.
- Appropriate abstention reviewed for every insufficient-evidence case.
- Median time ratio no greater than 0.50.
- At least three teams answer yes to reuse intent.

### Public-alpha gate

- Private-pilot gate passes on the frozen holdout.
- Public fixtures are synthetic or individually authorized.
- The public report states corpus composition and limitations.
- No claim extrapolates beyond the evaluated versions, incident families, and
  capture vantages.

## 14. Statistical interpretation

Twenty incidents are a product-discovery sample, not a certification sample.

If zero unsupported attributions are observed in twenty independent incidents,
the exact one-sided 95% upper confidence bound is approximately 13.9%. A simple
rule-of-three approximation gives 15%.

Approximately fifty-nine independent zero-event incidents are needed for the
same exact upper bound to fall below 5%.

These calculations assume independent, representative cases. Correlated traces
from the same stack reduce effective evidence. Therefore:

- report organization and system clustering;
- do not present 0 of 20 as a proven error rate below 5%;
- continue post-alpha monitoring;
- publish confidence intervals with rates; and
- never describe this pilot as certification.

The small sample also makes a binary statistical significance decision for
investigation time unreliable. Report paired distributions, medians, case-level
differences, and confidence intervals where appropriate.

## 15. Reuse-intent question

After the scored exercise, ask:

> If this tool supported the trace formats and incident families you encounter,
> would your team use it during the next VDA 5050 commissioning or support
> incident?

Allowed responses:

- yes;
- maybe, with the named missing requirement; or
- no, with the main reason.

Only yes counts toward the three-team gate. Multiple respondents from one team
count as one team.

## 16. Privacy and publication

The internal pilot report may contain pseudonymous case IDs but no unnecessary
participant identifiers.

The public report contains:

- aggregate corpus composition;
- incident-family counts;
- version counts;
- development and holdout metrics;
- confidence intervals and limitations;
- examples built from synthetic or authorized minimized cases; and
- known unsupported formats and diagnoses.

It does not contain:

- raw customer traces;
- customer or vendor identity without explicit approval;
- facility geometry;
- hostnames, IPs, credentials, serial numbers, or order identifiers;
- free-text payloads;
- an unkeyed digest of a private source trace; or
- reversible mappings.

Deletion is verified at the agreed retention deadline. Derived rules are
reviewed to ensure they do not contain identifying constants or payload text.

## 17. Failure and stop rules

Pause the pilot when:

- trace authority or consent is disputed;
- a possible secret or personal-data exposure is found;
- a scoring conflict reveals that the rule source is wrong;
- a tool output makes an unsupported vendor attribution;
- holdout cases leak into rule development;
- an importer silently drops records; or
- the study task differs materially between manual and tool-assisted arms.

After correction, affected cases are rerun only under a documented new study
revision. Original results remain in the audit record.

Stop or pivot the product when:

- fewer than three organizations will share usable evidence;
- actionable coverage remains below 70% after the highest-value diagnoses;
- time to accepted action is not materially improved;
- correct abstention requires capture metadata users cannot realistically
  obtain; or
- teams prefer existing inspection tools and do not value evidence-bounded
  recommendations.

## 18. Pilot evidence report

The final report includes:

1. protocol revision and dates;
2. partner and corpus composition;
3. development and holdout split;
4. ground-truth level distribution;
5. tool and source provenance;
6. every primary and secondary metric;
7. per-family results;
8. false attribution and false abstention review;
9. investigation-time distributions;
10. excluded cases and reasons;
11. privacy and deletion status;
12. deviations from this protocol;
13. statistical limitations;
14. reuse-intent results; and
15. ship, revise, pivot, or stop recommendation.

The raw evidence report remains private unless every included case permits its
release. A public summary is a separate artifact.

## 19. Approval boundary

This document does not authorize contacting organizations, receiving customer
data, executing a pilot, publishing results, or changing retention policies.

Every trace transfer requires its own agreement. The pilot begins only after
the implementation plan and this validation protocol are approved.
