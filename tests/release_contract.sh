#!/usr/bin/env bash
set -euo pipefail

repository_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
release_workflow="$repository_root/.github/workflows/release.yml"
ci_workflow="$repository_root/.github/workflows/ci.yml"
fixture_commit="0000000000000000000000000000000000000000"
rule_catalog="$repository_root/rules/phase1.json"
bundle_manifest="$repository_root/bundles/manifests/vda5050-3.0.0.phase1.bundle.json"
source_manifest="$repository_root/bundles/manifests/vda5050-3.0.0.sources.json"

catalog_digest="$(shasum -a 256 "$rule_catalog" | awk '{print $1}')"
catalog_bytes="$(wc -c < "$rule_catalog" | tr -d ' ')"
test "$catalog_digest" = "$(jq -r '.rule_implementation_digest' "$bundle_manifest")"
test "$catalog_digest" = "$(jq -r '.artifacts["rules/phase1.json"].sha256' "$bundle_manifest")"
test "$catalog_digest" = "$(jq -r '.project_rule_catalog.sha256' "$source_manifest")"
test "$catalog_bytes" = "$(jq -r '.project_rule_catalog.bytes' "$source_manifest")"

grep -Fq 'default: v0.2.0' "$release_workflow"
grep -Fxq '/dist/' "$repository_root/.gitignore"
grep -Fq 'scripts/verify-release-assets.sh' "$ci_workflow"
grep -Fq 'scripts/render-demo-video.sh' "$ci_workflow"
grep -Fq 'make demo-e2e' "$ci_workflow"
grep -Fq 'make demo-e2e' "$release_workflow"
grep -Fq 'make demo-fleet-e2e' "$ci_workflow"
grep -Fq 'demo-fleet-e2e' "$repository_root/Makefile"
for coverage_contract in "$repository_root/Makefile" "$ci_workflow" "$release_workflow"; do
  grep -Fq -- '--all-targets --exclude vda5050-demo --fail-under-lines 80 --fail-under-regions 80' \
    "$coverage_contract"
done

checkout_count="$(grep -c 'uses: actions/checkout@' "$release_workflow")"
# shellcheck disable=SC2016
explicit_ref_count="$(grep -c 'ref: ${{ env.RELEASE_SOURCE_REF }}' "$release_workflow")"
test "$checkout_count" -eq 6
test "$explicit_ref_count" -eq "$checkout_count"
# shellcheck disable=SC2016
grep -Fq 'RELEASE_SOURCE_REF: ${{ github.sha }}' "$release_workflow"
# shellcheck disable=SC2016
test "$(grep -c 'verify-release-tag.sh "$RELEASE_TAG" "$GITHUB_SHA" origin' "$release_workflow")" -eq 2
# shellcheck disable=SC2016
test "$(grep -c 'test "$source_commit" = "$GITHUB_SHA"' "$release_workflow")" -eq 3
# shellcheck disable=SC2016
if grep -Eq '^[[:space:]]+(pattern|path|subject-path): .*\*|gh release create .*\*' "$release_workflow"; then
  printf 'release workflow still uses a wildcard artifact contract\n' >&2
  exit 1
fi
# shellcheck disable=SC2016
grep -Fq 'gh release create "$RELEASE_TAG" "${assets[@]}"' "$release_workflow"
grep -Fq 'name: release-demo' "$release_workflow"
grep -Fq 'librsvg2-bin' "$release_workflow"
grep -Fq 'rsvg-convert --version' "$release_workflow"
test "$(grep -c 'name: release-payload' "$release_workflow")" -eq 2
# shellcheck disable=SC2016
grep -Fq 'make demo-video RELEASE_TAG="$RELEASE_TAG"' "$release_workflow"
# shellcheck disable=SC2016
grep -Fq 'vda5050-fleet-overview-${RELEASE_TAG}.mp4' "$release_workflow"
# shellcheck disable=SC2016
grep -Fq 'bash scripts/verify-release-assets.sh media "$release_dir" "$RELEASE_TAG"' "$release_workflow"
# shellcheck disable=SC2016
grep -Fq 'bash scripts/verify-release-assets.sh assemble "$release_dir" "$RELEASE_TAG"' "$release_workflow"
# shellcheck disable=SC2016
grep -Fq 'bash scripts/verify-release-assets.sh verify "$release_dir" "$RELEASE_TAG"' "$release_workflow"

