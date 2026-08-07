#!/usr/bin/env bash
set -euo pipefail

umask 077

repository_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
release_tag="${RELEASE_TAG:-}"
media_input="${DEMO_MEDIA_DIR:-dist}"
preview_input="${DEMO_PREVIEW:-}"
counts=(1 2 5 10 50 100)

fail() {
  printf 'demo-media: %s\n' "$1" >&2
  exit 1
}

for command_name in docker jq ffmpeg ffprobe; do
  command -v "$command_name" >/dev/null 2>&1 || fail "$command_name is required"
done
docker compose version >/dev/null 2>&1 || fail "Docker Compose is required"

[[ "$release_tag" =~ ^v[0-9]+\.[0-9]+\.[0-9]+$ ]] \
  || fail "RELEASE_TAG must match vX.Y.Z"
case "$media_input" in
  /*) media_dir="$media_input" ;;
  *) media_dir="$repository_root/$media_input" ;;
esac
expected_media_dir="$repository_root/dist"
[[ "$media_dir" == "$expected_media_dir" ]] \
  || fail "DEMO_MEDIA_DIR must resolve to the repository dist directory"
if [[ -L "$media_dir" ]]; then
  fail "dist must not be a symlink"
fi
mkdir -p -- "$media_dir"

preview_path=""
if [[ -n "$preview_input" ]]; then
  case "$preview_input" in
    /*) preview_path="$preview_input" ;;
    *) preview_path="$repository_root/$preview_input" ;;
  esac
  expected_preview="$repository_root/docs/assets/vda5050-demo-preview.gif"
  [[ "$preview_path" == "$expected_preview" ]] \
    || fail "DEMO_PREVIEW must resolve to docs/assets/vda5050-demo-preview.gif"
  [[ ! -L "$repository_root/docs/assets" && ! -L "$preview_path" ]] \
    || fail "preview path must not be a symlink"
  mkdir -p -- "$repository_root/docs/assets"
fi

work_dir="$(mktemp -d "${TMPDIR:-/tmp}/vda5050-demo-media.XXXXXX")"
suite_dir="$work_dir/suite"
cleanup() {
  status=$?
  trap - EXIT INT TERM
  rm -rf -- "$work_dir"
  exit "$status"
}
trap cleanup EXIT INT TERM

VDA5050_DEMO_SUITE_ARTIFACT_DIR="$suite_dir" \
  bash "$repository_root/scripts/run-tier1-fleet-suite.sh" all

jq -e '
  .schema == "vda5050-lab.tier1-suite/1" and
  .robot_counts == [1, 2, 5, 10, 50, 100] and
  .network_boundary == "COMPOSE_INTERNAL" and
  .physical_dut_authorized == false and
  .external_broker_authorized == false
' "$suite_dir/suite-manifest.json" >/dev/null \
  || fail "fleet suite manifest does not match the fixed release scales"

video_inputs=()
for count in "${counts[@]}"; do
  run_dir="$suite_dir/robots-$count"
  trace_path="$run_dir/trace.canonical.jsonl"
  report_path="$run_dir/doctor-report.json"
  jq -e --argjson count "$count" '
    .schema == "vda5050-lab.tier1-run/2" and
    .robot_count == $count and
    .coordinate_system.map_id == "warehouse-demo" and
    .fault.target_serial_number == "demo-001" and
    (.robots | length) == $count
  ' "$run_dir/run-manifest.json" >/dev/null \
    || fail "run manifest does not match fleet scale $count"

  bash "$repository_root/scripts/render-fleet-media.sh" \
    --trace "$trace_path" \
    --report "$report_path" \
    --output-dir "$media_dir" \
    --release-tag "$release_tag" \
    --robot-count "$count"

  padded_count="$(printf '%03d' "$count")"
  video_inputs+=("$media_dir/vda5050-fleet-${padded_count}-${release_tag}.mp4")
done

overview_video="$media_dir/vda5050-fleet-overview-${release_tag}.mp4"
overview_gif="$media_dir/vda5050-fleet-overview-${release_tag}.gif"
hero_video="$media_dir/vda5050-fleet-001-${release_tag}.mp4"
hero_gif="$media_dir/vda5050-fleet-001-${release_tag}.gif"
[[ ! -L "$overview_video" && ! -L "$overview_gif" ]] \
  || fail "overview output must not be a symlink"
[[ -f "$hero_video" && -f "$hero_gif" && ! -L "$hero_video" && ! -L "$hero_gif" ]] \
  || fail "single-robot hero media is missing or unsafe"

overview_tmp="$work_dir/overview.mp4"
ffmpeg -hide_banner -loglevel error -y \
  -i "${video_inputs[0]}" \
  -i "${video_inputs[1]}" \
  -i "${video_inputs[2]}" \
  -i "${video_inputs[3]}" \
  -i "${video_inputs[4]}" \
  -i "${video_inputs[5]}" \
  -filter_complex '
    [0:v]scale=640:360:flags=lanczos,setpts=PTS-STARTPTS[v0];
    [1:v]scale=640:360:flags=lanczos,setpts=PTS-STARTPTS[v1];
    [2:v]scale=640:360:flags=lanczos,setpts=PTS-STARTPTS[v2];
    [3:v]scale=640:360:flags=lanczos,setpts=PTS-STARTPTS[v3];
    [4:v]scale=640:360:flags=lanczos,setpts=PTS-STARTPTS[v4];
    [5:v]scale=640:360:flags=lanczos,setpts=PTS-STARTPTS[v5];
    [v0][v1][v2][v3][v4][v5]xstack=inputs=6:layout=0_0|640_0|1280_0|0_360|640_360|1280_360[grid];
    [grid]pad=1920:1080:0:180:color=0x07111f,format=yuv420p[overview]
  ' \
  -map '[overview]' -map_metadata -1 \
  -c:v libx264 -preset medium -crf 20 -threads 1 -movflags +faststart -an \
  -metadata creation_time='1970-01-01T00:00:00Z' \
  -metadata comment='fleet_scales=1,2,5,10,50,100;focused_agv=demo-001;coordinates=trace_payload' \
  "$overview_tmp"

overview_width="$(ffprobe -v error -select_streams v:0 -show_entries stream=width -of csv=p=0 "$overview_tmp")"
overview_height="$(ffprobe -v error -select_streams v:0 -show_entries stream=height -of csv=p=0 "$overview_tmp")"
overview_duration="$(ffprobe -v error -show_entries format=duration -of csv=p=0 "$overview_tmp")"
[[ "$overview_width" == 1920 && "$overview_height" == 1080 ]] \
  || fail "overview dimensions are not 1920x1080"
awk -v duration="$overview_duration" 'BEGIN { exit !(duration >= 7.9 && duration <= 8.1) }' \
  || fail "overview duration is outside the 8-second contract"

overview_palette="$work_dir/overview-palette.png"
overview_gif_tmp="$work_dir/overview.gif"
ffmpeg -hide_banner -loglevel error -y -i "$overview_tmp" \
  -vf 'fps=4,scale=960:540:flags=lanczos,palettegen=max_colors=64:stats_mode=diff' \
  "$overview_palette"
ffmpeg -hide_banner -loglevel error -y -i "$overview_tmp" -i "$overview_palette" \
  -lavfi 'fps=4,scale=960:540:flags=lanczos[x];[x][1:v]paletteuse=dither=bayer:bayer_scale=3' \
  -loop 0 "$overview_gif_tmp"

mv -f -- "$overview_tmp" "$overview_video"
mv -f -- "$overview_gif_tmp" "$overview_gif"

release_dir="$media_dir"
bash "$repository_root/scripts/verify-release-assets.sh" media "$release_dir" "$release_tag"

if [[ -n "$preview_path" ]]; then
  cp -- "$hero_gif" "$preview_path"
fi

printf 'Fleet media suite: %s\n' "$media_dir"
printf 'Scales: 1, 2, 5, 10, 50, 100 robots\n'
printf 'Overview: %s\n' "$overview_video"
