#!/usr/bin/env bash
set -euo pipefail

repository_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
release_workflow="$repository_root/.github/workflows/release.yml"
fixture_commit="0000000000000000000000000000000000000000"

checkout_count="$(grep -c 'uses: actions/checkout@' "$release_workflow")"
# shellcheck disable=SC2016
explicit_ref_count="$(grep -c 'ref: ${{ env.RELEASE_SOURCE_REF }}' "$release_workflow")"
test "$checkout_count" -eq 4
test "$explicit_ref_count" -eq "$checkout_count"
# shellcheck disable=SC2016
grep -Fq 'RELEASE_SOURCE_REF: ${{ github.sha }}' "$release_workflow"
# shellcheck disable=SC2016
test "$(grep -c 'verify-release-tag.sh "$RELEASE_TAG" "$GITHUB_SHA" origin' "$release_workflow")" -eq 2
# shellcheck disable=SC2016
test "$(grep -c 'test "$source_commit" = "$GITHUB_SHA"' "$release_workflow")" -eq 2
# shellcheck disable=SC2016
if grep -Fq 'gh release create "$RELEASE_TAG" dist/*' "$release_workflow"; then
  printf 'release publication still uses a wildcard payload\n' >&2
  exit 1
fi
# shellcheck disable=SC2016
grep -Fq 'gh release create "$RELEASE_TAG" "${assets[@]}"' "$release_workflow"
normalizer_line="$(grep -n 'bash scripts/normalize-cyclonedx.sh' "$release_workflow" | cut -d: -f1)"
upload_line="$(grep -n 'name: Upload SBOM' "$release_workflow" | cut -d: -f1)"
attestation_line="$(grep -n 'name: Create CycloneDX SBOM attestation' "$release_workflow" | cut -d: -f1)"
test "$normalizer_line" -lt "$upload_line"
test "$upload_line" -lt "$attestation_line"

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
