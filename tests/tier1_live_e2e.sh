#!/usr/bin/env bash
set -euo pipefail

repository_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
artifact_dir="$(mktemp -d "${TMPDIR:-/tmp}/vda5050-live-e2e.XXXXXX")"

cleanup() {
  status=$?
  trap - EXIT INT TERM
  rm -f -- "$artifact_dir"/*
  rmdir "$artifact_dir"
  exit "$status"
}
trap cleanup EXIT INT TERM

VDA5050_LIVE_ARTIFACT_DIR="$artifact_dir" \
VDA5050_LIVE_RUN_ID="live-ci-0001" \
VDA5050_LIVE_SCENARIO="fault" \
VDA5050_LIVE_HEADLESS="1" \
VDA5050_LIVE_BROWSER_TEST="${VDA5050_LIVE_BROWSER_TEST:-0}" \
  bash "$repository_root/scripts/run-live-demo.sh"

jq -s -e '
  ([.[] | select(.event_type == "SIM_SNAPSHOT") | .snapshot.robot_id] | unique | sort) == ["demo-001", "demo-002"] and
  any(.[]; .event_type == "SCENARIO_PHASE" and .phase == "CONNECTION_BROKEN") and
  any(.[]; .event_type == "SCENARIO_PHASE" and .phase == "RECONNECTED_WITHOUT_ONLINE") and
  all(.[]; .evidence == false) and
  ([.[] | select(.event_type == "SIM_SNAPSHOT")] | length) <= 480
' "$artifact_dir/sim-events.jsonl" >/dev/null

jq -s -e '
  ([.[] | select(.event_type == "WIRE_OBSERVED") | .payload.serialNumber // empty] | unique | sort) == ["demo-001", "demo-002"] and
  any(.[]; .event_type == "WIRE_OBSERVED" and .payload.connectionState == "CONNECTION_BROKEN") and
  all(.[]; .evidence == false)
' "$artifact_dir/wire-events.jsonl" >/dev/null

jq -e 'any(.findings[]; .rule_id == "LAB-D4-RECONNECT-STATE" and .evaluation.verdict == "INCONCLUSIVE")' \
  "$artifact_dir/doctor-passive.json" >/dev/null
jq -e 'any(.findings[]; .rule_id == "LAB-D4-RECONNECT-STATE" and .evaluation.verdict == "FAIL")' \
  "$artifact_dir/doctor-evidence.json" >/dev/null

printf 'Tier 1 live E2E passed.\n'
