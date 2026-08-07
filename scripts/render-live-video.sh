#!/usr/bin/env bash
set -euo pipefail

umask 077

repository_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
output_input="${VDA5050_LIVE_VIDEO_OUTPUT:-$repository_root/dist/vda5050-live-demo.mp4}"

for command_name in docker ffmpeg jq; do
  command -v "$command_name" >/dev/null 2>&1 || {
    printf 'render-live-video: %s is required\n' "$command_name" >&2
    exit 1
  }
done
case "$output_input" in
  /*) ;;
  *) output_input="$repository_root/$output_input" ;;
esac
output_parent="$(dirname "$output_input")"
mkdir -p -- "$output_parent"
[[ -d "$output_parent" && ! -L "$output_parent" ]] || {
  printf 'render-live-video: output directory must be a real directory\n' >&2
  exit 1
}
output_path="$output_parent/$(basename "$output_input")"
[[ "$output_path" == *.mp4 ]] || {
  printf 'render-live-video: output filename must end in .mp4\n' >&2
  exit 1
}
[[ ! -e "$output_path" ]] || {
  printf 'render-live-video: refusing to replace %s\n' "$output_path" >&2
  exit 1
}

work_dir="$(mktemp -d "${TMPDIR:-/tmp}/vda5050-live-video.XXXXXX")"
artifact_dir="$work_dir/artifacts"
media_dir="$work_dir/media"
mkdir "$artifact_dir" "$media_dir"
cleanup() {
  status=$?
  trap - EXIT INT TERM
  rm -f -- "$artifact_dir"/* "$media_dir"/* "$work_dir/output.mp4"
  rmdir "$artifact_dir" "$media_dir" "$work_dir"
  exit "$status"
}
trap cleanup EXIT INT TERM

VDA5050_LIVE_ARTIFACT_DIR="$artifact_dir" \
VDA5050_LIVE_MEDIA_DIR="$media_dir" \
VDA5050_LIVE_RUN_ID="live-video-0001" \
VDA5050_LIVE_SCENARIO="fault" \
VDA5050_LIVE_HEADLESS="1" \
  bash "$repository_root/scripts/run-live-demo.sh"

ffmpeg -nostdin -hide_banner -loglevel error \
  -i "$media_dir/live-demo.webm" \
  -c:v libx264 -preset medium -crf 20 -pix_fmt yuv420p -movflags +faststart \
  "$work_dir/output.mp4"
mv -- "$work_dir/output.mp4" "$output_path"
printf 'Live demo video: %s\n' "$output_path"
