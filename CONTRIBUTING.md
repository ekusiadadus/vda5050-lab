# Contributing

Contributions should make incident diagnosis more evidence-bounded, not merely
add another unqualified warning.

## Development workflow

1. Install Rust 1.97.1, `cargo-audit 0.22.2`, and `cargo-deny 0.20.2` as
   documented in the README.
2. Create a focused branch.
3. Add a failing test before changing behavior.
4. Run `make ci`.
5. Describe the proof boundary and any remaining INCONCLUSIVE path in the PR.

Never commit customer traces or reversible production data. Fixtures must be
synthetic, independently authorized for publication, or irreversibly minimized
under an approved handling process.

## Rule contributions

A protocol rule must include:

- a stable rule ID and applicable VDA version;
- authority, exact section, obligation, and normative subject;
- antecedent and required observation vantage;
- PASS, FAIL, INCONCLUSIVE, NOT_APPLICABLE, and UNKNOWN handling where
  meaningful;
- actor, time, completeness, and identity proof requirements;
- minimized positive, negative, and missing-evidence fixtures; and
- a catalog update whose digest is propagated to the source manifests.

GitHub issues, comments, and proposals are advisory context. They cannot
override the published VDA PDF unless they qualify as a formally published
erratum under the project's authority policy.

## Pull requests

Keep changes reviewable. Update the changelog for user-visible behavior, avoid
new network-capable runtime dependencies, and explain every new dependency.
The code must remain free of unsafe Rust and pass strict Clippy with warnings
denied.
