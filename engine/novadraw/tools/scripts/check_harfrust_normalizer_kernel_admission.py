#!/usr/bin/env python3
"""Freeze shared normalization without opening unproved public mark paths."""
import argparse
import difflib
import gzip
import hashlib
import json
import os
import shutil
import subprocess
from pathlib import Path

import check_harfrust_buffer_admission as h
from check_harfrust_context_admission import reconstruct
from check_harfrust_decomposition_miss_admission import validated_copy
import text_admission_build

CHANGED = {
    "src/hb/buffer.rs", "src/hb/buffer_admission.rs",
    "src/hb/shape_admission.rs", "src/hb/ot_shape_normalize.rs",
    "src/hb/ot_shape.rs",
}
ADDED = {"src/hb/normalization_kernel_tests.rs"}


def observe(binary, log, env):
    """Keep all raw bytes while avoiding expensive maximum compression."""
    digest, count = hashlib.sha256(), 0
    with subprocess.Popen([str(binary)], stdout=subprocess.PIPE, stderr=subprocess.PIPE,
                          env=env) as child:
        with gzip.open(log, "wb", compresslevel=1) as saved:
            while chunk := child.stdout.read(1024 * 1024):
                digest.update(chunk)
                count += chunk.count(b"\n")
                saved.write(chunk)
        errors = child.stderr.read()
        if child.wait():
            raise RuntimeError(errors.decode(errors="replace"))
    checked = hashlib.sha256()
    with gzip.open(log, "rb") as saved:
        while chunk := saved.read(1024 * 1024):
            checked.update(chunk)
    if checked.digest() != digest.digest():
        raise SystemExit("Compressed output identity differs")
    return digest.hexdigest(), count


def verify_sources(replayed):
    expected = {p.relative_to(replayed) for p in (replayed / "src").rglob("*") if p.is_file()}
    actual = {p.relative_to(h.PRODUCTION) for p in (h.PRODUCTION / "src").rglob("*") if p.is_file()}
    if expected != actual:
        raise SystemExit("Production source addition/removal")
    for relative in expected:
        if (replayed / relative).read_bytes() != (h.PRODUCTION / relative).read_bytes():
            raise SystemExit(f"Production source changed: {relative}")


