# Official LSMART integration inputs

This directory does not vendor or redistribute LSMART. The local demo requires
an external checkout of `https://github.com/smart-mapf/lifelong-smart.git` at
the exact commit recorded in `provenance.json`.

The causal overlay makes two narrow changes at the official execution boundary:

1. the ADG RPC serializes the action's documented `task_id` field instead of
   the unused `task_ptr`, so station completion identity is not lost;
2. an action batch obtained from the official LSMART ADG is written to the
   bridge outbox;
3. it is not inserted into the controller action queue until the matching VDA
   5050 order returns through the isolated MQTT broker; and
4. the actual ARGoS pose is projected to a bounded telemetry file for VDA state
   and visualization publication.

The official planner, task assigner, invocation policy, fail policy, ADG, PID
controller and ARGoS physics remain authoritative. The one-line RPC correction
does not change planning, dependencies, queueing, motion, or task accounting.
The integration does not claim to be an upstream LSMART feature.

The second patch only includes spdlog's ostream formatter shim. It is required
to compile the upstream `CellType` logging calls with Ubuntu 22.04's fmt 8 and
does not change planner or simulator behavior.

The upstream repository currently calls LSMART open source but has no root
license file at the pinned commit. This repository therefore stores only the
integration patch and provenance. Do not publish an LSMART-derived source
archive or container image until upstream licensing is clarified.