cyclonedx_block="$(sed -n '/name: Create CycloneDX SBOM attestation/,/name: Create prerelease/p' "$release_workflow")"
# shellcheck disable=SC2016
test "$(grep -c 'vda5050-doctor-${{ env.RELEASE_TAG }}-.*\.tar\.gz' <<<"$cyclonedx_block")" -eq 4
if grep -Fq 'vda5050-demo-' <<<"$cyclonedx_block"; then
  printf 'demo video was incorrectly included in the binary CycloneDX attestation\n' >&2
  exit 1
fi
normalizer_line="$(grep -n 'bash scripts/normalize-cyclonedx.sh' "$release_workflow" | cut -d: -f1)"
upload_line="$(grep -n 'name: Upload SBOM' "$release_workflow" | cut -d: -f1)"
attestation_line="$(grep -n 'name: Create CycloneDX SBOM attestation' "$release_workflow" | cut -d: -f1)"
test "$normalizer_line" -lt "$upload_line"
test "$upload_line" -lt "$attestation_line"

asset_test_root="$(mktemp -d "${TMPDIR:-/tmp}/vda5050-release-assets.XXXXXX")"
asset_release_tag="v9.9.9"
asset_names=(
  "vda5050-doctor-${asset_release_tag}-x86_64-unknown-linux-gnu.tar.gz"
  "vda5050-doctor-${asset_release_tag}-aarch64-unknown-linux-gnu.tar.gz"
  "vda5050-doctor-${asset_release_tag}-aarch64-apple-darwin.tar.gz"
  "vda5050-doctor-${asset_release_tag}-x86_64-pc-windows-msvc.tar.gz"
  "vda5050-doctor-${asset_release_tag}.cdx.json"
)
media_names=()
for robot_count in 001 002 005 010 050 100; do
  media_names+=(
    "vda5050-fleet-${robot_count}-${asset_release_tag}.mp4"
    "vda5050-fleet-${robot_count}-${asset_release_tag}.gif"
  )
done
media_names+=(
  "vda5050-fleet-overview-${asset_release_tag}.mp4"
  "vda5050-fleet-overview-${asset_release_tag}.gif"
)
for media_name in "${media_names[@]}"; do
  case "$media_name" in
    *.mp4) printf '\x00\x00\x00\x18ftypisomfixture' > "$asset_test_root/$media_name" ;;
    *.gif) printf 'GIF89afixture' > "$asset_test_root/$media_name" ;;
  esac
done
bash "$repository_root/scripts/verify-release-assets.sh" media "$asset_test_root" "$asset_release_tag"
for asset_name in "${asset_names[@]}"; do
  printf 'fixture %s\n' "$asset_name" > "$asset_test_root/$asset_name"
done
bash "$repository_root/scripts/verify-release-assets.sh" assemble "$asset_test_root" "$asset_release_tag"
test -f "$asset_test_root/SHA256SUMS"
bash "$repository_root/scripts/verify-release-assets.sh" verify "$asset_test_root" "$asset_release_tag"

printf 'unexpected\n' > "$asset_test_root/not-allowlisted.txt"
if bash "$repository_root/scripts/verify-release-assets.sh" verify "$asset_test_root" "$asset_release_tag"; then
  printf 'non-allowlisted release asset was accepted\n' >&2
  exit 1
fi
rm "$asset_test_root/not-allowlisted.txt"

printf 'tampered\n' >> "$asset_test_root/${asset_names[0]}"
if bash "$repository_root/scripts/verify-release-assets.sh" verify "$asset_test_root" "$asset_release_tag"; then
  printf 'checksum-mismatched release payload was accepted\n' >&2
  exit 1
fi
rm "$asset_test_root/SHA256SUMS"
bash "$repository_root/scripts/verify-release-assets.sh" assemble "$asset_test_root" "$asset_release_tag"

