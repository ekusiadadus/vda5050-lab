#!/usr/bin/env bash
set -euo pipefail

repository_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
test_root="$(mktemp -d "${TMPDIR:-/tmp}/vda5050-tier1-e2e.XXXXXX")"
artifact_dir="$test_root/artifacts"

cleanup() {
  rm -rf -- "$test_root"
}
trap cleanup EXIT

VDA5050_DEMO_RUN_ID=e2e-0001 \
VDA5050_DEMO_ARTIFACT_DIR="$artifact_dir" \
  bash "$repository_root/scripts/run-tier1-demo.sh"

jq -e '
  .schema == "vda5050-lab.tier1-run/2" and
  .robot_count == 1 and
  .same_job_isolated == true and
  .target == {
    "host": "broker",
    "port": 1883,
    "network_boundary": "COMPOSE_INTERNAL"
  } and
  .physical_dut_authorized == false
' "$artifact_dir/run-manifest.json" >/dev/null

jq -e '
  .report_validity == "VALID" and
  any(.findings[];
    .rule_id == "LAB-D4-RECONNECT-STATE" and
    .evaluation.verdict == "FAIL" and
    .investigation_target == "MOBILE_ROBOT"
  )
' "$artifact_dir/doctor-report.json" >/dev/null

test "$(find "$artifact_dir" -mindepth 1 -maxdepth 1 -type f | wc -l | tr -d '[:space:]')" -eq 4

preconnect_artifact_dir="$test_root/preconnect-artifacts"
mkdir -p -- "$preconnect_artifact_dir"
if docker run --rm \
  --network none \
  --read-only \
  --cap-drop ALL \
  --security-opt no-new-privileges \
  --user "$(id -u):$(id -g)" \
  --volume "$preconnect_artifact_dir:/artifacts" \
  vda5050-lab/tier1-demo:local \
  --broker-host broker \
  --broker-port 1883 \
  --isolated-network \
  --output-dir /artifacts \
  --run-id preconnect-0001 \
  --robot-count 1 \
  --no-animation; then
  printf 'network-none preconnect check unexpectedly reached the broker\n' >&2
  exit 1
fi

jq -e '
  .schema == "vda5050-lab.tier1-run/2" and
  .run_id == "preconnect-0001" and
  .robot_count == 1 and
  .same_job_isolated == true and
  .connect_is_side_effect == true and
  .physical_dut_authorized == false
' "$preconnect_artifact_dir/run-manifest.json" >/dev/null
test ! -e "$preconnect_artifact_dir/trace.canonical.jsonl"
test ! -e "$preconnect_artifact_dir/synthetic-evidence.json"
