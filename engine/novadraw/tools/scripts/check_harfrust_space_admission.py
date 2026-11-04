#!/usr/bin/env python3
"""Freeze space/hyphen fallback after twenty-five immutable patches."""
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


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--target-dir", type=Path, required=True)
    parser.add_argument("--linker", choices=["clang", "rust-lld"], default="rust-lld")
    args = parser.parse_args()
    output = h.artifact_output(args.output)
    prior = json.loads((output / "normalization-verification.json").read_text())
    patches = {**prior["prior_patches"], "normalization": prior["incremental_patch_sha256"]}
    root, original, before = reconstruct(output, patches, "text-harfrust-space-")
    sources = {str(p.relative_to(before)) for p in (before / "src").rglob("*") if p.is_file()}
    actual = {str(p.relative_to(h.PRODUCTION)) for p in (h.PRODUCTION / "src").rglob("*") if p.is_file()}
    if actual != sources:
        raise SystemExit("Source addition/removal outside slice")
    changed, patch = set(), []
    for relative in sorted(sources):
        old, new = (before / relative).read_bytes(), (h.PRODUCTION / relative).read_bytes()
        if old == new:
            continue
        changed.add(relative)
        patch.extend(difflib.unified_diff(old.decode().splitlines(keepends=True),
            new.decode().splitlines(keepends=True), fromfile=f"a/{relative}", tofile=f"b/{relative}"))
    if changed != {"src/hb/shape_admission.rs"}:
        raise SystemExit(f"Unexpected source changes: {changed}")
    path = output / "harfrust-0.3.2-space.patch"
    path.write_bytes("".join(patch).encode())
    applied = root / "after"
    shutil.copytree(before, applied)
    h.run(["git", "apply", "--no-index", str(path)], output / "space-patch-replay.log", os.environ, applied)
    for relative in actual:
        if (applied / relative).read_bytes() != (h.PRODUCTION / relative).read_bytes():
            raise SystemExit(f"Replay differs: {relative}")
    shutil.copy2(path, h.REPO / "dependencies" / path.name)
    env = text_admission_build.environment(args.target_dir.resolve(), args.linker)
    build = h.REPO / "target/text-harfrust-space-build"
    build.mkdir(exist_ok=True)
    src = h.REPO / "tests/consumers/harfrust-admission/src"
    for name, checksum in prior["fixture_sources_sha256"].items():
        if h.sha((src / f"{name}_fixtures.rs").read_bytes()) != checksum:
            raise SystemExit(f"Frozen fixture changed: {name}")
    for owner, library in [("original", original), ("production", h.PRODUCTION)]:
        h.manifest(build / f"lib-{owner}", library / "src/lib.rs", original / "Cargo.toml")
    invisible_signatures, invisible_counts = {}, {}
    for owner, library in [("original", original), ("production", h.PRODUCTION)]:
        # UnicodeBuffer.invisible has no public setter. Inject this ordinary
        # reference only into temporary library copies, never frozen sources.
        copied = root / f"invisible-{owner}"
        shutil.copytree(library, copied)
        lib = copied / "src/lib.rs"
        lib.write_text(lib.read_text() + "\n#[cfg(test)]\n#[path = " +
            json.dumps(str(src / "space_private_oracle.rs")) + "]\nmod space_private_oracle;\n")
        package = build / f"invisible-{owner}"
        h.manifest(package, lib, original / "Cargo.toml")
        for profile, flags in [("native", []), ("portable", ["--no-default-features"])]:
            log = output / f"space-invisible-{owner}-{profile}.log"
            h.run(["cargo", "test", "--offline", "--manifest-path", str(package / "Cargo.toml"),
                "--lib", *flags, "space_private_oracle::ordinary_invisible_reference",
                "--", "--nocapture", "--test-threads=1"], log, env)
            rows = []
            for line in log.read_bytes().splitlines():
                # libtest may prefix the first row with the test name.
                if b"SPACE/" in line:
                    rows.append(line[line.index(b"SPACE/"):])
            if len(rows) != 40:
                raise SystemExit(f"Wrong invisible denominator: {owner}/{profile}/{len(rows)}")
            key = f"{owner}/{profile}"
            invisible_signatures[key] = h.sha(b"\n".join(rows) + b"\n")
            invisible_counts[key] = len(rows)
    if len(set(invisible_signatures.values())) != 1:
        raise SystemExit("Ordinary invisible result differs")
    consumer, private = {}, {}
    for profile, flags in [("native", []), ("portable", ["--no-default-features"])]:
        log = output / f"space-private-{profile}.log"
        h.run(["cargo", "test", "--offline", "--manifest-path",
            str(build / "lib-production/Cargo.toml"), "--lib", *flags], log, env)
        private[profile] = h.sha(log.read_bytes())
        log = output / f"space-consumer-{profile}.log"
        consumer_flags = [] if profile == "native" else ["--no-default-features", "--features", "portable"]
        h.run(["cargo", "test", "--offline", "--manifest-path",
            str(h.REPO / "tests/consumers/harfrust-admission/Cargo.toml"), "--lib", *consumer_flags], log, env)
        consumer[profile] = h.sha(log.read_bytes())
    expected = {key.split("/")[0]: count for key, count in prior["observations_per_configuration"].items()}
    # 8 kernels * 2 GDEF * 10 maps * 2 passes * 2 scripts * 2 directions
    # * 4 texts * 3 cluster patterns * 4 levels * 2 capacities.
    expected["space"] = 122880
    signatures, counts = {}, {}
    for owner in ["original", "production"]:
        for oracle, count in expected.items():
            source = src / f"{oracle}_oracle.rs"
            if oracle != "space" and h.sha(source.read_bytes()) != prior["oracle_sources_sha256"][oracle]:
                raise SystemExit(f"Frozen oracle changed: {oracle}")
            h.SOURCE = source
            package = build / f"oracle-{owner}-{oracle}"
            for profile in ["native", "portable"]:
                h.manifest(package, build / f"lib-{owner}", original / "Cargo.toml", binary=True)
                manifest = package / "Cargo.toml"
                if profile == "portable":
                    manifest.write_text(manifest.read_text().replace(
                        "path = " + json.dumps(str(build / f"lib-{owner}")) + " }",
                        "path = " + json.dumps(str(build / f"lib-{owner}")) + ", default-features = false }"))
                h.run(["cargo", "build", "--offline", "--manifest-path", str(manifest), "--bin", "oracle"],
                    output / f"space-{oracle}-{owner}-{profile}-build.log", env)
                key = f"{oracle}/{owner}/{profile}"
                signatures[key], counts[key] = observe(args.target_dir.resolve() / "debug/oracle",
                    output / f"space-{oracle}-{owner}-{profile}.log.gz", env)
                if counts[key] != count:
                    raise SystemExit(f"Wrong observation count: {key}/{counts[key]}")
    for oracle in expected:
        selected = {v for k, v in signatures.items() if k.startswith(oracle + "/")}
        if len(selected) != 1:
            raise SystemExit(f"Ordinary output differs: {oracle}")
        if oracle != "space" and selected != {v for k, v in prior["ordinary_output_signatures"].items() if k.startswith(oracle + "/")}:
            raise SystemExit(f"Frozen signature changed: {oracle}")
    for relative in actual:
        if (applied / relative).read_bytes() != (h.PRODUCTION / relative).read_bytes():
            raise SystemExit(f"Source changed during verification: {relative}")
    receipt = {
        "archive_sha256": h.ARCHIVE_SHA, "prior_patches": patches,
        "incremental_patch_sha256": h.sha(path.read_bytes()),
        "patch_replays_and_reproduces_exact_source": True,
        "changed_after_normalization": sorted(changed), "unchanged_source_files": len(actual - changed),
        "total_source_files": len(actual), "ordinary_output_signatures": signatures,
        "observations_per_configuration": counts,
        "build_configuration": "Native/portable; debug=0; incremental=0; jobs=1",
        "ordinary_output_storage": "lossless gzip; signatures/counts use decompressed bytes",
        "external_test_log_sha256": consumer, "private_test_log_sha256": private,
        "invisible_ordinary_signatures": invisible_signatures,
        "invisible_ordinary_counts": invisible_counts,
        "invisible_oracle_source_sha256": h.sha((src / "space_private_oracle.rs").read_bytes()),
        "consumer_sources_sha256": {name: h.sha((src / name).read_bytes())
            for name in ["lib.rs", "space_tests.rs", "gdef_tests.rs", "normalization_tests.rs", "general_scalar_tests.rs",
                "shape_tests.rs", "rtl_tests.rs", "gpos_single_tests.rs", "simple_unicode_tests.rs"]},
        "oracle_sources_sha256": {name: h.sha((src / f"{name}_oracle.rs").read_bytes()) for name in expected},
        "fixture_sources_sha256": {**prior["fixture_sources_sha256"],
            "space": h.sha((src / "space_fixtures.rs").read_bytes())},
        "production_sources_sha256": {name: h.sha((h.PRODUCTION / name).read_bytes()) for name in changed},
        "shaping_admission_complete": False, "core_staged_adapter_enabled": False,
    }
    (output / "space-verification.json").write_text(json.dumps(receipt, indent=2) + "\n")
    print(json.dumps(receipt, indent=2))


if __name__ == "__main__":
    main()
