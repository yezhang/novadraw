#!/usr/bin/env python3
"""Materialize an isolated, checksum-pinned Parley storage candidate."""
import argparse
import hashlib
from pathlib import Path
import subprocess
import tarfile

REPO = Path(__file__).resolve().parents[2]
EVIDENCE = REPO / "doc/verification/evidence/text-stages-b-2026-11-02"
CHECKSUM = "ada5338c3a9794af7342e6f765b6e78740db37378aced034d7bf72c96b94ed94"


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--archive", type=Path, required=True,
                        help="Original crates.io parley-0.7.0.crate archive")
    parser.add_argument("--baseline", action="store_true",
                        help="Create an unpatched source baseline for lint comparison")
    args = parser.parse_args()
    archive = args.archive.resolve()
    actual = hashlib.sha256(archive.read_bytes()).hexdigest()
    if actual != CHECKSUM:
        raise SystemExit(f"Archive checksum mismatch: {actual}")
    destination = REPO / "target/text-stages-b"
    if args.baseline:
        destination = destination / "baseline"
    candidate = destination / "parley-0.7.0"
    destination.mkdir(parents=True, exist_ok=True)
    if candidate.exists():
        raise SystemExit(f"Candidate already exists; use its verified source: {candidate}")
    with tarfile.open(archive, "r:gz") as contents:
        contents.extractall(destination, filter="data")
    if args.baseline:
        manifest = candidate / "Cargo.toml"
        manifest.write_text(manifest.read_text() + "\n[workspace]\n")
        print(f"Archive SHA-256: {actual}")
        print(f"Unpatched source baseline: {candidate}")
        return
    patch = EVIDENCE / "parley-0.7.0-storage.patch"
    subprocess.run(["git", "apply", "--check", str(patch)], cwd=candidate, check=True)
    subprocess.run(["git", "apply", str(patch)], cwd=candidate, check=True)
    generated = candidate / "src/layout/storage.rs"
    if generated.read_bytes() != (EVIDENCE / "layout_storage.rs").read_bytes():
        raise SystemExit("Applied patch differs from reviewed source")
    # Keep direct candidate unit tests outside the production workspace.
    manifest = candidate / "Cargo.toml"
    manifest.write_text(
        manifest.read_text().replace(
            "[features]\n", "[features]\nstorage-candidate-tests = []\n", 1
        ) + "\n[workspace]\n"
    )
    # Published Parley lacks parley_dev; compile only the new private tests.
    library = candidate / "src/lib.rs"
    library.write_text(library.read_text().replace(
        "#[cfg(test)]\nmod tests;",
        '#[cfg(all(test, not(feature = "storage-candidate-tests")))]\nmod tests;',
        1,
    ))
    print(f"Archive SHA-256: {actual}")
    print(f"Patch SHA-256: {hashlib.sha256(patch.read_bytes()).hexdigest()}")
    print(f"Isolated candidate: {candidate}")
    print("Production Cargo manifests and lockfiles were not modified.")


if __name__ == "__main__":
    main()
