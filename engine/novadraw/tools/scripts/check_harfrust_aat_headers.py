#!/usr/bin/env python3
"""Freeze AAT header admission after replaying all five prior patches."""
import argparse
import difflib
import json
import os
from pathlib import Path
import shutil
import tarfile
import tempfile

import check_harfrust_buffer_admission as harness

REPO = harness.REPO
SOURCE = REPO / "tests/consumers/harfrust-admission/src/aat_oracle.rs"
CHANGED = {"src/hb/face.rs", "src/hb/aat/mod.rs"}
ADDED = {"src/hb/aat/cold_headers.rs"}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--target-dir", type=Path, required=True)
    args = parser.parse_args()
    output = harness.artifact_output(args.output)
    archive = output / "harfrust-0.3.2.crate"
    if harness.sha(archive.read_bytes()) != harness.ARCHIVE_SHA:
        raise SystemExit("Original archive checksum mismatch")
    lookup = json.loads((output / "lookup-verification.json").read_text())
    patches = {**lookup["prior_patches"],
               "lookup": lookup["incremental_patch_sha256"]}
    base = REPO / "target/text-harfrust-aat"
    base.mkdir(parents=True, exist_ok=True)
    root = Path(tempfile.mkdtemp(prefix="run-", dir=base))
    original = root / "original"
    original.mkdir()
    with tarfile.open(archive) as source:
        for member in source.getmembers():
            if not member.isfile():
                continue
            relative = Path(member.name).relative_to("harfrust-0.3.2")
            if relative.is_absolute() or ".." in relative.parts:
                raise SystemExit("Unexpected archive path")
            dest = original / relative
            dest.parent.mkdir(parents=True, exist_ok=True)
            dest.write_bytes(source.extractfile(member).read())
    before = root / "after-lookup"
    shutil.copytree(original, before)
    for name, checksum in patches.items():
        path = output / f"harfrust-0.3.2-{name}.patch"
        if harness.sha(path.read_bytes()) != checksum:
            raise SystemExit(f"Frozen {name} patch changed")
        harness.run(["git", "apply", "--no-index", str(path)],
                    output / f"aat-{name}-replay.log", os.environ, before)
    sources = {str(p.relative_to(before))
               for p in (before / "src").rglob("*") if p.is_file()}
    actual = {str(p.relative_to(harness.PRODUCTION))
              for p in (harness.PRODUCTION / "src").rglob("*") if p.is_file()}
    if actual != sources | ADDED:
        raise SystemExit("Unexpected source added or removed")
    patch, changed = [], []
    for relative in sorted(actual):
        old = (before / relative).read_bytes() if relative in sources else b""
        new = (harness.PRODUCTION / relative).read_bytes()
        if old == new:
            continue
        if relative not in CHANGED | ADDED:
            raise SystemExit(f"Unrelated source changed: {relative}")
        changed.append(relative)
        patch.extend(difflib.unified_diff(
            old.decode().splitlines(keepends=True), new.decode().splitlines(keepends=True),
            fromfile=f"a/{relative}" if relative in sources else "/dev/null",
            tofile=f"b/{relative}",
        ))
    patch_path = output / "harfrust-0.3.2-aat-headers.patch"
    patch_path.write_bytes("".join(patch).encode())
    applied = root / "after-aat"
    shutil.copytree(before, applied)
    harness.run(["git", "apply", "--no-index", str(patch_path)],
                output / "aat-patch-replay.log", os.environ, applied)
    for relative in actual:
        if (applied / relative).read_bytes() != (harness.PRODUCTION / relative).read_bytes():
            raise SystemExit(f"Patch replay differs: {relative}")
    shutil.copy2(patch_path, REPO / "dependencies" / patch_path.name)
    env = os.environ.copy()
    env["CARGO_TARGET_DIR"] = str(args.target_dir.resolve())
    env["CC"] = "/Library/Developer/CommandLineTools/usr/bin/clang"
    env["CARGO_TARGET_AARCH64_APPLE_DARWIN_LINKER"] = env["CC"]
    harness.SOURCE = SOURCE
    signatures, observations = {}, {}
    for name, library in [("original", original), ("production", harness.PRODUCTION)]:
        lib_package = root / f"lib-{name}"
        harness.manifest(lib_package, library / "src/lib.rs", original / "Cargo.toml")
        package = root / f"oracle-{name}"
        for profile in ["native", "portable"]:
            harness.manifest(package, lib_package, original / "Cargo.toml", binary=True)
            path = package / "Cargo.toml"
            if profile == "portable":
                path.write_text(path.read_text().replace(
                    "path = " + json.dumps(str(lib_package)) + " }",
                    "path = " + json.dumps(str(lib_package)) + ", default-features = false }",
                ))
            harness.run(["cargo", "+1.96.0", "build", "--offline", "--manifest-path",
                         str(path), "--bin", "oracle"],
                        output / f"aat-oracle-{name}-{profile}-build.log", env)
            log = output / f"aat-oracle-{name}-{profile}.log"
            harness.run([str(args.target_dir.resolve() / "debug/oracle")], log, env)
            signatures[f"{name}-{profile}"] = harness.sha(log.read_bytes())
            observations[f"{name}-{profile}"] = len(log.read_bytes().splitlines())
    if len(set(signatures.values())) != 1:
        raise SystemExit(f"Ordinary outputs differ: {signatures}")
    private_logs, consumer_logs = {}, {}
    for profile, flags in [("native", []), ("portable", ["--no-default-features"])]:
        log = output / f"aat-private-{profile}.log"
        harness.run(["cargo", "+1.96.0", "test", "--offline", "--manifest-path",
                     str(root / "lib-production/Cargo.toml"), "--lib", *flags], log, env)
        private_logs[profile] = harness.sha(log.read_bytes())
    consumer = REPO / "tests/consumers/harfrust-admission/Cargo.toml"
    for profile, flags in [
        ("native", []), ("portable", ["--no-default-features", "--features", "portable"]),
    ]:
        log = output / f"aat-consumer-{profile}.log"
        harness.run(["cargo", "+1.96.0", "test", "--offline", "--manifest-path",
                     str(consumer), "--lib", *flags], log, env)
        consumer_logs[profile] = harness.sha(log.read_bytes())
    receipt = {
        "archive_sha256": harness.ARCHIVE_SHA, "prior_patches": patches,
        "incremental_patch_sha256": harness.sha(patch_path.read_bytes()),
        "patch_replays_and_reproduces_exact_source": True,
        "changed_after_lookup": changed,
        "unchanged_source_files_after_lookup": len(sources - CHANGED),
        "oracle_source_sha256": harness.sha(SOURCE.read_bytes()),
        "ordinary_output_signatures": signatures,
        "observations_per_configuration": observations,
        "private_test_log_sha256": private_logs, "external_test_log_sha256": consumer_logs,
        "empty_aat_cache_headers_supported": True,
        "nonempty_aat_or_plan_or_shaping_admission_complete": False,
        "core_staged_adapter_enabled": False,
    }
    (output / "aat-verification.json").write_text(json.dumps(receipt, indent=2) + "\n")
    print(json.dumps(receipt, indent=2))


if __name__ == "__main__":
    main()
