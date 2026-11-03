#!/usr/bin/env python3
"""Verify ordinary line breaking against the checksum-pinned upstream source."""
import argparse
import gzip
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import tarfile
from check_harfrust_buffer_admission import artifact_output

REPO = Path(__file__).resolve().parents[2]
SOURCE = REPO / "tests/consumers/parley-storage/src/line_oracle.rs"
PRODUCTION = REPO / "dependencies/parley-0.7.0"
BASELINE = REPO / "target/text-stages-b/baseline/parley-0.7.0"
ARCHIVE_SHA256 = "ada5338c3a9794af7342e6f765b6e78740db37378aced034d7bf72c96b94ed94"


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--target-dir", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--archive", type=Path, required=True)
    args = parser.parse_args()
    output = artifact_output(args.output)
    output.mkdir(parents=True, exist_ok=True)
    archive = args.archive.resolve()
    if digest(archive) != ARCHIVE_SHA256:
        raise SystemExit("Original Parley archive checksum mismatch")
    with tarfile.open(archive, "r:gz") as contents:
        for member in contents.getmembers():
            if member.isfile() and member.name.startswith("parley-0.7.0/src/"):
                relative = Path(member.name).relative_to("parley-0.7.0")
                if (BASELINE / relative).read_bytes() != contents.extractfile(member).read():
                    raise SystemExit(f"Baseline source differs from archive: {relative}")
    root = REPO / "target/text-line-admission"
    root.mkdir(parents=True, exist_ok=True)
    env = os.environ.copy()
    env["CARGO_TARGET_DIR"] = str(args.target_dir.resolve())
    env["CC"] = "/Library/Developer/CommandLineTools/usr/bin/clang"
    env["CARGO_TARGET_AARCH64_APPLE_DARWIN_LINKER"] = env["CC"]
    results = {}
    for name, dependency in [("original", BASELINE), ("production", PRODUCTION)]:
        if not (dependency / "Cargo.toml").is_file():
            raise SystemExit(f"Missing verified dependency: {dependency}")
        package = root / name
        package.mkdir(exist_ok=True)
        manifest = package / "Cargo.toml"
        manifest.write_text(
            f'[package]\nname = "parley-line-oracle-{name}"\n'
            'version = "0.1.0"\nedition = "2024"\n[workspace]\n'
            f'[[bin]]\nname = "oracle"\npath = {json.dumps(str(SOURCE))}\n'
            '[dependencies]\nparley = { version = "=0.7.0", '
            f'path = {json.dumps(str(dependency))} }}\n'
        )
        with (output / f"line-oracle-{name}.log").open("wb") as stdout, (
            output / f"line-oracle-{name}-build.log"
        ).open("wb") as stderr:
            subprocess.run(
                ["cargo", "+1.96.0", "run", "--offline",
                 "--manifest-path", str(manifest), "--bin", "oracle"],
                check=True, env=env, stdout=stdout, stderr=stderr,
            )
        results[name] = digest(output / f"line-oracle-{name}.log")
    if results["original"] != results["production"]:
        raise SystemExit(f"Ordinary API oracle mismatch: {results}")
    # Isolate private dependency tests. Published Parley omits parley_dev.
    private = root / "private"
    shutil.copytree(PRODUCTION, private, dirs_exist_ok=True)
    manifest = private / "Cargo.toml"
    manifest.write_text(manifest.read_text() + "\n[workspace]\n")
    library = private / "src/lib.rs"
    library.write_text(library.read_text().replace("#[cfg(test)]\nmod tests;", "", 1))
    # Use the already verified consumer resolution, rather than the release
    # tarball's stale lockfile (which can require uncached dependency versions).
    shutil.copy2(REPO / "tests/consumers/parley-storage/Cargo.lock", private / "Cargo.lock")
    with (output / "line-private.log").open("wb") as log:
        subprocess.run(
            ["cargo", "+1.96.0", "test", "--offline", "--manifest-path",
             str(manifest), "--lib", "admission_tests"],
            check=True, env=env, stdout=log, stderr=subprocess.STDOUT,
        )
    receipt = {
        "original_archive_sha256": digest(archive),
        "cases": 960,
        "observations_per_case": ["aligned", "reflow", "partial-revert-drop"],
        "source_sha256": digest(SOURCE),
        "ordinary_signatures_sha256": results,
        "private_tests": 2,
        "private_excludes_unpublished_upstream_test_support": True,
    }
    for name in ["original", "production"]:
        raw = output / f"line-oracle-{name}.log"
        raw.with_suffix(".log.gz").write_bytes(gzip.compress(raw.read_bytes(), mtime=0))
    (output / "line-oracle-receipt.json").write_text(
        json.dumps(receipt, indent=2) + "\n"
    )
    print(json.dumps(receipt, indent=2))


if __name__ == "__main__":
    main()
