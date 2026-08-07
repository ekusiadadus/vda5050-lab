#!/usr/bin/env bash
set -euo pipefail

umask 077

usage() {
  printf '%s\n' \
    "usage: $0 --trace TRACE --report REPORT --output-dir DIR --release-tag vX.Y.Z --robot-count N" >&2
  exit 64
}

fail() {
  printf 'fleet-media: %s\n' "$1" >&2
  exit 1
}

trace_path=""
report_path=""
output_dir=""
release_tag=""
robot_count=""
while [[ "$#" -gt 0 ]]; do
  case "$1" in
    --trace | --report | --output-dir | --release-tag | --robot-count)
      [[ "$#" -ge 2 ]] || usage
      case "$1" in
        --trace) trace_path="$2" ;;
        --report) report_path="$2" ;;
        --output-dir) output_dir="$2" ;;
        --release-tag) release_tag="$2" ;;
        --robot-count) robot_count="$2" ;;
      esac
      shift 2
      ;;
    *) usage ;;
  esac
done

[[ "$release_tag" =~ ^v[0-9]+\.[0-9]+\.[0-9]+$ ]] \
  || fail "release tag must match vX.Y.Z"
[[ "$robot_count" =~ ^([1-9]|[1-9][0-9]|100)$ ]] \
  || fail "robot count must be an integer from 1 through 100"
[[ -n "$trace_path" && -f "$trace_path" && ! -L "$trace_path" ]] \
  || fail "trace must be a regular non-symlink file"
[[ -n "$report_path" && -f "$report_path" && ! -L "$report_path" ]] \
  || fail "report must be a regular non-symlink file"
[[ -n "$output_dir" ]] || fail "output directory is required"

for command_name in jq ffmpeg ffprobe awk sort wc; do
  command -v "$command_name" >/dev/null 2>&1 || fail "$command_name is required"
done
if command -v magick >/dev/null 2>&1; then
  image_command=(magick)
elif command -v convert >/dev/null 2>&1; then
  image_command=(convert)
else
  fail "ImageMagick is required"
fi
font_file=""
for candidate in \
  /usr/share/fonts/truetype/dejavu/DejaVuSans.ttf \
  /System/Library/Fonts/SFNS.ttf \
  /System/Library/Fonts/SFNSRounded.ttf; do
  if [[ -f "$candidate" ]]; then
    font_file="$candidate"
    break
  fi
done
[[ -n "$font_file" ]] || fail "a known DejaVu or SF system font is required"

trace_size="$(wc -c <"$trace_path" | tr -d ' ')"
report_size="$(wc -c <"$report_path" | tr -d ' ')"
[[ "$trace_size" -le 67108864 ]] || fail "trace exceeds the 64-MiB render limit"
[[ "$report_size" -le 4194304 ]] || fail "report exceeds the 4-MiB render limit"

if [[ -L "$output_dir" ]]; then
  fail "output directory must not be a symlink"
fi
mkdir -p -- "$output_dir"
output_dir="$(cd "$output_dir" && pwd -P)"

if command -v sha256sum >/dev/null 2>&1; then
  trace_digest="$(sha256sum "$trace_path" | awk '{print $1}')"
else
  trace_digest="$(shasum -a 256 "$trace_path" | awk '{print $1}')"
fi
report_digest="$(jq -er '.source_digest' "$report_path")" \
  || fail "report source_digest is missing"
[[ "$report_digest" == "$trace_digest" ]] \
  || fail "report source_digest does not match the trace"
jq -e '
  any(.findings[]?;
    .rule_id == "LAB-D4-RECONNECT-STATE" and
    .evaluation.verdict == "FAIL" and
    .investigation_target == "MOBILE_ROBOT"
  )
' "$report_path" >/dev/null || fail "report does not contain the expected D4 fault"

work_dir="$(mktemp -d "${TMPDIR:-/tmp}/vda5050-fleet-media.XXXXXX")"
frames_dir="$work_dir/frames"
mkdir -p -- "$frames_dir"
cleanup() {
  status=$?
  trap - EXIT INT TERM
  rm -rf -- "$work_dir"
  exit "$status"
}
trap cleanup EXIT INT TERM

