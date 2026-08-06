# Protocol sources and authority boundary

Status: provenance snapshot for VDA 5050 3.0.0, retrieved 2026-08-06.

This document records exactly which protocol bytes the Phase 1 rule catalog was
reviewed against. It does not claim that the current CLI has loaded the source
bundle, and it does not turn GitHub discussions into protocol requirements.

## Normative baseline

The baseline is the official VDA publication, **Interface for the Communication
between Mobile Robots and a Fleet Control**, version 3.0.0:

- publication page: <https://www.vda.de/en/news/publications/publication/vda-5050>
- PDF: <https://www.vda.de/dam/jcr%3A09f03b91-13e2-4db3-bf30-4f221710071b/VDA5050_EN.pdf>
- SHA-256: `becf70e4c4a97db4058464d333ef9f7492a32007577dda70fdc433de045dab20`
- retrieved size: 3,543,547 bytes
- rendered page count: 103
- PDF metadata creation time: 2026-04-17 22:21:10 +09:00
- retrieval date: 2026-08-06

The digest identifies the bytes retrieved over HTTPS on that date. It is not a
publisher signature or a claim that the URL can never serve different bytes.
The PDF is deliberately **not vendored** in this repository. Its publication
and redistribution terms must not be inferred from the GitHub repository's
license.

## Tagged source snapshot

The official GitHub repository was fixed to an immutable tag and commit:

| Field | Fixed value |
| --- | --- |
| Repository | <https://github.com/VDA5050/VDA5050> |
| Tag | `3.0.0` |
| Full commit | `e9ba560b0e2d3f66526550ad5d61b8a2ad936172` |
| Commit time | `2026-03-18T20:06:30+01:00` |
| `VDA5050_EN.md` SHA-256 | `e759665ee62953a380961580d6b0d1bf4fd3dfe9d22a60f142af1456aca6ccde` |
| `VDA5050_EN.md` size | 208,085 bytes |
| `README.md` SHA-256 | `e0d42c1a9bbd2df70119730e1f502c5d836fae32f6068a6fa549bbd94bc495ba` |
| `LICENSE.txt` SHA-256 | `e1a34a010d5572888c82ea7e9b7ee134517ef061159fdac8352e9a7c3acdd2dc` |

The tagged README explicitly makes the published VDA PDF controlling when it
differs from the Markdown or JSON Schemas. Therefore the Markdown is a
navigation and review aid, not a higher authority than the PDF.

## Released schema snapshot

The released schemas declare JSON Schema draft 2020-12. The `zoneSet` file uses
an `http` dialect URI while the other seven use `https`; the source manifest
records this exactly and does not rewrite the official bytes.

| Schema | SHA-256 | Bytes |
| --- | --- | ---: |
| `connection.schema` | `083dffd157e1af2d365e24b73dfc18e404addb3e1e46dd4c7fbab63cd777cafb` | 2,663 |
| `factsheet.schema` | `d2fd651dc496c4cab34c35b43a3d1293629c3cce954fae71ef6aebbda67991ec` | 46,059 |
| `instantActions.schema` | `230bfcab4617fb9e8b93c1b9c18c99da43b1c80619e3a1ecc8daa926cd792450` | 5,275 |
| `order.schema` | `f7e2d3b5c914e76d28d39e3e5a08b6997e56e9c4cbcceb634f13fe3573986d12` | 18,231 |
| `responses.schema` | `da6297b06f278ddca3bfd3c0dec982e3e62d2cf635e0cdb2dea93d090504aad8` | 2,707 |
| `state.schema` | `f00d6620e0d1d20efa99f8ce3bd1a25a578f0c01e96cc9b52548259f4847d307` | 34,506 |
| `visualization.schema` | `858eeab4758a5891e1620a8df333e47f341f61dd8d683a41bb0293217ddafc92` | 9,955 |
| `zoneSet.schema` | `d9a2d8a2f0d7300cc6d9158b1708b7f7d08fdefd5415b1991fbeebab08a9b402` | 14,711 |

The schemas are syntax sources. They do not replace behavioral clauses in the
PDF. Remote `$ref` resolution is denied by project policy. A validator identity
and `format` assertion mode must be selected and recorded before schema results
can be treated as reproducible; those choices are not yet implemented.

