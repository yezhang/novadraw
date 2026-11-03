#!/usr/bin/env python3
"""Verify the incremental normalized-instance patch after the frozen buffer patch."""
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
PRODUCTION = harness.PRODUCTION
SOURCE = REPO / "tests/consumers/harfrust-admission/src/instance_oracle.rs"
CHANGED = {"src/lib.rs", "src/hb/face.rs", "src/hb/ot/mod.rs"}
ADDED = {"src/hb/instance_admission.rs"}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--target-dir", type=Path, required=True)
    args = parser.parse_args()
    output = harness.artifact_output(args.output)
    output.mkdir(parents=True, exist_ok=True)
    archive = output / "harfrust-0.3.2.crate"
    if harness.sha(archive.read_bytes()) != harness.ARCHIVE_SHA:
        raise SystemExit("Original archive checksum mismatch")
    buffer_patch = output / "harfrust-0.3.2-buffer.patch"
    frozen = json.loads((output / "buffer-verification.json").read_text())
    if harness.sha(buffer_patch.read_bytes()) != frozen["patch_sha256"]:
        raise SystemExit("Frozen buffer patch changed")
    base = REPO / "target/text-harfrust-instance"
    base.mkdir(parents=True, exist_ok=True)
    root = Path(tempfile.mkdtemp(prefix="run-", dir=base))
    original = root / "original"
    original.mkdir(parents=True, exist_ok=True)
    originals = {}
    with tarfile.open(archive) as source:
        for member in source.getmembers():
            if not member.isfile():
                continue
            relative = Path(member.name).relative_to("harfrust-0.3.2")
            if relative.is_absolute() or ".." in relative.parts:
                raise SystemExit("Unexpected archive path")
            data = source.extractfile(member).read()
            originals[str(relative)] = data
            dest = original / relative
            dest.parent.mkdir(parents=True, exist_ok=True)
            dest.write_bytes(data)
    # Keep reproducible scratch trees in target; do not delete user files or
    # temporary trees as part of evidence generation.
    before = root / "after-buffer"
    before.mkdir(parents=True, exist_ok=True)
    shutil.copytree(original, before, dirs_exist_ok=True)
    replay = ["git", "apply", "--no-index", str(buffer_patch)]
    harness.run(replay, output / "instance-buffer-replay.log", os.environ, before)
    patch = []
    changed = []
    for relative in originals:
        old = (before / relative).read_bytes()
        new = (PRODUCTION / relative).read_bytes()
        if old == new or relative == "Cargo.lock":
            continue
        if relative not in CHANGED:
            raise SystemExit(f"Unexpected edit after buffer: {relative}")
        changed.append(relative)
        patch.extend(difflib.unified_diff(
            old.decode().splitlines(keepends=True), new.decode().splitlines(keepends=True),
            fromfile=f"a/{relative}", tofile=f"b/{relative}",
        ))
    for relative in sorted(ADDED):
        patch.extend(difflib.unified_diff(
            [], (PRODUCTION / relative).read_text().splitlines(keepends=True),
            fromfile="/dev/null", tofile=f"b/{relative}",
        ))
    # Also verify previously added buffer source and every other source file.
    expected_sources = {p for p in originals if p.startswith("src/")}
    expected_sources |= {"src/hb/buffer_admission.rs"}
    for relative in expected_sources - CHANGED:
        if (before / relative).read_bytes() != (PRODUCTION / relative).read_bytes():
            raise SystemExit(f"Unrelated source changed: {relative}")
    actual_sources = {str(p.relative_to(PRODUCTION)) for p in (PRODUCTION / "src").rglob("*")
                      if p.is_file()}
    if actual_sources != expected_sources | ADDED:
        raise SystemExit("Unexpected added or removed production source")
    patch_path = output / "harfrust-0.3.2-instance.patch"
    patch_path.write_bytes("".join(patch).encode())
    applied = root / "after-instance"
    applied.mkdir(parents=True, exist_ok=True)
    shutil.copytree(before, applied, dirs_exist_ok=True)
    harness.run(["git", "apply", "--no-index", str(patch_path)],
                output / "instance-patch-replay.log", os.environ, applied)
    for relative in expected_sources | ADDED:
        if (applied / relative).read_bytes() != (PRODUCTION / relative).read_bytes():
            raise SystemExit(f"Patch replay differs: {relative}")
    shutil.copy2(patch_path, REPO / "dependencies" / patch_path.name)
    env = os.environ.copy()
    env["CARGO_TARGET_DIR"] = str(args.target_dir.resolve())
    env["CC"] = "/Library/Developer/CommandLineTools/usr/bin/clang"
    env["CARGO_TARGET_AARCH64_APPLE_DARWIN_LINKER"] = env["CC"]
    harness.SOURCE = SOURCE
    signatures = {}
    for name, library in [("original", original), ("production", PRODUCTION)]:
        lib_package = root / f"lib-{name}"
        harness.manifest(lib_package, library / "src/lib.rs", original / "Cargo.toml")
        package = root / f"oracle-{name}"
        for profile in ["native", "portable"]:
            harness.manifest(package, lib_package, original / "Cargo.toml", binary=True)
            path = package / "Cargo.toml"
            if profile == "portable":
                text = path.read_text().replace(
                    "path = " + json.dumps(str(lib_package)) + " }",
                    "path = " + json.dumps(str(lib_package)) + ", default-features = false }",
                )
                path.write_text(text)
            harness.run(["cargo", "+1.96.0", "build", "--offline", "--manifest-path",
                         str(path), "--bin", "oracle"],
                        output / f"instance-oracle-{name}-{profile}-build.log", env)
            log = output / f"instance-oracle-{name}-{profile}.log"
            harness.run([str(args.target_dir.resolve() / "debug/oracle")], log, env)
            signatures[f"{name}-{profile}"] = harness.sha(log.read_bytes())
    if len(set(signatures.values())) != 1:
        raise SystemExit(f"Ordinary instance outputs differ: {signatures}")
    receipt = {
        "archive_sha256": harness.ARCHIVE_SHA,
        "prior_buffer_patch_sha256": frozen["patch_sha256"],
        "incremental_patch_sha256": harness.sha(patch_path.read_bytes()),
        "patch_applies_after_buffer_and_reproduces_exact_bytes": True,
        "changed_after_buffer": sorted(changed),
        "added_after_buffer": sorted(ADDED),
        "unchanged_source_files_after_buffer": len(expected_sources - CHANGED),
        "oracle_source_sha256": harness.sha(SOURCE.read_bytes()),
        "fixtures_source_sha256": harness.sha((SOURCE.parent / "instance_fixtures.rs").read_bytes()),
        "ordinary_output_signatures": signatures,
        "instance_cases_per_configuration": 65 * 6 * 3,
        "real_font_glyph_observations_per_configuration": 3 * 6 * 3 * 3,
        "covers_normalized_instances_and_feature_variation_selection_only": True,
        "user_coordinate_normalization_covered": False,
        "whole_cold_constructor_or_shaping_admission_complete": False,
    }
    (output / "instance-verification.json").write_text(json.dumps(receipt, indent=2) + "\n")
    print(json.dumps(receipt, indent=2))


if __name__ == "__main__":
    main()
