# Security Policy

## Reporting a vulnerability

Please do **not** open a public GitHub issue for security vulnerabilities.

Instead, report it privately via GitHub's
[private vulnerability reporting](https://docs.github.com/en/code-security/security-advisories/guidance-on-reporting-and-writing/privately-reporting-a-security-vulnerability)
on this repository, or email the maintainers directly if that channel is
unavailable to you. Include:

- A description of the vulnerability and its potential impact
- Steps to reproduce, or a minimal repro case
- Any relevant snippets or logs (redacted of anything sensitive)

We will acknowledge receipt within 5 business days and aim to provide a
remediation timeline within 14 days of confirming the report.

## Scope

This repository currently holds specifications, decision records, and the
roadmap — documentation, not executable software. The most relevant reports here
concern the repository's own supply chain: the `bootstrap` script and the
toolchain it fetches (git-std, prim), the CI workflow, and the git-hook
configuration.

As the compiler and toolchain described in `docs/ROADMAP.md` land, this policy
will grow trust-boundary scope notes (the IR loader, generated bindings, and the
diagnostic/reflection surfaces) alongside them.

## Supported versions

This project has not yet reached a tagged v0.1 release. Until then, only `main`
is supported; report against the latest commit.
