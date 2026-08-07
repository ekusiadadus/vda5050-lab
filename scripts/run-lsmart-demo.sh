#!/usr/bin/env bash
set -euo pipefail

umask 077

repository_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
compose_file="$repository_root/compose.lsmart.yaml"
source_input="${VDA5050_LSMART_SOURCE:-$repository_root/../lifelong-smart}"
artifact_input="${VDA5050_LSMART_ARTIFACT_DIR:-}"
run_id="${VDA5050_LSMART_RUN_ID:-lsmart-0001}"
web_port="${VDA5050_LSMART_WEB_PORT:-8080}"
headless="${VDA5050_LSMART_HEADLESS:-0}"
inject_fault="${VDA5050_LSMART_INJECT_FAULT:-true}"
expected_commit="ce0a020d8da10a806b23ffa3ccb56b1affff57d1"
expected_map_sha="318a9aa19525e7b63c1d11ea68063686997edc9f2fcf29641c53d8bc0dbf87da"
stack_started=false

fail() {
  printf 'lsmart-demo: %s\n' "$1" >&2
  exit 1
}

wait_for_service() {
  local service="$1"
  local container_id
  container_id="$("${compose[@]}" ps --all --quiet "$service")"
  [[ -n "$container_id" ]] || fail "$service container is missing"
  for _ in $(seq 1 2400); do
    if [[ "$(docker inspect --format '{{.State.Running}}' "$container_id")" == "false" ]]; then
      local exit_code
      exit_code="$(docker inspect --format '{{.State.ExitCode}}' "$container_id")"
      if [[ "$exit_code" != "0" ]]; then
        "${compose[@]}" logs --no-color "$service" >&2
        fail "$service exited with status $exit_code"
      fi
      return 0
    fi
    sleep 0.1
  done
  "${compose[@]}" logs --no-color "$service" >&2
  fail "$service exceeded its wait limit"
}

for command_name in curl docker git jq rg shasum; do
  command -v "$command_name" >/dev/null 2>&1 || fail "$command_name is required"
done
docker compose version >/dev/null 2>&1 || fail "Docker Compose is required"

[[ "$run_id" =~ ^[A-Za-z0-9_-]{1,32}$ ]] \
  || fail "run ID must contain only ASCII letters, digits, '-' or '_' and be 1..32 bytes"
if [[ ! "$web_port" =~ ^[0-9]+$ ]] || ((web_port < 1024 || web_port > 65535)); then
  fail "web port must be an integer in 1024..65535"
fi
[[ "$headless" == "0" || "$headless" == "1" ]] \
  || fail "VDA5050_LSMART_HEADLESS must be 0 or 1"
[[ "$inject_fault" == "true" || "$inject_fault" == "false" ]] \
  || fail "VDA5050_LSMART_INJECT_FAULT must be true or false"

