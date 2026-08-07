#!/usr/bin/env bash
set -euo pipefail

repository_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
release_workflow="$repository_root/.github/workflows/release.yml"

checkout_count="$(grep -c 'uses: actions/checkout@' "$release_workflow")"
# shellcheck disable=SC2016
explicit_ref_count="$(grep -c 'ref: ${{ env.RELEASE_SOURCE_REF }}' "$release_workflow")"
test "$checkout_count" -eq 4
test "$explicit_ref_count" -eq "$checkout_count"
grep -Fq "RELEASE_SOURCE_REF: \${{ github.event_name == 'push' && github.ref || github.sha }}" "$release_workflow"

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
)

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
