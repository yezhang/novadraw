#!/usr/bin/env python3
"""Verify shared-normalizer receipts and cross-target gates."""
import argparse
import gzip
import hashlib
import json
import os
import shutil
import subprocess
from pathlib import Path

import check_harfrust_buffer_admission as h
from check_harfrust_context_admission import reconstruct
from check_harfrust_multiple_gates import warning_signatures
from check_harfrust_normalizer_kernel_admission import verify_sources
import text_admission_build


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--target-dir", type=Path, required=True)
    parser.add_argument("--wasm-target-dir", type=Path, required=True)
    parser.add_argument("--linker", choices=["clang", "rust-lld"], default="rust-lld")
    args = parser.parse_args()
    output = h.artifact_output(args.output)
    verified = json.loads((output / "normalizer-kernel-verification.json").read_text())
    src = h.REPO / "tests/consumers/harfrust-admission/src"
    if h.sha((src / "space_private_oracle.rs").read_bytes()) != verified["invisible_oracle_source_sha256"]:
        raise SystemExit("Invisible oracle changed")
    for key, checksum in verified["reused_invisible_ordinary_signatures"].items():
        owner, profile = key.split("/")
        rows = [line[line.index(b"SPACE/"):] for line in
                (output / f"space-invisible-{owner}-{profile}.log").read_bytes().splitlines()
                if b"SPACE/" in line]
        if (len(rows) != verified["reused_invisible_ordinary_counts"][key] or
                h.sha(b"\n".join(rows) + b"\n") != checksum):
            raise SystemExit(f"Invisible receipt differs: {key}")
    for name, checksum in verified["consumer_sources_sha256"].items():
        if h.sha((src / name).read_bytes()) != checksum:
            raise SystemExit(f"Consumer changed: {name}")
    patch = output / "harfrust-0.3.2-normalizer-kernel.patch"
    if (h.sha(patch.read_bytes()) != verified["incremental_patch_sha256"] or
            patch.read_bytes() != (h.REPO / "dependencies" / patch.name).read_bytes()):
        raise SystemExit("Frozen patch differs")
    for kind, field in [("oracle", "oracle_sources_sha256"), ("fixtures", "fixture_sources_sha256")]:
        for name, checksum in verified[field].items():
            if h.sha((src / f"{name}_{kind}.rs").read_bytes()) != checksum:
                raise SystemExit(f"Frozen {kind} changed: {name}")
    for key, checksum in verified["ordinary_output_signatures"].items():
        oracle, owner, profile = key.split("/")
        digest, count = hashlib.sha256(), 0
        with gzip.open(output / f"normalizer-kernel-{oracle}-{owner}-{profile}.log.gz", "rb") as saved:
            while chunk := saved.read(1024 * 1024):
                digest.update(chunk)
                count += chunk.count(b"\n")
        if digest.hexdigest() != checksum or count != verified["observations_per_configuration"][key]:
            raise SystemExit(f"Output receipt differs: {key}")
    for profile in ["native", "portable"]:
        for kind, field in [("consumer", "external_test_log_sha256"), ("private", "private_test_log_sha256")]:
            if h.sha((output / f"normalizer-kernel-{kind}-{profile}.log").read_bytes()) != verified[field][profile]:
                raise SystemExit(f"Test receipt differs: {kind}/{profile}")
    for mode, source in [("cost", "decomposition_miss_cost.rs"),
                         ("ordinary-cost", "decomposition_miss_ordinary_cost.rs"),
                         ("marks-cost", "normalizer_kernel_ordinary_cost.rs")]:
        sampled = json.loads((output / f"normalizer-kernel-{mode}-summary.json").read_text())
        if h.sha((src / source).read_bytes()) != sampled["source_sha256"]:
            raise SystemExit(f"Cost source changed: {mode}")
        fixture_field = "input_sha256" if mode == "ordinary-cost" else "fixture_sha256"
        fixture = "normalizer_kernel_fixtures.rs" if mode == "marks-cost" else "decomposition_miss_fixtures.rs"
        if h.sha((src / fixture).read_bytes()) != sampled[fixture_field]:
            raise SystemExit(f"Cost fixture changed: {mode}")
        if sampled["production_sources_sha256"] != verified["production_sources_sha256"]:
            raise SystemExit(f"Cost production differs: {mode}")
        for key, checksum in sampled["clippy_before_sampling"].items():
            if h.sha((output / f"normalizer-kernel-{key}-cost-clippy.log").read_bytes()) != checksum:
                raise SystemExit(f"Pre-sampling Clippy receipt differs: {key}")
        if mode in ["ordinary-cost", "marks-cost"]:
            prefix = "marks" if mode == "marks-cost" else "ordinary"
            for key, group in sampled["groups"].items():
                owner, _, _, session = key.split("/")
                if h.sha((output / f"normalizer-kernel-{prefix}-{owner}-{session}.ndjson").read_bytes()) != group["raw_sha256"]:
                    raise SystemExit(f"Ordinary sample differs: {key}")
        elif h.sha((output / "normalizer-kernel-cost.ndjson").read_bytes()) != sampled["raw_sha256"]:
            raise SystemExit("Admission sample differs")
    formatting = verified.get("formatting_only_reuse_proof")
    if formatting:
        old = json.loads((output / "normalizer-kernel-before-formatting-cost-summary.json").read_text())
        new = json.loads((output / "normalizer-kernel-cost-summary.json").read_text())
        fields = ["work", "allocation", "calls", "glyphs", "mapping_work", "unicode_work"]
        if set(old["groups"]) != set(new["groups"]) or any(
            old["groups"][key][field] != new["groups"][key][field]
            for key in new["groups"] for field in fields
        ):
            raise SystemExit("Formatting-only edit changed sampled operation costs")
    _, original, replayed = reconstruct(output, verified["prior_patches"], "text-harfrust-normalizer-kernel-gates-")
    if formatting:
        old_patch = output / "normalizer-kernel-before-formatting.patch"
        if h.sha(old_patch.read_bytes()) != formatting["old_patch_sha256"]:
            raise SystemExit("Formatting proof baseline changed")
        old_source = replayed.parent / "formatting-source"
        shutil.copytree(replayed, old_source)
        h.run(["git", "apply", "--no-index", str(old_patch)],
              output / "normalizer-kernel-formatting-gates-replay.log", os.environ, old_source)
        name = formatting["source"]
        old_bytes = (old_source / name).read_bytes()
        new_bytes = (h.PRODUCTION / name).read_bytes()
        old_names = {p.relative_to(old_source) for p in (old_source / "src").rglob("*") if p.is_file()}
        new_names = {p.relative_to(h.PRODUCTION) for p in (h.PRODUCTION / "src").rglob("*") if p.is_file()}
        if old_names != new_names or any(
            (old_source / relative).read_bytes() != (h.PRODUCTION / relative).read_bytes()
            for relative in old_names if str(relative) != name
        ):
            raise SystemExit("Non-formatting source changed from formatting baseline")
        formatted = subprocess.run(
            ["rustfmt", "+1.96.0", "--edition", "2021", "--emit", "stdout",
             "--config", "skip_children=true,disable_all_formatting=false"],
            input=old_bytes, stdout=subprocess.PIPE, stderr=subprocess.PIPE, check=True,
        ).stdout
        if (h.sha(old_bytes) != formatting["before_sha256"] or
                h.sha(new_bytes) != formatting["after_sha256"] or formatted != new_bytes):
            raise SystemExit("Formatting proof does not match frozen sources")
    _, _, baseline_source = reconstruct(output, verified["prior_patches"], "text-harfrust-normalizer-kernel-baseline-")
    h.run(["git", "apply", "--no-index", str(patch)], output / "normalizer-kernel-gates-replay.log",
          os.environ, replayed)
    verify_sources(replayed)
    build = h.REPO / "target/text-harfrust-normalizer-kernel-build"
    baseline = build / "lib-before-normalizer-kernel"
    h.manifest(baseline, baseline_source / "src/lib.rs", original / "Cargo.toml")
    native = text_admission_build.environment(args.target_dir.resolve(), args.linker)
    wasm = text_admission_build.environment(args.wasm_target_dir.resolve(), "clang")
    wasm.pop("RUSTFLAGS", None)
    cargo, checks = ["cargo", "+1.96.0"], {}
    for profile, flags in [("native", []), ("portable", ["--no-default-features"])]:
        signatures = {}
        for owner, package in [("baseline", baseline), ("production", build / "lib-production")]:
            log = output / f"normalizer-kernel-clippy-{owner}-{profile}.jsonl"
            h.run(cargo + ["clippy", "--offline", "--manifest-path", str(package / "Cargo.toml"),
                          "--lib", "--message-format=json", *flags], log, native, h.REPO)
            signatures[owner] = warning_signatures(log)
            checks[f"clippy-{owner}-{profile}"] = {
                "passed": True, "warning_signatures": signatures[owner], "log_sha256": h.sha(log.read_bytes()),
            }
        if signatures["baseline"] != signatures["production"]:
            raise SystemExit(f"New Clippy diagnostics: {profile}")
        checks[f"clippy-comparison-{profile}"] = {
            "passed": True, "new_warnings": 0, "existing_warnings": len(signatures["baseline"]),
        }
    consumer = h.REPO / "tests/consumers/harfrust-admission/Cargo.toml"
    cost = h.REPO / "target/text-harfrust-normalizer-kernel-cost"
    commands = [
        ("consumer-clippy-native", cargo + ["clippy", "--offline", "--manifest-path", str(consumer), "--lib", "--", "-D", "warnings"], native),
        ("consumer-clippy-portable", cargo + ["clippy", "--offline", "--manifest-path", str(consumer), "--lib", "--no-default-features", "--features", "portable", "--", "-D", "warnings"], native),
        ("cost-clippy", cargo + ["clippy", "--offline", "--manifest-path", str(cost / "admission-production/Cargo.toml"), "--bin", "oracle", "--", "-D", "warnings"], native),
        ("ordinary-cost-clippy", cargo + ["clippy", "--offline", "--manifest-path", str(cost / "ordinary-production/Cargo.toml"), "--bin", "oracle", "--", "-D", "warnings"], native),
        ("marks-cost-clippy", cargo + ["clippy", "--offline", "--manifest-path", str(cost / "marks-production/Cargo.toml"), "--bin", "oracle", "--", "-D", "warnings"], native),
        ("core-native-check", cargo + ["check", "--offline", "-p", "novadraw", "--lib"], native),
        ("core-wasm-check", cargo + ["check", "--offline", "-p", "novadraw", "--lib", "--target", "wasm32-unknown-unknown"], wasm),
        ("consumer-wasm-build", cargo + ["build", "--offline", "--manifest-path", str(consumer), "--lib", "--no-default-features", "--features", "portable", "--target", "wasm32-unknown-unknown"], wasm),
    ]
    for name, command, env in commands:
        log = output / f"normalizer-kernel-{name}.log"
        h.run(command, log, env, h.REPO)
        checks[name] = {"command": command, "passed": True, "log_sha256": h.sha(log.read_bytes())}
    verify_sources(replayed)
    receipt = {
        "checks": checks, "linker": args.linker, "wasm_executed": False,
        "all_production_source_bytes_replayed": True,
        "production_sources_sha256": verified["production_sources_sha256"],
        "consumer_sources_sha256": verified["consumer_sources_sha256"],
        "formatting_proof_rechecked": bool(formatting),
        "formatting_sampled_costs_unchanged": bool(formatting),
    }
    (output / "normalizer-kernel-gates.json").write_text(json.dumps(receipt, indent=2) + "\n")
    print(json.dumps(receipt, indent=2))


if __name__ == "__main__":
    main()