## Authority and errata policy

The evaluator must apply this resolution policy:

1. Start with the published PDF as the base normative authority.
2. Apply an override only when VDA separately publishes a formal erratum that
   names the affected protocol version, exact clause, old text, and replacement
   text. The overlay applies only to that scope.
3. Use released schemas for syntax validation. A prose conflict is resolved in
   favor of the PDF unless a qualifying formal erratum changes the clause.
4. Use tagged Markdown as a traceable mirror and navigation aid.
5. Treat issues, pull requests, comments, answers, labels, milestones, and
   acknowledged/proposed changes as non-normative context.

No formal erratum was recognized in this snapshot. This means the runtime
bundle's `errata` array is intentionally empty, not that no GitHub discussion or
future correction exists.

## Phase 1 rule authority

[`rules/phase1.json`](../rules/phase1.json) is the machine-readable catalog. It
separates VDA conformance rules from project diagnostics:

- D1 sender hygiene and semantic uncertainty are project diagnostics. Section
  6.1.4.5 assigns the changed-duplicate response obligation to the mobile robot,
  not a corresponding sender-side `SHALL` to fleet control.
- D1 mobile-robot response uses PDF section 6.1.4.5 and expects
  `SAME_ORDER_UPDATE_ID` at protocol level `WARNING` when all applicability
  preconditions are proven.
- D2 sequence continuity uses PDF section 6.1.1 and is attributed to fleet
  control only when the order publisher is proven.
- D3 is a project diagnostic informed by sections 6.6.3 and 7.8. The catalog
  does not invent a universal response deadline.
- D4 uses PDF section 6.5 only after a reconnect/new connection epoch is
  independently established; `OFFLINE` alone does not prove that the robot came
  online again.
- D5 uses PDF section 6.1.3 but the first implementation evaluates only a
  bounded subset of the full cancellation state machine.

The D1 response and D4 reconnect entries are `IMPLEMENTED_GUARDED`. D1 requires
a trusted mobile-robot state proving the current order/update before the repeat;
otherwise applicability is `UNKNOWN` and the verdict is `UNEVALUATED`. D4
requires the same known participant and a changed participant connection epoch;
without that independent reconnect evidence the verdict remains
`INCONCLUSIVE`. These guards keep incomplete traces from becoming normative
failures.

## Machine-readable manifests

Two files serve different purposes:

- [`vda5050-3.0.0.sources.json`](../bundles/manifests/vda5050-3.0.0.sources.json)
  contains retrieval, licensing-boundary, authority, size, and digest metadata.
- [`vda5050-3.0.0.phase1.bundle.json`](../bundles/manifests/vda5050-3.0.0.phase1.bundle.json)
  matches the Rust `BundleManifest` wire shape and names the content-addressed
  objects required by the verifier.

The runtime manifest is not itself a verified bundle. A local bundle root must
contain each permitted artifact as `objects/<sha256>`, and `VerifiedBundle`
must verify every object before evaluation. The repository does not provide the
PDF object. The current `diagnose` command does not yet accept or verify this
bundle, so the manifest is provenance infrastructure rather than runtime proof.

## Reproduction checklist

Use a fresh temporary directory and preserve raw command output in an audit
record:

```sh
git clone --filter=blob:none --branch 3.0.0 \
  https://github.com/VDA5050/VDA5050.git VDA5050-3.0.0
git -C VDA5050-3.0.0 rev-parse HEAD
shasum -a 256 VDA5050-3.0.0/VDA5050_EN.md
shasum -a 256 VDA5050-3.0.0/json_schemas/*.schema
curl --fail --location --proto '=https' \
  --output VDA5050_EN.pdf \
  'https://www.vda.de/dam/jcr%3A09f03b91-13e2-4db3-bf30-4f221710071b/VDA5050_EN.pdf'
shasum -a 256 VDA5050_EN.pdf
pdfinfo VDA5050_EN.pdf
```

Do not replace the full commit with `main`, a moving branch, or a shortened
commit ID. A hash mismatch is a new source snapshot that requires review; it is
not something an importer may silently accept.
