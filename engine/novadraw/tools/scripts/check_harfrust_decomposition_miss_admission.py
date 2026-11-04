#!/usr/bin/env python3
"""Freeze terminal canonical b-miss after twenty-six immutable patches."""
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


def validated_copy(source, destination, signature, denominator):
    """Reuse only immutable original-version output after full byte validation."""
    digest, count = hashlib.sha256(), 0
    with gzip.open(source, "rb") as saved:
        while chunk := saved.read(1024 * 1024):
            digest.update(chunk)
            count += chunk.count(b"\n")
    if digest.hexdigest() != signature or count != denominator:
        raise SystemExit(f"Frozen original output differs: {source}")
    shutil.copy2(source, destination)
    return signature, count


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--target-dir", type=Path, required=True)
    parser.add_argument("--linker", choices=["clang", "rust-lld"], default="rust-lld")
    args = parser.parse_args()
    output = h.artifact_output(args.output)
    prior = json.loads((output / "space-verification.json").read_text())
    patches = {**prior["prior_patches"], "space": prior["incremental_patch_sha256"]}
    root, original, before = reconstruct(output, patches, "text-harfrust-decomposition-miss-")
    sources = {str(p.relative_to(before)) for p in (before / "src").rglob("*") if p.is_file()}
    actual = {str(p.relative_to(h.PRODUCTION)) for p in (h.PRODUCTION / "src").rglob("*") if p.is_file()}
    if actual != sources:
        raise SystemExit("Source addition/removal outside slice")
    changed, patch = set(), []
    for relative in sorted(sources):
        old, new = (before / relative).read_bytes(), (h.PRODUCTION / relative).read_bytes()
        if old != new:
            changed.add(relative)
            patch.extend(difflib.unified_diff(
                old.decode().splitlines(keepends=True), new.decode().splitlines(keepends=True),
                fromfile=f"a/{relative}", tofile=f"b/{relative}",
            ))
    if changed != {"src/hb/shape_admission.rs"}:
        raise SystemExit(f"Unexpected source changes: {changed}")
    path = output / "harfrust-0.3.2-decomposition-miss.patch"
    path.write_bytes("".join(patch).encode())
    applied = root / "after"
    shutil.copytree(before, applied)
    h.run(["git", "apply", "--no-index", str(path)],
          output / "decomposition-miss-patch-replay.log", os.environ, applied)
    for relative in actual:
        if (applied / relative).read_bytes() != (h.PRODUCTION / relative).read_bytes():
            raise SystemExit(f"Replay differs: {relative}")
    shutil.copy2(path, h.REPO / "dependencies" / path.name)
    env = text_admission_build.environment(args.target_dir.resolve(), args.linker)
    build = h.REPO / "target/text-harfrust-decomposition-miss-build"
    build.mkdir(exist_ok=True)
    src = h.REPO / "tests/consumers/harfrust-admission/src"
    for name, checksum in prior["fixture_sources_sha256"].items():
        if h.sha((src / f"{name}_fixtures.rs").read_bytes()) != checksum:
            raise SystemExit(f"Frozen fixture changed: {name}")
    for owner, library in [("original", original), ("production", h.PRODUCTION)]:
        h.manifest(build / f"lib-{owner}", library / "src/lib.rs", original / "Cargo.toml")
    consumer, private = {}, {}
    for profile, flags in [("native", []), ("portable", ["--no-default-features"])]:
        log = output / f"decomposition-miss-private-{profile}.log"
        h.run(["cargo", "test", "--offline", "--manifest-path",
               str(build / "lib-production/Cargo.toml"), "--lib", *flags], log, env)
        private[profile] = h.sha(log.read_bytes())
        log = output / f"decomposition-miss-consumer-{profile}.log"
        consumer_flags = [] if profile == "native" else ["--no-default-features", "--features", "portable"]
        h.run(["cargo", "test", "--offline", "--manifest-path",
               str(h.REPO / "tests/consumers/harfrust-admission/Cargo.toml"), "--lib", *consumer_flags], log, env)
        consumer[profile] = h.sha(log.read_bytes())
    expected = {key.split("/")[0]: count
                for key, count in prior["observations_per_configuration"].items()}
    # 8 kernels, 2 GDEF, 8 independent maps, 2 passes, 2 scripts,
    # 2 directions, 4 texts, 3 cluster patterns, 4 levels, 2 capacities.
    expected["decomposition_miss"] = 8 * 2 * 8 * 2 * 2 * 2 * 4 * 3 * 4 * 2
    signatures, counts, reused = {}, {}, []
    for owner in ["original", "production"]:
        for oracle, count in expected.items():
            source = src / f"{oracle}_oracle.rs"
            if oracle != "decomposition_miss" and h.sha(source.read_bytes()) != prior["oracle_sources_sha256"][oracle]:
                raise SystemExit(f"Frozen oracle changed: {oracle}")
            h.SOURCE = source
            package = build / f"oracle-{owner}-{oracle}"
            for profile in ["native", "portable"]:
                key = f"{oracle}/{owner}/{profile}"
                log = output / f"decomposition-miss-{oracle}-{owner}-{profile}.log.gz"
                if owner == "original" and oracle != "decomposition_miss":
                    signatures[key], counts[key] = validated_copy(
                        output / f"space-{oracle}-{owner}-{profile}.log.gz",
                        log, prior["ordinary_output_signatures"][key], count,
                    )
                    reused.append(key)
                    continue
                h.manifest(package, build / f"lib-{owner}", original / "Cargo.toml", binary=True)
                manifest = package / "Cargo.toml"
                if profile == "portable":
                    manifest.write_text(manifest.read_text().replace(
                        "path = " + json.dumps(str(build / f"lib-{owner}")) + " }",
                        "path = " + json.dumps(str(build / f"lib-{owner}")) + ", default-features = false }",
                    ))
                h.run(["cargo", "build", "--offline", "--manifest-path",
                       str(manifest), "--bin", "oracle"],
                      output / f"decomposition-miss-{oracle}-{owner}-{profile}-build.log", env)
                signatures[key], counts[key] = observe(
                    args.target_dir.resolve() / "debug/oracle", log, env,
                )
                if counts[key] != count:
                    raise SystemExit(f"Wrong observation count: {key}/{counts[key]}")
    for oracle in expected:
        selected = {v for k, v in signatures.items() if k.startswith(oracle + "/")}
        if len(selected) != 1:
            raise SystemExit(f"Ordinary output differs: {oracle}")
        if oracle != "decomposition_miss" and selected != {
            v for k, v in prior["ordinary_output_signatures"].items() if k.startswith(oracle + "/")
        }:
            raise SystemExit(f"Frozen signature changed: {oracle}")
    # Ordinary invisible tests have unchanged upstream kernels and test input.
    # Keep their frozen receipt separate from newly executed consumer tests.
    for key, checksum in prior["invisible_ordinary_signatures"].items():
        owner, profile = key.split("/")
        log = output / f"space-invisible-{owner}-{profile}.log"
        rows = [line[line.index(b"SPACE/"):] for line in log.read_bytes().splitlines()
                if b"SPACE/" in line]
        if len(rows) != prior["invisible_ordinary_counts"][key] or h.sha(b"\n".join(rows) + b"\n") != checksum:
            raise SystemExit(f"Frozen invisible output differs: {key}")
    if h.sha((src / "space_private_oracle.rs").read_bytes()) != prior["invisible_oracle_source_sha256"]:
        raise SystemExit("Invisible oracle changed")
    for relative in actual:
        if (applied / relative).read_bytes() != (h.PRODUCTION / relative).read_bytes():
            raise SystemExit(f"Source changed during verification: {relative}")
    receipt = {
        "archive_sha256": h.ARCHIVE_SHA, "prior_patches": patches,
        "incremental_patch_sha256": h.sha(path.read_bytes()),
        "patch_replays_and_reproduces_exact_source": True,
        "changed_after_space": sorted(changed), "unchanged_source_files": len(actual - changed),
        "total_source_files": len(actual), "ordinary_output_signatures": signatures,
        "observations_per_configuration": counts, "reused_original_configurations": reused,
        "build_configuration": "Native/portable; debug=0; incremental=0; jobs=1",
        "ordinary_output_storage": "lossless gzip; signatures/counts use decompressed bytes",
        "external_test_log_sha256": consumer, "private_test_log_sha256": private,
        "reused_invisible_ordinary_signatures": prior["invisible_ordinary_signatures"],
        "reused_invisible_ordinary_counts": prior["invisible_ordinary_counts"],
        "invisible_oracle_source_sha256": prior["invisible_oracle_source_sha256"],
        "consumer_sources_sha256": {name: h.sha((src / name).read_bytes()) for name in [
            "lib.rs", "decomposition_miss_tests.rs", "space_tests.rs", "gdef_tests.rs",
            "normalization_tests.rs", "general_scalar_tests.rs", "shape_tests.rs",
            "rtl_tests.rs", "gpos_single_tests.rs", "simple_unicode_tests.rs",
        ]},
        "oracle_sources_sha256": {name: h.sha((src / f"{name}_oracle.rs").read_bytes()) for name in expected},
        "fixture_sources_sha256": {**prior["fixture_sources_sha256"],
            "decomposition_miss": h.sha((src / "decomposition_miss_fixtures.rs").read_bytes())},
        "production_sources_sha256": {name: h.sha((h.PRODUCTION / name).read_bytes()) for name in changed},
        "shaping_admission_complete": False, "core_staged_adapter_enabled": False,
    }
    (output / "decomposition-miss-verification.json").write_text(json.dumps(receipt, indent=2) + "\n")
    print(json.dumps(receipt, indent=2))


if __name__ == "__main__":
    main()
