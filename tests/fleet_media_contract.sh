#!/usr/bin/env bash
set -euo pipefail

repository_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
renderer="$repository_root/scripts/render-fleet-media.sh"

test -f "$renderer"
grep -Fq 'BROKER TRACE + MATCHING SYNTHETIC EVIDENCE' "$renderer"
grep -Fq 'Passive trace only: INCONCLUSIVE / UNRESOLVED' "$renderer"

for command_name in jq ffmpeg ffprobe; do
  command -v "$command_name" >/dev/null 2>&1
done
if command -v magick >/dev/null 2>&1; then
  image_command=(magick)
else
  command -v convert >/dev/null 2>&1
  image_command=(convert)
fi

test_root="$(mktemp -d "${TMPDIR:-/tmp}/vda5050-fleet-media.XXXXXX")"
cleanup() {
  if [[ "${FLEET_MEDIA_TEST_KEEP:-0}" == 1 ]]; then
    printf 'Fleet media test artifacts retained at: %s\n' "$test_root"
  else
    rm -rf -- "$test_root"
  fi
}
trap cleanup EXIT

make_trace() {
  robot_count="$1"
  trace_path="$2"
  jq -cn --argjson robot_count "$robot_count" '
    range(0; $robot_count) as $robot |
    range(0; 5) as $step |
    ($robot / 10 | floor) as $column |
    ($robot % 10) as $row |
    ("demo-" + (($robot + 1) | tostring | ("000" + .)[-3:])) as $serial |
    {
      headerId: ($step + 1),
      timestamp: ("2026-08-07T00:00:0" + ($step | tostring) + ".000Z"),
      version: "3.0.0",
      manufacturer: "lab-demo",
      serialNumber: $serial,
      mobileRobotPosition: {
        x: (($column * 6) + $step),
        y: ($row * 2),
        theta: 0,
        mapId: "fleet-demo",
        localized: true
      },
      driving: ($step < 4)
    } as $payload |
    {
      record_type: "MESSAGE_OBSERVED",
      record: {
        event_id: ("event-" + ($robot | tostring) + "-" + ($step | tostring)),
        capture_id: "fleet-fixture",
        capture_point: "BROKER_EGRESS",
        source_sequence: (($robot * 5) + $step + 1),
        observed_topic: ("vda5050/v3/lab-demo/" + $serial + "/state"),
        payload: {
          storage: "INLINE",
          bytes: ($payload | tojson | explode),
          sha256: "fixture",
          size_bytes: (($payload | tojson) | length)
        }
      }
    }
  ' >"$trace_path"
}

make_report() {
  trace_path="$1"
  report_path="$2"
  if command -v sha256sum >/dev/null 2>&1; then
    trace_digest="$(sha256sum "$trace_path" | awk '{print $1}')"
  else
    trace_digest="$(shasum -a 256 "$trace_path" | awk '{print $1}')"
  fi
  jq -cn --arg digest "$trace_digest" '{
    source_digest: $digest,
    findings: [{
      rule_id: "LAB-D4-RECONNECT-STATE",
      evaluation: {verdict: "FAIL"},
      investigation_target: "MOBILE_ROBOT"
    }]
  }' >"$report_path"
}

for count in 1 100; do
  padded_count="$(printf '%03d' "$count")"
  trace_path="$test_root/trace-${padded_count}.jsonl"
  report_path="$test_root/report-${padded_count}.json"
  output_dir="$test_root/output-${padded_count}"
  mkdir -p -- "$output_dir"
  make_trace "$count" "$trace_path"
  make_report "$trace_path" "$report_path"

  bash "$renderer" \
    --trace "$trace_path" \
    --report "$report_path" \
    --output-dir "$output_dir" \
    --release-tag v9.9.9 \
    --robot-count "$count"

  video="$output_dir/vda5050-fleet-${padded_count}-v9.9.9.mp4"
  animation="$output_dir/vda5050-fleet-${padded_count}-v9.9.9.gif"
  test -f "$video" && test ! -L "$video"
  test -f "$animation" && test ! -L "$animation"
  test "$(ffprobe -v error -select_streams v:0 -show_entries stream=width -of csv=p=0 "$video")" = 1280
  test "$(ffprobe -v error -select_streams v:0 -show_entries stream=height -of csv=p=0 "$video")" = 720
  metadata="$(ffprobe -v error -show_entries format_tags=comment -of default=nw=1:nk=1 "$video")"
  grep -Fq "fleet_size=${count}" <<<"$metadata"
  grep -Fq 'focused_agv=demo-001' <<<"$metadata"
  grep -Fq 'coordinates=trace_payload' <<<"$metadata"
  dimensions="$("${image_command[@]}" identify -format '%wx%h' "${animation}[0]")"
  test "$dimensions" = 960x540
done

# The renderer must fail closed when the declared scale does not match the
# distinct robot identities actually present in the trace.
if bash "$renderer" \
  --trace "$test_root/trace-100.jsonl" \
  --report "$test_root/report-100.json" \
  --output-dir "$test_root/output-100" \
  --release-tag v9.9.9 \
  --robot-count 50; then
  printf 'renderer accepted a trace/robot-count mismatch\n' >&2
  exit 1
fi

printf 'Fleet media renderer verified for actual 1- and 100-robot traces.\n'
