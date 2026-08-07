#!/usr/bin/env bash
set -euo pipefail

repository_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
compose_file="$repository_root/compose.yaml"
suite_runner="$repository_root/scripts/run-tier1-fleet-suite.sh"
single_runner="$repository_root/scripts/run-tier1-demo.sh"
counts=(1 2 5 10 50 100)

test -f "$suite_runner"
grep -Fq 'demo-fleet:' "$repository_root/Makefile"
grep -Fq 'demo-fleet-e2e:' "$repository_root/Makefile"

test_root="$(mktemp -d "${TMPDIR:-/tmp}/vda5050-tier1-multirobot-contract.XXXXXX")"
cleanup() {
  rm -rf -- "$test_root"
}
trap cleanup EXIT

for count in "${counts[@]}"; do
  artifact_dir="$test_root/robots-$count"
  mkdir -p -- "$artifact_dir"
  resolved_config="$(
    VDA5050_DEMO_RUN_ID="contract-$count" \
    VDA5050_DEMO_ROBOT_COUNT="$count" \
    VDA5050_DEMO_ARTIFACT_DIR="$artifact_dir" \
    VDA5050_DEMO_UID="$(id -u)" \
    VDA5050_DEMO_GID="$(id -g)" \
    COMPOSE_PROJECT_NAME="vda5050-tier1-contract-$count" \
      docker compose --file "$compose_file" --project-directory "$repository_root" \
      config --format json
  )"
  jq -e --argjson count "$count" '
    (.services.demo.command | index("--robot-count")) as $index |
    .networks.tier1.internal == true and
    ((.services.broker.ports // []) | length == 0) and
    ((.services.demo.networks | keys) == ["tier1"]) and
    .services.doctor.network_mode == "none" and
    $index != null and
    .services.demo.command[$index + 1] == ($count | tostring)
  ' <<<"$resolved_config" >/dev/null
done

for invalid in 0 101 -1 1.0 '1;touch /tmp/unsafe'; do
  invalid_dir="$test_root/invalid-${invalid//[^A-Za-z0-9]/_}"
  mkdir -p -- "$invalid_dir"
  if VDA5050_DEMO_SUITE_ARTIFACT_DIR="$invalid_dir" \
    bash "$suite_runner" "$invalid" >"$test_root/invalid.stdout" 2>"$test_root/invalid.stderr"; then
    printf 'unsafe robot count was accepted: %s\n' "$invalid" >&2
    exit 1
  fi
  test -z "$(find "$invalid_dir" -mindepth 1 -print -quit)"

  direct_invalid_dir="$test_root/direct-invalid-${invalid//[^A-Za-z0-9]/_}"
  mkdir -p -- "$direct_invalid_dir"
  if VDA5050_DEMO_ROBOT_COUNT="$invalid" \
    VDA5050_DEMO_ARTIFACT_DIR="$direct_invalid_dir" \
    bash "$single_runner" >"$test_root/direct-invalid.stdout" 2>"$test_root/direct-invalid.stderr"; then
    printf 'unsafe direct robot count was accepted: %s\n' "$invalid" >&2
    exit 1
  fi
  test -z "$(find "$direct_invalid_dir" -mindepth 1 -print -quit)"
done
