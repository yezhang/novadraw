#!/usr/bin/env python3
"""Freeze simple Unicode admission after twenty-one immutable patches."""
import argparse
import difflib
import json
import os
import shutil
from pathlib import Path

import check_harfrust_buffer_admission as h
from check_harfrust_context_admission import reconstruct
from check_harfrust_multiple_admission import observe
import text_admission_build

CHANGED = {"src/hb/charmap.rs", "src/hb/shape_admission.rs"}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--target-dir", type=Path, required=True)
    parser.add_argument("--linker", choices=["clang", "rust-lld"], default="rust-lld")
    args = parser.parse_args()
    output = h.artifact_output(args.output)
    prior = json.loads((output / "rtl-verification.json").read_text())
    patches = {**prior["prior_patches"], "rtl": prior["incremental_patch_sha256"]}
    root, original, before = reconstruct(output, patches, "text-harfrust-simple-unicode-")
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
        patch.extend(difflib.unified_diff(old.decode().splitlines(keepends=True),
            new.decode().splitlines(keepends=True), fromfile=f"a/{relative}", tofile=f"b/{relative}"))
    if changed != CHANGED:
        raise SystemExit("Expected changes missing")
    path = output / "harfrust-0.3.2-simple-unicode.patch"
    path.write_bytes("".join(patch).encode())
    applied = root / "after"
    shutil.copytree(before, applied)
    h.run(["git", "apply", "--no-index", str(path)], output / "simple-unicode-patch-replay.log", os.environ, applied)
    for relative in actual:
        if (applied / relative).read_bytes() != (h.PRODUCTION / relative).read_bytes():
            raise SystemExit(f"Replay differs: {relative}")
    shutil.copy2(path, h.REPO / "dependencies" / path.name)
    env = text_admission_build.environment(args.target_dir.resolve(), args.linker)
    build = h.REPO / "target/text-harfrust-simple-unicode-build"
    build.mkdir(exist_ok=True)
    expected = {key.split("/")[0]: count for key, count in prior["observations_per_configuration"].items()}
    expected["simple_unicode"] = 552960
    for name, checksum in prior["fixture_sources_sha256"].items():
        if h.sha((h.REPO / f"tests/consumers/harfrust-admission/src/{name}_fixtures.rs").read_bytes()) != checksum:
            raise SystemExit(f"Frozen fixture changed: {name}")
    signatures, counts = {}, {}
    for owner, library in [("original", original), ("production", h.PRODUCTION)]:
        lib = build / f"lib-{owner}"
        h.manifest(lib, library / "src/lib.rs", original / "Cargo.toml")
        for oracle, count in expected.items():
            source = h.REPO / f"tests/consumers/harfrust-admission/src/{oracle}_oracle.rs"
            if oracle != "simple_unicode" and h.sha(source.read_bytes()) != prior["oracle_sources_sha256"][oracle]:
                raise SystemExit(f"Frozen oracle changed: {oracle}")
            h.SOURCE = source
            package = build / f"oracle-{owner}-{oracle}"
            for profile in ["native", "portable"]:
                h.manifest(package, lib, original / "Cargo.toml", binary=True)
                manifest = package / "Cargo.toml"
                if profile == "portable":
                    manifest.write_text(manifest.read_text().replace("path = " + json.dumps(str(lib)) + " }",
                        "path = " + json.dumps(str(lib)) + ", default-features = false }"))
                h.run(["cargo", "build", "--offline", "--manifest-path", str(manifest), "--bin", "oracle"],
                    output / f"simple-unicode-{oracle}-{owner}-{profile}-build.log", env)
                key = f"{oracle}/{owner}/{profile}"
                signatures[key], counts[key] = observe(args.target_dir.resolve() / "debug/oracle",
                    output / f"simple-unicode-{oracle}-{owner}-{profile}.log.gz", env)
                if counts[key] != count:
                    raise SystemExit(f"Wrong observation count: {key}/{counts[key]}")
        if owner == "production":
            for profile, flags in [("native", []), ("portable", ["--no-default-features"])]:
                h.run(["cargo", "test", "--offline", "--manifest-path", str(lib / "Cargo.toml"),
                    "--lib", *flags], output / f"simple-unicode-private-{profile}.log", env)
    for oracle in expected:
        selected = {v for k, v in signatures.items() if k.startswith(oracle + "/")}
        if len(selected) != 1:
            raise SystemExit(f"Ordinary outputs differ: {oracle}")
        if oracle != "simple_unicode" and selected != {v for k, v in prior["ordinary_output_signatures"].items() if k.startswith(oracle + "/")}:
            raise SystemExit(f"Frozen signature changed: {oracle}")
    consumer, private = {}, {}
    for profile, flags in [("native", []), ("portable", ["--no-default-features", "--features", "portable"])]:
        log = output / f"simple-unicode-consumer-{profile}.log"
        h.run(["cargo", "test", "--offline", "--manifest-path",
            str(h.REPO / "tests/consumers/harfrust-admission/Cargo.toml"), "--lib", *flags], log, env)
        consumer[profile] = h.sha(log.read_bytes())
        private[profile] = h.sha((output / f"simple-unicode-private-{profile}.log").read_bytes())
    for relative in actual:
        if (applied / relative).read_bytes() != (h.PRODUCTION / relative).read_bytes():
            raise SystemExit(f"Source changed during verification: {relative}")
    receipt = {
        "archive_sha256": h.ARCHIVE_SHA, "prior_patches": patches,
        "incremental_patch_sha256": h.sha(path.read_bytes()),
        "patch_replays_and_reproduces_exact_source": True,
        "changed_after_rtl": sorted(CHANGED),
        "unchanged_source_files": len(sources - CHANGED), "total_source_files": len(actual),
        "ordinary_output_signatures": signatures, "observations_per_configuration": counts,
        "build_configuration": "Native/portable, debug=0, incremental=0, jobs=1",
        "ordinary_output_storage": "lossless gzip; counts/signatures use decompressed bytes",
        "external_test_log_sha256": consumer, "private_test_log_sha256": private,
        "consumer_module_source_sha256": h.sha((h.REPO / "tests/consumers/harfrust-admission/src/lib.rs").read_bytes()),
        "consumer_test_source_sha256": h.sha((h.REPO / "tests/consumers/harfrust-admission/src/simple_unicode_tests.rs").read_bytes()),
        "updated_legacy_test_source_sha256": h.sha((h.REPO / "tests/consumers/harfrust-admission/src/shape_tests.rs").read_bytes()),
        "oracle_sources_sha256": {n: h.sha((h.REPO / f"tests/consumers/harfrust-admission/src/{n}_oracle.rs").read_bytes()) for n in expected},
        "fixture_sources_sha256": {**prior["fixture_sources_sha256"], "simple_unicode": h.sha((h.REPO / "tests/consumers/harfrust-admission/src/simple_unicode_fixtures.rs").read_bytes())},
        "production_sources_sha256": {r: h.sha((h.PRODUCTION / r).read_bytes()) for r in CHANGED},
        "shaping_admission_complete": False, "core_staged_adapter_enabled": False,
    }
    (output / "simple-unicode-verification.json").write_text(json.dumps(receipt, indent=2) + "\n")
    print(json.dumps(receipt, indent=2))


if __name__ == "__main__":
    main()
