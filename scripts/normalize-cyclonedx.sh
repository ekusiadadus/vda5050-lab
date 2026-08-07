#!/usr/bin/env bash
set -euo pipefail

sbom_path="${1:?SBOM path is required}"
release_tag="${2:?release tag is required}"
source_commit="${3:?source commit is required}"
canonical_repository="ekusiadadus/vda5050-lab"
repository="${GITHUB_REPOSITORY:-$canonical_repository}"

if [[ ! "$release_tag" =~ ^v(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)$ ]]; then
  printf 'invalid release tag: %s\n' "$release_tag" >&2
  exit 64
fi

if [[ ! "$source_commit" =~ ^[0-9a-f]{40}$ ]]; then
  printf 'invalid source commit: %s\n' "$source_commit" >&2
  exit 64
fi

if [[ "$repository" != "$canonical_repository" ]]; then
  printf 'unexpected repository identity: %s\n' "$repository" >&2
  exit 64
fi

if [[ -L "$sbom_path" || ! -f "$sbom_path" ]]; then
  printf 'SBOM must be a regular non-symlink file: %s\n' "$sbom_path" >&2
  exit 66
fi

sbom_size="$(wc -c < "$sbom_path" | tr -d '[:space:]')"
if [[ ! "$sbom_size" =~ ^[0-9]+$ ]] || (( sbom_size > 16 * 1024 * 1024 )); then
  printf 'SBOM exceeds the 16 MiB attestation limit: %s\n' "$sbom_path" >&2
  exit 65
fi

sha1_stream() {
  if command -v sha1sum >/dev/null 2>&1; then
    sha1sum | awk '{print $1}'
  elif command -v shasum >/dev/null 2>&1; then
    shasum -a 1 | awk '{print $1}'
  else
    printf 'sha1sum or shasum is required\n' >&2
    return 69
  fi
}

sha256_stream() {
  if command -v sha256sum >/dev/null 2>&1; then
    sha256sum | awk '{print $1}'
  elif command -v shasum >/dev/null 2>&1; then
    shasum -a 256 | awk '{print $1}'
  else
    printf 'sha256sum or shasum is required\n' >&2
    return 69
  fi
}

jq -e '
  (type == "object") and
  .bomFormat == "CycloneDX" and
  .specVersion == "1.5" and
  ((.version | type) == "number") and
  (.version >= 1) and
  (.version == (.version | floor)) and
  ((.components | type) == "array") and
  ((.components | length) > 0) and
  all(.components[]; (type == "object") and ((.type | type) == "string") and ((.name | type) == "string"))
' "$sbom_path" >/dev/null

body_digest="$(jq -cS 'del(.serialNumber)' "$sbom_path" | sha256_stream)"
if [[ ! "$body_digest" =~ ^[0-9a-f]{64}$ ]]; then
  printf 'failed to derive canonical SBOM body digest\n' >&2
  exit 70
fi

release_identity="https://github.com/${repository}/attestations/cyclonedx/1.5/${release_tag}/${source_commit}/${body_digest}"
uuid_hex="$({
  printf '\x6b\xa7\xb8\x11\x9d\xad\x11\xd1\x80\xb4\x00\xc0\x4f\xd4\x30\xc8'
  printf '%s' "$release_identity"
} | sha1_stream)"

if [[ ! "$uuid_hex" =~ ^[0-9a-f]{40}$ ]]; then
  printf 'failed to derive UUIDv5 digest\n' >&2
  exit 70
fi

variant_nibble="${uuid_hex:16:1}"
variant_value=$(( (16#$variant_nibble & 3) | 8 ))
printf -v variant_hex '%x' "$variant_value"
serial_number="urn:uuid:${uuid_hex:0:8}-${uuid_hex:8:4}-5${uuid_hex:13:3}-${variant_hex}${uuid_hex:17:3}-${uuid_hex:20:12}"

temporary_path="$(mktemp "${sbom_path}.tmp.XXXXXX")"
cleanup() {
  rm -f -- "$temporary_path"
}
trap cleanup EXIT

jq --arg serial_number "$serial_number" '
  if .bomFormat != "CycloneDX" or .specVersion != "1.5" then
    error("expected a CycloneDX 1.5 document")
  elif (.components | type) != "array" or (.components | length) == 0 then
    error("expected a non-empty components array")
  else
    .serialNumber = $serial_number
  end
' "$sbom_path" > "$temporary_path"

jq -e '
  .bomFormat == "CycloneDX" and
  .specVersion == "1.5" and
  ((.version | type) == "number") and
  (.version >= 1) and
  (.version == (.version | floor)) and
  (.serialNumber | test("^urn:uuid:[0-9a-f]{8}-[0-9a-f]{4}-5[0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$")) and
  ((.components | type) == "array") and
  ((.components | length) > 0) and
  all(.components[]; (type == "object") and ((.type | type) == "string") and ((.name | type) == "string"))
' "$temporary_path" >/dev/null

normalized_size="$(wc -c < "$temporary_path" | tr -d '[:space:]')"
if [[ ! "$normalized_size" =~ ^[0-9]+$ ]] || (( normalized_size > 16 * 1024 * 1024 )); then
  printf 'normalized SBOM exceeds the 16 MiB attestation limit: %s\n' "$sbom_path" >&2
  exit 65
fi

mv -- "$temporary_path" "$sbom_path"
trap - EXIT