invalid_media="${media_names[0]}"
printf '0000xxxx-invalid-mp4\n' > "$asset_test_root/$invalid_media"
if bash "$repository_root/scripts/verify-release-assets.sh" verify "$asset_test_root" "$asset_release_tag"; then
  printf 'invalid MP4 signature was accepted\n' >&2
  exit 1
fi

rm "$asset_test_root/$invalid_media"
ln -s "${asset_names[0]}" "$asset_test_root/$invalid_media"
if bash "$repository_root/scripts/verify-release-assets.sh" verify "$asset_test_root" "$asset_release_tag"; then
  printf 'symlink demo asset was accepted\n' >&2
  exit 1
fi

rm "$asset_test_root/$invalid_media"
truncate -s 104857601 "$asset_test_root/$invalid_media"
if bash "$repository_root/scripts/verify-release-assets.sh" verify "$asset_test_root" "$asset_release_tag"; then
  printf 'oversized demo asset was accepted\n' >&2
  exit 1
fi

if bash "$repository_root/scripts/verify-release-assets.sh" media "$asset_test_root" 'v9.9.9;unsafe'; then
  printf 'unsafe release tag was accepted by asset verifier\n' >&2
  exit 1
fi
rm -rf "$asset_test_root"

test_root="$(mktemp -d "${TMPDIR:-/tmp}/vda5050-release-contract.XXXXXX")"
trap 'rm -rf "$test_root"' EXIT

remote_repository="$test_root/remote.git"
source_repository="$test_root/source"
checkout_repository="$test_root/checkout"

git init --bare "$remote_repository"
git init "$source_repository"
git -C "$source_repository" config user.name "vda5050-lab test"
git -C "$source_repository" config user.email "test@vda5050-lab.invalid"
git -C "$source_repository" commit --allow-empty -m "release contract fixture"

expected_commit="$(git -C "$source_repository" rev-parse HEAD)"
git -C "$source_repository" tag -a v9.9.9 -m "annotated fixture"
git -C "$source_repository" tag v9.9.8
git -C "$source_repository" remote add origin "$remote_repository"
git -C "$source_repository" push origin HEAD:refs/heads/main refs/tags/v9.9.9 refs/tags/v9.9.8

git clone "$remote_repository" "$checkout_repository"
git -C "$checkout_repository" checkout --detach "$expected_commit"

(
  cd "$checkout_repository"
  bash "$repository_root/scripts/verify-release-tag.sh" v9.9.9 "$expected_commit"
  bash "$repository_root/scripts/verify-release-tag.sh" v9.9.9 "$expected_commit" origin
)

git -C "$source_repository" commit --allow-empty -m "moved tag fixture"
git -C "$source_repository" tag -f -a v9.9.9 -m "moved annotated fixture"
git -C "$source_repository" push --force origin refs/tags/v9.9.9
if (
  cd "$checkout_repository"
  bash "$repository_root/scripts/verify-release-tag.sh" v9.9.9 "$expected_commit" origin
); then
  printf 'remote moved release tag was accepted\n' >&2
  exit 1
fi
git -C "$checkout_repository" push --force origin refs/tags/v9.9.9

if (
  cd "$checkout_repository"
  bash "$repository_root/scripts/verify-release-tag.sh" v9.9.8 "$expected_commit"
); then
  printf 'lightweight release tag was accepted\n' >&2
  exit 1
fi

if (
  cd "$checkout_repository"
  bash "$repository_root/scripts/verify-release-tag.sh" v9.9.9 0000000000000000000000000000000000000000
); then
  printf 'mismatched release commit was accepted\n' >&2
  exit 1
fi

git -C "$checkout_repository" fetch --force --no-tags origin "+$expected_commit:refs/tags/v9.9.9"
test "$(git -C "$checkout_repository" cat-file -t refs/tags/v9.9.9)" = "commit"

if (
  cd "$checkout_repository"
  bash "$repository_root/scripts/verify-release-tag.sh" v9.9.9 "$expected_commit"
); then
  printf 'checkout-rewritten release tag was accepted\n' >&2
  exit 1
fi

