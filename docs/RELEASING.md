# Release procedure

Release artifacts are built from annotated tags that this project treats as
append-only. The current release line is a Developer Preview, not a
public-alpha evidence claim.

Do not confuse that operating rule with GitHub's server-side release
immutability. GitHub reports `v0.1.2` as `isImmutable: false`; consumers must
verify checksums and attestations. Enabling repository release immutability is
a [separate GitHub setting](https://docs.github.com/code-security/how-tos/secure-your-supply-chain/establish-provenance-and-integrity/prevent-release-changes)
and applies only to future releases.

## Preflight

1. Confirm `Cargo.toml`, `Cargo.lock`, `CHANGELOG.md`, and release notes agree,
   including the `vda5050-doctor` version and all fourteen fleet-media names.
2. Confirm the rule catalog digest matches both manifests.
3. Install the pinned release tools listed in the README, then run
   `make release-check` from a clean checkout. This includes the disposable
   Tier 1 pre-connect, synthetic FAIL, passive INCONCLUSIVE, and control
   no-D4-finding checks.
4. Run `make demo-video RELEASE_TAG=vX.Y.Z`. Require the isolated six-run fleet
   suite and media renderer to finish successfully, then validate all fourteen
   outputs with
   `bash scripts/verify-release-assets.sh media dist vX.Y.Z`.
5. Review `git diff --check`, the secret scan, and normal dependency graph.
6. Confirm no customer trace, generated report, generated MP4, or generated GIF
   is tracked.

## Publish

1. Commit the complete reviewed tree.
2. Push `main` and wait for CI.
3. Run the Release workflow manually on `main` with the intended tag value.
   This runs the contract suite natively, builds and packages all four targets,
   renders the isolated fleet media suite, and assembles the exact payload without
   publishing.
4. Require every dry-run build, SBOM, demo, and payload-assembly job to succeed.
5. Create an annotated tag: `git tag -a vX.Y.Z -m "vX.Y.Z"`.
6. Push only that tag.
7. The tag workflow re-runs quality gates, builds four native archives,
   generates a CycloneDX SBOM and fourteen media files, assembles an exact-name allowlist,
   generates checksums, creates Sigstore-backed GitHub attestations, and creates
   a prerelease from `docs/releases/vX.Y.Z.md`.

## Verify

1. Require every release workflow job to succeed.
2. Download all twenty assets and verify the nineteen checksummed payloads in
   `SHA256SUMS`.
3. Run `--version` and one synthetic diagnosis from a release archive.
4. Verify the default SLSA attestation for every archive and all fourteen media
   files. Verify the CycloneDX predicate only for the four Doctor archives.
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
payload assembly job. It downloads four native archives, one SBOM, and one
fourteen-file fleet-media artifact by six explicit artifact names. It rejects
symlinks, unexpected entries, invalid MP4/GIF signatures, MP4 files larger than
100 MiB, and GIF files larger than 50 MiB, then writes checksums in a fixed
allowlist order and applies the same CycloneDX discriminator used by the pinned
attestation action. The resulting `release-payload` artifact contains exactly
those nineteen payloads plus `SHA256SUMS`; the dry-run does not create
attestations or a release.

The release workflow does not use wildcard artifact downloads, upload paths,
attestation subjects, or `gh release` arguments. The default SLSA attestation
uses `SHA256SUMS` and therefore covers all fourteen fleet-media files as well as
the five software payloads. The CycloneDX document describes the Doctor binary graph, so its
attestation intentionally names only the four Doctor archives. Treating the
video as a CycloneDX subject would overstate the SBOM's scope.

The 80% line and region floors apply to the offline Doctor and supporting
libraries. `vda5050-demo` is deliberately excluded from the LLVM source-coverage
denominator because its MQTT and process-lifecycle behavior is exercised through
the Docker broker boundary. It is not untested: `make demo-e2e` is mandatory in
CI, local `release-check`, and the release demo job, and covers failed
pre-connect, synthetic FAIL, passive INCONCLUSIVE, and control behavior.

After publication, verify every archive twice: once with the default SLSA
predicate and once with `--predicate-type https://cyclonedx.org/bom`. In both
commands pin `--signer-workflow` to
`ekusiadadus/vda5050-lab/.github/workflows/release.yml` and `--source-ref` to
the exact annotated tag ref. For automated policy, also pin `--source-digest`
to the tag's peeled 40-character commit.
