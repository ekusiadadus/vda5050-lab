#!/usr/bin/env bash
set -euo pipefail

repository_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
release_workflow="$repository_root/.github/workflows/release.yml"
asset_verifier="$repository_root/scripts/verify-release-assets.sh"
release_tag="v9.9.9"
counts=(001 002 005 010 050 100)

grep -Fq 'bash tests/fleet_media_contract.sh' "$release_workflow"
# shellcheck disable=SC2016
grep -Fq 'bash scripts/verify-release-assets.sh media "$release_dir" "$RELEASE_TAG"' "$release_workflow"
# shellcheck disable=SC2016
grep -Fq 'vda5050-fleet-overview-${{ env.RELEASE_TAG }}.mp4' "$release_workflow"
# shellcheck disable=SC2016
grep -Fq 'vda5050-fleet-overview-${{ env.RELEASE_TAG }}.gif' "$release_workflow"

for count in "${counts[@]}"; do
  grep -Fq "vda5050-fleet-${count}-\${{ env.RELEASE_TAG }}.mp4" "$release_workflow"
  grep -Fq "vda5050-fleet-${count}-\${{ env.RELEASE_TAG }}.gif" "$release_workflow"
  test "$(grep -Ec "vda5050-fleet-${count}-.*\\.mp4" "$release_workflow")" -eq 3
  test "$(grep -Ec "vda5050-fleet-${count}-.*\\.gif" "$release_workflow")" -eq 3
done
test "$(grep -Ec 'vda5050-fleet-overview-.*\.mp4' "$release_workflow")" -eq 3
test "$(grep -Ec 'vda5050-fleet-overview-.*\.gif' "$release_workflow")" -eq 3
grep -Fq 'exactly twenty assets' "$repository_root/docs/releases/v0.2.0.md"
grep -Fq 'nineteen checksummed payloads' "$repository_root/docs/RELEASING.md"

test_root="$(mktemp -d "${TMPDIR:-/tmp}/vda5050-fleet-release.XXXXXX")"
trap 'rm -rf -- "$test_root"' EXIT

media_names=()
for count in "${counts[@]}"; do
  media_names+=(
    "vda5050-fleet-${count}-${release_tag}.mp4"
    "vda5050-fleet-${count}-${release_tag}.gif"
  )
done
media_names+=(
  "vda5050-fleet-overview-${release_tag}.mp4"
  "vda5050-fleet-overview-${release_tag}.gif"
)

for media_name in "${media_names[@]}"; do
  case "$media_name" in
    *.mp4) printf '\x00\x00\x00\x18ftypisomfixture' >"$test_root/$media_name" ;;
    *.gif) printf 'GIF89afixture' >"$test_root/$media_name" ;;
  esac
done
test "$(find "$test_root" -mindepth 1 -maxdepth 1 -type f | wc -l | tr -d ' ')" -eq 14
bash "$asset_verifier" media "$test_root" "$release_tag"

software_names=(
  "vda5050-doctor-${release_tag}-x86_64-unknown-linux-gnu.tar.gz"
  "vda5050-doctor-${release_tag}-aarch64-unknown-linux-gnu.tar.gz"
  "vda5050-doctor-${release_tag}-aarch64-apple-darwin.tar.gz"
  "vda5050-doctor-${release_tag}-x86_64-pc-windows-msvc.tar.gz"
  "vda5050-doctor-${release_tag}.cdx.json"
)
for software_name in "${software_names[@]}"; do
  printf 'fixture %s\n' "$software_name" >"$test_root/$software_name"
done

bash "$asset_verifier" assemble "$test_root" "$release_tag"
test "$(wc -l <"$test_root/SHA256SUMS" | tr -d ' ')" -eq 19
test "$(find "$test_root" -mindepth 1 -maxdepth 1 -type f | wc -l | tr -d ' ')" -eq 20
bash "$asset_verifier" verify "$test_root" "$release_tag"

printf 'unexpected\n' >"$test_root/not-allowlisted.bin"
if bash "$asset_verifier" verify "$test_root" "$release_tag"; then
  printf 'fleet release verifier accepted an extra asset\n' >&2
  exit 1
fi

printf 'Fleet release allowlist verified: 14 media, 19 payloads, 20 assets.\n'
