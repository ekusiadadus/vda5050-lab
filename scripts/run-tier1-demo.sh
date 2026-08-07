#!/usr/bin/env bash
set -euo pipefail

umask 077

repository_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
compose_file="$repository_root/compose.yaml"
run_id="${VDA5050_DEMO_RUN_ID:-demo-0001}"
robot_count="${VDA5050_DEMO_ROBOT_COUNT:-1}"
artifact_input="${VDA5050_DEMO_ARTIFACT_DIR:-}"
broker_started=false

fail() {
  printf 'tier1-demo: %s\n' "$1" >&2
  exit 1
}

if [[ ! "$robot_count" =~ ^[1-9][0-9]*$ ]] || ((robot_count > 100)); then
  fail "robot count must be an integer in the inclusive range 1..100"
fi

for command_name in docker jq; do
  command -v "$command_name" >/dev/null 2>&1 || fail "$command_name is required"
done
docker compose version >/dev/null 2>&1 || fail "Docker Compose is required"

if [[ ! "$run_id" =~ ^[A-Za-z0-9_-]{1,32}$ ]]; then
  fail "run ID must contain only ASCII letters, digits, '-' or '_' and be 1..32 bytes"
fi

if [[ -z "$artifact_input" ]]; then
  artifact_input="$(mktemp -d "${TMPDIR:-/tmp}/vda5050-tier1-artifacts.XXXXXX")"