positions="$work_dir/positions.tsv"
jq -r '
  select(.record_type == "MESSAGE_OBSERVED") |
  .record as $record |
  try ($record.payload.bytes | implode | fromjson) catch null |
  select(. != null) |
  select(.serialNumber | type == "string") |
  select(.mobileRobotPosition.x | type == "number") |
  select(.mobileRobotPosition.y | type == "number") |
  [
    .serialNumber,
    ($record.source_sequence // 0),
    .mobileRobotPosition.x,
    .mobileRobotPosition.y
  ] | @tsv
' "$trace_path" | LC_ALL=C sort -t $'\t' -k1,1 -k2,2n >"$positions"
[[ -s "$positions" ]] || fail "trace has no inline mobileRobotPosition observations"

if ! awk -F '\t' '
  NF != 4 { exit 1 }
  $1 !~ /^demo-[0-9][0-9][0-9]$/ { exit 1 }
  $2 !~ /^[0-9]+$/ { exit 1 }
  $3 !~ /^-?([0-9]+([.][0-9]+)?|[.][0-9]+)$/ { exit 1 }
  $4 !~ /^-?([0-9]+([.][0-9]+)?|[.][0-9]+)$/ { exit 1 }
' "$positions"; then
  fail "trace contains an unsafe identity, sequence, or coordinate"
fi

identity_count="$(cut -f1 "$positions" | LC_ALL=C sort -u | wc -l | tr -d ' ')"
[[ "$identity_count" -eq "$robot_count" ]] \
  || fail "trace contains $identity_count robots; declared scale is $robot_count"
grep -Fqx 'demo-001' <(cut -f1 "$positions" | LC_ALL=C sort -u) \
  || fail "fault target demo-001 is absent"

missing_samples="$(awk -F '\t' '
  $1 != previous && NR > 1 {
    if (samples < 2) missing += 1
    samples = 0
  }
  { previous = $1; samples += 1 }
  END {
    if (samples < 2) missing += 1
    print missing + 0
  }
' "$positions")"
[[ "$missing_samples" -eq 0 ]] || fail "every robot must have at least two observed positions"

read -r min_x max_x min_y max_y < <(awk -F '\t' '
  NR == 1 { min_x = max_x = $3; min_y = max_y = $4 }
  {
    if ($3 < min_x) min_x = $3
    if ($3 > max_x) max_x = $3
    if ($4 < min_y) min_y = $4
    if ($4 > max_y) max_y = $4
  }
  END { print min_x, max_x, min_y, max_y }
' "$positions")

render_svg() {
  svg_path="$1"
  frame_index="$2"
  case "$frame_index" in
    0) stage="OBSERVE 1/5"; detail="first broker-observed position" ;;
    1) stage="OBSERVE 2/5"; detail="actual trace x,y replay" ;;
    2) stage="OBSERVE 3/5"; detail="fleet progresses on released edges" ;;
    3) stage="OBSERVE 4/5"; detail="actual trace x,y replay" ;;
    4) stage="BASE END"; detail="released path endpoint reached" ;;
    5) stage="CONNECTION_BROKEN"; detail="demo-001 abnormal disconnect" ;;
    6) stage="RECONNECT"; detail="new epoch omits retained ONLINE" ;;
    7) stage="TRACE DOCTOR"; detail="D4 FAIL · target MOBILE_ROBOT" ;;
    *) fail "invalid frame index" ;;
  esac
  if [[ "$frame_index" -lt 5 ]]; then
    online_count="$robot_count"
    fault_count=0
  else
    online_count="$((robot_count - 1))"
    fault_count=1
  fi
  density_percent="$robot_count"

  {
    printf '%s\n' '<?xml version="1.0" encoding="UTF-8"?>'
    printf '%s\n' '<svg xmlns="http://www.w3.org/2000/svg" width="1280" height="720" viewBox="0 0 1280 720">'
    printf '%s\n' '<rect width="1280" height="720" fill="#07111f"/>'
    printf '%s\n' '<rect x="24" y="24" width="1232" height="672" rx="24" fill="#0d1b2d" stroke="#27415f" stroke-width="2"/>'
    printf '%s\n' '<rect x="48" y="88" width="874" height="548" rx="18" fill="#102238" stroke="#29435f"/>'
    printf '%s\n' '<rect x="944" y="88" width="286" height="548" rx="18" fill="#091726"/>'
    printf '%s\n' '<text x="52" y="62" fill="#e7f2ff" font-family="DejaVu Sans,Arial,sans-serif" font-size="25" font-weight="bold">VDA 5050 actual trace x,y · fleet scale</text>'
    printf '<text x="1198" y="62" text-anchor="end" fill="#68d8ff" font-family="DejaVu Sans,Arial,sans-serif" font-size="20">%03d AGVs</text>\n' "$robot_count"
    printf '<text x="970" y="130" fill="#819bb8" font-family="DejaVu Sans,Arial,sans-serif" font-size="17">STAGE</text>\n'
    printf '<text x="970" y="162" fill="#ffffff" font-family="DejaVu Sans,Arial,sans-serif" font-size="24" font-weight="bold">%s</text>\n' "$stage"
    printf '<text x="970" y="192" fill="#a5bad0" font-family="DejaVu Sans,Arial,sans-serif" font-size="15">%s</text>\n' "$detail"
    printf '<text x="970" y="246" fill="#819bb8" font-family="DejaVu Sans,Arial,sans-serif" font-size="16">FLEET SUMMARY</text>\n'
    printf '<text x="970" y="282" fill="#ffffff" font-family="DejaVu Sans,Arial,sans-serif" font-size="21">total       %d</text>\n' "$robot_count"
    printf '<text x="970" y="316" fill="#5ee5bd" font-family="DejaVu Sans,Arial,sans-serif" font-size="21">online      %d</text>\n' "$online_count"
    printf '<text x="970" y="350" fill="#ff7088" font-family="DejaVu Sans,Arial,sans-serif" font-size="21">fault       %d</text>\n' "$fault_count"
    printf '<text x="970" y="384" fill="#f7c65d" font-family="DejaVu Sans,Arial,sans-serif" font-size="21">density     %d%%</text>\n' "$density_percent"
    printf '<text x="970" y="430" fill="#819bb8" font-family="DejaVu Sans,Arial,sans-serif" font-size="16">OBSERVED EXTENT</text>\n'
    printf '<text x="970" y="462" fill="#d9e6f3" font-family="DejaVu Sans,Arial,sans-serif" font-size="16">x  %s .. %s</text>\n' "$min_x" "$max_x"
    printf '<text x="970" y="490" fill="#d9e6f3" font-family="DejaVu Sans,Arial,sans-serif" font-size="16">y  %s .. %s</text>\n' "$min_y" "$max_y"
    printf '%s\n' '<text x="970" y="536" fill="#819bb8" font-family="DejaVu Sans,Arial,sans-serif" font-size="16">FOCUSED AGV</text>'
    printf '%s\n' '<text x="970" y="570" fill="#ff7088" font-family="DejaVu Sans,Arial,sans-serif" font-size="23" font-weight="bold">demo-001</text>'
    printf '<text x="970" y="604" fill="#5f7894" font-family="DejaVu Sans,Arial,sans-serif" font-size="12">trace %s</text>\n' "${trace_digest:0:16}"
    printf '%s\n' '<text x="52" y="672" fill="#7892ad" font-family="DejaVu Sans,Arial,sans-serif" font-size="16">Synthetic isolated fleet · actual payload coordinates · no physical DUT</text>'

    awk -F '\t' \
      -v frame="$frame_index" \
      -v robot_count="$robot_count" \
      -v min_x="$min_x" -v max_x="$max_x" \
      -v min_y="$min_y" -v max_y="$max_y" '
      function escape(value) {
        gsub(/&/, "\\&amp;", value)
        gsub(/</, "\\&lt;", value)
        gsub(/>/, "\\&gt;", value)
        return value
      }
      {
        if (!seen[$1]++) {
          robots[++robot_total] = $1
        }
        sample_count[$1] += 1
        sample_x[$1 SUBSEP sample_count[$1]] = $3
        sample_y[$1 SUBSEP sample_count[$1]] = $4
      }
      END {
        x_span = max_x - min_x
        y_span = max_y - min_y
        if (x_span == 0) x_span = 1
        if (y_span == 0) y_span = 1
        radius = robot_count <= 10 ? 16 : (robot_count <= 50 ? 10 : 8)
        font_size = robot_count <= 10 ? 15 : (robot_count <= 50 ? 11 : 9)

        for (r = 1; r <= robot_total; r++) {
          serial = robots[r]
          if (frame <= 4) {
            sample = 1 + int(frame * (sample_count[serial] - 1) / 4)
          } else {
            sample = sample_count[serial]
          }
          x = sample_x[serial SUBSEP sample]
          y = sample_y[serial SUBSEP sample]
          px = 82 + ((x - min_x) / x_span) * 806
          py = 598 - ((y - min_y) / y_span) * 470
          id = substr(serial, length(serial) - 2)
          fault = serial == "demo-001" && frame >= 5
          fill = fault ? "#ff496c" : "#39c8f0"
          stroke = serial == "demo-001" ? "#ffe07a" : "#07111f"
          stroke_width = serial == "demo-001" ? 4 : 2
          printf "<circle cx=\"%.2f\" cy=\"%.2f\" r=\"%d\" fill=\"%s\" stroke=\"%s\" stroke-width=\"%d\"/>\n", px, py, radius, fill, stroke, stroke_width
          printf "<text x=\"%.2f\" y=\"%.2f\" text-anchor=\"middle\" dominant-baseline=\"middle\" fill=\"#ffffff\" font-family=\"DejaVu Sans,Arial,sans-serif\" font-size=\"%d\" font-weight=\"bold\">%s</text>\n", px, py + 1, font_size, escape(id)
        }
      }
    ' "$positions"
    printf '%s\n' '</svg>'
  } >"$svg_path"
}

