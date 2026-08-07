#!/usr/bin/env bash
set -euo pipefail

repository_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
test_root="$(mktemp -d "${TMPDIR:-/tmp}/vda5050-tier1-multirobot-e2e.XXXXXX")"
suite_dir="$test_root/suite"
counts=(1 2 5 10 50 100)

cleanup() {
  rm -rf -- "$test_root"
}
trap cleanup EXIT

VDA5050_DEMO_SUITE_ARTIFACT_DIR="$suite_dir" \
  bash "$repository_root/scripts/run-tier1-fleet-suite.sh" all

jq -e '
  .schema == "vda5050-lab.tier1-suite/1" and
  .robot_counts == [1, 2, 5, 10, 50, 100] and
  .network_boundary == "COMPOSE_INTERNAL" and
  .physical_dut_authorized == false and
  .external_broker_authorized == false and
  .budget_policy == {
    "messages": "10*N+16",
    "duration_seconds": 60,
    "actors": "N+2",
    "mqtt_connections": "N+2"
  } and
  (.runs | length) == 6 and
  all(.runs[];
    (.robot_count as $count |
      .run_id == ("fleet-robots-" + ($count | tostring)) and
      .artifact_directory == ("robots-" + ($count | tostring)) and
      .expected_hard_limits == {
        "messages": (10 * $count + 16),
        "duration_seconds": 60,
        "actors": ($count + 2),
        "mqtt_connections": ($count + 2)
      }
    )
  )
' "$suite_dir/suite-manifest.json" >/dev/null

for count in "${counts[@]}"; do
  run_dir="$suite_dir/robots-$count"
  manifest="$run_dir/run-manifest.json"
  trace="$run_dir/trace.canonical.jsonl"
  expected_messages=$((10 * count + 16))
  expected_actors=$((count + 2))

  test -d "$run_dir"
  test ! -L "$run_dir"
  test "$(find "$run_dir" -mindepth 1 -maxdepth 1 -type f | wc -l | tr -d '[:space:]')" -eq 4

  jq -e \
    --argjson count "$count" \
    --argjson messages "$expected_messages" \
    --argjson actors "$expected_actors" '
      .schema == "vda5050-lab.tier1-run/2" and
      .robot_count == $count and
      .synthetic == true and
      .same_job_isolated == true and
      .target == {
        "host": "broker",
        "port": 1883,
        "network_boundary": "COMPOSE_INTERNAL"
      } and
      .coordinate_system == {
        "map_id": "warehouse-demo",
        "unit": "m",
        "origin": "lower-left",
        "x_axis": "right",
        "y_axis": "up"
      } and
      .fault.target_serial_number == "demo-001" and
      .hard_limits == {
        "messages": $messages,
        "duration_seconds": 60,
        "actors": $actors,
        "mqtt_connections": $actors
      } and
      (.robots | length) == $count and
      ([.robots[].serial_number] | unique | length) == $count and
      ([.robots[].client_id] | unique | length) == $count and
      ([.robots[].topic_prefix] | unique | length) == $count and
      ([.robots[].order_id] | unique | length) == $count and
      (.client_ids | length) == $actors and
      ([.client_ids[]] | unique | length) == $actors and
      (.topic_allowlist | length) == (4 * $count) and
      ([.topic_allowlist[]] | unique | length) == (4 * $count) and
      (.retain_allowlist | length) == $count and
      ([.retain_allowlist[]] | unique | length) == $count and
      all(.robots[];
        (.serial_number | capture("^demo-(?<number>[0-9]{3})$").number | tonumber) as $number |
        ($number - 1) as $index |
        .topic_prefix == ("vda5050/v3/lab-demo/" + .serial_number) and
        .route.start.x == (($index / 10 | floor) * 6) and
        .route.start.y == (($index % 10) * 2) and
        .route.released_end.x == (.route.start.x + 4) and
        .route.released_end.y == .route.start.y and
        .route.horizon_end.x == (.route.start.x + 4) and
        .route.horizon_end.y == (.route.start.y + 1)
      )
    ' "$manifest" >/dev/null

  jq -s -e --slurpfile manifest "$manifest" '
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
  ' "$trace" >/dev/null
done

test "$(find "$suite_dir" -mindepth 1 -maxdepth 1 -type d | wc -l | tr -d '[:space:]')" -eq 6
test "$(find "$suite_dir" -mindepth 1 -maxdepth 1 -type f | wc -l | tr -d '[:space:]')" -eq 1
