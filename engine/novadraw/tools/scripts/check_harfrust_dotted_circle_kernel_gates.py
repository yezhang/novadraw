#!/usr/bin/env python3
"""Validate final circle evidence and run Native/portable/Wasm boundary checks."""
import argparse
import json
import os
from pathlib import Path

import check_harfrust_buffer_admission as h
from check_harfrust_context_admission import reconstruct
from check_harfrust_dotted_circle_kernel_cost import MODES, PREFIX
from check_harfrust_multiple_gates import warning_signatures
from check_harfrust_normalizer_kernel_admission import validate_existing, verify_sources
import text_admission_build


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--target-dir", type=Path, required=True)
    parser.add_argument("--wasm-target-dir", type=Path, required=True)
    parser.add_argument("--linker", choices=["clang", "rust-lld"], default="rust-lld")
    args = parser.parse_args()
    output = h.artifact_output(args.output)
    verified = json.loads((output / f"{PREFIX}-verification.json").read_text())
    src = h.REPO / "tests/consumers/harfrust-admission/src"
    patch = output / f"harfrust-0.3.2-{PREFIX}.patch"
    if (h.sha(patch.read_bytes()) != verified["incremental_patch_sha256"] or
            patch.read_bytes() != (h.REPO / "dependencies" / patch.name).read_bytes()):
        raise SystemExit("Frozen circle patch changed")
    for name, checksum in verified["consumer_sources_sha256"].items():
        if h.sha((src / name).read_bytes()) != checksum:
            raise SystemExit(f"Consumer changed: {name}")
    for kind, field in [("oracle", "oracle_sources_sha256"), ("fixtures", "fixture_sources_sha256")]:
        for name, checksum in verified[field].items():
            if h.sha((src / f"{name}_{kind}.rs").read_bytes()) != checksum:
                raise SystemExit(f"Frozen {kind} changed: {name}")
    for key, checksum in verified["ordinary_output_signatures"].items():
        oracle, owner, profile = key.split("/")
        validate_existing(output / f"{PREFIX}-{oracle}-{owner}-{profile}.log.gz",
                          checksum, verified["observations_per_configuration"][key])
    for key, checksum in verified["reused_invisible_ordinary_signatures"].items():
        owner, profile = key.split("/")
        rows = [line[line.index(b"SPACE/"):] for line in
                (output / f"space-invisible-{owner}-{profile}.log").read_bytes().splitlines()
                if b"SPACE/" in line]
        if (len(rows) != verified["reused_invisible_ordinary_counts"][key] or
                h.sha(b"\n".join(rows) + b"\n") != checksum):
            raise SystemExit(f"Invisible output receipt differs: {key}")
    if h.sha((src / "space_private_oracle.rs").read_bytes()) != verified["invisible_oracle_source_sha256"]:
        raise SystemExit("Invisible oracle changed")
    for profile in ["native", "portable"]:
        for kind, field in [("private", "private_test_log_sha256"),
                            ("consumer", "external_test_log_sha256")]:
            if h.sha((output / f"{PREFIX}-{kind}-{profile}.log").read_bytes()) != verified[field][profile]:
                raise SystemExit(f"Test receipt changed: {kind}/{profile}")
    raw_receipts, lint_receipts = {}, {}
    for mode, (source, fixture) in MODES.items():
        sampled = json.loads((output / f"{PREFIX}-{mode}-cost-summary.json").read_text())
        if (h.sha((src / source).read_bytes()) != sampled["source_sha256"] or
                h.sha((src / fixture).read_bytes()) != sampled["fixture_sha256"]):
            raise SystemExit(f"Sampled input changed: {mode}")
        if sampled["production_sources_sha256"] != verified["production_sources_sha256"]:
            raise SystemExit(f"Sampled production differs: {mode}")
        for key, checksum in sampled["clippy_before_sampling"].items():
            if h.sha((output / f"{PREFIX}-{key}-cost-clippy.log").read_bytes()) != checksum:
                raise SystemExit(f"Pre-sampling Clippy changed: {key}")
            lint_receipts[key] = checksum
        if mode == "admission":
            log = output / f"{PREFIX}-admission-cost.ndjson"
            if h.sha(log.read_bytes()) != sampled["raw_sha256"]:
                raise SystemExit("Admission raw samples changed")
            raw_receipts[log.name] = sampled["raw_sha256"]
        else:
            for key, group in sampled["groups"].items():
                owner, _, _, session = key.split("/")
                log = output / f"{PREFIX}-{mode}-{owner}-{session}.ndjson"
                if log.name in raw_receipts:
                    if raw_receipts[log.name] != group["raw_sha256"]:
                        raise SystemExit("Inconsistent ordinary sample receipts")
                    continue
                if h.sha(log.read_bytes()) != group["raw_sha256"]:
                    raise SystemExit(f"Ordinary raw samples changed: {key}")
                raw_receipts[log.name] = group["raw_sha256"]
    _, original, replayed = reconstruct(output, verified["prior_patches"],
                                       "text-harfrust-dotted-circle-gates-")
    build = h.REPO / f"target/text-harfrust-{PREFIX}-build"
    baseline = build / "lib-before-dotted-circle"
    h.manifest(baseline, replayed / "src/lib.rs", original / "Cargo.toml")
    native = text_admission_build.environment(args.target_dir.resolve(), args.linker)
    wasm = text_admission_build.environment(args.wasm_target_dir.resolve(), "clang")
    wasm.pop("RUSTFLAGS", None)
    cargo, checks = ["cargo"], {}
    for profile, flags in [("native", []), ("portable", ["--no-default-features"])]:
        signatures = {}
        for owner, package in [("baseline", baseline), ("production", build / "lib-production")]:
            log = output / f"{PREFIX}-clippy-{owner}-{profile}.jsonl"
            h.run(cargo + ["clippy", "--offline", "--manifest-path", str(package / "Cargo.toml"),
                          "--lib", "--message-format=json", *flags], log, native, h.REPO)
            signatures[owner] = warning_signatures(log)
            checks[f"clippy-{owner}-{profile}"] = {
                "passed": True, "warning_signatures": signatures[owner],
                "log_sha256": h.sha(log.read_bytes()),
            }
        if signatures["baseline"] != signatures["production"]:
            raise SystemExit(f"New library Clippy diagnostics: {profile}")
        checks[f"clippy-comparison-{profile}"] = {
            "passed": True, "new_warnings": 0, "existing_warnings": len(signatures["baseline"]),
        }
    h.run(["git", "apply", "--no-index", str(patch)], output / f"{PREFIX}-gates-replay.log",
          os.environ, replayed)
    verify_sources(replayed)
    consumer = h.REPO / "tests/consumers/harfrust-admission/Cargo.toml"
    commands = [
        ("consumer-clippy-native", cargo + ["clippy", "--offline", "--manifest-path",
         str(consumer), "--lib", "--", "-D", "warnings"], native),
        ("consumer-clippy-portable", cargo + ["clippy", "--offline", "--manifest-path",
         str(consumer), "--lib", "--no-default-features", "--features", "portable",
         "--", "-D", "warnings"], native),
        ("core-native-check", cargo + ["check", "--offline", "-p", "novadraw", "--lib"], native),
        ("core-wasm-check", cargo + ["check", "--offline", "-p", "novadraw", "--lib",
         "--target", "wasm32-unknown-unknown"], wasm),
        ("consumer-wasm-build", cargo + ["build", "--offline", "--manifest-path",
         str(consumer), "--lib", "--no-default-features", "--features", "portable",
         "--target", "wasm32-unknown-unknown"], wasm),
    ]
    for name, command, env in commands:
        log = output / f"{PREFIX}-{name}.log"
        h.run(command, log, env, h.REPO)
        checks[name] = {"command": command, "passed": True, "log_sha256": h.sha(log.read_bytes())}
    verify_sources(replayed)
    receipt = {
        "checks": checks, "linker": args.linker, "wasm_executed": False,
        "all_production_source_bytes_replayed": True,
        "production_sources_sha256": verified["production_sources_sha256"],
        "consumer_sources_sha256": verified["consumer_sources_sha256"],
        "pre_sampling_clippy_rechecked": lint_receipts, "raw_samples_rechecked": raw_receipts,
        "public_marks_admitted": False, "core_staged_adapter_enabled": False,
    }
    (output / f"{PREFIX}-gates.json").write_text(json.dumps(receipt, indent=2) + "\n")
    print("PASS: eleven boundary checks, seven pre-sampling Clippy receipts, all raw receipts")


if __name__ == "__main__":
    main()
