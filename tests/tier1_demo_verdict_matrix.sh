#!/usr/bin/env bash
set -euo pipefail

repository_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
test_root="$(mktemp -d "${TMPDIR:-/tmp}/vda5050-tier1-matrix.XXXXXX")"
incident_dir="$test_root/incident"
control_dir="$test_root/control"
project_name="vda5050-tier1-matrix-$$"
control_started=false

cleanup() {
  status=$?
  trap - EXIT INT TERM
  if [[ "$control_started" == true ]]; then
    VDA5050_DEMO_RUN_ID=control-0001 \
    VDA5050_DEMO_ARTIFACT_DIR="$control_dir" \
    VDA5050_DEMO_UID="$(id -u)" \
    VDA5050_DEMO_GID="$(id -g)" \
      docker compose --profile control --project-name "$project_name" \
      --file "$repository_root/compose.yaml" --project-directory "$repository_root" \
      down --volumes --remove-orphans >/dev/null 2>&1 || true
  fi
  rm -rf -- "$test_root"
  exit "$status"
}
trap cleanup EXIT INT TERM

mkdir -p -- "$incident_dir" "$control_dir"

VDA5050_DEMO_RUN_ID=incident-0001 \
VDA5050_DEMO_ARTIFACT_DIR="$incident_dir" \
  bash "$repository_root/scripts/run-tier1-demo.sh"

jq -e '
  any(.findings[];
    .rule_id == "LAB-D4-RECONNECT-STATE" and
    .evaluation.verdict == "FAIL" and
    .investigation_target == "MOBILE_ROBOT"
  )
' "$incident_dir/doctor-report.json" >/dev/null

docker run --rm \
  --network none \
  --read-only \
  --user "$(id -u):$(id -g)" \
  --cap-drop ALL \
  --security-opt no-new-privileges:true \
  --pids-limit 32 \
  --memory 256m \
  --cpus 1 \
  --tmpfs /tmp:size=8m,mode=1777,noexec,nosuid,nodev \
  --volume "$incident_dir:/artifacts:ro" \
  --entrypoint /usr/local/bin/vda5050-doctor \
  vda5050-lab/tier1-demo:local \
  diagnose /artifacts/trace.canonical.jsonl \
  --vda-version 3.0.0 \
  --input-format canonical-jsonl \
  --format json >"$incident_dir/passive-report.json"

jq -e '
  any(.findings[];
    .rule_id == "LAB-D4-RECONNECT-STATE" and
    .evaluation.verdict == "INCONCLUSIVE" and
    .investigation_target == "UNRESOLVED"
  )
' "$incident_dir/passive-report.json" >/dev/null

export VDA5050_DEMO_RUN_ID=control-0001
export VDA5050_DEMO_ARTIFACT_DIR="$control_dir"
VDA5050_DEMO_UID="$(id -u)"
VDA5050_DEMO_GID="$(id -g)"
export VDA5050_DEMO_UID VDA5050_DEMO_GID

compose=(
  docker compose --profile control --project-name "$project_name"
  --file "$repository_root/compose.yaml" --project-directory "$repository_root"
)

resolved_config="$("${compose[@]}" config --format json)"
jq -e '
  .networks.tier1.internal == true and
  ((.services.broker.ports // []) | length == 0) and
  ((.services["demo-control"].networks | keys) == ["tier1"]) and
  .services["demo-control"].command[-1] == "--control-online-after-reconnect"
' <<<"$resolved_config" >/dev/null

"${compose[@]}" up --detach --no-build broker
control_started=true

broker_ready=false
for _ in $(seq 1 100); do
  if "${compose[@]}" logs --no-color broker 2>&1 | grep -Eq 'mosquitto version [^ ]+ running'; then
    broker_ready=true
    break
  fi
  sleep 0.1
done
[[ "$broker_ready" == true ]] || {
  printf 'verdict-matrix: control broker did not become ready\n' >&2
  exit 1
}

"${compose[@]}" run --rm --no-deps demo-control
"${compose[@]}" run --rm --no-deps doctor >"$control_dir/doctor-report.json"

jq -e 'all(.findings[]; .rule_id != "LAB-D4-RECONNECT-STATE")' \
  "$control_dir/doctor-report.json" >/dev/null

connection_states="$({
  jq -r '
    select(.record_type == "MESSAGE_OBSERVED") |
    .record.payload.bytes | implode | fromjson | .connectionState // empty
  ' "$control_dir/trace.canonical.jsonl"
} | paste -sd, -)"
[[ "$connection_states" == "ONLINE,CONNECTION_BROKEN,ONLINE" ]] || {
  printf 'verdict-matrix: unexpected control connection states: %s\n' "$connection_states" >&2
  exit 1
}

printf 'Tier 1 verdict matrix verified: synthetic FAIL, passive INCONCLUSIVE, control no-D4.\n'
