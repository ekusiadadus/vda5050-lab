# Live Two-Robot Simulator Demo

This demo is the shortest way to see the full vda5050-lab story locally:
two deterministic robots move, one reconnects incorrectly, a recorder captures
the real MQTT exchange, and the offline Doctor explains what the trace can and
cannot prove.

## Run it

Prerequisites: Docker Desktop or Docker Engine with Compose, `curl`, `jq`, and
`rg`.

```sh
make live-demo
```

After the fixed scenario and both diagnoses complete, open:

```text
http://127.0.0.1:8080
```

The command keeps the read-only cockpit running and tears the isolated stack
down when you press Ctrl-C. Choose another localhost port with:

```sh
VDA5050_LIVE_WEB_PORT=18080 make live-demo
```

Run the same scenario without serving the cockpit:

```sh
VDA5050_LIVE_HEADLESS=1 make live-demo
```

Store artifacts in a known empty directory:

```sh
mkdir /tmp/vda5050-live-demo
VDA5050_LIVE_ARTIFACT_DIR=/tmp/vda5050-live-demo make live-demo
```

The runner rejects a non-empty directory, symlink directory, unsafe run ID,
non-local Web port, unexpected scenario, unsafe Compose topology, exposed
broker port, or non-internal MQTT network before participant MQTT CONNECT.

## What to watch

`demo-001` and `demo-002` receive separate VDA 5050 3.0.0 orders. Both move on
six-meter released edges. When `demo-001` crosses 2.5 meters, its first MQTT
connection ends and Mosquitto publishes `CONNECTION_BROKEN`. The robot
reconnects ten 20-Hz ticks later and keeps publishing state and visualization,
but the fault scenario intentionally omits its new retained `ONLINE` message.

The bottom panel compares Doctor against the same sealed trace:

- passive trace only: `INCONCLUSIVE / UNRESOLVED`;
- matching same-job synthetic manifest: `FAIL / MOBILE_ROBOT`.

Use the timeline slider to replay deterministic simulator snapshots. The map
and scenario phases are explicitly marked non-evidentiary; they never change a
Doctor result.

## Control

The control variant publishes `ONLINE` after reconnect:

```sh
VDA5050_LIVE_SCENARIO=control make live-demo
```

It should produce no `LAB-D4-RECONNECT-STATE` finding. That absence is not a
PASS or conformance certificate.

## Verification commands

```sh
make live-contract
make live-e2e
make live-browser-e2e
make web-test
```

`live-contract` checks the resolved Compose boundary. `live-e2e` runs the real
Mosquitto scenario headlessly and validates simulator, wire, capture, and both
Doctor results. `live-browser-e2e` adds the pinned Playwright smoke test.
`web-test` runs the UI model tests with 80% coverage floors and builds the
TypeScript/Tailwind assets in the pinned Node image.

The optional browser contract is isolated in the `browser-test` Compose
profile and official pinned Playwright image:

```sh
VDA5050_LIVE_BROWSER_TEST=1 VDA5050_LIVE_HEADLESS=1 make live-demo
```

Create a reproducible MP4 from a fresh real-MQTT run, browser replay, and
offline diagnosis:

```sh
make live-video
```

This requires `ffmpeg` and the pinned Playwright image. It creates
`dist/vda5050-live-demo.mp4` without replacing an existing file. Publish large
MP4 output as a GitHub Release asset; keep only a compact preview in the README.

## What this proves

It proves that this exact bounded software job can:

- execute deterministic continuous motion for two synthetic robots;
- exchange VDA 5050 3.0.0 messages through a real disposable MQTT broker;
- capture broker-egress observations in a separate process;
- preserve passive versus asserted evidence semantics; and
- reproduce the expected Doctor diagnosis offline.

It does not prove physical motion, collision safety, wireless behavior,
third-party interoperability, production readiness, vendor responsibility,
throughput, or official VDA conformance. See
[ADR 0005](adr/0005-deterministic-live-simulator-cockpit.md) for the complete
boundary.
