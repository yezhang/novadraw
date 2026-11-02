#!/usr/bin/env python3
"""Replay four frozen patches and verify the incremental lookup admission."""
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
SOURCE = REPO / "tests/consumers/harfrust-admission/src/lookup_oracle.rs"
CHANGED = {
    "src/lib.rs", "src/hb/face.rs", "src/hb/cold_admission.rs",
    "src/hb/ot/mod.rs", "src/hb/ot/lookup.rs", "src/hb/ot/contextual.rs",
    "src/hb/ot/gpos/pair.rs", "src/hb/ot/gsub/ligature.rs",
    "src/hb/ot_layout_gsubgpos.rs", "src/hb/set_digest.rs",
}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--target-dir", type=Path, required=True)
    args = parser.parse_args()
    output = args.output.resolve()
    archive = output / "harfrust-0.3.2.crate"
    if harness.sha(archive.read_bytes()) != harness.ARCHIVE_SHA:
        raise SystemExit("Original archive checksum mismatch")
    cold = json.loads((output / "cold-verification.json").read_text())
    patches = {**cold["prior_patches"], "cold": cold["incremental_patch_sha256"]}
    base = REPO / "target/text-harfrust-lookup"
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
    before = root / "after-cold"
    shutil.copytree(original, before)
    for name, checksum in patches.items():
        path = output / f"harfrust-0.3.2-{name}.patch"
        if harness.sha(path.read_bytes()) != checksum:
            raise SystemExit(f"Frozen {name} patch changed")
        harness.run(["git", "apply", "--no-index", str(path)],
                    output / f"lookup-{name}-replay.log", os.environ, before)
    sources = {str(p.relative_to(before)) for p in (before / "src").rglob("*") if p.is_file()}
    actual = {str(p.relative_to(PRODUCTION)) for p in (PRODUCTION / "src").rglob("*") if p.is_file()}
    if actual != sources:
        raise SystemExit("Unexpected source added or removed")
    patch = []
    changed = []
    for relative in sorted(sources):
        old, new = (before / relative).read_bytes(), (PRODUCTION / relative).read_bytes()
        if old == new:
            continue
        if relative not in CHANGED:
            raise SystemExit(f"Unrelated source changed: {relative}")
        changed.append(relative)
        patch.extend(difflib.unified_diff(
            old.decode().splitlines(keepends=True), new.decode().splitlines(keepends=True),
            fromfile=f"a/{relative}", tofile=f"b/{relative}",
        ))
    patch_path = output / "harfrust-0.3.2-lookup.patch"
    patch_path.write_bytes("".join(patch).encode())
    applied = root / "after-lookup"
    shutil.copytree(before, applied)
    harness.run(["git", "apply", "--no-index", str(patch_path)],
                output / "lookup-patch-replay.log", os.environ, applied)
    for relative in sources:
        if (applied / relative).read_bytes() != (PRODUCTION / relative).read_bytes():
            raise SystemExit(f"Patch replay differs: {relative}")
    shutil.copy2(patch_path, REPO / "dependencies" / patch_path.name)
    env = os.environ.copy()
    env["CARGO_TARGET_DIR"] = str(args.target_dir.resolve())
    env["CC"] = "/Library/Developer/CommandLineTools/usr/bin/clang"
    env["CARGO_TARGET_AARCH64_APPLE_DARWIN_LINKER"] = env["CC"]
    harness.SOURCE = SOURCE
    signatures, observations = {}, {}
    for name, library in [("original", original), ("production", PRODUCTION)]:
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
                        output / f"lookup-oracle-{name}-{profile}-build.log", env)
            log = output / f"lookup-oracle-{name}-{profile}.log"
            harness.run([str(args.target_dir.resolve() / "debug/oracle")], log, env)
            signatures[f"{name}-{profile}"] = harness.sha(log.read_bytes())
            observations[f"{name}-{profile}"] = len(log.read_bytes().splitlines())
    if len(set(signatures.values())) != 1:
        raise SystemExit(f"Ordinary lookup outputs differ: {signatures}")
    private_logs, consumer_logs = {}, {}
    private = root / "lib-production"
    for profile, flags in [("native", []), ("portable", ["--no-default-features"])]:
        log = output / f"lookup-private-{profile}.log"
        harness.run(["cargo", "+1.96.0", "test", "--offline", "--manifest-path",
                     str(private / "Cargo.toml"), "--lib", *flags], log, env)
        private_logs[profile] = harness.sha(log.read_bytes())
    consumer = REPO / "tests/consumers/harfrust-admission/Cargo.toml"
    for profile, flags in [
        ("native", []), ("portable", ["--no-default-features", "--features", "portable"]),
    ]:
        log = output / f"lookup-consumer-{profile}.log"
        harness.run(["cargo", "+1.96.0", "test", "--offline", "--manifest-path",
                     str(consumer), "--lib", *flags], log, env)
        consumer_logs[profile] = harness.sha(log.read_bytes())
    receipt = {
        "archive_sha256": harness.ARCHIVE_SHA, "prior_patches": patches,
        "incremental_patch_sha256": harness.sha(patch_path.read_bytes()),
        "patch_replays_and_reproduces_exact_source": True,
        "changed_after_cold": changed,
        "unchanged_source_files_after_cold": len(sources) - len(changed),
        "oracle_source_sha256": harness.sha(SOURCE.read_bytes()),
        "fixture_source_sha256": {n: harness.sha((SOURCE.parent / n).read_bytes())
                                 for n in ["cold_fixtures.rs", "lookup_fixtures.rs"]},
        "ordinary_output_signatures": signatures,
        "observations_per_configuration": observations,
        "private_test_log_sha256": private_logs, "external_test_log_sha256": consumer_logs,
        "lookup_cache_admission_complete": True,
        "aat_or_plan_or_shaping_admission_complete": False,
        "core_staged_adapter_enabled": False,
    }
    (output / "lookup-verification.json").write_text(json.dumps(receipt, indent=2) + "\n")
    print(json.dumps(receipt, indent=2))


if __name__ == "__main__":
    main()
