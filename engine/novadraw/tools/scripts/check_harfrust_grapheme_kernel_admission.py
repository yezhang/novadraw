#!/usr/bin/env python3
"""Freeze stored-continuation grouping using all unchanged ordinary matrices."""
import argparse
import difflib
import json
import os
import shutil
from pathlib import Path

import check_harfrust_buffer_admission as h
from check_harfrust_context_admission import reconstruct
from check_harfrust_decomposition_miss_admission import validated_copy
from check_harfrust_normalizer_kernel_admission import observe, verify_sources
import text_admission_build

PREFIX = "grapheme-kernel"
CHANGED = {
    "src/hb/buffer.rs", "src/hb/buffer_admission.rs", "src/hb/ot_shape.rs",
    "src/hb/ot_layout.rs", "src/hb/shape_admission.rs",
}
ADDED = {"src/hb/grapheme_kernel_tests.rs"}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--target-dir", type=Path, required=True)
    parser.add_argument("--linker", choices=["clang", "rust-lld"], default="rust-lld")
    args = parser.parse_args()
    output = h.artifact_output(args.output)
    prior = json.loads((output / "dotted-circle-kernel-verification.json").read_text())
    patches = {**prior["prior_patches"],
               "dotted-circle-kernel": prior["incremental_patch_sha256"]}
    root, original, before = reconstruct(output, patches, "text-harfrust-grapheme-kernel-")
    old_names = {str(p.relative_to(before)) for p in (before / "src").rglob("*") if p.is_file()}
    new_names = {str(p.relative_to(h.PRODUCTION)) for p in
                 (h.PRODUCTION / "src").rglob("*") if p.is_file()}
    if new_names - old_names != ADDED or old_names - new_names:
        raise SystemExit("Unexpected production source addition/removal")
    changed, lines = set(), []
    for name in sorted(new_names):
        old = (before / name).read_bytes() if name in old_names else b""
        new = (h.PRODUCTION / name).read_bytes()
        if old == new:
            continue
        changed.add(name)
        lines.extend(difflib.unified_diff(
            old.decode().splitlines(keepends=True), new.decode().splitlines(keepends=True),
            fromfile=f"a/{name}" if name in old_names else "/dev/null", tofile=f"b/{name}",
        ))
    if changed != CHANGED | ADDED:
        raise SystemExit(f"Unexpected production changes: {changed}")
    patch = output / f"harfrust-0.3.2-{PREFIX}.patch"
    patch.write_bytes("".join(lines).encode())
    applied = root / "after"
    shutil.copytree(before, applied)
    h.run(["git", "apply", "--no-index", str(patch)],
          output / f"{PREFIX}-patch-replay.log", os.environ, applied)
    verify_sources(applied)
    shutil.copy2(patch, h.REPO / "dependencies" / patch.name)
    src = h.REPO / "tests/consumers/harfrust-admission/src"
    for field, suffix in [("fixture_sources_sha256", "_fixtures.rs"),
                          ("oracle_sources_sha256", "_oracle.rs"),
                          ("consumer_sources_sha256", "")]:
        for name, checksum in prior[field].items():
            if field == "consumer_sources_sha256" and name == "rtl_tests.rs":
                # The new per-comparison record fee replaces the old singleton
                # estimate. Fixtures, cost programs and ordinary oracles freeze.
                continue
            if h.sha((src / f"{name}{suffix}").read_bytes()) != checksum:
                raise SystemExit(f"Frozen {field} changed: {name}")
    env = text_admission_build.environment(args.target_dir.resolve(), args.linker)
    build = h.REPO / f"target/text-harfrust-{PREFIX}-build"
    build.mkdir(exist_ok=True)
    for owner, library in [("original", original), ("production", h.PRODUCTION)]:
        h.manifest(build / f"lib-{owner}", library / "src/lib.rs", original / "Cargo.toml")
    private, consumer = {}, {}
    for profile, flags in [("native", []), ("portable", ["--no-default-features"])]:
        log = output / f"{PREFIX}-private-{profile}.log"
        h.run(["cargo", "+1.96.0", "test", "--offline", "--manifest-path",
               str(build / "lib-production/Cargo.toml"), "--lib", *flags], log, env)
        private[profile] = h.sha(log.read_bytes())
        log = output / f"{PREFIX}-consumer-{profile}.log"
        flags = [] if profile == "native" else ["--no-default-features", "--features", "portable"]
        h.run(["cargo", "+1.96.0", "test", "--offline", "--manifest-path",
               str(h.REPO / "tests/consumers/harfrust-admission/Cargo.toml"), "--lib", *flags],
              log, env)
        consumer[profile] = h.sha(log.read_bytes())
    signatures, counts, reused = {}, {}, []
    for key, expected in prior["observations_per_configuration"].items():
        oracle, owner, profile = key.split("/")
        log = output / f"{PREFIX}-{oracle}-{owner}-{profile}.log.gz"
        if owner == "original":
            signatures[key], counts[key] = validated_copy(
                output / f"dotted-circle-kernel-{oracle}-{owner}-{profile}.log.gz",
                log, prior["ordinary_output_signatures"][key], expected,
            )
            reused.append(key)
        else:
            h.SOURCE = src / f"{oracle}_oracle.rs"
            package = build / f"oracle-{owner}-{oracle}"
            h.manifest(package, build / f"lib-{owner}", original / "Cargo.toml", binary=True)
            manifest = package / "Cargo.toml"
            if profile == "portable":
                manifest.write_text(manifest.read_text().replace(
                    "path = " + json.dumps(str(build / f"lib-{owner}")) + " }",
                    "path = " + json.dumps(str(build / f"lib-{owner}"))
                    + ", default-features = false }",
                ))
            h.run(["cargo", "+1.96.0", "build", "--offline", "--manifest-path",
                   str(manifest), "--bin", "oracle"],
                  output / f"{PREFIX}-{oracle}-{owner}-{profile}-build.log", env)
            signatures[key], counts[key] = observe(
                args.target_dir.resolve() / "debug/oracle", log, env,
            )
        if counts[key] != expected or signatures[key] != prior["ordinary_output_signatures"][key]:
            raise SystemExit(f"Frozen ordinary output differs: {key}")
    for key, checksum in prior["reused_invisible_ordinary_signatures"].items():
        owner, profile = key.split("/")
        rows = [line[line.index(b"SPACE/"):] for line in
                (output / f"space-invisible-{owner}-{profile}.log").read_bytes().splitlines()
                if b"SPACE/" in line]
        if (len(rows) != prior["reused_invisible_ordinary_counts"][key] or
                h.sha(b"\n".join(rows) + b"\n") != checksum):
            raise SystemExit(f"Invisible ordinary output differs: {key}")
    if h.sha((src / "space_private_oracle.rs").read_bytes()) != prior["invisible_oracle_source_sha256"]:
        raise SystemExit("Frozen invisible oracle changed")
    verify_sources(applied)
    receipt = {
        **prior, "prior_patches": patches,
        "incremental_patch_sha256": h.sha(patch.read_bytes()),
        "changed_after_dotted_circle_kernel": sorted(changed),
        "total_source_files": len(new_names), "unchanged_source_files": len(new_names - changed),
        "ordinary_output_signatures": signatures, "observations_per_configuration": counts,
        "reused_original_configurations": reused,
        "private_test_log_sha256": private, "external_test_log_sha256": consumer,
        "production_sources_sha256": {
            name: h.sha((h.PRODUCTION / name).read_bytes()) for name in changed
        },
        "consumer_sources_sha256": {
            name: h.sha((src / name).read_bytes()) for name in prior["consumer_sources_sha256"]
        },
        "consumer_cost_assertion_migration": {
            "rtl_tests.rs": {"before": prior["consumer_sources_sha256"]["rtl_tests.rs"],
                             "after": h.sha((src / "rtl_tests.rs").read_bytes())},
        },
        "matrix_rationale": "Unchanged normalizer/circle matrices already exercise continuations; "
                            "focused private tests add local span, prefix, positions and rejection.",
    }
    receipt.pop("changed_after_normalizer_kernel", None)
    (output / f"{PREFIX}-verification.json").write_text(json.dumps(receipt, indent=2) + "\n")
    print(f"PASS: 22 frozen matrices, {sum(counts.values())} ordinary observations")


if __name__ == "__main__":
    main()
