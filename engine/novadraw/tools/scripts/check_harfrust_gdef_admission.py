#!/usr/bin/env python3
"""Freeze bounded GDEF property admission after twenty-two immutable patches."""
import argparse
import difflib
import gzip
import hashlib
import json
import os
import shutil

from pathlib import Path
import check_harfrust_buffer_admission as h
from check_harfrust_context_admission import reconstruct
from check_harfrust_multiple_admission import observe
import text_admission_build

CHANGED = {"src/hb/ot/mod.rs", "src/hb/shape_admission.rs"}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--target-dir", type=Path, required=True)
    parser.add_argument("--linker", choices=["clang", "rust-lld"], default="rust-lld")
    parser.add_argument("--reuse-frozen-oracles", action="store_true",
        help="Reuse only prior fifteen matrices after exact current patch and frozen output checks; always rerun GDEF")
    args = parser.parse_args()
    output = h.artifact_output(args.output)
    prior = json.loads((output / "simple-unicode-verification.json").read_text())
    patches = {**prior["prior_patches"], "simple-unicode": prior["incremental_patch_sha256"]}
    root, original, before = reconstruct(output, patches, "text-harfrust-gdef-")
    sources = {str(p.relative_to(before)) for p in (before / "src").rglob("*") if p.is_file()}
    actual = {str(p.relative_to(h.PRODUCTION)) for p in (h.PRODUCTION / "src").rglob("*") if p.is_file()}
    if sources != actual:
        raise SystemExit("Source addition/removal outside slice")
    patch, changed = [], set()
    for relative in sorted(actual):
        old, new = (before / relative).read_bytes(), (h.PRODUCTION / relative).read_bytes()
        if old == new:
            continue
        if relative not in CHANGED:
            raise SystemExit(f"Unrelated source changed: {relative}")
        changed.add(relative)
        patch.extend(difflib.unified_diff(
            old.decode().splitlines(keepends=True), new.decode().splitlines(keepends=True),
            fromfile=f"a/{relative}", tofile=f"b/{relative}"))
    if changed != CHANGED:
        raise SystemExit("Expected source changes missing")
    path = output / "harfrust-0.3.2-gdef.patch"
    patch_bytes = "".join(patch).encode()
    if args.reuse_frozen_oracles and path.read_bytes() != patch_bytes:
        raise SystemExit("Cannot reuse outputs after a production source change")
    path.write_bytes(patch_bytes)
    applied = root / "after"
    shutil.copytree(before, applied)
    h.run(["git", "apply", "--no-index", str(path)], output / "gdef-patch-replay.log", os.environ, applied)
    for relative in actual:
        if (applied / relative).read_bytes() != (h.PRODUCTION / relative).read_bytes():
            raise SystemExit(f"Replay differs: {relative}")
    shutil.copy2(path, h.REPO / "dependencies" / path.name)
    env = text_admission_build.environment(args.target_dir.resolve(), args.linker)
    build = h.REPO / "target/text-harfrust-gdef-build"
    build.mkdir(exist_ok=True)
    expected = {key.split("/")[0]: count for key, count in prior["observations_per_configuration"].items()}
    expected["gdef"] = 110592
    for name, checksum in prior["fixture_sources_sha256"].items():
        if h.sha((h.REPO / f"tests/consumers/harfrust-admission/src/{name}_fixtures.rs").read_bytes()) != checksum:
            raise SystemExit(f"Frozen fixture changed: {name}")
    signatures, counts = {}, {}
    for owner, library in [("original", original), ("production", h.PRODUCTION)]:
        lib = build / f"lib-{owner}"
        h.manifest(lib, library / "src/lib.rs", original / "Cargo.toml")
    for profile, flags in [("native", []), ("portable", ["--no-default-features"])]:
        h.run(["cargo", "+1.96.0", "test", "--offline", "--manifest-path",
            str(build / "lib-production/Cargo.toml"), "--lib", *flags],
            output / f"gdef-private-{profile}.log", env)
    for owner in ["original", "production"]:
        lib = build / f"lib-{owner}"
        for oracle, count in expected.items():
            source = h.REPO / f"tests/consumers/harfrust-admission/src/{oracle}_oracle.rs"
            if oracle != "gdef" and h.sha(source.read_bytes()) != prior["oracle_sources_sha256"][oracle]:
                raise SystemExit(f"Frozen oracle changed: {oracle}")
            h.SOURCE = source
            package = build / f"oracle-{owner}-{oracle}"
            for profile in ["native", "portable"]:
                key = f"{oracle}/{owner}/{profile}"
                observed = output / f"gdef-{oracle}-{owner}-{profile}.log.gz"
                if args.reuse_frozen_oracles and oracle != "gdef":
                    digest, observed_count = hashlib.sha256(), 0
                    with gzip.open(observed, "rb") as saved:
                        while chunk := saved.read(1024 * 1024):
                            digest.update(chunk)
                            observed_count += chunk.count(b"\n")
                    if digest.hexdigest() != prior["ordinary_output_signatures"][key] or observed_count != count:
                        raise SystemExit(f"Frozen saved output differs: {key}")
                    signatures[key], counts[key] = digest.hexdigest(), observed_count
                    continue
                h.manifest(package, lib, original / "Cargo.toml", binary=True)
                manifest = package / "Cargo.toml"
                if profile == "portable":
                    manifest.write_text(manifest.read_text().replace(
                        "path = " + json.dumps(str(lib)) + " }",
                        "path = " + json.dumps(str(lib)) + ", default-features = false }"))
                h.run(["cargo", "+1.96.0", "build", "--offline", "--manifest-path", str(manifest), "--bin", "oracle"],
                    output / f"gdef-{oracle}-{owner}-{profile}-build.log", env)
                signatures[key], counts[key] = observe(args.target_dir.resolve() / "debug/oracle",
                    observed, env)
                if counts[key] != count:
                    raise SystemExit(f"Wrong observation count: {key}/{counts[key]}")
    for oracle in expected:
        selected = {v for k, v in signatures.items() if k.startswith(oracle + "/")}
        if len(selected) != 1:
            raise SystemExit(f"Ordinary outputs differ: {oracle}")
        if oracle != "gdef" and selected != {
            v for k, v in prior["ordinary_output_signatures"].items() if k.startswith(oracle + "/")
        }:
            raise SystemExit(f"Frozen signature changed: {oracle}")
    consumer, private = {}, {}
    for profile, flags in [("native", []), ("portable", ["--no-default-features", "--features", "portable"])]:
        log = output / f"gdef-consumer-{profile}.log"
        h.run(["cargo", "+1.96.0", "test", "--offline", "--manifest-path",
            str(h.REPO / "tests/consumers/harfrust-admission/Cargo.toml"), "--lib", *flags], log, env)
        consumer[profile] = h.sha(log.read_bytes())
        private[profile] = h.sha((output / f"gdef-private-{profile}.log").read_bytes())
    for relative in actual:
        if (applied / relative).read_bytes() != (h.PRODUCTION / relative).read_bytes():
            raise SystemExit(f"Source changed during verification: {relative}")
    receipt = {
        "archive_sha256": h.ARCHIVE_SHA, "prior_patches": patches,
        "incremental_patch_sha256": h.sha(path.read_bytes()),
        "patch_replays_and_reproduces_exact_source": True,
        "changed_after_simple_unicode": sorted(CHANGED),
        "unchanged_source_files": len(sources - CHANGED), "total_source_files": len(actual),
        "ordinary_output_signatures": signatures, "observations_per_configuration": counts,
        "build_configuration": "Native/portable, debug=0, incremental=0, jobs=1",
        "ordinary_output_storage": "lossless gzip; counts/signatures use decompressed bytes",
        "external_test_log_sha256": consumer, "private_test_log_sha256": private,
        "consumer_module_source_sha256": h.sha((h.REPO / "tests/consumers/harfrust-admission/src/lib.rs").read_bytes()),
        "consumer_test_source_sha256": h.sha((h.REPO / "tests/consumers/harfrust-admission/src/gdef_tests.rs").read_bytes()),
        "updated_legacy_test_source_sha256": h.sha((h.REPO / "tests/consumers/harfrust-admission/src/gpos_single_tests.rs").read_bytes()),
        "prior_fifteen_matrices_reused_after_exact_patch_and_frozen_output_checks": args.reuse_frozen_oracles,
        "oracle_sources_sha256": {n: h.sha((h.REPO / f"tests/consumers/harfrust-admission/src/{n}_oracle.rs").read_bytes()) for n in expected},
        "fixture_sources_sha256": {**prior["fixture_sources_sha256"], "gdef": h.sha((h.REPO / "tests/consumers/harfrust-admission/src/gdef_fixtures.rs").read_bytes())},
        "production_sources_sha256": {r: h.sha((h.PRODUCTION / r).read_bytes()) for r in CHANGED},
        "shaping_admission_complete": False, "core_staged_adapter_enabled": False,
    }
    (output / "gdef-verification.json").write_text(json.dumps(receipt, indent=2) + "\n")
    print(json.dumps(receipt, indent=2))


if __name__ == "__main__":
    main()