case "$source_input" in
  /*) ;;
  *) fail "VDA5050_LSMART_SOURCE must be an absolute path" ;;
esac
[[ -d "$source_input" && ! -L "$source_input" ]] \
  || fail "LSMART source must be a real directory"
source_dir="$(cd "$source_input" && pwd -P)"
[[ "$(git -C "$source_dir" rev-parse HEAD)" == "$expected_commit" ]] \
  || fail "LSMART source is not at the pinned official commit"
[[ "$(git -C "$source_dir" remote get-url origin)" == "https://github.com/smart-mapf/lifelong-smart.git" ]] \
  || fail "LSMART origin is not the official repository"
[[ -z "$(git -C "$source_dir" status --porcelain=v1)" ]] \
  || fail "LSMART source checkout must be clean"
git -C "$source_dir" apply --check \
  "$repository_root/demo/lsmart/patches/0001-vda5050-causal-bridge.patch" \
  || fail "causal bridge overlay no longer applies to the pinned source"
git -C "$source_dir" apply --check \
  "$repository_root/demo/lsmart/patches/0002-ubuntu22-spdlog-ostream-compat.patch" \
  || fail "Ubuntu 22.04 spdlog compatibility overlay no longer applies"
actual_map_sha="$(shasum -a 256 "$source_dir/maps/kiva_large_w_mode.json" | awk '{print $1}')"
[[ "$actual_map_sha" == "$expected_map_sha" ]] || fail "pinned LSMART map digest changed"

if [[ -z "$artifact_input" ]]; then
  artifact_input="$(mktemp -d "${TMPDIR:-/tmp}/vda5050-lsmart-artifacts.XXXXXX")"
else
  case "$artifact_input" in
    /*) ;;
    *) fail "VDA5050_LSMART_ARTIFACT_DIR must be an absolute path" ;;
  esac
  [[ ! -L "$artifact_input" ]] || fail "artifact directory must not be a symlink"
  mkdir -p -- "$artifact_input"
fi
[[ -d "$artifact_input" && ! -L "$artifact_input" ]] \
  || fail "artifact path must be a real directory"
artifact_dir="$(cd "$artifact_input" && pwd -P)"
[[ "$artifact_dir" != "/" && "$artifact_dir" != "$repository_root" ]] \
  || fail "refusing unsafe artifact directory"
[[ -z "$(find "$artifact_dir" -mindepth 1 -maxdepth 1 -print -quit)" ]] \
  || fail "artifact directory must be empty"

bridge_dir="$(mktemp -d "${TMPDIR:-/tmp}/vda5050-lsmart-bridge.XXXXXX")"
mkdir -m 700 "$bridge_dir/outbox" "$bridge_dir/inbox" "$bridge_dir/telemetry"
install -m 600 "$source_dir/maps/kiva_large_w_mode.json" "$artifact_dir/lsmart-map.json"

export VDA5050_LSMART_SOURCE="$source_dir"
export VDA5050_LSMART_ARTIFACT_DIR="$artifact_dir"
export VDA5050_LSMART_BRIDGE_DIR="$bridge_dir"
export VDA5050_LSMART_RUN_ID="$run_id"
export VDA5050_LSMART_WEB_PORT="$web_port"
export VDA5050_LSMART_INJECT_FAULT="$inject_fault"
VDA5050_LSMART_UID="$(id -u)"
VDA5050_LSMART_GID="$(id -g)"
export VDA5050_LSMART_UID VDA5050_LSMART_GID
export COMPOSE_PROJECT_NAME="vda5050-lsmart-$$"

compose=(docker compose --file "$compose_file" --project-directory "$repository_root" --profile cockpit)

cleanup() {
  status=$?
  trap - EXIT INT TERM
  if [[ "$stack_started" == true ]]; then
    "${compose[@]}" down --volumes --remove-orphans >/dev/null 2>&1 || true
  fi
  if ((status != 0)); then
    printf 'lsmart-demo: Artifacts retained for diagnosis: %s\n' "$artifact_dir" >&2
    if [[ -f "$artifact_dir/lsmart-bridge-timeout.json" ]]; then
      printf 'lsmart-demo: Bridge timeout diagnostic: %s\n' \
        "$artifact_dir/lsmart-bridge-timeout.json" >&2
    fi
    if [[ -f "$artifact_dir/lsmart.log" ]]; then
      printf 'lsmart-demo: Official LSMART log: %s\n' "$artifact_dir/lsmart.log" >&2
    fi
  fi
  if [[ "$bridge_dir" == "${TMPDIR:-/tmp}"/vda5050-lsmart-bridge.* && -d "$bridge_dir" && ! -L "$bridge_dir" ]]; then
    rm -rf -- "$bridge_dir"
  fi
  exit "$status"
}
trap cleanup EXIT INT TERM

bash "$repository_root/tests/lsmart_integration_contract.sh" >/dev/null
"${compose[@]}" build lsmart_plan official_lsmart
"${compose[@]}" run --rm --no-deps lsmart_plan

jq -e --arg commit "$expected_commit" '
  .schema == "vda5050-lab.lsmart-run/1" and
  .synthetic == true and
  .source_commit == $commit and
  .redistribute_source == false and
  .redistribute_image == false and
  .map == "maps/kiva_large_w_mode.json" and
  .planner == "RHCR" and
  .fail_policy == "PIBT" and
  .task_assigner == "windowed" and
  .broker_host == "broker" and
  .broker_port == 1883 and
  (.robots | length) == 10
' "$artifact_dir/lsmart-run-manifest.json" >/dev/null \
  || fail "pre-CONNECT LSMART run plan failed validation"

"${compose[@]}" up --detach --no-build broker
stack_started=true
resolved_config="$("${compose[@]}" config --format json)"
tier1_network="$(jq -r '.networks.tier1.name' <<<"$resolved_config")"
docker network inspect "$tier1_network" \
  | jq -e 'length == 1 and .[0].Internal == true' >/dev/null \
  || fail "runtime Tier 1 network is not internal"
broker_id="$("${compose[@]}" ps --quiet broker)"
docker inspect "$broker_id" | jq -e '
  length == 1 and
  ((.[0].HostConfig.PortBindings // {}) | length == 0) and
  ((.[0].NetworkSettings.Ports // {}) | all(.[]; . == null))
' >/dev/null || fail "broker exposes a host port"

broker_ready=false
for _ in $(seq 1 100); do
  if "${compose[@]}" logs --no-color broker 2>&1 | rg -q 'mosquitto version [^ ]+ running'; then
    broker_ready=true
    break
  fi
  sleep 0.1
done
[[ "$broker_ready" == true ]] || fail "broker did not become ready"

"${compose[@]}" up --detach --no-build recorder
for _ in $(seq 1 100); do
  [[ -f "$artifact_dir/recorder-ready.json" ]] && break
  sleep 0.1
done
[[ -f "$artifact_dir/recorder-ready.json" ]] \
  || fail "recorder subscriptions did not become ready"

"${compose[@]}" up --detach --no-build vda5050-lsmart-bridge
for _ in $(seq 1 100); do
  [[ -f "$artifact_dir/lsmart-bridge-ready.json" ]] && break
  sleep 0.1
done
[[ -f "$artifact_dir/lsmart-bridge-ready.json" ]] \
  || fail "all VDA robot adapter subscriptions did not become ready"

"${compose[@]}" up --detach --no-build official_lsmart
wait_for_service official_lsmart
wait_for_service vda5050-lsmart-bridge
wait_for_service recorder

for artifact_name in lsmart-stats.json lsmart.log lsmart-sim-events.jsonl \
  wire-events.jsonl trace.canonical.jsonl synthetic-evidence.json \
  capture-sealed.json lsmart-bridge-summary.json; do
  [[ -f "$artifact_dir/$artifact_name" && ! -L "$artifact_dir/$artifact_name" ]] \
    || fail "missing or unsafe artifact: $artifact_name"
done

outbox_count="$(find "$bridge_dir/outbox" -maxdepth 1 -type f -name '*.json' | wc -l | tr -d ' ')"
inbox_count="$(find "$bridge_dir/inbox" -maxdepth 1 -type f -name '*.json' | wc -l | tr -d ' ')"
[[ "$outbox_count" -gt 0 && "$outbox_count" == "$inbox_count" ]] \
  || fail "LSMART action batches did not all traverse MQTT"
jq -e --argjson batches "$outbox_count" '
  .schema == "vda5050-lab.lsmart-bridge-summary/1" and
  .causal_mqtt_gate == true and
  .orders_published == $batches and
  .deliveries_released == $batches and
  .pose_samples > 0 and
  .fault_injected == true
' "$artifact_dir/lsmart-bridge-summary.json" >/dev/null \
  || fail "causal bridge summary failed validation"
jq -e '.total_finished_tasks > 0' "$artifact_dir/lsmart-stats.json" >/dev/null \
  || fail "official LSMART did not finish any warehouse task"
jq -s -e '
  ([.[] | select(.event_type == "SIM_SNAPSHOT") | .snapshot.robot_id] | unique | length) == 10 and
  all(.[]; .evidence == false)
' "$artifact_dir/lsmart-sim-events.jsonl" >/dev/null \
  || fail "ARGoS pose projection does not contain all 10 robots"

passive_tmp="$(mktemp "$artifact_dir/doctor-passive.json.tmp.XXXXXX")"
"${compose[@]}" run --rm --no-deps doctor_passive >"$passive_tmp"
mv -- "$passive_tmp" "$artifact_dir/doctor-passive.json"
evidence_tmp="$(mktemp "$artifact_dir/doctor-evidence.json.tmp.XXXXXX")"
"${compose[@]}" run --rm --no-deps doctor_evidence >"$evidence_tmp"
mv -- "$evidence_tmp" "$artifact_dir/doctor-evidence.json"

if [[ "$inject_fault" == "true" ]]; then
  jq -e 'any(.findings[]; .rule_id == "LAB-D4-RECONNECT-STATE" and .evaluation.verdict == "INCONCLUSIVE" and .investigation_target == "UNRESOLVED")' \
    "$artifact_dir/doctor-passive.json" >/dev/null \
    || fail "passive Doctor report did not preserve the INCONCLUSIVE boundary"
  jq -e 'any(.findings[]; .rule_id == "LAB-D4-RECONNECT-STATE" and .evaluation.verdict == "FAIL" and .investigation_target == "MOBILE_ROBOT")' \
    "$artifact_dir/doctor-evidence.json" >/dev/null \
    || fail "same-job evidence Doctor report did not reproduce the expected FAIL"
fi

printf 'Official LSMART causal demo completed.\n'
printf 'Artifacts: %s\n' "$artifact_dir"
if [[ "$headless" == "1" ]]; then
  exit 0
fi

"${compose[@]}" up --detach --no-build web_gateway
for _ in $(seq 1 100); do
  curl --fail --silent --show-error "http://127.0.0.1:$web_port/api/status" >/dev/null && break
  sleep 0.1
done
curl --fail --silent --show-error "http://127.0.0.1:$web_port/api/status" >/dev/null \
  || fail "web cockpit did not become ready"
printf 'Cockpit: http://127.0.0.1:%s\n' "$web_port"
printf 'Press Ctrl-C to stop the isolated stack.\n'
"${compose[@]}" logs --follow web_gateway
