# Contributing to Novadraw

Contributions are welcome through reproducible issue reports, documentation and example
improvements, and code changes with corresponding verification coverage.

## Before Contributing

Read the
[contribution workflow](engine/novadraw/book/contributor/src/01-contribution-workflow.md)
before opening an Issue or Pull Request. It defines the project's requirements for behavioral
contracts, state ownership, failure models, tests, and verification evidence.

For documentation-only changes, run:

```bash
cd engine/novadraw
cargo xtask docs
git diff --check
```

Use the verification commands required by the contribution workflow for code changes.

## Financial Support

If Novadraw is useful to you, you can
[sponsor its continued development through GitHub Sponsors](https://github.com/sponsors/yezhang).

Sponsorship is voluntary. It does not grant additional project governance rights or change the
terms of the Apache License 2.0.