for frame_index in 0 1 2 3 4 5 6 7; do
  frame_number="$(printf '%02d' "$frame_index")"
  svg_path="$frames_dir/frame-${frame_number}.svg"
  png_path="$frames_dir/frame-${frame_number}.png"
  render_svg "$svg_path" "$frame_index"
  "${image_command[@]}" -font "$font_file" "$svg_path" -strip "$png_path"
done

padded_count="$(printf '%03d' "$robot_count")"
video_name="vda5050-fleet-${padded_count}-${release_tag}.mp4"
gif_name="vda5050-fleet-${padded_count}-${release_tag}.gif"
video_path="$output_dir/$video_name"
gif_path="$output_dir/$gif_name"
[[ ! -L "$video_path" && ! -L "$gif_path" ]] || fail "media output must not be a symlink"

video_tmp="$work_dir/output.mp4"
ffmpeg -hide_banner -loglevel error -y \
  -framerate 1 -start_number 0 -i "$frames_dir/frame-%02d.png" \
  -map_metadata -1 -vf 'fps=30,format=yuv420p' \
  -c:v libx264 -preset medium -crf 20 -threads 1 -movflags +faststart -an \
  -metadata creation_time='1970-01-01T00:00:00Z' \
  -metadata comment="fleet_size=${robot_count};focused_agv=demo-001;coordinates=trace_payload;trace_sha256=${trace_digest}" \
  "$video_tmp"

