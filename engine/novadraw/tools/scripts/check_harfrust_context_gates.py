#!/usr/bin/env python3
"""Verify context receipts, baseline diagnostics and cross-target builds."""
import argparse
import gzip
import json
from pathlib import Path

import check_harfrust_buffer_admission as harness
from check_harfrust_context_admission import CHANGED, reconstruct
from check_harfrust_multiple_gates import warning_signatures
import text_admission_build


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--target-dir", type=Path, required=True)
    parser.add_argument("--wasm-target-dir", type=Path, required=True)
    parser.add_argument("--linker", choices=["clang", "rust-lld"], default="rust-lld")
    args = parser.parse_args()
    output = harness.artifact_output(args.output)
    verified = json.loads((output / "context-verification.json").read_text())
    for relative, checksum in verified["production_sources_sha256"].items():
        if harness.sha((harness.PRODUCTION / relative).read_bytes()) != checksum:
            raise SystemExit(f"Production changed after verification: {relative}")
    if harness.sha((output / "harfrust-0.3.2-context.patch").read_bytes()) != verified["incremental_patch_sha256"]:
        raise SystemExit("Frozen patch changed")
    if (harness.REPO / "dependencies/harfrust-0.3.2-context.patch").read_bytes() != (
            output / "harfrust-0.3.2-context.patch").read_bytes():
        raise SystemExit("Dependency patch copy differs")
    for oracle, checksum in verified["oracle_sources_sha256"].items():
        if harness.sha((harness.REPO /
            f"tests/consumers/harfrust-admission/src/{oracle}_oracle.rs").read_bytes()) != checksum:
            raise SystemExit(f"Frozen oracle source changed: {oracle}")
    for fixture, checksum in verified["fixture_sources_sha256"].items():
        if harness.sha((harness.REPO /
            f"tests/consumers/harfrust-admission/src/{fixture}_fixtures.rs").read_bytes()) != checksum:
            raise SystemExit(f"Frozen fixture changed: {fixture}")
    for key, checksum in verified["ordinary_output_signatures"].items():
        oracle, owner, profile = key.split("/")
        data = gzip.decompress((output / f"context-{oracle}-{owner}-{profile}.log.gz").read_bytes())
        if harness.sha(data) != checksum or len(data.splitlines()) != verified["observations_per_configuration"][key]:
            raise SystemExit(f"Output receipt differs: {key}")
    for profile in ["native", "portable"]:
        for kind, receipt in [("consumer", "external_test_log_sha256"), ("private", "private_test_log_sha256")]:
            if harness.sha((output / f"context-{kind}-{profile}.log").read_bytes()) != verified[receipt][profile]:
                raise SystemExit(f"Test receipt differs: {kind}/{profile}")
    for name in ["context-cost", "context-ordinary-cost"]:
        sampled = json.loads((output / f"{name}-summary.json").read_text())
        if harness.sha((harness.REPO /
            f"tests/consumers/harfrust-admission/src/{name.replace('-', '_')}.rs").read_bytes()) != sampled["source_sha256"]:
            raise SystemExit(f"Cost source differs: {name}")
        if name == "context-cost":
            if harness.sha((output / "context-cost.ndjson").read_bytes()) != sampled["raw_sha256"]:
                raise SystemExit("Cost receipt differs")
        else:
            for key, group in sampled["groups"].items():
                owner, _, _, session = key.split("/")
                if harness.sha((output / f"context-ordinary-{owner}-{session}.ndjson").read_bytes()) != group["raw_sha256"]:
                    raise SystemExit(f"Ordinary sample differs: {key}")
    _, original, before = reconstruct(output, verified["prior_patches"], "text-harfrust-context-lint-")
    package = harness.REPO / "target/text-harfrust-context-build/lib-before-context"
    harness.manifest(package, before / "src/lib.rs", original / "Cargo.toml")
    native = text_admission_build.environment(args.target_dir.resolve(), args.linker)
    wasm = text_admission_build.environment(args.wasm_target_dir.resolve(), "clang")
    wasm.pop("RUSTFLAGS", None)
    consumer = harness.REPO / "tests/consumers/harfrust-admission/Cargo.toml"
    dependency = harness.REPO / "target/text-harfrust-context-build/lib-production/Cargo.toml"
    cargo, receipts = ["cargo", "+1.96.0"], {}
    for profile, flags in [("native", []), ("portable", ["--no-default-features"])]:
        collected = {}
        for owner, manifest in [("baseline", package / "Cargo.toml"), ("production", dependency)]:
            command = cargo + ["clippy", "--offline", "--manifest-path", str(manifest),
                               "--lib", "--message-format=json", *flags]
            log = output / f"context-clippy-{owner}-{profile}.jsonl"
            harness.run(command, log, native, harness.REPO)
            collected[owner] = warning_signatures(log)
            receipts[f"clippy-{owner}-{profile}"] = {
                "command": command, "log_sha256": harness.sha(log.read_bytes()),
                "warning_signatures": collected[owner], "completed": True}
        if collected["baseline"] != collected["production"]:
            raise SystemExit(f"New dependency Clippy diagnostics: {profile}")
        receipts[f"clippy-comparison-{profile}"] = {
            "passed": True, "new_warnings": 0, "existing_warnings": len(collected["baseline"])}
    checks = [
        ("consumer-clippy-native", cargo + ["clippy", "--offline", "--manifest-path", str(consumer),
            "--lib", "--", "-D", "warnings"], native),
        ("consumer-clippy-portable", cargo + ["clippy", "--offline", "--manifest-path", str(consumer),
            "--lib", "--no-default-features", "--features", "portable", "--", "-D", "warnings"], native),
        ("cost-clippy", cargo + ["clippy", "--offline", "--manifest-path", str(consumer),
            "--bin", "context-cost", "--", "-D", "warnings"], native),
        ("ordinary-cost-clippy", cargo + ["clippy", "--offline", "--manifest-path", str(harness.REPO /
            "target/text-harfrust-context-cost/build/cost-production/Cargo.toml"),
            "--bin", "oracle", "--", "-D", "warnings"], native),
        ("core-native-check", cargo + ["check", "--offline", "-p", "novadraw", "--lib"], native),
        ("core-wasm-check", cargo + ["check", "--offline", "-p", "novadraw", "--lib",
            "--target", "wasm32-unknown-unknown"], wasm),
        ("consumer-wasm-build", cargo + ["build", "--offline", "--manifest-path", str(consumer),
            "--lib", "--no-default-features", "--features", "portable",
            "--target", "wasm32-unknown-unknown"], wasm)]
    for name, command, env in checks:
        log = output / f"context-{name}.log"
        harness.run(command, log, env, harness.REPO)
        receipts[name] = {"command": command, "log_sha256": harness.sha(log.read_bytes()), "passed": True}
    receipt = {"checks": receipts, "linker": args.linker, "wasm_executed": False,
        "production_sources_sha256": {r: harness.sha((harness.PRODUCTION / r).read_bytes()) for r in CHANGED}}
    (output / "context-gates.json").write_text(json.dumps(receipt, indent=2) + "\n")
    print(json.dumps(receipt, indent=2))


if __name__ == "__main__":
    main()
