# Verification Manifest

`suites.toml` is the executable source of truth for Novadraw verification commands and their
mapping to milestones, source areas, documents, platforms, and artifacts.

It does not define architecture or milestone completion state:

- `doc/design/` and accepted ADRs define behavior.
- `doc/roadmap/` and `doc/roadmap/editor/` define delivery state.
- `verification/suites.toml` defines how those claims are checked.
- `doc/verification/` records manual procedures and historical results.

## Commands

Run all commands from the Novadraw workspace root:

```bash
cargo xtask list
cargo xtask docs
cargo xtask check --quick
cargo xtask check --full
cargo xtask verify g5.4
cargo xtask manual g5.4
```

Suite selectors accept either a complete ID or an unambiguous dotted prefix. `verify --all`
deduplicates shared commands before execution.

Use the profiles at different workflow boundaries:

- `check --quick` runs workspace formatting and compilation for batch-level feedback.
- `check --full` runs formatting, workspace Clippy, and all workspace tests once at the final
  submission, push, merge, or milestone boundary. Clippy already performs the normal workspace
  compilation pass, so the full profile does not run a separate redundant `cargo check`.
- During the edit loop, prefer `cargo check -p <crate>`, an exact test, and the affected suite.
- Documentation-only changes use `cargo xtask docs` and `git diff --check`.

## Editor Headless Replay

The `g3`, `g4`, and `g5.2`-`g5.4` suites run `node-editor-demo` without creating a window. Native
input and replay both delegate to the same platform-neutral `EditorHarness`, so replay exercises the
real Tool, Request, Command, model notification, Viewer projection, and overlay-handle path.

Each replay starts from a fresh model and writes an ordered checkpoint report under
`target/verification/reports/`. On failure, the report retains the successfully verified prefix and
the first failing transition. Replay does not replace the manual Native/GPU acceptance step.

## Schema

- `commands`: structured executable definitions. Commands use an argv array and never a shell
  command string.
- `profiles`: ordered command sets for developer feedback and submission gates.
- `suites`: stable verification IDs with affected paths, documents, expected artifacts, automated
  commands, and an optional manual entry.
- `documentation`: parity ledgers and their allowed status vocabulary.

All paths are relative to the Novadraw workspace. Generated evidence belongs under
`target/verification/`; the manifest may name expected artifacts before they exist.

Committed audit evidence belongs under `verification/evidence/<audit-id>/`. Keep human-readable
conclusions in `doc/verification/`; do not place generated executables in either location.

`cargo xtask docs` rejects:

- unknown or unused command IDs;
- duplicate or malformed suite IDs;
- missing referenced documents or working directories;
- absolute paths and parent-directory traversal;
- parity ledger statuses outside the declared vocabulary.

When adding a new milestone slice, add or update its suite before copying commands into a review
document. Reviews should cite the suite ID and record its result rather than redefining the command
sequence.
