# Synthetic traces

These traces are invented test data. They contain no customer, warehouse, or
robot-fleet information.

Run the primary example from the repository root:

```sh
make diagnose-example
```

Or exercise each import/report path directly:

```sh
cargo run --locked --bin vda5050-doctor -- diagnose fixtures/synthetic/repeated-order-changed.jsonl --vda-version 3.0.0 --input-format envelope-jsonl --format json
cargo run --locked --bin vda5050-doctor -- diagnose fixtures/synthetic/reconnect-without-online.json --vda-version 3.0.0 --input-format envelope-array --format json
cargo run --locked --bin vda5050-doctor -- diagnose fixtures/synthetic/malformed-record.jsonl --vda-version 3.0.0 --input-format envelope-jsonl --format json
```

Expected high-level results:

- `repeated-order-changed.jsonl` produces the repeated-update diagnosis. Since
  an imported envelope does not prove the publisher identity or response-window
  completeness, the role-specific verdict remains `INCONCLUSIVE`.
- `reconnect-without-online.json` produces a reconnect-state diagnosis, also
  `INCONCLUSIVE` because an offline file cannot prove that no later publication
  existed outside the capture.
- `malformed-record.jsonl` retains the accepted record, reports one rejected
  record, and marks the overall report `PARTIAL`.

Do not use these fixtures as evidence of compliance or production readiness.
