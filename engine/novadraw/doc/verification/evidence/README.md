# Audit Evidence

This directory stores committed machine-readable or reproduction-oriented evidence for historical
audits. Human-readable conclusions and navigation remain under `doc/verification/`.

Each audit directory may contain:

- fixed source manifests and fingerprints;
- structured findings (`*.json` or `*.jsonl`);
- command logs;
- minimal probe source;
- grouped reviewer notes.

Do not commit compiled probes, generated reports already reproducible from source, or transient
`target/` output. Evidence is historical and must not be used as the current architecture or
milestone source of truth.

## Audits

- [`draw2d-gef-semantic-audit-2026-09-15/`](draw2d-gef-semantic-audit-2026-09-15/review_groups.md)
- [`draw2d-gef-semantic-audit-2026-09-16/`](draw2d-gef-semantic-audit-2026-09-16/review_groups.md)
