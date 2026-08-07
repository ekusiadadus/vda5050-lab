#!/usr/bin/env bash
set -euo pipefail

umask 077

repository_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
compose_file="$repository_root/compose.live.yaml"
artifact_input="${VDA5050_LIVE_ARTIFACT_DIR:-}"
run_id="${VDA5050_LIVE_RUN_ID:-live-0001}"
scenario="${VDA5050_LIVE_SCENARIO:-fault}"
web_port="${VDA5050_LIVE_WEB_PORT:-8080}"
headless="${VDA5050_LIVE_HEADLESS:-0}"
browser_test="${VDA5050_LIVE_BROWSER_TEST:-0}"
media_dir="${VDA5050_LIVE_MEDIA_DIR:-}"
stack_started=false

fail() {
  printf 'live-demo: %s\n' "$1" >&2
  exit 1
}

wait_for_service() {
  local service="$1"
  local container_id
  container_id="$("${compose[@]}" ps --all --quiet "$service")"
  [[ -n "$container_id" ]] || fail "$service container is missing"
  for _ in $(seq 1 700); do
    if [[ "$(docker inspect --format '{{.State.Running}}' "$container_id")" == "false" ]]; then
      local exit_code
      exit_code="$(docker inspect --format '{{.State.ExitCode}}' "$container_id")"
      [[ "$exit_code" == "0" ]] || {
        "${compose[@]}" logs --no-color "$service" >&2
        fail "$service exited with status $exit_code"
      }
      return 0
    fi
    sleep 0.1
  done
  fail "$service exceeded its wait limit"
}

verify_diagnosis() {
  if [[ "$scenario" == "fault" ]]; then
    jq -e 'any(.findings[]; .rule_id == "LAB-D4-RECONNECT-STATE" and .evaluation.verdict == "INCONCLUSIVE" and .investigation_target == "UNRESOLVED")' \
      "$artifact_dir/doctor-passive.json" >/dev/null \
      || fail "passive report did not preserve the INCONCLUSIVE boundary"
    jq -e 'any(.findings[]; .rule_id == "LAB-D4-RECONNECT-STATE" and .evaluation.verdict == "FAIL" and .investigation_target == "MOBILE_ROBOT")' \
      "$artifact_dir/doctor-evidence.json" >/dev/null \
      || fail "same-job evidence report did not reproduce the expected FAIL"
  else
    jq -e 'all(.findings[]; .rule_id != "LAB-D4-RECONNECT-STATE")' \
      "$artifact_dir/doctor-passive.json" "$artifact_dir/doctor-evidence.json" >/dev/null \
      || fail "control scenario emitted a reconnect finding"
  fi
}

for command_name in curl docker jq rg; do
  command -v "$command_name" >/dev/null 2>&1 || fail "$command_name is required"
done
docker compose version >/dev/null 2>&1 || fail "Docker Compose is required"

[[ "$run_id" =~ ^[A-Za-z0-9_-]{1,32}$ ]] \
  || fail "run ID must contain only ASCII letters, digits, '-' or '_' and be 1..32 bytes"
[[ "$scenario" == "fault" || "$scenario" == "control" ]] \
  || fail "scenario must be 'fault' or 'control'"
if [[ ! "$web_port" =~ ^[0-9]+$ ]] || ((web_port < 1024 || web_port > 65535)); then
  fail "web port must be an integer in 1024..65535"
fi
[[ "$headless" == "0" || "$headless" == "1" ]] || fail "VDA5050_LIVE_HEADLESS must be 0 or 1"
[[ "$browser_test" == "0" || "$browser_test" == "1" ]] \
  || fail "VDA5050_LIVE_BROWSER_TEST must be 0 or 1"
