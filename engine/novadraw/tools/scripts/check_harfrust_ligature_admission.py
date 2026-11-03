#!/usr/bin/env python3
"""Freeze ligature admission after fifteen immutable dependency patches."""
import argparse
import difflib
import json
import os
from pathlib import Path
import shutil
import tarfile
import tempfile

import check_harfrust_buffer_admission as harness
from check_harfrust_multiple_admission import observe
import text_admission_build

CHANGED = {"src/hb/shape_admission.rs", "src/hb/ot_layout.rs"}


def reconstruct(output, patches, prefix):
    archive = output / "harfrust-0.3.2.crate"
    if harness.sha(archive.read_bytes()) != harness.ARCHIVE_SHA:
        raise SystemExit("Archive checksum mismatch")
    root = Path(tempfile.mkdtemp(prefix=prefix, dir=harness.REPO / "target"))
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
    before = root / "before"
    shutil.copytree(original, before)
    for name, checksum in patches.items():
        path = output / f"harfrust-0.3.2-{name}.patch"
        if harness.sha(path.read_bytes()) != checksum:
            raise SystemExit(f"Frozen patch changed: {name}")
        harness.run(["git", "apply", "--no-index", str(path)],
                    output / f"ligature-{name}-replay.log", os.environ, before)
    return root, original, before


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--target-dir", type=Path, required=True)
    parser.add_argument("--linker", choices=["clang", "rust-lld"], default="rust-lld")
    args = parser.parse_args()
    output = harness.artifact_output(args.output)
    prior = json.loads((output / "multiple-verification.json").read_text())
    patches = {**prior["prior_patches"], "multiple": prior["incremental_patch_sha256"]}
    root, original, before = reconstruct(output, patches, "text-harfrust-ligature-")
    sources = {str(p.relative_to(before)) for p in (before / "src").rglob("*") if p.is_file()}
    actual = {str(p.relative_to(harness.PRODUCTION))
              for p in (harness.PRODUCTION / "src").rglob("*") if p.is_file()}
    if actual != sources:
        raise SystemExit("Unexpected source addition or removal")
    patch, changed = [], set()
    for relative in sorted(actual):
        old, new = (before / relative).read_bytes(), (harness.PRODUCTION / relative).read_bytes()
        if old == new:
            continue
        if relative not in CHANGED:
            raise SystemExit(f"Unrelated source changed: {relative}")
        changed.add(relative)
        patch.extend(difflib.unified_diff(
            old.decode().splitlines(keepends=True), new.decode().splitlines(keepends=True),
            fromfile=f"a/{relative}", tofile=f"b/{relative}"))
    if changed != CHANGED:
        raise SystemExit("Expected source change missing")
    path = output / "harfrust-0.3.2-ligature.patch"
    path.write_bytes("".join(patch).encode())
    applied = root / "after"
    shutil.copytree(before, applied)
    harness.run(["git", "apply", "--no-index", str(path)],
                output / "ligature-patch-replay.log", os.environ, applied)
    for relative in actual:
        if (applied / relative).read_bytes() != (harness.PRODUCTION / relative).read_bytes():
            raise SystemExit(f"Replay differs: {relative}")
    shutil.copy2(path, harness.REPO / "dependencies" / path.name)
    env = text_admission_build.environment(args.target_dir.resolve(), args.linker)
    build_root = harness.REPO / "target/text-harfrust-ligature-build"
    build_root.mkdir(exist_ok=True)
    signatures, counts = {}, {}
    expected = {"plan": 39744, "plan_layout": 50688, "private_language": 6720,
                "general_language": 24048, "script_callbacks": 51840, "shape": 12672,
                "single": 18432, "multiple": 36864, "ligature": 34560}
    for name, library in [("original", original), ("production", harness.PRODUCTION)]:
        lib = build_root / f"lib-{name}"
        harness.manifest(lib, library / "src/lib.rs", original / "Cargo.toml")
        for oracle, denominator in expected.items():
            source = harness.REPO / f"tests/consumers/harfrust-admission/src/{oracle}_oracle.rs"
            if oracle != "ligature" and harness.sha(source.read_bytes()) != prior["oracle_sources_sha256"][oracle]:
                raise SystemExit(f"Frozen oracle changed: {oracle}")
            harness.SOURCE = source
            package = build_root / f"oracle-{name}-{oracle}"
            for profile in ["native", "portable"]:
                harness.manifest(package, lib, original / "Cargo.toml", binary=True)
                manifest = package / "Cargo.toml"
                if profile == "portable":
                    manifest.write_text(manifest.read_text().replace(
                        "path = " + json.dumps(str(lib)) + " }",
                        "path = " + json.dumps(str(lib)) + ", default-features = false }"))
                harness.run(["cargo", "+1.96.0", "build", "--offline", "--manifest-path",
                             str(manifest), "--bin", "oracle"],
                            output / f"ligature-{oracle}-{name}-{profile}-build.log", env)
                key = f"{oracle}/{name}/{profile}"
                signatures[key], counts[key] = observe(args.target_dir.resolve() / "debug/oracle",
                    output / f"ligature-{oracle}-{name}-{profile}.log.gz", env)
                if counts[key] != denominator:
                    raise SystemExit(f"Wrong observation count: {key}/{counts[key]}")
        if name == "production":
            for profile, flags in [("native", []), ("portable", ["--no-default-features"])]:
                harness.run(["cargo", "+1.96.0", "test", "--offline", "--manifest-path",
                             str(lib / "Cargo.toml"), "--lib", *flags],
                            output / f"ligature-private-{profile}.log", env)
    for oracle in expected:
        selected = {v for k, v in signatures.items() if k.startswith(oracle + "/")}
        if len(selected) != 1:
            raise SystemExit(f"Ordinary output mismatch: {oracle}")
        if oracle != "ligature" and selected != {v for k, v in
                prior["ordinary_output_signatures"].items() if k.startswith(oracle + "/")}:
            raise SystemExit(f"Frozen ordinary signature changed: {oracle}")
    consumer = {}
    for profile, flags in [("native", []), ("portable", ["--no-default-features", "--features", "portable"])]:
        log = output / f"ligature-consumer-{profile}.log"
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
        "changed_after_multiple": sorted(CHANGED),
        "unchanged_source_files_after_multiple": len(sources - CHANGED),
        "total_source_files": len(actual), "ordinary_output_signatures": signatures,
        "observations_per_configuration": counts,
        "build_configuration": "Native/portable, debug=0, incremental=0, jobs=1",
        "ordinary_output_storage": "lossless gzip; signatures/counts refer to decompressed bytes",
        "external_test_log_sha256": consumer,
        "private_test_log_sha256": {p: harness.sha((output / f"ligature-private-{p}.log").read_bytes())
                                  for p in ["native", "portable"]},
        "oracle_sources_sha256": {n: harness.sha((harness.REPO /
            f"tests/consumers/harfrust-admission/src/{n}_oracle.rs").read_bytes()) for n in expected},
        "fixture_sources_sha256": {n: harness.sha((harness.REPO /
            f"tests/consumers/harfrust-admission/src/{n}_fixtures.rs").read_bytes())
            for n in ["single", "multiple", "ligature"]},
        "production_sources_sha256": {r: harness.sha((harness.PRODUCTION / r).read_bytes()) for r in CHANGED},
        "shaping_admission_complete": False, "core_staged_adapter_enabled": False,
    }
    (output / "ligature-verification.json").write_text(json.dumps(receipt, indent=2) + "\n")
    print(json.dumps(receipt, indent=2))


if __name__ == "__main__":
    main()