width="$(ffprobe -v error -select_streams v:0 -show_entries stream=width -of csv=p=0 "$video_tmp")"
height="$(ffprobe -v error -select_streams v:0 -show_entries stream=height -of csv=p=0 "$video_tmp")"
duration="$(ffprobe -v error -show_entries format=duration -of csv=p=0 "$video_tmp")"
[[ "$width" == 1280 && "$height" == 720 ]] || fail "video dimensions are not 1280x720"
awk -v duration="$duration" 'BEGIN { exit !(duration >= 7.9 && duration <= 8.1) }' \
  || fail "video duration is outside the 8-second contract"

palette="$work_dir/palette.png"
gif_tmp="$work_dir/output.gif"
ffmpeg -hide_banner -loglevel error -y -i "$video_tmp" \
  -vf 'fps=4,scale=960:540:flags=lanczos,palettegen=max_colors=64:stats_mode=diff' \
  "$palette"
ffmpeg -hide_banner -loglevel error -y -i "$video_tmp" -i "$palette" \
  -lavfi 'fps=4,scale=960:540:flags=lanczos[x];[x][1:v]paletteuse=dither=bayer:bayer_scale=3' \
  -loop 0 "$gif_tmp"

mv -f -- "$video_tmp" "$video_path"
mv -f -- "$gif_tmp" "$gif_path"
printf 'Fleet media: %s\n' "$video_path"
printf 'Fleet animation: %s\n' "$gif_path"
printf 'Fleet size: %s; trace SHA-256: %s\n' "$robot_count" "$trace_digest"
