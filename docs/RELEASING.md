# Release procedure

Releases are immutable, annotated-tag builds. The current release line is a
Developer Preview, not a public-alpha evidence claim.

## Preflight

1. Confirm `Cargo.toml`, `Cargo.lock`, `CHANGELOG.md`, and release notes agree.
2. Confirm the rule catalog digest matches both manifests.
3. Install the pinned release tools listed in the README, then run
   `make release-check` from a clean checkout.
4. Review `git diff --check`, the secret scan, and normal dependency graph.
5. Confirm no customer trace or generated report is tracked.

## Publish

1. Commit the complete reviewed tree.
2. Push `main` and wait for CI.
3. Run the Release workflow manually on `main` with the intended tag value.
   This runs the contract suite natively, builds, and packages all four targets
   without publishing.
4. Require every dry-run build and SBOM job to succeed.
5. Create an annotated tag: `git tag -a vX.Y.Z -m "vX.Y.Z"`.
6. Push only that tag.
7. The tag workflow re-runs quality gates, builds four native archives,
   generates a CycloneDX SBOM and checksums, creates Sigstore-backed GitHub
   attestations, and creates a prerelease from `docs/releases/vX.Y.Z.md`.

## Verify

1. Require every release workflow job to succeed.
2. Download every asset and verify `SHA256SUMS`.
3. Run `--version` and one synthetic diagnosis from a release archive.
4. Verify the attestation with `gh attestation verify`.
5. Confirm the GitHub release is marked prerelease and not latest.

Do not move or recreate a published tag. Correct a defective release with a new
patch version and a changelog entry.

The `v0.1.0` tag exercised this rule: its workflow stopped before publishing
when the checkout action rewrote the local annotated-tag ref. The tag remains
unchanged, and `v0.1.1` passes the tag ref explicitly to every checkout so the
annotated object remains available for validation.

The `v0.1.1` tag also remains unchanged. Its workflow completed the release
contract, all native builds, checksums, and SLSA provenance, then stopped
before publication because the CycloneDX generator omitted the `serialNumber`
required by the pinned attestation action. `v0.1.2` deterministically adds and
validates that field before the SBOM leaves the generator job.

The normalizer is a discriminator, identity, size, and structural guard around
the pinned generator output; it is not a general CycloneDX schema validator.
It runs only on a freshly generated file inside the single-writer GitHub-hosted
runner workspace. SHA-256 checksums and Sigstore attestations, not UUIDv5,
provide integrity and authenticity.

Before tagging, the manual release dry-run must also pass the unconditional
payload assembly job. It downloads exactly four native archives and one SBOM,
recomputes all checksums, and applies the same CycloneDX discriminator used by
the pinned attestation action. It does not create attestations or a release.

After publication, verify every archive twice: once with the default SLSA
predicate and once with `--predicate-type https://cyclonedx.org/bom`. In both
commands pin `--signer-workflow` to
`ekusiadadus/vda5050-lab/.github/workflows/release.yml` and `--source-ref` to
the exact annotated tag ref. For automated policy, also pin `--source-digest`
to the tag's peeled 40-character commit.
