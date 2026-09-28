# Security policy

SERSHI is pre-alpha software that is designed to act on your computer, so we take
security reports seriously even at this stage.

## Reporting a vulnerability

Please **do not open a public issue**. Report privately through GitHub's
[private vulnerability reporting](https://github.com/jpablortiz96/SERSHI/security/advisories/new)
for this repository.

Include what you found, how to reproduce it, and its impact. We aim to acknowledge
reports within 7 days. There is no bug bounty.

Especially relevant: ways to make SERSHI execute an action without passing the
policy engine, bypass confirmation, reach the OS from the WebView, leak secrets or
user content into logs, or escalate a skill's permissions.

## Supported versions

Only the latest commit on `main` is supported until v1.0.

## Security model

The threat model and design are documented in [docs/SECURITY.md](docs/SECURITY.md).
