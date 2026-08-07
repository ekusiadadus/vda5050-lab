#!/usr/bin/env bash
set -euo pipefail

repository_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
compose_file="$repository_root/compose.live.yaml"

fail() {
  printf 'tier1-live-contract: %s\n' "$1" >&2
  exit 1
}

command -v docker >/dev/null 2>&1 || fail "docker is required"
command -v jq >/dev/null 2>&1 || fail "jq is required"
[[ -f "$compose_file" ]] || fail "compose.live.yaml is missing"

artifact_dir="$(mktemp -d "${TMPDIR:-/tmp}/vda5050-live-contract.XXXXXX")"
trap 'rmdir "$artifact_dir" >/dev/null 2>&1 || true' EXIT
export VDA5050_LIVE_ARTIFACT_DIR="$artifact_dir"
export VDA5050_LIVE_RUN_ID="contract-0001"
export VDA5050_LIVE_SCENARIO="fault"
VDA5050_LIVE_UID="$(id -u)"
VDA5050_LIVE_GID="$(id -g)"
export VDA5050_LIVE_UID VDA5050_LIVE_GID
export VDA5050_LIVE_WEB_PORT="18080"

resolved="$(docker compose --file "$compose_file" --project-directory "$repository_root" --profile cockpit config --format json)"
jq -e '
  .networks.tier1.internal == true and
  (.networks.cockpit.internal // false) == false and
  ((.services.broker.ports // []) | length == 0) and
  ((.services.recorder.ports // []) | length == 0) and
  ((.services.robot_sim.ports // []) | length == 0) and
  ((.services.fleet_actor.ports // []) | length == 0) and
  .services.doctor_passive.network_mode == "none" and
  .services.doctor_evidence.network_mode == "none" and
  ((.services.web_gateway.networks | keys | sort) == ["cockpit"]) and
  (.services.web_gateway.ports | length) == 1 and
  .services.web_gateway.ports[0].host_ip == "127.0.0.1" and
  ((.services.web_gateway.volumes[] | select(.target == "/artifacts")).read_only == true) and
  ((.services.recorder.networks | keys | sort) == ["tier1"]) and
  ((.services.robot_sim.networks | keys | sort) == ["tier1"]) and
  ((.services.fleet_actor.networks | keys | sort) == ["tier1"])
' <<<"$resolved" >/dev/null || fail "resolved live Compose boundary failed"

printf 'Tier 1 live Compose contract passed.\n'
