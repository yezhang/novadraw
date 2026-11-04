#!/usr/bin/env python3
"""Freeze multiple-substitution admission after fourteen immutable patches."""
import argparse
import difflib
import gzip
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import tarfile
import tempfile

import check_harfrust_buffer_admission as harness
import text_admission_build

CHANGED = {"src/hb/shape_admission.rs", "src/hb/buffer_admission.rs"}


def observe(binary, log, env):
    """Save every raw byte losslessly without a duplicate uncompressed file."""
    digest, lines = hashlib.sha256(), 0
    with subprocess.Popen([str(binary)], stdout=subprocess.PIPE, stderr=subprocess.PIPE,
                          env=env) as child:
        with gzip.open(log, "wb") as destination:
            while chunk := child.stdout.read(1024 * 1024):
                digest.update(chunk)
                lines += chunk.count(b"\n")
                destination.write(chunk)
        errors = child.stderr.read()
        if child.wait():
            raise RuntimeError(errors.decode(errors="replace"))
    # Check both compression integrity and exact raw identity.
    check = hashlib.sha256()
    with gzip.open(log, "rb") as source:
        while chunk := source.read(1024 * 1024):
            check.update(chunk)
    if check.digest() != digest.digest():
        raise RuntimeError("Compressed observation differs")
    return digest.hexdigest(), lines


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--target-dir", type=Path, required=True)
    parser.add_argument("--linker", choices=["clang", "rust-lld"], default="clang")
    args = parser.parse_args()
    output = harness.artifact_output(args.output)
    archive = output / "harfrust-0.3.2.crate"
    if harness.sha(archive.read_bytes()) != harness.ARCHIVE_SHA:
        raise SystemExit("Archive checksum mismatch")
    prior = json.loads((output / "single-verification.json").read_text())
    patches = {**prior["prior_patches"], "single": prior["incremental_patch_sha256"]}
    root = Path(tempfile.mkdtemp(prefix="text-harfrust-multiple-", dir=harness.REPO / "target"))
    original = root / "original"
    original.mkdir()
    with tarfile.open(archive) as source:
        for member in source.getmembers():
            if not member.isfile():
                continue
            relative = Path(member.name).relative_to("harfrust-0.3.2")
            if relative.is_absolute() or ".." in relative.parts:
                raise SystemExit("Unexpected archive member")
            dest = original / relative
            dest.parent.mkdir(parents=True, exist_ok=True)
            dest.write_bytes(source.extractfile(member).read())
    before = root / "after-single"
    shutil.copytree(original, before)
    for name, checksum in patches.items():
        path = output / f"harfrust-0.3.2-{name}.patch"
        if harness.sha(path.read_bytes()) != checksum:
            raise SystemExit(f"Frozen patch changed: {name}")
        harness.run(["git", "apply", "--no-index", str(path)],
                    output / f"multiple-{name}-replay.log", os.environ, before)
    sources = {str(p.relative_to(before)) for p in (before / "src").rglob("*") if p.is_file()}
    actual = {str(p.relative_to(harness.PRODUCTION))
              for p in (harness.PRODUCTION / "src").rglob("*") if p.is_file()}
    if actual != sources:
        raise SystemExit("Unexpected source addition or removal")
    patch, changed = [], set()
    for relative in sorted(actual):
        old = (before / relative).read_bytes()
        new = (harness.PRODUCTION / relative).read_bytes()
        if old == new:
            continue
        if relative not in CHANGED:
            raise SystemExit(f"Unrelated source changed: {relative}")
        changed.add(relative)
        patch.extend(difflib.unified_diff(
            old.decode().splitlines(keepends=True), new.decode().splitlines(keepends=True),
            fromfile=f"a/{relative}", tofile=f"b/{relative}",
        ))
    if changed != CHANGED:
        raise SystemExit("Expected source change missing")
    path = output / "harfrust-0.3.2-multiple.patch"
    path.write_bytes("".join(patch).encode())
    applied = root / "after-multiple"
    shutil.copytree(before, applied)
    harness.run(["git", "apply", "--no-index", str(path)],
                output / "multiple-patch-replay.log", os.environ, applied)
    for relative in actual:
        if (applied / relative).read_bytes() != (harness.PRODUCTION / relative).read_bytes():
            raise SystemExit(f"Replay differs: {relative}")
    shutil.copy2(path, harness.REPO / "dependencies" / path.name)
    env = text_admission_build.environment(args.target_dir.resolve(), args.linker)
    build_root = harness.REPO / "target/text-harfrust-multiple-build"
    build_root.mkdir(exist_ok=True)
    signatures, counts = {}, {}
    expected = {"plan": 39744, "plan_layout": 50688, "private_language": 6720,
                "general_language": 24048, "script_callbacks": 51840, "shape": 12672,
                "single": 18432, "multiple": 36864}
    for name, library in [("original", original), ("production", harness.PRODUCTION)]:
        lib = build_root / f"lib-{name}"
        harness.manifest(lib, library / "src/lib.rs", original / "Cargo.toml")
        for oracle, denominator in expected.items():
            source = harness.REPO / f"tests/consumers/harfrust-admission/src/{oracle}_oracle.rs"
            if oracle != "multiple" and harness.sha(source.read_bytes()) != prior["oracle_sources_sha256"][oracle]:
                raise SystemExit(f"Frozen oracle changed: {oracle}")
            harness.SOURCE = source
            package = build_root / f"oracle-{name}-{oracle}"
            for profile in ["native", "portable"]:
                harness.manifest(package, lib, original / "Cargo.toml", binary=True)
                manifest = package / "Cargo.toml"
                if profile == "portable":
                    manifest.write_text(manifest.read_text().replace(
                        "path = " + json.dumps(str(lib)) + " }",
                        "path = " + json.dumps(str(lib)) + ", default-features = false }",
                    ))
                harness.run(["cargo", "build", "--offline", "--manifest-path",
                             str(manifest), "--bin", "oracle"],
                            output / f"multiple-{oracle}-{name}-{profile}-build.log", env)
                log = output / f"multiple-{oracle}-{name}-{profile}.log.gz"
                key = f"{oracle}/{name}/{profile}"
                signatures[key], counts[key] = observe(
                    args.target_dir.resolve() / "debug/oracle", log, env,
                )
                if counts[key] != denominator:
                    raise SystemExit(f"Wrong observation count: {key}/{counts[key]}")
        if name == "production":
            for profile, flags in [("native", []), ("portable", ["--no-default-features"])]:
                harness.run(["cargo", "test", "--offline", "--manifest-path",
                             str(lib / "Cargo.toml"), "--lib", *flags],
                            output / f"multiple-private-{profile}.log", env)
    for oracle in expected:
        selected = {k: v for k, v in signatures.items() if k.startswith(oracle + "/")}
        if len(set(selected.values())) != 1:
            raise SystemExit(f"Ordinary output mismatch: {oracle}")
        if oracle != "multiple" and set(selected.values()) != {
                value for key, value in prior["ordinary_output_signatures"].items()
                if key.startswith(oracle + "/")}:
            raise SystemExit(f"Frozen ordinary signature changed: {oracle}")
    consumer = {}
    for profile, flags in [("native", []), ("portable", ["--no-default-features", "--features", "portable"])]:
        log = output / f"multiple-consumer-{profile}.log"
        harness.run(["cargo", "test", "--offline", "--manifest-path",
                     str(harness.REPO / "tests/consumers/harfrust-admission/Cargo.toml"),
                     "--lib", *flags], log, env)
        consumer[profile] = harness.sha(log.read_bytes())
    for relative in actual:
        if (applied / relative).read_bytes() != (harness.PRODUCTION / relative).read_bytes():
            raise SystemExit(f"Source changed during verification: {relative}")
    receipt = {
        "archive_sha256": harness.ARCHIVE_SHA, "prior_patches": patches,
        "incremental_patch_sha256": harness.sha(path.read_bytes()),
        "patch_replays_and_reproduces_exact_source": True,
        "changed_after_single": sorted(CHANGED),
        "unchanged_source_files_after_single": len(sources - CHANGED),
        "total_source_files": len(actual), "ordinary_output_signatures": signatures,
        "build_configuration": "Native/portable, dev/test debug=0, incremental=0, jobs=1",
        "linker": args.linker, "observations_per_configuration": counts,
        "ordinary_output_storage": "lossless gzip; signatures and counts refer to decompressed bytes",
        "external_test_log_sha256": consumer,
        "private_test_log_sha256": {profile: harness.sha((output /
            f"multiple-private-{profile}.log").read_bytes()) for profile in ["native", "portable"]},
        "oracle_sources_sha256": {n: harness.sha((harness.REPO /
            f"tests/consumers/harfrust-admission/src/{n}_oracle.rs").read_bytes()) for n in expected},
        "multiple_fixture_sha256": harness.sha((harness.REPO /
            "tests/consumers/harfrust-admission/src/multiple_fixtures.rs").read_bytes()),
        "single_fixture_sha256": harness.sha((harness.REPO /
            "tests/consumers/harfrust-admission/src/single_fixtures.rs").read_bytes()),
        "supported_actual_shaping": "mapped ASCII LTR Latin/Common/unknown; zero-flag GSUB single and positive-length multiple substitution",
        "shaping_admission_complete": False, "core_staged_adapter_enabled": False,
    }
    (output / "multiple-verification.json").write_text(json.dumps(receipt, indent=2) + "\n")
    print(json.dumps(receipt, indent=2))


if __name__ == "__main__":
    main()
