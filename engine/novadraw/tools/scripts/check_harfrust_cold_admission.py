#!/usr/bin/env python3
"""Freeze cold-cache admission after three patches; verify source and outputs."""
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
SOURCE = REPO / "tests/consumers/harfrust-admission/src/cold_oracle.rs"
CHANGED = {
    "src/lib.rs", "src/hb/face.rs", "src/hb/cache.rs", "src/hb/aat/mod.rs",
    "src/hb/ot/mod.rs", "src/hb/ot/lookup.rs", "src/hb/set_digest.rs", "src/hb/tables.rs",
}
ADDED = {"src/hb/cold_admission.rs"}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--target-dir", type=Path, required=True)
    args = parser.parse_args()
    output = harness.artifact_output(args.output)
    archive = output / "harfrust-0.3.2.crate"
    if harness.sha(archive.read_bytes()) != harness.ARCHIVE_SHA:
        raise SystemExit("Original archive checksum mismatch")
    prior = json.loads((output / "variation-verification.json").read_text())
    patches = [
        ("buffer", prior["prior_patches"]["buffer"]),
        ("instance", prior["prior_patches"]["instance"]),
        ("variation", prior["incremental_patch_sha256"]),
    ]
    base = REPO / "target/text-harfrust-cold"
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
    before = root / "after-variation"
    shutil.copytree(original, before)
    for name, checksum in patches:
        path = output / f"harfrust-0.3.2-{name}.patch"
        if harness.sha(path.read_bytes()) != checksum:
            raise SystemExit(f"Frozen {name} patch changed")
        harness.run(["git", "apply", "--no-index", str(path)],
                    output / f"cold-{name}-replay.log", os.environ, before)
    sources = {str(p.relative_to(before)) for p in (before / "src").rglob("*") if p.is_file()}
    actual = {str(p.relative_to(PRODUCTION)) for p in (PRODUCTION / "src").rglob("*") if p.is_file()}
    if actual != sources | ADDED:
        raise SystemExit("Unexpected added or removed source")
    patch = []
    for relative in sorted(sources | ADDED):
        old = (before / relative).read_bytes() if relative in sources else b""
        new = (PRODUCTION / relative).read_bytes()
        if old == new:
            continue
        if relative not in CHANGED | ADDED:
            raise SystemExit(f"Unrelated source changed: {relative}")
        patch.extend(difflib.unified_diff(
            old.decode().splitlines(keepends=True), new.decode().splitlines(keepends=True),
            fromfile=f"a/{relative}" if relative in sources else "/dev/null",
            tofile=f"b/{relative}",
        ))
    patch_path = output / "harfrust-0.3.2-cold.patch"
    patch_path.write_bytes("".join(patch).encode())
    applied = root / "after-cold"
    shutil.copytree(before, applied)
    harness.run(["git", "apply", "--no-index", str(patch_path)],
                output / "cold-patch-replay.log", os.environ, applied)
    for relative in sources | ADDED:
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
                        output / f"cold-oracle-{name}-{profile}-build.log", env)
            log = output / f"cold-oracle-{name}-{profile}.log"
            harness.run([str(args.target_dir.resolve() / "debug/oracle")], log, env)
            signatures[f"{name}-{profile}"] = harness.sha(log.read_bytes())
    if len(set(signatures.values())) != 1:
        raise SystemExit(f"Ordinary cold constructor outputs differ: {signatures}")
    private = root / "lib-production"
    private_signatures = {}
    for profile, options in [("native", []), ("portable", ["--no-default-features"])]:
        log = output / f"cold-private-{profile}.log"
        harness.run(["cargo", "+1.96.0", "test", "--offline", "--manifest-path",
                     str(private / "Cargo.toml"), "--lib", *options], log, env)
        private_signatures[profile] = harness.sha(log.read_bytes())
    consumer = REPO / "tests/consumers/harfrust-admission/Cargo.toml"
    external_signatures = {}
    for profile, options in [
        ("native", []), ("portable", ["--no-default-features", "--features", "portable"]),
    ]:
        log = output / f"cold-consumer-{profile}.log"
        harness.run(["cargo", "+1.96.0", "test", "--offline", "--manifest-path",
                     str(consumer), "--lib", *options], log, env)
        external_signatures[profile] = harness.sha(log.read_bytes())
    receipt = {
        "archive_sha256": harness.ARCHIVE_SHA,
        "prior_patches": dict(patches),
        "incremental_patch_sha256": harness.sha(patch_path.read_bytes()),
        "patch_replays_and_reproduces_exact_source": True,
        "changed_after_variation": sorted(CHANGED),
        "added_after_variation": sorted(ADDED),
        "unchanged_source_files_after_variation": len(sources - CHANGED),
        "oracle_source_sha256": harness.sha(SOURCE.read_bytes()),
        "fixtures_source_sha256": harness.sha((SOURCE.parent / "cold_fixtures.rs").read_bytes()),
        "ordinary_output_signatures": signatures,
        "fonts_per_configuration": 143,
        "glyph_observations_per_configuration": 143 * 2 * 6 * 2,
        "private_test_log_sha256": private_signatures,
        "external_test_log_sha256": external_signatures,
        "admitted_native": "No AAT table; lazy lookup slots only",
        "admitted_portable": "No AAT table; zero eager lookups",
        "lookup_info_or_aat_subtable_admission_complete": False,
        "whole_cold_constructor_or_shaping_admission_complete": False,
        "core_staged_adapter_enabled": False,
    }
    (output / "cold-verification.json").write_text(json.dumps(receipt, indent=2) + "\n")
    print(json.dumps(receipt, indent=2))


if __name__ == "__main__":
    main()
