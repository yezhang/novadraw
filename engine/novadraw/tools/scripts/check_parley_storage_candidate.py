#!/usr/bin/env python3
"""Check candidate provenance, unchanged algorithms and recorded verification."""
import argparse
import hashlib
import json
from pathlib import Path
import tarfile

REPO = Path(__file__).resolve().parents[2]
EVIDENCE = REPO / "doc/verification/evidence/text-stages-b-2026-11-02"
CHECKSUM = "ada5338c3a9794af7342e6f765b6e78740db37378aced034d7bf72c96b94ed94"


def diagnostics(path):
    result = []
    for line in path.read_text().splitlines():
        event = json.loads(line)
        if event.get("reason") != "compiler-message":
            continue
        message = event["message"]
        if message["level"] != "error" or not message["spans"]:
            continue
        primary = next(s for s in message["spans"] if s["is_primary"])
        result.append({
            "file": primary["file_name"],
            "line": primary["line_start"],
            "code": (message.get("code") or {}).get("code"),
            "message": message["message"],
        })
    return sorted(result, key=lambda row: (row["file"], row["line"], row["message"]))


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--archive", required=True, type=Path)
    args = parser.parse_args()
    archive = args.archive.resolve()
    assert hashlib.sha256(archive.read_bytes()).hexdigest() == CHECKSUM
    candidate = REPO / "target/text-stages-b/parley-0.7.0"
    originals = {}
    with tarfile.open(archive, "r:gz") as contents:
        for member in contents.getmembers():
            if not member.isfile():
                continue
            relative = Path(member.name).relative_to("parley-0.7.0")
            if relative.suffix == ".rs" or relative.name.startswith("LICENSE"):
                originals[str(relative)] = contents.extractfile(member).read()
    verified = []
    for name, original in originals.items():
        expected = original
        if name == "src/layout/mod.rs":
            expected = original.replace(b"mod run;", b"mod run;\nmod storage;").replace(
                b"pub use run::{Run, RunMetrics};",
                b"pub use run::{Run, RunMetrics};\n"
                b"pub use storage::{LayoutArrayStorage, LayoutOwnedStorage, LayoutStorageField};",
            )
        elif name == "src/lib.rs":
            # Unit harness only: exclude unpublished parley_dev tests.
            expected = original.replace(
                b"#[cfg(test)]\nmod tests;",
                b'#[cfg(all(test, not(feature = "storage-candidate-tests")))]\nmod tests;',
            )
        assert (candidate / name).read_bytes() == expected, name
        verified.append(name)
    added = {str(path.relative_to(candidate)) for path in candidate.rglob("*.rs")} - originals.keys()
    assert added == {"src/layout/storage.rs"}, added
    assert (candidate / "src/layout/storage.rs").read_bytes() == (
        EVIDENCE / "layout_storage.rs"
    ).read_bytes()
    baseline_lints = diagnostics(EVIDENCE / "baseline-clippy.jsonl")
    patch_lints = diagnostics(EVIDENCE / "patch-clippy.jsonl")
    assert patch_lints == baseline_lints, (baseline_lints, patch_lints)
    for log, expected in [
        ("private-storage.log", "test result: ok. 5 passed"),
        ("consumer-native.log", "test result: ok. 3 passed"),
        ("consumer-portable.log", "test result: ok. 3 passed"),
        ("consumer-clippy.log", "Finished"),
        ("consumer-wasm.log", "Finished"),
    ]:
        assert expected in (EVIDENCE / log).read_text(), log
    receipt = {
        "archive_sha256": CHECKSUM,
        "upstream_git_revision": "10fedab4bfd4254a227a1a0f9602b6218e1fe447",
        "patch_sha256": hashlib.sha256((EVIDENCE / "parley-0.7.0-storage.patch").read_bytes()).hexdigest(),
        "reviewed_source_sha256": hashlib.sha256((EVIDENCE / "layout_storage.rs").read_bytes()).hexdigest(),
        "verified_original_sources_and_licenses": verified,
        "new_source": sorted(added),
        "baseline_and_candidate_strict_clippy_errors": baseline_lints,
        "new_strict_clippy_errors": 0,
        "private_tests_passed": 5,
        "native_consumer_tests_passed_per_configuration": 3,
        "observation_control_cases_per_configuration": 160,
        "wasm_portable_build": "passed; not executed",
        "upstream_full_suite": "not run; unpublished parley_dev unavailable",
        "production_dependency_changed": False,
        "staged_adapter_complete": False,
        "operation_accounting_complete": False,
        "independent_human_review": "pending",
    }
    (EVIDENCE / "verification.json").write_text(
        json.dumps(receipt, indent=2, ensure_ascii=False) + "\n"
    )
    print(json.dumps({key: value for key, value in receipt.items()
                      if key != "verified_original_sources_and_licenses"},
                     indent=2, ensure_ascii=False))


if __name__ == "__main__":
    main()
