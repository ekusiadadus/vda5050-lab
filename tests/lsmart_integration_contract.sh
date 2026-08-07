#!/usr/bin/env bash
set -euo pipefail

repository_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
provenance="$repository_root/demo/lsmart/provenance.json"
overlay="$repository_root/demo/lsmart/patches/0001-vda5050-causal-bridge.patch"
compat_overlay="$repository_root/demo/lsmart/patches/0002-ubuntu22-spdlog-ostream-compat.patch"
compose_file="$repository_root/compose.lsmart.yaml"

[[ -f "$provenance" ]]
jq -e '
  .schema == "vda5050-lab.lsmart-provenance/1" and
  .source_url == "https://github.com/smart-mapf/lifelong-smart.git" and
  .source_commit == "ce0a020d8da10a806b23ffa3ccb56b1affff57d1" and
  .source_license_status == "NO_ROOT_LICENSE_FILE_OBSERVED" and
  .redistribute_source == false and
  .redistribute_image == false and
  .argos.source_tag == "3.0.0-beta59" and
  .argos.source_commit == "3ef43eb857810a2a461a51f2574a9913cf56702f" and
  .argos.license == "MIT"
' "$provenance" >/dev/null

[[ -s "$overlay" ]]
[[ -s "$compat_overlay" ]]
rg -q 'vda_bridge_dir' "$overlay"
rg -q 'obtain_actions' "$overlay"
rg -q 'mqtt-delivered' "$overlay"
rg -q 'action.task_id' "$overlay"
rg -q 'spdlog/fmt/ostr.h' "$compat_overlay"
rg -q '/usr/local/lib/argos3' "$repository_root/demo/lsmart/Dockerfile"
rg -q 'libargos3core_simulator.so' "$repository_root/demo/lsmart/Dockerfile"

[[ -f "$compose_file" ]]
rg -q 'platform: linux/amd64' "$compose_file"
rg -q 'network_mode: none' "$compose_file"
rg -q 'vda5050-lsmart-bridge' "$compose_file"
if rg -q 'published:[[:space:]]+1883|^[[:space:]]*-[[:space:]]*"?1883:1883' "$compose_file"; then
  exit 1
fi

runner="$repository_root/scripts/run-lsmart-demo.sh"
[[ -x "$runner" ]]
rg -q 'total_finished_tasks > 0' "$runner"
rg -q 'outbox_count.*inbox_count' "$runner"
rg -q 'doctor-passive.json' "$runner"
rg -q 'Artifacts retained for diagnosis:' "$runner"
rg -q 'lsmart-bridge-timeout.json' "$runner"