if [[ -n "$media_dir" ]]; then
  case "$media_dir" in
    /*) ;;
    *) fail "VDA5050_LIVE_MEDIA_DIR must be an absolute path" ;;
  esac
  [[ -d "$media_dir" && ! -L "$media_dir" ]] || fail "media directory must be a real directory"
  [[ ! -e "$media_dir/live-demo.webm" ]] || fail "refusing to replace live-demo.webm"
  media_dir="$(cd "$media_dir" && pwd -P)"
  export VDA5050_LIVE_MEDIA_DIR="$media_dir"
fi

if [[ -z "$artifact_input" ]]; then
  artifact_input="$(mktemp -d "${TMPDIR:-/tmp}/vda5050-live-artifacts.XXXXXX")"
else
  case "$artifact_input" in
    /*) ;;
    *) fail "VDA5050_LIVE_ARTIFACT_DIR must be an absolute path" ;;
  esac
  [[ ! -L "$artifact_input" ]] || fail "artifact directory must not be a symlink"
  mkdir -p -- "$artifact_input"
fi
[[ -d "$artifact_input" && ! -L "$artifact_input" ]] || fail "artifact path must be a real directory"
artifact_dir="$(cd "$artifact_input" && pwd -P)"
[[ "$artifact_dir" != "/" && "$artifact_dir" != "$repository_root" ]] \
  || fail "refusing unsafe artifact directory"
[[ -z "$(find "$artifact_dir" -mindepth 1 -maxdepth 1 -print -quit)" ]] \
  || fail "artifact directory must be empty"

export VDA5050_LIVE_ARTIFACT_DIR="$artifact_dir"
export VDA5050_LIVE_RUN_ID="$run_id"
export VDA5050_LIVE_SCENARIO="$scenario"
export VDA5050_LIVE_WEB_PORT="$web_port"
VDA5050_LIVE_UID="$(id -u)"
VDA5050_LIVE_GID="$(id -g)"
export VDA5050_LIVE_UID VDA5050_LIVE_GID
export COMPOSE_PROJECT_NAME="vda5050-live-$$"

compose=(docker compose --file "$compose_file" --project-directory "$repository_root" --profile cockpit --profile browser-test --profile browser-record)

cleanup() {
  status=$?
  trap - EXIT INT TERM
  if [[ "$stack_started" == true ]]; then
    "${compose[@]}" down --volumes --remove-orphans >/dev/null 2>&1 || true
  fi
  exit "$status"
}
trap cleanup EXIT INT TERM

bash "$repository_root/tests/tier1_live_contract.sh" >/dev/null
"${compose[@]}" build live_plan
"${compose[@]}" run --rm --no-deps live_plan

jq -e '
  .schema == "vda5050-lab.tier1-live-run/1" and
  .synthetic == true and
  .same_job_isolated == true and
  .physical_dut_authorized == false and
  .broker_host == "broker" and
  .broker_port == 1883 and
  .hard_limits == {
    "messages": 384,
    "duration_seconds": 60,
    "actors": 4,
    "mqtt_connections": 4,
    "simulation_ticks": 240
  } and
  (.robots | length) == 2 and
  .fault_target == "demo-001"
' "$artifact_dir/run-manifest.json" >/dev/null || fail "pre-CONNECT live plan failed validation"

"${compose[@]}" up --detach --no-build broker
stack_started=true
resolved_config="$("${compose[@]}" config --format json)"
tier1_network="$(jq -r '.networks.tier1.name' <<<"$resolved_config")"
docker network inspect "$tier1_network" | jq -e 'length == 1 and .[0].Internal == true' >/dev/null \
  || fail "runtime Tier 1 network is not internal"
broker_id="$("${compose[@]}" ps --quiet broker)"
[[ -n "$broker_id" ]] || fail "broker did not start"
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
recorder_ready=false
for _ in $(seq 1 100); do
  if [[ -f "$artifact_dir/recorder-ready.json" ]]; then
    recorder_ready=true
    break
  fi
  sleep 0.1
done
[[ "$recorder_ready" == true ]] || fail "recorder subscriptions did not become ready"

"${compose[@]}" up --detach --no-build robot_sim
"${compose[@]}" run --rm --no-deps fleet_actor
wait_for_service recorder
wait_for_service robot_sim

for artifact_name in sim-events.jsonl wire-events.jsonl trace.canonical.jsonl synthetic-evidence.json capture-sealed.json; do
  [[ -f "$artifact_dir/$artifact_name" && ! -L "$artifact_dir/$artifact_name" ]] \
    || fail "missing or unsafe artifact: $artifact_name"
done

passive_tmp="$(mktemp "$artifact_dir/doctor-passive.json.tmp.XXXXXX")"
"${compose[@]}" run --rm --no-deps doctor_passive >"$passive_tmp"
mv -- "$passive_tmp" "$artifact_dir/doctor-passive.json"
evidence_tmp="$(mktemp "$artifact_dir/doctor-evidence.json.tmp.XXXXXX")"
"${compose[@]}" run --rm --no-deps doctor_evidence >"$evidence_tmp"
mv -- "$evidence_tmp" "$artifact_dir/doctor-evidence.json"

verify_diagnosis

printf 'Live Tier 1 demo completed.\n'
printf 'Artifacts: %s\n' "$artifact_dir"
if [[ "$headless" == "1" && "$browser_test" == "0" && -z "$media_dir" ]]; then
  exit 0
fi

if [[ "$browser_test" == "1" || -n "$media_dir" ]]; then
  "${compose[@]}" build browser_test
fi
"${compose[@]}" up --detach --no-build web_gateway
web_ready=false
for _ in $(seq 1 100); do
  if curl --fail --silent --show-error "http://127.0.0.1:$web_port/api/status" >/dev/null; then
    web_ready=true
    break
  fi
  sleep 0.1
done
[[ "$web_ready" == true ]] || fail "web cockpit did not become ready"
if [[ "$browser_test" == "1" ]]; then
  "${compose[@]}" run --rm --no-deps browser_test
fi
if [[ -n "$media_dir" ]]; then
  "${compose[@]}" run --rm --no-deps browser_recorder
  [[ -f "$media_dir/live-demo.webm" && ! -L "$media_dir/live-demo.webm" ]] \
    || fail "browser recorder did not produce live-demo.webm"
fi
if [[ "$headless" == "1" ]]; then
  exit 0
fi
printf 'Cockpit: http://127.0.0.1:%s\n' "$web_port"
printf 'Press Ctrl-C to stop the isolated stack.\n'
"${compose[@]}" logs --follow web_gateway
