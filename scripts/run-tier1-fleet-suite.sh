#!/usr/bin/env bash
set -euo pipefail

umask 077

repository_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
single_runner="$repository_root/scripts/run-tier1-demo.sh"
selection="${1:-all}"
suite_input="${VDA5050_DEMO_SUITE_ARTIFACT_DIR:-}"
fixed_counts=(1 2 5 10 50 100)

fail() {
  printf 'tier1-fleet-suite: %s\n' "$1" >&2
  exit 1
}

if (($# > 1)); then
  fail "usage: run-tier1-fleet-suite.sh [all|1|2|5|10|50|100]"
fi

case "$selection" in
  all)
    counts=("${fixed_counts[@]}")
    ;;
  1 | 2 | 5 | 10 | 50 | 100)
    counts=("$selection")
    ;;
  *)
    fail "robot count must be one of 1, 2, 5, 10, 50, 100, or 'all'"
    ;;
esac

if [[ -z "$suite_input" ]]; then
  suite_input="$(mktemp -d "${TMPDIR:-/tmp}/vda5050-tier1-fleet-suite.XXXXXX")"
else
  case "$suite_input" in
    /*) ;;
    *) fail "VDA5050_DEMO_SUITE_ARTIFACT_DIR must be an absolute path" ;;
  esac
  if [[ -L "$suite_input" ]]; then
    fail "suite artifact directory must not be a symlink"
  fi
  mkdir -p -- "$suite_input"
fi

if [[ -L "$suite_input" || ! -d "$suite_input" ]]; then
  fail "suite artifact path must be a regular directory"
fi
suite_dir="$(cd "$suite_input" && pwd -P)"
if [[ "$suite_dir" == "/" || "$suite_dir" == "$repository_root" ]]; then
  fail "refusing unsafe suite artifact directory"
fi
if [[ -n "$(find "$suite_dir" -mindepth 1 -maxdepth 1 -print -quit)" ]]; then
  fail "suite artifact directory must be empty"
fi

counts_json="$(printf '%s\n' "${counts[@]}" | jq -s '.')"
runs_json="$(
  for count in "${counts[@]}"; do
    jq -n --argjson count "$count" '{
      robot_count: $count,
      run_id: ("fleet-robots-" + ($count | tostring)),
      artifact_directory: ("robots-" + ($count | tostring)),
      expected_hard_limits: {
        messages: (10 * $count + 16),
        duration_seconds: 60,
        actors: ($count + 2),
        mqtt_connections: ($count + 2)
      }
    }'
  done | jq -s '.'
)"

suite_manifest_tmp="$(mktemp "$suite_dir/suite-manifest.json.tmp.XXXXXX")"
jq -n \
  --argjson robot_counts "$counts_json" \
  --argjson runs "$runs_json" '{
    schema: "vda5050-lab.tier1-suite/1",
    robot_counts: $robot_counts,
    network_boundary: "COMPOSE_INTERNAL",
    broker: {
      service: "broker",
      host_port_published: false
    },
    physical_dut_authorized: false,
    external_broker_authorized: false,
    budget_policy: {
      messages: "10*N+16",
      duration_seconds: 60,
      actors: "N+2",
      mqtt_connections: "N+2"
    },
    runs: $runs
  }' >"$suite_manifest_tmp"
mv -- "$suite_manifest_tmp" "$suite_dir/suite-manifest.json"

for count in "${counts[@]}"; do
  run_dir="$suite_dir/robots-$count"
  mkdir -- "$run_dir"
  VDA5050_DEMO_ROBOT_COUNT="$count" \
  VDA5050_DEMO_RUN_ID="fleet-robots-$count" \
  VDA5050_DEMO_ARTIFACT_DIR="$run_dir" \
    bash "$single_runner"

  expected_messages=$((10 * count + 16))
  expected_actors=$((count + 2))
  jq -e \
    --argjson count "$count" \
    --argjson messages "$expected_messages" \
    --argjson actors "$expected_actors" '
      .schema == "vda5050-lab.tier1-run/2" and
      .robot_count == $count and
      .same_job_isolated == true and
      .target.network_boundary == "COMPOSE_INTERNAL" and
      .physical_dut_authorized == false and
      .hard_limits.messages == $messages and
      .hard_limits.duration_seconds == 60 and
      .hard_limits.actors == $actors and
      .hard_limits.mqtt_connections == $actors
    ' "$run_dir/run-manifest.json" >/dev/null \
    || fail "resolved run manifest does not match robot count $count"
done

printf 'Tier 1 fleet suite completed in isolated per-run networks.\n'
printf 'Suite artifacts: %s\n' "$suite_dir"
