#!/usr/bin/env bash
set -euo pipefail

release_tag="${1:?release tag is required}"
expected_commit="${2:?expected commit is required}"

if [[ ! "$release_tag" =~ ^v[0-9]+\.[0-9]+\.[0-9]+$ ]]; then
  printf 'invalid release tag: %s\n' "$release_tag" >&2
  exit 64
fi

if [[ ! "$expected_commit" =~ ^[0-9a-f]{40}$ ]]; then
  printf 'invalid expected commit: %s\n' "$expected_commit" >&2
  exit 64
fi

test "$(git cat-file -t "refs/tags/$release_tag")" = "tag"
test "$(git rev-parse "refs/tags/$release_tag^{commit}")" = "$expected_commit"
test "$(git rev-parse 'HEAD^{commit}')" = "$expected_commit"
