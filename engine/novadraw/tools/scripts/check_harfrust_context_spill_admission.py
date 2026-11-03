#!/usr/bin/env python3
"""Freeze matching backup storage after seventeen immutable patches."""
import argparse
import difflib
import json
import os
import shutil
from pathlib import Path

import check_harfrust_buffer_admission as harness
from check_harfrust_context_admission import reconstruct
from check_harfrust_multiple_admission import observe
import text_admission_build

CHANGED = {"src/hb/shape_admission.rs", "src/hb/ot_layout.rs", "src/hb/ot_layout_gsubgpos.rs"}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--target-dir", type=Path, required=True)
    parser.add_argument("--linker", choices=["clang", "rust-lld"], default="rust-lld")
    args = parser.parse_args()
    output = harness.artifact_output(args.output)
    prior = json.loads((output / "context-verification.json").read_text())
    patches = {**prior["prior_patches"], "context": prior["incremental_patch_sha256"]}
    root, original, before = reconstruct(output, patches, "text-harfrust-spill-")
    sources = {str(p.relative_to(before)) for p in (before / "src").rglob("*") if p.is_file()}
    actual = {str(p.relative_to(harness.PRODUCTION))
              for p in (harness.PRODUCTION / "src").rglob("*") if p.is_file()}
    if sources != actual:
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
    path = output / "harfrust-0.3.2-context-spill.patch"
    path.write_bytes("".join(patch).encode())
    applied = root / "after"
    shutil.copytree(before, applied)
    harness.run(["git", "apply", "--no-index", str(path)],
                output / "context-spill-patch-replay.log", os.environ, applied)
    for relative in actual:
        if (applied / relative).read_bytes() != (harness.PRODUCTION / relative).read_bytes():
            raise SystemExit(f"Replay differs: {relative}")
    shutil.copy2(path, harness.REPO / "dependencies" / path.name)
    env = text_admission_build.environment(args.target_dir.resolve(), args.linker)
    build_root = harness.REPO / "target/text-harfrust-context-spill-build"
    build_root.mkdir(exist_ok=True)
    signatures, counts = {}, {}
    expected = {key.split("/")[0]: value for key, value in prior["observations_per_configuration"].items()}
    expected["context_spill"] = 3840
    for name, library in [("original", original), ("production", harness.PRODUCTION)]:
        lib = build_root / f"lib-{name}"
        harness.manifest(lib, library / "src/lib.rs", original / "Cargo.toml")
        for oracle, denominator in expected.items():
            source = harness.REPO / f"tests/consumers/harfrust-admission/src/{oracle}_oracle.rs"
            if oracle != "context_spill" and harness.sha(source.read_bytes()) != prior["oracle_sources_sha256"][oracle]:
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
                            output / f"context-spill-{oracle}-{name}-{profile}-build.log", env)
                key = f"{oracle}/{name}/{profile}"
                signatures[key], counts[key] = observe(args.target_dir.resolve() / "debug/oracle",
                    output / f"context-spill-{oracle}-{name}-{profile}.log.gz", env)
                if counts[key] != denominator:
                    raise SystemExit(f"Wrong observations: {key}/{counts[key]}")
        if name == "production":
            for profile, flags in [("native", []), ("portable", ["--no-default-features"])]:
                harness.run(["cargo", "+1.96.0", "test", "--offline", "--manifest-path",
                             str(lib / "Cargo.toml"), "--lib", *flags],
                            output / f"context-spill-private-{profile}.log", env)
    for oracle in expected:
        selected = {v for k, v in signatures.items() if k.startswith(oracle + "/")}
        if len(selected) != 1:
            raise SystemExit(f"Ordinary output mismatch: {oracle}")
        if oracle != "context_spill" and selected != {v for k, v in
                prior["ordinary_output_signatures"].items() if k.startswith(oracle + "/")}:
            raise SystemExit(f"Frozen signature changed: {oracle}")
    consumer, private = {}, {}
    for profile, flags in [("native", []), ("portable", ["--no-default-features", "--features", "portable"])]:
        log = output / f"context-spill-consumer-{profile}.log"
        harness.run(["cargo", "+1.96.0", "test", "--offline", "--manifest-path",
                     str(harness.REPO / "tests/consumers/harfrust-admission/Cargo.toml"),
                     "--lib", *flags], log, env)
        consumer[profile] = harness.sha(log.read_bytes())
        private[profile] = harness.sha((output / f"context-spill-private-{profile}.log").read_bytes())
    for relative in actual:
        if (applied / relative).read_bytes() != (harness.PRODUCTION / relative).read_bytes():
            raise SystemExit(f"Source changed during verification: {relative}")
    receipt = {
        "archive_sha256": harness.ARCHIVE_SHA, "prior_patches": patches,
        "incremental_patch_sha256": harness.sha(path.read_bytes()),
        "patch_replays_and_reproduces_exact_source": True,
        "changed_after_context": sorted(CHANGED), "unchanged_source_files": len(sources - CHANGED),
        "total_source_files": len(actual), "ordinary_output_signatures": signatures,
        "observations_per_configuration": counts,
        "build_configuration": "Native/portable, debug=0, incremental=0, jobs=1",
        "ordinary_output_storage": "lossless gzip; signatures/counts use decompressed bytes",
        "external_test_log_sha256": consumer, "private_test_log_sha256": private,
        "oracle_sources_sha256": {n: harness.sha((harness.REPO /
            f"tests/consumers/harfrust-admission/src/{n}_oracle.rs").read_bytes()) for n in expected},
        "fixture_sources_sha256": {n: harness.sha((harness.REPO /
            f"tests/consumers/harfrust-admission/src/{n}_fixtures.rs").read_bytes())
            for n in ["single", "multiple", "ligature", "context"]},
        "production_sources_sha256": {r: harness.sha((harness.PRODUCTION / r).read_bytes()) for r in CHANGED},
        "shaping_admission_complete": False, "core_staged_adapter_enabled": False,
    }
    (output / "context-spill-verification.json").write_text(json.dumps(receipt, indent=2) + "\n")
    print(json.dumps(receipt, indent=2))


if __name__ == "__main__":
    main()
