#!/usr/bin/env bash
set -euo pipefail

usage() {
  printf 'usage: %s media|assemble|verify RELEASE_DIR vX.Y.Z\n' "$0" >&2
  exit 64
}

test "$#" -eq 3 || usage

mode="$1"
release_dir="$2"
release_tag="$3"

case "$mode" in
  media | assemble | verify) ;;
  *) usage ;;
esac

if [[ ! "$release_tag" =~ ^v[0-9]+\.[0-9]+\.[0-9]+$ ]]; then
  printf 'invalid release tag: %s\n' "$release_tag" >&2
  exit 65
fi
if [[ ! -d "$release_dir" || -L "$release_dir" ]]; then
  printf 'release directory must be a non-symlink directory: %s\n' "$release_dir" >&2
  exit 66
fi

software_names=(
  "vda5050-doctor-${release_tag}-x86_64-unknown-linux-gnu.tar.gz"
  "vda5050-doctor-${release_tag}-aarch64-unknown-linux-gnu.tar.gz"
  "vda5050-doctor-${release_tag}-aarch64-apple-darwin.tar.gz"
  "vda5050-doctor-${release_tag}-x86_64-pc-windows-msvc.tar.gz"
  "vda5050-doctor-${release_tag}.cdx.json"
)
media_names=()
for robot_count in 001 002 005 010 050 100; do
  media_names+=(
    "vda5050-fleet-${robot_count}-${release_tag}.mp4"
    "vda5050-fleet-${robot_count}-${release_tag}.gif"
  )
done
media_names+=(
  "vda5050-fleet-overview-${release_tag}.mp4"
  "vda5050-fleet-overview-${release_tag}.gif"
)
payload_names=("${software_names[@]}" "${media_names[@]}")

case "$mode" in
  media) expected_count="${#media_names[@]}" ;;
  assemble) expected_count="${#payload_names[@]}" ;;
  verify) expected_count="$((${#payload_names[@]} + 1))" ;;
esac

actual_count="$(find "$release_dir" -mindepth 1 -maxdepth 1 -print | wc -l | tr -d ' ')"
if [[ "$actual_count" -ne "$expected_count" ]]; then
  printf 'release directory contains %s entries; expected %s\n' "$actual_count" "$expected_count" >&2
  exit 67
fi

if [[ "$mode" == "media" ]]; then
  required_names=("${media_names[@]}")
else
  required_names=("${payload_names[@]}")
fi
for name in "${required_names[@]}"; do
  path="$release_dir/$name"
  if [[ ! -f "$path" || -L "$path" ]]; then
    printf 'release payload is missing, non-regular, or a symlink: %s\n' "$name" >&2
    exit 68
  fi
done

for media_name in "${media_names[@]}"; do
  media_path="$release_dir/$media_name"
  media_size="$(wc -c < "$media_path" | tr -d ' ')"
  case "$media_name" in
    *.mp4)
      if [[ "$media_size" -lt 12 || "$media_size" -gt 104857600 ]]; then
        printf 'MP4 size is outside the 12-byte to 100-MiB contract: %s (%s bytes)\n' \
          "$media_name" "$media_size" >&2
        exit 69
      fi
      if [[ "$(LC_ALL=C dd if="$media_path" bs=1 skip=4 count=4 2>/dev/null)" != "ftyp" ]]; then
        printf 'MP4 does not have an ISO BMFF ftyp signature: %s\n' "$media_name" >&2
        exit 70
      fi
      ;;
    *.gif)
      if [[ "$media_size" -lt 10 || "$media_size" -gt 52428800 ]]; then
        printf 'GIF size is outside the 10-byte to 50-MiB contract: %s (%s bytes)\n' \
          "$media_name" "$media_size" >&2
        exit 69
      fi
      gif_signature="$(LC_ALL=C dd if="$media_path" bs=1 count=6 2>/dev/null)"
      if [[ "$gif_signature" != "GIF87a" && "$gif_signature" != "GIF89a" ]]; then
        printf 'GIF does not have a recognized signature: %s\n' "$media_name" >&2
        exit 70
      fi
      ;;
  esac
done
if [[ "$mode" == "media" ]]; then
  exit 0
fi

checksum_file="$release_dir/SHA256SUMS"
if [[ "$mode" == "assemble" ]]; then
  (
    cd "$release_dir"
    if command -v sha256sum >/dev/null 2>&1; then
      sha256sum "${payload_names[@]}" > SHA256SUMS
    else
      shasum -a 256 "${payload_names[@]}" > SHA256SUMS
    fi
  )
fi

if [[ ! -f "$checksum_file" || -L "$checksum_file" ]]; then
  printf 'SHA256SUMS is missing, non-regular, or a symlink\n' >&2
  exit 71
fi

checksum_index=0
while read -r digest name extra; do
  if [[ -n "${extra:-}" || ! "$digest" =~ ^[0-9a-f]{64}$ ]]; then
    printf 'invalid SHA256SUMS record at index %s\n' "$checksum_index" >&2
    exit 72
  fi
  if [[ "$checksum_index" -ge "${#payload_names[@]}" || "$name" != "${payload_names[$checksum_index]}" ]]; then
    printf 'SHA256SUMS does not match the ordered release allowlist\n' >&2
    exit 73
  fi
  checksum_index="$((checksum_index + 1))"
done < "$checksum_file"
if [[ "$checksum_index" -ne "${#payload_names[@]}" ]]; then
  printf 'SHA256SUMS contains %s records; expected %s\n' "$checksum_index" "${#payload_names[@]}" >&2
  exit 74
fi

(
  cd "$release_dir"
  if command -v sha256sum >/dev/null 2>&1; then
    sha256sum --check --strict SHA256SUMS
  else
    shasum -a 256 --check SHA256SUMS
  fi
)