sbom_path="$test_root/SBOM fixture.json"
printf '%s\n' '{"bomFormat":"CycloneDX","specVersion":"1.5","version":1,"components":[{"type":"application","name":"fixture","version":"9.9.9"}]}' > "$sbom_path"
bash "$repository_root/scripts/normalize-cyclonedx.sh" "$sbom_path" v9.9.9 "$fixture_commit"
first_serial_number="$(jq -r '.serialNumber' "$sbom_path")"
test "$first_serial_number" = "urn:uuid:8d58485c-01be-5ca6-839c-31e2adce77cd"
first_sbom_digest="$(shasum -a 256 "$sbom_path" | awk '{print $1}')"

jq -e '
  .bomFormat == "CycloneDX" and
  .specVersion == "1.5" and
  (.serialNumber | test("^urn:uuid:[0-9a-f]{8}-[0-9a-f]{4}-5[0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$")) and
  ((.components | length) == 1)
' "$sbom_path" >/dev/null

bash "$repository_root/scripts/normalize-cyclonedx.sh" "$sbom_path" v9.9.9 "$fixture_commit"
test "$(jq -r '.serialNumber' "$sbom_path")" = "$first_serial_number"
test "$(shasum -a 256 "$sbom_path" | awk '{print $1}')" = "$first_sbom_digest"

if bash "$repository_root/scripts/normalize-cyclonedx.sh" "$sbom_path" 'v9.9.9;echo injected' "$fixture_commit"; then
  printf 'unsafe release tag was accepted by SBOM normalizer\n' >&2
  exit 1
fi

malformed_sbom="$test_root/malformed.json"
printf '%s\n' '{"bomFormat":"CycloneDX","specVersion":"1.4","components":[]}' > "$malformed_sbom"
if bash "$repository_root/scripts/normalize-cyclonedx.sh" "$malformed_sbom" v9.9.9 "$fixture_commit"; then
  printf 'invalid CycloneDX document was accepted\n' >&2
  exit 1
fi

symlink_sbom="$test_root/symlink.json"
ln -s "$sbom_path" "$symlink_sbom"
if bash "$repository_root/scripts/normalize-cyclonedx.sh" "$symlink_sbom" v9.9.9 "$fixture_commit"; then
  printf 'symlink SBOM was accepted\n' >&2
  exit 1
fi

wrong_repository_sha="$(shasum -a 256 "$sbom_path" | awk '{print $1}')"
if GITHUB_REPOSITORY='other/repository' bash "$repository_root/scripts/normalize-cyclonedx.sh" "$sbom_path" v9.9.9 "$fixture_commit"; then
  printf 'unexpected repository identity was accepted\n' >&2
  exit 1
fi
test "$(shasum -a 256 "$sbom_path" | awk '{print $1}')" = "$wrong_repository_sha"

oversized_sbom="$test_root/oversized.json"
truncate -s 16777217 "$oversized_sbom"
oversized_digest="$(shasum -a 256 "$oversized_sbom" | awk '{print $1}')"
if bash "$repository_root/scripts/normalize-cyclonedx.sh" "$oversized_sbom" v9.9.9 "$fixture_commit"; then
  printf 'oversized SBOM was accepted\n' >&2
  exit 1
fi
test "$(shasum -a 256 "$oversized_sbom" | awk '{print $1}')" = "$oversized_digest"
test -z "$(find "$test_root" -maxdepth 1 -name 'oversized.json.tmp.*' -print -quit)"

changed_sbom="$test_root/changed.json"
printf '%s\n' '{"bomFormat":"CycloneDX","specVersion":"1.5","version":1,"components":[{"type":"application","name":"changed","version":"9.9.9"}]}' > "$changed_sbom"
bash "$repository_root/scripts/normalize-cyclonedx.sh" "$changed_sbom" v9.9.9 "$fixture_commit"
test "$(jq -r '.serialNumber' "$changed_sbom")" != "$first_serial_number"

changed_commit_sbom="$test_root/changed-commit.json"
printf '%s\n' '{"bomFormat":"CycloneDX","specVersion":"1.5","version":1,"components":[{"type":"application","name":"fixture","version":"9.9.9"}]}' > "$changed_commit_sbom"
bash "$repository_root/scripts/normalize-cyclonedx.sh" "$changed_commit_sbom" v9.9.9 1111111111111111111111111111111111111111
test "$(jq -r '.serialNumber' "$changed_commit_sbom")" != "$first_serial_number"
