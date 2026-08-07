#!/usr/bin/env bash
set -euo pipefail

run_id="${VDA5050_LSMART_RUN_ID:?VDA5050_LSMART_RUN_ID must be set}"
bridge_dir="${VDA5050_LSMART_BRIDGE_DIR:-/bridge}"
artifact_dir="${VDA5050_LSMART_ARTIFACT_DIR:-/artifacts}"

[[ "$run_id" =~ ^[A-Za-z0-9_-]+$ ]]
[[ -d "$bridge_dir" && ! -L "$bridge_dir" ]]
[[ -d "$artifact_dir" && ! -L "$artifact_dir" ]]
[[ -d "$bridge_dir/outbox" && -d "$bridge_dir/inbox" && -d "$bridge_dir/telemetry" ]]
[[ ! -e "$bridge_dir/complete.json" ]]
[[ ! -e "$artifact_dir/lsmart-stats.json" ]]
[[ ! -e "$artifact_dir/lsmart.log" ]]

export VDA5050_LSMART_RUN_ID="$run_id"
export VDA5050_LSMART_BRIDGE_DIR="$bridge_dir"

python3 /usr/project/run_lifelong.py /usr/project/maps/kiva_large_w_mode.json \
  --num_agents 10 \
  --headless True \
  --argos_config_filepath "$bridge_dir/output.argos" \
  --stats_name "$artifact_dir/lsmart-stats.json" \
  --save_stats True \
  --output_log "$artifact_dir/lsmart.log" \
  --port_num 8182 \
  --n_threads 1 \
  --sim_duration 600 \
  --sim_window_tick 10 \
  --ticks_per_second 10 \
  --velocity 200 \
  --planner RHCR \
  --container True \
  --seed 42 \
  --screen 0 \
  --backup_solver PIBT \
  --planner_invoke_policy default \
  --task_assigner_type windowed \
  --planning_window 10 \
  --solver PBS \
  --single_agent_solver SIPP \
  --rotation False \
  --cutoffTime 1

python3 - "$bridge_dir/complete.json" "$run_id" <<'PY'
import json
import os
import sys

target, run_id = sys.argv[1:]
temporary = target + ".tmp"
with open(temporary, "x", encoding="utf-8") as output:
    json.dump({"schema": "vda5050-lab.lsmart-complete/1", "run_id": run_id}, output)
    output.write("\n")
    output.flush()
    os.fsync(output.fileno())
os.rename(temporary, target)
PY
