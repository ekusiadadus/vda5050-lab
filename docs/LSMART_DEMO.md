# Official LSMART warehouse demo

This optional Tier 1 demo runs the official LSMART planner/controller stack
and ARGoS simulator, sends its action batches causally through VDA 5050 and an
isolated Mosquitto broker, records the broker exchange, and runs Doctor
offline.

## Prerequisites

- Docker Desktop with amd64 emulation enabled (required on Apple Silicon)
- Git, `jq`, `curl`, `rg`, and `shasum`
- a clean external checkout of the official LSMART repository at the pinned
  commit

```sh
git clone https://github.com/smart-mapf/lifelong-smart.git ../lifelong-smart
git -C ../lifelong-smart checkout ce0a020d8da10a806b23ffa3ccb56b1affff57d1
make lsmart-demo
```

Set `VDA5050_LSMART_SOURCE` when the checkout is elsewhere. The first build
compiles ARGoS beta59 and LSMART for `linux/amd64`, so it is substantially
slower than later cached runs.

Useful options:

```sh
VDA5050_LSMART_HEADLESS=1 make lsmart-demo
VDA5050_LSMART_WEB_PORT=18080 make lsmart-demo
VDA5050_LSMART_ARTIFACT_DIR=/absolute/empty/path make lsmart-demo
VDA5050_LSMART_INJECT_FAULT=false make lsmart-demo
```

## What is actually running

```text
official RHCR/PBS + task assigner + ADG
                  │ action batch
                  ▼
          VDA 5050 Fleet bridge ──order──▶ isolated Mosquitto
                                                │
                                                ▼
                                      per-robot MQTT adapter
                                                │ exact return validated
                                                ▼
                               official controller queue + ARGoS
                                                │ state / visualization
                                                ▼
                                  broker recorder ──▶ offline Doctor
```

An integration overlay preserves the ADG action's documented task ID at the
existing RPC boundary and holds every new batch before the controller queue.
It cannot enter the official action queue until the matching VDA order has
returned through MQTT. Planning and ADG dependencies are unchanged. The runner
proves at the end that every outbox batch has one inbox delivery, order and
delivery counts match, all ten ARGoS robot poses were observed, and LSMART
finished at least one warehouse task.

The default scenario reconnects robot `0` without publishing `ONLINE`. Doctor
analyzes the same sealed capture twice: the passive observation must remain
`INCONCLUSIVE / UNRESOLVED`, while the trace-digest-bound same-job evidence can
support the synthetic `FAIL / MOBILE_ROBOT` result.

## Containment and provenance

- Mosquitto has no host port and runs only on a Docker `internal` network.
- Official LSMART has `network_mode: none`; its server, planner, controller,
  and ARGoS processes communicate over container loopback and bounded files.
- Doctor has `network_mode: none` and receives a read-only artifact mount.
- The Web gateway is artifact-only and binds only to `127.0.0.1`.
- The source URL, LSMART commit, ARGoS tag/commit, overlay, fixed seed, map,
  robot count, ticks, and broker endpoint are pinned.

The pinned LSMART revision has no root license file. For that reason this
repository does not vendor its source and the derived local image must not be
published or redistributed until upstream licensing is clarified. The patch
is an integration maintained by this project, not an upstream feature.

## Claim boundary

This is a synthetic same-job demonstration of a VDA communication and
diagnostic path. It does not prove VDA conformance, vendor interoperability,
planner performance, physical dynamics, localization, wireless behavior,
functional safety, production usefulness, or fault ownership outside this
isolated run.
