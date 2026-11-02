#!/usr/bin/env python3
"""Verify Harfrust input admission provenance, private tests and ordinary outputs."""
import argparse
import difflib
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import tarfile
import tempfile

REPO = Path(__file__).resolve().parents[2]
PRODUCTION = REPO / "dependencies/harfrust-0.3.2"
ARCHIVE_SHA = "92c020db12c71d8a12a3fe7607873cade3a01a6287e29d540c8723276221b9d8"
SOURCE = REPO / "tests/consumers/harfrust-admission/src/oracle.rs"
CHANGED = {"src/lib.rs", "src/hb/buffer.rs", "rustfmt.toml"}
ADDED = {"src/hb/buffer_admission.rs"}


def sha(data):
    return hashlib.sha256(data).hexdigest()


def run(command, log, env, cwd=None):
    with log.open("wb") as output:
        subprocess.run(command, check=True, env=env, cwd=cwd,
                       stdout=output, stderr=subprocess.STDOUT)


def manifest(package, library, original, binary=False):
    package.mkdir(parents=True, exist_ok=True)
    # Only build dependencies are needed for library tests. The published
    # benchmark CLI and criterion are neither production nor test dependencies
    # of this isolated harness. Preserve all original dependency declarations.
    text = original.read_text()
    start = text.index("[dependencies.bitflags]")
    end = text.index("[dev-dependencies.criterion]")
    target = f'[[bin]]\nname = "oracle"\npath = {json.dumps(str(SOURCE))}\n' if binary else (
        f'[lib]\nname = "harfrust"\npath = {json.dumps(str(library))}\n'
    )
    # The oracle depends on a separate isolated original/production library,
    # allowing exactly the same Rust source to use the ordinary public API.
    if binary:
        deps = f'[dependencies]\nharfrust = {{ path = {json.dumps(str(library))} }}\n'
    else:
        deps = text[start:end]
    (package / "Cargo.toml").write_text(
        '[package]\nname = "harfrust"\nversion = "0.3.2"\nedition = "2021"\n'
        if not binary else
        '[package]\nname = "buffer-oracle"\nversion = "0.1.0"\nedition = "2024"\n'
    )
    with (package / "Cargo.toml").open("a") as output:
        output.write(
            '[workspace]\n[features]\ndefault = ["std"]\nstd = ["read-fonts/std"]\n'
            if not binary else '[workspace]\n'
        )
        output.write(target + deps)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--archive", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--target-dir", type=Path, required=True)
    args = parser.parse_args()
    archive = args.archive.resolve()
    if sha(archive.read_bytes()) != ARCHIVE_SHA:
        raise SystemExit("Harfrust archive checksum mismatch")
    output = args.output.resolve()
    output.mkdir(parents=True, exist_ok=True)
    root = REPO / "target/text-harfrust-buffer"
    baseline = root / "original"
    baseline.mkdir(parents=True, exist_ok=True)
    originals = {}
    with tarfile.open(archive) as source:
        for member in source.getmembers():
            if not member.isfile():
                continue
            relative = Path(member.name).relative_to("harfrust-0.3.2")
            if ".." in relative.parts or relative.is_absolute():
                raise SystemExit("Unexpected archive member")
            data = source.extractfile(member).read()
            originals[str(relative)] = data
            dest = baseline / relative
            dest.parent.mkdir(parents=True, exist_ok=True)
            dest.write_bytes(data)
    changed = []
    patch = []
    for relative, original in originals.items():
        local = (PRODUCTION / relative).read_bytes()
        # The dependency lockfile is not production source; Cargo can update it.
        if local == original or relative == "Cargo.lock":
            continue
        if relative not in CHANGED:
            raise SystemExit(f"Unexpected production edit: {relative}")
        changed.append(relative)
        patch.extend(difflib.unified_diff(
            original.decode().splitlines(keepends=True),
            local.decode().splitlines(keepends=True),
            fromfile=f"a/{relative}", tofile=f"b/{relative}",
        ))
    for relative in sorted(ADDED):
        local = (PRODUCTION / relative).read_bytes()
        patch.extend(difflib.unified_diff(
            [], local.decode().splitlines(keepends=True),
            fromfile="/dev/null", tofile=f"b/{relative}",
        ))
    patch_path = output / "harfrust-0.3.2-buffer.patch"
    patch_path.write_bytes("".join(patch).encode())
    # Apply the mixed-newline patch to an exact extracted baseline and compare
    # every edited byte; do not normalize upstream CRLF.
    with tempfile.TemporaryDirectory(prefix="patch-", dir=root) as scratch:
        applied = Path(scratch)
        shutil.copytree(baseline, applied, dirs_exist_ok=True)
        run(["git", "apply", "--no-index", str(patch_path)],
            output / "buffer-patch-apply.log", os.environ, applied)
        for relative in CHANGED | ADDED:
            if (applied / relative).read_bytes() != (PRODUCTION / relative).read_bytes():
                raise SystemExit(f"Patch reproduction differs: {relative}")
    shutil.copy2(archive, output / archive.name)
    shutil.copy2(patch_path, REPO / "dependencies" / patch_path.name)
    env = os.environ.copy()
    env["CARGO_TARGET_DIR"] = str(args.target_dir.resolve())
    env["CC"] = "/Library/Developer/CommandLineTools/usr/bin/clang"
    env["CARGO_TARGET_AARCH64_APPLE_DARWIN_LINKER"] = env["CC"]
    results = {}
    for name, library in [("original", baseline), ("production", PRODUCTION)]:
        lib_package = root / f"lib-{name}"
        manifest(lib_package, library / "src/lib.rs", baseline / "Cargo.toml")
        package = root / f"oracle-{name}"
        manifest(package, lib_package, baseline / "Cargo.toml", binary=True)
        for profile, flags in [("native", []), ("portable", ["--no-default-features"])]:
            # Set the oracle dependency's feature choice in the isolated manifest.
            path = package / "Cargo.toml"
            text = path.read_text()
            text = text.replace('path = ' + json.dumps(str(lib_package)) + ' }',
                                'path = ' + json.dumps(str(lib_package)) +
                                ', default-features = false }') if flags else text
            path.write_text(text)
            build = output / f"buffer-oracle-{name}-{profile}-build.log"
            run(["cargo", "+1.96.0", "build", "--offline", "--manifest-path",
                 str(path), "--bin", "oracle"], build, env)
            log = output / f"buffer-oracle-{name}-{profile}.log"
            run([str(args.target_dir.resolve() / "debug/oracle")], log, env)
            results[f"{name}-{profile}"] = sha(log.read_bytes())
            if name == "production":
                run(["cargo", "+1.96.0", "test", "--offline", "--manifest-path",
                     str(lib_package / "Cargo.toml"), "--lib",
                     "buffer::admission::tests", *flags],
                    output / f"buffer-private-{profile}.log", env)
    if len(set(results.values())) != 1:
        raise SystemExit(f"Ordinary oracle mismatch: {results}")
    receipt = {
        "archive_sha256": ARCHIVE_SHA,
        "source_files_verified": sum(p.startswith("src/") for p in originals),
        "changed_original_files": sorted(changed),
        "added_files": sorted(ADDED),
        "patch_sha256": sha(patch_path.read_bytes()),
        "patch_reproduces_exact_bytes": True,
        "oracle_source_sha256": sha(SOURCE.read_bytes()),
        "ordinary_signatures": results,
        "cases_per_configuration": 216,
        "observations_per_case": 2,
        "private_tests_per_configuration": 5,
        "covers_input_buffer_only": True,
        "shaping_admission_complete": False,
        "latency_measured": False,
    }
    (output / "buffer-verification.json").write_text(json.dumps(receipt, indent=2) + "\n")
    print(json.dumps(receipt, indent=2))


if __name__ == "__main__":
    main()
