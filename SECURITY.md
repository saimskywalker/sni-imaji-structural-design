# Security policy

## Supported versions

`imaji-structural-design` is not yet published to crates.io. Only the latest `0.1.x`
on `main` gets security fixes.

| Version | Supported |
|---|---|
| 0.1.x (`main`) | yes |

## Reporting a vulnerability

Please do not open a public issue for a vulnerability.

Report it privately through GitHub's private vulnerability reporting: open the
[Security tab](https://github.com/saimskywalker/sni-imaji-structural-design/security)
and choose "Report a vulnerability". Only the maintainer can see the report.

This crate is pure calculation with no I/O and no dependencies, so the
realistic cases are input that makes it panic or loop instead of returning an
error. A wrong result from a correct-looking input is an engineering bug, not
a vulnerability: please report that with the bug form, with the worked
example that shows it.

Please include:

- the smallest input or code snippet that shows it
- what happened (the panic and backtrace, or how long it ran) and what you
  expected
- the crate version or commit, and `rustc --version`

You should hear back usually within a week. Once a fix is ready it is merged
and the advisory is published, with credit to you unless you prefer otherwise.