else
  case "$artifact_input" in
    /*) ;;
    *) fail "VDA5050_DEMO_ARTIFACT_DIR must be an absolute path" ;;
  esac
  if [[ -L "$artifact_input" ]]; then
    fail "artifact directory must not be a symlink"
  fi
  mkdir -p -- "$artifact_input"
fi

if [[ -L "$artifact_input" || ! -d "$artifact_input" ]]; then
  fail "artifact path must be a regular directory"
fi
artifact_dir="$(cd "$artifact_input" && pwd -P)"
if [[ "$artifact_dir" == "/" || "$artifact_dir" == "$repository_root" ]]; then
  fail "refusing unsafe artifact directory"
fi
if [[ -n "$(find "$artifact_dir" -mindepth 1 -maxdepth 1 -print -quit)" ]]; then
  fail "artifact directory must be empty"
fi

export VDA5050_DEMO_ARTIFACT_DIR="$artifact_dir"
export VDA5050_DEMO_RUN_ID="$run_id"
export VDA5050_DEMO_ROBOT_COUNT="$robot_count"
VDA5050_DEMO_UID="$(id -u)"
VDA5050_DEMO_GID="$(id -g)"
export VDA5050_DEMO_UID VDA5050_DEMO_GID
export COMPOSE_PROJECT_NAME="vda5050-tier1-$$"

compose=(docker compose --file "$compose_file" --project-directory "$repository_root")

cleanup() {
  status=$?
  trap - EXIT INT TERM
  if [[ "$broker_started" == true ]]; then
    "${compose[@]}" down --volumes --remove-orphans >/dev/null 2>&1 || true
  fi
  exit "$status"
}
trap cleanup EXIT INT TERM

resolved_config="$("${compose[@]}" config --format json)"
jq -e '
  .networks.tier1.internal == true and
  .networks.tier1.driver == "bridge" and
  ((.services.broker.ports // []) | length == 0) and
  ((.services.broker.networks | keys) == ["tier1"]) and
  ((.services.demo.networks | keys) == ["tier1"]) and
  .services.doctor.network_mode == "none" and
  .services.broker.image == "eclipse-mosquitto:2.1.2-alpine@sha256:6f8d8a947c506f8a2290ec65cd4bd2bc7cb4d43fb5f6271f861cb013e2ef9797" and
  .services.demo.command == [
    "--broker-host", "broker",
    "--broker-port", "1883",
    "--isolated-network",
    "--output-dir", "/artifacts",
    "--run-id", env.VDA5050_DEMO_RUN_ID,
    "--robot-count", env.VDA5050_DEMO_ROBOT_COUNT,
    "--no-animation"
  ]
' <<<"$resolved_config" >/dev/null || fail "resolved Compose safety contract failed"

"${compose[@]}" build demo
"${compose[@]}" up --detach --no-build broker
broker_started=true

network_name="$(jq -r '.networks.tier1.name' <<<"$resolved_config")"
docker network inspect "$network_name" | jq -e 'length == 1 and .[0].Internal == true' >/dev/null \
  || fail "runtime Docker network is not internal"

broker_id="$("${compose[@]}" ps --quiet broker)"
[[ -n "$broker_id" ]] || fail "broker container did not start"
docker inspect "$broker_id" | jq -e '
  length == 1 and
  ((.[0].HostConfig.PortBindings // {}) | length == 0) and
  ((.[0].NetworkSettings.Ports // {}) | all(.[]; . == null))
' >/dev/null || fail "broker exposes a host port"

broker_ready=false
for _ in $(seq 1 100); do
  if "${compose[@]}" logs --no-color broker 2>&1 | grep -Eq 'mosquitto version [^ ]+ running'; then
    broker_ready=true
    break
  fi
  if [[ "$(docker inspect --format '{{.State.Running}}' "$broker_id")" != true ]]; then
    break
  fi
  sleep 0.1
done
[[ "$broker_ready" == true ]] || fail "broker did not become ready without an MQTT probe"

"${compose[@]}" run --rm --no-deps demo

for artifact_name in run-manifest.json trace.canonical.jsonl synthetic-evidence.json; do
  [[ -f "$artifact_dir/$artifact_name" && ! -L "$artifact_dir/$artifact_name" ]] \
    || fail "missing or unsafe artifact: $artifact_name"
done

expected_messages=$((10 * robot_count + 16))
expected_actors=$((robot_count + 2))
jq -e \
  --argjson robot_count "$robot_count" \
  --argjson expected_messages "$expected_messages" \
  --argjson expected_actors "$expected_actors" '
  . as $manifest |
  .schema == "vda5050-lab.tier1-run/2" and
  .robot_count == $robot_count and
  .synthetic == true and
  .same_job_isolated == true and
  .target.host == "broker" and
  .target.port == 1883 and
  .target.network_boundary == "COMPOSE_INTERNAL" and
  .coordinate_system == {
    "map_id": "warehouse-demo",
    "unit": "m",
    "origin": "lower-left",
    "x_axis": "right",
    "y_axis": "up"
  } and
  .connect_is_side_effect == true and
  .physical_dut_authorized == false and
  .hard_limits == {
    "messages": $expected_messages,
    "duration_seconds": 60,
    "actors": $expected_actors,
    "mqtt_connections": $expected_actors
  } and
  .fault.target_serial_number == "demo-001" and
  (.robots | length) == $robot_count and
  ([.robots[].serial_number] | unique | length) == $robot_count and
  ([.robots[].client_id] | unique | length) == $robot_count and
  ([.robots[].topic_prefix] | unique | length) == $robot_count and
  ([.robots[].order_id] | unique | length) == $robot_count and
  (.client_ids | length) == $expected_actors and
  ([.client_ids[]] | unique | length) == $expected_actors and
  (.topic_allowlist | length) == (4 * $robot_count) and
  ([.topic_allowlist[]] | unique | length) == (4 * $robot_count) and
  (.retain_allowlist | length) == $robot_count and
  ([.retain_allowlist[]] | unique | length) == $robot_count and
  all(.robots[];
    (.topic_prefix + "/connection") as $connection_topic |
    (.topic_prefix + "/order") as $order_topic |
    (.topic_prefix + "/state") as $state_topic |
    (.topic_prefix + "/visualization") as $visualization_topic |
    .topic_prefix == ("vda5050/v3/lab-demo/" + .serial_number) and
    ($manifest.retain_allowlist | index($connection_topic)) != null and
    ($manifest.topic_allowlist | index($connection_topic)) != null and
    ($manifest.topic_allowlist | index($order_topic)) != null and
    ($manifest.topic_allowlist | index($state_topic)) != null and
    ($manifest.topic_allowlist | index($visualization_topic)) != null and
    (.route.start.x | type) == "number" and
    (.route.start.y | type) == "number" and
    (.route.released_end.x | type) == "number" and
    (.route.released_end.y | type) == "number" and
    (.route.horizon_end.x | type) == "number" and
    (.route.horizon_end.y | type) == "number"
  )
' "$artifact_dir/run-manifest.json" >/dev/null || fail "run manifest violates the Tier 1 safety contract"

jq -s -e --slurpfile manifest "$artifact_dir/run-manifest.json" '
  [
    .[] |
    select(.record_type == "MESSAGE_OBSERVED") |
    (.record.payload.bytes | implode | fromjson)
  ] as $payloads |
  ([
    $payloads[] |
    .. |
    objects |
    select(has("x") or has("y"))
  ]) as $coordinates |
  ([$payloads[].serialNumber] | unique | sort) ==
    ([$manifest[0].robots[].serial_number] | sort) and
  ($coordinates | length) > 0 and
  all($coordinates[];
    has("x") and has("y") and
    (.x | type) == "number" and
    (.y | type) == "number"
  )
' "$artifact_dir/trace.canonical.jsonl" >/dev/null \
  || fail "trace does not cover every resolved robot and coordinate"

report_tmp="$(mktemp "$artifact_dir/doctor-report.json.tmp.XXXXXX")"
if ! "${compose[@]}" run --rm --no-deps doctor >"$report_tmp"; then
  rm -f -- "$report_tmp"
  fail "offline doctor container failed"
fi
jq -e '
  .tool == "vda5050-doctor" and
  .vda_version == "3.0.0" and
  any(.findings[];
    .rule_id == "LAB-D4-RECONNECT-STATE" and
    .evaluation.verdict == "FAIL"
  )
' "$report_tmp" >/dev/null || {
  rm -f -- "$report_tmp"
  fail "doctor report did not reproduce the expected reconnect finding"
}
mv -- "$report_tmp" "$artifact_dir/doctor-report.json"

printf 'Tier 1 demo completed without a physical or external target.\n'
printf 'Artifacts: %s\n' "$artifact_dir"
