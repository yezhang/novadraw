#!/usr/bin/env python3
"""Replay nine frozen patches and verify private-language plan admission."""
import argparse
import difflib
import json
import os
from pathlib import Path
import shutil
import tarfile
import tempfile

import check_harfrust_buffer_admission as harness

CHANGED = {
    "src/hb/common.rs", "src/hb/tag.rs", "src/hb/ot_map.rs",
    "src/hb/ot_shape.rs", "src/hb/ot_shape_plan.rs",
}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--target-dir", type=Path, required=True)
    args = parser.parse_args()
    output = harness.artifact_output(args.output)
    archive = output / "harfrust-0.3.2.crate"
    if harness.sha(archive.read_bytes()) != harness.ARCHIVE_SHA:
        raise SystemExit("Original archive checksum mismatch")
    prior = json.loads((output / "plan-layout-verification.json").read_text())
    patches = {**prior["prior_patches"], "plan-layout": prior["incremental_patch_sha256"]}
    base = harness.REPO / "target/text-harfrust-private-language"
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
                raise SystemExit("Unexpected archive member")
            dest = original / relative
            dest.parent.mkdir(parents=True, exist_ok=True)
            dest.write_bytes(source.extractfile(member).read())
    before = root / "after-plan-layout"
    shutil.copytree(original, before)
    for name, checksum in patches.items():
        path = output / f"harfrust-0.3.2-{name}.patch"
        if harness.sha(path.read_bytes()) != checksum:
            raise SystemExit(f"Frozen patch changed: {name}")
        harness.run(["git", "apply", "--no-index", str(path)],
                    output / f"private-language-{name}-replay.log", os.environ, before)
    sources = {str(p.relative_to(before)) for p in (before / "src").rglob("*") if p.is_file()}
    actual = {str(p.relative_to(harness.PRODUCTION))
              for p in (harness.PRODUCTION / "src").rglob("*") if p.is_file()}
    if actual != sources:
        raise SystemExit("Unexpected source addition or removal")
    patch, changed = [], []
    for relative in sorted(actual):
        old = (before / relative).read_bytes()
        new = (harness.PRODUCTION / relative).read_bytes()
        if old == new:
            continue
        if relative not in CHANGED:
            raise SystemExit(f"Unrelated source changed: {relative}")
        changed.append(relative)
        patch.extend(difflib.unified_diff(
            old.decode().splitlines(keepends=True), new.decode().splitlines(keepends=True),
            fromfile=f"a/{relative}", tofile=f"b/{relative}",
        ))
    if set(changed) != CHANGED:
        raise SystemExit("Expected source change missing")
    path = output / "harfrust-0.3.2-private-language.patch"
    path.write_bytes("".join(patch).encode())
    applied = root / "after-private-language"
    shutil.copytree(before, applied)
    harness.run(["git", "apply", "--no-index", str(path)],
                output / "private-language-patch-replay.log", os.environ, applied)
    for relative in actual:
        if (applied / relative).read_bytes() != (harness.PRODUCTION / relative).read_bytes():
            raise SystemExit(f"Replay differs: {relative}")
    shutil.copy2(path, harness.REPO / "dependencies" / path.name)
    env = os.environ.copy()
    env["CARGO_TARGET_DIR"] = str(args.target_dir.resolve())
    env["CC"] = "/Library/Developer/CommandLineTools/usr/bin/clang"
    env["CARGO_TARGET_AARCH64_APPLE_DARWIN_LINKER"] = env["CC"]
    signatures, counts = {}, {}
    for name, library in [("original", original), ("production", harness.PRODUCTION)]:
        lib_package = root / f"lib-{name}"
        harness.manifest(lib_package, library / "src/lib.rs", original / "Cargo.toml")
        for oracle, expected in [("plan", 39744), ("plan_layout", 50688),
                                 ("private_language", 6720)]:
            harness.SOURCE = harness.REPO / f"tests/consumers/harfrust-admission/src/{oracle}_oracle.rs"
            package = root / f"oracle-{name}-{oracle}"
            for profile in ["native", "portable"]:
                harness.manifest(package, lib_package, original / "Cargo.toml", binary=True)
                manifest = package / "Cargo.toml"
                if profile == "portable":
                    manifest.write_text(manifest.read_text().replace(
                        "path = " + json.dumps(str(lib_package)) + " }",
                        "path = " + json.dumps(str(lib_package)) + ", default-features = false }",
                    ))
                harness.run(["cargo", "+1.96.0", "build", "--offline", "--manifest-path",
                             str(manifest), "--bin", "oracle"],
                            output / f"private-language-{oracle}-{name}-{profile}-build.log", env)
                log = output / f"private-language-{oracle}-{name}-{profile}.log"
                harness.run([str(args.target_dir.resolve() / "debug/oracle")], log, env)
                key = f"{oracle}/{name}/{profile}"
                signatures[key] = harness.sha(log.read_bytes())
                counts[key] = len(log.read_bytes().splitlines())
                if counts[key] != expected:
                    raise SystemExit(f"Wrong observation count: {key}/{counts[key]}")
        for profile, flags in [("native", []), ("portable", ["--no-default-features"])]:
            if name == "production":
                harness.run(["cargo", "+1.96.0", "test", "--offline", "--manifest-path",
                             str(lib_package / "Cargo.toml"), "--lib", *flags],
                            output / f"private-language-private-{profile}.log", env)
    for oracle in ["plan", "plan_layout", "private_language"]:
        if len({value for key, value in signatures.items() if key.startswith(oracle + "/")}) != 1:
            raise SystemExit(f"Ordinary output mismatch: {oracle}")
    consumer = {}
    for profile, flags in [("native", []), ("portable", ["--no-default-features", "--features", "portable"])]:
        log = output / f"private-language-consumer-{profile}.log"
        harness.run(["cargo", "+1.96.0", "test", "--offline", "--manifest-path",
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
        "changed_after_plan_layout": sorted(CHANGED),
        "unchanged_source_files_after_plan_layout": len(sources - CHANGED),
        "ordinary_output_signatures": signatures, "observations_per_configuration": counts,
        "oracle_sources_sha256": {name: harness.sha((harness.REPO /
            f"tests/consumers/harfrust-admission/src/{name}_oracle.rs").read_bytes())
            for name in ["plan", "plan_layout", "private_language"]},
        "external_test_log_sha256": consumer,
        "private_test_log_sha256": {profile: harness.sha(
            (output / f"private-language-private-{profile}.log").read_bytes())
            for profile in ["native", "portable"]},
        "private_language_plan_admission_supported": True,
        "general_language_callbacks_and_shaping_admission_complete": False,
        "core_staged_adapter_enabled": False,
    }
    (output / "private-language-verification.json").write_text(json.dumps(receipt, indent=2) + "\n")
    print(json.dumps(receipt, indent=2))


if __name__ == "__main__":
    main()
