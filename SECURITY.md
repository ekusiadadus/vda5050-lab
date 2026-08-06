# Security policy

## Supported versions

| Version | Supported |
| --- | --- |
| `0.1.x` | Yes |
| unreleased development snapshots | No guarantee |

## Reporting a vulnerability

Use the repository's **Security > Report a vulnerability** flow so the report
is private. Do not open a public issue for a suspected vulnerability.

Do not attach customer traces, credentials, warehouse layouts, robot
identifiers, broker addresses, or other sensitive operational data. Reproduce
with synthetic input whenever possible. If sensitive evidence is essential,
first describe its type and wait for an agreed transfer method.

Include:

- affected version and operating system;
- input format and the smallest synthetic reproducer;
- expected and observed behavior;
- security impact and required preconditions; and
- whether the issue can cause unsupported attribution, unbounded resource use,
  path escape, output injection, or source-integrity failure.

The maintainer will acknowledge a complete report within seven days. A fix,
advisory, and coordinated disclosure date depend on severity and reproducibility.

## Security boundary

`vda5050-doctor` is an offline analysis tool. It does not connect to MQTT,
publish VDA messages, control robots, or upload telemetry. Reports are not
certification or functional-safety evidence. See the README and release notes
for version-specific limitations.