def validate_existing(log, signature, denominator):
    digest, count = hashlib.sha256(), 0
    with gzip.open(log, "rb") as saved:
        while chunk := saved.read(1024 * 1024):
            digest.update(chunk)
            count += chunk.count(b"\n")
    if digest.hexdigest() != signature or count != denominator:
        raise SystemExit(f"Existing output differs: {log}")
    return signature, count


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--target-dir", type=Path, required=True)
    parser.add_argument("--linker", choices=["clang", "rust-lld"], default="rust-lld")
    parser.add_argument("--format-only-reuse", action="store_true",
                        help="Reuse outputs only after exact rustfmt-only source proof")
    args = parser.parse_args()
    output = h.artifact_output(args.output)
    prior = json.loads((output / "decomposition-miss-verification.json").read_text())
    patches = {**prior["prior_patches"], "decomposition-miss": prior["incremental_patch_sha256"]}
    root, original, before = reconstruct(output, patches, "text-harfrust-normalizer-kernel-")
    formatting_receipt, formatting_proof = None, None
    if args.format_only_reuse:
        saved_receipt = output / "normalizer-kernel-before-formatting-verification.json"
        saved_patch = output / "normalizer-kernel-before-formatting.patch"
        if not saved_receipt.exists():
            shutil.copy2(output / "normalizer-kernel-verification.json", saved_receipt)
            shutil.copy2(output / "harfrust-0.3.2-normalizer-kernel.patch", saved_patch)
        formatting_receipt = json.loads(saved_receipt.read_text())
        if formatting_receipt["prior_patches"] != patches:
            raise SystemExit("Formatting proof uses a different baseline")
        src = h.REPO / "tests/consumers/harfrust-admission/src"
        # Several oracles share fixtures; validate the recorded fixture set,
        # rather than inventing a same-name fixture for every oracle.
        for name, checksum in formatting_receipt["fixture_sources_sha256"].items():
            if h.sha((src / f"{name}_fixtures.rs").read_bytes()) != checksum:
                raise SystemExit(f"Formatting reuse fixture changed: {name}")
        if h.sha(saved_patch.read_bytes()) != formatting_receipt["incremental_patch_sha256"]:
            raise SystemExit("Formatting baseline patch changed")
        prior_source = root / "before-formatting"
        shutil.copytree(before, prior_source)
        h.run(["git", "apply", "--no-index", str(saved_patch)],
              output / "normalizer-kernel-formatting-baseline-replay.log", os.environ, prior_source)
        names = {str(p.relative_to(prior_source)) for p in (prior_source / "src").rglob("*") if p.is_file()}
        current = {str(p.relative_to(h.PRODUCTION)) for p in (h.PRODUCTION / "src").rglob("*") if p.is_file()}
        if names != current:
            raise SystemExit("Source set changed during formatting")
        differences = {name for name in names
                       if (prior_source / name).read_bytes() != (h.PRODUCTION / name).read_bytes()}
        if differences != {"src/hb/shape_admission.rs"}:
            raise SystemExit(f"Not the expected formatting-only edit: {differences}")
        name = "src/hb/shape_admission.rs"
        old_bytes, new_bytes = (prior_source / name).read_bytes(), (h.PRODUCTION / name).read_bytes()
        formatted = subprocess.run(
            ["rustfmt", "+1.96.0", "--edition", "2021", "--emit", "stdout",
             "--config", "skip_children=true,disable_all_formatting=false"],
            input=old_bytes, stdout=subprocess.PIPE, stderr=subprocess.PIPE, check=True,
        ).stdout
        if formatted != new_bytes:
            raise SystemExit("New source is not exact rustfmt output of frozen source")
        formatting_proof = {
            "source": name, "before_sha256": h.sha(old_bytes), "after_sha256": h.sha(new_bytes),
            "old_patch_sha256": h.sha(saved_patch.read_bytes()),
            "exact_rustfmt_output": True,
            "unchanged_other_source_files": len(names) - 1,
        }
    sources = {str(p.relative_to(before)) for p in (before / "src").rglob("*") if p.is_file()}
    actual = {str(p.relative_to(h.PRODUCTION)) for p in (h.PRODUCTION / "src").rglob("*") if p.is_file()}
    if actual - sources != ADDED or sources - actual:
        raise SystemExit("Unexpected source addition/removal")
    changed, patch = set(), []
    for relative in sorted(actual):
        old = (before / relative).read_bytes() if relative in sources else b""
        new = (h.PRODUCTION / relative).read_bytes()
        if old == new:
            continue
        changed.add(relative)
        patch.extend(difflib.unified_diff(
            old.decode().splitlines(keepends=True), new.decode().splitlines(keepends=True),
            fromfile=f"a/{relative}" if relative in sources else "/dev/null",
            tofile=f"b/{relative}",
        ))
    if changed != CHANGED | ADDED:
        raise SystemExit(f"Unexpected changed sources: {changed}")
    path = output / "harfrust-0.3.2-normalizer-kernel.patch"
    path.write_bytes("".join(patch).encode())
    applied = root / "after"
    shutil.copytree(before, applied)
    h.run(["git", "apply", "--no-index", str(path)],
          output / "normalizer-kernel-patch-replay.log", os.environ, applied)
    verify_sources(applied)
    shutil.copy2(path, h.REPO / "dependencies" / path.name)
    env = text_admission_build.environment(args.target_dir.resolve(), args.linker)
    build = h.REPO / "target/text-harfrust-normalizer-kernel-build"
    build.mkdir(exist_ok=True)
    src = h.REPO / "tests/consumers/harfrust-admission/src"
    for name, checksum in prior["fixture_sources_sha256"].items():
        if h.sha((src / f"{name}_fixtures.rs").read_bytes()) != checksum:
            raise SystemExit(f"Frozen fixture changed: {name}")
    for owner, library in [("original", original), ("production", h.PRODUCTION)]:
        h.manifest(build / f"lib-{owner}", library / "src/lib.rs", original / "Cargo.toml")
    private, consumer = {}, {}
    for profile, flags in [("native", []), ("portable", ["--no-default-features"])]:
        log = output / f"normalizer-kernel-private-{profile}.log"
        h.run(["cargo", "+1.96.0", "test", "--offline", "--manifest-path",
               str(build / "lib-production/Cargo.toml"), "--lib", *flags], log, env)
        private[profile] = h.sha(log.read_bytes())
        log = output / f"normalizer-kernel-consumer-{profile}.log"
        flags = [] if profile == "native" else ["--no-default-features", "--features", "portable"]
        h.run(["cargo", "+1.96.0", "test", "--offline", "--manifest-path",
               str(h.REPO / "tests/consumers/harfrust-admission/Cargo.toml"), "--lib", *flags], log, env)
        consumer[profile] = h.sha(log.read_bytes())
    expected = {key.split("/")[0]: count for key, count in prior["observations_per_configuration"].items()}
    expected["normalizer_kernel"] = 8 * 2 * 8 * 2 * 5 * 2 * 10 * 3 * 4 * 2
    signatures, counts, reused = {}, {}, []
    for owner in ["original", "production"]:
        for oracle, count in expected.items():
            source = src / f"{oracle}_oracle.rs"
            if oracle != "normalizer_kernel" and h.sha(source.read_bytes()) != prior["oracle_sources_sha256"][oracle]:
                raise SystemExit(f"Frozen oracle changed: {oracle}")
            h.SOURCE = source
            package = build / f"oracle-{owner}-{oracle}"
            for profile in ["native", "portable"]:
                key = f"{oracle}/{owner}/{profile}"
                log = output / f"normalizer-kernel-{oracle}-{owner}-{profile}.log.gz"
                if formatting_receipt:
                    if h.sha(source.read_bytes()) != formatting_receipt["oracle_sources_sha256"][oracle]:
                        raise SystemExit(f"Formatting reuse input changed: {oracle}")
                    signatures[key], counts[key] = validate_existing(
                        log, formatting_receipt["ordinary_output_signatures"][key], count,
                    )
                    reused.append(key)
                    continue
                if owner == "original" and oracle != "normalizer_kernel":
                    signatures[key], counts[key] = validated_copy(
                        output / f"decomposition-miss-{oracle}-{owner}-{profile}.log.gz",
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
                h.run(["cargo", "+1.96.0", "build", "--offline", "--manifest-path",
                       str(manifest), "--bin", "oracle"],
                      output / f"normalizer-kernel-{oracle}-{owner}-{profile}-build.log", env)
                signatures[key], counts[key] = observe(args.target_dir.resolve() / "debug/oracle", log, env)
                if counts[key] != count:
                    raise SystemExit(f"Wrong observation count: {key}/{counts[key]}")
    for oracle in expected:
        selected = {v for k, v in signatures.items() if k.startswith(oracle + "/")}
        if len(selected) != 1:
            raise SystemExit(f"Ordinary output differs: {oracle}")
        if oracle != "normalizer_kernel" and selected != {
            v for k, v in prior["ordinary_output_signatures"].items() if k.startswith(oracle + "/")
        }:
            raise SystemExit(f"Frozen signature changed: {oracle}")
    for key, checksum in prior["reused_invisible_ordinary_signatures"].items():
        owner, profile = key.split("/")
        rows = [line[line.index(b"SPACE/"):] for line in
                (output / f"space-invisible-{owner}-{profile}.log").read_bytes().splitlines()
                if b"SPACE/" in line]
        if (len(rows) != prior["reused_invisible_ordinary_counts"][key] or
                h.sha(b"\n".join(rows) + b"\n") != checksum):
            raise SystemExit(f"Frozen invisible output differs: {key}")
    if h.sha((src / "space_private_oracle.rs").read_bytes()) != prior["invisible_oracle_source_sha256"]:
        raise SystemExit("Invisible oracle changed")
    verify_sources(applied)
    receipt = {
        "archive_sha256": h.ARCHIVE_SHA, "prior_patches": patches,
        "incremental_patch_sha256": h.sha(path.read_bytes()),
        "patch_replays_and_reproduces_exact_source": True,
        "changed_after_decomposition_miss": sorted(changed),
        "unchanged_source_files": len(actual - changed), "total_source_files": len(actual),
        "ordinary_output_signatures": signatures, "observations_per_configuration": counts,
        "reused_original_configurations": reused,
        "build_configuration": "Native/portable; debug=0; incremental=0; jobs=1",
        "ordinary_output_storage": "lossless gzip; signatures/counts use decompressed bytes",
        "external_test_log_sha256": consumer, "private_test_log_sha256": private,
        "reused_invisible_ordinary_signatures": prior["reused_invisible_ordinary_signatures"],
        "reused_invisible_ordinary_counts": prior["reused_invisible_ordinary_counts"],
        "invisible_oracle_source_sha256": prior["invisible_oracle_source_sha256"],
        "consumer_sources_sha256": {name: h.sha((src / name).read_bytes())
                                    for name in prior["consumer_sources_sha256"]},
        "oracle_sources_sha256": {name: h.sha((src / f"{name}_oracle.rs").read_bytes()) for name in expected},
        "fixture_sources_sha256": {**prior["fixture_sources_sha256"],
            "normalizer_kernel": h.sha((src / "normalizer_kernel_fixtures.rs").read_bytes())},
        "production_sources_sha256": {name: h.sha((h.PRODUCTION / name).read_bytes()) for name in changed},
        "shaping_admission_complete": False, "public_marks_admitted": False,
        "core_staged_adapter_enabled": False,
    }
    if formatting_receipt:
        receipt["reused_original_configurations"] = [key for key in reused if "/original/" in key]
        receipt["reused_production_ordinary_configurations"] = [key for key in reused if "/production/" in key]
        receipt["formatting_only_reuse_proof"] = formatting_proof
    (output / "normalizer-kernel-verification.json").write_text(json.dumps(receipt, indent=2) + "\n")
    print(json.dumps(receipt, indent=2))


if __name__ == "__main__":
    main()
